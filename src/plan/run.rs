//! How a plan runs: frames, resolution, modes, pacing, streaming, isolation, compilation.

use mr_core::PassMode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RunSpec {
    /// Measured frames (camera paths are parametrised over this length).
    pub frames: u32,
    /// Unmeasured frames rendered first at the frame-0 camera.
    pub warmup: u32,
    pub resolution: [u32; 2],
    pub modes: Vec<PassMode>,
    /// Chunks streamed around the camera (Chebyshev radius, per LOD level).
    pub view_radius: u32,
    /// Highest LOD level streamed (capped by each renderer's own maximum).
    pub max_lod: u8,
    /// Frames a renderer may have in flight before the host waits.
    pub in_flight: u32,
    pub pacing: Pacing,
    pub streaming: Streaming,
    /// Run all renderers at the same time (e.g. side by side in the window) instead of
    /// one after another. Sequential is fairer for benchmarking.
    pub parallel: bool,
    pub isolation: Isolation,
    /// Command prefix for worker processes, e.g. `["systemd-run", "--user", "--scope",
    /// "-p", "MemoryMax=8G"]` or `["nice", "-n", "5"]`.
    pub launcher: Vec<String>,
    /// Run every renderer's pass this many times, round-robin across renderers, and report
    /// medians and spread. GPU boost clocks and temperature drift between runs (notably on
    /// laptops), so single runs can differ by tens of percent.
    pub repeats: u32,
    /// The devices renderers run on (GPU adapters by slot, CPU threads), e.g.
    /// `devices = { gpus = ["nvidia"] }`; a renderer's own `devices` overrides it.
    pub devices: mr_core::DeviceConfig,
    /// `native`: run every renderer an executor runs (CPU or GPU) as a pipeline compiled for
    /// its graph and this machine (`pipelines`), built before their pass
    /// if not cached; `break-in`: the same, built for speed whatever the build takes (native, whole-program LTO, profile-guided and both, each timed here, the fastest
    /// kept);
    /// `off`: the generic build.
    pub compile: Compile,
    /// Time every stage of every frame: each GPU stage costs a timestamp
    /// (measured: +0.1–0.6% GPU time); `false` times nothing.
    pub stage_times: bool,
    /// The rate of the simulated clock scripted runs keep: frame `n` (warm-up included) is
    /// at `n / clock_fps` seconds, the time the link's delays are measured in.
    pub clock_fps: f64,
}

impl RunSpec {
    /// The server's chunk loads per frame (`None`: every chunk in range before each frame).
    pub fn budget(&self) -> Option<usize> {
        match self.streaming {
            Streaming::Complete => None,
            Streaming::Budget { budget } => Some(budget),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compile {
    #[default]
    Off,
    Native,
    #[serde(rename = "break-in")]
    BreakIn,
}

impl Compile {
    pub fn name(self) -> &'static str {
        match self {
            Compile::Off => "off",
            Compile::Native => "native",
            Compile::BreakIn => "break-in",
        }
    }
}

impl Default for RunSpec {
    fn default() -> Self {
        Self {
            frames: 300,
            warmup: 10,
            resolution: [1280, 720],
            modes: vec![PassMode::UNLIT],
            view_radius: 12,
            max_lod: 0,
            in_flight: 2,
            pacing: Pacing::Max,
            streaming: Streaming::Complete,
            parallel: false,
            isolation: Isolation::Process,
            launcher: vec![],
            repeats: 1,
            devices: Default::default(),
            compile: Compile::Off,
            stage_times: true,
            clock_fps: 60.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Pacing {
    /// As fast as possible (bounded by `in_flight`).
    Max,
    /// Submit at a fixed rate, measuring whether renderers keep up.
    #[serde(untagged)]
    Fps { fps: f64 },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Streaming {
    /// Every chunk in range is delivered before each frame: deterministic, comparable.
    Complete,
    /// At most `budget` chunk loads per frame, nearest first: realistic pop-in.
    #[serde(untagged)]
    Budget { budget: usize },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Isolation {
    /// Each renderer in its own process (default; crash- and resource-isolated).
    Process,
    /// Renderer on a thread of the host process (for debugging/profiling in the IDE).
    Thread,
}
