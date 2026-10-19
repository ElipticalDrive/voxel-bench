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
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use glam::{DVec3, Vec3};
use mr_core::{BlockId, BlockRegistry, Camera, Environment, FrameRequest, Light, MeshData, MeshInstance, MeshUpdate, ParamSpec, ParamValue, PassMode, RendererInfo, WorldUpdate};
use mr_ipc::HostMsg;
use mr_world::{Scene, World};

use crate::compare::{self, Quality};
use crate::plan::{Plan, RendererSpec};
use crate::present::Presenter;
use crate::stream::{Region, Streamer};
use crate::tune::{HistoryPoint, Tuner, TunerConfig};
use crate::worker::{self, Worker};

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
    /// Lanes shown in a presenter window of their own (bit `i`: lane `i`).
    pub windows: u64,
    /// Those windows' paper white: the frames' 1.0 relative to the display's SDR white.
    pub paper_white: f32,
}

pub enum Cmd {
    Edit { origin: DVec3, dir: Vec3, place: Option<BlockId> },
    AddLight { position: DVec3, color: Vec3, intensity: f32, radius: f32 },
    ClearUserLights,
    /// Change one knob (renderer parameter or `host.*`). Restart-only ones hot-swap.
    SetParam { lane: usize, name: String, value: ParamValue },
    /// Restart a lane with the plan's spec for it (a hot swap), keeping the knob values
    /// changed since that the plan's spec leaves as it was ([`keep_knobs`]).
    Restart { lane: usize, spec: RendererSpec },
    /// Hot-swap a lane to a spec made from its running one (a graph edit).
    Swap { lane: usize, spec: RendererSpec },
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
    /// The spec the plan gave it last (start, ↻); `spec` adds the knobs changed since.
    pub given: RendererSpec,
    pub status: LaneStatus,
    pub frame: Option<LiveFrame>,
    pub samples: VecDeque<Sample>,
    pub counters: Vec<(String, f64)>,
    /// Time per stage, smoothed over recent frames, with the frame each
    /// last ran in.
    pub stages: Vec<mr_core::StageTime>,
    stage_seen: Vec<u64>,
    /// What the colours of the latest frame mean (a diagnostic `view` is active).
    pub legend: Option<mr_core::Legend>,
    pub rss_mb: Option<f64>,
    pub gpu_mb: Option<f64>,
    pub init_ms: f32,
    pub frames: u64,
    pub chunks_sent: u64,
    pub pending_chunks: usize,
    /// What reached the lane's client by its latest frame (bytes over its link, holes).
    pub arrivals: mr_core::link::Arrivals,
    /// Knobs (host first, then renderer) with current values.
    pub params: Vec<(ParamSpec, ParamValue)>,
    pub quality: VecDeque<QualitySample>,
    /// Hot-swap progress, when one is underway.
    pub swap: Option<String>,
    /// Timeline markers (swaps, resizes) for the plots.
    pub events: Vec<(f64, String)>,
    pub tuner: Option<TunerView>,
    pub resolution: [u32; 2],
    /// How its presenter window presents, while one is open.
    pub window: Option<String>,
}

impl LaneView {
    /// Folds a frame's stage times into the shown ones (exponential average). A stage the
    /// frame did not run keeps its value for a while (some run only in some frames), then
    /// goes (a swap or a knob removed it).
    fn smooth_stages(&mut self, frame: Vec<mr_core::StageTime>) {
        const A: f32 = 0.1;
        const KEEP: u64 = 300;
        let mix = |old: Option<f32>, new: Option<f32>| match (old, new) {
            (Some(o), Some(n)) => Some(o + A * (n - o)),
            (o, n) => n.or(o),
        };
        for s in frame {
            match self.stages.iter().position(|o| o.name == s.name && o.nodes == s.nodes) {
                Some(k) => {
                    let o = &mut self.stages[k];
                    o.cpu_ms = mix(o.cpu_ms, s.cpu_ms);
                    o.gpu_ms = mix(o.gpu_ms, s.gpu_ms);
                    self.stage_seen[k] = self.frames;
                }
                None => {
                    self.stages.push(s);
                    self.stage_seen.push(self.frames);
                }
            }
        }
        let mut k = 0;
        while k < self.stages.len() {
            if self.stage_seen[k] + KEEP < self.frames {
                self.stages.remove(k);
                self.stage_seen.remove(k);
            } else {
                k += 1;
            }
        }
    }

    fn new(spec: RendererSpec) -> Self {
        Self {
            given: spec.clone(),
            spec,
            status: LaneStatus::Starting,
            frame: None,
            samples: VecDeque::new(),
            counters: vec![],
            stages: vec![],
            stage_seen: vec![],
            legend: None,
            rss_mb: None,
            gpu_mb: None,
            init_ms: 0.0,
            frames: 0,
            chunks_sent: 0,
            pending_chunks: 0,
            arrivals: Default::default(),
            params: vec![],
            quality: VecDeque::new(),
            swap: None,
            events: vec![],
            tuner: None,
            resolution: [0, 0],
            window: None,
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
    /// When the session started: the zero of the lanes' event times.
    pub start: Option<Instant>,
    /// Lanes whose presenter window went (closed by the user, or failed): the GUI takes
    /// them out of [`Control::windows`].
    pub closed_windows: Vec<usize>,
    /// A presenter window's keys changed the paper white: the GUI takes it over.
    pub paper_white: Option<f32>,
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

/// What a lane of a session hands the lane of the next one (F5) that runs the same plan
/// renderer: the spec the plan gave it, the one it runs, and its host knobs
/// (name, default, value).
pub struct Carry {
    pub given: RendererSpec,
    pub now: RendererSpec,
    pub host: Vec<(String, ParamValue, ParamValue)>,
}

impl Carry {
    /// From the lanes of a running session.
    pub fn of(lanes: &[LaneView]) -> Vec<Carry> {
        lanes
            .iter()
            .map(|v| Carry {
                given: v.given.clone(),
                now: v.spec.clone(),
                host: v.params.iter().filter(|(p, _)| p.name.starts_with("host.") && p.name != "host.max_lod").map(|(p, v)| (p.name.clone(), p.default.clone(), v.clone())).collect(),
            })
            .collect()
    }
}

/// The spec a lane restarts with when the plan gives it `new` (it was given `given` last
/// and runs `now`): `new` with the knob values changed since (where `now` differs from
/// `given`) that `new` leaves as `given` had them. The components are `new`'s; renderer
/// knobs carry over on the same back end only.
pub fn keep_knobs(given: &RendererSpec, now: &RendererSpec, new: RendererSpec) -> RendererSpec {
    let mut kept = new.clone();
    if new.name == now.name {
        for (k, v) in &now.options {
            let component = mr_components::Slot::ALL.iter().any(|s| s.name() == k);
            if !component && given.options.get(k) != Some(v) && new.options.get(k) == given.options.get(k) {
                kept.options.insert(k.clone(), v.clone());
            }
        }
    }
    let lod = |s: &RendererSpec| s.client_knob("max_lod");
    if lod(now) != lod(given) && lod(&new) == lod(given) && kept.set_client_knob("max_lod", lod(now)).is_err() {
        return new;
    }
    if kept.resolve().is_err() {
        return new;
    }
    kept
}

impl LiveHandle {
    /// Starts a session whose lanes take over what `carry` (the previous session's lanes,
    /// [`Carry::of`]) changed: lane `i` from carry `i` when the counts match, else from
    /// the carry given the same spec.
    pub fn start(plan: Plan, carry: Vec<Carry>, control: Control, repaint: impl Fn() + Send + 'static) -> Self {
        let control = Arc::new(Mutex::new(control));
        let shared = Arc::new(Mutex::new(Shared { status: "starting".into(), ..Default::default() }));
        let (cmd, rx) = crossbeam_channel::unbounded();
        let (c, s) = (control.clone(), shared.clone());
        let thread = std::thread::Builder::new()
            .name("live-session".into())
            .spawn(move || {
                let r = Session::run(plan, carry, c, s.clone(), rx, Box::new(repaint));
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

/// The links' clock: seconds since the GUI's first message.
fn clock() -> f64 {
    static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
    START.elapsed().as_secs_f64()
}

/// One worker plus everything needed to keep it in sync with the world.
struct Runner {
    spec: RendererSpec,
    worker: Option<Worker>,
    spawning: Option<Spawn>,
    /// What its client takes of the server's stream (the client, in the worker, derives
    /// what its renderer holds).
    region: Region,
    /// Has taken the server's snapshot (then follows its stream).
    joined: bool,
    /// The reference's own server without a budget: it sees the whole region at once.
    own: Option<Streamer>,
    /// The server's stream not yet sent.
    pending: Vec<WorldUpdate>,
    /// Chunks sent to it over the link.
    chunks_sent: u64,
    /// Mesh mode: geometry and instance updates not yet sent.
    pending_meshes: Vec<MeshUpdate>,
    pending_params: Vec<(String, ParamValue)>,
    in_flight: VecDeque<Flight>,
    next_id: u64,
    res: [u32; 2],
}

impl Runner {
    /// Queues the server's messages for its client.
    fn take(&mut self, stream: &[WorldUpdate]) {
        self.chunks_sent += stream.iter().filter(|u| matches!(u, WorldUpdate::ChunkLoad { .. })).count() as u64;
        self.pending.extend_from_slice(stream);
    }

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
        let time = clock();
        crate::session::send_stream(w, std::mem::take(&mut self.pending), camera.position, time)?;
        for m in std::mem::take(&mut self.pending_meshes) {
            w.send(HostMsg::Meshes(m))?;
        }
        if !self.pending_params.is_empty() {
            w.send(HostMsg::SetParams(std::mem::take(&mut self.pending_params)))?;
        }
        let id = self.next_id;
        self.next_id += 1;
        w.send(HostMsg::Render { req: FrameRequest { frame_id: id, camera, mode: mode.lighting, meshes: mode.meshes, readback: slot.is_some() }, slot, time })?;
        self.in_flight.push_back(Flight { id, sent: Instant::now(), purpose });
        Ok(true)
    }
}

struct Lane {
    run: Runner,
    /// The spec the plan gave it last ([`keep_knobs`]).
    given: RendererSpec,
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

impl Lane {
    /// The spec it runs, or the one a hot swap is loading.
    fn latest(&self) -> &RendererSpec {
        self.standby.as_ref().map_or(&self.run.spec, |s| &s.spec)
    }
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
    /// The one stream every lane's client takes (radius and levels: the largest any takes).
    server: Streamer,
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
    /// Presenter windows by lane.
    presenters: BTreeMap<usize, Presenter>,
}

mod probe;
mod session;
mod tick;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod demo;

#[cfg(test)]
mod scale_curve;
