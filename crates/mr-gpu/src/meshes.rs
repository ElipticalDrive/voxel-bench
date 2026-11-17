//! Mesh mode for GPU rasterisers (`mr_core::mesh`): keeps the scene's meshes and instances,
//! writes camera-relative world-space triangles each frame (meshes are small), and draws
//! them into the renderer's targets and depth.
//!
//! Meshes are a surface source: their fragments write the hit record's mesh
//! capability (`Records`: the packed record, `Ids`: `visbuffer`'s encoding), and the pixel is
//! shaded wherever the strategy shades (the ray caster's walk, the common deferred pass),
//! decoding the triangle from the same vertices ([`surface_wgsl`]). Forward drawers shade
//! the fragments themselves ([`ForwardSpec`]: the region's module of `mesh.wgsl` with
//! `SHADE`, over its program and bind groups) in three flavours: `Fill` (depth test +
//! write), `Depth` (pre-pass, no colour) and `Equal` (shading after a pre-pass). Either way
//! the shading component shades the mesh surface. Back faces are not culled: meshes may be
//! open or inconsistently wound.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};
use mr_core::{Camera, MeshData, MeshInstance, MeshUpdate, RenderMode};

use crate::Gpu;

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

/// A region's forward mesh shading: [`MESH_WGSL`] with `SHADE` compiled over the region's
/// program (its frame as `mesh_frame()`, its shading component), drawn with the region's
/// pipeline `layout` and the bind groups its drawer set.
pub struct ForwardSpec<'a> {
    pub module: &'a wgpu::ShaderModule,
    pub layout: &'a wgpu::PipelineLayout,
    /// The output's format.
    pub format: wgpu::TextureFormat,
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

impl Forward {
    fn new(gpu: &Gpu, f: &ForwardSpec) -> Self {
        use wgpu::CompareFunction::{Equal, Greater};
        Self {
            fill: pipeline(gpu, "mesh fill", f.module, f.layout, Some("mesh_fs"), f.format, true, Greater),
            depth: pipeline(gpu, "mesh depth", f.module, f.layout, None, f.format, true, Greater),
            equal: pipeline(gpu, "mesh equal", f.module, f.layout, Some("mesh_fs"), f.format, false, Equal),
        }
    }
}

/// A mesh pipeline: `mesh_vs` over the vertex layout, fragment `entry` (none: depth only)
/// into `target`, depth `compare` with (`write`) or without writing, no culling.
fn pipeline(gpu: &Gpu, label: &str, module: &wgpu::ShaderModule, layout: &wgpu::PipelineLayout, entry: Option<&str>, target: wgpu::TextureFormat, write: bool, compare: wgpu::CompareFunction) -> wgpu::RenderPipeline {
    let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x3];
    let targets = [Some(wgpu::ColorTargetState { format: target, blend: None, write_mask: wgpu::ColorWrites::ALL })];
    gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
}

pub struct GpuMeshes {
    data: HashMap<u32, Arc<MeshData>>,
    instances: Vec<MeshInstance>,
    dirty: bool,
    /// The camera block and whether with the BVH the triangles were last built for.
    built_for: Option<(IVec3, bool)>,
    vbuf: wgpu::Buffer,
    vcount: u32,
    frame_buf: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Forward shading, when built with a [`ForwardSpec`].
    forward: Option<Forward>,
    records: wgpu::RenderPipeline,
    ids: wgpu::RenderPipeline,
}

impl GpuMeshes {
    /// `forward`: the region's forward shading (`Fill`, `Depth`, `Equal`); record passes
    /// need none.
    pub fn new(gpu: &Gpu, forward: Option<ForwardSpec>) -> Self {
        let dev = &gpu.device;
        use crate::program::{COMMON, Program, VOXELS};
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
        use wgpu::CompareFunction::Greater;
        use wgpu::TextureFormat::{R32Uint, Rg32Uint};
        let forward = forward.map(|f| Forward::new(gpu, &f));
        let module = crate::program::compile(dev, "mesh records", &Program::world(None), &records_root(), &[VOXELS, COMMON]);
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
            records: pipeline(gpu, "mesh records", &module, &pl, Some("mesh_record_fs"), Rg32Uint, true, Greater),
            ids: pipeline(gpu, "mesh ids", &module, &pl, Some("mesh_id_fs"), R32Uint, true, Greater),
        }
    }

    /// Replaces the forward passes (the region's shading changed).
    pub fn set_forward(&mut self, gpu: &Gpu, forward: ForwardSpec) {
        self.forward = Some(Forward::new(gpu, &forward));
    }

    /// Vertices are drawn and, as storage, read by [`surface_wgsl`]'s decoding.
    fn vertex_buffer(gpu: &Gpu, size: u64) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh vertices"), size, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    /// The triangles (a header, their vertices, their BVH: [`mesh_words`]) for
    /// [`surface_wgsl`]'s binding. It may be replaced by [`Self::prepare`].
    pub fn vertices(&self) -> &wgpu::Buffer {
        &self.vbuf
    }

    /// The occlusion module's bind group 2 for the next draws (it may change when the
    /// renderer's buffers grow).
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
    /// triangles; with `rays` (the frame's shading casts rays that meet the triangles: lit,
    /// in a program with them bound) also their BVH, else an empty one. Call once per frame
    /// before drawing. Returns whether the vertex buffer was replaced (bind groups holding
    /// [`Self::vertices`] then need rebuilding).
    pub fn prepare(&mut self, gpu: &Gpu, cam: &Camera, view_proj: glam::Mat4, mode: RenderMode, rays: bool) -> bool {
        let (cam_block, frac) = cam.split_position();
        let f = MeshFrame { view_proj: view_proj.to_cols_array_2d(), cam_frac: frac.to_array(), mode: (mode == RenderMode::Lit) as u32 };
        gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&f));
        if !self.dirty && self.built_for == Some((cam_block, rays)) {
            return false;
        }
        self.dirty = false;
        self.built_for = Some((cam_block, rays));
        let origin = cam_block.as_dvec3();
        let mut tris: Vec<[Vertex; 3]> = vec![];
        for inst in &self.instances {
            let Some(m) = self.data.get(&inst.mesh) else { continue };
            let xf = inst.matrix_rel(origin);
            for t in m.indices.as_chunks::<3>().0 {
                let p: [Vec3; 3] = std::array::from_fn(|k| xf.transform_point3(Vec3::from_array(m.positions[t[k] as usize])));
                let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                tris.push(std::array::from_fn(|k| Vertex { pos: p[k].to_array(), color: m.colors[t[k] as usize], normal: n.to_array() }));
            }
        }
        // Records name at most MESH_TRIS triangles.
        if tris.len() > MESH_TRIS {
            gpu.fail(format!("meshes: {} triangles, more than the {MESH_TRIS} a hit record can name; frames would miss the rest", tris.len()));
            tris.truncate(MESH_TRIS);
        }
        // Three writes, so the vertices go up as they are, without a copy into one buffer
        // (frames whose rays do not meet the triangles upload no more than before there was
        // a BVH).
        let (head, nodes) = mesh_words(&mut tris, rays);
        let verts: &[u8] = bytemuck::cast_slice(&tris);
        let (head, nodes): (&[u8], &[u8]) = (bytemuck::cast_slice(&head), bytemuck::cast_slice(&nodes));
        let size = (head.len() + verts.len() + nodes.len()) as u64;
        let grown = size > self.vbuf.size();
        if grown {
            self.vbuf = Self::vertex_buffer(gpu, size.next_power_of_two());
        }
        gpu.queue.write_buffer(&self.vbuf, 0, head);
        gpu.queue.write_buffer(&self.vbuf, head.len() as u64, verts);
        gpu.queue.write_buffer(&self.vbuf, (head.len() + verts.len()) as u64, nodes);
        self.vcount = tris.len() as u32 * 3;
        grown
    }

    /// Draws the meshes into an open render pass: record passes with their own pipeline
    /// layout (the frame at group 0), forward passes over the bind groups the region's
    /// drawer set (its [`ForwardSpec`] layout).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, kind: MeshPass) {
        if self.vcount == 0 {
            return;
        }
        let pipeline = match kind {
            MeshPass::Records => &self.records,
            MeshPass::Ids => &self.ids,
            _ => {
                let f = self.forward.as_ref().expect("forward mesh passes need the region's shading (GpuMeshes::new)");
                match kind {
                    MeshPass::Fill => &f.fill,
                    MeshPass::Depth => &f.depth,
                    _ => &f.equal,
                }
            }
        };
        pass.set_pipeline(pipeline);
        if matches!(kind, MeshPass::Records | MeshPass::Ids) {
            pass.set_bind_group(0, &self.bind, &[]);
        }
        pass.set_vertex_buffer(0, self.vbuf.slice(MESH_HEAD as u64 * 4..));
        pass.draw(0..self.vcount, 0..1);
    }
}

/// Words before the vertices in the vertex buffer (`MESH_HEAD` in `mesh_surface.wgsl`).
const MESH_HEAD: usize = 4;

/// The vertex buffer's words as `mesh_surface.wgsl` reads them, reordering `tris` into
/// their BVH's leaves' order: a header (the BVH's first word, then padding to
/// [`MESH_HEAD`]), the vertices (seven words each, three per triangle: `tris` itself, not
/// returned), the BVH (returned after the header). BVH node
/// `k` (the root `k = 0`) is 8 words: its box (lo, hi: 6 f32), then a leaf's first triangle
/// and count, or an inner node's children (the second in bits 0-28, its split axis in bits
/// 29-30, bit 31 set; the first child on the axis' lower side). Without triangles,
/// or without `bvh`, one empty leaf.
fn mesh_words(tris: &mut [[Vertex; 3]], bvh: bool) -> ([u32; MESH_HEAD], Vec<u32>) {
    let mut nodes = vec![];
    let bounds = |t: &[Vertex; 3]| {
        let p = t.map(|v| Vec3::from_array(v.pos).as_dvec3());
        (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]))
    };
    if bvh && !tris.is_empty() {
        mr_core::mesh::build_bvh(tris, &mut nodes, 0, &bounds);
    }
    let mut head = [0u32; MESH_HEAD];
    head[0] = (MESH_HEAD + tris.len() * 3 * std::mem::size_of::<Vertex>() / 4) as u32;
    let mut words = vec![0u32; nodes.len().max(1) * 8];
    for (k, n) in nodes.iter().enumerate() {
        let w = &mut words[k * 8..][..8];
        // The boxes are of f32 corners, so exact in f32.
        for i in 0..3 {
            w[i] = (n.lo[i] as f32).to_bits();
            w[3 + i] = (n.hi[i] as f32).to_bits();
        }
        (w[6], w[7]) = if n.leaf { (n.a as u32, (n.b - n.a) as u32) } else { (n.a as u32, n.b as u32 | (n.axis as u32) << 29 | 1 << 31) };
    }
    (head, words)
}

/// The mesh pass's shader (the root of its programs). The root provides `mesh_frame()`
/// ([`MESH_FRAME_WGSL`] for the record passes; a forward drawer's frame) and, with `SHADE`,
/// `shade_mesh` (the shading component's, `shading::mesh`).
pub const MESH_WGSL: &str = include_str!("mesh.wgsl");

/// The record passes' camera: the `MeshFrame` uniform at group 0, binding 0.
pub const MESH_FRAME_WGSL: &str = "@group(0) @binding(0) var<uniform> mf: MeshFrame;\nfn mesh_frame() -> MeshFrame { return mf; }\n";

/// The record passes' root: [`MESH_FRAME_WGSL`] then [`MESH_WGSL`].
pub fn records_root() -> String {
    format!("{MESH_FRAME_WGSL}{MESH_WGSL}")
}

/// The module path of [`surface_wgsl`] in programs that decode mesh hits.
pub const MESH_SURFACE: &str = "mr::mesh_surface";

/// The mesh-surface module (`mesh_surface`; `mesh_surface.wgsl`) with the triangles
/// ([`GpuMeshes::vertices`]) at `group`, `binding`. It depends on the world modules only;
/// the shading component shades what it decodes (`shade_mesh`, `shading::mesh`).
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

/// Mesh mode for ray casters: meshes are rasterised first into a record and
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
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
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
        let meshes = GpuMeshes::new(gpu, None);
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
        if self.meshes.prepare(gpu, cam, view_proj, mode, mode == RenderMode::Lit) {
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
        self.meshes.draw(&mut pass, MeshPass::Records);
    }
}

#[cfg(test)]
mod tests {
    /// The record passes' program composes without lighting or shadow rays (forward shading,
    /// `SHADE`, is the face executor's tests'), and the record's constants agree with Rust.
    #[test]
    fn record_program_composes() {
        use crate::program::{COMMON, Program, VOXELS};
        let m = Program::world(None).naga(&super::records_root(), &[VOXELS, COMMON]).unwrap();
        wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all()).validate(&m).unwrap();
        assert!(m.entry_points.iter().all(|e| e.name != "mesh_fs"));
        assert!(crate::voxels::VOXELS_WGSL.contains(&format!("const MESH_TRIS: u32 = {:#x}u;", super::MESH_TRIS)));
    }

    /// `mesh_surface.wgsl`'s walk, word for word on the CPU over the buffer `prepare` writes
    /// (a header, the triangles, the BVH), finds what testing every triangle finds.
    #[test]
    fn bvh_words_walk_matches_brute_force() {
        use super::{MESH_HEAD, Vertex, mesh_words};
        use glam::Vec3;
        let mut seed = 9u32;
        let mut r = |s: f32| {
            seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
            ((seed >> 8) as f32 / 16777216.0 * 2.0 - 1.0) * s
        };
        let mut tris: Vec<[Vertex; 3]> = (0..500)
            .map(|_| {
                let c = Vec3::new(r(6.0), r(6.0), r(6.0));
                std::array::from_fn(|_| Vertex { pos: (c + Vec3::new(r(0.8), r(0.8), r(0.8))).to_array(), color: [0; 4], normal: [0.0; 3] })
            })
            .collect();
        let (head, nodes) = mesh_words(&mut tris, true);
        let buf = [&head[..], bytemuck::cast_slice(&tris), &nodes].concat();
        let pos = |v: u32| Vec3::from_array(std::array::from_fn(|k| f32::from_bits(buf[MESH_HEAD + (v * 7) as usize + k])));
        let tri_t = |tri: u32, o: Vec3, d: Vec3| -> f32 {
            let p0 = pos(tri * 3);
            let (e1, e2) = (pos(tri * 3 + 1) - p0, pos(tri * 3 + 2) - p0);
            let pv = d.cross(e2);
            let det = e1.dot(pv);
            if det.abs() < 1e-12 {
                return -1.0;
            }
            let inv = 1.0 / det;
            let tv = o - p0;
            let u = tv.dot(pv) * inv;
            if !(0.0..=1.0).contains(&u) {
                return -1.0;
            }
            let qv = tv.cross(e1);
            let v = d.dot(qv) * inv;
            if v < 0.0 || u + v > 1.0 {
                return -1.0;
            }
            e2.dot(qv) * inv
        };
        let walk = |o: Vec3, d: Vec3, max_t: f32| -> Option<f32> {
            let mut best = (max_t, None);
            let dd = Vec3::select(d.abs().cmplt(Vec3::splat(1e-20)), Vec3::splat(1e-20), d);
            let inv = dd.recip();
            let mut stack = vec![0u32];
            while let Some(k) = stack.pop() {
                let w = (buf[0] + 8 * k) as usize;
                let f = |i: usize| f32::from_bits(buf[w + i]);
                let (lo, hi) = (Vec3::new(f(0), f(1), f(2)), Vec3::new(f(3), f(4), f(5)));
                let (t0, t1) = ((lo - o) * inv, (hi - o) * inv);
                if t0.min(t1).max_element().max(0.0) > t0.max(t1).min_element().min(best.0) {
                    continue;
                }
                let (a, b) = (buf[w + 6], buf[w + 7]);
                if b & 1 << 31 != 0 {
                    // The far child first, so the near one is walked first.
                    let (near, far) = if d[(b >> 29 & 3) as usize] < 0.0 { (b & 0x1fff_ffff, a) } else { (a, b & 0x1fff_ffff) };
                    stack.extend([far, near]);
                    continue;
                }
                for i in a..a + b {
                    let t = tri_t(i, o, d);
                    if t > 1e-4 && t < best.0 {
                        best = (t, Some(i));
                    }
                }
            }
            best.1.map(|_| best.0)
        };
        let mut hits = 0;
        for _ in 0..2000 {
            let o = Vec3::new(r(15.0), r(15.0), r(15.0));
            let d = (Vec3::new(r(6.0), r(6.0), r(6.0)) - o).normalize();
            let max_t = 5.0 + r(1.0).abs() * 25.0;
            let brute = (0..tris.len() as u32).map(|i| tri_t(i, o, d)).filter(|&t| t > 1e-4 && t < max_t).min_by(f32::total_cmp);
            assert_eq!(walk(o, d, max_t), brute);
            hits += brute.is_some() as u32;
        }
        assert!(hits > 200, "{hits} rays hit");
    }
}
