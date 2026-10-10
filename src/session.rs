//! The frame loop: drives one or more renderer lanes through a timeline of frames,
//! keeping their worlds in sync and routing finished frames to the outputs.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::bail;
use glam::Vec3;
use mr_core::{Camera, FrameRequest, Light, RenderMode, WorldUpdate};
use mr_ipc::{HostMsg, WorkerMsg};
use mr_world::scenario::{FrameEvents, resolve_edits};
use mr_world::{Scene, World};

use crate::compare::{self, Quality};
use crate::plan::{Pacing, Readback, RunSpec, SaveSpec};
use crate::report::{FrameRecord, PassReport};
use crate::stream::Streamer;
use crate::window::{UiEvent, UiHandle};
use crate::worker::{Worker, WorkerEvent, recv_timeout};

pub struct FrameInput {
    /// Index in the measured timeline (warm-up frames use 0).
    pub frame: u32,
    pub camera: Camera,
    pub mode: RenderMode,
    pub events: FrameEvents,
    /// False: advance world state only (e.g. reference passes skip unsampled frames).
    pub render: bool,
    /// False for warm-up frames.
    pub recorded: bool,
}

pub trait Driver {
    fn next(&mut self) -> anyhow::Result<Option<FrameInput>>;
}

/// Follows a scene's scripted camera path and events.
pub struct ScriptedDriver {
    pub scene: Scene,
    pub mode: RenderMode,
    pub warmup: u32,
    pub frames: u32,
    /// Render only frames where `f % n == 0` (and the last one).
    pub only_every: Option<u32>,
    i: u32,
}

impl ScriptedDriver {
    pub fn new(scene: Scene, mode: RenderMode, warmup: u32, frames: u32, only_every: Option<u32>) -> Self {
        Self { scene, mode, warmup, frames, only_every, i: 0 }
    }
}

impl Driver for ScriptedDriver {
    fn next(&mut self) -> anyhow::Result<Option<FrameInput>> {
        let i = self.i;
        self.i += 1;
        if i < self.warmup {
            let camera = self.scene.camera(0);
            return Ok(Some(FrameInput { frame: 0, camera, mode: self.mode, events: Default::default(), render: true, recorded: false }));
        }
        let f = i - self.warmup;
        if f >= self.frames {
            return Ok(None);
        }
        let camera = self.scene.camera(f);
        let events = self.scene.events(f, &camera)?;
        let render = self.only_every.is_none_or(|n| sampled(f, n, self.frames));
        Ok(Some(FrameInput { frame: f, camera, mode: self.mode, events, render, recorded: true }))
    }
}

pub fn sampled(frame: u32, every: u32, frames: u32) -> bool {
    (every > 0 && frame % every == 0) || frame + 1 == frames
}

/// Free-flying camera controlled from the window.
pub struct InteractiveDriver {
    pub ui: UiHandle,
    pub scene: Scene,
    pub camera: Camera,
    pub mode: RenderMode,
    pub speed: f32,
    frame: u32,
    last: Instant,
    fps_window: (Instant, u32),
}

impl InteractiveDriver {
    pub fn new(ui: UiHandle, scene: Scene, mode: RenderMode) -> Self {
        let camera = scene.camera(0);
        Self { ui, scene, camera, mode, speed: 20.0, frame: 0, last: Instant::now(), fps_window: (Instant::now(), 0) }
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
                self.mode = if self.mode == RenderMode::Unlit { RenderMode::Lit } else { RenderMode::Unlit };
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
        Ok(Some(FrameInput { frame: self.frame, camera: self.camera, mode: self.mode, events, render: true, recorded: false }))
    }
}

struct InFlight {
    id: u64,
    frame: u32,
    mode: RenderMode,
    sent: Instant,
    recorded: bool,
}

pub struct Lane {
    pub worker: Worker,
    pub streamer: Streamer,
    pub report: PassReport,
    pub ui_index: usize,
    width: u32,
    height: u32,
    in_flight: VecDeque<InFlight>,
    next_id: u64,
    completions: Vec<f64>,
    last_ui: Option<Instant>,
}

impl Lane {
    pub fn new(worker: Worker, streamer: Streamer, report: PassReport, ui_index: usize, width: u32, height: u32) -> Self {
        Self { worker, streamer, report, ui_index, width, height, in_flight: VecDeque::new(), next_id: 0, completions: vec![], last_ui: None }
    }
}

/// Where finished frames go.
pub struct Outputs {
    pub ui: Option<UiHandle>,
    pub readback: Readback,
    pub save: Option<(SaveSpec, PathBuf)>,
    pub compare_every: Option<u32>,
    /// Reference images by (mode, frame).
    pub references: HashMap<(RenderMode, u32), Arc<Vec<u8>>>,
    /// When set, sampled frames are stored as references instead of compared.
    pub recording_reference: bool,
    pub frames: u32,
    writer: Option<crossbeam_channel::Sender<(PathBuf, u32, u32, Vec<u8>)>>,
}

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
                        if let Err(e) = image::save_buffer(&path, &px, w, h, image::ExtendedColorType::Rgba8) {
                            tracing::error!("saving {}: {e}", path.display());
                        }
                    }
                });
            }
            tx
        });
        Self { ui, readback, save, compare_every, references: HashMap::new(), recording_reference: false, frames, writer }
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

    fn deliver(&mut self, lane: &mut Lane, frame: u32, mode: RenderMode, recorded: bool, px: &[u8]) -> Option<Quality> {
        let (w, h) = (lane.width, lane.height);
        if let Some(ui) = &self.ui {
            // Throttle what goes to the window to ~60 Hz per lane.
            if lane.last_ui.is_none_or(|t| t.elapsed() > Duration::from_millis(15)) {
                lane.last_ui = Some(Instant::now());
                ui.send(UiEvent::Frame { lane: lane.ui_index, width: w, height: h, pixels: Arc::new(px.to_vec()) });
            }
        }
        if !recorded {
            return None;
        }
        let tag = format!("{}-{}", sanitize(&lane.report.label), mode.name());
        let mut quality = None;
        if self.compare_hit(frame) {
            if self.recording_reference {
                self.references.insert((mode, frame), Arc::new(px.to_vec()));
            } else if let Some(r) = self.references.get(&(mode, frame)) {
                if r.len() == px.len() {
                    quality = Some(compare::compare(r, px, w));
                    if let (true, Some((s, dir)), Some(tx)) = (self.save_hit(frame), &self.save, &self.writer) {
                        if s.diffs {
                            let _ = tx.send((dir.join(format!("{tag}/{frame:06}-diff.png")), w, h, compare::diff_image(r, px)));
                        }
                    }
                }
            }
        }
        if self.save_hit(frame) {
            if let (Some((_, dir)), Some(tx)) = (&self.save, &self.writer) {
                let _ = tx.send((dir.join(format!("{tag}/{frame:06}.png")), w, h, px.to_vec()));
            }
        }
        quality
    }
}

pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

const BATCH: usize = 256;

fn send_updates(w: &Worker, mut ups: Vec<WorldUpdate>) -> anyhow::Result<()> {
    while !ups.is_empty() {
        let rest = if ups.len() > BATCH { ups.split_off(BATCH) } else { vec![] };
        w.send(HostMsg::Updates(std::mem::replace(&mut ups, rest)))?;
    }
    Ok(())
}

fn on_msg(lane: &mut Lane, out: &mut Outputs, start: Instant, m: WorkerMsg) -> anyhow::Result<()> {
    let WorkerMsg::FrameDone { frame_id, stats, slot, update_ms } = m else {
        bail!("[{}] unexpected message {m:?}", lane.report.label);
    };
    let Some(f) = lane.in_flight.pop_front() else { bail!("[{}] completed a frame never requested", lane.report.label) };
    if f.id != frame_id {
        bail!("[{}] frames completed out of order ({} before {})", lane.report.label, frame_id, f.id);
    }
    let latency_ms = f.sent.elapsed().as_secs_f64() * 1e3;
    let quality = match slot {
        Some(s) => {
            let n = (lane.width * lane.height * 4) as usize;
            let px = lane.worker.shm.slot(s)[..n].to_vec();
            lane.worker.free_slot(s);
            out.deliver(lane, f.frame, f.mode, f.recorded, &px)
        }
        None => None,
    };
    if f.recorded {
        lane.completions.push(start.elapsed().as_secs_f64() * 1e3);
        lane.report.per_frame.push(FrameRecord { frame: f.frame, latency_ms, cpu_ms: stats.cpu_ms, gpu_ms: stats.gpu_ms, update_ms, counters: stats.counters, quality });
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
    lights: &[Light],
    run: &RunSpec,
    out: &mut Outputs,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let mut first = true;
    let mut rendered = 0u64;
    let mut pace_start: Option<Instant> = None;
    let mut measure_start: Option<Instant> = None;
    let mut missed = 0u32;
    while let Some(input) = driver.next()? {
        let edits = resolve_edits(world, input.events.edits);
        let changed = world.apply_edits(&edits);
        for lane in lanes.iter_mut() {
            let mut ups = vec![];
            if first {
                ups.push(WorldUpdate::LightUpsert(lights.to_vec()));
            }
            lane.streamer.edits(&edits, &changed, &mut ups);
            lane.streamer.update(world, input.camera.position, &mut ups);
            if !input.events.lights.is_empty() {
                ups.push(WorldUpdate::LightUpsert(input.events.lights.clone()));
            }
            if let Some(env) = input.events.environment {
                ups.push(WorldUpdate::SetEnvironment(env));
            }
            if first {
                let t = Instant::now();
                send_updates(&lane.worker, ups)?;
                lane.worker.flush(|m| bail!("unexpected {m:?} during initial load"))?;
                lane.report.initial_load_ms = t.elapsed().as_secs_f64() * 1e3;
            } else if !ups.is_empty() {
                send_updates(&lane.worker, ups)?;
            }
        }
        first = false;
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
            let req = FrameRequest { frame_id: id, camera: input.camera, mode: input.mode, readback: slot.is_some() };
            lane.in_flight.push_back(InFlight { id, frame: input.frame, mode: input.mode, sent: Instant::now(), recorded: input.recorded });
            lane.worker.send(HostMsg::Render { req, slot })?;
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
        lane.report.chunks_sent = lane.streamer.chunks_sent;
        lane.report.peak_rss_mb = lane.worker.peak_rss().map(|b| b as f64 / 1048576.0);
        lane.worker.send(HostMsg::QueryMemory)?;
        if let Ok(WorkerMsg::Memory(m)) = recv_timeout(&lane.worker.rx, Duration::from_secs(30)) {
            lane.report.memory = m;
        }
        let completions = std::mem::take(&mut lane.completions);
        lane.report.finish(&completions);
    }
    Ok(())
}
