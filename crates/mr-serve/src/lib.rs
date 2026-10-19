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
use mr_core::link::{Arrivals, Link};
use mr_core::{CompletedFrame, FrameSink, Renderer, RendererEntry, Waker};
use mr_ipc::{FrameSlots, HostMsg, MsgWriter, PROTOCOL_VERSION, WorkerMsg};
use mr_world::stream::Generating;

/// The `worker --socket <endpoint>` arguments a worker process is started with.
pub fn socket_arg(mut args: impl Iterator<Item = String>) -> anyhow::Result<String> {
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
    /// Per frame requested: its readback slot and what had reached the client by then.
    slots: &'a mut HashMap<u64, (Option<u32>, Arrivals)>,
    update_ms: &'a mut f32,
    error: Option<anyhow::Error>,
}

impl FrameSink for Sink<'_> {
    fn complete(&mut self, frame: CompletedFrame<'_>) {
        let (slot, arrivals) = self.slots.remove(&frame.frame_id).unwrap_or_default();
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

/// The messages the link delivered by `time`.
fn arrived(link: &mut Link, time: f64) -> Vec<mr_core::WorldUpdate> {
    let mut v = vec![];
    link.deliver(time, &mut v);
    v
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
    // The renderer's region: its radius, and the levels it asked for that it supports.
    let supported = info.max_lod;
    let levels = params.client;
    let mut client = Client::new(params.view_radius_chunks, params.max_lod.min(supported), levels);
    // The world generated here (`HostMsg::Generate`), and whether it generated this frame.
    let mut generating: Option<Generating> = None;
    let mut generated = false;
    let reach = |c: &Client| mr_core::client::reach(c.radius(), c.max_lod(), levels);
    // The link in front of it: none when free, the stream then as it came.
    let mut link = (!params.link.free()).then(|| Link::new(params.link));
    let mut renderer: Box<dyn Renderer> = (entry.create)(params, &devices, waker)?;
    let mut shm = FrameSlots::open(&shm)?;
    out.send(&WorkerMsg::Ready { info, init_ms: t.elapsed().as_secs_f32() * 1e3 })?;

    let mut slots = HashMap::new();
    let mut update_ms = 0f32;
    // Bytes over a free link (a modelled one counts its own).
    let mut free_bytes = 0u64;
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
            HostMsg::Stream { updates, at, time } => {
                if link.is_none() {
                    free_bytes += updates.iter().map(|u| mr_core::link::wire_bytes(u) as u64).sum::<u64>();
                }
                let t = Instant::now();
                let mut ups = vec![];
                let got = match &mut link {
                    Some(l) => {
                        l.send(time, updates);
                        arrived(l, time)
                    }
                    None => updates,
                };
                match &mut generating {
                    Some(g) => {
                        let mut local = vec![];
                        g.receive(&got, &mut local);
                        if let Some(at) = at {
                            g.update(at, &mut local);
                            generated = true;
                        }
                        client.receive(&local, &mut ups);
                    }
                    None => client.receive(&got, &mut ups),
                }
                if let Some(at) = at {
                    client.update(at, &mut ups);
                }
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
            HostMsg::Render { req, slot, time } => {
                // What arrived by now; the camera may have crossed into chunks the replica holds.
                let t = Instant::now();
                let mut ups = vec![];
                let got = link.as_mut().map(|l| arrived(l, time)).unwrap_or_default();
                match &mut generating {
                    Some(g) => {
                        // Generated once per frame: here if no stream ended at a camera.
                        let mut local = vec![];
                        g.receive(&got, &mut local);
                        if !std::mem::take(&mut generated) {
                            g.update(req.camera.position, &mut local);
                        }
                        client.receive(&local, &mut ups);
                    }
                    None => client.receive(&got, &mut ups),
                }
                client.update(req.camera.position, &mut ups);
                if !ups.is_empty() {
                    renderer.update(ups)?;
                    *sink.update_ms += t.elapsed().as_secs_f32() * 1e3;
                }
                let holes = link.as_ref().map_or(0, |l| l.chunks_in_flight()) + generating.as_ref().map_or(0, |g| g.pending());
                let (bytes, arrived) = link.as_ref().map_or((free_bytes, free_bytes), |l| (l.stats().bytes, l.stats().arrived));
                let arrivals = Arrivals { bytes, arrived, holes: holes as u32 };
                sink.slots.insert(req.frame_id, (slot.filter(|_| req.readback), arrivals));
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
                        client.receive(&local, &mut ups);
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
