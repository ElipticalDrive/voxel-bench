//! The hit records' edge between two GPUs' executors: the transfer adapter the schedule names
//! for `visibility` -> `shading`.hits when its ends run on different GPU slots. Each executor
//! holds the edge's resource on its GPU ([`crate::exec::Edges`]); the transfer carries one to
//! the other through the host (read back from the first, written to the second: no peer copy
//! in wgpu). Neither executor's nodes know of it: the first's rays write their records as on
//! one GPU, the second's shading reads the records that arrived.
//!
//! Edge `now` delivers the records just sent (the GPUs take turns); edge `prev` (history) the
//! last frame's, so the second GPU shades frame N - 1 while the first casts N: frame N's
//! image shows the view of frame N - 1 (the first frame's, nothing arrived yet: sky), and the
//! last records cast are never shaded. Its costs are a GPU stage of the first GPU (`copy`:
//! the records into the readback buffer), a CPU stage (`transfer`: the upload) and the wait
//! for the first GPU's frame (the counter `transfer_wait_ms`).
//!
//! The first GPU's edge holds the records in a buffer (rays) or a render target of 8-byte
//! texels (the `gbuffer` drawer's): a target is copied out with its rows padded to 256 bytes
//! (wgpu's copy alignment) and written into the second GPU's records buffer unpadded.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use mr_components::graph::{self, When};
use mr_core::*;
use mr_gpu::{Ticket, wgpu};

use crate::Core;
use crate::exec::Hits;

/// Frames' stats captured from the first GPU's ring (its frames are no frames of their own).
#[derive(Default)]
struct Captured(VecDeque<(u64, FrameStats)>);

impl FrameSink for Captured {
    fn complete(&mut self, frame: CompletedFrame<'_>) {
        self.0.push_back((frame.frame_id, frame.stats));
    }
}

impl Captured {
    /// Frame `id`'s stats (older ones are dropped: their frames were not shaded).
    fn take(&mut self, id: u64) -> Option<FrameStats> {
        while let Some((i, _)) = self.0.front() {
            if *i > id {
                return None;
            }
            let (i, stats) = self.0.pop_front().unwrap();
            if i == id {
                return Some(stats);
            }
        }
        None
    }
}

/// A frame's records on their way: copied into `staging[buffer]` on the first GPU by
/// submission `index`, mapped once `mapped`; cast with `camera`.
struct Sent {
    frame_id: u64,
    camera: Camera,
    buffer: usize,
    index: wgpu::SubmissionIndex,
    mapped: Arc<AtomicBool>,
}

/// The hits edge from the first GPU (`near`) to the second (`far`).
pub(crate) struct Transfer {
    /// On the first GPU: the records read back (two for edge `prev`: one being written while
    /// the other's are shaded).
    staging: Vec<wgpu::Buffer>,
    /// The records sent and not yet delivered, oldest first.
    sent: VecDeque<Sent>,
    /// Edge `prev`: the reader gets the last frame's records.
    late: bool,
    near: Captured,
    /// The edge's ends (the nodes of the transfer's stages); its last CPU times: waiting for
    /// the first GPU's frame (its passes and the records' copy) and the upload.
    nodes: Vec<String>,
    wait_ms: f32,
    upload_ms: f32,
}

/// The bytes of a row of `width` records as the first GPU's edge is copied out: a render
/// target's padded to wgpu's copy alignment, `None` for a buffer (copied whole).
fn padded_row(near: &Core) -> Option<u32> {
    let edge = near.edges.hits()?;
    edge.texture().map(|_| (near.width * 8).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
}

fn staging(near: &Core, late: bool) -> Vec<wgpu::Buffer> {
    let size = padded_row(near).map_or(Hits::size(near.width, near.height), |row| row as u64 * near.height as u64);
    let one = || {
        near.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hit records readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    (0..if late { 2 } else { 1 }).map(|_| one()).collect()
}

impl Transfer {
    /// The transfer of `g`'s hits edge between the executors' cores, which both hold it.
    pub fn of(g: &graph::Checked, near: &Core, far: &Core) -> anyhow::Result<Self> {
        let (Some(edge), Some(_)) = (near.edges.hits().filter(|h| h.holds_records()), far.edges.records()) else { anyhow::bail!("no hit records cross between the two GPUs (placement = deferred, or draw = gbuffer or visbuffer)") };
        let late = g.graph.nodes.get(&edge.to).is_some_and(|n| n.when("hits") == When::Prev);
        tracing::info!("`{}` -> `{}`.hits: {} -> {} ({})", edge.from, edge.to, near.gpu.info.name, far.gpu.info.name, if late { "prev" } else { "now" });
        Ok(Self { staging: staging(near, late), sent: VecDeque::new(), late, near: Captured::default(), nodes: vec![edge.from.clone(), edge.to.clone()], wait_ms: 0.0, upload_ms: 0.0 })
    }

    /// The first GPU's frame sink: its frames' stats, kept for the frame the second completes.
    pub fn sink(&mut self) -> &mut dyn FrameSink {
        &mut self.near
    }

    /// The first half: submits the first GPU's frame `enc` with its records copied out (with
    /// the frame's stats from the first GPU) and starts reading them back.
    pub fn send(&mut self, near: &mut Core, mut enc: wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, stats: FrameStats) {
        let edge = near.edges.hits().expect("checked at creation");
        // `prev`: the other buffer than the records sent last frame, which are delivered next.
        let buffer = self.sent.back().map_or(0, |s| (s.buffer + 1) % self.staging.len());
        near.ring.stage(&mut enc, ticket, "copy", &self.nodes);
        match (edge.texture(), padded_row(near)) {
            (Some(texture), Some(row)) => enc.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo { buffer: &self.staging[buffer], layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None } },
                texture.size(),
            ),
            _ => {
                let records = edge.buffer();
                enc.copy_buffer_to_buffer(records, 0, &self.staging[buffer], 0, records.size());
            }
        }
        let index = near.ring.submit(&near.gpu, enc, ticket, &near.output, req.frame_id, false, stats);
        let mapped = Arc::new(AtomicBool::new(false));
        let done = mapped.clone();
        let failure = near.gpu.failure();
        self.staging[buffer].slice(..).map_async(wgpu::MapMode::Read, move |r| match r {
            Ok(()) => done.store(true, Ordering::Release),
            Err(e) => failure.set(format!("hit records read-back failed: {e}")),
        });
        self.sent.push_back(Sent { frame_id: req.frame_id, camera: req.camera, buffer, index, mapped });
    }

    /// The second half: the records the edge's timing names (`now`: the frame just sent;
    /// `prev`: the one before) waited for and written to the second GPU. Returns the first
    /// GPU's stats of their frame and the camera they were cast with (the first frame of
    /// `prev`: nothing arrived yet, the second GPU's records are cleared, `req`'s camera).
    pub fn arrive(&mut self, near: &mut Core, far: &Core, req: &FrameRequest) -> anyhow::Result<(FrameStats, Camera)> {
        let Some(s) = (self.sent.len() > usize::from(self.late)).then(|| self.sent.pop_front()).flatten() else {
            (self.wait_ms, self.upload_ms) = (0.0, 0.0);
            return Ok((FrameStats::default(), req.camera));
        };
        let start = Instant::now();
        let mut stats = None;
        loop {
            near.ring.poll(&near.gpu, &mut self.near, false)?;
            stats = stats.or_else(|| self.near.take(s.frame_id));
            if s.mapped.load(Ordering::Acquire) && stats.is_some() {
                break;
            }
            near.gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(s.index.clone()), timeout: None })?;
        }
        let arrived = Instant::now();
        let staging = &self.staging[s.buffer];
        let records = far.edges.records().expect("checked at creation").buffer();
        let data = staging.slice(..).get_mapped_range()?;
        match padded_row(near) {
            // A target's rows, unpadded into the records.
            Some(row) => {
                let width = near.width as usize * 8;
                let size = wgpu::BufferSize::new((width * near.height as usize) as u64).expect("a frame has pixels");
                let mut view = far.gpu.queue.write_buffer_with(records, 0, size).ok_or_else(|| anyhow::anyhow!("writing the hit records on {}", far.gpu.info.name))?;
                for (y, from) in data.chunks(row as usize).enumerate() {
                    view.slice(y * width..(y + 1) * width).copy_from_slice(&from[..width]);
                }
            }
            None => far.gpu.queue.write_buffer(records, 0, &data),
        }
        drop(data);
        staging.unmap();
        self.wait_ms = (arrived - start).as_secs_f32() * 1e3;
        self.upload_ms = arrived.elapsed().as_secs_f32() * 1e3;
        Ok((stats.unwrap_or_default(), s.camera))
    }

    /// The last delivery's CPU stage (the upload), its wait for the first GPU and the CPU
    /// time of both, in ms.
    pub fn times(&self) -> (StageTime, f32, f32) {
        (StageTime::cpu("transfer", &self.nodes, self.upload_ms), self.wait_ms, self.wait_ms + self.upload_ms)
    }

    /// After the first GPU's resize: the records sent and not delivered are waited for and
    /// dropped.
    pub fn resize(&mut self, near: &Core) -> anyhow::Result<()> {
        for s in std::mem::take(&mut self.sent) {
            while !s.mapped.load(Ordering::Acquire) {
                near.gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(s.index.clone()), timeout: None })?;
            }
            self.staging[s.buffer].unmap();
        }
        self.staging = staging(near, self.late);
        Ok(())
    }

    pub fn memory_bytes(&self) -> u64 {
        self.staging.iter().map(|b| b.size()).sum()
    }
}
