//! `gpu-voxelspace`, the voxelspace executor's GPU lane (components, IDEAS §12): storage
//! `heightmap`, visibility `columns`, shading `direct` or `pathtrace` (heightmap-gradient
//! normals on column tops; shadow and bounce rays walk the heightmap, `hm_trace`). The march
//! is the columns component's root (`mr_components::visibility::columns::GPU_WGSL`) in one
//! compute pass; shading inline in the march or deferred (`placement`, PLAN step 12c: the
//! march writes hit records, the common pass `shading/deferred.wgsl` shades them).

use std::sync::Arc;
use std::time::Instant;

use glam::{IVec2, IVec3};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::heightmap::{self, Heightmap};
use mr_components::visibility::columns;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

use crate::{Core, Lane};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-voxelspace".into(),
        description: "Comanche-style Voxel Space: per-column front-to-back heightmap march with a y-buffer (2.5D, no overhangs)".into(),
        tags: vec!["gpu".into(), "compute".into(), "heightmap".into(), "2.5d".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, vec![
            ParamSpec::choice("storage", "Storage component", &mr_components::names_on(BACKEND, mr_components::Slot::Storage), "heightmap", false),
            ParamSpec::choice("visibility", "Visibility component", &mr_components::names_on(BACKEND, mr_components::Slot::Visibility), "columns", false),
            ParamSpec::choice("shading", "Shading component", &mr_components::names_on(BACKEND, mr_components::Slot::Shading), "direct", false),
            mr_components::placement_param(BACKEND, "inline"),
            shadows_param(),
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
    visibility: String,
    shading: String,
    placement: String,
    shadows: Shadows,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    frames_in_flight: usize,
}

pub(crate) const BACKEND: &str = "gpu-voxelspace";

impl Default for Options {
    fn default() -> Self {
        Self { storage: "heightmap".into(), visibility: "columns".into(), shading: "direct".into(), placement: "inline".into(), shadows: Shadows::All, bounces: 1, spp: 1, accumulate: true, frames_in_flight: 3 }
    }
}

pub(crate) struct GpuColumns {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// The march (shading inline, or writing records), and the deferred shading pass.
    pipeline: wgpu::ComputePipeline,
    shade: Option<wgpu::ComputePipeline>,
    /// `placement = deferred`: one `pack_hit` record per pixel (fixed at creation: only then
    /// does group 0 carry them).
    records: Option<wgpu::Buffer>,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    /// The heightmap storage's GPU copy (`Core::hm` uploaded); `hm_info` = mask, columns a
    /// walk may cross.
    hm_buf: wgpu::Buffer,
    hm_info: wgpu::Buffer,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    depth: usize,
    waker: Waker,
}

impl Lane for GpuColumns {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, core: &Core) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        use mr_components::{Slot, require};
        for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        // The host hands the device in (GPU slot 0); this back end only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY })?;
        // Used for CPU chunk storage, the texture atlas and lighting parameters only.
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                upload_voxels: false,
                pool_mb: 64,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let hm = &core.hm;
        let hm_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heightmap"),
            size: hm.hm.len() as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hm_info = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("heightmap info"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        gpu.queue.write_buffer(&hm_info, 0, bytemuck::cast_slice(&hm.info()));
        // Shadow and bounce rays walk the heightmap (its `ctrace`).
        let deferred = mr_components::parse_placement(&opts.placement)?;
        let (program, root, deps) = program(&opts.shading, deferred);
        let module = mr_gpu::program::compile(&gpu.device, "voxelspace", &program, &root, &deps);
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxelspace"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                ShadeState::layout_entries(4, wgpu::ShaderStages::COMPUTE)[0],
                ShadeState::layout_entries(4, wgpu::ShaderStages::COMPUTE)[1],
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ][..if deferred { 7 } else { 6 }],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxelspace"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
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
        let (pipeline, shade) = (pipe("main"), deferred.then(|| pipe("shade")));
        let records = deferred.then(|| Self::records(&gpu, p.width, p.height));
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<columns::Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let accum = ShadeState::new(&gpu, p.width, p.height, &opts.shading, opts.bounces, opts.spp, opts.accumulate);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &hm_buf, &hm_info, &accum, records.as_ref());
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()).with_stages(p.stage_times);
        Ok(Self { gpu, world, pipeline, shade, records, layout0, camera_buf, hm_buf, hm_info, accum, output, bind0, ring, depth, waker })
    }

    fn apply(&mut self, updates: Vec<WorldUpdate>) -> Vec<IVec3> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        self.world.apply(&self.gpu, updates)
    }

    fn chunk(&self, pos: IVec3) -> Option<&ChunkData> {
        self.world.chunk(pos)
    }

    /// The heightmap's transfer to the GPU: its changed rows.
    fn heights(&mut self, hm: &Heightmap, tiles: &[IVec2]) {
        for &c in tiles {
            for (start, len) in hm.dirty_rows(c) {
                self.gpu.queue.write_buffer(&self.hm_buf, start as u64 * 4, bytemuck::cast_slice(&hm.hm[start..start + len]));
            }
        }
        self.gpu.queue.write_buffer(&self.hm_info, 0, bytemuck::cast_slice(&hm.info()));
    }

    fn render(&mut self, core: &Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (width, height) = (core.width, core.height);
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
        let cam = columns::Camera {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: width as f32 / height as f32,
            up: c.up().to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            origin: frac.to_array(),
            sky: u32::from_le_bytes(encode_srgb(self.world.env.sky)),
            width,
            height,
            _p0: 0,
            max_cells: core.max_cells,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        self.accum.begin(&self.gpu, c, req.mode);
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let deferred = self.shade.is_some();
        if deferred {
            self.ring.stage(&mut enc, ticket, "columns", &core.stage_nodes[1]);
        } else {
            self.ring.stage(&mut enc, ticket, "columns+shade", &core.stage_nodes[0]);
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("voxelspace"),
                timestamp_writes: self.ring.compute_timestamps(ticket, true, !deferred),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(64), 1, 1);
        }
        if let Some(shade) = &self.shade {
            self.ring.stage(&mut enc, ticket, "shade", &core.stage_nodes[2]);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: self.ring.compute_timestamps(ticket, false, true) });
            pass.set_pipeline(shade);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: core.counters().into_iter().chain(self.accum.counter(req.mode)).collect(),
            stages: Vec::new(),
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            _ if self.accum.set_param(name, value).is_some() => {}
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, core: &Core) -> anyhow::Result<()> {
        let (width, height) = (core.width, core.height);
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        if self.records.is_some() {
            self.records = Some(Self::records(&self.gpu, width, height));
        }
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.hm_buf, &self.hm_info, &self.accum, self.records.as_ref());
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone()).with_stages(self.ring.stages());
        Ok(())
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        let hm = core.hm.hm.len() as u64 * 4;
        (0, hm + self.world.memory_bytes() + self.accum.memory_bytes() + self.records.as_ref().map_or(0, |r| r.size()) + core.width as u64 * core.height as u64 * 4)
    }
}

impl GpuColumns {
    /// The hit records of `placement = deferred` (8 bytes per pixel).
    fn records(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("hit records"), size: (width as u64 * height as u64 * 8).max(16), usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
    }

    #[allow(clippy::too_many_arguments)]
    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, hm: &wgpu::Buffer, hm_info: &wgpu::Buffer, accum: &ShadeState, records: Option<&wgpu::Buffer>) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        let mut entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 2, resource: hm.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: hm_info.as_entire_binding() },
            accum.bind_entries(4)[0].clone(),
            accum.bind_entries(4)[1].clone(),
        ];
        entries.extend(records.map(|r| wgpu::BindGroupEntry { binding: 6, resource: r.as_entire_binding() }));
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("voxelspace"), layout, entries: &entries })
    }
}

/// The shader: the heightmap storage (group 2) is its own kernel (`ctrace`; storage and walk
/// in one, IDEAS §18's exceptions), the ray services through it, the column march and the
/// shading component as the root; with `deferred`, the march writes records and
/// the common deferred pass shades them.
fn program(shading: &str, deferred: bool) -> (mr_gpu::program::Program, String, [&'static str; 5]) {
    use mr_gpu::program::{COMMON, LIGHTING, Program, VOXELS};
    let mut p = Program::world(Some("heightmap::rays"));
    p.add("heightmap", heightmap::gpu_wgsl(0, 2), &[VOXELS]);
    mr_components::shaders::rays(&mut p, "heightmap::rays", "heightmap");
    let shade = mr_components::shading::gpu_wgsl(shading).expect("validated");
    let place = if deferred { format!("{}\n{}", columns::DEFERRED_WGSL, mr_components::shading::DEFERRED_PACKED_WGSL) } else { String::new() };
    if deferred {
        p.define("DEFERRED");
    }
    let root = format!("{}\n{}\n{shade}\n{place}", ShadeState::wgsl(0, 4), columns::GPU_WGSL);
    (p, root, ["heightmap", "heightmap::rays", LIGHTING, VOXELS, COMMON])
}

#[cfg(test)]
mod tests {
    /// The shader composes from its modules and validates for every shading and placement
    /// (no GPU needed).
    #[test]
    fn programs_compose() {
        use mr_gpu::wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        for shading in mr_components::names_on("gpu-voxelspace", mr_components::Slot::Shading) {
            for deferred in [false, true] {
                let (p, root, deps) = super::program(shading, deferred);
                let m = p.naga(&root, &deps).unwrap_or_else(|e| panic!("{shading} {deferred}: {e}"));
                Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{shading} {deferred}: {e:?}"));
            }
        }
    }
}
