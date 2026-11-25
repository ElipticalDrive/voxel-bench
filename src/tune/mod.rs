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

mod algorithms;
mod config;
#[cfg(test)]
mod tests;
mod tuner;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mr_core::{ParamSpec, ParamValue};

pub use config::{Algorithm, Goal, QualityMetric, TunerConfig, host_params};

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
    Trial {
        knob: String,
        revert: ParamValue,
        value: ParamValue,
    },
    /// A rejected trial was undone; wait for things to settle, then try another.
    Reverting,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Explore,
    /// Settled on the best configuration; watching for changes.
    Hold {
        violations: u32,
        windows: u32,
    },
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

fn q(p: Option<f64>) -> String {
    p.map(|p| format!(", {:.1} dB", p.min(99.0)))
        .unwrap_or_default()
}
