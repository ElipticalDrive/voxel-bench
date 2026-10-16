//! The face drawer's host side (IDEAS §16 step 2), shared by every back end that
//! rasterises block faces (gpu-raster, gpu-discover, cpu-discover): the pipelines of the
//! three raster visibility components and the passes that run them.
//!
//! * `raster` (forward): faces drawn and shaded in one pass (`fill`); in lit mode with a
//!   depth prepass first, then shaded where the depth is equal (`depth`, `equal`).
//! * `visbuffer` / `gbuffer` (deferred): faces drawn into an id (or hit point and block)
//!   target, then one compute pass shades each pixel once (`shade`), reading the targets
//!   through the images group ([`FaceDrawer::images_layout`], the shade layout's last
//!   group).
//!
//! The back end owns the shader module (its face source plus `faces_wgsl`,
//! `raster_wgsl(mode)`, the storage and the shading), the bind groups and the draw calls;
//! the drawer owns the pipelines, the deferred targets and the pass structure.

use mr_gpu::meshes::MeshPass;
use mr_gpu::{FrameRing, Gpu, Ticket, wgpu};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrawMode {
    Raster,
    Visbuffer,
    Gbuffer,
}

impl DrawMode {
    pub const NAMES: [&'static str; 3] = ["raster", "visbuffer", "gbuffer"];

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "raster" => Self::Raster,
            "visbuffer" => Self::Visbuffer,
            "gbuffer" => Self::Gbuffer,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// Shades in a compute pass after drawing (once per pixel, whatever the overdraw).
    pub fn deferred(self) -> bool {
        self != Self::Raster
    }
}

/// What the drawer's pipelines are built from.
pub struct DrawerSpec<'a> {
    pub module: &'a wgpu::ShaderModule,
    /// The raster passes' groups (0..n).
    pub render_layout: &'a wgpu::PipelineLayout,
    /// The render layout's groups plus [`FaceDrawer::images_layout`] as group n.
    pub shade_layout: &'a wgpu::PipelineLayout,
    /// Triangle strips (quads of 4 vertices) or lists.
    pub strip: bool,
}

enum Pipes {
    Forward { fill: wgpu::RenderPipeline, depth: wgpu::RenderPipeline, equal: wgpu::RenderPipeline },
    Deferred { ids: wgpu::RenderPipeline, shade: wgpu::ComputePipeline },
}

pub struct FaceDrawer {
    mode: DrawMode,
    pipes: Pipes,
    images_layout: wgpu::BindGroupLayout,
    /// visbuffer: face ids; gbuffer: block ids (1x1 while forward).
    ids: wgpu::TextureView,
    /// gbuffer: hit point and face (1x1 otherwise).
    gpos: wgpu::TextureView,
    width: u32,
    height: u32,
}

/// One frame's drawing.
pub struct DrawFrame<'a> {
    pub out: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub clear: wgpu::Color,
    /// Forward: lay down depth first and shade where it is equal (lit mode).
    pub prepass: bool,
    /// Keep the depth after drawing (read later, e.g. by incremental discovery).
    pub keep_depth: bool,
    /// The render layout's groups, in order (also set for the deferred shade pass, whose
    /// images follow them at group 3).
    pub binds: &'a [&'a wgpu::BindGroup],
    /// Further groups after the images (storages of rays walking their own, PLAN step 9c).
    pub more: &'a [(u32, &'a wgpu::BindGroup)],
    pub ring: &'a FrameRing,
    pub ticket: Ticket,
    /// Whether these passes carry the frame's (begin, end) timestamps.
    pub ts: (bool, bool),
    /// The graph nodes of the drawer's timed stages.
    pub nodes: &'a DrawNodes,
}

/// The graph nodes the drawer's stages run (PLAN step 11): drawing the faces, deferred
/// shading, and what `extra` draws (meshes). A forward drawer runs them in one stage.
#[derive(Clone, Debug, Default)]
pub struct DrawNodes {
    pub draw: Vec<String>,
    pub shade: Vec<String>,
    pub extra: Vec<String>,
}

fn target(gpu: &Gpu, label: &str, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl FaceDrawer {
    /// The deferred shade pass's images group: ids (0), output (1), G-buffer points (2).
    pub fn images_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let compute = wgpu::ShaderStages::COMPUTE;
        let tex = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: compute,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("deferred images"),
            entries: &[
                tex(0, wgpu::TextureSampleType::Uint),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: compute,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                tex(2, wgpu::TextureSampleType::Float { filterable: false }),
            ],
        })
    }

    /// `images_layout` is the one the shade layout was built with.
    pub fn new(gpu: &Gpu, spec: &DrawerSpec, mode: DrawMode, images_layout: &wgpu::BindGroupLayout, width: u32, height: u32) -> anyhow::Result<Self> {
        let pipes = Self::pipes(gpu, spec, mode)?;
        let mut d = Self { mode, pipes, images_layout: images_layout.clone(), ids: target(gpu, "ids", 1, 1, wgpu::TextureFormat::R32Uint), gpos: target(gpu, "gbuffer points", 1, 1, wgpu::TextureFormat::Rgba32Float), width, height };
        d.resize(gpu, width, height);
        Ok(d)
    }

    pub fn mode(&self) -> DrawMode {
        self.mode
    }

    /// After a mode or shader change.
    pub fn rebuild(&mut self, gpu: &Gpu, spec: &DrawerSpec, mode: DrawMode) -> anyhow::Result<()> {
        self.pipes = Self::pipes(gpu, spec, mode)?;
        self.mode = mode;
        self.resize(gpu, self.width, self.height);
        Ok(())
    }

    /// Deferred targets at full size only while used.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        (self.width, self.height) = (width, height);
        let size = |used: bool| if used { (width, height) } else { (1, 1) };
        let (w, h) = size(self.mode.deferred());
        self.ids = target(gpu, "ids", w, h, wgpu::TextureFormat::R32Uint);
        let (w, h) = size(self.mode == DrawMode::Gbuffer);
        self.gpos = target(gpu, "gbuffer points", w, h, wgpu::TextureFormat::Rgba32Float);
    }

    pub fn memory_bytes(&self) -> u64 {
        match self.mode {
            DrawMode::Raster => 0,
            DrawMode::Visbuffer => self.width as u64 * self.height as u64 * 4,
            DrawMode::Gbuffer => self.width as u64 * self.height as u64 * 20,
        }
    }

    fn pipes(gpu: &Gpu, spec: &DrawerSpec, mode: DrawMode) -> anyhow::Result<Pipes> {
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let render = |label, targets: &[Option<wgpu::ColorTargetState>], write: bool, compare, fragment: bool| {
            gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(spec.render_layout),
                vertex: wgpu::VertexState { module: spec.module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState {
                    topology: if spec.strip { wgpu::PrimitiveTopology::TriangleStrip } else { wgpu::PrimitiveTopology::TriangleList },
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: fragment.then(|| wgpu::FragmentState { module: spec.module, entry_point: Some("fs"), compilation_options: Default::default(), targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction::{Equal, Greater};
        use wgpu::TextureFormat::{R32Uint, Rgba8Unorm, Rgba32Float};
        let color = [Some(wgpu::ColorTargetState { format: Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let pipes = match mode {
            DrawMode::Raster => Pipes::Forward {
                fill: render("fill", &color, true, Greater, true),
                depth: render("depth prepass", &color, true, Greater, false),
                equal: render("shade equal", &color, false, Equal, true),
            },
            DrawMode::Visbuffer | DrawMode::Gbuffer => {
                let ids = if mode == DrawMode::Gbuffer {
                    render("gbuffer", &[Some(Rgba32Float.into()), Some(R32Uint.into())], true, Greater, true)
                } else {
                    render("visibility ids", &[Some(R32Uint.into())], true, Greater, true)
                };
                let shade = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("deferred shade"),
                    layout: Some(spec.shade_layout),
                    module: spec.module,
                    entry_point: Some("shade"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                Pipes::Deferred { ids, shade }
            }
        };
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling the {} drawer: {e}", mode.name());
        }
        Ok(pipes)
    }

    /// Encodes the frame's face passes. `draw` issues the face draws into a pass whose
    /// pipeline and groups are set; `extra` draws more into the same depth (gpu-raster's
    /// meshes: in each forward pass, after shading when deferred); `main` replaces the
    /// forward main pipeline (gpu-discover's painter order and overdraw view; no prepass).
    pub fn encode(
        &self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        f: &DrawFrame,
        draw: &dyn Fn(&mut wgpu::RenderPass<'_>),
        extra: Option<&dyn Fn(&mut wgpu::RenderPass<'_>, MeshPass)>,
        main: Option<&wgpu::RenderPipeline>,
    ) {
        let depth_attachment = |load, store| Some(wgpu::RenderPassDepthStencilAttachment { view: f.depth, depth_ops: Some(wgpu::Operations { load, store }), stencil_ops: None });
        let keep = |keep: bool| if keep { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard };
        let run = |pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline| {
            pass.set_pipeline(pipeline);
            for (i, g) in f.binds.iter().enumerate() {
                pass.set_bind_group(i as u32, *g, &[]);
            }
            for (i, g) in f.more {
                pass.set_bind_group(*i, *g, &[]);
            }
            draw(pass);
        };
        match &self.pipes {
            Pipes::Forward { fill, depth, equal } => {
                let n = f.nodes;
                let nodes: Vec<String> = n.draw.iter().chain(&n.shade).chain(extra.map_or(&[][..], |_| &n.extra[..])).cloned().collect();
                f.ring.stage(enc, f.ticket, "draw+shade", &nodes);
                let prepass = f.prepass && main.is_none();
                if prepass {
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("depth prepass"),
                        color_attachments: &[],
                        depth_stencil_attachment: depth_attachment(wgpu::LoadOp::Clear(0.0), wgpu::StoreOp::Store),
                        timestamp_writes: if f.ts.0 { f.ring.render_timestamps(f.ticket, true, false) } else { None },
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    run(&mut pass, depth);
                    if let Some(extra) = extra {
                        extra(&mut pass, MeshPass::Depth);
                    }
                }
                let begin = f.ts.0 && !prepass;
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("shade"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: f.out,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(f.clear), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: depth_attachment(if prepass { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) }, keep(f.keep_depth)),
                    timestamp_writes: if begin || f.ts.1 { f.ring.render_timestamps(f.ticket, begin, f.ts.1) } else { None },
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                run(&mut pass, main.unwrap_or(if prepass { equal } else { fill }));
                if let Some(extra) = extra {
                    extra(&mut pass, if prepass { MeshPass::Equal } else { MeshPass::Fill });
                }
            }
            Pipes::Deferred { ids, shade } => {
                f.ring.stage(enc, f.ticket, "draw", &f.nodes.draw);
                {
                    let attach = |view| {
                        Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })
                    };
                    let gbuffer = [attach(&self.gpos), attach(&self.ids)];
                    let idbuffer = [attach(&self.ids)];
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("visibility ids"),
                        color_attachments: if self.mode == DrawMode::Gbuffer { &gbuffer } else { &idbuffer },
                        depth_stencil_attachment: depth_attachment(wgpu::LoadOp::Clear(0.0), keep(f.keep_depth || extra.is_some())),
                        timestamp_writes: if f.ts.0 { f.ring.render_timestamps(f.ticket, true, false) } else { None },
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    run(&mut pass, ids);
                }
                let images = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("deferred images"),
                    layout: &self.images_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.ids) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(f.out) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.gpos) },
                    ],
                });
                f.ring.stage(enc, f.ticket, "shade", &f.nodes.shade);
                {
                    let end = f.ts.1 && extra.is_none();
                    let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: if end { f.ring.compute_timestamps(f.ticket, false, true) } else { None } });
                    pass.set_pipeline(shade);
                    for (i, g) in f.binds.iter().enumerate() {
                        pass.set_bind_group(i as u32, *g, &[]);
                    }
                    pass.set_bind_group(f.binds.len() as u32, &images, &[]);
                    for (i, g) in f.more {
                        pass.set_bind_group(*i, *g, &[]);
                    }
                    pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
                }
                if let Some(extra) = extra {
                    f.ring.stage(enc, f.ticket, "meshes", &f.nodes.extra);
                    // Drawn after shading, depth-tested against the faces.
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("after shading"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: f.out,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: depth_attachment(wgpu::LoadOp::Load, keep(f.keep_depth)),
                        timestamp_writes: if f.ts.1 { f.ring.render_timestamps(f.ticket, false, true) } else { None },
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    extra(&mut pass, MeshPass::Fill);
                }
            }
        }
    }
}
