//! Mesh mode for GPU rasterisers (`mr_core::mesh`): keeps the scene's meshes and instances,
//! writes camera-relative world-space triangles each frame (meshes are small), and draws
//! them into the renderer's colour and depth targets with the shared lighting.
//!
//! Pipelines come in the three flavours raster renderers use: `Fill` (depth test + write),
//! `Depth` (pre-pass, no colour) and `Equal` (shading after a pre-pass). Back faces are not
//! culled: meshes may be open or inconsistently wound.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};
use mr_core::{Camera, MeshData, MeshInstance, MeshUpdate, RenderMode};

use crate::Gpu;
use crate::voxels::{DDA_OCCLUSION_WGSL, VoxelWorld, shader_source};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    color: [u8; 4],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MeshFrame {
    view_proj: [[f32; 4]; 4],
    cam_frac: [f32; 3],
    mode: u32,
}

#[derive(Copy, Clone, PartialEq)]
pub enum MeshPass {
    Fill,
    Depth,
    Equal,
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
    fill: wgpu::RenderPipeline,
    depth: wgpu::RenderPipeline,
    equal: wgpu::RenderPipeline,
}

impl GpuMeshes {
    /// `world` provides bind group 1 (lighting and shadow rays); its visibility must
    /// include the fragment stage.
    pub fn new(gpu: &Gpu, world: &VoxelWorld, color: wgpu::TextureFormat) -> Self {
        let dev = &gpu.device;
        let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("meshes"), source: wgpu::ShaderSource::Wgsl(shader_source(DDA_OCCLUSION_WGSL, include_str!("mesh.wgsl")).into()) });
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
        let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("meshes"), bind_group_layouts: &[Some(&layout0), Some(world.layout())], immediate_size: 0 });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x3];
        let pipeline = |label, write: bool, compare, fragment: bool| {
            let targets = [Some(wgpu::ColorTargetState { format: color, blend: None, write_mask: wgpu::ColorWrites::ALL })];
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("mesh_vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout { array_stride: size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })],
                },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias: Default::default() }),
                multisample: Default::default(),
                fragment: fragment.then(|| wgpu::FragmentState { module: &module, entry_point: Some("mesh_fs"), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            data: HashMap::new(),
            instances: vec![],
            dirty: false,
            built_for: None,
            vbuf: dev.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh vertices"), size: 256, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
            vcount: 0,
            frame_buf,
            bind,
            fill: pipeline("mesh fill", true, wgpu::CompareFunction::Greater, true),
            depth: pipeline("mesh depth", true, wgpu::CompareFunction::Greater, false),
            equal: pipeline("mesh equal", false, wgpu::CompareFunction::Equal, true),
        }
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
    /// triangles. Call once per frame before drawing.
    pub fn prepare(&mut self, gpu: &Gpu, cam: &Camera, view_proj: glam::Mat4, mode: RenderMode) {
        let (cam_block, frac) = cam.split_position();
        let f = MeshFrame { view_proj: view_proj.to_cols_array_2d(), cam_frac: frac.to_array(), mode: (mode == RenderMode::Lit) as u32 };
        gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&f));
        if !self.dirty && self.built_for == Some(cam_block) {
            return;
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
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        if bytes.len() as u64 > self.vbuf.size() {
            self.vbuf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh vertices"), size: (bytes.len() as u64).next_power_of_two(), usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !bytes.is_empty() {
            gpu.queue.write_buffer(&self.vbuf, 0, bytes);
        }
        self.vcount = verts.len() as u32;
    }

    /// Draws the meshes into an open render pass (sets its own pipeline and bind groups).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, kind: MeshPass, world_bg: &wgpu::BindGroup) {
        if self.vcount == 0 {
            return;
        }
        pass.set_pipeline(match kind {
            MeshPass::Fill => &self.fill,
            MeshPass::Depth => &self.depth,
            MeshPass::Equal => &self.equal,
        });
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_bind_group(1, world_bg, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.draw(0..self.vcount, 0..1);
    }
}

/// WGSL for ray casters compositing meshes (bind group 3, see [`MeshComposite`]):
/// `mesh_t(pixel, d, forward)` is the distance along the unit primary ray `d` to the mesh
/// surface at that pixel (1e30 where there is none); `mesh_color(pixel)` its shaded colour.
pub const MESH_COMPOSITE_WGSL: &str = r#"
struct MeshComp { near: f32, enabled: u32, _p0: u32, _p1: u32 }
@group(3) @binding(0) var mesh_color_tex: texture_2d<f32>;
@group(3) @binding(1) var mesh_depth_tex: texture_depth_2d;
@group(3) @binding(2) var<uniform> mesh_comp: MeshComp;

fn mesh_t(p: vec2<u32>, d: vec3<f32>, forward: vec3<f32>) -> f32 {
    if (mesh_comp.enabled == 0u) { return 1e30; }
    let z = textureLoad(mesh_depth_tex, p, 0);
    if (z <= 0.0) { return 1e30; }
    // Reverse-Z infinite projection: depth = near / view distance along `forward`.
    return (mesh_comp.near / z) / dot(d, forward);
}

fn mesh_color(p: vec2<u32>) -> vec4<f32> {
    return textureLoad(mesh_color_tex, p, 0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CompUniform {
    near: f32,
    enabled: u32,
    _p: [u32; 2],
}

/// Mesh mode for ray casters (PLAN 7c): meshes are rasterised first into a colour + depth
/// target of the frame's size (fully shaded, with voxel shadows in lit mode); the ray
/// caster then casts each primary ray only up to the mesh's distance at that pixel
/// (`mesh_t`), and shows `mesh_color` where no voxel is nearer. Exact for primary
/// visibility, and capped rays trace less.
pub struct MeshComposite {
    pub meshes: GpuMeshes,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

impl MeshComposite {
    pub fn new(gpu: &Gpu, world: &VoxelWorld, width: u32, height: u32) -> Self {
        let dev = &gpu.device;
        let tex = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh composite"),
            entries: &[
                tex(0, wgpu::TextureSampleType::Float { filterable: false }),
                tex(1, wgpu::TextureSampleType::Depth),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let uniform = dev.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh composite"), size: size_of::<CompUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let (color, depth) = Self::targets(gpu, width, height);
        let bind = Self::bind(gpu, &layout, &color, &depth, &uniform);
        Self { meshes: GpuMeshes::new(gpu, world, wgpu::TextureFormat::Rgba8Unorm), layout, uniform, color, depth, bind }
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
        (mk("mesh colour", wgpu::TextureFormat::Rgba8Unorm), mk("mesh depth", wgpu::TextureFormat::Depth32Float))
    }

    fn bind(gpu: &Gpu, layout: &wgpu::BindGroupLayout, color: &wgpu::TextureView, depth: &wgpu::TextureView, uniform: &wgpu::Buffer) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh composite"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
            ],
        })
    }

    /// Layout of bind group 3 for the ray caster's pipeline.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// Bind group 3 for the ray caster's dispatch.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        (self.color, self.depth) = Self::targets(gpu, width, height);
        self.bind = Self::bind(gpu, &self.layout, &self.color, &self.depth, &self.uniform);
    }

    /// Whether `encode` will draw a pass this frame (so the renderer can move its frame's
    /// begin timestamp onto it).
    pub fn active(&self, meshes: bool) -> bool {
        meshes && self.meshes.has_instances()
    }

    /// Rasterises the meshes for this frame (when `meshes` is on) ahead of the ray caster's
    /// dispatch, and tells the shader whether to composite. `ts` carries the frame's begin
    /// timestamp when [`Self::active`].
    pub fn encode(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, cam: &Camera, aspect: f32, mode: RenderMode, meshes: bool, world_bg: &wgpu::BindGroup, ts: Option<wgpu::RenderPassTimestampWrites<'_>>) {
        let on = self.active(meshes);
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&CompUniform { near: cam.near, enabled: on as u32, _p: [0; 2] }));
        if !on {
            return;
        }
        let view_proj = cam.projection(aspect) * cam.view_rotation();
        self.meshes.prepare(gpu, cam, view_proj, mode);
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mesh composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color,
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
        self.meshes.draw(&mut pass, MeshPass::Fill, world_bg);
    }
}
