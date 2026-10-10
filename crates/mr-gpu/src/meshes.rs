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
