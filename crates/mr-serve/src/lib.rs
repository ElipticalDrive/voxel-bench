//! The worker side of the IPC protocol: hosts exactly one renderer, driven by the host.
//! The full worker (`mr-worker`, every renderer) and generated pipeline workers (PLAN step
//! 8: one composition compiled for this machine) both serve through [`run`].

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, bail};
use mr_core::{CompletedFrame, FrameSink, Renderer, RendererEntry, Waker};
use mr_ipc::{FrameSlots, HostMsg, MsgWriter, PROTOCOL_VERSION, WorkerMsg};

/// The `worker --socket <path>` arguments a worker process is started with.
pub fn socket_arg(mut args: impl Iterator<Item = String>) -> anyhow::Result<PathBuf> {
    while let Some(a) = args.next() {
        if a == "--socket" {
            return args.next().map(PathBuf::from).context("--socket needs a path");
        }
    }
    bail!("usage: <worker> worker --socket <path>")
}

/// Devices for a process without a GPU API (CPU-only pipelines).
pub fn cpu_devices(_: &mr_core::DeviceConfig) -> mr_core::Devices {
    mr_core::Devices { gpus: None }
}

struct Sink<'a> {
    out: &'a mut MsgWriter<WorkerMsg>,
    shm: &'a mut FrameSlots,
    slots: &'a mut HashMap<u64, Option<u32>>,
    update_ms: &'a mut f32,
    error: Option<anyhow::Error>,
}

impl FrameSink for Sink<'_> {
    fn complete(&mut self, frame: CompletedFrame<'_>) {
        let slot = self.slots.remove(&frame.frame_id).flatten();
        if let (Some(s), Some(px)) = (slot, frame.pixels) {
            let dst = self.shm.slot_mut(s);
            if px.len() > dst.len() {
                self.error.get_or_insert(anyhow::anyhow!("frame of {} bytes exceeds slot", px.len()));
                return;
            }
            dst[..px.len()].copy_from_slice(px);
        }
        let msg = WorkerMsg::FrameDone {
            frame_id: frame.frame_id,
            stats: frame.stats,
            slot: slot.filter(|_| frame.pixels.is_some()),
            update_ms: std::mem::take(self.update_ms),
        };
        if let Err(e) = self.out.send(&msg) {
            self.error.get_or_insert(e);
        }
    }
}

/// Connects to the host at `socket` and serves one of `entries` until shutdown. `devices`
/// opens the process's devices for the configuration the host sends.
pub fn run(socket: &Path, entries: Vec<RendererEntry>, devices: fn(&mr_core::DeviceConfig) -> mr_core::Devices) -> anyhow::Result<()> {
    let stream = UnixStream::connect(socket).with_context(|| format!("connecting to {}", socket.display()))?;
    let (mut input, mut out) = mr_ipc::split::<HostMsg, WorkerMsg>(stream)?;
    out.send(&WorkerMsg::Hello { pid: std::process::id(), protocol: PROTOCOL_VERSION, renderers: entries.iter().map(|e| (e.info)()).collect() })?;

    // Drain the socket on a separate thread so the host never blocks on a busy renderer.
    // The renderer's waker feeds the same queue, so completions are handled promptly.
    let (tx, rx) = crossbeam_channel::unbounded::<Input>();
    let wake_tx = tx.clone();
    let waker = Waker::new(move || {
        let _ = wake_tx.send(Input::Wake);
    });
    std::thread::Builder::new().name("ipc-reader".into()).spawn(move || {
        loop {
            match input.recv() {
                Ok(Some(m)) => {
                    if tx.send(Input::Host(m)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::error!("ipc read failed: {e:#}");
                    break;
                }
            }
        }
    })?;

    let result = serve(&rx, &mut out, waker, entries, devices);
    if let Err(e) = &result {
        let _ = out.send(&WorkerMsg::Error(format!("{e:#}")));
    }
    result
}

enum Input {
    Host(HostMsg),
    Wake,
}

fn serve(rx: &crossbeam_channel::Receiver<Input>, out: &mut MsgWriter<WorkerMsg>, waker: Waker, entries: Vec<RendererEntry>, devices: fn(&mr_core::DeviceConfig) -> mr_core::Devices) -> anyhow::Result<()> {
    let Ok(Input::Host(HostMsg::Init { renderer: name, params, shm })) = rx.recv() else {
        bail!("expected Init as first message");
    };
    let entry = entries.into_iter().find(|e| (e.info)().name == name).with_context(|| format!("unknown renderer `{name}`"))?;
    let info = (entry.info)();
    tracing::info!("initialising {name} at {}x{}", params.width, params.height);
    let t = Instant::now();
    // The process's devices, handed to the renderer.
    let devices = devices(&params.devices);
    let mut renderer: Box<dyn Renderer> = (entry.create)(params, &devices, waker)?;
    let mut shm = FrameSlots::open(&shm)?;
    out.send(&WorkerMsg::Ready { info, init_ms: t.elapsed().as_secs_f32() * 1e3 })?;

    let mut slots = HashMap::new();
    let mut update_ms = 0f32;
    while let Ok(msg) = rx.recv() {
        let mut new_shm = None;
        let mut sink = Sink { out, shm: &mut shm, slots: &mut slots, update_ms: &mut update_ms, error: None };
        let msg = match msg {
            Input::Wake => {
                renderer.poll(&mut sink)?;
                if let Some(e) = sink.error.take() {
                    return Err(e);
                }
                continue;
            }
            Input::Host(m) => m,
        };
        match msg {
            HostMsg::Updates(u) => {
                let t = Instant::now();
                renderer.update(u)?;
                *sink.update_ms += t.elapsed().as_secs_f32() * 1e3;
            }
            HostMsg::Meshes(m) => {
                let t = Instant::now();
                renderer.update_meshes(m)?;
                *sink.update_ms += t.elapsed().as_secs_f32() * 1e3;
            }
            HostMsg::Render { req, slot } => {
                sink.slots.insert(req.frame_id, slot.filter(|_| req.readback));
                renderer.render(&req, &mut sink)?;
            }
            HostMsg::Flush => {
                renderer.flush(&mut sink)?;
                sink.out.send(&WorkerMsg::Flushed)?;
            }
            HostMsg::Resize { width, height, shm: spec } => {
                renderer.flush(&mut sink)?;
                renderer.resize(width, height)?;
                new_shm = Some(spec);
            }
            HostMsg::SetParams(ps) => {
                for (name, value) in ps {
                    if let Err(e) = renderer.set_param(&name, &value) {
                        sink.out.send(&WorkerMsg::ParamError { name, error: format!("{e:#}") })?;
                    }
                }
            }
            HostMsg::QueryMemory => sink.out.send(&WorkerMsg::Memory(renderer.memory()))?,
            HostMsg::Shutdown => break,
            HostMsg::Init { .. } => bail!("duplicate Init"),
        }
        if let Some(e) = sink.error.take() {
            return Err(e);
        }
        if let Some(spec) = new_shm {
            shm = FrameSlots::open(&spec)?;
        }
    }
    Ok(())
}
