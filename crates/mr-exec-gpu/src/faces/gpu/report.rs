//! Diagnostics: the counters read back and the quantities the post's views show, with the
//! passes that write them (all only while one is viewed).

use mr_gpu::Pipelines as _;
use std::sync::atomic::Ordering;

use mr_core::*;
use mr_gpu::wgpu;

use super::buffers::ViewUniform;
use super::*;
use crate::Core;
use crate::faces::category::CategoryRaster;

/// What the sampler writes for the post's view.
pub(super) const QUANTITIES: &[Quantity] = &[
    Quantity::categorical(
        "tiles",
        "why level-0 tiles were discovered this frame",
        "level-0 tiles by why they were discovered this frame (white full, blue edited block / changed chunk, cyan entering strip (still_holes = late), red uncovered or hole, yellow parallax, green refresh; dark = skipped)",
        &[
            ("full frame", Some([255, 255, 255])),
            ("edited block / changed chunk", Some([64, 102, 255])),
            ("entering strip", Some([26, 230, 255])),
            ("uncovered or hole", Some([255, 51, 38])),
            ("parallax", Some([255, 217, 26])),
            ("refresh", Some([51, 230, 77])),
            ("skipped (darkened)", Some([0, 0, 0])),
        ],
    ),
    Quantity::categorical(
        "rays",
        "where discovery cast rays this frame",
        "a dot per ray cast this frame",
        &[
            ("ray cast this frame", Some([255, 255, 51])),
            ("no ray (darkened)", Some([0, 0, 0])),
        ],
    ),
    Quantity::categorical(
        "churn",
        "faces new to the set",
        "faces new to the set this frame in red, carried-over faces grey",
        &[
            ("face new to the set this frame", Some([255, 38, 26])),
            ("carried over (grey)", Some([128, 128, 128])),
        ],
    ),
    Quantity::ramp(
        "overdraw",
        "fragments drawn per pixel",
        "fragments drawn per pixel by the face list in its order (linear; depth-tested, so compare order = depth vs painter)",
        false,
        false,
    ),
    Quantity::categorical(
        "quads",
        "drawn quads",
        "one colour per drawn quad (shows merging)",
        &[("one random colour per drawn quad", None)],
    ),
];

/// The passes of the view chosen, made on its first frame.
pub(super) struct ViewPasses {
    name: &'static str,
    size: (u32, u32),
    /// `tiles`, `rays` (`view.wgsl`).
    writer: Option<wgpu::ComputePipeline>,
    /// `churn`, `quads`: the list's category raster.
    raster: Option<CategoryRaster>,
    /// `overdraw`: group 0 with the quantity buffer, the counting raster (depth-tested,
    /// painter) and its depth.
    count: Option<(
        wgpu::BindGroupLayout,
        [wgpu::RenderPipeline; 2],
        wgpu::TextureView,
    )>,
}

impl GpuSampler {
    /// The passes of view `name` at the core's size.
    fn view_passes(&self, core: &Core, name: &'static str) -> ViewPasses {
        let (gpu, dev) = (&core.gpu, &core.gpu.device);
        let (w, h) = (core.width, core.height);
        let mut v = ViewPasses {
            name,
            size: (w, h),
            writer: None,
            raster: None,
            count: None,
        };
        let writer = |entry: &str| {
            let src = format!("{}{}", crate::view::wgsl(), include_str!("../view.wgsl"));
            let module = mr_gpu::shader(dev, "discover view", src);
            // Its own layout: the bindings the entry point uses.
            dev.compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: gpu.cache(),
            })
        };
        match name {
            "tiles" | "rays" => v.writer = Some(writer(name)),
            "churn" | "quads" => {
                let entry = if name == "churn" {
                    "fs_churn"
                } else {
                    "fs_quads"
                };
                v.raster = Some(CategoryRaster::new(
                    core,
                    name,
                    &self.drawing.pl_draw,
                    &self.draw_mod,
                    entry,
                ));
            }
            "overdraw" => {
                // The drawing's group 0 with the quantity buffer, and its program counting.
                let mut entries = core.layout_entries_with(false);
                let buf = |binding| {
                    crate::storage_entry(
                        binding,
                        true,
                        wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    )
                };
                entries.extend([buf(crate::REGION_BINDING), buf(crate::REGION_BINDING + 1)]);
                let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("overdraw"),
                    entries: &entries,
                });
                let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("overdraw"),
                    bind_group_layouts: &core
                        .storages
                        .layouts(&[Some(&layout), Some(core.world.layout())]),
                    immediate_size: 0,
                });
                let d = &self.drawing;
                let mut program = d.program.clone();
                program.define("VIEW_OVERDRAW");
                if dev
                    .features()
                    .contains(mr_components::shading::state::FRAGMENT_FEATURE)
                {
                    program.define("EARLY_DEPTH");
                }
                let module = crate::faces::DrawParts::compile(
                    gpu,
                    "overdraw",
                    &program,
                    &d.deps,
                    &format!("{}\n{}", d.draw_src, crate::view::atomic_wgsl()),
                );
                let pipes = [
                    wgpu::CompareFunction::Greater,
                    wgpu::CompareFunction::Always,
                ]
                .map(|c| {
                    crate::faces::category::face_raster(
                        core,
                        "overdraw",
                        &pl,
                        &module,
                        "fs_overdraw",
                        None,
                        c,
                    )
                });
                v.count = Some((layout, pipes, crate::faces::depth_view(gpu, w, h, false)));
            }
            _ => {}
        }
        v
    }

    /// The post's view's quantity, where it is the sampler's: the drawn list's category
    /// raster or overdraw count, the discovery's tiles or rays (a static frame's: none).
    pub(super) fn encode_view(
        &mut self,
        core: &Core,
        enc: &mut wgpu::CommandEncoder,
        ticket: mr_gpu::Ticket,
    ) {
        let Some(viewing) = &core.view else { return };
        let name = viewing.quantity.name;
        if !QUANTITIES.iter().any(|q| q.name == name) {
            return;
        }
        if self
            .views
            .as_ref()
            .is_none_or(|v| v.name != name || v.size != (core.width, core.height))
        {
            self.views = Some(self.view_passes(core, name));
        }
        // A diagnostic, not a node's work.
        core.ring.stage(enc, ticket, "view", &[]);
        let f = &self.now;
        let v = self.views.as_ref().unwrap();
        let (world, bound) = (
            f.world.as_ref().expect("set up"),
            f.bound.as_ref().expect("stored"),
        );
        let (bind_list, list, draw_ofs) = self.drawn();
        if let Some((layout, pipes, depth)) = &v.count {
            let mut entries = core.bind_entries_with(None);
            entries.extend([
                wgpu::BindGroupEntry {
                    binding: crate::REGION_BINDING,
                    resource: list.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: crate::REGION_BINDING + 1,
                    resource: self.cpos.as_entire_binding(),
                },
            ]);
            let bind = core
                .gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("overdraw"),
                    layout,
                    entries: &entries,
                });
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overdraw"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipes[(self.order == "painter") as usize]);
            pass.set_bind_group(0, &bind, &[]);
            pass.set_bind_group(1, world, &[]);
            bound.render(&mut pass);
            pass.draw_indirect(&self.args, draw_ofs);
            return;
        }
        // Nothing is new in a static frame.
        if name == "churn" && f.redraw {
            return;
        }
        if let Some(r) = &v.raster {
            r.encode(core, enc, bind_list, world, bound, |pass| {
                pass.draw_indirect(&self.args, draw_ofs)
            });
            return;
        }
        let Some(writer) = &v.writer else { return };
        let u = ViewUniform {
            frame: if f.redraw { 0 } else { self.frame },
            tile: self.vis.tile,
            cols: f.cols,
            rows: f.rows,
            need_off: self.sized.need_off,
            lat_w: self.sized.lat_w,
            off_x: f.off.0,
            off_y: f.off.1,
            width: core.width,
            height: core.height,
            _p: [0; 2],
        };
        core.gpu
            .queue
            .write_buffer(&self.view_buf, 0, bytemuck::bytes_of(&u));
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: self.view_buf.as_entire_binding(),
            },
            viewing.bind_entry(),
        ];
        match name {
            "tiles" => entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: self.sized.need.as_entire_binding(),
            }),
            _ => entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: self.sized.cast_frame.as_entire_binding(),
            }),
        }
        let bind = core
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("discover view"),
                layout: &writer.get_bind_group_layout(0),
                entries: &entries,
            });
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("discover view"),
            timestamp_writes: None,
        });
        pass.set_pipeline(writer);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(core.width.div_ceil(8), core.height.div_ceil(8), 1);
    }

    /// Takes counters of the most recent finished frame from the staging buffers.
    pub(super) fn harvest(&mut self) {
        let mut newest: Option<(u32, Vec<u32>)> = None;
        let mut parity = 0;
        for st in &self.stages {
            if st.state.load(Ordering::SeqCst) == 2 {
                let words: Vec<u32> = {
                    let v = st.buf.get_mapped_range(..).expect("mapped");
                    bytemuck::cast_slice(&v[..]).to_vec()
                };
                st.buf.unmap();
                st.state.store(0, Ordering::SeqCst);
                let (frame, par) = st.tag;
                if newest
                    .as_ref()
                    .is_none_or(|(f, _)| frame.wrapping_sub(*f) as i32 > 0)
                {
                    newest = Some((frame, words));
                    parity = par;
                }
            }
        }
        if let Some((_, w)) = newest {
            self.last_stats = Some(w);
            self.last_parity = parity as usize;
        }
    }

    pub(super) fn counters(&self, core: &Core, mode: RenderMode) -> Vec<(String, f64)> {
        let mut out = vec![("chunk_slots".into(), self.slots.len() as f64)];
        out.extend(core.accum.counter(mode));
        out.extend(core.accel_stats().0);
        if self.incremental && self.cover.events {
            // 0 static (skipped), 1 still (rotation / chunk changes), 2 moving, 3 full.
            out.push(("frame_kind".into(), self.last_kind as u32 as f64));
            let total = self.kind_counts.iter().sum::<u64>().max(1) as f64;
            for (k, name) in ["static", "still", "moving", "full"].iter().enumerate() {
                out.push((
                    format!("frames_{name}_pct"),
                    self.kind_counts[k] as f64 / total * 100.0,
                ));
            }
        }
        let Some(w) = &self.last_stats else {
            return out;
        };
        if self.last_kind == FrameKind::Static {
            // Nothing was discovered: report the redraw, not the last discovery's work.
            let faces = w[STATS_WORDS as usize + self.last_parity] as f64;
            out.extend([
                ("rays".into(), 0.0),
                ("rays_per_px".into(), 0.0),
                ("faces_s".into(), faces),
                ("faces_new".into(), 0.0),
                ("faces_dropped".into(), 0.0),
                ("tiles_needed_pct".into(), 0.0),
            ]);
            return out;
        }
        let levels = self.levels();
        let rays: u64 = (0..levels).map(|l| w[8 + l] as u64).sum();
        let px = (core.width * core.height) as f64;
        let faces = w[STATS_WORDS as usize + self.last_parity];
        if self.profile.is_some() {
            let base = STATS_WORDS as usize + 4;
            let ts: Vec<u64> = (0..PROFILE_TS as usize)
                .map(|k| w[base + 2 * k] as u64 | (w[base + 2 * k + 1] as u64) << 32)
                .collect();
            let ms = |a: usize, b: usize| {
                if ts[b] > ts[a] {
                    (ts[b] - ts[a]) as f64 * self.ts_period as f64 / 1e6
                } else {
                    0.0
                }
            };
            out.push(("ms_clear_survive".into(), ms(0, 1)));
            let (mut r, mut c) = (0.0, 0.0);
            for l in 0..levels {
                let (a, b) = (ms(1 + 2 * l, 2 + 2 * l), ms(2 + 2 * l, 3 + 2 * l));
                out.push((format!("ms_L{l}_rays"), a));
                out.push((format!("ms_L{l}_classify"), b));
                r += a;
                c += b;
            }
            out.push(("ms_rays".into(), r));
            out.push(("ms_classify".into(), c));
            let raster = PROFILE_TS as usize - 1;
            out.push(("ms_raster".into(), ms(raster - 1, raster)));
        }
        out.push(("rays".into(), rays as f64));
        out.push(("rays_per_px".into(), rays as f64 / px));
        out.push(("faces_s".into(), faces as f64));
        out.push(("survivors".into(), w[32] as f64));
        out.push(("faces_from_tiles".into(), w[39] as f64));
        out.push(("faces_from_1px".into(), w[38] as f64));
        out.push(("footprint_rejects".into(), w[37] as f64));
        out.push(("overflow".into(), w[34] as f64));
        out.push(("closure_tiles".into(), w[40] as f64));
        out.push(("faces_from_closure".into(), w[41] as f64));
        out.push(("no_chunk_slot".into(), w[35] as f64));
        for l in 0..levels {
            out.push((
                format!("acc_L{l}_{}px", self.vis.tile >> l),
                w[16 + l] as f64,
            ));
            out.push((format!("sky_L{l}"), w[24 + l] as f64));
            out.push((format!("tiles_L{l}"), w[l] as f64));
            out.push((format!("jobs_L{l}"), w[48 + l] as f64));
            out.push((format!("rays_L{l}"), w[8 + l] as f64));
        }
        let prev_faces = w[STATS_WORDS as usize + 1 - self.last_parity] as f64;
        out.push(("faces_new".into(), w[44] as f64));
        if self.merging() {
            let quads = w[STATS_WORDS as usize + 2] as f64;
            out.push(("quads".into(), quads));
            out.push(("faces_per_quad".into(), faces as f64 / quads.max(1.0)));
        }
        out.push(("faces_dropped".into(), (prev_faces - w[32] as f64).max(0.0)));
        if self.incremental {
            let tiles =
                (core.width.div_ceil(self.vis.tile) * core.height.div_ceil(self.vis.tile)) as f64;
            out.push(("tiles_needed".into(), w[42] as f64));
            out.push(("tiles_needed_pct".into(), w[42] as f64 / tiles * 100.0));
            out.push(("reveal_clamped".into(), w[43] as f64));
        }
        if self.vis.audit {
            out.push(("audit_miss_px".into(), w[33] as f64));
            out.push(("audit_miss_pct".into(), w[33] as f64 / px * 100.0));
            out.push(("audit_hit_px".into(), w[36] as f64));
        }
        out
    }
}
