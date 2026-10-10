//! Scene features: cheap, renderer-independent measurements of a scenario (how much world
//! is in range, how solid and how open it is, how the camera moves, what changes), stored
//! with every observation. A planner that learns this machine (IDEAS §14) can then relate
//! performance to properties of a scene instead of to scenario names, and so generalise
//! to scenes it has not measured.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{DVec3, IVec3};
use mr_core::{BlockRegistry, CHUNK_SIZE, ChunkData, ChunkKey, WorldUpdate};
use mr_world::{Scene, SceneSpec, World};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::stream::Streamer;

/// Frames sampled for the occupancy and ray measurements: first, middle, last.
const SAMPLES: usize = 3;
/// Ray grid per sampled frame (16:9).
const RAYS: (u32, u32) = (32, 18);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneFeatures {
    /// Camera speed in blocks per frame (mean, max) and turn rate in degrees per frame
    /// (mean, max), over the measured frames.
    pub cam_speed: f64,
    pub cam_speed_max: f64,
    pub cam_turn_deg: f64,
    pub cam_turn_deg_max: f64,
    /// Non-empty LOD-0 chunks within the view radius at frame 0, and non-empty chunks newly
    /// in range per later frame (mean, max): the streaming load.
    pub chunks_initial: f64,
    pub chunk_loads: f64,
    pub chunk_loads_max: f64,
    /// Within the view radius (averaged over sampled frames): the share of chunks in the
    /// cube that are not air, of voxels that are solid, and of voxels that are solid with an
    /// air neighbour (a surface measure).
    pub chunk_fill: f64,
    pub solid_fraction: f64,
    pub surface_fraction: f64,
    /// What the camera sees (a ray grid per sampled frame, up to the view distance): the
    /// share of rays that hit nothing, and the mean and median distance of the hits in
    /// blocks.
    pub sky_fraction: f64,
    pub hit_distance: f64,
    pub hit_distance_median: f64,
    /// Scripted block edits per frame (mean, max), lights, whether lights or the sun move,
    /// and mesh triangles placed in the scene (all instances).
    pub edits: f64,
    pub edits_max: f64,
    pub lights: u32,
    pub lights_animated: bool,
    pub sun_moves: bool,
    pub mesh_triangles: u64,
}

impl SceneFeatures {
    /// Measures `spec` over `frames` frames at view radius `radius` (chunks), against
    /// `world` (chunks are generated as needed and stay cached for the run).
    pub fn measure(spec: &SceneSpec, blocks: Arc<BlockRegistry>, world: &mut World, frames: u32, radius: u32) -> anyhow::Result<Self> {
        let mut scene = Scene::new(spec.clone(), blocks, world, frames)?;
        let mut f = SceneFeatures { lights: scene.initial_lights().len() as u32, ..Default::default() };
        let cams: Vec<_> = (0..frames).map(|i| scene.camera(i)).collect();

        // Camera motion.
        let (mut speed, mut turn) = (vec![], vec![]);
        for w in cams.windows(2) {
            speed.push((w[1].position - w[0].position).length());
            turn.push(w[0].forward().angle_between(w[1].forward()).to_degrees() as f64);
        }
        (f.cam_speed, f.cam_speed_max) = mean_max(&speed);
        (f.cam_turn_deg, f.cam_turn_deg_max) = mean_max(&turn);

        // Streaming load.
        let mut s = Streamer::new(radius, 0, None);
        let mut out = vec![];
        let mut loads = vec![];
        for (i, c) in cams.iter().enumerate() {
            out.clear();
            s.update(world, c.position, &mut out);
            let n = out.iter().filter(|u| matches!(u, WorldUpdate::ChunkLoad { .. })).count() as f64;
            if i == 0 {
                f.chunks_initial = n;
            } else {
                loads.push(n);
            }
        }
        (f.chunk_loads, f.chunk_loads_max) = mean_max(&loads);

        // Occupancy and what the camera sees, at sampled frames.
        let picks: Vec<usize> = (0..SAMPLES).map(|k| k * (cams.len().max(1) - 1) / (SAMPLES - 1)).collect();
        let (mut fill, mut solid, mut surface) = (0.0, 0.0, 0.0);
        let mut hits = vec![];
        let mut rays = 0usize;
        let max_dist = (radius as i32 * CHUNK_SIZE) as f64;
        for &i in &picks {
            let c = &cams[i];
            let (a, b, d) = occupancy(world, c.position, radius as i32);
            (fill, solid, surface) = (fill + a, solid + b, surface + d);
            for y in 0..RAYS.1 {
                for x in 0..RAYS.0 {
                    let dir = c.ray_dir(x, y, RAYS.0, RAYS.1).as_dvec3();
                    rays += 1;
                    if let Some((cell, _)) = world.raycast(c.position, dir, max_dist) {
                        hits.push((cell.as_dvec3() + 0.5 - c.position).length());
                    }
                }
            }
        }
        let n = picks.len() as f64;
        (f.chunk_fill, f.solid_fraction, f.surface_fraction) = (fill / n, solid / n, surface / n);
        f.sky_fraction = 1.0 - hits.len() as f64 / rays.max(1) as f64;
        f.hit_distance = mean_max(&hits).0;
        hits.sort_by(f64::total_cmp);
        f.hit_distance_median = hits.get(hits.len() / 2).copied().unwrap_or(0.0);

        // Events and content.
        let mut edits = vec![];
        for (i, c) in cams.iter().enumerate() {
            let ev = scene.events(i as u32, c)?;
            edits.push(ev.edits.len() as f64);
            f.lights_animated |= !ev.lights.is_empty();
            f.sun_moves |= ev.environment.is_some();
        }
        (f.edits, f.edits_max) = mean_max(&edits);
        let tris: std::collections::HashMap<u32, u64> = scene.mesh_data().iter().map(|m| (m.id, m.indices.len() as u64 / 3)).collect();
        f.mesh_triangles = scene.mesh_instances(0).iter().map(|i| tris.get(&i.mesh).copied().unwrap_or(0)).sum();
        Ok(f)
    }

    /// A one-line description for reports.
    pub fn describe(&self) -> String {
        format!(
            "camera {:.2} blocks/frame (max {:.1}), turn {:.2}°/frame; {:.0} chunks at start, {:.1} loads/frame; chunk fill {:.0}%, solid {:.1}%, surface {:.2}%; sky {:.0}%, hits at {:.0} blocks (median {:.0}); {:.1} edits/frame; {} lights{}{}; {} mesh triangles",
            self.cam_speed,
            self.cam_speed_max,
            self.cam_turn_deg,
            self.chunks_initial,
            self.chunk_loads,
            100.0 * self.chunk_fill,
            100.0 * self.solid_fraction,
            100.0 * self.surface_fraction,
            100.0 * self.sky_fraction,
            self.hit_distance,
            self.hit_distance_median,
            self.edits,
            self.lights,
            if self.lights_animated { " (animated)" } else { "" },
            if self.sun_moves { ", moving sun" } else { "" },
            self.mesh_triangles
        )
    }
}

fn mean_max(v: &[f64]) -> (f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0);
    }
    (v.iter().sum::<f64>() / v.len() as f64, v.iter().copied().fold(f64::MIN, f64::max))
}

/// (non-air chunk share, solid voxel share, surface voxel share) of the LOD-0 chunk cube of
/// radius `r` around `pos`. A surface voxel is solid with an air neighbour, across chunk
/// borders too (neighbours outside the cube count as solid).
fn occupancy(world: &mut World, pos: DVec3, r: i32) -> (f64, f64, f64) {
    let center = ChunkKey::of_block(pos.floor().as_ivec3()).pos;
    let mut keys = vec![];
    for z in -r..=r {
        for y in -r..=r {
            for x in -r..=r {
                let key = ChunkKey::lod0(center + IVec3::new(x, y, z));
                if world.may_contain_blocks(key) {
                    keys.push(key);
                }
            }
        }
    }
    let total = ((2 * r + 1) as f64).powi(3);
    let data = world.get_many(&keys);
    let chunks: HashMap<IVec3, ChunkData> = keys.iter().map(|k| k.pos).zip(data).collect();
    let n = CHUNK_SIZE;
    // Whether absolute block `p` (in chunk `c` or a neighbour) is air.
    let air = |c: IVec3, p: IVec3| -> bool {
        let q = c * n + p;
        let ck = q >> mr_core::chunk::CHUNK_BITS as i32;
        if (ck - center).abs().max_element() > r {
            return false;
        }
        chunks.get(&ck).is_none_or(|d| d.get(q & (n - 1)).is_air())
    };
    let dirs = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
    let exposed = |c: IVec3, p: IVec3| dirs.iter().any(|&d| air(c, p + d));
    let (filled, solid, surface) = chunks
        .par_iter()
        .map(|(&c, d)| match d {
            ChunkData::Uniform(b) if b.is_air() => (0u64, 0u64, 0u64),
            ChunkData::Uniform(_) => {
                // Only the border layer can touch air.
                let mut e = 0u64;
                for z in 0..n {
                    for y in 0..n {
                        for x in 0..n {
                            let p = IVec3::new(x, y, z);
                            if (p.min_element() == 0 || p.max_element() == n - 1) && exposed(c, p) {
                                e += 1;
                            }
                        }
                    }
                }
                (1, mr_core::CHUNK_VOLUME as u64, e)
            }
            ChunkData::Dense(dc) => {
                let (mut s, mut e) = (0u64, 0u64);
                for z in 0..n {
                    for y in 0..n {
                        for x in 0..n {
                            let p = IVec3::new(x, y, z);
                            if dc.get(p).is_air() {
                                continue;
                            }
                            s += 1;
                            e += exposed(c, p) as u64;
                        }
                    }
                }
                (1, s, e)
            }
        })
        .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));
    let voxels = total * mr_core::CHUNK_VOLUME as f64;
    (filled as f64 / total, solid as f64 / voxels, surface as f64 / voxels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mr_world::textures::default_registry;

    /// A static camera over flat ground: no motion, no loads after the first frame, about
    /// half the rays hit the ground, and the ground's top layer is surface.
    #[test]
    fn flat_static() {
        let text = "generator = { type = \"flat\" }\ncamera = { type = \"static\", position = [0, 20, 0], target = [100, 12, 100] }\n";
        let spec: SceneSpec = toml::from_str(text).unwrap();
        let mut world = World::new(spec.generator.build());
        let f = SceneFeatures::measure(&spec, Arc::new(default_registry()), &mut world, 10, 3).unwrap();
        assert_eq!((f.cam_speed, f.cam_turn_deg, f.chunk_loads, f.edits), (0.0, 0.0, 0.0, 0.0));
        assert!(f.chunks_initial > 0.0 && f.chunk_fill > 0.0 && f.solid_fraction > 0.0 && f.surface_fraction > 0.0);
        assert!(f.sky_fraction > 0.0 && f.sky_fraction < 1.0, "{}", f.describe());
        assert!(f.hit_distance > 0.0);
    }
}
