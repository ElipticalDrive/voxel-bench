//! Mesh mode for GPU rasterisers (`mr_core::mesh`): keeps the scene's meshes and instances,
//! writes camera-relative world-space triangles each frame (meshes are small), and draws
//! them into the renderer's targets and depth.
//!
//! Meshes are a surface source (PLAN step 12d): their fragments write the hit record's mesh
//! capability (`Records`: the packed record, `Ids`: `visbuffer`'s encoding), and the pixel is
//! shaded wherever the strategy shades (the ray caster's walk, the common deferred pass),
//! decoding the triangle from the same vertices ([`surface_wgsl`]). Forward drawers shade
//! the fragments themselves in three flavours: `Fill` (depth test + write), `Depth`
//! (pre-pass, no colour) and `Equal` (shading after a pre-pass). Back faces are not culled:
//! meshes may be open or inconsistently wound.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};
use mr_core::{Camera, MeshData, MeshInstance, MeshUpdate, RenderMode};

use crate::Gpu;
use crate::voxels::VoxelWorld;

/// Seven words, as `mesh_surface.wgsl` reads them.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    color: [u8; 4],
    normal: [f32; 3],
}

/// Triangles a record can name (`MESH_TRIS` in `voxels.wgsl`: bits 1-22).
pub const MESH_TRIS: usize = 1 << 22;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MeshFrame {
    view_proj: [[f32; 4]; 4],
    cam_frac: [f32; 3],
    mode: u32,
}

/// A renderer's shadow rays for forward mesh shading: a program with the world, lighting
/// and the ray services at `rays` (`occluded()`, reading bind group 2 with `layout`, visible
/// to the fragment stage).
pub struct Occlusion<'a> {
    pub program: &'a crate::program::Program,
    pub rays: &'a str,
    pub layout: &'a wgpu::BindGroupLayout,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MeshPass {
    /// Forward: shaded, depth test and write.
    Fill,
    /// Forward: the depth prepass.
    Depth,
    /// Forward: shaded where the depth is equal.
    Equal,
    /// The packed hit record (rg32uint).
    Records,
    /// `visbuffer`'s face ids (r32uint).
    Ids,
}

struct Forward {
    fill: wgpu::RenderPipeline,
    depth: wgpu::RenderPipeline,
    equal: wgpu::RenderPipeline,
}

pub struct GpuMeshes {
    data: HashMap<u32, Arc<MeshData>>,
    instances: Vec<MeshInstance>,
    dirty: bool,
    built_for: Option<IVec3>,
    vbuf: wgpu::Buffer,
    vcount: u32,
    frame_buf: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Forward shading, when built with shadow rays.
    forward: Option<Forward>,
    records: wgpu::RenderPipeline,
    ids: wgpu::RenderPipeline,
    /// The [`Occlusion`]'s bind group 2, set by the renderer each frame.
    occlusion_bg: Option<wgpu::BindGroup>,
    warned: bool,
}

impl GpuMeshes {
    /// `world` provides bind group 1 (lighting and shadow rays); its visibility must
    /// include the fragment stage. `occlusion`: for forward shading (`Fill`, `Depth`,
    /// `Equal`), the renderer's shadow rays through its composed storage (bind group 2, see
    /// [`Self::set_occlusion_bind_group`]); record passes need none.
    pub fn new(gpu: &Gpu, world: &VoxelWorld, occlusion: Option<Occlusion>) -> Self {
        let dev = &gpu.device;
        use crate::program::{COMMON, LIGHTING, Program, VOXELS};
        let layout0 = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let frame_buf = dev.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh frame"), size: size_of::<MeshFrame>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bind = dev.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("mesh frame"), layout: &layout0, entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() }] });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x3];
        let pipeline = |label, module: &wgpu::ShaderModule, layout: &wgpu::PipelineLayout, entry: Option<&str>, target: wgpu::TextureFormat, write: bool, compare| {
            let targets = [Some(wgpu::ColorTargetState { format: target, blend: None, write_mask: wgpu::ColorWrites::ALL })];
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("mesh_vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout { array_stride: size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })],
                },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias: Default::default() }),
                multisample: Default::default(),
                fragment: entry.map(|e| wgpu::FragmentState { module, entry_point: Some(e), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: gpu.cache(),
            })
        };
        use wgpu::CompareFunction::{Equal, Greater};
        use wgpu::TextureFormat::{R32Uint, Rg32Uint, Rgba8Unorm};
        let forward = occlusion.map(|occ| {
            let mut program = occ.program.clone();
            program.define("SHADE");
            let module = crate::program::compile(dev, "meshes", &program, MESH_WGSL, &[occ.rays, LIGHTING, VOXELS, COMMON]);
            let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("meshes"), bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(occ.layout)], immediate_size: 0 });
            Forward {
                fill: pipeline("mesh fill", &module, &pl, Some("mesh_fs"), Rgba8Unorm, true, Greater),
                depth: pipeline("mesh depth", &module, &pl, None, Rgba8Unorm, true, Greater),
                equal: pipeline("mesh equal", &module, &pl, Some("mesh_fs"), Rgba8Unorm, false, Equal),
            }
        });
        let module = crate::program::compile(dev, "mesh records", &Program::world(None), MESH_WGSL, &[VOXELS, COMMON]);
        let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("mesh records"), bind_group_layouts: &[Some(&layout0)], immediate_size: 0 });
        Self {
            data: HashMap::new(),
            instances: vec![],
            dirty: false,
            built_for: None,
            vbuf: Self::vertex_buffer(gpu, 256),
            vcount: 0,
            frame_buf,
            bind,
            forward,
            records: pipeline("mesh records", &module, &pl, Some("mesh_record_fs"), Rg32Uint, true, Greater),
            ids: pipeline("mesh ids", &module, &pl, Some("mesh_id_fs"), R32Uint, true, Greater),
            occlusion_bg: None,
            warned: false,
        }
    }

    /// Vertices are drawn and, as storage, read by [`surface_wgsl`]'s decoding.
    fn vertex_buffer(gpu: &Gpu, size: u64) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh vertices"), size, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    /// The triangles' vertices (seven words each, three per triangle) for
    /// [`surface_wgsl`]'s binding. It may be replaced by [`Self::prepare`].
    pub fn vertices(&self) -> &wgpu::Buffer {
        &self.vbuf
    }

    /// The occlusion module's bind group 2 for the next draws (it may change when the
    /// renderer's buffers grow).
    pub fn set_occlusion_bind_group(&mut self, bg: &wgpu::BindGroup) {
        self.occlusion_bg = Some(bg.clone());
    }

    pub fn apply(&mut self, u: MeshUpdate) {
        match u {
            MeshUpdate::Meshes(ms) => {
                for m in ms {
                    self.data.insert(m.id, m);
                }
            }
            MeshUpdate::Instances(i) => self.instances = i,
        }
        self.dirty = true;
    }

    /// Whether any mesh instance exists (nothing to draw otherwise).
    pub fn has_instances(&self) -> bool {
        !self.instances.is_empty()
    }

    pub fn triangles(&self) -> u32 {
        self.vcount / 3
    }

    /// Updates the per-frame uniform and, if instances or the camera's block changed, the
    /// triangles. Call once per frame before drawing. Returns whether the vertex buffer was
    /// replaced (bind groups holding [`Self::vertices`] then need rebuilding).
    pub fn prepare(&mut self, gpu: &Gpu, cam: &Camera, view_proj: glam::Mat4, mode: RenderMode) -> bool {
        let (cam_block, frac) = cam.split_position();
        let f = MeshFrame { view_proj: view_proj.to_cols_array_2d(), cam_frac: frac.to_array(), mode: (mode == RenderMode::Lit) as u32 };
        gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&f));
        if !self.dirty && self.built_for == Some(cam_block) {
            return false;
        }
        self.dirty = false;
        self.built_for = Some(cam_block);
        let origin = cam_block.as_dvec3();
        let mut verts: Vec<Vertex> = vec![];
        for inst in &self.instances {
            let Some(m) = self.data.get(&inst.mesh) else { continue };
            let xf = inst.matrix_rel(origin);
            for t in m.indices.chunks_exact(3) {
                let p: [Vec3; 3] = std::array::from_fn(|k| xf.transform_point3(Vec3::from_array(m.positions[t[k] as usize])));
                let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                for (k, &i) in t.iter().enumerate() {
                    verts.push(Vertex { pos: p[k].to_array(), color: m.colors[i as usize], normal: n.to_array() });
                }
            }
        }
        // Records name at most MESH_TRIS triangles.
        if verts.len() > MESH_TRIS * 3 {
            if !self.warned {
                tracing::warn!("meshes: {} triangles, drawing the first {MESH_TRIS}", verts.len() / 3);
                self.warned = true;
            }
            verts.truncate(MESH_TRIS * 3);
        }
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        let grown = bytes.len() as u64 > self.vbuf.size();
        if grown {
            self.vbuf = Self::vertex_buffer(gpu, (bytes.len() as u64).next_power_of_two());
        }
        if !bytes.is_empty() {
            gpu.queue.write_buffer(&self.vbuf, 0, bytes);
        }
        self.vcount = verts.len() as u32;
        grown
    }

    /// Draws the meshes into an open render pass (sets its own pipeline and bind groups;
    /// record passes only group 0).
    /// `world_bg`: the world's group, for forward passes.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, kind: MeshPass, world_bg: Option<&wgpu::BindGroup>) {
        if self.vcount == 0 {
            return;
        }
        let pipeline = match kind {
            MeshPass::Records => &self.records,
            MeshPass::Ids => &self.ids,
            _ => {
                let f = self.forward.as_ref().expect("forward mesh passes need shadow rays (GpuMeshes::new)");
                pass.set_bind_group(1, world_bg.expect("forward mesh passes read the world"), &[]);
                if let Some(bg) = &self.occlusion_bg {
                    pass.set_bind_group(2, bg, &[]);
                }
                match kind {
                    MeshPass::Fill => &f.fill,
                    MeshPass::Depth => &f.depth,
                    _ => &f.equal,
                }
            }
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.draw(0..self.vcount, 0..1);
    }
}

/// The mesh pass's shader (the root of its programs; with `SHADE`, shadow rays from the
/// occlusion module).
pub const MESH_WGSL: &str = include_str!("mesh.wgsl");

/// The module path of [`surface_wgsl`] in programs that decode mesh hits.
pub const MESH_SURFACE: &str = "mr::mesh_surface";

/// The mesh-surface module (`mesh_surface`, `shade_mesh`; `mesh_surface.wgsl`) with the
/// triangles ([`GpuMeshes::vertices`]) at `group`, `binding`. It depends on a lighting module
/// (the mesh-shadow site's) and the world modules.
pub fn surface_wgsl(group: u32, binding: u32) -> String {
    format!("@group({group}) @binding({binding}) var<storage, read> mesh_verts: array<u32>;\n{}", include_str!("mesh_surface.wgsl"))
}

/// WGSL for ray casters with mesh records (bind group 3, see [`MeshRecords`]; the triangles
/// at binding 3 come with [`surface_wgsl`]`(3, 3)`): `mesh_t(pixel, d, forward)` is the
/// distance along the unit primary ray `d` to the mesh surface at that pixel (1e30 where there
/// is none), `mesh_record(pixel)` its record.
pub const MESH_RECORDS_WGSL: &str = r#"
struct MeshComp { near: f32, enabled: u32, _p0: u32, _p1: u32 }
@group(3) @binding(0) var mesh_rec_tex: texture_2d<u32>;
@group(3) @binding(1) var mesh_depth_tex: texture_depth_2d;
@group(3) @binding(2) var<uniform> mesh_comp: MeshComp;

fn mesh_t(p: vec2<u32>, d: vec3<f32>, forward: vec3<f32>) -> f32 {
    if (mesh_comp.enabled == 0u) { return 1e30; }
    let z = textureLoad(mesh_depth_tex, p, 0);
    if (z <= 0.0) { return 1e30; }
    // Reverse-Z infinite projection: depth = near / view distance along `forward`.
    return (mesh_comp.near / z) / dot(d, forward);
}

fn mesh_record(p: vec2<u32>) -> vec2<u32> {
    return textureLoad(mesh_rec_tex, p, 0).xy;
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CompUniform {
    near: f32,
    enabled: u32,
    _p: [u32; 2],
}

/// Mesh mode for ray casters (PLAN 7c, 12d): meshes are rasterised first into a record and
/// depth target of the frame's size (records only: no shading); the ray caster casts each
/// primary ray only up to the mesh's distance at that pixel (`mesh_t`), and where no voxel is
/// nearer the pixel's surface is the mesh's record, shaded inline by the walk or written for
/// the deferred pass. Exact for primary visibility, and capped rays trace less.
pub struct MeshRecords {
    pub meshes: GpuMeshes,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    records: wgpu::TextureView,
    depth: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

impl MeshRecords {
    pub fn new(gpu: &Gpu, world: &VoxelWorld, width: u32, height: u32) -> Self {
        let dev = &gpu.device;
        // Fragment too: ray casters may run per pixel in a fragment shader.
        let stages = wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT;
        let tex = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: stages,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let buf = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: stages, ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh records"),
            entries: &[
                tex(0, wgpu::TextureSampleType::Uint),
                tex(1, wgpu::TextureSampleType::Depth),
                buf(2, wgpu::BufferBindingType::Uniform),
                buf(3, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let uniform = dev.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh records"), size: size_of::<CompUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let (records, depth) = Self::targets(gpu, width, height);
        let meshes = GpuMeshes::new(gpu, world, None);
        let bind = Self::bind(gpu, &layout, &records, &depth, &uniform, meshes.vertices());
        Self { meshes, layout, uniform, records, depth, bind }
    }

    fn targets(gpu: &Gpu, w: u32, h: u32) -> (wgpu::TextureView, wgpu::TextureView) {
        let mk = |label, format| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        (mk("mesh records", wgpu::TextureFormat::Rg32Uint), mk("mesh depth", wgpu::TextureFormat::Depth32Float))
    }

    fn bind(gpu: &Gpu, layout: &wgpu::BindGroupLayout, records: &wgpu::TextureView, depth: &wgpu::TextureView, uniform: &wgpu::Buffer, verts: &wgpu::Buffer) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh records"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(records) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: verts.as_entire_binding() },
            ],
        })
    }

    fn rebind(&mut self, gpu: &Gpu) {
        self.bind = Self::bind(gpu, &self.layout, &self.records, &self.depth, &self.uniform, self.meshes.vertices());
    }

    /// Layout of bind group 3 for the ray caster's pipeline.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// Bind group 3 for the ray caster's passes.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        (self.records, self.depth) = Self::targets(gpu, width, height);
        self.rebind(gpu);
    }

    /// Whether `encode` will draw a pass this frame (so the renderer can move its frame's
    /// begin timestamp onto it).
    pub fn active(&self, meshes: bool) -> bool {
        meshes && self.meshes.has_instances()
    }

    /// Rasterises the meshes' records for this frame (when `meshes` is on) ahead of the ray
    /// caster's dispatch, and tells the shader whether to read them. `ts` carries the frame's
    /// begin timestamp when [`Self::active`].
    pub fn encode(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, cam: &Camera, aspect: f32, mode: RenderMode, meshes: bool, ts: Option<wgpu::RenderPassTimestampWrites<'_>>) {
        let on = self.active(meshes);
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&CompUniform { near: cam.near, enabled: on as u32, _p: [0; 2] }));
        if !on {
            return;
        }
        let view_proj = cam.projection(aspect) * cam.view_rotation();
        if self.meshes.prepare(gpu, cam, view_proj, mode) {
            self.rebind(gpu);
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mesh records"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.records,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: ts,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.meshes.draw(&mut pass, MeshPass::Records, None);
    }
}

#[cfg(test)]
mod tests {
    /// The record passes' program composes without lighting or shadow rays (forward shading,
    /// `SHADE`, is the back ends' tests'), and the record's constants agree with Rust.
    #[test]
    fn record_program_composes() {
        use crate::program::{COMMON, Program, VOXELS};
        let m = Program::world(None).naga(super::MESH_WGSL, &[VOXELS, COMMON]).unwrap();
        wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all()).validate(&m).unwrap();
        assert!(m.entry_points.iter().all(|e| e.name != "mesh_fs"));
        assert!(crate::voxels::VOXELS_WGSL.contains(&format!("const MESH_TRIS: u32 = {:#x}u;", super::MESH_TRIS)));
    }
}
