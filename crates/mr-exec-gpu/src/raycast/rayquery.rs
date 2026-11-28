//! The ray-query caster (`rayquery-gpu`; the primary rays' kernel API `rayquery`): hardware ray
//! tracing (RT cores) through wgpu ray queries, assembled from components (`mr-components`):
//!
//! * **geometry** decides what goes into each chunk's BLAS: `greedy` / `naive` quads as
//!   triangles (traversal and intersection entirely in fixed-function hardware), or `boxes`,
//!   one AABB per occupied 4^3 brick with a small software DDA inside each candidate
//!   (procedural geometry);
//! * **visibility** `raycast` with the **traversal** kernel `rayquery`: primary, shadow and
//!   bounce rays are ray queries;
//! * **shading**: the same `shade_surface` as the other back ends (`direct`, `pathtrace`);
//! * **meshes** `composite` (optional): the meshes' hit records rasterised first
//!   (`mr_gpu::meshes::MeshRecords`, as the walk does), each primary ray query ends at its
//!   pixel's mesh hit, and where no voxel is nearer the mesh's surface is shaded.
//!
//! The TLAS is rebuilt every frame with instances translated relative to the camera's
//! block, so precision never degrades far from the origin. Replaces gpu-rtx and
//! gpu-rtx-bricks.

use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state as shade_state;
use mr_components::storage::gpu_data::Bound;
use mr_components::traversal;
use mr_core::*;
use mr_gpu::meshes::{MESH_RECORDS_WGSL, MeshRecords};
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{GpuNeeds, wgpu};
use serde::Deserialize;

use super::{Cast, CastProgram, Meshed, Ticket};
use crate::accel::AccelSpec;
use crate::{Core, CoreSpec};

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::GPU_RAYQUERY;


pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu".into(),
        description: "GPU hardware ray tracing (wgpu ray queries; RT cores): one BLAS per chunk from the geometry component (triangles or brick boxes), a TLAS of chunks translated to the camera block each frame; primary, shadow and bounce rays are ray queries".into(),
        tags: vec!["gpu".into(), "raytrace".into(), "hardware-rt".into(), "modular".into(), "wgpu".into()],
        features: Features::of(&[Feature::Lit, Feature::Volume, Feature::Meshes, Feature::MeshRays]),
        params: REGION.lighting_knobs(params()),
    }
}


fn params() -> Vec<ParamSpec> {
    vec![
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
        REGION.placement_param("inline"),
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

/// The region's own knobs (the nodes' are read from the graph).
#[derive(Deserialize)]
#[serde(default)]
struct Options {
    /// Prefer fast BVH builds over fast traversal (helps edit-heavy scenes).
    fast_build: bool,
    build_budget: usize,
    /// Voxel pool for the boxes geometry.
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { fast_build: false, build_budget: 0, pool_mb: 2048, frames_in_flight: 3 }
    }
}

/// The geometry node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct GeometryKnobs {
    cull: bool,
    /// Triangle facing culled for primary rays.
    cull_faces: Cull,
}

impl Default for GeometryKnobs {
    fn default() -> Self {
        Self { cull: true, cull_faces: Cull::Back }
    }
}

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    placement: String,
    shadows: Shadows,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { placement: "inline".into(), shadows: Shadows::All }
    }
}


/// The ray-query caster's own part: its sites (the primary rays a ray query, the shading's
/// rays ray queries or walks), the primary rays' facing culled. The acceleration structures
/// are the core's.
pub(crate) struct RayQuery {
    walks: Walks,
    shadow: usize,
    bounce: usize,
    /// The mesh-shadow rays' site (the meshes' `shadow`, else the shadow rays').
    mesh_shadow: usize,
    /// The meshes stage draws meshes (`composite`); `built`: what the compiled program has.
    meshes: bool,
    built: bool,
    /// The meshes' hit records, rasterised before the primary rays (mesh mode).
    records: MeshRecords,
    /// The meshes node (its timed stage).
    stage_nodes: Vec<String>,
    /// Triangle facing culled for primary rays.
    cull_faces: Cull,
    /// The walks' storages' bind groups this frame.
    bound: Bound,
}

/// The storage groups of walks at the shading's sites (3 is the mesh records').
const STORAGE_GROUPS: &[u32] = &[2, 4, 5, 6];

impl RayQuery {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self, String, Option<crate::remote::Far>)> {
        let opts: Options = p.options()?;
        let geometry_id = g.find(Role::Geometry).ok_or_else(|| anyhow::anyhow!("no geometry node"))?;
        let geometry = g.node(geometry_id).component.clone();
        let gk: GeometryKnobs = g.knobs(geometry_id).map_err(anyhow::Error::msg)?;
        let shading = g.node(g.shading()).component.clone();
        let k: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
        let mut walks = Walks::of(g);
        let site = |role, port| g.site(role, port).ok_or_else(|| anyhow::anyhow!("{} `{port}` rays are not bound", role.name()));
        let primary = walks.site(g, site(Role::Visibility, "primary")?, STORAGE_GROUPS, false);
        anyhow::ensure!(primary == 0 && walks.is_query(0), "the primary rays are ray queries");
        let shadow = match super::shading_site(g, "shadow") {
            Some(_) => walks.site(g, site(Role::Shading, "shadow")?, STORAGE_GROUPS, false),
            None => primary,
        };
        let bounce = match super::shading_site(g, "bounce") {
            Some(Some(s)) => walks.site(g, s, STORAGE_GROUPS, false),
            _ => shadow,
        };
        let mesh_shadow = g.site(Role::Meshes, "shadow").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
        let meshes = g.find(Role::Meshes).is_some_and(|id| g.node(id).component != "none");
        let accel = AccelSpec { geometry, cull: gk.cull, fast_build: opts.fast_build, build_budget: opts.build_budget, gates: crate::accel::gates_of(g, geometry_id) };
        let spec = CoreSpec {
                gpu: GpuNeeds { experimental: true, required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY, optional_features: wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS },
                walks: &walks,
                accel: Some(accel),
                stages: wgpu::ShaderStages::COMPUTE,
                max_lod: 0,
                voxels: false,
                hits: mr_components::shading_deferred(g, &k.placement)?.then_some(crate::exec::Form::Records),
                shading: &shading,
                shadows: k.shadows,
                // Walks at the shading's sites (ray queries need no step limit).
                max_steps: 2048,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
        };
        let core = Core::new(p, devices, waker.clone(), g, spec.clone())?;
        let far = crate::remote::create(p, devices, waker, g, &core, &spec)?;
        tracing::info!("rayquery-gpu: primary {}, shadow {}, bounce {}, shading {shading}", walks.label(0), walks.label(shadow), walks.label(bounce));
        let records = MeshRecords::new(&core.gpu, p.width, p.height);
        let rq = Self { walks, shadow, bounce, mesh_shadow, meshes, built: meshes, records, stage_nodes: g.ids(&[Role::Meshes]), cull_faces: gk.cull_faces, bound: Bound(Vec::new()) };
        Ok((core, rq, shading, far))
    }
}

impl Cast for RayQuery {
    const REGION: &'static mr_components::graph::Builds = REGION;
    const RT_FLAGS: &'static str = "cam.ray_flags";

    fn layout_entries(&self, _stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry> {
        vec![]
    }

    fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![]
    }

    fn layouts<'a>(&'a self, core: &'a Core, [layout0, world]: [&'a wgpu::BindGroupLayout; 2]) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
        core.storages.layouts(&[Some(layout0), Some(world), None, Some(self.records.layout())])
    }

    fn program(&self, shading: &str, meshed: Meshed) -> CastProgram {
        program(&self.walks, [self.shadow, self.bounce, self.mesh_shadow], shading, meshed)
    }

    fn stale(&self) -> bool {
        self.meshes != self.built
    }

    fn compiled(&mut self) {
        self.built = self.meshes;
    }

    fn meshes(&self) -> bool {
        self.meshes
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.records.meshes.apply(update);
    }

    fn encode_meshes(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, first: bool) -> bool {
        let want = req.meshes && self.built;
        let on = self.records.active(want);
        let ts = if on { core.ring.render_timestamps(ticket, first, false) } else { None };
        if on {
            core.ring.stage(enc, ticket, "meshes", &self.stage_nodes);
        }
        self.records.encode(&core.gpu, enc, &req.camera, core.width as f32 / core.height as f32, req.mode, want, ts);
        on
    }

    fn surfaces(&self) -> Option<crate::cross::Part> {
        Some(crate::cross::Part::Texture(self.records.records().clone()))
    }

    /// The mesh records: compiled into the program, rebuilt at the next frame.
    fn switch(&mut self, s: &mr_components::graph::Switch) -> Option<anyhow::Result<()>> {
        match s.role {
            Role::Meshes => self.meshes = s.component.is_some(),
            _ => return None,
        }
        Some(Ok(()))
    }

    fn camera(&self) -> [u32; 2] {
        let cull = match self.cull_faces {
            Cull::Back => 0x10,
            Cull::Front => 0x20,
            Cull::None => 0,
        };
        [0, if self.walks.geometry.as_ref().is_some_and(|g| g.component != "boxes") { cull } else { 0 }]
    }

    fn updated(&mut self, _core: &Core, _changed: &[IVec3]) -> anyhow::Result<()> {
        Ok(())
    }

    fn storages(&mut self, core: &mut Core) {
        self.bound = core.storages.prepare(&core.gpu, &core.world);
    }

    fn groups(&self) -> impl Iterator<Item = (u32, &wgpu::BindGroup)> {
        std::iter::once((3, self.records.bind_group())).chain(self.bound.0.iter().map(|(g, b)| (*g, b)))
    }

    fn stats(&self, _core: &Core) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        (vec![], None, vec![])
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "cull_faces" => {
                self.cull_faces = match value.as_str() {
                    Some("front") => Cull::Front,
                    Some("none") => Cull::None,
                    _ => Cull::Back,
                }
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    fn resize(&mut self, core: &Core) {
        self.records.resize(&core.gpu, core.width, core.height);
    }

    fn memory_bytes(&self) -> u64 {
        0
    }
}

/// The shader: the world, lighting and the shading component as modules, the shading's ray
/// services through their sites (ray queries: virtual functions the root code overrides,
/// see `Walks::services`); the root casts the primary rays with the ray-query kernel's
/// `ctrace` over the trace the core's root code holds (`Core::bind_program`); `meshed`: with
/// the mesh records at group 3 (primary rays end at them), and with `Rays` the shading's rays
/// meeting the triangles too.
fn program(walks: &Walks, [shadow, bounce, mesh_shadow]: [usize; 3], shading: &str, meshed: Meshed) -> CastProgram {
    use mr_components::shaders;
    let mut program = mr_gpu::program::Program::world(Some("rays::shadow"));
    walks.add_to(&mut program);
    let rays = shaders::shading_rays(&mut program, walks, shadow, bounce);
    shaders::shading(&mut program, shading, &rays.iter().map(String::as_str).collect::<Vec<_>>(), 0, crate::STATE_BINDING);
    let meshes = (meshed != Meshed::No).then(|| shaders::meshes(&mut program, walks, shading, [shadow, bounce], mesh_shadow, 3, 3, meshed == Meshed::Rays));
    let geometry = walks.geometry.as_ref().map_or("-", |g| g.component.as_str());
    let records = if meshed != Meshed::No { MESH_RECORDS_WGSL } else { "" };
    let root = format!("enable wgpu_ray_query;\n// Composition: geometry = {geometry}, traversal = {}, shading = {shading}\n{records}\n{}\n{}", walks.kernels[0].component, include_str!("rayquery.wgsl"), traversal::rt_wgsl(&walks.kernels[0].component).expect("validated"));
    let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string()];
    deps.extend(meshes.into_iter().flatten());
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
        use mr_components::graph::{Change, shipped};
        let base = shipped("rayquery-gpu").unwrap();
        let (geometries, shadings) = (base.choices("geometry"), base.choices("shading"));
        assert!(geometries.len() > 1 && shadings.len() > 1, "{geometries:?} {shadings:?}");
        for geometry in geometries {
            for &shading in &shadings {
                for deferred in [false, true] {
                    let bounce = mr_components::shading::state::stateful(shading).then(|| Change::bind("shading", "bounce", "kernel"));
                    let edits: Vec<Change> = [Change::component("geometry", geometry), Change::component("shading", shading)].into_iter().chain(bounce).collect();
                    let ck = base.with_edits(&edits).unwrap().check().unwrap();
                    let mut walks = Walks::of(&ck);
                    walks.site(&ck, ck.site(Role::Visibility, "primary").unwrap(), STORAGE_GROUPS, false);
                    let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), STORAGE_GROUPS, false);
                    for meshed in [Meshed::No, Meshed::Records, Meshed::Rays] {
                        let mut lp = program(&walks, [shadow, shadow, shadow], shading, meshed);
                        lp.program.root(format!("enable wgpu_ray_query;\n{}", crate::accel::wgsl(geometry, RayQuery::RT_FLAGS)));
                        super::super::tests::validate(lp, deferred, super::super::View::Image, false, false, &format!("{geometry} {shading} {deferred} {meshed:?}"));
                    }
                }
            }
        }
    }
}
