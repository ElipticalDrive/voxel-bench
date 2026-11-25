//! The tuner's life cycle: measurement, evaluation, applying values, settling and watching the best configuration.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mr_core::{ParamSpec, ParamValue};

use super::config::NEVER_TUNED;
use super::{Action, Algorithm, Eval, Goal, HistoryPoint, Phase, PointKind, QualityMetric, Stage, Tuner, TunerConfig, q};

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

    pub(super) fn note(&mut self, s: String) {
        self.log.push_back(s);
        if self.log.len() > 80 {
            self.log.pop_front();
        }
    }

    pub(super) fn rand(&mut self) -> f64 {
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
    pub(super) fn measure(&self) -> (f64, f64) {
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

    pub(super) fn evaluate(&self) -> Eval {
        let (fps, noise) = self.measure();
        Eval { score: self.cfg.goal.score(fps, self.psnr), fps, noise, psnr: self.psnr }
    }

    /// Required improvement over `best`: the goal's margin or the measurement noise
    /// (two standard errors of the fps difference, converted to score units), whichever
    /// is larger.
    pub(super) fn required_gain(&self, best: &Eval, new: &Eval) -> f64 {
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

    pub(super) fn tunable(&self) -> Vec<ParamSpec> {
        self.specs.iter().filter(|s| !self.cfg.locked.contains(&s.name) && !NEVER_TUNED.contains(&s.name.as_str()) && (s.live || self.cfg.allow_restart)).cloned().collect()
    }

    fn knob_snapshot(&self) -> Vec<(String, f64)> {
        self.tunable().iter().map(|s| (s.name.clone(), crate::bayes::encode(s, self.values.get(&s.name).unwrap_or(&s.default)))).collect()
    }

    pub(super) fn push_history(&mut self, now: f64, e: &Eval, kind: PointKind) {
        let knobs = self.knob_snapshot();
        self.history.push(HistoryPoint { t: now, score: e.score, fps: e.fps, psnr: e.psnr, kind, knobs });
    }

    pub(super) fn set(&mut self, name: &str, v: ParamValue, now: f64) -> Action {
        self.values.insert(name.to_string(), v.clone());
        self.restart_phase(now);
        Action::Set(vec![(name.to_string(), v)])
    }

    /// Applies a whole configuration (only the differences).
    pub(super) fn apply(&mut self, target: &BTreeMap<String, ParamValue>, now: f64) -> Action {
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

    pub(super) fn restart_phase(&mut self, now: f64) {
        self.phase_start = now;
        self.times.clear();
        self.psnr = None;
        self.probe_requested = false;
    }

    /// Records a candidate best configuration (the currently applied values).
    pub(super) fn consider_best(&mut self, e: &Eval) -> bool {
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
    pub(super) fn ready(&mut self, now: f64) -> Result<(), Action> {
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

    pub(super) fn collapsed(&self, now: f64) -> bool {
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

}
