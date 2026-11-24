//! Diagnostics: the counters read back, the `view` legends and overlays.

use std::sync::atomic::Ordering;

use mr_core::*;
use mr_gpu::wgpu;

use super::buffers::OverlayUniform;
use super::knobs::view_code;
use super::*;
use crate::Core;

impl GpuSampler {
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
                if newest.as_ref().is_none_or(|(f, _)| frame.wrapping_sub(*f) as i32 > 0) {
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

    /// What the active `view`'s colours mean.
    pub(super) fn legend(&self) -> Option<Legend> {
        Some(match self.vis.view.as_str() {
            "overdraw" => Legend::ramp("fragments shaded per pixel", self.vis.view_max.clamp(1, 255) as f64, false),
            "tiles" => Legend::categories(&[
                ("full frame", Some([255, 255, 255])),
                ("edited block / changed chunk", Some([64, 102, 255])),
                ("entering strip", Some([26, 230, 255])),
                ("uncovered or hole", Some([255, 51, 38])),
                ("parallax", Some([255, 217, 26])),
                ("refresh", Some([51, 230, 77])),
                ("skipped (darkened)", Some([0, 0, 0])),
            ]),
            "rays" => Legend::categories(&[("ray cast this frame", Some([255, 255, 51])), ("no ray (darkened)", Some([0, 0, 0]))]),
            "churn" => Legend::categories(&[("face new to the set this frame", Some([255, 38, 26])), ("carried over (grey)", Some([128, 128, 128]))]),
            "quads" => Legend::categories(&[("one random colour per drawn quad", None)]),
            _ => return None,
        })
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
                out.push((format!("frames_{name}_pct"), self.kind_counts[k] as f64 / total * 100.0));
            }
        }
        let Some(w) = &self.last_stats else { return out };
        if self.last_kind == FrameKind::Static {
            // Nothing was discovered: report the redraw, not the last discovery's work.
            let faces = w[STATS_WORDS as usize + self.last_parity] as f64;
            out.extend([("rays".into(), 0.0), ("rays_per_px".into(), 0.0), ("faces_s".into(), faces), ("faces_new".into(), 0.0), ("faces_dropped".into(), 0.0), ("tiles_needed_pct".into(), 0.0)]);
            return out;
        }
        let levels = self.levels();
        let rays: u64 = (0..levels).map(|l| w[8 + l] as u64).sum();
        let px = (core.width * core.height) as f64;
        let faces = w[STATS_WORDS as usize + self.last_parity];
        if self.profile.is_some() {
            let base = STATS_WORDS as usize + 4;
            let ts: Vec<u64> = (0..PROFILE_TS as usize).map(|k| w[base + 2 * k] as u64 | (w[base + 2 * k + 1] as u64) << 32).collect();
            let ms = |a: usize, b: usize| if ts[b] > ts[a] { (ts[b] - ts[a]) as f64 * self.ts_period as f64 / 1e6 } else { 0.0 };
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
            out.push((format!("acc_L{l}_{}px", self.vis.tile >> l), w[16 + l] as f64));
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
            let tiles = (core.width.div_ceil(self.vis.tile) * core.height.div_ceil(self.vis.tile)) as f64;
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

    /// `view = tiles | rays`: blends the diagnostic overlay over the output (`overdraw`:
    /// replaces the counts with their colours). `frame` is the
    /// discovery frame to show (0: nothing was discovered this frame).
    pub(super) fn encode_overlay(&mut self, core: &Core, enc: &mut wgpu::CommandEncoder, ticket: mr_gpu::Ticket, frame: u32, off_x: u32, off_y: u32, cols: u32, rows: u32) {
        let view = view_code(&self.vis.view);
        if view < 2 {
            return;
        }
        // A diagnostic, not a node's work.
        core.ring.stage(enc, ticket, "view", &[]);
        let u = OverlayUniform { view, frame, tile: self.vis.tile, cols, rows, need_off: self.sized.need_off, lat_w: self.sized.lat_w, off_x, off_y, view_max: self.vis.view_max.clamp(1, 255), _p: [0; 2] };
        if view == 4 {
            // The overlay ramps the counts, so it reads them from a copy of the output.
            let size = wgpu::Extent3d { width: core.width, height: core.height, depth_or_array_layers: 1 };
            enc.copy_texture_to_texture(core.output.as_image_copy(), self.sized.counts.as_image_copy(), size);
        }
        core.gpu.queue.write_buffer(&self.overlay_buf, 0, bytemuck::bytes_of(&u));
        if self.bind_ov.is_none() {
            self.bind_ov = Some(core.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("discover overlay"),
                layout: &self.layout_ov,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.overlay_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.sized.need.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.sized.cast_frame.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.sized.counts.create_view(&Default::default())) },
                ],
            }));
        }
        let out_view = core.output.create_view(&Default::default());
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("discover overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &out_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipe_overlay);
        pass.set_bind_group(0, self.bind_ov.as_ref(), &[]);
        pass.draw(0..3, 0..1);
    }
}
