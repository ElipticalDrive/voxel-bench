//! Online auto-tuning of renderer knobs towards a goal.
//!
//! Pure logic (no I/O): the live session feeds it frame completion times and quality
//! probes and applies whatever it asks for. Knobs are [`ParamSpec`]s, including the
//! host-side ones (`host.*`), so renderer options, resolution and streaming are tuned
//! uniformly.
//!
//! Life cycle: **explore** (the chosen algorithm proposes trials) → once nothing has
//! improved for `patience` trials, **hold** the best configuration and only watch it →
//! if the goal is violated or the result degrades for a while (the scene changed),
//! explore again starting from the best known point. Stopping the tuner restores the best
//! configuration ([`Tuner::best_values`]). Every evaluation estimates its own frame-rate
//! noise, and an improvement must beat both the goal's margin and that noise.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mr_core::{ParamKind, ParamSpec, ParamValue};
use serde::{Deserialize, Serialize};

/// Host-side knobs every lane has, alongside its renderer's own parameters.
/// Knobs that change what is shown rather than how fast the same image is made (diagnostic
/// views; `meshes = none` drops the meshes): never tuned.
const NEVER_TUNED: &[&str] = &["view", "view_max", "meshes"];

pub fn host_params(base_view_radius: u32) -> Vec<ParamSpec> {
    vec![
        ParamSpec::float("host.render_scale", "Render resolution relative to the plan's (frames are upscaled for display)", 0.2, 1.0, false, 1.0, true),
        ParamSpec::int("host.view_radius", "Chunks streamed around the camera", 1, 64, base_view_radius as i64, true),
        ParamSpec::int("host.in_flight", "Frames the host lets this renderer queue", 1, 8, 2, true),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Goal {
    /// Best quality while holding at least `fps`.
    MaxQualityAtFps { fps: f64 },
    /// Highest fps while keeping quality (dB) at or above `psnr`.
    MaxFpsAtQuality { psnr: f64 },
    /// Highest fps, quality ignored.
    MaxFps,
}

impl Goal {
    pub fn needs_quality(&self) -> bool {
        !matches!(self, Goal::MaxFps)
    }

    /// Higher is better. `psnr` (the configured quality metric, dB) is required when
    /// [`Goal::needs_quality`].
    ///
    /// Constrained goals are lexicographic: every configuration that satisfies the
    /// constraint scores above every one that does not. Among those that fail, the one
    /// closer to satisfying it wins (quality only breaks ties).
    pub fn score(&self, fps: f64, psnr: Option<f64>) -> f64 {
        let q = psnr.unwrap_or(0.0).min(99.0);
        match *self {
            Goal::MaxQualityAtFps { fps: target } => {
                if fps >= target * 0.98 {
                    100.0 + q
                } else {
                    100.0 * fps / target + q * 1e-3
                }
            }
            Goal::MaxFpsAtQuality { psnr: floor } => {
                if q >= floor {
                    100.0 + 100.0 * (fps / 1000.0).ln_1p()
                } else {
                    q + 1e-3 * fps.ln_1p()
                }
            }
            Goal::MaxFps => fps,
        }
    }

    /// Whether the constraint (if any) is met.
    pub fn satisfied(&self, fps: f64, psnr: Option<f64>) -> bool {
        match *self {
            Goal::MaxQualityAtFps { fps: target } => fps >= target * 0.98,
            Goal::MaxFpsAtQuality { psnr: floor } => psnr.unwrap_or(0.0) >= floor,
            Goal::MaxFps => true,
        }
    }

    /// Smallest score improvement worth switching for, in the goal's own units
    /// (≈0.1 dB of quality, ≈1 % of frame rate).
    pub fn margin(&self, best: f64) -> f64 {
        match *self {
            Goal::MaxQualityAtFps { .. } => 0.1,
            Goal::MaxFpsAtQuality { .. } => 0.5,
            Goal::MaxFps => 0.01 * best.abs().max(1.0),
        }
    }

    /// How far below the best score a held configuration may fall before the tuner
    /// considers the situation changed.
    fn degradation(&self, best: f64) -> f64 {
        match *self {
            Goal::MaxQualityAtFps { .. } => 2.0,
            Goal::MaxFpsAtQuality { .. } => 8.0,
            Goal::MaxFps => 0.12 * best.abs().max(1.0),
        }
    }
}

/// Which quality measure (in dB) the tuner's quality goals refer to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum QualityMetric {
    /// Pixel-exact; any resolution change costs ~30 dB at once on pixel-scale detail.
    Psnr,
    /// Multi-scale structural similarity: degrades gracefully with resolution.
    #[default]
    MsSsim,
}

impl QualityMetric {
    pub fn name(self) -> &'static str {
        match self {
            QualityMetric::Psnr => "PSNR",
            QualityMetric::MsSsim => "MS-SSIM",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Algorithm {
    /// Randomised hill climbing with per-knob adaptive steps.
    LocalSearch,
    /// Dynamic resolution: proportional control of `host.render_scale` towards the
    /// frame-rate target (games' classic answer; a baseline for the learners).
    ResolutionController,
    /// Gaussian-process Bayesian optimisation over all unlocked knobs at once
    /// (constrained expected improvement; see [`crate::bayes`]).
    Bayesian,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TunerConfig {
    pub goal: Goal,
    /// Quality measure the goal's dB values refer to.
    #[serde(default)]
    pub metric: QualityMetric,
    pub algorithm: Algorithm,
    /// Knob names the tuner may not touch.
    pub locked: BTreeSet<String>,
    /// Whether knobs that need a renderer restart (hot swap) may be tuned.
    pub allow_restart: bool,
    /// Seconds to ignore after a change (pipelines warm up, streaming settles).
    pub settle_s: f64,
    /// Seconds of frames measured per evaluation.
    pub window_s: f64,
    /// Trials without improvement before settling on the best configuration
    /// (0 = never stop exploring).
    #[serde(default = "default_patience")]
    pub patience: u32,
}

fn default_patience() -> u32 {
    14
}

impl Default for TunerConfig {
    fn default() -> Self {
        Self {
            goal: Goal::MaxQualityAtFps { fps: 120.0 },
            metric: QualityMetric::MsSsim,
            algorithm: Algorithm::LocalSearch,
            locked: BTreeSet::new(),
            allow_restart: false,
            settle_s: 0.4,
            window_s: 1.2,
            patience: default_patience(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Idle,
    Set(Vec<(String, ParamValue)>),
    /// The tuner needs a quality measurement for the current settings.
    NeedProbe,
}

#[derive(Clone, Debug)]
struct Eval {
    score: f64,
    fps: f64,
    /// Standard error of the fps estimate.
    noise: f64,
    psnr: Option<f64>,
}

#[derive(Clone, Debug)]
enum Phase {
    /// Measuring the incumbent configuration.
    Baseline,
    /// Measuring a trial change; `revert` restores the incumbent.
    Trial { knob: String, revert: ParamValue, value: ParamValue },
    /// A rejected trial was undone; wait for things to settle, then try another.
    Reverting,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Explore,
    /// Settled on the best configuration; watching for changes.
    Hold { violations: u32, windows: u32 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PointKind {
    #[default]
    Rejected,
    Accepted,
    /// A periodic check while holding the best configuration.
    Hold,
}

#[derive(Clone, Debug, Default)]
pub struct HistoryPoint {
    pub t: f64,
    pub score: f64,
    pub fps: f64,
    pub psnr: Option<f64>,
    pub kind: PointKind,
    /// Tunable knobs at this point, each normalised to `[0, 1]`.
    pub knobs: Vec<(String, f64)>,
}

struct BoState {
    opt: crate::bayes::Optimizer,
    dims: Vec<ParamSpec>,
    init: Vec<Vec<f64>>,
    evals: u32,
    predicted: Option<(f64, Option<f64>)>,
}

pub struct Tuner {
    pub cfg: TunerConfig,
    specs: Vec<ParamSpec>,
    /// Currently applied values.
    pub values: BTreeMap<String, ParamValue>,
    /// Values when the tuner started.
    pub start_values: BTreeMap<String, ParamValue>,
    best: Option<(BTreeMap<String, ParamValue>, Eval)>,
    stage: Stage,
    steps: BTreeMap<String, f64>,
    phase: Phase,
    phase_start: f64,
    /// Completion times of frames in the current measurement window.
    times: Vec<f64>,
    psnr: Option<f64>,
    probe_requested: bool,
    incumbent: Option<Eval>,
    trials_since_baseline: u32,
    /// Trials since the best score last improved.
    stale: u32,
    /// Moves rejected since the incumbent last changed (not retried until it does).
    rejected: BTreeSet<(String, String)>,
    rng: u64,
    pub log: VecDeque<String>,
    pub history: Vec<HistoryPoint>,
    pub trials: u32,
    pub accepted: u32,
    bo: Option<BoState>,
}

impl Tuner {
    pub fn new(cfg: TunerConfig, specs: Vec<ParamSpec>, values: BTreeMap<String, ParamValue>, now: f64) -> Self {
        let steps = specs.iter().map(|s| (s.name.clone(), 0.15)).collect();
        let mut t = Self {
            cfg,
            specs,
            start_values: values.clone(),
            values,
            best: None,
            stage: Stage::Explore,
            steps,
            phase: Phase::Baseline,
            phase_start: now,
            times: vec![],
            psnr: None,
            probe_requested: false,
            incumbent: None,
            trials_since_baseline: 0,
            stale: 0,
            rejected: BTreeSet::new(),
            rng: 0x9e37_79b9_7f4a_7c15 ^ now.to_bits(),
            log: VecDeque::new(),
            history: vec![],
            trials: 0,
            accepted: 0,
            bo: None,
        };
        t.note(format!("started: {:?} (quality = {}) via {:?}", t.cfg.goal, t.cfg.metric.name(), t.cfg.algorithm));
        t
    }

    fn note(&mut self, s: String) {
        self.log.push_back(s);
        if self.log.len() > 80 {
            self.log.pop_front();
        }
    }

    fn rand(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Best configuration found so far (what stopping the tuner restores).
    pub fn best_values(&self) -> Option<&BTreeMap<String, ParamValue>> {
        self.best.as_ref().map(|b| &b.0)
    }

    /// Short description of the best result: score, fps and quality.
    pub fn best_summary(&self) -> Option<String> {
        self.best.as_ref().map(|(_, e)| format!("score {:.2} · {:.0} fps{}", e.score, e.fps, q(e.psnr)))
    }

    pub fn holding(&self) -> bool {
        matches!(self.stage, Stage::Hold { .. })
    }

    pub fn status(&self) -> String {
        if let Stage::Hold { violations, .. } = self.stage {
            return if violations > 0 { format!("holding best · goal off-target {violations}/3 checks") } else { "holding best · watching for changes".into() };
        }
        if let Some(bo) = &self.bo {
            return match bo.predicted {
                Some((f, p)) => format!("exploring · evaluation {} (model predicts {f:.0} fps{})", bo.evals + 1, q(p)),
                None => format!("exploring · evaluation {}", bo.evals + 1),
            };
        }
        if self.cfg.algorithm == Algorithm::ResolutionController {
            return "controlling render scale".into();
        }
        match &self.phase {
            Phase::Baseline if self.incumbent.is_none() => "measuring baseline".into(),
            Phase::Baseline => "re-measuring the current best".into(),
            Phase::Trial { knob, value, .. } => format!("exploring · trying {knob} = {} ({} without improvement)", value.display(), self.stale),
            Phase::Reverting => "reverting a rejected trial".into(),
        }
    }

    pub fn on_frame(&mut self, t: f64, _interval_ms: f64) {
        if t >= self.phase_start + self.cfg.settle_s {
            self.times.push(t);
        }
    }

    /// A quality probe of the *current* settings finished; both values are in dB and the
    /// configured metric is used.
    pub fn on_probe(&mut self, t: f64, psnr: f64, ms_ssim_db: f64) {
        let v = match self.cfg.metric {
            QualityMetric::Psnr => psnr,
            QualityMetric::MsSsim => ms_ssim_db,
        };
        if self.probe_requested && t >= self.phase_start + self.cfg.settle_s {
            self.psnr = Some(v);
            self.probe_requested = false;
        }
    }

    /// Throughput over the window, and its standard error from three sub-windows.
    /// (Completion intervals are useless: GPU renderers with several frames in flight
    /// complete them in bursts.)
    fn measure(&self) -> (f64, f64) {
        let (Some(&a), Some(&b)) = (self.times.first(), self.times.last()) else { return (0.0, 0.0) };
        if b <= a {
            return (0.0, 0.0);
        }
        let fps = (self.times.len() - 1) as f64 / (b - a);
        let third = (b - a) / 3.0;
        let subs: Vec<f64> = (0..3)
            .map(|i| {
                let (lo, hi) = (a + third * i as f64, a + third * (i + 1) as f64);
                self.times.iter().filter(|&&t| t >= lo && t < hi).count() as f64 / third
            })
            .collect();
        let mean = subs.iter().sum::<f64>() / 3.0;
        let sd = (subs.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / 2.0).sqrt();
        (fps, (sd / 3f64.sqrt()).max(fps * 0.005))
    }

    fn evaluate(&self) -> Eval {
        let (fps, noise) = self.measure();
        Eval { score: self.cfg.goal.score(fps, self.psnr), fps, noise, psnr: self.psnr }
    }

    /// Required improvement over `best`: the goal's margin or the measurement noise
    /// (two standard errors of the fps difference, converted to score units), whichever
    /// is larger.
    fn required_gain(&self, best: &Eval, new: &Eval) -> f64 {
        let g = &self.cfg.goal;
        // Local slope of the score in fps, one-sided so it never straddles the goal's
        // threshold (where the score steps).
        let slope = |e: &Eval| {
            let h = (e.fps * 1e-3).max(1e-3);
            let up = (g.score(e.fps + h, e.psnr) - g.score(e.fps, e.psnr)).abs();
            let down = (g.score(e.fps, e.psnr) - g.score(e.fps - h, e.psnr)).abs();
            up.min(down) / h
        };
        let noise = 2.0 * best.noise.hypot(new.noise) * slope(best).max(slope(new));
        g.margin(best.score).max(noise)
    }

    fn tunable(&self) -> Vec<ParamSpec> {
        self.specs.iter().filter(|s| !self.cfg.locked.contains(&s.name) && !NEVER_TUNED.contains(&s.name.as_str()) && (s.live || self.cfg.allow_restart)).cloned().collect()
    }

    fn knob_snapshot(&self) -> Vec<(String, f64)> {
        self.tunable().iter().map(|s| (s.name.clone(), crate::bayes::encode(s, self.values.get(&s.name).unwrap_or(&s.default)))).collect()
    }

    fn push_history(&mut self, now: f64, e: &Eval, kind: PointKind) {
        let knobs = self.knob_snapshot();
        self.history.push(HistoryPoint { t: now, score: e.score, fps: e.fps, psnr: e.psnr, kind, knobs });
    }

    fn set(&mut self, name: &str, v: ParamValue, now: f64) -> Action {
        self.values.insert(name.to_string(), v.clone());
        self.restart_phase(now);
        Action::Set(vec![(name.to_string(), v)])
    }

    /// Applies a whole configuration (only the differences).
    fn apply(&mut self, target: &BTreeMap<String, ParamValue>, now: f64) -> Action {
        let changes: Vec<(String, ParamValue)> = target.iter().filter(|(k, v)| self.values.get(*k) != Some(v)).map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in &changes {
            self.values.insert(k.clone(), v.clone());
        }
        self.restart_phase(now);
        if changes.is_empty() { Action::Idle } else { Action::Set(changes) }
    }

    /// Restart the current measurement (call while the system is in flux: hot swaps,
    /// resizes), so nothing measured before `now` counts.
    pub fn hold(&mut self, now: f64) {
        self.restart_phase(now);
    }

    fn restart_phase(&mut self, now: f64) {
        self.phase_start = now;
        self.times.clear();
        self.psnr = None;
        self.probe_requested = false;
    }

    /// Records a candidate best configuration (the currently applied values).
    fn consider_best(&mut self, e: &Eval) -> bool {
        let better = match &self.best {
            None => true,
            Some((_, b)) => e.score > b.score + self.required_gain(b, e),
        };
        if better {
            self.best = Some((self.values.clone(), e.clone()));
            self.stale = 0;
        }
        better
    }

    /// Ready to evaluate: window complete and (if needed) quality measured. Returns the
    /// action to take while not ready.
    fn ready(&mut self, now: f64) -> Result<(), Action> {
        if now - self.phase_start < self.cfg.settle_s + self.cfg.window_s || self.times.len() < 4 {
            return Err(Action::Idle);
        }
        if self.cfg.goal.needs_quality() && self.psnr.is_none() {
            if !self.probe_requested {
                self.probe_requested = true;
                return Err(Action::NeedProbe);
            }
            return Err(Action::Idle);
        }
        Ok(())
    }

    fn collapsed(&self, now: f64) -> bool {
        match self.cfg.goal {
            Goal::MaxQualityAtFps { fps } => now - self.phase_start > self.cfg.settle_s + 0.3 && self.times.len() >= 5 && self.measure().0 < fps * 0.5,
            _ => false,
        }
    }

    pub fn tick(&mut self, now: f64) -> Action {
        if self.cfg.algorithm == Algorithm::ResolutionController {
            return self.controller(now);
        }
        if let Stage::Hold { .. } = self.stage {
            return self.watch(now);
        }
        if let Some((_, b)) = &self.best {
            // An unmet goal earns three times the patience before giving up on it.
            let patience = if self.cfg.goal.satisfied(b.fps, b.psnr) { self.cfg.patience } else { self.cfg.patience * 3 };
            if self.cfg.patience > 0 && self.stale >= patience {
                return self.settle(now);
            }
        }
        if self.cfg.algorithm == Algorithm::Bayesian {
            return self.bayesian(now);
        }
        self.local_search(now)
    }

    /// Converged: apply the best configuration and start watching it.
    fn settle(&mut self, now: f64) -> Action {
        let (values, _) = self.best.clone().unwrap();
        self.note(format!("settled after {} trials without improvement: {}", self.stale, self.best_summary().unwrap_or_default()));
        self.stage = Stage::Hold { violations: 0, windows: 0 };
        self.phase = Phase::Baseline;
        self.apply(&values, now)
    }

    /// Holding: re-measure the best configuration; resume exploring if the goal is
    /// violated or the result degraded for three consecutive checks.
    fn watch(&mut self, now: f64) -> Action {
        let Stage::Hold { violations, windows } = self.stage else { return Action::Idle };
        // Quality is only re-probed every third check (probes cost reference time).
        let needs_q = self.cfg.goal.needs_quality() && windows % 3 == 0;
        if now - self.phase_start < self.cfg.settle_s + self.cfg.window_s.max(1.0) || self.times.len() < 4 {
            return Action::Idle;
        }
        if needs_q && self.psnr.is_none() {
            if !self.probe_requested {
                self.probe_requested = true;
                return Action::NeedProbe;
            }
            return Action::Idle;
        }
        let best = self.best.as_ref().map(|b| b.1.clone()).unwrap();
        let (fps, noise) = self.measure();
        let psnr = self.psnr.or(best.psnr);
        let e = Eval { score: self.cfg.goal.score(fps, psnr), fps, noise, psnr };
        let off = !self.cfg.goal.satisfied(fps, psnr) && self.cfg.goal.satisfied(best.fps, best.psnr) || e.score < best.score - self.cfg.goal.degradation(best.score);
        self.push_history(now, &e, PointKind::Hold);
        let violations = if off { violations + 1 } else { 0 };
        if !off {
            // Track slow drift so the reference point stays honest.
            if let Some((_, b)) = &mut self.best {
                b.fps = 0.8 * b.fps + 0.2 * fps;
                if needs_q {
                    b.psnr = psnr;
                }
                b.score = self.cfg.goal.score(b.fps, b.psnr);
            }
        }
        if violations >= 3 {
            self.note(format!("conditions changed ({fps:.0} fps{}): exploring again from the best known settings", q(psnr)));
            self.stage = Stage::Explore;
            self.stale = 0;
            self.rejected.clear();
            self.incumbent = None;
            self.best = None;
            self.phase = Phase::Baseline;
            if let Some(bo) = &mut self.bo {
                bo.opt.obs.clear();
            }
            self.restart_phase(now);
            return Action::Idle;
        }
        self.stage = Stage::Hold { violations, windows: windows + 1 };
        self.restart_phase(now);
        Action::Idle
    }

    fn local_search(&mut self, now: f64) -> Action {
        if matches!(self.phase, Phase::Reverting) {
            return if now - self.phase_start >= self.cfg.settle_s { self.propose(now) } else { Action::Idle };
        }
        // Early abort: a trial that collapses the frame rate is reverted immediately.
        if let Phase::Trial { knob, revert, value } = self.phase.clone() {
            if self.collapsed(now) {
                self.note(format!("abort {knob} = {}: frame rate collapsed ({:.0} fps)", value.display(), self.measure().0));
                self.rejected.insert((knob.clone(), value.display()));
                self.fail_step(&knob);
                self.trials += 1;
                self.trials_since_baseline += 1;
                self.stale += 1;
                self.phase = Phase::Reverting;
                return self.set(&knob, revert, now);
            }
        }
        if let Err(a) = self.ready(now) {
            return a;
        }
        let eval = self.evaluate();
        match self.phase.clone() {
            Phase::Baseline => {
                self.note(format!("baseline: score {:.2} ({:.0}±{:.0} fps{})", eval.score, eval.fps, 2.0 * eval.noise, q(eval.psnr)));
                self.push_history(now, &eval, PointKind::Accepted);
                // The incumbent is re-measured: its fresh measurement replaces the old one.
                if let Some((v, b)) = &mut self.best {
                    if *v == self.values {
                        *b = eval.clone();
                    }
                }
                if self.best.is_none() {
                    self.consider_best(&eval);
                }
                self.incumbent = Some(eval);
                self.trials_since_baseline = 0;
                self.propose(now)
            }
            Phase::Reverting => Action::Idle,
            Phase::Trial { knob, revert, value } => {
                self.trials += 1;
                self.trials_since_baseline += 1;
                let inc = self.incumbent.clone().unwrap_or(eval.clone());
                let need = self.required_gain(&inc, &eval);
                let accepted = eval.score > inc.score + need;
                self.push_history(now, &eval, if accepted { PointKind::Accepted } else { PointKind::Rejected });
                if accepted {
                    self.accepted += 1;
                    self.note(format!("✔ {knob} = {}: {:.2} > {:.2} + {need:.2} ({:.0} fps{})", value.display(), eval.score, inc.score, eval.fps, q(eval.psnr)));
                    *self.steps.get_mut(&knob).unwrap() = (self.steps[&knob] * 1.5).min(0.5);
                    self.incumbent = Some(eval.clone());
                    self.rejected.clear();
                    if !self.consider_best(&eval) {
                        self.stale += 1;
                    }
                    if self.trials_since_baseline >= 8 {
                        self.phase = Phase::Baseline;
                        self.restart_phase(now);
                        return Action::Idle;
                    }
                    self.propose(now)
                } else {
                    self.stale += 1;
                    self.note(format!("✘ {knob} = {}: {:.2} ≤ {:.2} + {need:.2} ({:.0} fps{})", value.display(), eval.score, inc.score, eval.fps, q(eval.psnr)));
                    self.rejected.insert((knob.clone(), value.display()));
                    self.fail_step(&knob);
                    // Revert; every few trials re-measure the incumbent, since the scene drifts.
                    self.phase = if self.trials_since_baseline >= 8 { Phase::Baseline } else { Phase::Reverting };
                    self.set(&knob, revert, now)
                }
            }
        }
    }

    fn fail_step(&mut self, knob: &str) {
        if let Some(s) = self.steps.get_mut(knob) {
            *s = (*s * 0.7).max(0.02);
        }
    }

    fn propose(&mut self, now: f64) -> Action {
        let knobs = self.tunable();
        if knobs.is_empty() {
            if self.history.len() <= 1 {
                self.note("nothing to tune: every knob is locked".into());
            }
            self.phase = Phase::Baseline;
            self.restart_phase(now);
            return Action::Idle;
        }
        for _ in 0..24 {
            let k = knobs[(self.rand() * knobs.len() as f64) as usize % knobs.len()].clone();
            let cur = self.values.get(&k.name).cloned().unwrap_or(k.default.clone());
            let step = self.steps[&k.name];
            let up = self.rand() < 0.5;
            let next = match &k.kind {
                ParamKind::Bool => ParamValue::Bool(!cur.as_bool().unwrap_or(false)),
                ParamKind::Int { min, max } => {
                    let span = (*max - *min).max(1) as f64;
                    let c = cur.as_i64().unwrap_or(*min);
                    // Wide ranges move multiplicatively, narrow ones by at least one.
                    let d = if span > 200.0 { ((c.abs().max(1) as f64) * step * 2.0).round() as i64 } else { (step * span).round() as i64 }.max(1);
                    ParamValue::Int((if up { c + d } else { c - d }).clamp(*min, *max))
                }
                ParamKind::Float { min, max, log } => {
                    let c = cur.as_f64().unwrap_or(*min);
                    let n = if *log { if up { c * (1.0 + step) } else { c / (1.0 + step) } } else if up { c + step * (max - min) } else { c - step * (max - min) };
                    ParamValue::Float((n.clamp(*min, *max) * 1000.0).round() / 1000.0)
                }
                ParamKind::Choice(opts) => {
                    let others: Vec<&String> = opts.iter().filter(|o| Some(o.as_str()) != cur.as_str()).collect();
                    if others.is_empty() {
                        continue;
                    }
                    ParamValue::Choice(others[(self.rand() * others.len() as f64) as usize % others.len()].clone())
                }
            };
            if next == cur || self.rejected.contains(&(k.name.clone(), next.display())) {
                continue;
            }
            self.phase = Phase::Trial { knob: k.name.clone(), revert: cur, value: next.clone() };
            return self.set(&k.name, next, now);
        }
        // Every neighbouring move was already rejected: we're at a local optimum. Settle,
        // or (never settling) forget the rejections and keep sampling.
        if self.cfg.patience == 0 {
            self.rejected.clear();
        } else {
            self.stale = self.stale.max(self.cfg.patience * 3);
        }
        self.phase = Phase::Baseline;
        self.restart_phase(now);
        Action::Idle
    }

    fn bayesian(&mut self, now: f64) -> Action {
        use crate::bayes::{Observation, Optimizer, decode, encode};
        if self.bo.is_none() {
            let dims = self.tunable();
            if dims.is_empty() {
                if self.history.is_empty() {
                    self.note("nothing to tune: every knob is locked".into());
                    self.history.push(HistoryPoint::default());
                }
                return Action::Idle;
            }
            // Safe start: the current settings, then a few nearby variations.
            let x0: Vec<f64> = dims.iter().map(|d| encode(d, self.values.get(&d.name).unwrap_or(&d.default))).collect();
            let mut init = vec![];
            for _ in 0..4 {
                init.push(x0.iter().map(|v| (v + (self.rand() - 0.5) * 0.3).clamp(0.0, 1.0)).collect());
            }
            self.note(format!("bayesian optimisation over {} knobs", dims.len()));
            self.bo = Some(BoState { opt: Optimizer::new(self.cfg.goal), dims, init, evals: 0, predicted: None });
        }
        let collapsed = self.collapsed(now);
        if !collapsed {
            if let Err(a) = self.ready(now) {
                return a;
            }
        }
        let mut eval = self.evaluate();
        eval.fps = eval.fps.max(1e-3);
        if collapsed {
            eval.psnr = None;
            eval.score = self.cfg.goal.score(eval.fps, None);
        }
        let improved = !collapsed && self.consider_best(&eval);
        if !improved {
            self.stale += 1;
        }
        self.trials += 1;
        if improved {
            self.accepted += 1;
        }
        self.push_history(now, &eval, if improved { PointKind::Accepted } else { PointKind::Rejected });
        let bo = self.bo.as_mut().unwrap();
        let x: Vec<f64> = bo.dims.iter().map(|d| encode(d, self.values.get(&d.name).unwrap_or(&d.default))).collect();
        bo.opt.observe(Observation { x: x.clone(), log_fps: eval.fps.ln(), psnr: eval.psnr });
        bo.evals += 1;
        let pred = bo.predicted.take();
        let evals = bo.evals;
        let summary = bo.dims.iter().map(|d| format!("{}={}", d.name.trim_start_matches("host."), self.values.get(&d.name).map(|v| v.display()).unwrap_or_default())).collect::<Vec<_>>().join(" ");
        self.note(format!(
            "{} #{}: {:.0}±{:.0} fps{}{}  [{summary}]",
            if collapsed { "⚠ collapsed" } else if improved { "✔" } else { "·" },
            evals,
            eval.fps,
            2.0 * eval.noise,
            q(eval.psnr),
            pred.map(|(f, p)| format!(" (predicted {f:.0} fps{})", q(p))).unwrap_or_default()
        ));

        // Next point: initial designs, a periodic re-check of the best, or the model's pick.
        let bo = self.bo.as_mut().unwrap();
        let best_x = self.best.as_ref().map(|(v, _)| bo.dims.iter().map(|d| encode(d, v.get(&d.name).unwrap_or(&d.default))).collect::<Vec<f64>>());
        let next = if let Some(x) = bo.init.pop() {
            x
        } else if bo.evals % 7 == 0 && best_x.is_some() {
            best_x.clone().unwrap()
        } else {
            let d = bo.dims.len();
            let center = best_x.clone().unwrap_or(x.clone());
            let mut candidates: Vec<Vec<f64>> = Vec::with_capacity(512);
            let mut r = self.rng;
            let mut rand = || {
                r ^= r << 13;
                r ^= r >> 7;
                r ^= r << 17;
                (r >> 11) as f64 / (1u64 << 53) as f64
            };
            for _ in 0..256 {
                candidates.push((0..d).map(|_| rand()).collect());
            }
            for (c, spread) in [(&center, 0.08), (&center, 0.2), (&x, 0.1)] {
                for _ in 0..96 {
                    candidates.push(c.iter().map(|v| (v + (rand() - 0.5) * 2.0 * spread).clamp(0.0, 1.0)).collect());
                }
            }
            self.rng = r;
            let bo = self.bo.as_mut().unwrap();
            match bo.opt.suggest(&candidates) {
                Some((x, f, p)) => {
                    bo.predicted = Some((f, p));
                    x
                }
                None => center,
            }
        };
        let bo = self.bo.as_ref().unwrap();
        let mut target = self.values.clone();
        for (d, v) in bo.dims.iter().zip(&next) {
            target.insert(d.name.clone(), decode(d, *v));
        }
        self.apply(&target, now)
    }

    fn controller(&mut self, now: f64) -> Action {
        const KNOB: &str = "host.render_scale";
        let target = match self.cfg.goal {
            Goal::MaxQualityAtFps { fps } => fps,
            _ => {
                if self.history.is_empty() {
                    self.note("the resolution controller needs a frame-rate target goal".into());
                    self.history.push(HistoryPoint::default());
                }
                return Action::Idle;
            }
        };
        if self.cfg.locked.contains(KNOB) {
            return Action::Idle;
        }
        // Short, frequent windows: this is a feedback loop, not a search.
        if now - self.phase_start < 0.15 + 0.35 || self.times.len() < 4 {
            return Action::Idle;
        }
        let (fps, noise) = self.measure();
        let cur = self.values.get(KNOB).and_then(|v| v.as_f64()).unwrap_or(1.0);
        // Pixel cost ~ scale², so fps ∝ 1/scale²: aim for target with 5 % headroom.
        let want = (cur * (fps / (target * 1.05)).sqrt()).clamp(0.2, 1.0);
        let next = cur + (want - cur) * 0.6;
        let e = Eval { score: fps, fps, noise, psnr: None };
        self.best = Some((self.values.clone(), e.clone()));
        self.push_history(now, &e, PointKind::Accepted);
        if (next - cur).abs() < 0.01 {
            self.restart_phase(now);
            return Action::Idle;
        }
        let v = ParamValue::Float((next * 1000.0).round() / 1000.0);
        self.set(KNOB, v, now)
    }
}

fn q(p: Option<f64>) -> String {
    p.map(|p| format!(", {:.1} dB", p.min(99.0))).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic renderer: quality rises with scale and steps, cost rises with both.
    /// `slowdown` multiplies frame cost (a scene change).
    fn simulate(values: &BTreeMap<String, ParamValue>, slowdown: f64) -> (f64, f64) {
        let scale = values["host.render_scale"].as_f64().unwrap();
        let steps = values["max_steps"].as_i64().unwrap() as f64;
        let shadows = values["shadows"].as_str().unwrap() == "all";
        let cost_ms = (0.5 + 6.0 * scale * scale + steps / 2000.0 + if shadows { 2.0 } else { 0.0 }) * slowdown;
        let psnr = 30.0 + 20.0 * scale + (steps.min(512.0) / 512.0) * 5.0 + if shadows { 6.0 } else { 0.0 };
        (1000.0 / cost_ms, psnr)
    }

    struct Run {
        t: Tuner,
        values: BTreeMap<String, ParamValue>,
        now: f64,
        rng: u64,
    }

    impl Run {
        fn new(cfg: TunerConfig) -> Self {
            Self::seeded(cfg, 0.0)
        }

        /// The tuner's random seed derives from its start time.
        fn seeded(cfg: TunerConfig, start: f64) -> Self {
            let mut specs = host_params(8);
            specs.push(ParamSpec::int("max_steps", "", 32, 4096, 4096, true));
            specs.push(ParamSpec::choice("shadows", "", &["none", "sun", "all"], "none", true));
            let mut values: BTreeMap<String, ParamValue> = specs.iter().map(|s| (s.name.clone(), s.default.clone())).collect();
            values.insert("host.render_scale".into(), ParamValue::Float(1.0));
            Self { t: Tuner::new(cfg, specs, values.clone(), start), values, now: start, rng: 12345 ^ start.to_bits() }
        }

        /// Runs for `seconds` with per-frame time jitter of `jitter` (fraction).
        fn run(&mut self, seconds: f64, slowdown: f64, jitter: f64) {
            let end = self.now + seconds;
            while self.now < end {
                let (fps, psnr) = simulate(&self.values, slowdown);
                self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let r = (self.rng >> 33) as f64 / (1u64 << 31) as f64 - 0.5;
                self.now += (1.0 + r * 2.0 * jitter) / fps;
                self.t.on_frame(self.now, 1000.0 / fps);
                match self.t.tick(self.now) {
                    Action::Set(changes) => self.values.extend(changes),
                    Action::NeedProbe => self.t.on_probe(self.now, psnr, psnr),
                    Action::Idle => {}
                }
            }
        }
    }

    #[test]
    fn local_search_meets_fps_target_and_improves_quality() {
        let mut r = Run::new(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 150.0 }, patience: 0, ..Default::default() });
        r.run(120.0, 1.0, 0.0);
        let best = r.t.best_values().unwrap().clone();
        let (fps, psnr) = simulate(&best, 1.0);
        assert!(fps >= 150.0 * 0.95, "fps {fps} best {best:?}\n{:?}", r.t.log);
        assert!(psnr > 45.0, "psnr {psnr} best {best:?}\n{:?}", r.t.log);
        assert!(r.t.accepted > 0);
    }

    #[test]
    fn bayesian_meets_fps_target_and_improves_quality() {
        let mut r = Run::new(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 150.0 }, algorithm: Algorithm::Bayesian, patience: 0, ..Default::default() });
        r.run(90.0, 1.0, 0.0);
        let best = r.t.best_values().unwrap().clone();
        let (fps, psnr) = simulate(&best, 1.0);
        assert!(fps >= 147.0 && psnr > 50.0, "best {fps:.0} fps {psnr:.1} dB\n{:?}", r.t.log);
    }

    #[test]
    fn locked_knobs_are_never_touched() {
        let mut cfg = TunerConfig { goal: Goal::MaxFps, ..Default::default() };
        cfg.locked = ["host.render_scale".to_string(), "shadows".to_string()].into();
        let mut r = Run::new(cfg);
        r.run(40.0, 1.0, 0.0);
        assert_eq!(r.values["host.render_scale"], ParamValue::Float(1.0));
        assert_eq!(r.values["shadows"], ParamValue::Choice("none".into()));
    }

    #[test]
    fn controller_converges_on_target() {
        let mut r = Run::new(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 200.0 }, algorithm: Algorithm::ResolutionController, ..Default::default() });
        r.run(20.0, 1.0, 0.0);
        let (fps, _) = simulate(&r.values, 1.0);
        assert!((190.0..260.0).contains(&fps), "fps {fps} values {:?}", r.values);
    }

    #[test]
    fn settles_on_best_then_resumes_when_the_scene_changes() {
        for (algorithm, seed) in [Algorithm::LocalSearch, Algorithm::Bayesian].into_iter().flat_map(|a| (1..=4).map(move |s| (a, s as f64 * 0.37))) {
            let mut r = Run::seeded(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 150.0 }, algorithm, patience: 10, ..Default::default() }, seed);
            r.run(200.0, 1.0, 0.05);
            assert!(r.t.holding(), "{algorithm:?} seed {seed} should have settled\n{:?}", r.t.log);
            // Holding means the best configuration is applied.
            assert_eq!(Some(&r.values), r.t.best_values(), "{algorithm:?}");
            let (fps, _) = simulate(&r.values, 1.0);
            assert!(fps >= 145.0, "{algorithm:?} seed {seed} settled at {fps:.0} fps\n{:?}", r.t.log);
            // The scene gets 60 % more expensive: the target is missed, so it explores again.
            r.run(20.0, 1.6, 0.05);
            assert!(r.t.log.iter().any(|l| l.contains("exploring again")), "{algorithm:?} seed {seed}\n{:?}", r.t.log);
            r.run(200.0, 1.6, 0.05);
            let best = r.t.best_values().unwrap().clone();
            let (fps, _) = simulate(&best, 1.6);
            assert!(fps >= 145.0, "{algorithm:?} seed {seed} re-tuned best {fps:.0} fps\n{:?}", r.t.log);
        }
    }

    #[test]
    fn noise_does_not_produce_false_improvements() {
        // Only one knob, and it does nothing: every "improvement" would be noise.
        let mut specs = vec![ParamSpec::int("useless", "", 0, 100, 50, true)];
        specs.extend(host_params(8));
        let cfg = TunerConfig { goal: Goal::MaxFps, locked: ["host.render_scale", "host.view_radius", "host.in_flight"].iter().map(|s| s.to_string()).collect(), patience: 0, ..Default::default() };
        let values: BTreeMap<String, ParamValue> = specs.iter().map(|s| (s.name.clone(), s.default.clone())).collect();
        let mut t = Tuner::new(cfg, specs, values, 0.0);
        let (mut now, mut rng) = (0.0, 7u64);
        while now < 120.0 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r = (rng >> 33) as f64 / (1u64 << 31) as f64 - 0.5;
            now += (1.0 + r * 0.6) / 200.0; // 200 fps, ±30 % frame-time jitter
            t.on_frame(now, 5.0);
            t.tick(now);
        }
        assert!(t.trials > 20);
        assert!(t.accepted <= 2, "accepted {} of {} trials on pure noise\n{:?}", t.accepted, t.trials, t.log);
    }
}
