//! A face source's category views: its face list rasterised on its own with a fragment entry
//! returning each fragment's category (a `u32`, as `mr_core::Quantity` packs it) into a
//! target of its own, then copied into the post's quantity buffer. Built on the view's first
//! frame; nothing exists while no category view of the source is chosen.

use mr_gpu::Pipelines as _;
use mr_components::storage::gpu_data::Bound;
use mr_gpu::wgpu;

use crate::Core;

/// A raster of a face list over `layout` (`vs` and fragment `entry` of `module`) into
/// `target` (none: side effects only), depth-tested with `compare`.
pub(crate) fn face_raster(core: &Core, label: &str, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, entry: &str, target: Option<wgpu::TextureFormat>, compare: wgpu::CompareFunction) -> wgpu::RenderPipeline {
    let targets = [target.map(|format| wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
    core.gpu.device.render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState { module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, front_face: wgpu::FrontFace::Ccw, cull_mode: Some(wgpu::Face::Back), ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true), depth_compare: Some(compare), stencil: Default::default(), bias: Default::default() }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState { module, entry_point: Some(entry), compilation_options: Default::default(), targets: if target.is_some() { &targets[..] } else { &[] } }),
        multiview_mask: None,
        cache: core.gpu.cache(),
    })
}

/// The copy of a category target into the quantity buffer.
const COPY_WGSL: &str = "
@group(0) @binding(0) var ids: texture_2d<u32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) p: vec3<u32>) {
    let d = textureDimensions(ids);
    if (p.x >= d.x || p.y >= d.y) { return; }
    quantity[p.y * d.x + p.x] = textureLoad(ids, p.xy, 0).r;
}
";

/// One category view of a face source: its raster, target, depth and copy.
pub(crate) struct CategoryRaster {
    /// The view's quantity and the size it was made at.
    pub name: &'static str,
    pub size: (u32, u32),
    pipe: wgpu::RenderPipeline,
    ids: wgpu::TextureView,
    depth: wgpu::TextureView,
    copy: wgpu::ComputePipeline,
}

impl CategoryRaster {
    /// View `name`'s raster at the core's size: fragment `entry` of `module` (over the
    /// drawing's `layout`).
    pub fn new(core: &Core, name: &'static str, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, entry: &str) -> Self {
        let (gpu, (w, h)) = (&core.gpu, (core.width, core.height));
        let ids = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("view categories"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Uint,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let pipe = face_raster(core, entry, layout, module, entry, Some(wgpu::TextureFormat::R32Uint), wgpu::CompareFunction::Greater);
        let src = format!("{}{COPY_WGSL}", crate::view::wgsl());
        let copy_mod = mr_gpu::shader(&gpu.device, "view copy", src);
        let copy = gpu.device.compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("view copy"), layout: None, module: &copy_mod, entry_point: Some("main"), compilation_options: Default::default(), cache: gpu.cache() });
        Self { name, size: (w, h), pipe, ids, depth: super::depth_view(gpu, w, h, false), copy }
    }

    /// Whether it is view `name`'s at the core's size.
    pub fn is(&self, core: &Core, name: &str) -> bool {
        self.name == name && self.size == (core.width, core.height)
    }

    /// The raster (group 0 `bind`, the world, the storages; `draw` issues the draw) and the
    /// copy into the quantity buffer of the view chosen.
    pub fn encode(&self, core: &Core, enc: &mut wgpu::CommandEncoder, bind: &wgpu::BindGroup, world: &wgpu::BindGroup, bound: &Bound, draw: impl FnOnce(&mut wgpu::RenderPass<'_>)) {
        let Some(viewing) = &core.view else { return };
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("view raster"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.ids, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, bind, &[]);
            pass.set_bind_group(1, world, &[]);
            bound.render(&mut pass);
            draw(&mut pass);
        }
        let entries = [wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.ids) }, viewing.bind_entry()];
        let bind = core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("view copy"), layout: &self.copy.get_bind_group_layout(0), entries: &entries });
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("view copy"), timestamp_writes: None });
        pass.set_pipeline(&self.copy);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(self.size.0.div_ceil(8), self.size.1.div_ceil(8), 1);
    }
}
