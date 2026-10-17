//! The ray-query lane (`gpu-rt`; the primary rays' kernel API `rayquery`): hardware ray
//! tracing (RT cores) through wgpu ray queries, assembled from components (`mr-components`,
//! IDEAS §12):
//!
//! * **geometry** decides what goes into each chunk's BLAS: `greedy` / `naive` quads as
//!   triangles (traversal and intersection entirely in fixed-function hardware), or `boxes`,
//!   one AABB per occupied 4^3 brick with a small software DDA inside each candidate
//!   (procedural geometry);
//! * **visibility** `raycast` with the **traversal** kernel `rayquery`: primary, shadow and
//!   bounce rays are ray queries;
//! * **shading**: the same `shade_surface` as the other back ends (`direct`, `pathtrace`).
//!
//! The TLAS is rebuilt every frame with instances translated relative to the camera's
//! block, so precision never degrades far from the origin. Replaces gpu-rtx and
//! gpu-rtx-bricks.

use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state as shade_state;
use mr_components::storage::gpu_data::Bound;
use mr_components::{Slot, components, names_on, placement_param, require, traversal};
use mr_core::*;
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{GpuNeeds, wgpu};
use serde::Deserialize;

use super::{Before, Cast, CastProgram, Passes, Ticket};
use crate::accel::AccelSpec;
use crate::{Core, CoreSpec};

pub(crate) const BACKEND: &str = "gpu-rt";

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "GPU-RT back end: hardware ray queries over a TLAS of per-chunk BLASes built from the geometry component (triangles or brick boxes); shading components as elsewhere".into(),
        tags: vec!["gpu".into(), "raytrace".into(), "hardware-rt".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

fn describe(slot: Slot) -> String {
    components(slot).filter(|c| mr_components::unavailable(BACKEND, slot, c.name).is_none()).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::choice("geometry", &format!("Geometry component (what the BLASes hold). {}", describe(Slot::Geometry)), &names_on(BACKEND, Slot::Geometry), "greedy", false),
        ParamSpec::choice("visibility", &format!("Visibility strategy. {}", describe(Slot::Visibility)), &names_on(BACKEND, Slot::Visibility), "raycast", false),
        ParamSpec::choice("traversal", &format!("Traversal kernel. {}", describe(Slot::Traversal)), &names_on(BACKEND, Slot::Traversal), "rayquery", false),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names_on(BACKEND, Slot::Shading), "direct", true),
        ParamSpec::choice("merge", "greedy: merge faces of the same block only (`exposure` is raster-only)", &["block"], "block", false).of("geometry", &["greedy"]),
        ParamSpec::bool("cull", "naive: skip faces hidden by solid neighbours (false = every face of every solid block)", true, false).of("geometry", &["naive"]),
        ParamSpec::choice("cull_faces", "Triangle facing culled for primary rays (triangle geometries)", &["back", "front", "none"], "back", true).of("geometry", &["greedy", "naive"]),
        shadows_param(),
        ParamSpec::bool("fast_build", "Prefer fast BVH builds over fast traversal (a build flag, not code)", false, true).baked(false),
        ParamSpec::int("build_budget", "Chunk BLAS rebuilds per frame, nearest first (0 = all)", 0, 100000, 0, true),
    ]
    .into_iter()
    .chain(shade_state::params())
    .chain([
        placement_param(BACKEND, "inline"),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB; boxes geometry)", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ])
    .collect()
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Cull {
    #[default]
    Back,
    Front,
    None,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    geometry: String,
    visibility: String,
    traversal: String,
    shading: String,
    placement: String,
    merge: String,
    cull: bool,
    /// Triangle facing culled for primary rays.
    cull_faces: Cull,
    shadows: Shadows,
    /// Prefer fast BVH builds over fast traversal (helps edit-heavy scenes).
    fast_build: bool,
    build_budget: usize,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    /// Voxel pool for the boxes geometry.
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            geometry: "greedy".into(),
            visibility: "raycast".into(),
            traversal: "rayquery".into(),
            shading: "direct".into(),
            placement: "inline".into(),
            merge: "block".into(),
            cull: true,
            cull_faces: Cull::Back,
            shadows: Shadows::All,
            fast_build: false,
            build_budget: 0,
            bounces: 1,
            spp: 1,
            accumulate: true,
            pool_mb: 2048,
            frames_in_flight: 3,
        }
    }
}


/// The ray-query lane's own part: its sites (the primary rays a ray query, the shading's
/// rays ray queries or walks), the primary rays' facing culled. The acceleration structures
/// are the core's.
pub(crate) struct RayQuery {
    walks: Walks,
    shadow: usize,
    bounce: usize,
    opts: Options,
    /// The walks' storages' bind groups this frame.
    bound: Bound,
}

/// The storage groups of walks at the shading's sites.
const STORAGE_GROUPS: &[u32] = &[2, 3, 4, 5];

impl RayQuery {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self, String)> {
        let opts: Options = p.options()?;
        let geometry = require(BACKEND, Slot::Geometry, &opts.geometry)?;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        require(BACKEND, Slot::Traversal, &opts.traversal)?;
        let shading = require(BACKEND, Slot::Shading, &opts.shading)?;
        anyhow::ensure!(opts.merge == "block", "merge = `{}`: gpu-rt has no per-pixel block lookup (only `block`)", opts.merge);
        let mut walks = Walks::default();
        let site = |role, port| g.site(role, port).ok_or_else(|| anyhow::anyhow!("{BACKEND}: {} `{port}` rays are not bound", role.name()));
        let primary = walks.site(g, site(Role::Visibility, "primary")?, STORAGE_GROUPS, false);
        anyhow::ensure!(primary == 0 && walks.is_query(0), "{BACKEND}: the primary rays are ray queries");
        let shadow = walks.site(g, site(Role::Shading, "shadow")?, STORAGE_GROUPS, false);
        let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
        let accel = AccelSpec { geometry, cull: opts.cull, fast_build: opts.fast_build, build_budget: opts.build_budget };
        let core = Core::new(
            p,
            devices,
            waker,
            g,
            CoreSpec {
                gpu: GpuNeeds { experimental: true, required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY, optional_features: wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS },
                walks: &walks,
                accel: Some(accel),
                stages: wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                voxels: false,
                records: mr_components::parse_placement(&opts.placement)?,
                shading: &shading,
                bounces: opts.bounces,
                spp: opts.spp,
                accumulate: opts.accumulate,
                shadows: opts.shadows,
                // Walks at the shading's sites (ray queries need no step limit).
                max_steps: 2048,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
            },
        )?;
        tracing::info!("gpu-rt: primary {}, shadow {}, bounce {}, shading {shading}", walks.label(0), walks.label(shadow), walks.label(bounce));
        Ok((core, Self { walks, shadow, bounce, opts, bound: Bound(Vec::new()) }, shading))
    }
}

impl Cast for RayQuery {
    const BACKEND: &'static str = BACKEND;
    const RT_FLAGS: &'static str = "cam.ray_flags";

    fn layout_entries(&self, _stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry> {
        vec![]
    }

    fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![]
    }

    fn layouts<'a>(&'a self, core: &'a Core, [layout0, world]: [&'a wgpu::BindGroupLayout; 2]) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
        core.storages.layouts(&[Some(layout0), Some(world)])
    }

    fn program(&self, shading: &str, _meshed: bool) -> CastProgram {
        program(&self.walks, self.shadow, self.bounce, shading)
    }

    fn stale(&self) -> bool {
        false
    }

    fn compiled(&mut self) {}

    fn camera(&self) -> [u32; 2] {
        let cull = match self.opts.cull_faces {
            Cull::Back => 0x10,
            Cull::Front => 0x20,
            Cull::None => 0,
        };
        [0, if self.walks.geometry.as_ref().is_some_and(|g| g.component != "boxes") { cull } else { 0 }]
    }

    fn updated(&mut self, _core: &Core, _changed: &[IVec3]) -> anyhow::Result<()> {
        Ok(())
    }

    fn prepare(&mut self, _core: &mut Core, _passes: &mut Passes, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket, _req: &FrameRequest) {}

    fn before(&mut self, core: &mut Core, _passes: &Passes, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket, _req: &FrameRequest, _world: &wgpu::BindGroup) -> Before {
        self.bound = core.storages.prepare(&core.gpu, &core.world);
        Before { first: true, meshed: false }
    }

    fn groups(&self) -> impl Iterator<Item = (u32, &wgpu::BindGroup)> {
        self.bound.0.iter().map(|(g, b)| (*g, b))
    }

    fn stats(&self, _core: &Core) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        (vec![], None, vec![])
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "cull_faces" => {
                self.opts.cull_faces = match value.as_str() {
                    Some("front") => Cull::Front,
                    Some("none") => Cull::None,
                    _ => Cull::Back,
                }
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    fn resize(&mut self, _core: &Core) {}

    fn memory_bytes(&self) -> u64 {
        0
    }
}

/// The shader: the world, lighting and the shading component as modules, the shading's ray
/// services through their sites (ray queries: virtual functions the root code overrides,
/// see `Walks::services`); the root casts the primary rays with the ray-query kernel's
/// `ctrace` over the trace the core's root code holds (`Core::trace_root`).
fn program(walks: &Walks, shadow: usize, bounce: usize, shading: &str) -> CastProgram {
    use mr_components::shaders;
    let mut program = mr_gpu::program::Program::world(Some("rays::shadow"));
    walks.add_to(&mut program);
    let rays = shaders::shading_rays(&mut program, walks, shadow, bounce);
    shaders::shading(&mut program, shading, &rays.iter().map(String::as_str).collect::<Vec<_>>(), 0, crate::STATE_BINDING);
    let geometry = walks.geometry.as_ref().map_or("-", |g| g.component.as_str());
    let root = format!("enable wgpu_ray_query;\n// Composition: geometry = {geometry}, traversal = {}, shading = {shading}\n{}\n{}", walks.kernels[0].component, include_str!("rayquery.wgsl"), traversal::rt_wgsl(&walks.kernels[0].component).expect("validated"));
    let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string()];
    deps.extend(rays);
    deps.extend([mr_gpu::program::LIGHTING, mr_gpu::program::VOXELS, mr_gpu::program::COMMON].map(String::from));
    CastProgram { program, root, deps }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader composes from its modules (ray services overridden by the root's ray
    /// queries) and validates for every geometry and shading, inline and deferred (no GPU
    /// needed).
    #[test]
    fn programs_compose() {
        use mr_components::graph::Graph;
        for geometry in names_on(BACKEND, Slot::Geometry) {
            for shading in names_on(BACKEND, Slot::Shading) {
                for deferred in [false, true] {
                    let c = mr_components::Composition { geometry: geometry.into(), shading: shading.into(), ..mr_components::Composition::new(BACKEND) };
                    let ck = Graph::from_composition(&c).unwrap().check().unwrap();
                    let mut walks = Walks::default();
                    walks.site(&ck, ck.site(Role::Visibility, "primary").unwrap(), STORAGE_GROUPS, false);
                    let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), STORAGE_GROUPS, false);
                    let mut lp = program(&walks, shadow, shadow, shading);
                    lp.program.root(format!("enable wgpu_ray_query;\n{}", crate::accel::wgsl(geometry, RayQuery::RT_FLAGS)));
                    super::super::tests::validate(lp, deferred, false, false, &format!("{geometry} {shading} {deferred}"));
                }
            }
        }
    }
}
