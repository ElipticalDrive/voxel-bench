//! The frame loop: drives one or more renderer lanes through a timeline of frames,
//! keeping their worlds in sync and routing finished frames to the outputs.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::bail;
use glam::{DVec3, Vec3};
use mr_core::{Camera, Environment, FrameRequest, Light, MeshData, MeshInstance, MeshUpdate, PassMode, WorldUpdate};
use mr_ipc::{HostMsg, WorkerMsg};
use mr_world::scenario::{FrameEvents, resolve_edits};
use mr_world::{Scene, World};

use crate::compare::{self, Quality};
use crate::plan::{Pacing, Readback, RunSpec, SaveSpec};
use crate::report::{FirstComplete, FrameRecord, PassReport};
use crate::stream::{Region, Streamer};
use crate::window::{UiEvent, UiHandle};
use crate::worker::{Worker, WorkerEvent, recv_timeout};

pub struct FrameInput {
    /// Index in the measured timeline (warm-up frames use 0).
    pub frame: u32,
    pub camera: Camera,
    pub mode: PassMode,
    pub events: FrameEvents,
    /// Mesh instances this frame (mesh mode); `None` when the pass draws no meshes.
    pub meshes: Option<Vec<MeshInstance>>,
    /// False: advance world state only (e.g. reference passes skip unsampled frames).
    pub render: bool,
    /// False for warm-up frames.
    pub recorded: bool,
    /// The frame's time in seconds (the link's clock): simulated in
    /// scripted runs, the wall clock in interactive ones.
    pub time: f64,
}

pub trait Driver {
    fn next(&mut self) -> anyhow::Result<Option<FrameInput>>;
}

/// Follows a scene's scripted camera path and events.
pub struct ScriptedDriver {
    pub scene: Scene,
    pub mode: PassMode,
    pub warmup: u32,
    pub frames: u32,
    /// Render only frames where `f % n == 0` (and the last one).
    pub only_every: Option<u32>,
    /// The simulated clock's frames per second (`run.clock_fps`).
    pub clock_fps: f64,
    i: u32,
}

impl ScriptedDriver {
    pub fn new(scene: Scene, mode: PassMode, warmup: u32, frames: u32, only_every: Option<u32>, clock_fps: f64) -> Self {
        Self { scene, mode, warmup, frames, only_every, clock_fps, i: 0 }
    }
}

impl Driver for ScriptedDriver {
    fn next(&mut self) -> anyhow::Result<Option<FrameInput>> {
        let i = self.i;
        self.i += 1;
        let time = i as f64 / self.clock_fps;
        if i < self.warmup {
            let camera = self.scene.camera(0);
            let meshes = self.mode.meshes.then(|| self.scene.mesh_instances(0));
            return Ok(Some(FrameInput { frame: 0, camera, mode: self.mode, events: Default::default(), meshes, render: true, recorded: false, time }));
        }
        let f = i - self.warmup;
        if f >= self.frames {
            return Ok(None);
        }
        let camera = self.scene.camera(f);
        let events = self.scene.events(f, &camera)?;
        let render = self.only_every.is_none_or(|n| sampled(f, n, self.frames));
        let meshes = self.mode.meshes.then(|| self.scene.mesh_instances(f));
        Ok(Some(FrameInput { frame: f, camera, mode: self.mode, events, meshes, render, recorded: true, time }))
    }
}

pub fn sampled(frame: u32, every: u32, frames: u32) -> bool {
    (every > 0 && frame.is_multiple_of(every)) || frame + 1 == frames
}

/// Free-flying camera controlled from the window.
pub struct InteractiveDriver {
    pub ui: UiHandle,
    pub scene: Scene,
    pub camera: Camera,
    pub mode: PassMode,
    pub speed: f32,
    frame: u32,
    last: Instant,
    start: Instant,
    fps_window: (Instant, u32),
}

impl InteractiveDriver {
    pub fn new(ui: UiHandle, scene: Scene, mode: PassMode) -> Self {
        let camera = scene.camera(0);
        Self { ui, scene, camera, mode, speed: 20.0, frame: 0, last: Instant::now(), start: Instant::now(), fps_window: (Instant::now(), 0) }
    }
}

impl Driver for InteractiveDriver {
    fn next(&mut self) -> anyhow::Result<Option<FrameInput>> {
        use winit::keyboard::KeyCode as K;
        if self.ui.closed() {
            return Ok(None);
        }
        let dt = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        {
            let mut inp = self.ui.input.lock().unwrap();
            self.camera.yaw += inp.look.x * 0.004;
            self.camera.pitch = (self.camera.pitch - inp.look.y * 0.004).clamp(-1.55, 1.55);
            inp.look = Default::default();
            self.speed *= 1.25f32.powi(std::mem::take(&mut inp.speed_steps));
            if std::mem::take(&mut inp.toggle_mode) {
                self.mode.lighting = if self.mode.lighting == mr_core::RenderMode::Unlit { mr_core::RenderMode::Lit } else { mr_core::RenderMode::Unlit };
            }
            let f = self.camera.forward();
            let fwd = Vec3::new(f.x, 0.0, f.z).normalize_or_zero();
            let right = self.camera.right();
            let mut m = Vec3::ZERO;
            for (k, d) in [(K::KeyW, fwd), (K::KeyS, -fwd), (K::KeyD, right), (K::KeyA, -right), (K::Space, Vec3::Y), (K::ShiftLeft, -Vec3::Y)] {
                if inp.held.contains(&k) {
                    m += d;
                }
            }
            let boost = if inp.held.contains(&K::ControlLeft) { 5.0 } else { 1.0 };
            self.camera.position += (m.normalize_or_zero() * self.speed * boost * dt).as_dvec3();
            if std::mem::take(&mut inp.print_camera) {
                let p = self.camera.position;
                println!(
                    "camera = {{ type = \"static\", position = [{:.2}, {:.2}, {:.2}], yaw_deg = {:.1}, pitch_deg = {:.1} }}",
                    p.x,
                    p.y,
                    p.z,
                    self.camera.yaw.to_degrees(),
                    self.camera.pitch.to_degrees()
                );
            }
        }
        let events = self.scene.events(self.frame, &self.camera)?;
        self.frame += 1;
        self.fps_window.1 += 1;
        let el = self.fps_window.0.elapsed().as_secs_f64();
        if el > 0.5 {
            let p = self.camera.position;
            self.ui.send(UiEvent::Title(format!(
                "multirenderer — {:.0} fps — {} — ({:.0}, {:.0}, {:.0}) — WASD/Space/Shift move, drag look, wheel speed, L lit, P print camera",
                self.fps_window.1 as f64 / el,
                self.mode.name(),
                p.x,
                p.y,
                p.z
            )));
            self.fps_window = (Instant::now(), 0);
        }
        let meshes = self.mode.meshes.then(|| self.scene.mesh_instances(self.frame));
        Ok(Some(FrameInput { frame: self.frame, camera: self.camera, mode: self.mode, events, meshes, render: true, recorded: false, time: self.start.elapsed().as_secs_f64() }))
    }
}

struct InFlight {
    id: u64,
    frame: u32,
    mode: PassMode,
    sent: Instant,
    recorded: bool,
    /// The frame's time on the run's clock (s).
    time: f64,
    /// The server's chunks of the lane's region not yet sent when the frame was requested.
    backlog: u32,
}

pub struct Lane {
    pub worker: Worker,
    /// What its client takes of the server's stream (the client runs in the worker).
    pub region: Region,
    pub report: PassReport,
    pub ui_index: usize,
    width: u32,
    height: u32,
    in_flight: VecDeque<InFlight>,
    next_id: u64,
    completions: Vec<f64>,
    /// Bytes arrived over its link by the previous frame.
    arrived: u64,
    last_ui: Option<Instant>,
    /// Comparisons still running off the frame loop: (index into `per_frame`, result).
    pending_quality: Vec<(usize, crossbeam_channel::Receiver<Quality>)>,
}

impl Lane {
    pub fn new(worker: Worker, region: Region, report: PassReport, ui_index: usize, width: u32, height: u32) -> Self {
        Self { worker, region, report, ui_index, width, height, in_flight: VecDeque::new(), next_id: 0, completions: vec![], arrived: 0, last_ui: None, pending_quality: vec![] }
    }
}

/// Where finished frames go.
pub struct Outputs {
    pub ui: Option<UiHandle>,
    pub readback: Readback,
    pub save: Option<(SaveSpec, PathBuf)>,
    pub compare_every: Option<u32>,
    /// Reference images by (mode, frame).
    pub references: HashMap<(PassMode, u32), Arc<Vec<u8>>>,
    /// When set, sampled frames are stored as references instead of compared.
    pub recording_reference: bool,
    pub frames: u32,
    writer: Option<crossbeam_channel::Sender<SaveJob>>,
    /// Comparison threads. SSIM and MS-SSIM take ~18 ms per 960×540 frame; on the frame loop
    /// they stalled the pipeline and halved the measured fps of fast renderers.
    comparer: crossbeam_channel::Sender<Box<dyn FnOnce() + Send>>,
}

type SaveJob = (PathBuf, u32, u32, Vec<u8>);

impl Outputs {
    pub fn new(ui: Option<UiHandle>, readback: Readback, save: Option<(SaveSpec, PathBuf)>, compare_every: Option<u32>, frames: u32) -> Self {
        let writer = save.as_ref().map(|_| {
            let (tx, rx) = crossbeam_channel::unbounded::<(PathBuf, u32, u32, Vec<u8>)>();
            for _ in 0..4 {
                let rx = rx.clone();
                std::thread::spawn(move || {
                    for (path, w, h, px) in rx {
                        if let Some(d) = path.parent() {
                            let _ = std::fs::create_dir_all(d);
                        }
                        if let Err(e) = save_image(&path, &px, w, h) {
                            tracing::error!("saving {}: {e}", path.display());
                        }
                    }
                });
            }
            tx
        });
        // Bounded, so comparing every frame of a long run cannot queue unbounded images; a full
        // queue blocks the frame loop again, which only such quality-focused runs hit.
        let (comparer, jobs) = crossbeam_channel::bounded::<Box<dyn FnOnce() + Send>>(64);
        for _ in 0..2 {
            let jobs = jobs.clone();
            std::thread::spawn(move || {
                for job in jobs {
                    job();
                }
            });
        }
        Self { ui, readback, save, compare_every, references: HashMap::new(), recording_reference: false, frames, writer, comparer }
    }

    fn save_hit(&self, f: u32) -> bool {
        self.save.as_ref().is_some_and(|(s, _)| sampled(f, s.every, self.frames))
    }

    fn compare_hit(&self, f: u32) -> bool {
        self.compare_every.is_some_and(|n| sampled(f, n, self.frames))
    }

    fn wants(&self, f: u32, recorded: bool) -> bool {
        match self.readback {
            Readback::Never => false,
            Readback::Always => true,
            Readback::Auto => self.ui.is_some() || (recorded && (self.save_hit(f) || self.compare_hit(f))),
        }
    }

    /// Hands a finished frame to the window, the saver and the comparison. The comparison runs on
    /// the comparer threads; its result arrives on the returned channel.
    /// `px` is the frame in the worker's format; the window takes it as it is (it presents a
    /// float frame in HDR where the display can). Comparisons and PNGs take its RGBA8 form (a
    /// float frame converted only when one of them does); the float frame of an `hdr` post is
    /// saved as Radiance `.hdr` next to the PNG.
    fn deliver(&mut self, lane: &mut Lane, frame: u32, mode: PassMode, recorded: bool, px: Vec<u8>) -> Option<crossbeam_channel::Receiver<Quality>> {
        let (w, h) = (lane.width, lane.height);
        // Throttle what goes to the window to ~60 Hz per lane.
        let to_ui = self.ui.is_some() && lane.last_ui.is_none_or(|t| t.elapsed() > Duration::from_millis(15));
        if !to_ui && !(recorded && (self.compare_hit(frame) || self.save_hit(frame))) {
            return None;
        }
        if let (true, Some(ui)) = (to_ui, &self.ui) {
            lane.last_ui = Some(Instant::now());
            ui.send(UiEvent::Frame { lane: lane.ui_index, width: w, height: h, format: lane.worker.format, pixels: Arc::new(px.clone()) });
        }
        if !recorded {
            return None;
        }
        let (px, hdr) = crate::worker::rgba8(lane.worker.format, px);
        let tag = format!("{}-{}", sanitize(&lane.report.label), mode.name());
        let mut quality = None;
        if self.compare_hit(frame) {
            if self.recording_reference {
                self.references.insert((mode, frame), Arc::new(px.to_vec()));
            } else if let Some(r) = self.references.get(&(mode, frame))
                && r.len() == px.len() {
                let diff = match (self.save_hit(frame), &self.save, &self.writer) {
                    (true, Some((s, dir)), Some(tx)) if s.diffs => Some((dir.join(format!("{tag}/{frame:06}-diff.png")), tx.clone())),
                    _ => None,
                };
                let (r, px) = (r.clone(), px.clone());
                let (tx, rx) = crossbeam_channel::bounded(1);
                self.comparer
                    .send(Box::new(move || {
                        let _ = tx.send(compare::compare(&r, &px, w));
                        if let Some((path, saver)) = diff {
                            let _ = saver.send((path, w, h, compare::diff_image(&r, &px)));
                        }
                    }))
                    .expect("comparer threads stopped");
                quality = Some(rx);
            }
        }
        if self.save_hit(frame)
            && let (Some((_, dir)), Some(tx)) = (&self.save, &self.writer) {
            if let Some(hdr) = hdr {
                let _ = tx.send((dir.join(format!("{tag}/{frame:06}.hdr")), w, h, hdr));
            }
            let _ = tx.send((dir.join(format!("{tag}/{frame:06}.png")), w, h, px));
        }
        quality
    }
}

/// Saves RGBA8 `px` (a PNG), or for a `.hdr` path a float frame (`Rgba16F`) as Radiance
/// RGBE in linear light.
fn save_image(path: &Path, px: &[u8], w: u32, h: u32) -> anyhow::Result<()> {
    if path.extension().is_some_and(|e| e == "hdr") {
        use mr_core::lighting::{half_to_f32, srgb_extended_to_linear};
        let rgb: Vec<image::Rgb<f32>> = px.chunks_exact(8).map(|p| image::Rgb(std::array::from_fn(|c| srgb_extended_to_linear(half_to_f32(u16::from_le_bytes([p[2 * c], p[2 * c + 1]])))))).collect();
        image::codecs::hdr::HdrEncoder::new(std::io::BufWriter::new(std::fs::File::create(path)?)).encode(&rgb, w as usize, h as usize)?;
    } else {
        image::save_buffer(path, px, w, h, image::ExtendedColorType::Rgba8)?;
    }
    Ok(())
}

pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

const BATCH: usize = 256;

/// Sends the server's stream to a worker in batches, sent at `time`; its client derives its
/// renderer's region at camera position `at` after the last.
pub fn send_stream(w: &Worker, mut ups: Vec<WorldUpdate>, at: DVec3, time: f64) -> anyhow::Result<()> {
    while !ups.is_empty() {
        let rest = if ups.len() > BATCH { ups.split_off(BATCH) } else { vec![] };
        let last = rest.is_empty();
        w.send(HostMsg::Stream { updates: std::mem::replace(&mut ups, rest), at: last.then_some(at), time })?;
    }
    Ok(())
}

fn on_msg(lane: &mut Lane, out: &mut Outputs, start: Instant, m: WorkerMsg) -> anyhow::Result<()> {
    let WorkerMsg::FrameDone { frame_id, stats, slot, update_ms, arrivals } = m else {
        bail!("[{}] unexpected message {m:?}", lane.report.label);
    };
    let Some(f) = lane.in_flight.pop_front() else { bail!("[{}] completed a frame never requested", lane.report.label) };
    if f.id != frame_id {
        bail!("[{}] frames completed out of order ({} before {})", lane.report.label, frame_id, f.id);
    }
    let latency_ms = f.sent.elapsed().as_secs_f64() * 1e3;
    let quality = match slot {
        Some(s) => {
            let px = lane.worker.take_frame(s, (lane.width * lane.height) as usize);
            out.deliver(lane, f.frame, f.mode, f.recorded, px)
        }
        None => None,
    };
    // What its client lacked and what crossed its link.
    let holes = arrivals.holes + f.backlog;
    let arrived_bytes = arrivals.arrived - lane.arrived;
    lane.arrived = arrivals.arrived;
    lane.report.wire_bytes = arrivals.bytes;
    lane.report.arrived_bytes = arrivals.arrived;
    if holes == 0 && lane.report.first_complete.is_none() {
        lane.report.first_complete = Some(FirstComplete { frame: f.frame, ms: start.elapsed().as_secs_f64() * 1e3, clock_s: f.time });
    }
    if f.recorded {
        lane.completions.push(start.elapsed().as_secs_f64() * 1e3);
        if let Some(rx) = quality {
            lane.pending_quality.push((lane.report.per_frame.len(), rx));
        }
        lane.report.per_frame.push(FrameRecord { frame: f.frame, latency_ms, cpu_ms: stats.cpu_ms, gpu_ms: stats.gpu_ms, update_ms, arrived_bytes, holes, counters: stats.counters, stages: stats.stages, quality: None });
    }
    Ok(())
}

fn pump(lane: &mut Lane, out: &mut Outputs, start: Instant, block: bool) -> anyhow::Result<()> {
    if block {
        let m = recv_timeout(&lane.worker.rx, Duration::from_secs(600)).map_err(|e| e.context(format!("[{}]", lane.report.label)))?;
        on_msg(lane, out, start, m)?;
    }
    while let Ok(ev) = lane.worker.rx.try_recv() {
        match ev {
            WorkerEvent::Msg(WorkerMsg::Error(e)) => bail!("[{}] worker error: {e}", lane.report.label),
            WorkerEvent::Msg(m) => on_msg(lane, out, start, m)?,
            WorkerEvent::Closed => bail!("[{}] worker exited unexpectedly", lane.report.label),
        }
    }
    Ok(())
}

fn flush_lane(lane: &mut Lane, out: &mut Outputs, start: Instant) -> anyhow::Result<()> {
    lane.worker.send(HostMsg::Flush)?;
    loop {
        let m = recv_timeout(&lane.worker.rx, Duration::from_secs(600)).map_err(|e| e.context(format!("[{}] flushing", lane.report.label)))?;
        if let WorkerMsg::Flushed = m {
            return Ok(());
        }
        on_msg(lane, out, start, m)?;
    }
}

pub fn run_pass(
    lanes: &mut [Lane],
    driver: &mut dyn Driver,
    world: &mut World,
    environment: Environment,
    lights: &[Light],
    meshes: &[Arc<MeshData>],
    run: &RunSpec,
    out: &mut Outputs,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let mut first = true;
    let mut rendered = 0u64;
    let mut pace_start: Option<Instant> = None;
    let mut measure_start: Option<Instant> = None;
    let mut missed = 0u32;
    let mut sent_instances: Option<Vec<MeshInstance>> = None;
    // The server streams one region, as large as the largest any client needs streamed at
    // each level; clients generating the world need nothing streamed.
    let streams = lanes.iter().any(|l| !l.region.generates);
    let radii = crate::stream::union(lanes.iter().map(|l| l.region.streamed()), 0);
    let mut server = Streamer::new(radii.clone(), run.budget());
    for l in lanes.iter() {
        l.worker.send(HostMsg::Follow { radii: radii.clone() })?;
    }
    while let Some(input) = driver.next()? {
        // Mesh mode: geometry once, instances whenever they change.
        let instances = input.meshes.filter(|i| sent_instances.as_ref() != Some(i));
        let edits = resolve_edits(world, input.events.edits);
        let changed = world.apply_edits(&edits);
        let (mut head, mut tail) = (vec![], vec![]);
        // Everything a renderer knows of the world comes over the stream,
        // the environment too (its creation's is a default until then).
        if first {
            head.push(WorldUpdate::SetEnvironment(environment));
            head.push(WorldUpdate::LightUpsert(lights.to_vec()));
        }
        if !input.events.lights.is_empty() {
            tail.push(WorldUpdate::LightUpsert(input.events.lights.clone()));
        }
        if let Some(env) = input.events.environment {
            tail.push(WorldUpdate::SetEnvironment(env));
        }
        let mut stream = head.clone();
        if streams {
            server.edits(&edits, &changed, &mut stream);
            server.update(world, input.camera.position, &mut stream);
        }
        stream.extend_from_slice(&tail);
        // Clients generating the world get every edit (from the start: those before the pass).
        let mut edit_stream = head;
        let before = if first { world.edits() } else { vec![] };
        if first && !before.is_empty() {
            edit_stream.push(WorldUpdate::SetBlocks(before));
        } else if !first && !edits.is_empty() {
            edit_stream.push(WorldUpdate::SetBlocks(edits));
        }
        edit_stream.extend(tail);
        // What crossed the link to each client.
        let loads = stream.iter().filter(|u| matches!(u, WorldUpdate::ChunkLoad { .. })).count() as u64;
        for lane in lanes.iter_mut() {
            let stream = if lane.region.generates { &edit_stream } else { &stream };
            if !lane.region.generates {
                lane.report.chunks_sent += loads;
            }
            if first && !meshes.is_empty() {
                lane.worker.send(HostMsg::Meshes(MeshUpdate::Meshes(meshes.to_vec())))?;
            }
            if let Some(i) = &instances {
                lane.worker.send(HostMsg::Meshes(MeshUpdate::Instances(i.clone())))?;
            }
            if first {
                let t = Instant::now();
                send_stream(&lane.worker, stream.clone(), input.camera.position, input.time)?;
                lane.worker.flush(|m| bail!("unexpected {m:?} during initial load"))?;
                lane.report.initial_load_ms = t.elapsed().as_secs_f64() * 1e3;
            } else if !stream.is_empty() {
                send_stream(&lane.worker, stream.clone(), input.camera.position, input.time)?;
            }
        }
        first = false;
        if instances.is_some() {
            sent_instances = instances;
        }
        if !input.render {
            continue;
        }
        if let Pacing::Fps { fps } = run.pacing {
            let ps = *pace_start.get_or_insert_with(Instant::now);
            let target = ps + Duration::from_secs_f64(rendered as f64 / fps);
            let now = Instant::now();
            if now < target {
                std::thread::sleep(target - now);
            } else if input.recorded && now > target + Duration::from_secs_f64(1.0 / fps) {
                missed += 1;
            }
        }
        if input.recorded && measure_start.is_none() {
            // Let warm-up frames drain so they don't pollute the first measurements.
            for lane in lanes.iter_mut() {
                while !lane.in_flight.is_empty() {
                    pump(lane, out, start, true)?;
                }
            }
            measure_start = Some(Instant::now());
        }
        for lane in lanes.iter_mut() {
            while lane.in_flight.len() >= run.in_flight.max(1) as usize {
                pump(lane, out, start, true)?;
            }
            pump(lane, out, start, false)?;
            let readback = out.wants(input.frame, input.recorded);
            let slot = if readback { lane.worker.take_slot() } else { None };
            let id = lane.next_id;
            lane.next_id += 1;
            let req = FrameRequest { frame_id: id, camera: input.camera, mode: input.mode.lighting, meshes: input.mode.meshes, readback: slot.is_some() };
            let backlog = if streams && !lane.region.generates { server.pending_within(&lane.region.streamed()) as u32 } else { 0 };
            lane.in_flight.push_back(InFlight { id, frame: input.frame, mode: input.mode, sent: Instant::now(), recorded: input.recorded, time: input.time, backlog });
            lane.worker.send(HostMsg::Render { req, slot, time: input.time })?;
        }
        rendered += 1;
    }
    for lane in lanes.iter_mut() {
        flush_lane(lane, out, start)?;
        if !lane.in_flight.is_empty() {
            bail!("[{}] flush returned with frames in flight", lane.report.label);
        }
        let ms = measure_start.unwrap_or(start);
        let ms_off = ms.duration_since(start).as_secs_f64() * 1e3;
        lane.report.wall_s = lane.completions.last().map(|&c| (c - ms_off) / 1e3).unwrap_or(0.0);
        if matches!(run.pacing, Pacing::Fps { .. }) {
            lane.report.missed_deadlines = Some(missed);
        }
        lane.report.peak_rss_mb = lane.worker.peak_rss().map(|b| b as f64 / 1048576.0);
        lane.worker.send(HostMsg::QueryMemory)?;
        if let Ok(WorkerMsg::Memory(m)) = recv_timeout(&lane.worker.rx, Duration::from_secs(30)) {
            lane.report.memory = m;
        }
        for (i, rx) in lane.pending_quality.drain(..) {
            lane.report.per_frame[i].quality = Some(rx.recv()?);
        }
        let completions = std::mem::take(&mut lane.completions);
        lane.report.finish(&completions);
        // A client generating the world takes edits only: its update cost is generation's.
        if lane.region.generates {
            lane.report.update_ns_per_byte = None;
        }
    }
    Ok(())
}
