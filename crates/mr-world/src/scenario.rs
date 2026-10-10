//! Declarative scenes: a world generator, lighting, a camera path and scripted events,
//! all deterministic functions of the frame index so every renderer sees the same thing.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, bail};
use glam::{DVec3, IVec3, Vec3};
use mr_core::renderer::BlockEdit;
use mr_core::{BlockId, BlockRegistry, Camera, Environment, Light, Sun};
use serde::{Deserialize, Serialize};

use crate::World;
use crate::generators::GenSpec;
use crate::noise::{hash3, unit};

type V3 = [f64; 3];

fn v(a: V3) -> DVec3 {
    DVec3::from_array(a)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    pub generator: GenSpec,
    #[serde(default)]
    pub environment: EnvSpec,
    #[serde(default)]
    pub lights: Vec<LightSpec>,
    pub camera: CameraPathSpec,
    #[serde(default = "default_fov")]
    pub fov_deg: f32,
    #[serde(default)]
    pub events: Vec<EventSpec>,
}

fn default_fov() -> f32 {
    70.0
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvSpec {
    pub sky: Option<[f32; 3]>,
    pub ambient: Option<[f32; 3]>,
    /// Direction the sunlight travels, or `false`/absent for the default.
    pub sun_direction: Option<[f32; 3]>,
    pub sun_color: Option<[f32; 3]>,
    #[serde(default)]
    pub no_sun: bool,
    pub exposure: Option<f32>,
}

impl EnvSpec {
    pub fn build(&self) -> Environment {
        let mut e = Environment::default();
        if let Some(s) = self.sky {
            e.sky = Vec3::from_array(s);
        }
        if let Some(a) = self.ambient {
            e.ambient = Vec3::from_array(a);
        }
        if let Some(x) = self.exposure {
            e.exposure = x;
        }
        if self.no_sun {
            e.sun = None;
        } else if self.sun_direction.is_some() || self.sun_color.is_some() {
            let d = self.sun_direction.map(Vec3::from_array).unwrap_or(Vec3::new(-0.4, -1.0, -0.25));
            let c = self.sun_color.map(Vec3::from_array).unwrap_or(Vec3::ONE);
            e.sun = Some(Sun { direction: d.normalize(), color: c });
        }
        e
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LightSpec {
    Point {
        position: V3,
        #[serde(default = "white")]
        color: [f32; 3],
        #[serde(default = "default_intensity")]
        intensity: f32,
        #[serde(default = "default_radius")]
        radius: f32,
    },
    /// `count` lights scattered in the box `center +- extent`. With `on_surface` each light
    /// is dropped onto the first solid block below its random position and lifted by 1.5.
    Scatter {
        count: u32,
        #[serde(default)]
        seed: u64,
        center: Option<V3>,
        extent: V3,
        color: Option<[f32; 3]>,
        #[serde(default = "default_intensity")]
        intensity: f32,
        #[serde(default = "default_radius")]
        radius: f32,
        #[serde(default)]
        on_surface: bool,
    },
}

fn white() -> [f32; 3] {
    [1.0; 3]
}
fn default_intensity() -> f32 {
    20.0
}
fn default_radius() -> f32 {
    16.0
}

/// Camera paths are parametrised over the run length (`t = frame / frames`), except
/// periodic ones which use their own period in frames.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CameraPathSpec {
    Static {
        position: Option<V3>,
        target: Option<V3>,
        #[serde(default)]
        yaw_deg: f32,
        #[serde(default)]
        pitch_deg: f32,
    },
    Orbit {
        center: V3,
        radius: f64,
        #[serde(default)]
        height: f64,
        period: u32,
    },
    Line {
        from: Option<V3>,
        to: V3,
        look_at: Option<V3>,
    },
    /// Smooth (Catmull-Rom) path through `points`, looking along the direction of travel.
    Flythrough { points: Vec<V3> },
    /// Rotates in place: worst case for anything caching view-dependent work.
    Spin {
        position: Option<V3>,
        period: u32,
        #[serde(default)]
        pitch_deg: f32,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventSpec {
    /// Fill (or dig, with `block = "air"`) a sphere at a given frame.
    Sphere { frame: u32, center: V3, radius: f64, block: String },
    /// Fill an axis-aligned box `[min, max]` (inclusive) at a given frame.
    Box { frame: u32, min: [i32; 3], max: [i32; 3], block: String },
    /// Every `every` frames, set `count` random blocks within `radius` of `center`
    /// (default: the camera). `block = "toggle"` flips air/solid.
    RandomEdits {
        #[serde(default = "one")]
        every: u32,
        count: u32,
        #[serde(default)]
        seed: u64,
        center: Option<V3>,
        radius: f64,
        block: String,
    },
    /// Lights wander around their spawn position on Lissajous curves.
    AnimateLights { amplitude: f64, period: u32 },
    /// The sun rotates around the Z axis once every `period` frames.
    SunCycle { period: u32 },
}

fn one() -> u32 {
    1
}

/// Changes produced by the scene for one frame.
#[derive(Default, Debug)]
pub struct FrameEvents {
    pub edits: Vec<BlockEdit>,
    pub lights: Vec<Light>,
    pub environment: Option<Environment>,
}

/// Runtime of a [`SceneSpec`].
pub struct Scene {
    pub spec: SceneSpec,
    pub blocks: Arc<BlockRegistry>,
    pub environment: Environment,
    base_lights: Vec<Light>,
    spawn: DVec3,
    frames: u32,
}

impl Scene {
    /// `frames` is the run length used to parametrise non-periodic camera paths.
    pub fn new(spec: SceneSpec, blocks: Arc<BlockRegistry>, world: &mut World, frames: u32) -> anyhow::Result<Self> {
        let spawn = world.generator.spawn();
        let environment = spec.environment.build();
        let mut s = Self { spec, blocks, environment, base_lights: vec![], spawn, frames: frames.max(1) };
        s.base_lights = s.build_lights(world)?;
        for e in &s.spec.events {
            if let EventSpec::Sphere { block, .. } | EventSpec::Box { block, .. } | EventSpec::RandomEdits { block, .. } = e {
                if block != "toggle" {
                    s.block_id(block)?;
                }
            }
        }
        Ok(s)
    }

    fn block_id(&self, name: &str) -> anyhow::Result<BlockId> {
        if name == "air" {
            return Ok(BlockId::AIR);
        }
        self.blocks.find(name).with_context(|| format!("unknown block `{name}`"))
    }

    pub fn initial_lights(&self) -> &[Light] {
        &self.base_lights
    }

    fn build_lights(&self, world: &mut World) -> anyhow::Result<Vec<Light>> {
        let mut out = vec![];
        for spec in &self.spec.lights {
            match spec {
                LightSpec::Point { position, color, intensity, radius } => out.push(Light {
                    id: out.len() as u32,
                    position: v(*position),
                    color: Vec3::from_array(*color),
                    intensity: *intensity,
                    radius: *radius,
                }),
                LightSpec::Scatter { count, seed, center, extent, color, intensity, radius, on_surface } => {
                    let c = center.map(v).unwrap_or(self.spawn);
                    let ext = v(*extent);
                    let mut i = 0u32;
                    let mut placed = 0;
                    while placed < *count {
                        if i > count * 20 + 1000 {
                            bail!("could not place {count} lights (placed {placed}); is the scatter box solid?");
                        }
                        let n = i as i32;
                        let r = |k: i32| unit(hash3(*seed, n, k, 0x11)) as f64 * 2.0 - 1.0;
                        i += 1;
                        let mut p = c + DVec3::new(r(0), r(1), r(2)) * ext;
                        if *on_surface {
                            let top = (c.y + ext.y) as i32;
                            let bottom = (c.y - ext.y) as i32;
                            let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
                            let Some(y) = (bottom..=top).rev().find(|&y| world.block(IVec3::new(x, y, z)).is_solid()) else {
                                continue;
                            };
                            if world.block(IVec3::new(x, y + 1, z)).is_solid() {
                                continue;
                            }
                            p.y = y as f64 + 2.5;
                        } else if world.block(p.floor().as_ivec3()).is_solid() {
                            continue;
                        }
                        let col = color.map(Vec3::from_array).unwrap_or_else(|| {
                            let h = hash3(*seed, i as i32, 0, 0x22);
                            Vec3::new(1.0, 0.55 + unit(h) * 0.4, 0.25 + unit(h >> 16) * 0.5)
                        });
                        out.push(Light { id: out.len() as u32, position: p, color: col, intensity: *intensity, radius: *radius });
                        placed += 1;
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn camera(&self, frame: u32) -> Camera {
        let fov = self.spec.fov_deg;
        let t = frame as f64 / self.frames as f64;
        let mut cam = match &self.spec.camera {
            CameraPathSpec::Static { position, target, yaw_deg, pitch_deg } => {
                let p = position.map(v).unwrap_or(self.spawn);
                match target {
                    Some(tg) => Camera::look_at(p, v(*tg), fov),
                    None => Camera { position: p, yaw: yaw_deg.to_radians(), pitch: pitch_deg.to_radians(), ..Default::default() },
                }
            }
            CameraPathSpec::Orbit { center, radius, height, period } => {
                let a = std::f64::consts::TAU * frame as f64 / *period.max(&1) as f64;
                let c = v(*center);
                let p = c + DVec3::new(a.sin() * radius, *height, a.cos() * radius);
                Camera::look_at(p, c, fov)
            }
            CameraPathSpec::Line { from, to, look_at } => {
                let a = from.map(v).unwrap_or(self.spawn);
                let b = v(*to);
                let p = a.lerp(b, t);
                Camera::look_at(p, look_at.map(v).unwrap_or(b + (b - a)), fov)
            }
            CameraPathSpec::Flythrough { points } => {
                let pts: Vec<DVec3> = points.iter().copied().map(v).collect();
                let p = catmull_rom(&pts, t);
                let ahead = catmull_rom(&pts, (t + 0.002).min(1.0));
                let ahead = if (ahead - p).length() < 1e-6 { p + (p - catmull_rom(&pts, t - 0.002)) } else { ahead };
                Camera::look_at(p, ahead, fov)
            }
            CameraPathSpec::Spin { position, period, pitch_deg } => Camera {
                position: position.map(v).unwrap_or(self.spawn),
                yaw: std::f32::consts::TAU * frame as f32 / *period.max(&1) as f32,
                pitch: pitch_deg.to_radians(),
                ..Default::default()
            },
        };
        cam.fov_y = fov.to_radians();
        cam
    }

    /// Scripted changes that happen at `frame` (call once per frame, in order).
    pub fn events(&mut self, frame: u32, camera: &Camera) -> anyhow::Result<FrameEvents> {
        let mut ev = FrameEvents::default();
        for e in self.spec.events.clone() {
            match e {
                EventSpec::Sphere { frame: f, center, radius, block } if f == frame => {
                    let b = self.block_id(&block)?;
                    let c = v(center);
                    let r = radius.ceil() as i32;
                    let ci = c.floor().as_ivec3();
                    for z in -r..=r {
                        for y in -r..=r {
                            for x in -r..=r {
                                let p = ci + IVec3::new(x, y, z);
                                if (p.as_dvec3() + 0.5 - c).length() <= radius {
                                    ev.edits.push(BlockEdit { pos: p, block: b });
                                }
                            }
                        }
                    }
                }
                EventSpec::Box { frame: f, min, max, block } if f == frame => {
                    let b = self.block_id(&block)?;
                    for z in min[2]..=max[2] {
                        for y in min[1]..=max[1] {
                            for x in min[0]..=max[0] {
                                ev.edits.push(BlockEdit { pos: IVec3::new(x, y, z), block: b });
                            }
                        }
                    }
                }
                EventSpec::RandomEdits { every, count, seed, center, radius, block } if frame % every.max(1) == 0 => {
                    let toggle = block == "toggle";
                    let b = if toggle { BlockId::AIR } else { self.block_id(&block)? };
                    let c = center.map(v).unwrap_or(camera.position);
                    for i in 0..count {
                        let r = |k: i32| unit(hash3(seed ^ frame as u64, i as i32, k, 0x33)) as f64 * 2.0 - 1.0;
                        let p = (c + DVec3::new(r(0), r(1), r(2)) * radius).floor().as_ivec3();
                        let block = if toggle {
                            // Toggling needs the current state; resolved by the host against its world.
                            BlockId(u16::MAX)
                        } else {
                            b
                        };
                        ev.edits.push(BlockEdit { pos: p, block });
                    }
                }
                EventSpec::AnimateLights { amplitude, period } => {
                    let a = std::f64::consts::TAU * frame as f64 / period.max(1) as f64;
                    ev.lights = self
                        .base_lights
                        .iter()
                        .map(|l| {
                            let ph = l.id as f64 * 1.618;
                            let off = DVec3::new((a + ph).sin(), (a * 0.7 + ph).sin() * 0.3, (a * 1.3 + ph).cos());
                            Light { position: l.position + off * amplitude, ..*l }
                        })
                        .collect();
                }
                EventSpec::SunCycle { period } => {
                    let a = std::f32::consts::TAU * frame as f32 / period.max(1) as f32;
                    let mut env = self.environment;
                    let color = env.sun.map(|s| s.color).unwrap_or(Vec3::ONE);
                    env.sun = Some(Sun { direction: Vec3::new(a.sin(), -a.cos(), -0.3).normalize(), color });
                    ev.environment = Some(env);
                }
                _ => {}
            }
        }
        Ok(ev)
    }
}

fn catmull_rom(p: &[DVec3], t: f64) -> DVec3 {
    match p.len() {
        0 => DVec3::ZERO,
        1 => p[0],
        n => {
            let t = t.clamp(0.0, 1.0) * (n - 1) as f64;
            let i = (t.floor() as usize).min(n - 2);
            let f = t - i as f64;
            let get = |k: isize| p[k.clamp(0, n as isize - 1) as usize];
            let (p0, p1, p2, p3) = (get(i as isize - 1), get(i as isize), get(i as isize + 1), get(i as isize + 2));
            let f2 = f * f;
            let f3 = f2 * f;
            0.5 * (2.0 * p1 + (p2 - p0) * f + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * f2 + (3.0 * p1 - p0 - 3.0 * p2 + p3) * f3)
        }
    }
}

/// Resolves `toggle` edits (block id `u16::MAX`) against the current world state, and
/// removes duplicate positions (last write wins).
pub fn resolve_edits(world: &mut World, edits: Vec<BlockEdit>) -> Vec<BlockEdit> {
    let mut last: HashMap<IVec3, BlockId> = HashMap::new();
    let mut order = vec![];
    for e in edits {
        let b = if e.block.0 == u16::MAX {
            if world.block(e.pos).is_solid() { BlockId::AIR } else { crate::textures::blocks::STONE }
        } else {
            e.block
        };
        if last.insert(e.pos, b).is_none() {
            order.push(e.pos);
        }
    }
    order.into_iter().map(|p| BlockEdit { pos: p, block: last[&p] }).collect()
}
