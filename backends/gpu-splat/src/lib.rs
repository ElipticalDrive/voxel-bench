//! GPU-splat back end (components, IDEAS §12): geometry `points`, visibility `splat`,
//! shading `direct`, and the **storage** lit-mode shadow rays walk (refloor; group 2).
//! Was the gpu-splat renderer.
//!
//! GPU point splatting: every face-exposed block becomes one point (geometry `points` on
//! the CPU, `mr_components::geometry::points`), uploaded as the schedule's transfer of the
//! points to the GPU; the splat passes and their resolve, the common deferred shading pass
//! over the ids (PLAN step 12c), are visibility `splat` (`mr_components::visibility::splat`).
//! This back end keeps the plumbing: buffers, bind groups, passes.

use std::sync::Arc;
use std::time::Instant;

use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::geometry::points::{Point, Points};
use mr_components::graph::{self, Role};
use mr_components::shaders::Walks;
use mr_components::storage::gpu_data::{self, StorageSet};
use mr_components::visibility::splat;
use mr_components::{names_on, require, Slot};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, d, w| Ok(Box::new(GpuSplat::new(p, d, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-splat".into(),
        description: "GPU-splat back end (storage for shadow rays, points geometry, splat visibility, shading). Compute point splatting: one point per exposed block, atomicMin depth + id buffer, ray-box resolve".into(),
        tags: vec!["gpu".into(), "compute".into(), "points".into(), "splat".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, vec![
            ParamSpec::choice("storage", "Storage component the lit-mode shadow rays walk (with the traversal kernel)", &names_on(BACKEND, Slot::Storage), "bricks", false),
            ParamSpec::choice("geometry", "Geometry component", &names_on(BACKEND, Slot::Geometry), "points", false),
            ParamSpec::choice("visibility", "Visibility component", &names_on(BACKEND, Slot::Visibility), "splat", false),
            ParamSpec::choice("traversal", "Traversal kernel of the lit-mode shadow rays", &names_on(BACKEND, Slot::Traversal), "refloor", false),
            ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false),
            ParamSpec::int("max_splat", "Largest splat side (px)", 1, 64, 8, true).of("visibility", &["splat"]),
            shadows_param(),
            ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
            ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
        ]
        .into_iter()
        .chain(shade_state::params())
        .collect()),
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    geometry: String,
    visibility: String,
    traversal: String,
    shading: String,
    max_splat: u32,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    shadows: Shadows,
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { storage: "bricks".into(), geometry: "points".into(), visibility: "splat".into(), traversal: "refloor".into(), shading: "direct".into(), max_splat: 8, bounces: 1, spp: 1, accumulate: true, shadows: Shadows::All, pool_mb: 2048, frames_in_flight: 3 }
    }
}

const BACKEND: &str = "gpu-splat";

pub struct GpuSplat {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// The storage instances' data: the shadow rays' (group 2), the bounces' if their own.
    storages: StorageSet,
    pipes: [wgpu::ComputePipeline; 4],
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    zbuf: wgpu::Buffer,
    idbuf: wgpu::Buffer,
    points_buf: wgpu::Buffer,
    bind0: wgpu::BindGroup,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    ring: FrameRing,
    /// The nodes of the timed stages (PLAN step 11): splatting, then the deferred pass.
    stage_nodes: [Vec<String>; 2],
    /// The geometry node's state: every chunk's points.
    points: Points,
    count: u32,
    max_splat: u32,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn pixel_buffer(gpu: &Gpu, w: u32, h: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("splat fb"), size: w as u64 * h as u64 * 4, usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
}

impl GpuSplat {
    pub fn new(p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        for (slot, name) in [(Slot::Geometry, &opts.geometry), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        // The sites' walks (PLAN step 9c): shadow rays (the storage and traversal stages),
        // bounces their own or the same.
        let g = graph::of_init(BACKEND, &p)?;
        // The points cross from the CPU (geometry) to the GPU (visibility): the schedule's
        // transfer, its adapter `upload`.
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        if !schedule.transfers.iter().any(|e| e.port == "points" && g.node(&e.to).role == Role::Visibility) {
            anyhow::bail!("{BACKEND}: the points do not cross from the CPU to the GPU");
        }
        let (walks, program, own, deps) = program(&g, &opts.shading)?;
        // The host hands the device in (GPU slot 0); this back end only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                // Region storages (tree64, ...) hold their own data instead of the voxel pool.
                upload_voxels: StorageSet::needs_pool(&walks),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: 2048,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let storages = StorageSet::new(&gpu, &walks, wgpu::ShaderStages::COMPUTE, gpu_data::Limits { mb: opts.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        let module = mr_gpu::program::compile(&gpu.device, "splat", &program, &own, &deps.iter().map(String::as_str).collect::<Vec<_>>());
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("splat"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                storage_entry(2, true),
                storage_entry(3, false),
                storage_entry(4, false),
                ShadeState::layout_entries(5, wgpu::ShaderStages::COMPUTE)[0],
                ShadeState::layout_entries(5, wgpu::ShaderStages::COMPUTE)[1],
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("splat"),
            bind_group_layouts: &storages.layouts(&[Some(&layout0), Some(world.layout())]),
            immediate_size: 0,
        });
        let pipe = |e: &str| {
            gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(e),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(e),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipes = splat::ENTRIES.map(pipe);
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<splat::Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let points_buf = Self::points_buffer(&gpu, 1024);
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let zbuf = pixel_buffer(&gpu, p.width, p.height);
        let idbuf = pixel_buffer(&gpu, p.width, p.height);
        let accum = ShadeState::new(&gpu, p.width, p.height, &opts.shading, opts.bounces, opts.spp, opts.accumulate);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &points_buf, &zbuf, &idbuf, &accum);
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()).with_stages(p.stage_times);
        Ok(Self {
            storages,
            gpu,
            world,
            pipes,
            layout0,
            camera_buf,
            output,
            zbuf,
            idbuf,
            points_buf,
            bind0,
            accum,
            ring,
            stage_nodes: [g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            points: Points::default(),
            count: 0,
            max_splat: opts.max_splat.clamp(1, 64),
            width: p.width,
            height: p.height,
            depth,
            waker,
        })
    }

    fn points_buffer(gpu: &Gpu, n: u64) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("points"),
            size: n.max(1) * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, pts: &wgpu::Buffer, z: &wgpu::Buffer, id: &wgpu::Buffer, accum: &ShadeState) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("splat"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: pts.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: z.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: id.as_entire_binding() },
                accum.bind_entries(5)[0].clone(),
                accum.bind_entries(5)[1].clone(),
            ],
        })
    }

    fn rebind(&mut self) {
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.points_buf, &self.zbuf, &self.idbuf, &self.accum);
    }

    /// The points' transfer: every chunk's points into one GPU buffer (rebuilt after any
    /// change).
    fn upload(&mut self, all: Vec<Point>) {
        self.count = all.len() as u32;
        if (all.len() as u64) * 16 > self.points_buf.size() {
            self.points_buf = Self::points_buffer(&self.gpu, all.len() as u64 * 3 / 2);
            self.rebind();
        }
        if !all.is_empty() {
            self.gpu.queue.write_buffer(&self.points_buf, 0, bytemuck::cast_slice(&all));
        }
    }
}

/// The shader: the shadow and bounce rays' walks (PLAN step 9c), the splat passes, the
/// shading component and the common deferred pass (the resolve) as the root.
fn program(g: &graph::Checked, shading: &str) -> anyhow::Result<(Walks, mr_gpu::program::Program, String, Vec<String>)> {
    let mut walks = Walks::default();
    let shadow = walks.site(g, g.site(Role::Shading, "shadow").ok_or_else(|| anyhow::anyhow!("gpu-splat: shadow rays are not bound"))?, &[2, 3, 4], false);
    let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, &[2, 3, 4], false));
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let own = format!("{}\n{}\n{shade}\n{}", splat::GPU_WGSL, ShadeState::wgsl(0, 5), mr_components::shading::DEFERRED_WGSL);
    let (program, deps) = mr_components::shaders::sites_program(&walks, 0, 0, 0, bounce);
    Ok((walks, program, own, deps))
}

impl Renderer for GpuSplat {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        let changed = self.world.apply(&self.gpu, updates);
        self.storages.update(&self.gpu, &self.world, &changed)?;
        let world = &self.world;
        self.points.update(changed, |c| world.chunk(c));
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if let Some(all) = self.points.changed() {
            self.upload(all);
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
        let cam = splat::Camera {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            origin: frac.to_array(),
            sky: u32::from_le_bytes(encode_srgb(self.world.env.sky)),
            width: self.width,
            height: self.height,
            count: self.count,
            max_splat: self.max_splat,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        self.accum.begin(&self.gpu, c, req.mode);
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let bound = self.storages.prepare(&self.gpu, &self.world);
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let groups = self.count.div_ceil(64);
        let (gx, gy) = (groups.min(65535), groups.div_ceil(65535));
        self.ring.stage(&mut enc, ticket, "splat", &self.stage_nodes[0]);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("splat"), timestamp_writes: self.ring.compute_timestamps(ticket, true, false) });
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            bound.compute(&mut pass);
            pass.set_pipeline(&self.pipes[0]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
            if groups > 0 {
                pass.set_pipeline(&self.pipes[1]);
                pass.dispatch_workgroups(gx, gy, 1);
                pass.set_pipeline(&self.pipes[2]);
                pass.dispatch_workgroups(gx, gy, 1);
            }
        }
        self.ring.stage(&mut enc, ticket, "shade", &self.stage_nodes[1]);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shade"), timestamp_writes: self.ring.compute_timestamps(ticket, false, true) });
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            bound.compute(&mut pass);
            pass.set_pipeline(&self.pipes[3]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: [("points".into(), self.count as f64), ("dropped_chunks".into(), self.world.dropped_chunks() as f64)].into_iter().chain(self.accum.counter(req.mode)).collect(),
            stages: Vec::new(),
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "max_splat" => self.max_splat = value.as_i64().unwrap_or(8).clamp(1, 64) as u32,
            "shadows" => self.world.cfg.shadows.set(value)?,
            _ if self.accum.set_param(name, value).is_some() => {}
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.zbuf = pixel_buffer(&self.gpu, width, height);
        self.idbuf = pixel_buffer(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        self.rebind();
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone()).with_stages(self.ring.stages());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let cpu = self.points.bytes();
        MemoryReport { cpu_bytes: cpu, gpu_bytes: self.world.memory_bytes() + self.storages.memory_bytes() + self.points_buf.size() + self.accum.memory_bytes() + self.width as u64 * self.height as u64 * 12 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every composition's shader composes from its modules and validates (no GPU needed).
    #[test]
    fn programs_compose() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let mut n = 0;
        for st in names_on(BACKEND, Slot::Storage) {
            for k in names_on(BACKEND, Slot::Traversal) {
                for shading in names_on(BACKEND, Slot::Shading) {
                    let flat = mr_components::Composition { storage: st.into(), traversal: k.into(), shading: shading.into(), ..mr_components::Composition::new(BACKEND) };
                    let Ok(gr) = mr_components::graph::Graph::from_composition(&flat) else { continue };
                    if gr.validate().is_err() {
                        continue;
                    }
                    let (_, p, root, deps) = program(&gr.check().unwrap(), shading).unwrap();
                    let m = p.naga(&root, &deps.iter().map(String::as_str).collect::<Vec<_>>()).unwrap_or_else(|e| panic!("{flat:?}: {e}"));
                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{flat:?}: {e:?}"));
                    n += 1;
                }
            }
        }
        assert!(n > 20, "only {n} compositions composed");
    }
}
