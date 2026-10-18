//! Host <-> worker protocol.
//!
//! Control messages travel over a local socket (a Unix socket on Linux, a named pipe on
//! Windows) as length-prefixed postcard blobs. Frame pixels travel through shared memory of
//! fixed-size slots (a file under `/dev/shm` on Linux, a pagefile-backed named mapping on
//! Windows), so a 4K frame never goes through the socket.

use std::io::{BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;

use anyhow::Context;
use glam::DVec3;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{ListenerNonblockingMode, ListenerOptions, Name, RecvHalf, SendHalf};
pub use interprocess::local_socket::{Listener, Stream};
use mr_core::{FrameRequest, FrameStats, InitParams, MemoryReport, MeshUpdate, ParamValue, RendererInfo, WorldUpdate};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const PROTOCOL_VERSION: u32 = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShmSpec {
    /// The region's name (`FrameSlots::create`'s), the same on every system.
    pub name: String,
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
    /// The client generates the world itself (PLAN step 14d: the graph's generator on the
    /// client): `world` is the server's generator spec (`mr_world::GenSpec` as JSON, the
    /// shared seed), `budget` the chunks it generates per frame (`None`: all at once). The
    /// server then sends it edits, lights and the environment only.
    Generate { world: String, budget: Option<u32> },
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
    /// `update_ms` is time spent applying world updates since the previous frame;
    /// `arrivals` what had reached its client when the frame was requested (PLAN step 14e).
    FrameDone { frame_id: u64, stats: FrameStats, slot: Option<u32>, update_ms: f32, arrivals: mr_core::link::Arrivals },
    Flushed,
    Memory(MemoryReport),
    /// A parameter change was rejected (non-fatal).
    ParamError { name: String, error: String },
    Error(String),
}

pub struct MsgWriter<T> {
    w: BufWriter<SendHalf>,
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
    r: BufReader<RecvHalf>,
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

/// The address a host listens on for one worker, from a name unique to it: a socket file in
/// the runtime directory on Unix, a named pipe on Windows.
pub fn endpoint(tag: &str) -> String {
    if cfg!(unix) {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        dir.join(format!("{tag}.sock")).to_string_lossy().into_owned()
    } else {
        tag.to_string()
    }
}

fn name(endpoint: &str) -> std::io::Result<Name<'_>> {
    if cfg!(unix) {
        endpoint.to_fs_name::<interprocess::local_socket::GenericFilePath>()
    } else {
        endpoint.to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    }
}

/// Listens at `endpoint` (host side); `accept` does not block, the streams it returns do.
pub fn listen(endpoint: &str) -> anyhow::Result<Listener> {
    ListenerOptions::new()
        .name(name(endpoint)?)
        .try_overwrite(true)
        .nonblocking(ListenerNonblockingMode::Accept)
        .create_sync()
        .with_context(|| format!("listening at {endpoint}"))
}

/// The next worker connecting to `l`, if one is waiting.
pub fn try_accept(l: &Listener) -> anyhow::Result<Option<Stream>> {
    match l.accept() {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Connects to the host listening at `endpoint` (worker side).
pub fn connect(endpoint: &str) -> anyhow::Result<Stream> {
    Stream::connect(name(endpoint)?).with_context(|| format!("connecting to {endpoint}"))
}

/// Splits a connected socket into a typed reader and writer.
pub fn split<In: DeserializeOwned, Out: Serialize>(s: Stream) -> (MsgReader<In>, MsgWriter<Out>) {
    let (r, w) = s.split();
    (
        MsgReader { r: BufReader::with_capacity(1 << 20, r), buf: vec![], _t: PhantomData },
        MsgWriter { w: BufWriter::with_capacity(1 << 20, w), buf: vec![], _t: PhantomData },
    )
}

/// Fixed-size frame slots in shared memory.
pub struct FrameSlots {
    map: shm::Map,
    spec: ShmSpec,
}

impl FrameSlots {
    /// Creates (host side) a new region called `name`.
    pub fn create(name: &str, slots: u32, slot_bytes: u64) -> anyhow::Result<Self> {
        let spec = ShmSpec { name: name.to_string(), slots, slot_bytes };
        Ok(Self { map: shm::Map::create(name, spec.bytes()).with_context(|| format!("creating shared memory {name}"))?, spec })
    }

    /// Opens (worker side) an existing region.
    pub fn open(spec: &ShmSpec) -> anyhow::Result<Self> {
        let map = shm::Map::open(&spec.name, spec.bytes()).with_context(|| format!("opening shared memory {}", spec.name))?;
        Ok(Self { map, spec: spec.clone() })
    }

    pub fn spec(&self) -> &ShmSpec {
        &self.spec
    }

    pub fn slot(&self, i: u32) -> &[u8] {
        let n = self.spec.slot_bytes as usize;
        &self.map.bytes()[i as usize * n..(i as usize + 1) * n]
    }

    pub fn slot_mut(&mut self, i: u32) -> &mut [u8] {
        let n = self.spec.slot_bytes as usize;
        &mut self.map.bytes_mut()[i as usize * n..(i as usize + 1) * n]
    }

    /// Host side, once the worker has the region open: it then lives only as long as the
    /// mappings, so nothing leaks if the host is killed (Linux removes the file; a Windows
    /// mapping already goes with its last handle).
    pub fn unlink(&mut self) {
        self.map.unlink();
    }
}

impl ShmSpec {
    fn bytes(&self) -> usize {
        (self.slots as u64 * self.slot_bytes) as usize
    }
}

#[cfg(unix)]
mod shm {
    use std::fs::OpenOptions;
    use std::path::PathBuf;

    use anyhow::bail;
    use memmap2::MmapMut;

    /// A file under `/dev/shm`, mapped; the creator removes it (`unlink`, or when dropped).
    pub struct Map {
        map: MmapMut,
        owned: Option<PathBuf>,
    }

    fn path(name: &str) -> PathBuf {
        PathBuf::from(format!("/dev/shm/{name}"))
    }

    impl Map {
        pub fn create(name: &str, bytes: usize) -> anyhow::Result<Self> {
            let path = path(name);
            let f = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path)?;
            f.set_len(bytes as u64)?;
            Ok(Self { map: unsafe { MmapMut::map_mut(&f)? }, owned: Some(path) })
        }

        pub fn open(name: &str, bytes: usize) -> anyhow::Result<Self> {
            let path = path(name);
            let f = OpenOptions::new().read(true).write(true).open(&path)?;
            if f.metadata()?.len() < bytes as u64 {
                bail!("{} too small", path.display());
            }
            Ok(Self { map: unsafe { MmapMut::map_mut(&f)? }, owned: None })
        }

        pub fn bytes(&self) -> &[u8] {
            &self.map
        }

        pub fn bytes_mut(&mut self) -> &mut [u8] {
            &mut self.map
        }

        pub fn unlink(&mut self) {
            if let Some(p) = self.owned.take() {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    impl Drop for Map {
        fn drop(&mut self) {
            self.unlink();
        }
    }
}

#[cfg(windows)]
mod shm {
    use anyhow::bail;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW, PAGE_READWRITE,
        UnmapViewOfFile, VirtualQuery,
    };

    /// A pagefile-backed named mapping (session-local), freed with its last handle.
    pub struct Map {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        len: usize,
    }

    // The view is plain memory shared with another process; the handle is process-wide.
    unsafe impl Send for Map {}
    unsafe impl Sync for Map {}

    fn wide(name: &str) -> Vec<u16> {
        format!("Local\\{name}").encode_utf16().chain([0]).collect()
    }

    impl Map {
        pub fn create(name: &str, bytes: usize) -> anyhow::Result<Self> {
            let size = bytes as u64;
            let handle = unsafe { CreateFileMappingW(INVALID_HANDLE_VALUE, std::ptr::null(), PAGE_READWRITE, (size >> 32) as u32, size as u32, wide(name).as_ptr()) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            Self::view(handle, bytes)
        }

        pub fn open(name: &str, bytes: usize) -> anyhow::Result<Self> {
            let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide(name).as_ptr()) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let map = Self::view(handle, 0)?;
            if map.len < bytes {
                bail!("{name} too small");
            }
            Ok(Self { len: bytes, ..map })
        }

        /// Maps all of `handle`; `bytes` 0 takes the mapping's size (rounded up to pages).
        fn view(handle: HANDLE, bytes: usize) -> anyhow::Result<Self> {
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, 0) };
            if view.Value.is_null() {
                let e = std::io::Error::last_os_error();
                unsafe { CloseHandle(handle) };
                return Err(e.into());
            }
            let mut len = bytes;
            if len == 0 {
                let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
                unsafe { VirtualQuery(view.Value, &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) };
                len = info.RegionSize;
            }
            Ok(Self { handle, view, len })
        }

        pub fn bytes(&self) -> &[u8] {
            unsafe { std::slice::from_raw_parts(self.view.Value.cast(), self.len) }
        }

        pub fn bytes_mut(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.view.Value.cast(), self.len) }
        }

        pub fn unlink(&mut self) {}
    }

    impl Drop for Map {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(self.view);
                CloseHandle(self.handle);
            }
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
        let arrivals = mr_core::link::Arrivals { bytes: 1 << 40, arrived: 1 << 39, holes: 3 };
        let msg = WorkerMsg::FrameDone { frame_id: 7, stats: stats.clone(), slot: Some(1), update_ms: 0.125, arrivals };
        let back: WorkerMsg = postcard::from_bytes(&postcard::to_allocvec(&msg).unwrap()).unwrap();
        let WorkerMsg::FrameDone { frame_id: 7, stats: s, slot: Some(1), update_ms, arrivals: a } = back else { panic!("{back:?}") };
        assert_eq!((s.stages, s.counters, s.gpu_ms, update_ms, a), (stats.stages, stats.counters, stats.gpu_ms, 0.125, arrivals));
    }
}
