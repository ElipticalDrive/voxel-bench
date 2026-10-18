//! Host <-> worker protocol.
//!
//! Control messages travel over a Unix stream socket as length-prefixed postcard blobs.
//! Frame pixels travel through a shared-memory file of fixed-size slots, so a 4K frame
//! never goes through the socket.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use glam::DVec3;
use memmap2::MmapMut;
use mr_core::{FrameRequest, FrameStats, InitParams, MemoryReport, MeshUpdate, ParamValue, RendererInfo, WorldUpdate};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const PROTOCOL_VERSION: u32 = 6;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShmSpec {
    pub path: PathBuf,
    pub slots: u32,
    pub slot_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HostMsg {
    Init { renderer: String, params: InitParams, shm: ShmSpec },
    /// The server's stream (PLAN step 14b: the same for every client), in batches; the
    /// worker's client derives its renderer's updates from it, at camera position `at`
    /// (set on a batch that ends what the server sent for a frame). `time`: when the server
    /// sent it, in seconds of the host's clock (simulated in scripted runs), which the
    /// client's link delays it by (PLAN step 14c).
    Stream { updates: Vec<WorldUpdate>, at: Option<DVec3>, time: f64 },
    /// The server streams this region (a radius per LOD level) and neither it nor the
    /// client's changes: a client covering it takes the stream as it is.
    Follow { radii: Vec<u32> },
    /// Mesh mode: geometry and instances, applied in order with `Updates`.
    Meshes(MeshUpdate),
    /// `slot` is the shared-memory slot to write pixels into when `req.readback` is set.
    /// `time`: the frame's time on the host's clock (what has arrived over the link by then
    /// reaches the renderer first).
    Render { req: FrameRequest, slot: Option<u32>, time: f64 },
    Resize { width: u32, height: u32, shm: ShmSpec },
    /// Live parameter changes, applied in order before the next frame.
    SetParams(Vec<(String, ParamValue)>),
    Flush,
    QueryMemory,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerMsg {
    Hello { pid: u32, protocol: u32, renderers: Vec<RendererInfo> },
    Ready { info: RendererInfo, init_ms: f32 },
    /// `update_ms` is time spent applying world updates since the previous frame.
    FrameDone { frame_id: u64, stats: FrameStats, slot: Option<u32>, update_ms: f32 },
    Flushed,
    Memory(MemoryReport),
    /// A parameter change was rejected (non-fatal).
    ParamError { name: String, error: String },
    Error(String),
}

pub struct MsgWriter<T> {
    w: BufWriter<UnixStream>,
    buf: Vec<u8>,
    _t: PhantomData<T>,
}

impl<T: Serialize> MsgWriter<T> {
    pub fn send(&mut self, msg: &T) -> anyhow::Result<()> {
        self.buf.clear();
        self.buf = postcard::to_extend(msg, std::mem::take(&mut self.buf))?;
        let len = u32::try_from(self.buf.len()).context("message over 4 GiB")?;
        self.w.write_all(&len.to_le_bytes())?;
        self.w.write_all(&self.buf)?;
        self.w.flush()?;
        Ok(())
    }
}

pub struct MsgReader<T> {
    r: BufReader<UnixStream>,
    buf: Vec<u8>,
    _t: PhantomData<T>,
}

impl<T: DeserializeOwned> MsgReader<T> {
    /// Blocks for the next message; `Ok(None)` on clean EOF.
    pub fn recv(&mut self) -> anyhow::Result<Option<T>> {
        let mut len = [0u8; 4];
        match self.r.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        let len = u32::from_le_bytes(len) as usize;
        self.buf.resize(len, 0);
        self.r.read_exact(&mut self.buf)?;
        Ok(Some(postcard::from_bytes(&self.buf)?))
    }
}

/// Splits a connected socket into a typed reader and writer.
pub fn split<In: DeserializeOwned, Out: Serialize>(s: UnixStream) -> anyhow::Result<(MsgReader<In>, MsgWriter<Out>)> {
    let r = s.try_clone()?;
    Ok((
        MsgReader { r: BufReader::with_capacity(1 << 20, r), buf: vec![], _t: PhantomData },
        MsgWriter { w: BufWriter::with_capacity(1 << 20, s), buf: vec![], _t: PhantomData },
    ))
}

/// Fixed-size frame slots in a shared-memory file.
pub struct FrameSlots {
    map: MmapMut,
    spec: ShmSpec,
    owner: bool,
}

impl FrameSlots {
    /// Creates (host side) a new shared-memory file under `/dev/shm`.
    pub fn create(name: &str, slots: u32, slot_bytes: u64) -> anyhow::Result<Self> {
        let path = PathBuf::from(format!("/dev/shm/{name}"));
        let f = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path)?;
        f.set_len(slots as u64 * slot_bytes)?;
        let map = unsafe { MmapMut::map_mut(&f)? };
        Ok(Self { map, spec: ShmSpec { path, slots, slot_bytes }, owner: true })
    }

    /// Opens (worker side) an existing shared-memory file.
    pub fn open(spec: &ShmSpec) -> anyhow::Result<Self> {
        let f: File = OpenOptions::new().read(true).write(true).open(&spec.path)?;
        if f.metadata()?.len() < spec.slots as u64 * spec.slot_bytes {
            bail!("shared memory file {} too small", spec.path.display());
        }
        let map = unsafe { MmapMut::map_mut(&f)? };
        Ok(Self { map, spec: spec.clone(), owner: false })
    }

    pub fn spec(&self) -> &ShmSpec {
        &self.spec
    }

    pub fn slot(&self, i: u32) -> &[u8] {
        let n = self.spec.slot_bytes as usize;
        &self.map[i as usize * n..(i as usize + 1) * n]
    }

    pub fn slot_mut(&mut self, i: u32) -> &mut [u8] {
        let n = self.spec.slot_bytes as usize;
        &mut self.map[i as usize * n..(i as usize + 1) * n]
    }

    pub fn path(&self) -> &Path {
        &self.spec.path
    }
}

impl Drop for FrameSlots {
    fn drop(&mut self) {
        if self.owner {
            let _ = std::fs::remove_file(&self.spec.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frame stats cross the worker protocol intact, stage times included (postcard is not
    /// self-describing: a skipped field breaks every message after it).
    #[test]
    fn frame_stats_round_trip() {
        let stats = mr_core::FrameStats {
            cpu_ms: 1.5,
            gpu_ms: Some(0.5),
            counters: vec![("rays".into(), 3.0)],
            legend: None,
            stages: vec![mr_core::StageTime::cpu("geometry", &["geometry".into()], 2.0), mr_core::StageTime { name: "draw".into(), nodes: vec!["draw".into()], cpu_ms: None, gpu_ms: Some(0.25) }],
        };
        let msg = WorkerMsg::FrameDone { frame_id: 7, stats: stats.clone(), slot: Some(1), update_ms: 0.125 };
        let back: WorkerMsg = postcard::from_bytes(&postcard::to_allocvec(&msg).unwrap()).unwrap();
        let WorkerMsg::FrameDone { frame_id: 7, stats: s, slot: Some(1), update_ms } = back else { panic!("{back:?}") };
        assert_eq!((s.stages, s.counters, s.gpu_ms, update_ms), (stats.stages, stats.counters, stats.gpu_ms, 0.125));
    }
}
