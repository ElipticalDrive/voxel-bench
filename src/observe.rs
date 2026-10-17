//! Observations: one machine-readable record per measured pass (renderer × mode × scenario),
//! tagged with the machine it ran on. `run` appends them to `observations.jsonl` in the run
//! directory; `catalogue` reads any number of them back. They are the raw material for
//! deciding what each renderer is good at, and later for a planner that learns which
//! composition and knobs suit this machine and the scene at hand (IDEAS §14).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::features::SceneFeatures;
use crate::plan::Plan;
use crate::report::PassReport;

/// Bumped when fields change meaning or are added (2: `features`; 3: `sites`, `graph`,
/// `graph_key`; 4: `stages`).
pub const VERSION: u32 = 4;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    pub cpu: String,
    pub threads: usize,
    pub memory_gb: f64,
    pub gpu: String,
    pub gpu_driver: String,
    pub gpu_backend: String,
    pub os: String,
}

impl Machine {
    /// This machine, as far as it can be found out cheaply (missing parts stay empty).
    pub fn detect() -> Self {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let cpu = cpuinfo.lines().find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string())).unwrap_or_default();
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let kb = meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok()).unwrap_or(0.0);
        let os = std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|s| format!("{} {}", std::env::consts::OS, s.trim())).unwrap_or_else(|_| std::env::consts::OS.into());
        let gpu = mr_gpu::adapter_info(None).ok();
        Self {
            cpu,
            threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
            memory_gb: (kb / 1048576.0 * 10.0).round() / 10.0,
            gpu: gpu.as_ref().map(|g| g.name.clone()).unwrap_or_default(),
            gpu_driver: gpu.as_ref().map(|g| format!("{} {}", g.driver, g.driver_info).trim().to_string()).unwrap_or_default(),
            gpu_backend: gpu.as_ref().map(|g| format!("{:?}", g.backend)).unwrap_or_default(),
            os,
        }
    }

    /// Short identity for grouping: CPU and GPU.
    pub fn key(&self) -> String {
        format!("{} + {}", self.cpu, self.gpu)
    }
}

/// One measured pass.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub version: u32,
    /// The run directory's name (UTC time stamp and plan name).
    pub run: String,
    pub machine: Machine,
    /// The scenario: the plan's name and scene (generator, camera path, lights, events,
    /// meshes) and how it was run.
    pub plan: String,
    pub scene: serde_json::Value,
    /// Measured properties of the scenario (renderer-independent; absent before version 2).
    #[serde(default)]
    pub features: Option<SceneFeatures>,
    pub resolution: [u32; 2],
    pub view_radius: u32,
    pub frames: u32,
    pub streaming: String,
    pub mode: String,
    /// What rendered it: the label, the renderer entry, the composition (if composed) and
    /// the options beyond the composition (knobs).
    pub label: String,
    pub renderer: String,
    pub composition: Option<mr_components::Composition>,
    /// Sites bound otherwise (PLAN step 9d; absent before version 3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sites: BTreeMap<String, String>,
    /// The graph, when composition and sites cannot express it (version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<mr_components::graph::Graph>,
    /// The canonical graph's key: records of one pipeline share it (version 3; older
    /// records: `pipeline()`'s key, or `multirenderer graph --keys`).
    #[serde(default)]
    pub graph_key: Option<String>,
    pub options: serde_json::Value,
    pub repeat: u32,
    pub error: Option<String>,
    /// Throughput and pacing: completed frames per second, and the time between
    /// completions (mean, p50, p99).
    pub fps: f64,
    pub frame_ms: Option<f64>,
    pub frame_p50_ms: Option<f64>,
    pub frame_p99_ms: Option<f64>,
    /// Means over the frames: GPU execution time (timestamp queries), the renderer's CPU
    /// time, and the host-measured latency of a frame.
    pub gpu_ms: Option<f64>,
    pub cpu_ms: Option<f64>,
    pub latency_ms: Option<f64>,
    pub init_ms: f64,
    pub initial_load_ms: f64,
    pub peak_rss_mb: Option<f64>,
    pub gpu_mb: Option<f64>,
    /// Against the reference: mean and worst PSNR, mean share of bad pixels.
    pub psnr: Option<f64>,
    pub psnr_min: Option<f64>,
    pub bad_pct: Option<f64>,
    /// The renderer's own counters, averaged over the frames.
    pub counters: BTreeMap<String, f64>,
    /// Time per stage (PLAN step 11; version 4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<ObservedStage>,
}

/// A stage's mean times over the frames that ran it, and the nodes it ran.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservedStage {
    pub name: String,
    /// Node ids in the pipeline's graph, and what each is (`role.component`).
    pub nodes: Vec<String>,
    pub components: Vec<String>,
    pub frames: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_ms: Option<f64>,
}

impl Observation {
    pub fn new(r: &PassReport, plan: &Plan, run: &str, machine: &Machine, features: Option<&SceneFeatures>) -> Self {
        let mut sums: BTreeMap<String, (f64, u32)> = BTreeMap::new();
        for f in &r.per_frame {
            for (k, v) in &f.counters {
                let e = sums.entry(k.clone()).or_default();
                e.0 += v;
                e.1 += 1;
            }
        }
        let q = r.quality.as_ref();
        Self {
            version: VERSION,
            run: run.into(),
            machine: machine.clone(),
            plan: plan.name.clone(),
            scene: serde_json::to_value(&plan.scene).unwrap_or_default(),
            features: features.cloned(),
            resolution: r.resolution,
            view_radius: r.view_radius,
            frames: plan.run.frames,
            streaming: match plan.run.streaming {
                crate::plan::Streaming::Complete => "complete".into(),
                crate::plan::Streaming::Budget { budget } => format!("budget {budget}"),
            },
            mode: r.mode.map(|m| m.name().to_string()).unwrap_or_default(),
            label: r.label.clone(),
            renderer: r.renderer.clone(),
            composition: r.composition.clone(),
            sites: r.sites.clone(),
            graph: r.graph.clone(),
            graph_key: r.graph_key.clone(),
            options: serde_json::to_value(&r.options).unwrap_or_default(),
            repeat: r.repeat,
            error: r.error.clone(),
            fps: r.fps,
            frame_ms: r.frame_interval_ms.as_ref().map(|s| s.mean),
            frame_p50_ms: r.frame_interval_ms.as_ref().map(|s| s.p50),
            frame_p99_ms: r.frame_interval_ms.as_ref().map(|s| s.p99),
            gpu_ms: r.gpu_ms.as_ref().map(|s| s.mean),
            cpu_ms: r.cpu_ms.as_ref().map(|s| s.mean),
            latency_ms: r.latency_ms.as_ref().map(|s| s.mean),
            init_ms: r.init_ms as f64,
            initial_load_ms: r.initial_load_ms,
            peak_rss_mb: r.peak_rss_mb,
            gpu_mb: (r.memory.gpu_bytes > 0).then(|| r.memory.gpu_bytes as f64 / 1048576.0),
            psnr: q.map(|q| q.psnr.mean),
            psnr_min: q.map(|q| q.psnr.min),
            bad_pct: q.map(|q| q.bad_pct.mean),
            counters: sums.into_iter().map(|(k, (s, n))| (k, s / n.max(1) as f64)).collect(),
            stages: Vec::new(),
        }
        .with_stages(r)
    }

    fn with_stages(mut self, r: &PassReport) -> Self {
        let g = self.pipeline();
        let what = |id: &String| g.as_ref().and_then(|g| g.nodes.get(id)).map_or_else(|| id.clone(), |n| format!("{}.{}", n.role.name(), n.component));
        self.stages = r
            .stages
            .iter()
            .map(|s| ObservedStage {
                name: s.name.clone(),
                components: s.nodes.iter().map(what).collect(),
                nodes: s.nodes.clone(),
                frames: s.frames,
                cpu_ms: s.cpu_ms.as_ref().map(|x| x.mean),
                gpu_ms: s.gpu_ms.as_ref().map(|x| x.mean),
            })
            .collect();
        self
    }
}

impl Observation {
    /// The pipeline it measured: its graph, or its composition with its sites (`None` for
    /// standalone renderers).
    pub fn pipeline(&self) -> Option<mr_components::graph::Graph> {
        // Records keep the knobs that spell timings (cpu-discover `pipelined`) and, before
        // step 13b, hole detection (`incremental`): they load as the graphs they ran.
        let options: toml::Table = serde_json::from_value(self.options.clone()).unwrap_or_default();
        let incremental = options.get("incremental").and_then(|v| v.as_bool()) == Some(true);
        if let Some(g) = self.graph.as_ref().filter(|g| !incremental || g.nodes.values().any(|n| n.role == mr_components::graph::Role::Holes)) {
            return Some(g.clone().with_options(&options));
        }
        let mut c = self.composition.clone()?;
        if c.backend == "gpu-discover" && incremental && (c.holes.is_empty() || c.holes == "none") {
            c.holes = "coverage".into();
        }
        mr_components::graph::Graph::from_composition(&c).and_then(|g| g.with_sites(&self.sites)).ok().map(|g| g.with_options(&options))
    }
}

/// Writes the run's observations (every pass except the reference's) as JSON lines.
pub fn write(path: &Path, reports: &[PassReport], plan: &Plan, run: &str, machine: &Machine, features: Option<&SceneFeatures>) -> anyhow::Result<usize> {
    let mut text = String::new();
    let mut n = 0;
    for r in reports.iter().filter(|r| !r.label.ends_with("(ref)")) {
        text += &serde_json::to_string(&Observation::new(r, plan, run, machine, features))?;
        text.push('\n');
        n += 1;
    }
    std::fs::write(path, text)?;
    Ok(n)
}

/// Every `observations.jsonl` under `paths` (files, or directories searched recursively).
pub fn find(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = vec![];
    for p in paths {
        if p.is_file() {
            out.push(p.clone());
        } else if let Ok(rd) = std::fs::read_dir(p) {
            let mut sub: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            sub.sort();
            for s in sub {
                if s.is_dir() {
                    out.extend(find(&[s]));
                } else if s.file_name().is_some_and(|n| n == "observations.jsonl") {
                    out.push(s);
                }
            }
        }
    }
    out
}

pub fn read(files: &[PathBuf]) -> anyhow::Result<Vec<Observation>> {
    let mut out = vec![];
    for f in files {
        let text = std::fs::read_to_string(f)?;
        for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            out.push(serde_json::from_str(line).map_err(|e| anyhow::anyhow!("{}:{}: {e}", f.display(), i + 1))?);
        }
    }
    Ok(out)
}
