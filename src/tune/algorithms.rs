//! The search algorithms: local search, Bayesian optimisation and the resolution controller.

use mr_core::{ParamKind, ParamValue};

use super::{Action, BoState, Eval, Goal, HistoryPoint, Phase, PointKind, Tuner, q};

impl Tuner {
    pub(super) fn local_search(&mut self, now: f64) -> Action {
        if matches!(self.phase, Phase::Reverting) {
            return if now - self.phase_start >= self.cfg.settle_s {
                self.propose(now)
            } else {
                Action::Idle
            };
        }
        // Early abort: a trial that collapses the frame rate is reverted immediately.
        if let Phase::Trial {
            knob,
            revert,
            value,
        } = self.phase.clone()
            && self.collapsed(now)
        {
            self.note(format!(
                "abort {knob} = {}: frame rate collapsed ({:.0} fps)",
                value.display(),
                self.measure().0
            ));
            self.rejected.insert((knob.clone(), value.display()));
            self.fail_step(&knob);
            self.trials += 1;
            self.trials_since_baseline += 1;
            self.stale += 1;
            self.phase = Phase::Reverting;
            return self.set(&knob, revert, now);
        }
        if let Err(a) = self.ready(now) {
            return a;
        }
        let eval = self.evaluate();
        match self.phase.clone() {
            Phase::Baseline => {
                self.note(format!(
                    "baseline: score {:.2} ({:.0}±{:.0} fps{})",
                    eval.score,
                    eval.fps,
                    2.0 * eval.noise,
                    q(eval.psnr)
                ));
                self.push_history(now, &eval, PointKind::Accepted);
                // The incumbent is re-measured: its fresh measurement replaces the old one.
                if let Some((v, b)) = &mut self.best
                    && *v == self.values
                {
                    *b = eval.clone();
                }
                if self.best.is_none() {
                    self.consider_best(&eval);
                }
                self.incumbent = Some(eval);
                self.trials_since_baseline = 0;
                self.propose(now)
            }
            Phase::Reverting => Action::Idle,
            Phase::Trial {
                knob,
                revert,
                value,
            } => {
                self.trials += 1;
                self.trials_since_baseline += 1;
                let inc = self.incumbent.clone().unwrap_or(eval.clone());
                let need = self.required_gain(&inc, &eval);
                let accepted = eval.score > inc.score + need;
                self.push_history(
                    now,
                    &eval,
                    if accepted {
                        PointKind::Accepted
                    } else {
                        PointKind::Rejected
                    },
                );
                if accepted {
                    self.accepted += 1;
                    self.note(format!(
                        "✔ {knob} = {}: {:.2} > {:.2} + {need:.2} ({:.0} fps{})",
                        value.display(),
                        eval.score,
                        inc.score,
                        eval.fps,
                        q(eval.psnr)
                    ));
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
                    self.note(format!(
                        "✘ {knob} = {}: {:.2} ≤ {:.2} + {need:.2} ({:.0} fps{})",
                        value.display(),
                        eval.score,
                        inc.score,
                        eval.fps,
                        q(eval.psnr)
                    ));
                    self.rejected.insert((knob.clone(), value.display()));
                    self.fail_step(&knob);
                    // Revert; every few trials re-measure the incumbent, since the scene drifts.
                    self.phase = if self.trials_since_baseline >= 8 {
                        Phase::Baseline
                    } else {
                        Phase::Reverting
                    };
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
            let cur = self
                .values
                .get(&k.name)
                .cloned()
                .unwrap_or(k.default.clone());
            let step = self.steps[&k.name];
            let up = self.rand() < 0.5;
            let next = match &k.kind {
                ParamKind::Bool => ParamValue::Bool(!cur.as_bool().unwrap_or(false)),
                ParamKind::Int { min, max } => {
                    let span = (*max - *min).max(1) as f64;
                    let c = cur.as_i64().unwrap_or(*min);
                    // Wide ranges move multiplicatively, narrow ones by at least one.
                    let d = if span > 200.0 {
                        ((c.abs().max(1) as f64) * step * 2.0).round() as i64
                    } else {
                        (step * span).round() as i64
                    }
                    .max(1);
                    ParamValue::Int((if up { c + d } else { c - d }).clamp(*min, *max))
                }
                ParamKind::Float { min, max, log } => {
                    let c = cur.as_f64().unwrap_or(*min);
                    let n = if *log {
                        if up {
                            c * (1.0 + step)
                        } else {
                            c / (1.0 + step)
                        }
                    } else if up {
                        c + step * (max - min)
                    } else {
                        c - step * (max - min)
                    };
                    ParamValue::Float((n.clamp(*min, *max) * 1000.0).round() / 1000.0)
                }
                ParamKind::Choice(opts) => {
                    let others: Vec<&String> = opts
                        .iter()
                        .filter(|o| Some(o.as_str()) != cur.as_str())
                        .collect();
                    if others.is_empty() {
                        continue;
                    }
                    ParamValue::Choice(
                        others[(self.rand() * others.len() as f64) as usize % others.len()].clone(),
                    )
                }
            };
            if next == cur || self.rejected.contains(&(k.name.clone(), next.display())) {
                continue;
            }
            self.phase = Phase::Trial {
                knob: k.name.clone(),
                revert: cur,
                value: next.clone(),
            };
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

    pub(super) fn bayesian(&mut self, now: f64) -> Action {
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
            let x0: Vec<f64> = dims
                .iter()
                .map(|d| encode(d, self.values.get(&d.name).unwrap_or(&d.default)))
                .collect();
            let mut init = vec![];
            for _ in 0..4 {
                init.push(
                    x0.iter()
                        .map(|v| (v + (self.rand() - 0.5) * 0.3).clamp(0.0, 1.0))
                        .collect(),
                );
            }
            self.note(format!("bayesian optimisation over {} knobs", dims.len()));
            self.bo = Some(BoState {
                opt: Optimizer::new(self.cfg.goal),
                dims,
                init,
                evals: 0,
                predicted: None,
            });
        }
        let collapsed = self.collapsed(now);
        if !collapsed && let Err(a) = self.ready(now) {
            return a;
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
        self.push_history(
            now,
            &eval,
            if improved {
                PointKind::Accepted
            } else {
                PointKind::Rejected
            },
        );
        let bo = self.bo.as_mut().unwrap();
        let x: Vec<f64> = bo
            .dims
            .iter()
            .map(|d| encode(d, self.values.get(&d.name).unwrap_or(&d.default)))
            .collect();
        bo.opt.observe(Observation {
            x: x.clone(),
            log_fps: eval.fps.ln(),
            psnr: eval.psnr,
        });
        bo.evals += 1;
        let pred = bo.predicted.take();
        let evals = bo.evals;
        let summary = bo
            .dims
            .iter()
            .map(|d| {
                format!(
                    "{}={}",
                    d.name.trim_start_matches("host."),
                    self.values
                        .get(&d.name)
                        .map(|v| v.display())
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        self.note(format!(
            "{} #{}: {:.0}±{:.0} fps{}{}  [{summary}]",
            if collapsed {
                "⚠ collapsed"
            } else if improved {
                "✔"
            } else {
                "·"
            },
            evals,
            eval.fps,
            2.0 * eval.noise,
            q(eval.psnr),
            pred.map(|(f, p)| format!(" (predicted {f:.0} fps{})", q(p)))
                .unwrap_or_default()
        ));

        // Next point: initial designs, a periodic re-check of the best, or the model's pick.
        let bo = self.bo.as_mut().unwrap();
        let best_x = self.best.as_ref().map(|(v, _)| {
            bo.dims
                .iter()
                .map(|d| encode(d, v.get(&d.name).unwrap_or(&d.default)))
                .collect::<Vec<f64>>()
        });
        let next = if let Some(x) = bo.init.pop() {
            x
        } else if bo.evals.is_multiple_of(7) && best_x.is_some() {
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
                    candidates.push(
                        c.iter()
                            .map(|v| (v + (rand() - 0.5) * 2.0 * spread).clamp(0.0, 1.0))
                            .collect(),
                    );
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

    pub(super) fn controller(&mut self, now: f64) -> Action {
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
        let cur = self
            .values
            .get(KNOB)
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        // Pixel cost ~ scale², so fps ∝ 1/scale²: aim for target with 5 % headroom.
        let want = (cur * (fps / (target * 1.05)).sqrt()).clamp(0.2, 1.0);
        let next = cur + (want - cur) * 0.6;
        let e = Eval {
            score: fps,
            fps,
            noise,
            psnr: None,
        };
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
