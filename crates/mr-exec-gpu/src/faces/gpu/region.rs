//! The frame's life: world updates and the chunk slot table, knob changes, resizes, each
//! frame's setup (its kind, camera and uniforms) and finish.

use std::sync::atomic::Ordering;

use bytemuck::Zeroable;
use glam::IVec3;
use mr_core::*;
use mr_components::graph::Role;
use mr_gpu::voxels::hash3;
use mr_gpu::wgpu;

use super::buffers::{LevelUniform, Params, make_sized};
use super::*;
use crate::Core;
use crate::exec::Region;

impl GpuSampler {
    /// Keeps one slot per loaded non-air chunk; face keys index chunks by slot.
    fn update_slots(&mut self, core: &Core, changed: &[IVec3]) {
        for &pos in changed {
            let present = core.world.chunk(pos).is_some();
            match (present, self.slots.get(&pos).copied()) {
                (true, None) => match self.free_slots.pop() {
                    Some(s) => {
                        self.slots.insert(pos, s);
                        self.slots_dirty = true;
                    }
                    None => core.gpu.fail(format!("discover-gpu: more than {MAX_SLOTS} non-air chunks; frames would miss the faces of the rest; lower the client's radius")),
                },
                (false, Some(s)) => {
                    self.slots.remove(&pos);
                    self.free_slots.push(s);
                    self.slots_dirty = true;
                }
                _ => {}
            }
        }
    }

    fn upload_slots(&mut self, core: &Core) {
        if !self.slots_dirty {
            return;
        }
        self.slots_dirty = false;
        let mut table = vec![[0i32, 0, 0, -1]; CHUNK_TABLE];
        let mut pos = vec![[0i32; 4]; MAX_SLOTS];
        let mask = CHUNK_TABLE - 1;
        let mut max_probe = 0;
        for (p, &s) in &self.slots {
            let mut i = hash3(*p) as usize & mask;
            let mut probe = 0;
            while table[i][3] >= 0 {
                i = (i + 1) & mask;
                probe += 1;
            }
            max_probe = max_probe.max(probe);
            table[i] = [p.x, p.y, p.z, s as i32];
            pos[s as usize] = [p.x, p.y, p.z, 1];
        }
        self.chunk_probe = max_probe;
        core.gpu.queue.write_buffer(&self.ctable, 0, bytemuck::cast_slice(&table));
        core.gpu.queue.write_buffer(&self.cpos, 0, bytemuck::cast_slice(&pos));
    }
}

impl exec::State for GpuSampler {
    fn updating(&mut self, updates: &[WorldUpdate]) {
        if self.incremental {
            // Event areas for the next frame: edited blocks exactly, chunk loads whole.
            for u in updates {
                match u {
                    WorldUpdate::SetBlocks(edits) => self.pending.extend(edits.iter().map(|e| (e.pos, 1))),
                    WorldUpdate::ChunkLoad { key, .. } | WorldUpdate::ChunkUnload { key } if key.lod == 0 => self.pending.push((key.pos * 32, 32)),
                    _ => {}
                }
            }
        }
    }

    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()> {
        self.update_slots(core, changed);
        Ok(())
    }

    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        self.set_knob(name, value).or_else(|| self.drawing.set_param(core, name, value))
    }

    /// Hole detection comes and goes with its node; the drawer is the drawing's.
    fn switch(&mut self, core: &mut Core, s: &mr_components::graph::Switch) -> Option<anyhow::Result<()>> {
        if s.role != Role::Holes {
            return self.drawing.switch(core, s);
        }
        self.incremental = s.component.is_some();
        self.pending.clear();
        Some(Ok(()))
    }

    fn resize(&mut self, core: &Core) {
        self.drawing.resize(core);
        self.sized = make_sized(&core.gpu, core.width, core.height);
        self.bind_faces(core);
        self.bind_disc = None;
        self.views = None;
        self.force_full = true;
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        let s = &self.sized;
        let set = (self.keys[0].size() + self.vals[0].size() + self.lists[0].size()) * 2 + self.jobs.size() + self.seen.size() + s.changed_tiles.size() + s.need.size() + core.width as u64 * core.height as u64 * 4;
        (0, set + s.cache_c.size() + s.cache_t.size() + s.claim.size() + s.rayq.size() + s.tileq.size() + self.ctable.size() + self.cpos.size() + self.drawing.memory_bytes(core))
    }
}

impl Region for GpuSampler {
    fn compile(&mut self, _: &mut Core, req: &FrameRequest) -> anyhow::Result<()> {
        if self.drawing.drawer.mode().deferred() && self.order == "painter" {
            anyhow::bail!("order = painter needs draw = raster (it draws forward)");
        }
        self.drawing.check_state(req.mode, Some(self.order == "painter"))
    }

    /// The frame's discovery settings (its kind: static when nothing that affects visibility
    /// changed, else still, moving or full), camera and world.
    fn setup(&mut self, core: &mut Core, req: &FrameRequest) {
        core.accum.begin(&core.gpu, &req.camera, req.mode);
        self.harvest();
        self.upload_slots(core);
        let events = self.incremental && self.cover.events;
        let c = &req.camera;
        let tan_now = (c.fov_y * 0.5).tan();
        let same_pos = self.prev_pos == Some(c.position);
        let same_rot = self.prev_cam.is_some_and(|p| p == (c.forward().to_array(), c.right().to_array(), c.up().to_array(), tan_now));
        if events && same_pos && same_rot && self.pending.is_empty() && !self.force_full && self.frame > 0 {
            // Nothing that affects visibility changed: redraw the last face list (only
            // shading runs: lights may move).
            self.last_kind = FrameKind::Static;
            self.kind_counts[FrameKind::Static as usize] += 1;
            self.drawing.write_frame(core, c, req.mode, 0, IVec3::ZERO);
            let tile = self.vis.tile;
            let world = Some(core.world.prepare(&core.gpu, c).clone());
            self.now = Frame { redraw: true, parity: (self.frame & 1) as usize, cols: core.width.div_ceil(tile), rows: core.height.div_ceil(tile), world, out: Some(core.output.create_view(&Default::default())), ..Frame::default() };
            return;
        }
        self.frame = self.frame.wrapping_add(1).max(1);
        let parity = (self.frame & 1) as usize;
        let tile = self.vis.tile.next_power_of_two().clamp(2, MAX_TILE);
        self.vis.tile = tile;
        let levels = self.levels();
        let (off_x, off_y) = if self.vis.jitter && !self.incremental {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            ((self.rng as u32) % tile, ((self.rng >> 32) as u32) % tile)
        } else {
            (0, 0)
        };
        let (width, height) = (core.width, core.height);
        let cols = (width + off_x).div_ceil(tile);
        let rows = (height + off_y).div_ceil(tile);
        let (_, frac) = c.split_position();
        let aspect = width as f32 / height as f32;
        let tan_half_fov = (c.fov_y * 0.5).tan();
        let queue_cap = self.sized.lat_w * self.sized.lat_h;
        // Incremental mode: camera translation since last frame, and changed chunks' tiles.
        let moved = self.prev_pos.map(|p| (c.position - p).length()).unwrap_or(f64::INFINITY);
        self.prev_pos = Some(c.position);
        let f_px = height as f32 * 0.5 / tan_half_fov;
        let mut full = self.force_full || moved > 64.0;
        let mut any_changed = false;
        if self.incremental && !self.pending.is_empty() {
            let vp = c.projection(aspect) * c.view_rotation();
            let mut mask = vec![0u32; (cols * rows) as usize];
            for (lo, size) in self.pending.drain(..) {
                match box_rect(&vp, lo, size, c.position, width, height, tile, cols, rows) {
                    Some(Some([x0, y0, x1, y1])) => {
                        for y in y0..=y1 {
                            mask[(y * cols + x0) as usize..=(y * cols + x1) as usize].fill(1);
                        }
                        any_changed = true;
                    }
                    Some(None) => {}
                    None => full = true,
                }
            }
            if any_changed {
                core.gpu.queue.write_buffer(&self.sized.changed_tiles, 0, bytemuck::cast_slice(&mask));
            }
        }
        let kind = if full {
            FrameKind::Full
        } else if events && same_pos {
            FrameKind::Still
        } else {
            FrameKind::Moving
        };
        self.last_kind = kind;
        self.kind_counts[kind as usize] += 1;
        let prev_cam = if full { None } else { self.prev_cam };
        self.prev_cam = Some((c.forward().to_array(), c.right().to_array(), c.up().to_array(), tan_half_fov));
        self.force_full = false;
        let prm = Params {
            forward: c.forward().to_array(),
            tan_half_fov,
            right: c.right().to_array(),
            aspect,
            up: c.up().to_array(),
            frame: self.frame,
            origin: frac.to_array(),
            tile,
            width,
            height,
            off_x,
            off_y,
            lat_w: self.sized.lat_w,
            lat_h: self.sized.lat_h,
            cols,
            rows,
            keep: if self.incremental { self.cover.keep_unseen } else { self.vis.keep_frames }.max(1),
            criterion: (self.vis.criterion == "face") as u32,
            sky_tile: self.vis.sky_tile,
            max_fp: self.vis.max_footprint,
            table_mask: self.table_cap - 1,
            list_cap: self.list_cap,
            chunk_mask: CHUNK_TABLE as u32 - 1,
            chunk_probe: self.chunk_probe,
            queue_cap,
            parity: parity as u32,
            strict: self.vis.strict as u32,
            still: (kind == FrameKind::Still) as u32,
            closure_px: self.vis.closure_px,
            closure_box: self.vis.closure_box as i32,
            block_faces: self.vis.block_faces as u32,
            inline_fp: self.vis.inline_fp,
            occluders: self.vis.occluders,
            refresh: self.cover.refresh,
            par_k: if moved.is_finite() { f_px * moved as f32 / c.near } else { 0.0 },
            changed: any_changed as u32,
            full: full as u32,
            r_max: self.cover.reveal_tiles.max(1),
            need_off: self.sized.need_off,
            par_thresh: self.cover.parallax_px,
            prev_forward: prev_cam.map(|p| p.0).unwrap_or_default(),
            prev_tan: prev_cam.map(|p| p.3).unwrap_or(1.0),
            prev_right: prev_cam.map(|p| p.1).unwrap_or_default(),
            has_prev: prev_cam.is_some() as u32,
            prev_up: prev_cam.map(|p| p.2).unwrap_or_default(),
            keep_out: self.vis.keep_frames.max(1),
            morton: self.vis.morton as u32,
            morton_side: (cols + 1).max(rows + 1).next_power_of_two(),
            _q: [0; 2],
        };
        core.gpu.queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&prm));
        let mut lvls = vec![LevelUniform::zeroed(); MAX_LEVELS + 1];
        for (l, u) in lvls.iter_mut().enumerate() {
            *u = LevelUniform {
                level: l as u32,
                size: tile >> l.min(31),
                in_off: (l as u32 & 1) * queue_cap,
                out_off: ((l as u32 + 1) & 1) * queue_cap,
                parity: parity as u32,
                cap: queue_cap,
                list_cap: self.list_cap,
                _p: 0,
                _pad: [0; 56],
            };
        }
        core.gpu.queue.write_buffer(&self.level_buf, 0, bytemuck::cast_slice(&lvls));
        self.drawing.write_frame(core, c, req.mode, 0, IVec3::ZERO);
        let world = core.world.prepare(&core.gpu, c).clone();
        if self.bind_disc.is_none() {
            self.bind_disc = Some(self.make_bind_disc(core));
        }
        let bind_disc = self.bind_disc.as_ref().unwrap()[parity].clone();
        // Counter/timestamp staging buffer for this frame (if one is free).
        let stage = self.stages.iter().position(|s| s.state.load(Ordering::SeqCst) == 0);
        self.now = Frame {
            parity,
            off: (off_x, off_y),
            cols,
            rows,
            levels,
            world: Some(world),
            bind_disc: Some(bind_disc),
            out: Some(core.output.create_view(&Default::default())),
            stage,
            ..Frame::default()
        };
    }

    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        (self.counters(core, req.mode), None, core.accel_stats().1)
    }

    fn quantities(&self) -> &'static [Quantity] {
        super::report::QUANTITIES
    }

    fn view(&mut self, core: &Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket) {
        self.encode_view(core, enc, ticket);
    }

    /// After the timed passes: hole detection's sky mask (from this frame's drawing, for the
    /// next), the audit (so it does not inflate gpu_ms) and the counters' copy into the
    /// staging buffer.
    fn finish(&mut self, core: &Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket) {
        let Frame { redraw, parity, stage, .. } = self.now;
        let (width, height) = (core.width, core.height);
        if !redraw {
            let (world_bg, storage_bg, bind_disc) = self.groups();
            let pixels = |pass: &mut wgpu::ComputePass<'_>, pipe: &wgpu::ComputePipeline| {
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, bind_disc, &[0]);
                pass.set_bind_group(1, world_bg, &[]);
                storage_bg.compute(pass);
                let g = (width * height).div_ceil(256);
                pass.dispatch_workgroups(g.min(32768), g.div_ceil(32768), 1);
            };
            if self.incremental {
                core.ring.stage(enc, ticket, "sky mask", &self.holes_nodes);
                pixels(&mut enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sky mask"), timestamp_writes: None }), &self.pipe_sky);
            }
        }
        if redraw {
            return;
        }
        if self.vis.audit {
            let (world_bg, storage_bg, bind_disc) = self.groups();
            core.ring.stage(enc, ticket, "audit", &[]);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("discover audit"), timestamp_writes: None });
            pass.set_pipeline(&self.pipe_audit);
            pass.set_bind_group(0, bind_disc, &[0]);
            pass.set_bind_group(1, world_bg, &[]);
            storage_bg.compute(&mut pass);
            let g = (width * height).div_ceil(256);
            pass.dispatch_workgroups(g.min(32768), g.div_ceil(32768), 1);
        }
        if let Some(i) = stage {
            let b = &self.stages[i].buf;
            enc.copy_buffer_to_buffer(&self.stats, 0, b, 0, STATS_WORDS * 4);
            enc.copy_buffer_to_buffer(&self.lcount, 0, b, STATS_WORDS * 4, 16);
            if let (Some(q), Some(res)) = (&self.profile, &self.profile_resolve) {
                let base = i as u32 * PROFILE_TS;
                enc.resolve_query_set(q, base..base + PROFILE_TS, res, i as u64 * 256);
                enc.copy_buffer_to_buffer(res, i as u64 * 256, b, STATS_WORDS * 4 + 16, PROFILE_TS as u64 * 8);
            }
            self.stages[i].tag = (self.frame, parity as u32);
        }
    }

    /// Maps the frame's counters asynchronously.
    fn submitted(&mut self, _core: &Core) {
        if let Some(i) = self.now.stage {
            let st = &self.stages[i];
            st.state.store(1, Ordering::SeqCst);
            let flag = st.state.clone();
            st.buf.map_async(wgpu::MapMode::Read, .., move |r| flag.store(if r.is_ok() { 2 } else { 3 }, Ordering::SeqCst));
        }
    }
}

/// Screen rectangle, in level-0 tiles, of the world box `[min, min + size)`: `Some(None)`
/// if it is off screen. A box straddling the camera plane that may be visible gets the
/// whole screen (its projection is unbounded); `None` is never returned for boxes behind
/// or beside the camera, only kept for callers that want a forced full frame.
fn box_rect(view_proj: &glam::Mat4, min: IVec3, size: i32, cam: glam::DVec3, width: u32, height: u32, tile: u32, cols: u32, rows: u32) -> Option<Option<[u32; 4]>> {
    let clips: [glam::Vec4; 8] = std::array::from_fn(|k| {
        let k = k as i32;
        let corner = (min + IVec3::new(k & 1, (k >> 1) & 1, k >> 2) * size).as_dvec3();
        *view_proj * (corner - cam).as_vec3().extend(1.0)
    });
    // Outside one frustum plane entirely (left, right, bottom, top, near): off screen.
    let outside = |f: &dyn Fn(&glam::Vec4) -> bool| clips.iter().all(f);
    if outside(&|c| c.x < -c.w) || outside(&|c| c.x > c.w) || outside(&|c| c.y < -c.w) || outside(&|c| c.y > c.w) || outside(&|c| c.w <= 1e-3) {
        return Some(None);
    }
    if clips.iter().any(|c| c.w <= 1e-3) {
        return Some(Some([0, 0, cols - 1, rows - 1]));
    }
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for clip in clips {
        let ndc = clip.truncate().truncate() / clip.w;
        let px = glam::Vec2::new((ndc.x * 0.5 + 0.5) * width as f32, (0.5 - ndc.y * 0.5) * height as f32);
        lo = lo.min(px);
        hi = hi.max(px);
    }
    if hi.x < 0.0 || hi.y < 0.0 || lo.x >= width as f32 || lo.y >= height as f32 {
        return Some(None);
    }
    let t = tile as f32;
    let x0 = (lo.x.max(0.0) / t) as u32;
    let y0 = (lo.y.max(0.0) / t) as u32;
    let x1 = ((hi.x / t) as u32).min(cols - 1);
    let y1 = ((hi.y / t) as u32).min(rows - 1);
    Some(Some([x0, y0, x1, y1]))
}
