//! Construction: the discovery's programs, pipelines, buffers and bind groups.

use mr_gpu::Pipelines as _;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use std::collections::HashMap;

use mr_components::draw::{self, drawer::DrawNodes};
use mr_components::graph::{Checked, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state as shade_state;
use mr_components::visibility::quadtree;
use mr_core::*;
use mr_gpu::wgpu;

/// What sets the face lists' sizes (named when one is over the device's limit).
const FACES_SET_BY: &str = "`max_faces` (the face lists' capacity)";

use super::buffers::{Params, ViewUniform, make_sized, storage};
use super::knobs::{DrawKnobs, HolesKnobs, Options, ShadingKnobs, VisibilityKnobs};
use super::*;
use crate::Core;
use crate::faces::{CoreOpts, SourceSpec};

impl GpuSampler {
    pub(super) fn new(
        p: &InitParams,
        devices: &Devices,
        waker: Waker,
        g: &Checked,
    ) -> anyhow::Result<(Core, Self, Option<crate::remote::Far>)> {
        let opts: Options = p.options()?;
        let node = |role: Role| {
            g.find(role)
                .ok_or_else(|| anyhow::anyhow!("no {} node", role.name()))
        };
        let vis: VisibilityKnobs = g
            .knobs(node(Role::Visibility)?)
            .map_err(anyhow::Error::msg)?;
        let draw_id = node(Role::Draw)?;
        let draw = g.node(draw_id).component.clone();
        let dk: DrawKnobs = g.knobs(draw_id).map_err(anyhow::Error::msg)?;
        let shading = g.node(g.shading()).component.clone();
        let k: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
        // Hole detection is a node of the graph (the drawer's faces iterating through it).
        let holes = g.find(Role::Holes);
        let cover: HolesKnobs = holes
            .map_or(Ok(HolesKnobs::default()), |h| g.knobs(h))
            .map_err(anyhow::Error::msg)?;
        // The sites' walks: discovery rays (the storage and traversal stages),
        // the storage exposure tests read, shadow and bounce rays; each its own or shared.
        let mut walks = Walks::of(g);
        let bound = |role, port: &str| {
            g.site(role, port)
                .ok_or_else(|| anyhow::anyhow!("`{port}` rays are not bound"))
        };
        let rays = walks.site(g, bound(Role::Visibility, "rays")?, STORAGE_GROUPS, false);
        let exposure = walks.storage(
            g,
            g.input(Role::Visibility, "exposure")
                .ok_or_else(|| anyhow::anyhow!("exposure is not bound"))?,
            STORAGE_GROUPS,
        );
        let shadow = walks.site(g, bound(Role::Shading, "shadow")?, STORAGE_GROUPS, false);
        let bounce = g
            .site(Role::Shading, "bounce")
            .map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
        walks.set_max_steps(opts.max_steps);
        let o = CoreOpts {
            shading: &shading,
            draw: &draw,
            bounces: k.bounces,
            spp: k.spp,
            accumulate: k.accumulate,
            shadows: k.shadows,
            max_steps: opts.max_steps,
            pool_mb: opts.pool_mb,
            frames_in_flight: opts.frames_in_flight,
        };
        let (mut core, drawing, far) = crate::faces::new(
            p,
            devices,
            waker,
            g,
            &o,
            opts.depth_prepass,
            SourceSpec {
                wgsl: include_str!("../set_faces.wgsl").into(),
                walks,
                lead: Some((rays, exposure)),
                lookup: None,
                sites: [shadow, bounce],
                varyings: true,
                strip: true,
                meshes: None,
                draw_meshes: false,
                features: wgpu::Features::empty(),
                depth_read: true,
                nodes: DrawNodes {
                    draw: g.ids(&[Role::Draw]),
                    shade: g.ids(&[Role::Shading]),
                    extra: Vec::new(),
                },
            },
        )?;
        // The post's view (the drawing's group 0 never binds its quantity buffer).
        let (view, view_max) = mr_components::post::view_chosen(g, "image", 256);
        core.set_view(&view, super::report::QUANTITIES)?;
        core.view_max = view_max.clamp(1, 16384);
        let (gpu, dev) = (&core.gpu, &core.gpu.device);
        let out_format = core.output_format();
        let module = |label, src: String| mr_gpu::shader(dev, label, src);
        // Discovery's pieces: the quadtree sampler and hole detection, each
        // its own root over the shared face set (a module after the walks'). The sampler
        // reads hole detection's tiles only where they are bound (shader def `HOLES`).
        let mut program = drawing.program.clone();
        program.add(
            quadtree::SET,
            quadtree::SET_WGSL,
            &drawing.deps.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let disc_deps: Vec<&str> = std::iter::once(quadtree::SET)
            .chain(drawing.deps.iter().map(String::as_str))
            .collect();
        let discovery = |label, root: &str, program: &mr_gpu::program::Program| {
            mr_gpu::program::compile(dev, label, program, root, &disc_deps)
        };
        let mut with_holes = program.clone();
        with_holes.define("HOLES");
        let disc_mods = [
            discovery("quadtree", quadtree::GPU_WGSL, &program),
            discovery("quadtree holes", quadtree::GPU_WGSL, &with_holes),
        ];
        let holes_mod = discovery(
            "holes",
            mr_components::holes::coverage::GPU_WGSL,
            &with_holes,
        );
        let args_mod = module("discover args", include_str!("../args.wgsl").to_string());
        // The forward raster's fragment stage, for painter order and the face-id and view
        // rasters.
        // Drawing only (the shading on another GPU), no shading fragment stage (`fs`).
        let fs = if drawing.draw_only {
            String::new()
        } else {
            shade_state::fragment_wgsl(
                &shading,
                draw::gpu_wgsl("raster").unwrap(),
                "fs",
                dev.features(),
            )
        };
        let draw_mod = drawing.module(&core, "discover raster", &fs);

        let buf = |binding, ty, vis, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: dynamic,
                min_binding_size: None,
            },
            count: None,
        };
        let c = wgpu::ShaderStages::COMPUTE;
        let uni = wgpu::BufferBindingType::Uniform;
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let mut disc_entries = vec![buf(0, uni, c, false), buf(1, uni, c, true)];
        for b in 2..=10 {
            disc_entries.push(buf(b, rw, c, false));
        }
        for b in 11..=14 {
            disc_entries.push(buf(b, ro, c, false));
        }
        disc_entries.push(buf(15, rw, c, false));
        disc_entries.push(buf(16, rw, c, false));
        disc_entries.push(buf(17, rw, c, false));
        disc_entries.push(buf(18, rw, c, false));
        let tex = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: c,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        disc_entries.push(tex(19, wgpu::TextureSampleType::Uint));
        disc_entries.push(tex(20, wgpu::TextureSampleType::Depth));
        disc_entries.push(buf(21, ro, c, false));
        disc_entries.push(buf(22, rw, c, false));
        disc_entries.push(buf(23, ro, c, false));
        disc_entries.push(buf(24, rw, c, false));
        disc_entries.push(buf(25, rw, c, false));
        disc_entries.push(buf(26, rw, c, false));
        disc_entries.push(buf(27, rw, c, false));
        let layout_disc = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("discover"),
            entries: &disc_entries,
        });
        let layout_args = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("discover args"),
            entries: &[
                buf(0, uni, c, true),
                buf(1, ro, c, false),
                buf(2, rw, c, false),
                buf(3, ro, c, false),
            ],
        });
        let pl_disc = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("discover"),
            bind_group_layouts: &core
                .storages
                .layouts(&[Some(&layout_disc), Some(core.world.layout())]),
            immediate_size: 0,
        });
        let pl_args = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("discover args"),
            bind_group_layouts: &[Some(&layout_args)],
            immediate_size: 0,
        });
        let compute = |pl: &wgpu::PipelineLayout, m: &wgpu::ShaderModule, entry: &str| {
            dev.compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(pl),
                module: m,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: gpu.cache(),
            })
        };
        // Without and with hole detection's tiles.
        let pipe_survive = [0, 1].map(|h| compute(&pl_disc, &disc_mods[h], "survive"));
        let pipe_rays = [0, 1].map(|h| compute(&pl_disc, &disc_mods[h], "rays"));
        let pipe_classify = [0, 1].map(|h| compute(&pl_disc, &disc_mods[h], "classify"));
        let disc_mod = &disc_mods[0];
        let pipe_audit = compute(&pl_disc, disc_mod, "audit");
        let pipe_level_args = compute(&pl_args, &args_mod, "level_args");
        let pipe_finalize = compute(&pl_args, &args_mod, "finalize");
        let pipe_job_args = compute(&pl_args, &args_mod, "job_args");
        let pipe_footprint = compute(&pl_disc, disc_mod, "footprint");
        let pipe_mark = compute(&pl_disc, &holes_mod, "mark");
        let pipe_need_raw = compute(&pl_disc, &holes_mod, "need_raw");
        let pipe_need_final = compute(&pl_disc, &holes_mod, "need_final");
        let pipe_sky = compute(&pl_disc, &holes_mod, "sky_update");
        let pipe_need_still = compute(&pl_disc, &holes_mod, "need_still");
        let pipe_sort_hist = compute(&pl_disc, disc_mod, "sort_hist");
        let pipe_sort_scan = compute(&pl_disc, disc_mod, "sort_scan");
        let pipe_sort_scatter = compute(&pl_disc, disc_mod, "sort_scatter");
        let pipe_merge = compute(&pl_disc, disc_mod, "merge_rows");
        let pipe_merge_args = compute(&pl_args, &args_mod, "merge_args");
        let pl_draw = &drawing.pl_draw;
        let pipeline = |label, write: bool, compare, fragment: bool| {
            let targets = [Some(wgpu::ColorTargetState {
                format: out_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })];
            dev.render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pl_draw),
                vertex: wgpu::VertexState {
                    module: &draw_mod,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
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
                fragment: fragment.then(|| wgpu::FragmentState {
                    module: &draw_mod,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: gpu.cache(),
            })
        };
        // Painter order: no depth test, but depth is still written (the last fragment is the
        // nearest), since incremental mode reads it.
        let pipe_paint = (!drawing.draw_only)
            .then(|| pipeline("painter", true, wgpu::CompareFunction::Always, true));
        let pipe_id = crate::faces::category::face_raster(
            &core,
            "face ids",
            pl_draw,
            &draw_mod,
            "fs_id",
            Some(wgpu::TextureFormat::R32Uint),
            wgpu::CompareFunction::Greater,
        );

        let list_cap = vis.max_faces.clamp(1 << 16, 1 << 24);
        let table_cap = (list_cap * 2).next_power_of_two();
        let none = wgpu::BufferUsages::empty();
        let uniform = |label, size| {
            dev.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let params_buf = uniform("discover params", size_of::<Params>() as u64);
        let level_buf = uniform("discover levels", LEVEL_STRIDE * (MAX_LEVELS as u64 + 1));
        let view_buf = uniform("discover view", size_of::<ViewUniform>() as u64);
        let stats = storage(
            gpu,
            "discover counters",
            STATS_WORDS * 4,
            wgpu::BufferUsages::COPY_SRC,
            "fixed sizes",
        );
        let args = storage(
            gpu,
            "discover args",
            ARGS_WORDS * 4,
            wgpu::BufferUsages::INDIRECT,
            "fixed sizes",
        );
        let lcount = storage(
            gpu,
            "list counts",
            16,
            wgpu::BufferUsages::COPY_SRC,
            "fixed sizes",
        );
        let keys = [0, 1].map(|_| {
            storage(
                gpu,
                "face set keys",
                table_cap as u64 * 4,
                none,
                FACES_SET_BY,
            )
        });
        let vals = [0, 1].map(|_| {
            storage(
                gpu,
                "face set frames",
                table_cap as u64 * 4,
                none,
                FACES_SET_BY,
            )
        });
        let lists =
            [0, 1].map(|_| storage(gpu, "face lists", list_cap as u64 * 16, none, FACES_SET_BY));
        let hist = storage(gpu, "sort buckets", 2048 * 4, none, "fixed sizes");
        let merged = storage(
            gpu,
            "merged quads",
            list_cap as u64 * 16,
            none,
            FACES_SET_BY,
        );
        let sorted = storage(
            gpu,
            "sorted faces",
            list_cap as u64 * 16,
            none,
            FACES_SET_BY,
        );
        let ctable = storage(
            gpu,
            "chunk slot table",
            CHUNK_TABLE as u64 * 16,
            none,
            "fixed sizes",
        );
        gpu.queue.write_buffer(
            &ctable,
            0,
            bytemuck::cast_slice(&vec![[0i32, 0, 0, -1]; CHUNK_TABLE]),
        );
        let cpos = storage(
            gpu,
            "slot chunk positions",
            MAX_SLOTS as u64 * 16,
            none,
            "fixed sizes",
        );
        let bind_merged = drawing.bind(&core, &merged, &cpos);
        let bind_sorted = drawing.bind(&core, &sorted, &cpos);
        let bind_args = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("discover args"),
            layout: &layout_args,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &level_buf,
                        offset: 0,
                        size: wgpu::BufferSize::new(32),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: stats.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: args.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: lcount.as_entire_binding(),
                },
            ],
        });
        let bind_draw = [0, 1].map(|i| drawing.bind(&core, &lists[i], &cpos));
        let stages = (0..STAGES)
            .map(|_| Stage {
                buf: dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("counter stage"),
                    size: STATS_WORDS * 4 + 16 + PROFILE_TS as u64 * 8,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                state: Arc::new(AtomicU8::new(0)),
                tag: (0, 0),
            })
            .collect();
        let sized = make_sized(gpu, p.width, p.height);
        let s = Self {
            vis_nodes: g.ids(&[Role::Visibility]),
            holes_nodes: g.ids(&[Role::Holes]),
            draw_ids: g.ids(&[Role::Draw]),
            sized,
            vis,
            cover,
            order: dk.order,
            incremental: holes.is_some(),
            pipe_survive,
            pipe_rays,
            pipe_classify,
            pipe_audit,
            pipe_level_args,
            pipe_finalize,
            pipe_job_args,
            pipe_footprint,
            pipe_mark,
            pipe_need_raw,
            pipe_need_final,
            pipe_sky,
            pipe_need_still,
            pipe_sort_hist,
            pipe_sort_scan,
            pipe_sort_scatter,
            pipe_paint,
            hist,
            sorted,
            bind_sorted,
            pipe_merge,
            pipe_merge_args,
            merged,
            bind_merged,
            view_buf,
            views: None,
            kind_counts: [0; 4],
            last_kind: FrameKind::Full,
            pipe_id,
            draw_mod,
            layout_disc,
            params_buf,
            level_buf,
            stats,
            args,
            lcount,
            jobs: storage(
                gpu,
                "footprint jobs",
                JOB_CAP * 32,
                wgpu::BufferUsages::empty(),
                "fixed sizes",
            ),
            keys,
            vals,
            lists,
            ctable,
            cpos,
            seen: storage(gpu, "seen", list_cap as u64 * 4, none, FACES_SET_BY),
            pending: vec![],
            prev_pos: None,
            prev_cam: None,
            force_full: true,
            bind_disc: None,
            bind_args,
            bind_draw,
            profile: gpu.timestamps.then(|| {
                dev.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("discover phases"),
                    ty: wgpu::QueryType::Timestamp,
                    count: STAGES as u32 * PROFILE_TS,
                })
            }),
            profile_resolve: gpu.timestamps.then(|| {
                dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("phase ts"),
                    size: STAGES as u64 * 256,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                })
            }),
            ts_period: gpu.queue.get_timestamp_period(),
            stages,
            last_stats: None,
            last_parity: 0,
            slots: HashMap::new(),
            free_slots: (0..MAX_SLOTS as u32).rev().map(|s| s as u16).collect(),
            slots_dirty: false,
            chunk_probe: 0,
            table_cap,
            list_cap,
            frame: 0,
            rng: 0x9e3779b97f4a7c15,
            drawing,
            now: Frame::default(),
        };
        Ok((core, s, far))
    }

    pub(super) fn make_bind_disc(&self, core: &Core) -> [wgpu::BindGroup; 2] {
        let s = &self.sized;
        [0usize, 1].map(|par| {
            let prev = 1 - par;
            fn res(b: &wgpu::Buffer) -> wgpu::BindingResource<'_> {
                b.as_entire_binding()
            }
            let bufs: [(u32, wgpu::BindingResource); 28] = [
                (0, res(&self.params_buf)),
                (
                    1,
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.level_buf,
                        offset: 0,
                        size: wgpu::BufferSize::new(32),
                    }),
                ),
                (2, res(&self.stats)),
                (3, res(&s.cache_c)),
                (4, res(&s.cache_t)),
                (5, res(&s.claim)),
                (6, res(&s.rayq)),
                (7, res(&s.tileq)),
                (8, res(&self.keys[par])),
                (9, res(&self.vals[par])),
                (10, res(&self.lists[par])),
                (11, res(&self.vals[prev])),
                (12, res(&self.lists[prev])),
                (13, res(&self.ctable)),
                (14, res(&self.cpos)),
                (15, res(&self.lcount)),
                (16, res(&self.jobs)),
                (17, res(&self.seen)),
                (18, res(&s.need)),
                (19, wgpu::BindingResource::TextureView(&s.idtex)),
                (20, wgpu::BindingResource::TextureView(&self.drawing.depth)),
                (21, res(&s.changed_tiles)),
                (22, res(&s.sky[par])),
                (23, res(&s.sky[prev])),
                (24, res(&s.cast_frame)),
                (25, res(&self.hist)),
                (26, res(&self.sorted)),
                (27, res(&self.merged)),
            ];
            let entries: Vec<_> = bufs
                .into_iter()
                .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
                .collect();
            core.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("discover"),
                    layout: &self.layout_disc,
                    entries: &entries,
                })
        })
    }
}
