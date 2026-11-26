//! Gates on edges between two passes of one GPU executor: what the producer's passes left in
//! the edge's resources (buffers or render targets, the region's: [`crate::exec::Region::crossing`],
//! or the executor's hits edge) is read back before the reader's pass, crosses the gates on
//! the host in its serialized form ([`Crossed`]: the frame's id and size, each resource's
//! bytes) and the last that arrived is written into the same resources for the reader's pass.
//! After it, what the producer left there is copied back, so state the producer keeps across
//! frames (a face set, meshes) is its own again and the passes after the reader see what they
//! always did. Before anything arrived the reader reads zeros (no hit: sky; no tile, no face).
//!
//! The read-back splits the frame's commands: what comes before the reader is submitted and
//! waited for (a gate on a GPU edge costs a round trip through the host, part of what it
//! simulates). On a viewed edge (hits, ray starts, mesh surfaces, tiles: per pixel or tile of
//! a camera) the reader's pass and the passes after it run with the camera of the frame what
//! arrived was made in (this frame's when the network zeroed its id). The time on the host is
//! the CPU stage `gate`.

use std::collections::VecDeque;
use std::time::Instant;

use mr_components::graph::Data;
use mr_core::gate::{Get, Put, Wire, WireError};
use mr_core::*;
use mr_gpu::wgpu;

use crate::{CameraUniform, Core};

/// A resource holding (part of) what crosses an edge between passes.
#[derive(Clone)]
pub(crate) enum Part {
    Buffer(wgpu::Buffer),
    /// A render target or storage texture (one level, one layer, a colour format).
    Texture(wgpu::Texture),
}

impl Part {
    /// Its bytes as they cross (a texture's rows unpadded).
    fn bytes(&self) -> u64 {
        match self {
            Part::Buffer(b) => b.size(),
            Part::Texture(t) => self.row().0 as u64 * t.height() as u64,
        }
    }

    /// A texture's row bytes, unpadded and padded to wgpu's copy alignment (a buffer: its size).
    fn row(&self) -> (u32, u32) {
        match self {
            Part::Buffer(b) => (b.size() as u32, b.size() as u32),
            Part::Texture(t) => {
                let row = t.width() * t.format().block_copy_size(None).unwrap_or(4);
                (row, row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
            }
        }
    }

    /// Its bytes copied out (a texture's rows padded).
    fn staged(&self) -> u64 {
        match self {
            Part::Buffer(b) => b.size(),
            Part::Texture(t) => self.row().1 as u64 * t.height() as u64,
        }
    }

    fn copy_out(&self, enc: &mut wgpu::CommandEncoder, to: &wgpu::Buffer) {
        match self {
            Part::Buffer(b) => enc.copy_buffer_to_buffer(b, 0, to, 0, b.size()),
            Part::Texture(t) => enc.copy_texture_to_buffer(
                t.as_image_copy(),
                wgpu::TexelCopyBufferInfo { buffer: to, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.row().1), rows_per_image: None } },
                t.size(),
            ),
        }
    }

    fn copy_in(&self, enc: &mut wgpu::CommandEncoder, from: &wgpu::Buffer) {
        match self {
            Part::Buffer(b) => enc.copy_buffer_to_buffer(from, 0, b, 0, b.size()),
            Part::Texture(t) => enc.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo { buffer: from, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.row().1), rows_per_image: None } },
                t.as_image_copy(),
                t.size(),
            ),
        }
    }

    /// `bytes` (unpadded; [`Self::bytes`] of them) written into it before the next submission.
    fn write(&self, queue: &wgpu::Queue, bytes: &[u8]) {
        match self {
            Part::Buffer(b) => queue.write_buffer(b, 0, bytes),
            Part::Texture(t) => queue.write_texture(
                t.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.row().0), rows_per_image: None },
                t.size(),
            ),
        }
    }

    fn same(&self, other: &Part) -> bool {
        match (self, other) {
            (Part::Buffer(a), Part::Buffer(b)) => a == b,
            (Part::Texture(a), Part::Texture(b)) => a == b,
            _ => false,
        }
    }
}

/// What crosses a gated edge between passes on the wire: the frame's id, width and height,
/// then each resource's bytes (a count, then each with its length).
pub(crate) struct Crossed {
    frame_id: u64,
    width: u32,
    height: u32,
    parts: Vec<Vec<u8>>,
}

impl Wire for Crossed {
    fn encode(&self, out: &mut Vec<u8>) {
        out.reserve(20 + self.parts.iter().map(|p| 4 + p.len()).sum::<usize>());
        let mut p = Put(out);
        p.u64(self.frame_id);
        p.u32(self.width);
        p.u32(self.height);
        p.u32(self.parts.len() as u32);
        for part in &self.parts {
            p.u32(part.len() as u32);
            p.bytes(part);
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut g = Get(bytes);
        let (frame_id, width, height) = (g.u64()?, g.u32()?, g.u32()?);
        let n = g.count(4)?;
        let mut parts = Vec::with_capacity(n);
        for _ in 0..n {
            let len = g.u32()? as usize;
            parts.push(g.take(len)?.to_vec());
        }
        g.end()?;
        Ok(Crossed { frame_id, width, height, parts })
    }
}

/// Whether what crosses `data` edges is seen from the frame's camera (per pixel or tile).
fn viewed(data: Data) -> bool {
    matches!(data, Data::Hits | Data::Starts | Data::Surfaces | Data::Tiles)
}

/// The gates on one edge between two passes of this executor and their state.
pub(crate) struct EdgeGate {
    pub reader: String,
    pub port: String,
    data: Data,
    /// The index of the reader's pass in the executor's frame order.
    pub before: usize,
    chain: mr_core::gate::GateChain,
    /// The resources last crossed, their read-back and the copy of what the producer left.
    parts: Vec<Part>,
    staging: Vec<wgpu::Buffer>,
    keep: Vec<wgpu::Buffer>,
    /// The last that arrived and fits the resources.
    last: Option<Crossed>,
    /// The cameras of the frames sent and not yet arrived (by frame id), the one of `last`.
    cameras: VecDeque<(u64, CameraUniform)>,
    shown: Option<CameraUniform>,
}

/// The gates of `core`'s graph on edges whose reader is one of `passes` (node ids in frame
/// order), except the hits a split's transfer carries.
pub(crate) fn gates(core: &Core, passes: &[String], split: bool) -> Vec<EdgeGate> {
    core.gates
        .iter()
        .filter(|e| !(split && e.data == Data::Hits && e.port == "hits"))
        .filter_map(|e| {
            let before = passes.iter().position(|p| *p == e.reader)?;
            Some(EdgeGate {
                reader: e.reader.clone(),
                port: e.port.clone(),
                data: e.data,
                before,
                chain: mr_core::gate::GateChain::new(&e.edge(), &e.gates),
                parts: Vec::new(),
                staging: Vec::new(),
                keep: Vec::new(),
                last: None,
                cameras: VecDeque::new(),
                shown: None,
            })
        })
        .collect()
}

impl EdgeGate {
    /// Before the reader's pass: `parts` (what the producer left) read back through the
    /// gates and the last that arrived written in their place. Submits `enc` so far and
    /// waits for it; `enc` is a new encoder after. Returns the host's time in ms.
    pub fn cross(&mut self, core: &Core, parts: Vec<Part>, enc: &mut wgpu::CommandEncoder, req: &FrameRequest) -> anyhow::Result<f32> {
        let gpu = &core.gpu;
        let fresh = parts.len() != self.parts.len() || parts.iter().zip(&self.parts).any(|(a, b)| !a.same(b) || a.staged() != b.staged());
        if fresh {
            self.staging = parts.iter().map(|p| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("gate read-back"), size: p.staged(), usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })).collect();
            self.keep = parts.iter().map(|p| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("gate keep"), size: p.staged(), usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })).collect();
            self.parts = parts;
        }
        for ((p, s), k) in self.parts.iter().zip(&self.staging).zip(&self.keep) {
            p.copy_out(enc, s);
            p.copy_out(enc, k);
        }
        let done = std::mem::replace(enc, gpu.device.create_command_encoder(&Default::default()));
        let index = gpu.queue.submit([done.finish()]);
        for s in &self.staging {
            s.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        }
        gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None })?;
        let start = Instant::now();
        let bytes: Vec<Vec<u8>> = self
            .parts
            .iter()
            .zip(&self.staging)
            .map(|(p, s)| {
                let data = s.slice(..).get_mapped_range()?;
                let (row, padded) = p.row();
                let out = match p {
                    Part::Buffer(_) => data.to_vec(),
                    Part::Texture(_) => data.chunks(padded as usize).flat_map(|r| &r[..row as usize]).copied().collect(),
                };
                drop(data);
                s.unmap();
                Ok(out)
            })
            .collect::<Result<_, wgpu::MapRangeError>>()
            .map_err(|e| anyhow::anyhow!("reading back what crosses `{}`.{}: {e}", self.reader, self.port))?;
        let (w, h) = (core.width, core.height);
        self.cameras.push_back((req.frame_id, core.camera.get()));
        let got = self.chain.gates.pass(req.time, Some(Crossed { frame_id: req.frame_id, width: w, height: h, parts: bytes }));
        // The last that fits the resources as they are (one sent before a resize does not).
        let sizes: Vec<u64> = self.parts.iter().map(Part::bytes).collect();
        if let Some(c) = got.into_iter().rev().find(|c| (c.width, c.height) == (w, h) && c.parts.iter().map(|p| p.len() as u64).eq(sizes.iter().copied())) {
            while self.cameras.front().is_some_and(|(id, _)| *id < c.frame_id) {
                self.cameras.pop_front();
            }
            self.shown = Some(self.cameras.front().filter(|(id, _)| *id == c.frame_id).map_or(core.camera.get(), |(_, cam)| *cam));
            self.last = Some(c);
        }
        // Cameras of messages the network has lost for good are dropped with the oldest.
        while self.cameras.len() > 64 {
            self.cameras.pop_front();
        }
        match &self.last {
            Some(c) => self.parts.iter().zip(&c.parts).for_each(|(p, b)| p.write(&gpu.queue, b)),
            None => self.parts.iter().zip(&sizes).for_each(|(p, &n)| p.write(&gpu.queue, &vec![0; n as usize])),
        }
        if let Some(cam) = self.shown.filter(|_| self.last.is_some() && viewed(self.data)) {
            core.rewrite_camera(cam);
        }
        Ok(start.elapsed().as_secs_f32() * 1e3)
    }

    /// After the reader's pass: what the producer left copied back.
    pub fn restore(&self, enc: &mut wgpu::CommandEncoder) {
        for (p, k) in self.parts.iter().zip(&self.keep) {
            p.copy_in(enc, k);
        }
    }

    /// After a resize: nothing of the old size fits.
    pub fn resize(&mut self) {
        self.last = None;
        self.shown = None;
    }

    pub fn nodes(&self) -> Vec<String> {
        self.chain.nodes.iter().map(|(id, _)| id.clone()).collect()
    }

    pub fn reports(&self) -> Vec<mr_core::gate::GateReport> {
        self.chain.reports()
    }

    pub fn memory_bytes(&self) -> u64 {
        self.staging.iter().chain(&self.keep).map(|b| b.size()).sum()
    }
}
