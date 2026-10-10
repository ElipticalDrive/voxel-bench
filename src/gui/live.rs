//! Live session: renderer lanes running continuously behind the GUI.
//!
//! * Lanes are not in lockstep: each renders the latest camera as fast as it can (or up to
//!   the fps cap) with its own queue of pending world updates.
//! * Frames are only read back at the display rate, so metrics reflect the renderer.
//! * Workers spawn asynchronously. Restarting a renderer (new options, restart-only knobs)
//!   is a *hot swap*: the replacement loads and warms up in the background while the old
//!   one keeps rendering, then takes over in a single step.
//! * Knobs (renderer parameters plus `host.*` ones like render scale) change live.
//! * Quality probes periodically render the current view on the reference renderer too
//!   and score every lane against it, without pausing anything.
//! * Each lane can run an auto-tuner ([`crate::tune`]) that turns its knobs towards a goal.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use glam::{DVec3, Vec3};
use mr_core::renderer::BlockEdit;
use mr_core::{BlockId, BlockRegistry, Camera, Environment, FrameRequest, Light, MeshData, MeshInstance, MeshUpdate, ParamSpec, ParamValue, PassMode, RendererInfo, WorldUpdate};
use mr_ipc::{HostMsg, WorkerMsg};
use mr_world::scenario::resolve_edits;
use mr_world::textures::default_registry;
use mr_world::{Scene, World};

use crate::compare::{self, Quality};
use crate::plan::{Plan, RendererSpec, RunSpec, Streaming};
use crate::stream::Streamer;
use crate::tune::{self, Action, HistoryPoint, Tuner, TunerConfig};
use crate::worker::{self, Worker, WorkerEvent};

pub const MAX_SAMPLES: usize = 4000;
const USER_LIGHT_BASE: u32 = 1 << 30;
const SLOTS: u32 = 10;
const REFERENCE: usize = usize::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraControl {
    Free(Camera),
    Path,
}

/// Written by the GUI every frame, read by the session every tick.
#[derive(Clone, Copy, Debug)]
pub struct Control {
    pub camera: CameraControl,
    pub mode: PassMode,
    pub scene_frame: u32,
    pub events: bool,
    /// Per-lane frame-rate cap; 0 = uncapped.
    pub fps_cap: f64,
    pub display_hz: f64,
    /// Seconds between automatic quality probes; 0 = only on demand.
    pub probe_interval: f64,
}

pub enum Cmd {
    Edit { origin: DVec3, dir: Vec3, place: Option<BlockId> },
    AddLight { position: DVec3, color: Vec3, intensity: f32, radius: f32 },
    ClearUserLights,
    /// Change one knob (renderer parameter or `host.*`). Restart-only ones hot-swap.
    SetParam { lane: usize, name: String, value: ParamValue },
    /// Hot-swap a lane to a new renderer spec.
    Restart { lane: usize, spec: RendererSpec },
    AddLane(RendererSpec),
    RemoveLane(usize),
    /// Quality probe that also keeps the images (for the Compare tab).
    Compare,
    Tune { lane: usize, config: Option<TunerConfig> },
    Stop,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub t: f64,
    pub interval_ms: f32,
    pub latency_ms: f32,
    pub cpu_ms: f32,
    pub gpu_ms: Option<f32>,
    pub update_ms: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct QualitySample {
    pub t: f64,
    pub psnr: f64,
    pub ssim_db: f64,
    pub ms_ssim_db: f64,
    pub bad_pct: f64,
}

#[derive(Clone)]
pub struct LiveFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<u8>>,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LaneStatus {
    Starting,
    Running,
    Failed(String),
}

#[derive(Clone)]
pub struct TunerView {
    pub config: TunerConfig,
    pub status: String,
    pub log: Vec<String>,
    pub history: Vec<HistoryPoint>,
    pub trials: u32,
    pub accepted: u32,
    /// Best result so far (score, fps, quality).
    pub best: Option<String>,
    /// Knobs whose best value differs from the value when tuning started: (name, start, best).
    pub best_diff: Vec<(String, String, String)>,
    /// Settled on the best configuration and only watching it.
    pub holding: bool,
}

pub struct LaneView {
    pub spec: RendererSpec,
    pub status: LaneStatus,
    pub frame: Option<LiveFrame>,
    pub samples: VecDeque<Sample>,
    pub counters: Vec<(String, f64)>,
    pub rss_mb: Option<f64>,
    pub gpu_mb: Option<f64>,
    pub init_ms: f32,
    pub frames: u64,
    pub chunks_sent: u64,
    pub pending_chunks: usize,
    /// Knobs (host first, then renderer) with current values.
    pub params: Vec<(ParamSpec, ParamValue)>,
    pub quality: VecDeque<QualitySample>,
    /// Hot-swap progress, when one is underway.
    pub swap: Option<String>,
    /// Timeline markers (swaps, resizes) for the plots.
    pub events: Vec<(f64, String)>,
    pub tuner: Option<TunerView>,
    pub resolution: [u32; 2],
}

impl LaneView {
    fn new(spec: RendererSpec) -> Self {
        Self {
            spec,
            status: LaneStatus::Starting,
            frame: None,
            samples: VecDeque::new(),
            counters: vec![],
            rss_mb: None,
            gpu_mb: None,
            init_ms: 0.0,
            frames: 0,
            chunks_sent: 0,
            pending_chunks: 0,
            params: vec![],
            quality: VecDeque::new(),
            swap: None,
            events: vec![],
            tuner: None,
            resolution: [0, 0],
        }
    }
}

pub struct CompareLane {
    pub label: String,
    pub quality: Quality,
    pub image: Arc<Vec<u8>>,
    pub diff: Arc<Vec<u8>>,
}

pub struct CompareResult {
    pub seq: u64,
    pub mode: PassMode,
    pub camera: Camera,
    pub width: u32,
    pub height: u32,
    pub reference_label: String,
    pub reference: Arc<Vec<u8>>,
    pub lanes: Vec<CompareLane>,
}

#[derive(Default)]
pub struct Shared {
    pub lanes: Vec<LaneView>,
    pub camera: Option<Camera>,
    pub status: String,
    pub log: Vec<String>,
    pub compare: Option<CompareResult>,
    pub comparing: bool,
    pub lights: usize,
    pub cached_chunks: usize,
    pub finished: bool,
    pub reference: String,
}

impl Shared {
    fn log(&mut self, s: impl Into<String>) {
        let s = s.into();
        tracing::info!("{s}");
        self.log.push(s);
        if self.log.len() > 500 {
            self.log.drain(..100);
        }
    }
}

pub struct LiveHandle {
    pub control: Arc<Mutex<Control>>,
    pub shared: Arc<Mutex<Shared>>,
    cmd: Sender<Cmd>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LiveHandle {
    pub fn start(plan: Plan, control: Control, repaint: impl Fn() + Send + 'static) -> Self {
        let control = Arc::new(Mutex::new(control));
        let shared = Arc::new(Mutex::new(Shared { status: "starting".into(), ..Default::default() }));
        let (cmd, rx) = crossbeam_channel::unbounded();
        let (c, s) = (control.clone(), shared.clone());
        let thread = std::thread::Builder::new()
            .name("live-session".into())
            .spawn(move || {
                let r = Session::run(plan, c, s.clone(), rx, Box::new(repaint));
                let mut sh = s.lock().unwrap();
                match r {
                    Err(e) => {
                        sh.log(format!("session failed: {e:#}"));
                        sh.status = format!("failed: {e:#}");
                    }
                    Ok(()) => sh.status = "stopped".into(),
                }
                sh.finished = true;
            })
            .expect("spawn live session");
        Self { control, shared, cmd, thread: Some(thread) }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.cmd.send(c);
    }
}

impl Drop for LiveHandle {
    fn drop(&mut self) {
        let _ = self.cmd.send(Cmd::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ------------------------------------------------------------------------- internals

#[derive(Clone, Copy, PartialEq)]
enum Purpose {
    Live,
    Probe(u64),
    Warm,
}

struct Flight {
    id: u64,
    sent: Instant,
    purpose: Purpose,
}

type Spawn = Receiver<anyhow::Result<Worker>>;

/// One worker plus everything needed to keep it in sync with the world.
struct Runner {
    spec: RendererSpec,
    worker: Option<Worker>,
    spawning: Option<Spawn>,
    streamer: Streamer,
    pending: Vec<WorldUpdate>,
    /// Mesh mode: geometry and instance updates not yet sent.
    pending_meshes: Vec<MeshUpdate>,
    pending_params: Vec<(String, ParamValue)>,
    in_flight: VecDeque<Flight>,
    next_id: u64,
    res: [u32; 2],
}

impl Runner {
    fn submit(&mut self, camera: Camera, mode: PassMode, readback: bool, purpose: Purpose) -> anyhow::Result<bool> {
        let Some(w) = self.worker.as_mut() else { return Ok(false) };
        let slot = if readback {
            match w.take_slot() {
                Some(s) => Some(s),
                None if purpose != Purpose::Live => return Ok(false),
                None => None,
            }
        } else {
            None
        };
        let mut ups = std::mem::take(&mut self.pending);
        while !ups.is_empty() {
            let rest = if ups.len() > 256 { ups.split_off(256) } else { vec![] };
            w.send(HostMsg::Updates(std::mem::replace(&mut ups, rest)))?;
        }
        for m in std::mem::take(&mut self.pending_meshes) {
            w.send(HostMsg::Meshes(m))?;
        }
        if !self.pending_params.is_empty() {
            w.send(HostMsg::SetParams(std::mem::take(&mut self.pending_params)))?;
        }
        let id = self.next_id;
        self.next_id += 1;
        w.send(HostMsg::Render { req: FrameRequest { frame_id: id, camera, mode: mode.lighting, meshes: mode.meshes, readback: slot.is_some() }, slot })?;
        self.in_flight.push_back(Flight { id, sent: Instant::now(), purpose });
        Ok(true)
    }
}

struct Lane {
    run: Runner,
    info: Option<RendererInfo>,
    /// Replacement being loaded for a hot swap.
    standby: Option<Runner>,
    standby_warm: u32,
    values: BTreeMap<String, ParamValue>,
    want_res: Option<[u32; 2]>,
    last_done: Option<Instant>,
    last_sent: Option<Instant>,
    last_display: Option<Instant>,
    last_mem: Instant,
    seq: u64,
    tuner: Option<Tuner>,
}

struct Probe {
    id: u64,
    camera: Camera,
    mode: PassMode,
    keep_images: bool,
    started: bool,
    reference: Option<Arc<Vec<u8>>>,
    lanes: HashMap<usize, ([u32; 2], Arc<Vec<u8>>)>,
    waiting: usize,
}

struct Session {
    plan: Plan,
    blocks: Arc<BlockRegistry>,
    world: World,
    scene: Scene,
    env: Environment,
    lights: HashMap<u32, Light>,
    /// Mesh mode: the scene's mesh geometry and the instances last sent.
    mesh_data: Vec<Arc<MeshData>>,
    instances: Vec<MeshInstance>,
    lanes: Vec<Lane>,
    reference: Option<Lane>,
    shared: Arc<Mutex<Shared>>,
    repaint: Box<dyn Fn() + Send>,
    start: Instant,
    applied_frame: Option<u32>,
    next_light: u32,
    probe: Option<Probe>,
    probe_seq: u64,
    last_probe: Instant,
    compare_seq: u64,
}

fn spawn_async(run: &RunSpec, spec: &RendererSpec, blocks: Arc<BlockRegistry>, env: Environment, res: [u32; 2]) -> Spawn {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let (run, spec) = (run.clone(), spec.clone());
    std::thread::spawn(move || {
        let _ = tx.send(worker::spawn_renderer(&run, &spec, blocks, env, None, res, SLOTS));
    });
    rx
}

fn budget(plan: &Plan) -> Option<usize> {
    Some(match plan.run.streaming {
        Streaming::Complete => 2048,
        Streaming::Budget { budget } => budget,
    })
}

impl Session {
    fn run(plan: Plan, control: Arc<Mutex<Control>>, shared: Arc<Mutex<Shared>>, rx: Receiver<Cmd>, repaint: Box<dyn Fn() + Send>) -> anyhow::Result<()> {
        shared.lock().unwrap().status = "generating world".into();
        let blocks = Arc::new(default_registry());
        let mut world = World::new(plan.scene.generator.build());
        let scene = Scene::new(plan.scene.clone(), blocks.clone(), &mut world, plan.run.frames)?;
        let lights = scene.initial_lights().iter().map(|l| (l.id, *l)).collect();
        let env = scene.environment;
        let mesh_data = scene.mesh_data();
        let instances = scene.mesh_instances(0);
        let mut s = Session {
            blocks,
            world,
            scene,
            env,
            lights,
            mesh_data,
            instances,
            lanes: vec![],
            reference: None,
            shared,
            repaint,
            start: Instant::now(),
            applied_frame: None,
            next_light: USER_LIGHT_BASE,
            probe: None,
            probe_seq: 0,
            last_probe: Instant::now(),
            compare_seq: 0,
            plan,
        };
        s.shared.lock().unwrap().status = "running".into();
        for spec in s.plan.renderers.clone() {
            s.add_lane(spec);
        }
        loop {
            let mut busy = false;
            while let Ok(cmd) = rx.try_recv() {
                busy = true;
                if let Cmd::Stop = cmd {
                    s.shutdown();
                    return Ok(());
                }
                s.command(cmd);
            }
            let ctl = *control.lock().unwrap();
            busy |= s.tick(&ctl)?;
            if !busy {
                std::thread::sleep(Duration::from_micros(250));
            }
        }
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn new_runner(&self, spec: RendererSpec, res: [u32; 2], radius: u32) -> Runner {
        Runner {
            spawning: Some(spawn_async(&self.plan.run, &spec, self.blocks.clone(), self.env, res)),
            spec,
            worker: None,
            streamer: Streamer::new(radius, 0, budget(&self.plan)),
            pending: vec![WorldUpdate::LightUpsert(self.lights.values().copied().collect()), WorldUpdate::SetEnvironment(self.env)],
            pending_meshes: if self.mesh_data.is_empty() {
                vec![]
            } else {
                vec![MeshUpdate::Meshes(self.mesh_data.clone()), MeshUpdate::Instances(self.instances.clone())]
            },
            pending_params: vec![],
            in_flight: VecDeque::new(),
            next_id: 0,
            res,
        }
    }

    fn new_lane(&self, spec: RendererSpec) -> Lane {
        let radius = spec.view_radius(&self.plan.run);
        let mut values = BTreeMap::new();
        for p in tune::host_params(radius) {
            values.insert(p.name.clone(), p.default.clone());
        }
        values.insert("host.in_flight".into(), ParamValue::Int(self.plan.run.in_flight.clamp(1, 8) as i64));
        Lane {
            run: self.new_runner(spec, self.plan.run.resolution, radius),
            info: None,
            standby: None,
            standby_warm: 0,
            values,
            want_res: None,
            last_done: None,
            last_sent: None,
            last_display: None,
            last_mem: Instant::now(),
            seq: 0,
            tuner: None,
        }
    }

    fn add_lane(&mut self, spec: RendererSpec) {
        let lane = self.new_lane(spec.clone());
        self.lanes.push(lane);
        self.shared.lock().unwrap().lanes.push(LaneView::new(spec));
        self.publish_params(self.lanes.len() - 1);
        (self.repaint)();
    }

    fn lane_mut(&mut self, i: usize) -> Option<&mut Lane> {
        if i == REFERENCE { self.reference.as_mut() } else { self.lanes.get_mut(i) }
    }

    fn log(&self, s: impl Into<String>) {
        self.shared.lock().unwrap().log(s);
    }

    /// All knobs of a lane: host ones, then the renderer's.
    fn specs(&self, i: usize) -> Vec<ParamSpec> {
        let mut v = tune::host_params(self.plan.run.view_radius);
        if let Some(info) = self.lanes.get(i).and_then(|l| l.info.as_ref()) {
            if info.max_lod > 0 {
                v.push(ParamSpec::int("host.max_lod", "LOD levels streamed beyond full resolution (restart)", 0, info.max_lod as i64, 0, false));
            }
            v.extend(info.params.iter().cloned());
        }
        v
    }

    fn publish_params(&self, i: usize) {
        let Some(lane) = self.lanes.get(i) else { return };
        let params = self.specs(i).into_iter().map(|s| (s.clone(), lane.values.get(&s.name).cloned().unwrap_or(s.default))).collect();
        let mut sh = self.shared.lock().unwrap();
        if let Some(v) = sh.lanes.get_mut(i) {
            v.params = params;
            v.spec = lane.run.spec.clone();
            v.resolution = lane.run.res;
        }
    }

    fn event(&self, i: usize, what: String) {
        let t = self.now();
        let mut sh = self.shared.lock().unwrap();
        if let Some(v) = sh.lanes.get_mut(i) {
            v.events.push((t, what.clone()));
        }
        let label = sh.lanes.get(i).map(|v| v.spec.label().to_string()).unwrap_or_default();
        sh.log(format!("{label}: {what}"));
    }

    fn fan_out(&mut self, u: WorldUpdate) {
        for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
            l.run.pending.push(u.clone());
            if let Some(s) = &mut l.standby {
                s.pending.push(u.clone());
            }
        }
    }

    fn apply_edits(&mut self, edits: Vec<BlockEdit>) {
        let edits = resolve_edits(&mut self.world, edits);
        if edits.is_empty() {
            return;
        }
        let changed = self.world.apply_edits(&edits);
        for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
            for r in std::iter::once(&mut l.run).chain(l.standby.as_mut()) {
                let mut ups = std::mem::take(&mut r.pending);
                r.streamer.edits(&edits, &changed, &mut ups);
                r.pending = ups;
            }
        }
    }

    /// Changes one knob of lane `i`.
    fn set_param(&mut self, i: usize, name: &str, value: ParamValue) {
        let Some(spec) = self.specs(i).into_iter().find(|s| s.name == name) else {
            self.log(format!("lane {i}: unknown knob `{name}`"));
            return;
        };
        let Some(value) = spec.clamp(&value) else { return };
        let base = self.plan.run.resolution;
        let Some(lane) = self.lanes.get_mut(i) else { return };
        if lane.values.get(name) == Some(&value) {
            return;
        }
        lane.values.insert(name.to_string(), value.clone());
        match name {
            "host.render_scale" => {
                let s = value.as_f64().unwrap_or(1.0);
                let r = [((base[0] as f64 * s).round() as u32).max(16), ((base[1] as f64 * s).round() as u32).max(16)];
                if r != lane.run.res {
                    lane.want_res = Some(r);
                }
            }
            "host.view_radius" => {
                let r = value.as_i64().unwrap_or(8) as u32;
                lane.run.streamer.set_radius(r);
                if let Some(s) = &mut lane.standby {
                    s.streamer.set_radius(r);
                }
            }
            "host.in_flight" => {}
            "host.max_lod" => {
                let mut new = lane.run.spec.clone();
                new.max_lod = Some(value.as_i64().unwrap_or(0) as u8);
                self.hot_swap(i, new);
            }
            _ if spec.live => {
                lane.run.spec.options.insert(name.into(), crate::gui::param_to_toml(&value));
                lane.run.pending_params.push((name.to_string(), value));
            }
            _ => {
                let mut new = lane.run.spec.clone();
                new.options.insert(name.into(), crate::gui::param_to_toml(&value));
                self.hot_swap(i, new);
            }
        }
        self.publish_params(i);
    }

    fn hot_swap(&mut self, i: usize, spec: RendererSpec) {
        let radius = self.lanes[i].run.streamer.radius();
        let res = self.lanes[i].run.res;
        let standby = self.new_runner(spec.clone(), res, radius);
        let lane = &mut self.lanes[i];
        if let Some(old) = lane.standby.take().and_then(|s| s.worker) {
            std::thread::spawn(move || old.shutdown());
        }
        lane.standby = Some(standby);
        lane.standby_warm = 0;
        self.shared.lock().unwrap().lanes[i].swap = Some("spawning".into());
        self.event(i, format!("hot swap → {} {}", spec.name, toml::to_string(&spec.options).unwrap_or_default().replace('\n', " ")));
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Edit { origin, dir, place } => {
                if let Some((hit, normal)) = self.world.raycast(origin, dir.as_dvec3(), 512.0) {
                    let edit = match place {
                        Some(b) => BlockEdit { pos: hit + normal, block: b },
                        None => BlockEdit { pos: hit, block: BlockId::AIR },
                    };
                    self.apply_edits(vec![edit]);
                }
            }
            Cmd::AddLight { position, color, intensity, radius } => {
                let l = Light { id: self.next_light, position, color, intensity, radius };
                self.next_light += 1;
                self.lights.insert(l.id, l);
                self.fan_out(WorldUpdate::LightUpsert(vec![l]));
                self.log(format!("light {} at ({:.1}, {:.1}, {:.1})", l.id - USER_LIGHT_BASE, position.x, position.y, position.z));
            }
            Cmd::ClearUserLights => {
                let ids: Vec<u32> = self.lights.keys().copied().filter(|&id| id >= USER_LIGHT_BASE).collect();
                for id in &ids {
                    self.lights.remove(id);
                }
                self.fan_out(WorldUpdate::LightRemove(ids));
            }
            Cmd::SetParam { lane, name, value } => self.set_param(lane, &name, value),
            Cmd::Restart { lane, spec } => {
                if lane < self.lanes.len() {
                    self.hot_swap(lane, spec);
                }
            }
            Cmd::AddLane(spec) => self.add_lane(spec),
            Cmd::RemoveLane(i) => {
                if i < self.lanes.len() {
                    let lane = self.lanes.remove(i);
                    for w in [lane.run.worker, lane.standby.and_then(|s| s.worker)].into_iter().flatten() {
                        std::thread::spawn(move || w.shutdown());
                    }
                    let mut sh = self.shared.lock().unwrap();
                    sh.lanes.remove(i);
                    sh.comparing = false;
                    self.probe = None;
                }
            }
            Cmd::Compare => self.request_probe(true),
            Cmd::Tune { lane, config } => {
                let now = self.now();
                let specs = self.specs(lane);
                let Some(l) = self.lanes.get_mut(lane) else { return };
                // Stopping (or restarting) restores the best configuration found.
                let was_tuning = l.tuner.is_some();
                let best: Vec<(String, ParamValue)> = l
                    .tuner
                    .as_ref()
                    .and_then(|t| t.best_values())
                    .map(|b| b.iter().filter(|(k, v)| l.values.get(*k) != Some(v)).map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                l.tuner = None;
                if was_tuning {
                    let list: Vec<String> = best.iter().map(|(k, v)| format!("{k}={}", v.display())).collect();
                    self.event(lane, format!("tuner stopped; restored best: {}", if list.is_empty() { "(already applied)".into() } else { list.join(", ") }));
                }
                for (name, value) in best {
                    self.set_param(lane, &name, value);
                }
                let l = &mut self.lanes[lane];
                l.tuner = config.map(|c| Tuner::new(c, specs, l.values.clone(), now));
                if l.tuner.is_none() {
                    self.shared.lock().unwrap().lanes[lane].tuner = None;
                }
            }
            Cmd::Stop => {}
        }
    }

    fn request_probe(&mut self, keep_images: bool) {
        if keep_images {
            self.shared.lock().unwrap().comparing = true;
        }
        if let Some(p) = &mut self.probe {
            p.keep_images |= keep_images;
            return;
        }
        self.probe_seq += 1;
        self.probe = Some(Probe {
            id: self.probe_seq,
            camera: Camera::default(),
            mode: PassMode::UNLIT,
            keep_images,
            started: false,
            reference: None,
            lanes: HashMap::new(),
            waiting: 0,
        });
    }

    fn camera(&self, ctl: &Control) -> Camera {
        match ctl.camera {
            CameraControl::Free(c) => c,
            CameraControl::Path => self.scene.camera(ctl.scene_frame),
        }
    }

    fn tick(&mut self, ctl: &Control) -> anyhow::Result<bool> {
        let mut busy = false;
        let now = self.now();
        busy |= self.poll_spawns();

        // Scripted scene time.
        if ctl.events && self.applied_frame != Some(ctl.scene_frame) {
            let from = match self.applied_frame {
                Some(a) if a < ctl.scene_frame => (a + 1).max(ctl.scene_frame.saturating_sub(120)),
                _ => ctl.scene_frame,
            };
            let cam = self.camera(ctl);
            let (mut edits, mut lights, mut env) = (vec![], None, None);
            for f in from..=ctl.scene_frame {
                let ev = self.scene.events(f, &cam)?;
                edits.extend(ev.edits);
                if !ev.lights.is_empty() {
                    lights = Some(ev.lights);
                }
                env = ev.environment.or(env);
            }
            self.applied_frame = Some(ctl.scene_frame);
            self.apply_edits(edits);
            if !self.mesh_data.is_empty() {
                let inst = self.scene.mesh_instances(ctl.scene_frame);
                if inst != self.instances {
                    self.instances = inst.clone();
                    for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
                        l.run.pending_meshes.push(MeshUpdate::Instances(inst.clone()));
                        if let Some(s) = &mut l.standby {
                            s.pending_meshes.push(MeshUpdate::Instances(inst.clone()));
                        }
                    }
                }
            }
            if let Some(ls) = lights {
                for l in &ls {
                    self.lights.insert(l.id, *l);
                }
                self.fan_out(WorldUpdate::LightUpsert(ls));
            }
            if let Some(e) = env {
                self.env = e;
                self.fan_out(WorldUpdate::SetEnvironment(e));
            }
            busy = true;
        }

        let cam = self.camera(ctl);
        {
            let mut sh = self.shared.lock().unwrap();
            sh.camera = Some(cam);
            sh.lights = self.lights.len();
            sh.cached_chunks = self.world.cached_chunks();
        }
        let world = &mut self.world;
        for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
            for r in std::iter::once(&mut l.run).chain(l.standby.as_mut()) {
                if r.worker.is_some() {
                    let mut ups = std::mem::take(&mut r.pending);
                    r.streamer.update(world, cam.position, &mut ups);
                    r.pending = ups;
                }
            }
        }

        let mut failures: Vec<(usize, anyhow::Error)> = vec![];
        busy |= self.progress_swaps(cam, ctl.mode, &mut failures);
        self.apply_resizes(&mut failures);

        // Quality probes.
        if ctl.probe_interval > 0.0 && self.probe.is_none() && self.last_probe.elapsed().as_secs_f64() >= ctl.probe_interval {
            self.request_probe(false);
        }
        if self.probe.as_ref().is_some_and(|p| !p.started) {
            self.start_probe(cam, ctl.mode, &mut failures);
        }

        // Live frames.
        let instant = Instant::now();
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            if lane.run.worker.is_none() || lane.want_res.is_some() {
                continue;
            }
            let in_flight = lane.values.get("host.in_flight").and_then(|v| v.as_i64()).unwrap_or(2).max(1) as usize;
            if lane.run.in_flight.len() >= in_flight {
                continue;
            }
            if ctl.fps_cap > 0.0 && lane.last_sent.is_some_and(|t| instant.duration_since(t).as_secs_f64() < 1.0 / ctl.fps_cap) {
                continue;
            }
            let display = lane.last_display.is_none_or(|t| instant.duration_since(t).as_secs_f64() >= 1.0 / ctl.display_hz.max(1.0));
            match lane.run.submit(cam, ctl.mode, display, Purpose::Live) {
                Ok(true) => {
                    if display {
                        lane.last_display = Some(instant);
                    }
                    lane.last_sent = Some(instant);
                    busy = true;
                }
                Ok(false) => {}
                Err(e) => failures.push((i, e)),
            }
        }

        for i in (0..self.lanes.len()).chain(self.reference.is_some().then_some(REFERENCE)) {
            match self.pump(i) {
                Ok(b) => busy |= b,
                Err(e) => failures.push((i, e)),
            }
        }
        if self.probe.as_ref().is_some_and(|p| p.started && p.waiting == 0) {
            self.finish_probe();
        }
        for (i, e) in failures {
            self.fail(i, e);
        }
        self.run_tuners(now);
        Ok(busy)
    }

    fn poll_spawns(&mut self) -> bool {
        let mut busy = false;
        let n = self.lanes.len();
        for i in (0..n).chain(self.reference.is_some().then_some(REFERENCE)) {
            let result = match self.lane_mut(i).and_then(|l| l.run.spawning.as_ref()).map(|rx| rx.try_recv()) {
                Some(Ok(r)) => r,
                _ => continue,
            };
            busy = true;
            let (run_spec, bud) = (self.plan.run.clone(), budget(&self.plan));
            let lane = self.lane_mut(i).unwrap();
            lane.run.spawning = None;
            match result {
                Ok(w) => {
                    let info = w.info.clone();
                    let init_ms = w.init_ms;
                    // Renderer knob values from its options (else defaults).
                    for p in &info.params {
                        let v = lane.run.spec.options.get(&p.name).and_then(|t| serde_json::to_value(t).ok()).and_then(|j| p.from_json(&j)).unwrap_or(p.default.clone());
                        lane.values.insert(p.name.clone(), v);
                    }
                    // Now that its LOD support is known, stream the levels it can use.
                    let lods = lane.run.spec.max_lod(&run_spec, info.max_lod);
                    if lods > 0 {
                        let mut s = Streamer::new(lane.run.streamer.radius(), lods, bud);
                        if i == REFERENCE {
                            s.set_budget(None);
                        }
                        lane.run.streamer = s;
                    }
                    lane.values.insert("host.max_lod".into(), ParamValue::Int(lods as i64));
                    lane.info = Some(info);
                    lane.run.worker = Some(w);
                    if i != REFERENCE {
                        {
                            let mut sh = self.shared.lock().unwrap();
                            if let Some(v) = sh.lanes.get_mut(i) {
                                v.status = LaneStatus::Running;
                                v.init_ms = init_ms;
                            }
                        }
                        self.publish_params(i);
                        self.event(i, format!("started ({init_ms:.0} ms init)"));
                    } else {
                        self.log("reference renderer started");
                    }
                }
                Err(e) => {
                    if i == REFERENCE {
                        self.log(format!("reference failed to start: {e:#}"));
                        self.reference = None;
                        self.probe = None;
                        self.shared.lock().unwrap().comparing = false;
                    } else {
                        let msg = format!("{e:#}");
                        self.shared.lock().unwrap().lanes[i].status = LaneStatus::Failed(msg.clone());
                        self.log(format!("lane {} failed to start: {msg}", i + 1));
                    }
                }
            }
        }
        let (run_spec, bud) = (self.plan.run.clone(), budget(&self.plan));
        for i in 0..n {
            let lane = &mut self.lanes[i];
            let Some(sb) = &mut lane.standby else { continue };
            let Some(result) = sb.spawning.as_ref().and_then(|rx| rx.try_recv().ok()) else { continue };
            sb.spawning = None;
            busy = true;
            match result {
                Ok(w) => {
                    let lods = sb.spec.max_lod(&run_spec, w.info.max_lod);
                    if lods > 0 {
                        sb.streamer = Streamer::new(sb.streamer.radius(), lods, bud);
                    }
                    sb.worker = Some(w);
                    self.shared.lock().unwrap().lanes[i].swap = Some("loading world".into());
                }
                Err(e) => {
                    lane.standby = None;
                    self.shared.lock().unwrap().lanes[i].swap = None;
                    self.event(i, format!("hot swap failed: {e:#}"));
                }
            }
        }
        busy
    }

    /// Streams to standbys, warms them with unseen frames, and swaps them in when ready.
    fn progress_swaps(&mut self, cam: Camera, mode: PassMode, failures: &mut Vec<(usize, anyhow::Error)>) -> bool {
        let mut busy = false;
        for i in 0..self.lanes.len() {
            let lane = &mut self.lanes[i];
            let Some(sb) = &mut lane.standby else { continue };
            if sb.worker.is_none() {
                continue;
            }
            // Warm-up frames once everything in range has streamed.
            if sb.streamer.pending() == 0 && sb.in_flight.is_empty() && lane.standby_warm < 3 {
                match sb.submit(cam, mode, false, Purpose::Warm) {
                    Ok(_) => busy = true,
                    Err(e) => {
                        failures.push((i, e.context("hot swap")));
                        continue;
                    }
                }
            }
            let mut done = 0;
            if let Some(w) = &sb.worker {
                while let Ok(ev) = w.rx.try_recv() {
                    match ev {
                        WorkerEvent::Msg(WorkerMsg::FrameDone { .. }) => {
                            sb.in_flight.pop_front();
                            done += 1;
                        }
                        WorkerEvent::Msg(WorkerMsg::Error(e)) | WorkerEvent::Msg(WorkerMsg::ParamError { error: e, .. }) => {
                            failures.push((i, anyhow::anyhow!("hot swap: {e}")));
                        }
                        WorkerEvent::Closed => failures.push((i, anyhow::anyhow!("hot swap: replacement exited"))),
                        _ => {}
                    }
                }
            }
            lane.standby_warm += done;
            let progress = if sb.streamer.pending() > 0 { format!("loading world ({} chunks left)", sb.streamer.pending()) } else { format!("warming up ({}/3)", lane.standby_warm) };
            if lane.standby_warm < 3 {
                self.shared.lock().unwrap().lanes[i].swap = Some(progress);
                continue;
            }
            // Swap: the replacement takes over; the old worker shuts down in the background.
            let sb = lane.standby.take().unwrap();
            if let Some(old) = std::mem::replace(&mut lane.run, sb).worker {
                std::thread::spawn(move || old.shutdown());
            }
            let info = lane.run.worker.as_ref().map(|w| w.info.clone());
            if let Some(info) = info {
                for p in &info.params {
                    if let Some(v) = lane.run.spec.options.get(&p.name).and_then(|t| serde_json::to_value(t).ok()).and_then(|j| p.from_json(&j)) {
                        lane.values.insert(p.name.clone(), v);
                    }
                }
                lane.info = Some(info);
            }
            lane.last_done = None;
            if let Some(t) = &mut lane.tuner {
                t.hold(self.start.elapsed().as_secs_f64());
            }
            self.shared.lock().unwrap().lanes[i].swap = None;
            self.publish_params(i);
            self.event(i, "hot swap complete".into());
            busy = true;
        }
        busy
    }

    fn apply_resizes(&mut self, failures: &mut Vec<(usize, anyhow::Error)>) {
        for i in 0..self.lanes.len() {
            let lane = &mut self.lanes[i];
            let Some(r) = lane.want_res else { continue };
            if !lane.run.in_flight.is_empty() || lane.run.worker.is_none() {
                continue;
            }
            lane.want_res = None;
            if let Err(e) = lane.run.worker.as_mut().unwrap().resize(r[0], r[1], SLOTS) {
                failures.push((i, e.context("resize")));
                continue;
            }
            lane.run.res = r;
            lane.last_done = None;
            if let Some(t) = &mut lane.tuner {
                t.hold(self.start.elapsed().as_secs_f64());
            }
            self.publish_params(i);
            self.event(i, format!("render resolution {}×{}", r[0], r[1]));
        }
    }

    fn start_probe(&mut self, cam: Camera, mode: PassMode, failures: &mut Vec<(usize, anyhow::Error)>) {
        if self.reference.is_none() {
            let name = self.plan.output.compare.as_ref().map(|c| c.reference.clone()).unwrap_or_else(|| "cpu-reference".into());
            let options = self.plan.output.compare.as_ref().map(|c| c.reference_options.clone()).unwrap_or_default();
            let spec = RendererSpec { label: Some(format!("{name} (ref)")), options, ..RendererSpec::new(name) };
            self.shared.lock().unwrap().reference = spec.label().to_string();
            self.log(format!("starting reference {}", spec.label()));
            let mut r = self.new_lane(spec);
            r.run.streamer.set_budget(None);
            self.reference = Some(r);
            return;
        }
        let Some(reference) = &mut self.reference else { return };
        if reference.run.worker.is_none() {
            return; // still spawning
        }
        // The reference must see the whole streamed region.
        let mut ups = std::mem::take(&mut reference.run.pending);
        loop {
            reference.run.streamer.update(&mut self.world, cam.position, &mut ups);
            if reference.run.streamer.pending() == 0 {
                break;
            }
        }
        reference.run.pending = ups;
        let probe = self.probe.as_mut().unwrap();
        probe.started = true;
        probe.camera = cam;
        probe.mode = mode;
        let id = probe.id;
        match reference.run.submit(cam, mode, true, Purpose::Probe(id)) {
            Ok(true) => probe.waiting += 1,
            Ok(false) => {}
            Err(e) => failures.push((REFERENCE, e)),
        }
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            if lane.run.worker.is_none() || lane.want_res.is_some() {
                continue;
            }
            match lane.run.submit(cam, mode, true, Purpose::Probe(id)) {
                Ok(true) => probe.waiting += 1,
                Ok(false) => {}
                Err(e) => failures.push((i, e)),
            }
        }
        self.last_probe = Instant::now();
    }

    fn finish_probe(&mut self) {
        let Some(p) = self.probe.take() else { return };
        let t = self.now();
        let mut sh = self.shared.lock().unwrap();
        sh.comparing = false;
        let Some(reference) = p.reference else {
            sh.log("probe failed: no reference image");
            return;
        };
        let ref_res = self.reference.as_ref().map(|r| r.run.res).unwrap_or(self.plan.run.resolution);
        let mut kept = vec![];
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            let Some((res, px)) = p.lanes.get(&i) else { continue };
            // Bilinear, centre-aligned: what a display path does with a lower-resolution frame.
            let px = if *res != ref_res { Arc::new(compare::resample(px, *res, ref_res, true)) } else { px.clone() };
            let q = compare::compare(&reference, &px, ref_res[0]);
            if let Some(v) = sh.lanes.get_mut(i) {
                v.quality.push_back(QualitySample { t, psnr: q.psnr.min(99.0), ssim_db: q.ssim_db, ms_ssim_db: q.ms_ssim_db, bad_pct: q.bad_pct });
                if v.quality.len() > 2000 {
                    v.quality.pop_front();
                }
            }
            if let Some(tu) = &mut lane.tuner {
                tu.on_probe(t, q.psnr.min(99.0), q.ms_ssim_db);
            }
            if p.keep_images {
                kept.push(CompareLane { label: lane.run.spec.label().to_string(), quality: q, diff: Arc::new(compare::diff_image(&reference, &px)), image: px });
            }
        }
        if p.keep_images {
            self.compare_seq += 1;
            let summary: Vec<String> = kept.iter().map(|l| format!("{} {:.1} dB / {:.2}% bad", l.label, l.quality.psnr.min(99.0), l.quality.bad_pct)).collect();
            sh.log(format!("compare [{}]: {}", p.mode.name(), summary.join(", ")));
            let reference_label = sh.reference.clone();
            sh.compare = Some(CompareResult {
                seq: self.compare_seq,
                mode: p.mode,
                camera: p.camera,
                width: ref_res[0],
                height: ref_res[1],
                reference_label,
                reference,
                lanes: kept,
            });
        }
        drop(sh);
        (self.repaint)();
    }

    fn pump(&mut self, i: usize) -> anyhow::Result<bool> {
        let start = self.start;
        let shared = self.shared.clone();
        let Some(lane) = self.lane_mut(i) else { return Ok(false) };
        let Some(w) = &mut lane.run.worker else { return Ok(false) };
        let mut busy = false;
        let mut repaint = false;
        let mut probe_px: Vec<(u64, [u32; 2], Arc<Vec<u8>>)> = vec![];
        if lane.last_mem.elapsed() > Duration::from_secs(1) && i != REFERENCE {
            lane.last_mem = Instant::now();
            w.send(HostMsg::QueryMemory)?;
            let rss = w.rss().map(|b| b as f64 / 1048576.0);
            let mut sh = shared.lock().unwrap();
            if let Some(v) = sh.lanes.get_mut(i) {
                v.rss_mb = rss;
                v.chunks_sent = lane.run.streamer.chunks_sent;
                v.pending_chunks = lane.run.streamer.pending();
            }
        }
        while let Ok(ev) = w.rx.try_recv() {
            busy = true;
            let msg = match ev {
                WorkerEvent::Msg(WorkerMsg::Error(e)) => anyhow::bail!("worker error: {e}"),
                WorkerEvent::Closed => anyhow::bail!("worker exited"),
                WorkerEvent::Msg(m) => m,
            };
            match msg {
                WorkerMsg::Memory(m) => {
                    if let Some(v) = shared.lock().unwrap().lanes.get_mut(i) {
                        v.gpu_mb = (m.gpu_bytes > 0).then(|| m.gpu_bytes as f64 / 1048576.0);
                    }
                }
                WorkerMsg::ParamError { name, error } => shared.lock().unwrap().log(format!("lane {}: {name}: {error}", i.wrapping_add(1))),
                WorkerMsg::FrameDone { frame_id, stats, slot, update_ms } => {
                    let Some(f) = lane.run.in_flight.pop_front() else { anyhow::bail!("unexpected frame") };
                    if f.id != frame_id {
                        anyhow::bail!("frames out of order");
                    }
                    let now = Instant::now();
                    let res = lane.run.res;
                    let pixels = slot.map(|s| {
                        let n = (res[0] * res[1] * 4) as usize;
                        let px = Arc::new(w.shm.slot(s)[..n].to_vec());
                        w.free_slot(s);
                        px
                    });
                    match f.purpose {
                        Purpose::Live => {
                            let t = now.duration_since(start).as_secs_f64();
                            let interval_ms = lane.last_done.map(|d| now.duration_since(d).as_secs_f32() * 1e3).unwrap_or(0.0);
                            lane.last_done = Some(now);
                            if let Some(tu) = &mut lane.tuner {
                                tu.on_frame(t, interval_ms as f64);
                            }
                            let sample = Sample { t, interval_ms, latency_ms: now.duration_since(f.sent).as_secs_f32() * 1e3, cpu_ms: stats.cpu_ms, gpu_ms: stats.gpu_ms, update_ms };
                            let mut sh = shared.lock().unwrap();
                            if let Some(v) = sh.lanes.get_mut(i) {
                                v.samples.push_back(sample);
                                if v.samples.len() > MAX_SAMPLES {
                                    v.samples.pop_front();
                                }
                                v.counters = stats.counters;
                                v.frames += 1;
                                if let Some(px) = pixels {
                                    lane.seq += 1;
                                    v.frame = Some(LiveFrame { width: res[0], height: res[1], pixels: px, seq: lane.seq });
                                    repaint = true;
                                }
                            }
                        }
                        Purpose::Probe(id) => {
                            if let Some(px) = pixels {
                                probe_px.push((id, res, px));
                            }
                        }
                        Purpose::Warm => {}
                    }
                }
                m => anyhow::bail!("unexpected message {m:?}"),
            }
        }
        for (id, res, px) in probe_px {
            if let Some(p) = &mut self.probe {
                if p.id == id && p.waiting > 0 {
                    p.waiting -= 1;
                    if i == REFERENCE {
                        p.reference = Some(px);
                    } else {
                        p.lanes.insert(i, (res, px));
                    }
                }
            }
        }
        if repaint {
            (self.repaint)();
        }
        Ok(busy)
    }

    fn run_tuners(&mut self, now: f64) {
        let mut probe = false;
        for i in 0..self.lanes.len() {
            let busy = self.lanes[i].standby.is_some() || self.lanes[i].want_res.is_some();
            let Some(t) = &mut self.lanes[i].tuner else { continue };
            if busy {
                t.hold(now);
                continue;
            }
            match t.tick(now) {
                Action::Idle => {}
                Action::NeedProbe => probe = true,
                Action::Set(changes) => {
                    for (name, value) in changes {
                        self.set_param(i, &name, value);
                    }
                }
            }
            let t = self.lanes[i].tuner.as_ref().unwrap();
            let view = TunerView {
                config: t.cfg.clone(),
                status: t.status(),
                log: t.log.iter().cloned().collect(),
                history: t.history.iter().rev().take(400).rev().cloned().collect(),
                trials: t.trials,
                accepted: t.accepted,
                best: t.best_summary(),
                best_diff: t
                    .best_values()
                    .map(|b| {
                        b.iter()
                            .filter_map(|(k, v)| {
                                let start = t.start_values.get(k)?;
                                (start != v).then(|| (k.clone(), start.display(), v.display()))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                holding: t.holding(),
            };
            if let Some(v) = self.shared.lock().unwrap().lanes.get_mut(i) {
                v.tuner = Some(view);
            }
        }
        if probe {
            self.request_probe(false);
        }
    }

    fn fail(&mut self, i: usize, e: anyhow::Error) {
        let msg = format!("{e:#}");
        if i == REFERENCE {
            self.reference = None;
            self.probe = None;
            let mut sh = self.shared.lock().unwrap();
            sh.comparing = false;
            sh.log(format!("reference failed: {msg}"));
            return;
        }
        let Some(lane) = self.lanes.get_mut(i) else { return };
        if msg.starts_with("hot swap") {
            if let Some(w) = lane.standby.take().and_then(|s| s.worker) {
                std::thread::spawn(move || w.shutdown());
            }
            self.shared.lock().unwrap().lanes[i].swap = None;
            self.event(i, msg);
            return;
        }
        lane.run.worker = None;
        lane.run.in_flight.clear();
        let mut sh = self.shared.lock().unwrap();
        sh.log(format!("lane {} failed: {msg}", i + 1));
        if let Some(v) = sh.lanes.get_mut(i) {
            v.status = LaneStatus::Failed(msg);
        }
        drop(sh);
        if let Some(p) = &mut self.probe {
            if p.started && !p.lanes.contains_key(&i) {
                p.waiting = p.waiting.saturating_sub(1);
            }
        }
    }

    fn shutdown(&mut self) {
        let mut handles = vec![];
        for l in self.lanes.drain(..).chain(self.reference.take()) {
            for w in [l.run.worker, l.standby.and_then(|s| s.worker)].into_iter().flatten() {
                handles.push(std::thread::spawn(move || w.shutdown()));
            }
        }
        for h in handles {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(shared: &Arc<Mutex<Shared>>, what: &str, secs: u64, f: impl Fn(&Shared) -> bool) {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(secs) {
            if f(&shared.lock().unwrap()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let msg = {
            let sh = shared.lock().unwrap();
            format!("timed out waiting for {what}; status {}; log:\n{}", sh.status, sh.log.join("\n"))
        };
        panic!("{msg}");
    }

    /// Drives a whole live session headlessly (needs a GPU; run with `--ignored`).
    #[test]
    #[ignore]
    fn live_session_end_to_end() {
        let plan = Plan::parse(
            r#"
            [scene]
            generator = { type = "showcase" }
            camera = { type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }
            [run]
            resolution = [320, 180]
            view_radius = 3
            isolation = "thread"
            [[renderer]]
            name = "gpu-dda"
            [[renderer]]
            name = "gpu-raster"
            [[renderer]]
            name = "gpu-dda-lod"
            view_radius = 2
            max_lod = 2
            "#,
            &[],
        )
        .unwrap();
        let control = Control { camera: CameraControl::Path, mode: PassMode::UNLIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 60.0, probe_interval: 0.0 };
        let live = LiveHandle::start(plan.clone(), control, || {});
        wait(&live.shared, "frames from all lanes", 60, |s| s.lanes.len() == 3 && s.lanes.iter().all(|l| l.frame.is_some() && l.samples.len() > 20));
        assert!(live.shared.lock().unwrap().lanes[0].params.iter().any(|(p, _)| p.name == "max_steps"));

        let cam = live.shared.lock().unwrap().camera.unwrap();
        live.control.lock().unwrap().camera = CameraControl::Free(Camera { position: cam.position + DVec3::new(0.0, 2.0, 0.0), ..cam });
        live.control.lock().unwrap().mode = PassMode::LIT;

        live.send(Cmd::Edit { origin: DVec3::new(20.5, 6.0, 0.5), dir: Vec3::NEG_Y, place: None });
        live.send(Cmd::AddLight { position: DVec3::new(20.0, 4.0, 2.0), color: Vec3::ONE, intensity: 30.0, radius: 20.0 });
        wait(&live.shared, "light", 10, |s| s.lights == 1);

        // Live knobs: a renderer parameter and the render scale (resize).
        live.send(Cmd::SetParam { lane: 0, name: "shadows".into(), value: ParamValue::Choice("sun".into()) });
        live.send(Cmd::SetParam { lane: 0, name: "host.render_scale".into(), value: ParamValue::Float(0.5) });
        wait(&live.shared, "resize", 20, |s| s.lanes[0].frame.as_ref().is_some_and(|f| f.width == 160 && f.height == 90));

        // Hot swap (restart-only knob): lane 1 must keep producing frames throughout.
        let frames_before = live.shared.lock().unwrap().lanes[1].frames;
        live.send(Cmd::SetParam { lane: 1, name: "frames_in_flight".into(), value: ParamValue::Int(2) });
        wait(&live.shared, "hot swap start", 10, |s| s.lanes[1].swap.is_some());
        wait(&live.shared, "hot swap complete", 60, |s| s.lanes[1].swap.is_none() && s.lanes[1].events.iter().any(|e| e.1 == "hot swap complete"));
        assert!(live.shared.lock().unwrap().lanes[1].frames > frames_before + 5, "lane kept rendering during the swap");

        // LOD depth is a restart knob: sliding it hot-swaps the LOD lane.
        live.send(Cmd::SetParam { lane: 2, name: "host.max_lod".into(), value: ParamValue::Int(1) });
        wait(&live.shared, "lod hot swap", 60, |s| s.lanes[2].events.iter().filter(|e| e.1 == "hot swap complete").count() >= 1 && s.lanes[2].swap.is_none());
        assert!(live.shared.lock().unwrap().lanes[2].params.iter().any(|(p, v)| p.name == "host.max_lod" && *v == ParamValue::Int(1)));

        // Comparison (keeps images, includes a lane at half resolution).
        live.send(Cmd::Compare);
        wait(&live.shared, "comparison", 60, |s| s.compare.is_some());
        {
            let sh = live.shared.lock().unwrap();
            let c = sh.compare.as_ref().unwrap();
            assert_eq!(c.lanes.len(), 3);
            assert!(c.lanes[2].quality.psnr > 15.0, "LOD lane (2-block cells cannot show the 1-block floor checker): {}", c.lanes[2].quality.psnr);
            assert!(c.lanes[1].quality.psnr > 35.0, "{} psnr {}", c.lanes[1].label, c.lanes[1].quality.psnr);
            assert!(c.lanes[0].quality.psnr > 9.0, "half-res lane still comparable (aliasing at 160x90 is heavy): {:?} {:?} ref {}x{}", c.lanes[0].quality, sh.lanes[0].resolution, c.width, c.height);
            assert!(sh.lanes.iter().all(|l| l.status == LaneStatus::Running), "{:?}", sh.log);
        }

        // Tuner: max fps with quality ignored; it must act and record history.
        live.send(Cmd::Tune { lane: 1, config: Some(TunerConfig { goal: tune::Goal::MaxFps, settle_s: 0.1, window_s: 0.3, ..Default::default() }) });
        wait(&live.shared, "tuner trials", 30, |s| s.lanes[1].tuner.as_ref().is_some_and(|t| t.trials >= 2));
        // Stopping restores the best configuration found.
        live.send(Cmd::Tune { lane: 1, config: None });
        wait(&live.shared, "tuner stopped", 10, |s| s.lanes[1].tuner.is_none() && s.lanes[1].events.iter().any(|(_, e)| e.starts_with("tuner stopped")));
        let restored: Vec<(String, String)> = {
            let sh = live.shared.lock().unwrap();
            let (_, e) = sh.lanes[1].events.iter().rev().find(|(_, e)| e.starts_with("tuner stopped")).unwrap();
            e.split_once(": ").unwrap().1.split(", ").filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
        };
        wait(&live.shared, "best restored", 10, |s| restored.iter().all(|(k, v)| s.lanes[1].params.iter().any(|(p, x)| &p.name == k && &x.display() == v)));

        live.send(Cmd::RemoveLane(0));
        wait(&live.shared, "remove", 10, |s| s.lanes.len() == 2);
        drop(live);
    }

    /// Mesh mode through a live session (needs a GPU; run with `--ignored`): the probe's
    /// reference draws the meshes, so a mesh-capable lane matches it closely and a lane
    /// without mesh support does not.
    #[test]
    #[ignore]
    fn live_meshes_end_to_end() {
        let plan = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/meshes.toml")), &[
            "run.isolation=\"thread\"".into(),
            "renderer=[{name=\"gpu-raster\"},{name=\"gpu-tree64\"}]".into(),
            "run.resolution=[480,270]".into(),
        ])
        .unwrap();
        let mode = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
        let control = Control { camera: CameraControl::Path, mode, scene_frame: 50, events: true, fps_cap: 0.0, display_hz: 30.0, probe_interval: 0.3 };
        let live = LiveHandle::start(plan, control, || {});
        wait(&live.shared, "probes on both lanes", 120, |s| s.lanes.len() == 2 && s.lanes.iter().all(|l| !l.quality.is_empty()));
        let (with, without) = {
            let sh = live.shared.lock().unwrap();
            (sh.lanes[0].quality.back().unwrap().psnr, sh.lanes[1].quality.back().unwrap().psnr)
        };
        println!("mesh mode probe: gpu-raster {with:.1} dB, gpu-tree64 (no meshes) {without:.1} dB");
        assert!(with > 45.0, "gpu-raster with meshes vs reference: {with:.1} dB");
        assert!(with > without + 5.0, "meshes should matter: gpu-raster {with:.1} dB, gpu-tree64 (no meshes) {without:.1} dB");
        drop(live);
    }
}

#[cfg(test)]
mod demo {
    use super::*;

    /// Watch the tuner work on real terrain: `cargo test --release --bin multirenderer
    /// tuner_demo -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn tuner_demo() {
        let plan = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/terrain.toml")), &[
            "run.isolation=\"thread\"".into(),
            "renderer=[{name=\"gpu-dda\"}]".into(),
            "run.resolution=[1920,1080]".into(),
        ])
        .unwrap();
        let control = Control { camera: CameraControl::Path, mode: PassMode::LIT, scene_frame: 150, events: false, fps_cap: 0.0, display_hz: 10.0, probe_interval: 0.0 };
        let live = LiveHandle::start(plan, control, || {});
        let t0 = Instant::now();
        while live.shared.lock().unwrap().lanes.first().is_none_or(|l| l.samples.len() < 50) {
            assert!(t0.elapsed().as_secs() < 60);
            std::thread::sleep(Duration::from_millis(50));
        }
        let fps = |s: &Shared| {
            let v = &s.lanes[0].samples;
            let w: Vec<f32> = v.iter().rev().take(200).map(|x| x.interval_ms).filter(|x| *x > 0.0).collect();
            1000.0 / (w.iter().sum::<f32>() / w.len() as f32)
        };
        println!("untuned: {:.0} fps", fps(&live.shared.lock().unwrap()));
        let target = std::env::var("TARGET_FPS").ok().and_then(|s| s.parse().ok()).unwrap_or(900.0);
        let locked = std::env::var("LOCK").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(String::from).collect();
        let algorithm = match std::env::var("ALGO").as_deref() {
            Ok("bayes") => tune::Algorithm::Bayesian,
            Ok("controller") => tune::Algorithm::ResolutionController,
            _ => tune::Algorithm::LocalSearch,
        };
        live.send(Cmd::Tune { lane: 0, config: Some(TunerConfig { goal: tune::Goal::MaxQualityAtFps { fps: target }, locked, algorithm, ..Default::default() }) });
        let secs = std::env::var("SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
        std::thread::sleep(Duration::from_secs(secs));
        let sh = live.shared.lock().unwrap();
        let tv = sh.lanes[0].tuner.as_ref().unwrap();
        println!("target {target} fps → now {:.0} fps; {}; best {:?} {:?}", fps(&sh), tv.status, tv.best, tv.best_diff);
        for l in &tv.log {
            println!("  {l}");
        }
        for (p, v) in &sh.lanes[0].params {
            println!("  knob {} = {}", p.name, v.display());
        }
    }
}

#[cfg(test)]
mod scale_curve {
    use super::*;

    /// Prints quality vs render scale for a renderer: `RENDERER=gpu-dda cargo test
    /// --release --bin multirenderer scale_curve -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scale_curve() {
        let renderer = std::env::var("RENDERER").unwrap_or("gpu-dda".into());
        let plan = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/terrain.toml")), &[
            "run.isolation=\"thread\"".into(),
            format!("renderer=[{{name=\"{renderer}\"}}]"),
            "run.resolution=[1280,720]".into(),
        ])
        .unwrap();
        let control = Control { camera: CameraControl::Path, mode: PassMode::LIT, scene_frame: 150, events: false, fps_cap: 0.0, display_hz: 10.0, probe_interval: 0.0 };
        let live = LiveHandle::start(plan, control, || {});
        let wait = |f: &dyn Fn(&Shared) -> bool| {
            let t = Instant::now();
            while !f(&live.shared.lock().unwrap()) {
                assert!(t.elapsed().as_secs() < 120, "timeout");
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        wait(&|s| s.lanes.first().is_some_and(|l| l.samples.len() > 100 && l.pending_chunks == 0));
        for scale in [1.0, 0.98, 0.9, 0.75, 0.5, 0.35] {
            live.send(Cmd::SetParam { lane: 0, name: "host.render_scale".into(), value: ParamValue::Float(scale) });
            std::thread::sleep(Duration::from_millis(600));
            let n = live.shared.lock().unwrap().lanes[0].quality.len();
            live.send(Cmd::Compare);
            wait(&|s| s.lanes[0].quality.len() > n);
            let sh = live.shared.lock().unwrap();
            if let (Ok(dir), Some(c)) = (std::env::var("DUMP"), &sh.compare) {
                let _ = image::save_buffer(format!("{dir}/ref.png"), &c.reference, c.width, c.height, image::ExtendedColorType::Rgba8);
                let _ = image::save_buffer(format!("{dir}/lane-{scale:.2}.png"), &c.lanes[0].image, c.width, c.height, image::ExtendedColorType::Rgba8);
            }
            let q = sh.lanes[0].quality.back().unwrap();
            println!("scale {scale:.2} ({}x{}): PSNR {:.1} dB, SSIM {:.1} dB, MS-SSIM {:.1} dB, {:.2}% bad", sh.lanes[0].resolution[0], sh.lanes[0].resolution[1], q.psnr, q.ssim_db, q.ms_ssim_db, q.bad_pct);
        }
    }
}
