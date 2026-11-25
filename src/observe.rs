//! Observations: one machine-readable record per measured pass (renderer × mode × scenario),
//! tagged with the machine it ran on. `run` appends them to `observations.jsonl` in the run
//! directory; `catalogue` reads any number of them back. They are the raw material for
//! deciding what each renderer is good at, and later for a planner that learns which
//! pipeline and knobs suit this machine and the scene at hand.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::features::SceneFeatures;
use crate::plan::Plan;
use crate::report::PassReport;

/// Bumped when fields change meaning or are added (2: `features`; 3: `sites`, `graph`,
/// `graph_key`; 4: `stages`; 5: `stream`; 6: a pipeline is its `graph`, always, and
/// `renderer` the executor that ran it). Records of pipelines from before version 6 are not
/// read (`parse`).
pub const VERSION: u32 = 6;

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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
    /// What rendered it: the label, the executor (`cpu`, `gpu`; a standalone renderer's
    /// name), the pipeline's graph (none for standalone renderers) and the options (knobs).
    pub label: String,
    pub renderer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<mr_components::graph::Graph>,
    /// The canonical graph's key when it was written: records of one pipeline share it
    /// (`pipeline()`'s key, or `multirenderer graph --keys`, give today's).
    #[serde(default)]
    pub graph_key: Option<String>,
    pub options: serde_json::Value,
    pub repeat: u32,
    pub error: Option<String>,
    /// The lanes of its pass that failed part-way (its timings shared the machine with them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beside_failed: Vec<String>,
    /// What the plan asked of it that it lacks and ran without.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_supported: Vec<String>,
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
    /// Counted by the renderer (what it sizes) and measured (its devices' allocators at their
    /// peak; a few older records hold the pass's end).
    pub gpu_mb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_device_mb: Option<f64>,
    /// Against the reference: mean and worst PSNR, mean share of bad pixels.
    pub psnr: Option<f64>,
    pub psnr_min: Option<f64>,
    pub bad_pct: Option<f64>,
    /// What it is given of the world when less than the reference (its LOD region), and its
    /// mean PSNR and bad pixels against a reference given the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub psnr_like: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bad_pct_like: Option<f64>,
    /// Its graph's edges that arrive a frame late, and its mean PSNR and bad pixels once they
    /// caught up (the compared frames held still; against the like reference where it has one).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub late: Vec<String>,
    /// For one that accumulates lit frames: the paths per pixel its settled frames averaged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converged_spp: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub psnr_settled: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bad_pct_settled: Option<f64>,
    /// The renderer's own counters, averaged over the frames.
    pub counters: BTreeMap<String, f64>,
    /// Time per stage (version 4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<ObservedStage>,
    /// The stream to its client (version 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<ObservedStream>,
}

/// What crossed the link and what was missing (`PassReport`'s fields of the same names).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservedStream {
    pub wire_bytes: u64,
    pub arrived_bytes: u64,
    pub update_ns_per_byte: Option<f64>,
    /// Mean and largest chunks missing per frame, and the frames with any.
    pub holes: f64,
    pub holes_max: f64,
    pub frames_with_holes: usize,
    /// The first frame with nothing missing: its number and wall time from the pass's start.
    pub first_complete_frame: Option<u32>,
    pub first_complete_ms: Option<f64>,
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
            graph: r.graph.clone(),
            graph_key: r.graph_key.clone(),
            options: serde_json::to_value(&r.options).unwrap_or_default(),
            repeat: r.repeat,
            error: r.error.clone(),
            beside_failed: r.beside_failed.clone(),
            not_supported: r.not_supported.clone(),
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
            gpu_device_mb: (r.memory.gpu_device_bytes > 0).then(|| r.memory.gpu_device_bytes as f64 / 1048576.0),
            psnr: q.map(|q| q.psnr.mean),
            psnr_min: q.map(|q| q.psnr.min),
            bad_pct: q.map(|q| q.bad_pct.mean),
            like: r.like.clone(),
            psnr_like: r.quality_like.as_ref().map(|q| q.psnr.mean),
            bad_pct_like: r.quality_like.as_ref().map(|q| q.bad_pct.mean),
            late: r.late.clone(),
            converged_spp: r.converged_spp,
            psnr_settled: r.quality_settled.as_ref().map(|q| q.psnr.mean),
            bad_pct_settled: r.quality_settled.as_ref().map(|q| q.bad_pct.mean),
            counters: sums.into_iter().map(|(k, (s, n))| (k, s / n.max(1) as f64)).collect(),
            stages: Vec::new(),
            stream: (r.error.is_none()).then(|| ObservedStream {
                wire_bytes: r.wire_bytes,
                arrived_bytes: r.arrived_bytes,
                update_ns_per_byte: r.update_ns_per_byte,
                holes: r.holes.as_ref().map_or(0.0, |h| h.mean),
                holes_max: r.holes.as_ref().map_or(0.0, |h| h.max),
                frames_with_holes: r.frames_with_holes,
                first_complete_frame: r.first_complete.map(|c| c.frame),
                first_complete_ms: r.first_complete.map(|c| c.ms),
            }),
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
    /// The pipeline it measured (`None` for standalone renderers).
    pub fn pipeline(&self) -> Option<mr_components::graph::Graph> {
        // Records keep the knobs that spell timings (discover-cpu `pipelined`): they load as the
        // graphs they ran.
        let options: toml::Table = serde_json::from_value(self.options.clone()).unwrap_or_default();
        self.graph.clone().map(|g| g.with_options(&options))
    }

    /// The record in `line`; `None` for one this build no longer reads: a pipeline's from
    /// before version 6 (named in a form since removed). A standalone renderer's reads at
    /// any version. A record newer than this build ([`VERSION`]) is an error naming both
    /// versions: what it holds may mean something this build would misread.
    pub fn parse(line: &str) -> serde_json::Result<Option<Self>> {
        let older = |v: &serde_json::Value| v["version"].as_u64().is_some_and(|n| n < VERSION as u64);
        if let Some(why) = newer(line) {
            return Err(serde::de::Error::custom(why));
        }
        match serde_json::from_str::<Self>(line) {
            Ok(o) if o.version >= VERSION => Ok(Some(o)),
            Ok(o) => {
                let v: serde_json::Value = serde_json::from_str(line)?;
                Ok((v["graph"].is_null() && v["composition"].is_null()).then_some(o))
            }
            Err(_) if serde_json::from_str::<serde_json::Value>(line).is_ok_and(|v| older(&v)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Why the record in `line` is refused, if it is newer than this build ([`VERSION`]).
pub fn newer(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let n = v["version"].as_u64().filter(|n| *n > VERSION as u64)?;
    Some(format!("an observation of version {n}, newer than this build reads (version {VERSION}): rebuild"))
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

/// The records of `files`, and how many were left out: those this build no longer reads
/// (`Observation::parse`). Anything else unreadable is an error.
pub fn read(files: &[PathBuf]) -> anyhow::Result<(Vec<Observation>, usize)> {
    let (mut out, mut left_out) = (vec![], 0);
    for f in files {
        let text = std::fs::read_to_string(f)?;
        for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            match Observation::parse(line) {
                Ok(Some(o)) => out.push(o),
                Ok(None) => left_out += 1,
                Err(e) => anyhow::bail!("{}:{}: {e}", f.display(), i + 1),
            }
        }
    }
    Ok((out, left_out))
}
