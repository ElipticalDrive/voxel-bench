//! The nodes' passes: storage, hole detection, the quadtree's discovery, drawing and shading.

use mr_core::*;
use mr_components::draw::drawer::DrawFrame;
use mr_components::storage::gpu_data::Bound;
use mr_gpu::wgpu;

use super::*;
use crate::Core;

impl GpuSampler {
    /// The drawing's bind groups over the face lists (anew when the drawing's group changes).
    pub(super) fn bind_faces(&mut self, core: &Core) {
        self.bind_merged = self.drawing.bind(core, &self.merged, &self.cpos);
        self.bind_sorted = self.drawing.bind(core, &self.sorted, &self.cpos);
        self.bind_draw = [0, 1].map(|i| self.drawing.bind(core, &self.lists[i], &self.cpos));
        if let Some((list, _, bind)) = &mut self.replay_faces {
            *bind = self.drawing.bind(core, list, &self.cpos);
        }
    }

    /// Row merge active (painter order sorts single faces, so it takes precedence).
    pub(super) fn merging(&self) -> bool {
        self.vis.merge == "rows" && self.order != "painter"
    }

    pub(super) fn levels(&self) -> usize {
        (self.vis.tile.trailing_zeros() as usize + 1).min(MAX_LEVELS)
    }

    /// A timestamp `k` of the phase profile into this frame's slot (when it has one).
    fn mark(&self, enc: &mut wgpu::CommandEncoder, k: u32) {
        if let (Some(q), Some(i)) = (&self.profile, self.now.stage) {
            let base = i as u32 * PROFILE_TS;
            enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ts"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites { query_set: q, beginning_of_pass_write_index: Some(base + k), end_of_pass_write_index: None }),
            });
        }
    }

    /// The frame's groups: world, storages, discovery's.
    pub(super) fn groups(&self) -> (&wgpu::BindGroup, &Bound, &wgpu::BindGroup) {
        let f = &self.now;
        (f.world.as_ref().expect("set up"), f.bound.as_ref().expect("stored"), f.bind_disc.as_ref().expect("set up"))
    }

    /// The storages' groups for discovery's and the shadow rays, after the acceleration
    /// structures.
    pub(super) fn store(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        if std::mem::replace(&mut self.now.stored, true) {
            return;
        }
        self.now.bound = Some(Bound(core.storages.prepare(&core.gpu, &core.world).0.clone()));
        if core.build_accel(enc, ticket, req) {
            self.bind_faces(core);
        }
    }

    /// Discovery's start: an empty pass carrying the frame's begin timestamp (so the clears
    /// are timed), its first stage, this frame's set cleared.
    fn open(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.store(core, enc, ticket, req);
        if self.now.redraw || std::mem::replace(&mut self.now.opened, true) {
            return;
        }
        let parity = self.now.parity;
        enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("frame begin"), timestamp_writes: core.ring.compute_timestamps(ticket, true, false) });
        let first = if self.incremental { ("holes", &self.holes_nodes) } else { ("discover", &self.vis_nodes) };
        core.ring.stage(enc, ticket, first.0, first.1);
        self.mark(enc, 0);
        enc.clear_buffer(&self.stats, 0, None);
        enc.clear_buffer(&self.keys[parity], 0, None);
        enc.clear_buffer(&self.vals[parity], 0, None);
        enc.clear_buffer(&self.lcount, parity as u64 * 4, Some(4));
        enc.clear_buffer(&self.lcount, 8, Some(4));
    }

    /// The holes node: last frame's faces at this frame's camera (which faces are still seen,
    /// and where the image is uncovered or may reveal something new); rotation and chunk
    /// changes only (`still_holes = late`): the entering strip and changed chunks.
    pub(super) fn holes(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.open(core, enc, ticket, req);
        if self.now.redraw || !self.incremental || std::mem::replace(&mut self.now.holed, true) {
            return;
        }
        let (world_bg, storage_bg, bind_disc) = self.groups();
        let (parity, cols, rows) = (self.now.parity, self.now.cols, self.now.rows);
        let kind = self.last_kind;
        if kind == FrameKind::Still && self.cover.still_holes == "late" {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("still needs"), timestamp_writes: None });
            pass.set_bind_group(0, bind_disc, &[0]);
            pass.set_bind_group(1, world_bg, &[]);
            storage_bg.compute(&mut pass);
            pass.set_pipeline(&self.pipe_need_still);
            pass.dispatch_workgroups((cols * rows).div_ceil(256), 1, 1);
            return;
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("face ids"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.sized.idtex,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.drawing.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // Behind a gate on the faces: the list that crossed last frame.
            let (bind, args) = self.replay_faces.as_ref().map_or((&self.bind_draw[1 - parity], &self.args), |(_, a, b)| (b, a));
            pass.set_pipeline(&self.pipe_id);
            pass.set_bind_group(0, bind, &[]);
            pass.set_bind_group(1, world_bg, &[]);
            storage_bg.render(&mut pass);
            pass.draw_indirect(args, (72 + (1 - parity as u64) * 4) * 4);
        }
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("incremental needs"), timestamp_writes: None });
        pass.set_bind_group(0, bind_disc, &[0]);
        pass.set_bind_group(1, world_bg, &[]);
        storage_bg.compute(&mut pass);
        if kind != FrameKind::Still {
            // Still frames count faces in view as seen in `survive` instead.
            pass.set_pipeline(&self.pipe_mark);
            let g = (core.width * core.height).div_ceil(256);
            pass.dispatch_workgroups(g.min(32768), g.div_ceil(32768), 1);
        }
        pass.set_pipeline(&self.pipe_need_raw);
        pass.dispatch_workgroups((cols * rows).div_ceil(256), 1, 1);
        pass.set_pipeline(&self.pipe_need_final);
        pass.dispatch_workgroups((cols * rows).div_ceil(256), 1, 1);
    }

    /// The visibility node: last frame's survivors, the quadtree's levels, the list's
    /// arguments and the row merge.
    pub(super) fn discover(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.holes(core, enc, ticket, req);
        if self.now.redraw || std::mem::replace(&mut self.now.discovered, true) {
            return;
        }
        let (world_bg, storage_bg, bind_disc) = self.groups();
        let (parity, cols, rows, levels) = (self.now.parity, self.now.cols, self.now.rows, self.now.levels);
        let holes = self.incremental as usize;
        let lofs = |l: usize| (l as u64 * LEVEL_STRIDE) as u32;
        let aofs = |w: u64| w * 4;
        if self.incremental {
            core.ring.stage(enc, ticket, "discover", &self.vis_nodes);
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("survive"), timestamp_writes: None });
            pass.set_bind_group(1, world_bg, &[]);
            storage_bg.compute(&mut pass);
            // Temporal survivors from last frame's list.
            pass.set_pipeline(&self.pipe_survive[holes]);
            pass.set_bind_group(0, bind_disc, &[lofs(0)]);
            pass.dispatch_workgroups_indirect(&self.args, aofs(64 + (1 - parity as u64) * 4));
        }
        self.mark(enc, 1);
        // Level-0 work items: the grid, or the Z-curve over its enclosing power-of-two square.
        let morton = self.vis.morton;
        let l0_items = |w: u32, h: u32| if morton { w.max(h).next_power_of_two().pow(2) } else { w * h };
        for l in 0..levels {
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover rays"), timestamp_writes: None });
                if l > 0 {
                    pass.set_pipeline(&self.pipe_level_args);
                    pass.set_bind_group(0, &self.bind_args, &[lofs(l)]);
                    pass.dispatch_workgroups(1, 1, 1);
                }
                pass.set_bind_group(0, bind_disc, &[lofs(l)]);
                pass.set_bind_group(1, world_bg, &[]);
                storage_bg.compute(&mut pass);
                pass.set_pipeline(&self.pipe_rays[holes]);
                if l == 0 {
                    pass.dispatch_workgroups(l0_items(cols + 1, rows + 1).div_ceil(256), 1, 1);
                } else {
                    pass.dispatch_workgroups_indirect(&self.args, aofs(l as u64 * 8));
                }
            }
            self.mark(enc, 2 + 2 * l as u32);
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover classify"), timestamp_writes: None });
                pass.set_bind_group(0, bind_disc, &[lofs(l)]);
                pass.set_bind_group(1, world_bg, &[]);
                storage_bg.compute(&mut pass);
                pass.set_pipeline(&self.pipe_classify[holes]);
                if l == 0 {
                    pass.dispatch_workgroups(l0_items(cols, rows).div_ceil(256), 1, 1);
                } else {
                    pass.dispatch_workgroups_indirect(&self.args, aofs(l as u64 * 8 + 4));
                }
                pass.set_pipeline(&self.pipe_job_args);
                pass.set_bind_group(0, &self.bind_args, &[lofs(l)]);
                pass.dispatch_workgroups(1, 1, 1);
                pass.set_pipeline(&self.pipe_footprint);
                pass.set_bind_group(0, bind_disc, &[lofs(l)]);
                pass.dispatch_workgroups_indirect(&self.args, aofs(80 + l as u64 * 4));
            }
            self.mark(enc, 3 + 2 * l as u32);
        }
        // Unused level slots still get written so every resolved query is available.
        for l in levels..MAX_LEVELS {
            self.mark(enc, 2 + 2 * l as u32);
            self.mark(enc, 3 + 2 * l as u32);
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover finalize"), timestamp_writes: None });
            pass.set_pipeline(&self.pipe_finalize);
            pass.set_bind_group(0, &self.bind_args, &[lofs(0)]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.mark(enc, 2 + 2 * MAX_LEVELS as u32);
        if self.merging() {
            core.ring.stage(enc, ticket, "merge", &self.vis_nodes);
            // Merge rows of faces into runs (timed with the raster).
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("row merge"), timestamp_writes: None });
            pass.set_bind_group(0, bind_disc, &[lofs(0)]);
            pass.set_bind_group(1, world_bg, &[]);
            storage_bg.compute(&mut pass);
            pass.set_pipeline(&self.pipe_merge);
            pass.dispatch_workgroups_indirect(&self.args, aofs(64 + parity as u64 * 4));
            pass.set_pipeline(&self.pipe_merge_args);
            pass.set_bind_group(0, &self.bind_args, &[lofs(0)]);
            pass.dispatch_workgroups(1, 1, 1);
        }
    }

    /// The drawer's passes over list `parity` (depth pre-pass + equal-depth shading in lit
    /// mode); a static frame's carry the frame's begin timestamp too.
    fn encode(&self, core: &Core, ticket: mr_gpu::Ticket, req: &FrameRequest, pass: impl FnOnce(&DrawFrame, &dyn Fn(&mut wgpu::RenderPass<'_>), Option<&wgpu::RenderPipeline>)) {
        let f = &self.now;
        let painter = self.order == "painter";
        let sky = core.sky();
        let clear = wgpu::Color { r: sky[0], g: sky[1], b: sky[2], a: 1.0 };
        let (bind_draw, _, draw_ofs) = self.drawn();
        let (st, more) = f.bound.as_ref().expect("stored").split();
        let frame = DrawFrame {
            out: f.out.as_ref().expect("set up"),
            depth: &self.drawing.depth,
            ids: self.drawing.ids(core),
            clear,
            prepass: self.drawing.depth_prepass && req.mode == RenderMode::Lit && !painter,
            keep_depth: self.incremental,
            binds: &[bind_draw, f.world.as_ref().expect("set up"), st.expect("discovery rays walk a storage")],
            more: &more,
            mesh_verts: None,
            ring: &core.ring,
            ticket,
            ts: (f.redraw, true),
            nodes: &self.drawing.draw_nodes,
        };
        let draw = |pass: &mut wgpu::RenderPass<'_>| pass.draw_indirect(&self.args, draw_ofs);
        let main = painter.then(|| self.pipe_paint.as_ref().expect("painter order draws forward (refused when drawing only)"));
        pass(&frame, &draw, main);
    }

    /// The list the drawer draws this frame (painter order's sorted, the row merge's runs or
    /// the frame's list): its draw bind group, its buffer and its indirect arguments' offset.
    pub(super) fn drawn(&self) -> (&wgpu::BindGroup, &wgpu::Buffer, u64) {
        let parity = self.now.parity;
        if self.order == "painter" {
            (&self.bind_sorted, &self.sorted, (72 + parity as u64 * 4) * 4)
        } else if self.merging() {
            (&self.bind_merged, &self.merged, 108 * 4)
        } else {
            (&self.bind_draw[parity], &self.lists[parity], (72 + parity as u64 * 4) * 4)
        }
    }

    /// The draw node: painter order's sort (far to near), then one instanced quad per face of
    /// the list (indirect), shaded in the fragments when forward.
    pub(super) fn draw(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.discover(core, enc, ticket, req);
        if std::mem::replace(&mut self.now.drawn, true) {
            return;
        }
        if !self.now.redraw && self.order == "painter" {
            // Sort the list back to front (timed with the raster).
            let (world_bg, storage_bg, bind_disc) = self.groups();
            let aofs = 4 * (64 + self.now.parity as u64 * 4);
            core.ring.stage(enc, ticket, "sort", &self.draw_ids);
            enc.clear_buffer(&self.hist, 0, None);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("painter sort"), timestamp_writes: None });
            pass.set_bind_group(0, bind_disc, &[0]);
            pass.set_bind_group(1, world_bg, &[]);
            storage_bg.compute(&mut pass);
            pass.set_pipeline(&self.pipe_sort_hist);
            pass.dispatch_workgroups_indirect(&self.args, aofs);
            pass.set_pipeline(&self.pipe_sort_scan);
            pass.dispatch_workgroups(1, 1, 1);
            pass.set_pipeline(&self.pipe_sort_scatter);
            pass.dispatch_workgroups_indirect(&self.args, aofs);
        }
        self.encode(core, ticket, req, |f, draw, main| {
            self.drawing.drawer.encode_draw(enc, f, draw, None, main);
            self.drawing.drawer.encode_decode(&core.gpu, enc, f);
        });
    }

    /// The shading node: the deferred drawers' pass over the ids or records.
    pub(super) fn shade(&mut self, core: &mut Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, req: &FrameRequest) {
        self.draw(core, enc, ticket, req);
        self.encode(core, ticket, req, |f, _, _| self.drawing.drawer.encode_shade(&core.gpu, enc, f, false, false));
        if !self.now.redraw {
            self.mark(enc, 3 + 2 * MAX_LEVELS as u32);
        }
    }
}
