//! The worker side of the IPC protocol: hosts exactly one renderer, driven by the host.
//! The full worker (`mr-worker`, every renderer) and generated pipeline workers (one composition compiled for this machine) both serve through [`run`]. The worker is
//! the client side of the link: it takes the server's stream and its
//! [`Client`] derives what the renderer holds before the renderer sees it; the
//! stream reaches the client through its [`Link`] (latency, bandwidth, loss) unless free;
//! the client may generate the world itself ([`Generating`]), the server's
//! stream then carrying edits only.

use std::collections::HashMap;
use std::time::Instant;

use anyhow::{Context, bail};
use mr_core::client::Client;
use mr_core::gate::{Frame, GateChain};
use mr_core::gate::{Arrivals, wire_bytes};
use mr_core::{CompletedFrame, FrameSink, Renderer, RendererEntry, Waker};
use mr_ipc::{FrameSlots, HostMsg, MsgWriter, PROTOCOL_VERSION, WorkerMsg};
use mr_world::stream::Generating;

pub mod fault;

/// Whether this worker is a process of its own (not a thread in the host).
static OWN_PROCESS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Marks this worker as a process of its own: when the host's end of the socket closes
/// without a shutdown (the host died or gave up on it), the process exits at once, even with
/// its renderer hung, rather than outliving its host. A worker thread never exits the process.
pub fn own_process() {
    OWN_PROCESS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The `worker --socket <endpoint>` arguments a worker process is started with (and marks it
/// as one, [`own_process`]).
pub fn socket_arg(mut args: impl Iterator<Item = String>) -> anyhow::Result<String> {
    own_process();
    while let Some(a) = args.next() {
        if a == "--socket" {
            return args.next().context("--socket needs an endpoint");
        }
    }
    bail!("usage: <worker> worker --socket <endpoint>")
}

/// Devices for a process without a GPU API (CPU-only pipelines).
pub fn cpu_devices(_: &mr_core::DeviceConfig) -> mr_core::Devices {
    mr_core::Devices { gpus: None }
}

struct Sink<'a> {
    out: &'a mut MsgWriter<WorkerMsg>,
    shm: &'a mut FrameSlots,
    /// Per frame requested: its readback slot, what had reached the client by then and the
    /// host's clock.
    slots: &'a mut HashMap<u64, (Option<u32>, Arrivals, f64)>,
    update_ms: &'a mut f32,
    /// The gates behind the graph's output, if any.
    frames: &'a mut Option<FrameGates>,
    error: Option<anyhow::Error>,
}

/// The gates behind the graph's output: every frame is serialized through them; the host
/// gets the last frame that arrived at the current size (black before any has).
struct FrameGates {
    chain: GateChain,
    width: u32,
    height: u32,
    last: Vec<u8>,
}

impl FrameGates {
    /// Sends `px`, rendered at `time`; `last` is then the frame delivered.
    fn pass(&mut self, time: f64, px: &[u8]) {
        let sent = Frame { width: self.width, height: self.height, pixels: px.to_vec() };
        let got = self.chain.gates.pass(time, Some(sent));
        let size = (self.width, self.height);
        if let Some(f) = got.into_iter().rev().find(|f| f.pixels.len() == px.len() && (f.width, f.height) == size) {
            self.last = f.pixels;
        }
        if self.last.len() != px.len() {
            self.last = vec![0; px.len()];
        }
    }
}

impl FrameSink for Sink<'_> {
    fn complete(&mut self, mut frame: CompletedFrame<'_>) {
        let (slot, mut arrivals, time) = self.slots.remove(&frame.frame_id).unwrap_or_default();
        let gated = match (self.frames.as_mut(), frame.pixels) {
            (Some(g), Some(px)) => {
                g.pass(time, px);
                arrivals.gates.extend(g.chain.reports());
                Some(g.last.as_slice())
            }
            (_, px) => px,
        };
        if let (Some(s), Some(px)) = (slot, gated) {
            let t0 = std::time::Instant::now();
            let dst = self.shm.slot_mut(s);
            if px.len() > dst.len() {
                self.error.get_or_insert(anyhow::anyhow!("frame of {} bytes exceeds slot", px.len()));
                return;
            }
            dst[..px.len()].copy_from_slice(px);
            frame.stats.counters.push(("readback_shm_ms".into(), t0.elapsed().as_secs_f64() * 1e3));
        }
        let msg = WorkerMsg::FrameDone {
            frame_id: frame.frame_id,
            stats: frame.stats,
            slot: slot.filter(|_| frame.pixels.is_some()),
            update_ms: std::mem::take(self.update_ms),
            arrivals,
        };
        if let Err(e) = self.out.send(&msg) {
            self.error.get_or_insert(e);
        }
    }
}

/// Connects to the host at `socket` and serves one of `entries` until shutdown. `devices`
/// opens the process's devices for the configuration the host sends.
pub fn run(socket: &str, entries: Vec<RendererEntry>, devices: fn(&mr_core::DeviceConfig) -> mr_core::Devices) -> anyhow::Result<()> {
    let (mut input, mut out) = mr_ipc::split::<HostMsg, WorkerMsg>(mr_ipc::connect(socket)?);
    out.send(&WorkerMsg::Hello { pid: std::process::id(), protocol: PROTOCOL_VERSION, renderers: entries.iter().map(|e| e.name.to_string()).collect() })?;

    // Drain the socket on a separate thread so the host never blocks on a busy renderer.
    // The renderer's waker feeds the same queue, so completions are handled promptly.
    let (tx, rx) = crossbeam_channel::unbounded::<Input>();
    let wake_tx = tx.clone();
    let waker = Waker::new(move || {
        let _ = wake_tx.send(Input::Wake);
    });
    std::thread::Builder::new().name("ipc-reader".into()).spawn(move || {
        let mut shutdown = false;
        loop {
            match input.recv() {
                Ok(Some(m)) => {
                    shutdown |= matches!(m, HostMsg::Shutdown);
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
        // The host is gone without asking us to stop: nobody is left to report to, and a
        // hung renderer would keep the process (and its device memory) alive forever.
        if !shutdown && OWN_PROCESS.load(std::sync::atomic::Ordering::Relaxed) {
            // The host's death usually took the log pipe with it: a failed write must not
            // stop the exit (tracing-subscriber reports it with `eprintln!`, which panics).
            let _ = std::panic::catch_unwind(|| tracing::warn!("the host closed the connection without a shutdown; exiting"));
            std::process::exit(3);
        }
    })?;

    // A panic is the renderer's failure, said to the host like an error: unwinding alone
    // would leave the socket open (the reader thread holds it), and with thread isolation the
    // host would wait for a message that never comes.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| serve(&rx, &mut out, waker, entries, devices)))
        .unwrap_or_else(|p| Err(anyhow::anyhow!("panicked: {}", panic_message(&*p))));
    if let Err(e) = &result {
        let _ = out.send(&WorkerMsg::Error(format!("{e:#}")));
    }
    result
}

/// The text of a panic's payload.
fn panic_message(p: &(dyn std::any::Any + Send)) -> &str {
    p.downcast_ref::<&str>().copied().or_else(|| p.downcast_ref::<String>().map(String::as_str)).unwrap_or("(no message)")
}

/// The world's changes `got` by `time` sent through the gates (one message per change:
/// `gate::world_messages`), and what came out of them by then.
fn gated(gates: &mut Option<GateChain>, time: f64, got: Vec<mr_core::WorldUpdate>) -> Vec<mr_core::WorldUpdate> {
    match gates {
        Some(g) => g.gates.pass_all(time, mr_core::gate::world_messages(got)).into_iter().flatten().collect(),
        None => got,
    }
}

/// The gates on the world's side of a renderer, each chain on its own edge: into the client
/// (the server-to-client crossing), on the generated chunks (a generator on the client's
/// cpu) and on the client's changes into the renderer.
struct WorldGates {
    stream: Option<GateChain>,
    chunks: Option<GateChain>,
    client: Option<GateChain>,
}

impl WorldGates {
    fn new(p: &mr_core::InitParams) -> Self {
        let chain = |edge: &str, nodes: &[mr_core::gate::GateNode]| (!nodes.is_empty()).then(|| GateChain::new(edge, nodes));
        Self { stream: chain("client.changes", &p.gates), chunks: chain("client.chunks", &p.chunk_gates), client: chain("changes", &p.client_gates) }
    }

    fn chains(&self) -> impl Iterator<Item = &GateChain> {
        [&self.stream, &self.chunks, &self.client].into_iter().flatten()
    }

    /// Chunk loads inside any of them.
    fn holes(&self) -> u32 {
        self.chains().map(|c| c.gates.pieces_in_flight()).sum()
    }
}

// Few in flight at a time: boxing `HostMsg` would save no memory worth having.
#[allow(clippy::large_enum_variant)]
enum Input {
    Host(HostMsg),
    Wake,
}

fn serve(rx: &crossbeam_channel::Receiver<Input>, out: &mut MsgWriter<WorkerMsg>, waker: Waker, entries: Vec<RendererEntry>, devices: fn(&mr_core::DeviceConfig) -> mr_core::Devices) -> anyhow::Result<()> {
    let Ok(Input::Host(HostMsg::Init { renderer: name, mut params, shm })) = rx.recv() else {
        bail!("expected Init as first message");
    };
    let entry = entries.into_iter().find(|e| e.name == name).with_context(|| format!("unknown renderer `{name}`"))?;
    let info = (entry.info)(&params)?;
    tracing::info!("initialising {name} at {}x{}", params.width, params.height);
    let t = Instant::now();
    mr_core::shader_clock::take();
    // The process's devices, handed to the renderer.
    let devices = devices(&params.devices);
    // The renderer's region: its radius, and the levels it asked for that it supports.
    // The renderer is asked only for what it reports: no level beyond its own.
    let supported = info.features.max_lod();
    params.max_lod = params.max_lod.min(supported);
    let levels = params.client;
    let mut client = Client::new(params.view_radius_chunks, params.max_lod, levels);
    // The world generated here (`HostMsg::Generate`), and whether it generated this frame.
    let mut generating: Option<Generating> = None;
    let mut generated = false;
    let reach = |c: &Client| mr_core::client::reach(c.radius(), c.max_lod(), levels);
    // The gates on its world's side (none: the stream as it came).
    let mut wg = WorldGates::new(&params);
    // The host's clock at its last message (what changes made outside a frame cross at).
    let mut now = 0f64;
    // The gates behind its output: each frame crosses them (read back for them).
    let mut frames = (!params.frame_gates.is_empty()).then(|| FrameGates { chain: GateChain::new("frame", &params.frame_gates), width: params.width, height: params.height, last: vec![] });
    // A fault asked for (tests only: `fault::ENV`), at build or later.
    let fault = fault::Fault::of_options(&params.options_json)?;
    if let Some(f) = fault {
        f.trip(fault::At::Build)?;
    }
    let mut renderer: Box<dyn Renderer> = (entry.create)(*params, &devices, waker)?;
    if let Some(f) = fault {
        renderer = Box::new(fault::Faulty::new(renderer, f));
    }
    let mut shm = FrameSlots::open(&shm)?;
    out.send(&WorkerMsg::Ready { info, init_ms: t.elapsed().as_secs_f32() * 1e3, shaders: mr_core::shader_clock::take() })?;

    let mut slots = HashMap::new();
    let mut update_ms = 0f32;
    // Bytes of the world's stream sent ungated, and that came out of its gates.
    let (mut free_bytes, mut arrived_bytes) = (0u64, 0u64);
    while let Ok(msg) = rx.recv() {
        let mut new_shm = None;
        let mut sink = Sink { out, shm: &mut shm, slots: &mut slots, update_ms: &mut update_ms, frames: &mut frames, error: None };
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
            HostMsg::Stream { updates, at, time } => {
                now = time;
                if wg.stream.is_none() {
                    free_bytes += updates.iter().map(|u| wire_bytes(u) as u64).sum::<u64>();
                }
                let t = Instant::now();
                let mut ups = vec![];
                let got = gated(&mut wg.stream, time, updates);
                arrived_bytes += got.iter().map(|u| wire_bytes(u) as u64).sum::<u64>();
                match &mut generating {
                    Some(g) => {
                        let mut local = vec![];
                        g.receive(&got, &mut local);
                        if let Some(at) = at {
                            g.update(at, &mut local);
                            generated = true;
                        }
                        client.receive(&gated(&mut wg.chunks, time, local), &mut ups);
                    }
                    None => client.receive(&got, &mut ups),
                }
                if let Some(at) = at {
                    client.update(at, &mut ups);
                }
                let ups = gated(&mut wg.client, time, ups);
                if !ups.is_empty() {
                    renderer.update(ups)?;
                }
                *sink.update_ms += t.elapsed().as_secs_f32() * 1e3;
            }
            // A client generating the world follows its own generator's region.
            HostMsg::Follow { radii } => client.follow(if generating.is_some() { reach(&client) } else { radii }.as_slice()),
            HostMsg::Generate { world, budget } => {
                let spec: mr_world::GenSpec = serde_json::from_str(&world).context("the world's generator spec")?;
                generating = Some(Generating::new(&spec, reach(&client), budget.map(|b| b as usize)));
            }
            HostMsg::Meshes(m) => {
                let t = Instant::now();
                renderer.update_meshes(m)?;
                *sink.update_ms += t.elapsed().as_secs_f32() * 1e3;
            }
            HostMsg::Render { mut req, slot, time } => {
                // What arrived by now; the camera may have crossed into chunks the replica holds.
                let t = Instant::now();
                let mut ups = vec![];
                now = time;
                let got = gated(&mut wg.stream, time, vec![]);
                arrived_bytes += got.iter().map(|u| wire_bytes(u) as u64).sum::<u64>();
                match &mut generating {
                    Some(g) => {
                        // Generated once per frame: here if no stream ended at a camera.
                        let mut local = vec![];
                        g.receive(&got, &mut local);
                        if !std::mem::take(&mut generated) {
                            g.update(req.camera.position, &mut local);
                        }
                        client.receive(&gated(&mut wg.chunks, time, local), &mut ups);
                    }
                    None => client.receive(&got, &mut ups),
                }
                client.update(req.camera.position, &mut ups);
                let ups = gated(&mut wg.client, time, ups);
                if !ups.is_empty() {
                    renderer.update(ups)?;
                    *sink.update_ms += t.elapsed().as_secs_f32() * 1e3;
                }
                let holes = wg.holes() as usize + generating.as_ref().map_or(0, |g| g.pending());
                let bytes = wg.stream.as_ref().map_or(free_bytes, |c| c.gates.0[0].stats().bytes);
                let gates = wg.chains().flat_map(GateChain::reports).collect();
                let arrivals = Arrivals { bytes, arrived: arrived_bytes, holes: holes as u32, gates };
                sink.slots.insert(req.frame_id, (slot.filter(|_| req.readback), arrivals, time));
                // Gates behind the output need the pixels, whether the host asked for them or not.
                req.readback |= sink.frames.is_some();
                req.time = time;
                renderer.render(&req, &mut sink)?;
            }
            HostMsg::Flush => {
                renderer.flush(&mut sink)?;
                sink.out.send(&WorkerMsg::Flushed)?;
            }
            HostMsg::Resize { width, height, shm: spec } => {
                renderer.flush(&mut sink)?;
                renderer.resize(width, height)?;
                if let Some(f) = sink.frames.as_mut() {
                    (f.width, f.height) = (width, height);
                }
                new_shm = Some(spec);
            }
            HostMsg::Edit(edits) => {
                if let Err(e) = renderer.edit(&edits) {
                    sink.out.send(&WorkerMsg::ParamError { name: "edit".into(), error: format!("{e:#}") })?;
                }
            }
            HostMsg::SetParams(ps) => {
                for (name, value) in ps {
                    // The client's region (the host's `view_radius`, `max_lod`).
                    let level = || value.as_i64().map(|v| v.clamp(0, 255) as u32);
                    let r = match name.as_str() {
                        "client.radius" => level().map(|v| client.set_radius(v)).context("client.radius takes an integer"),
                        "client.max_lod" => level().map(|v| client.set_max_lod((v as u8).min(supported))).context("client.max_lod takes an integer"),
                        _ => renderer.set_param(&name, &value),
                    };
                    // Its generator follows the client's region.
                    if let (Some(g), "client.radius" | "client.max_lod") = (&mut generating, name.as_str()) {
                        let (mut local, mut ups) = (vec![], vec![]);
                        g.set_radii(&reach(&client), &mut local);
                        client.receive(&gated(&mut wg.chunks, now, local), &mut ups);
                        let ups = gated(&mut wg.client, now, ups);
                        if !ups.is_empty() {
                            renderer.update(ups)?;
                        }
                    }
                    if let Err(e) = r {
                        sink.out.send(&WorkerMsg::ParamError { name, error: format!("{e:#}") })?;
                    }
                }
            }
            HostMsg::QueryMemory => sink.out.send(&WorkerMsg::Memory(renderer.memory()))?,
            HostMsg::Shutdown => {
                if let Some(f) = fault {
                    f.trip(fault::At::Shutdown)?;
                }
                break;
            }
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
