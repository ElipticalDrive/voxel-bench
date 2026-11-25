//! What the tuner is asked for: the goal, the quality metric, the algorithm and the host-side knobs.

use std::collections::BTreeSet;

use mr_core::ParamSpec;
use serde::{Deserialize, Serialize};

/// Host-side knobs every lane has, alongside its renderer's own parameters.
/// Knobs that change what is shown rather than how fast the same image is made (diagnostic
/// views; `meshes = none` drops the meshes): never tuned.
pub(super) const NEVER_TUNED: &[&str] = &["view", "view_max", "meshes"];

pub fn host_params(base_view_radius: u32) -> Vec<ParamSpec> {
    vec![
        ParamSpec::float(
            "host.render_scale",
            "Render resolution relative to the plan's (frames are upscaled for display)",
            0.2,
            1.0,
            false,
            1.0,
            true,
        ),
        ParamSpec::int(
            "host.view_radius",
            "Chunks (32 blocks each) its client holds around the camera: view distance vs memory",
            1,
            64,
            base_view_radius as i64,
            true,
        ),
        ParamSpec::int(
            "host.in_flight",
            "Frames the host lets this renderer queue",
            1,
            8,
            2,
            true,
        ),
    ]
}

// The names are the plans' words for the goals.
#[allow(clippy::enum_variant_names)]
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
    pub(super) fn degradation(&self, best: f64) -> f64 {
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
