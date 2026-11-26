//! Observations: one machine-readable record per measured pass (renderer × mode × scenario),
//! tagged with the machine it ran on. `run` appends them to `observations.jsonl` in the run
//! directory; `catalogue` reads any number of them back. They are the raw material for
//! deciding what each renderer is good at, and later for a planner that learns which
//! pipeline and knobs suit this machine and the scene at hand.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::features::SceneFeatures;
use crate::plan::Plan;
use crate::report::PassReport;

/// Bumped only when a field changes meaning (2: `features`; 3: `sites`, `graph`,
/// `graph_key`; 4: `stages`; 5: `stream`; 6: a pipeline is its `graph`, always, and
/// `renderer` the executor that ran it; 7: the metrics an open map, `metrics` and `methods`).
/// A new metric is not a new version: it is an entry in `crate::metrics`. Records of
/// pipelines from before version 6 are not read; records of version 6 read with their
/// metrics moved into the map (`parse`).
pub const VERSION: u32 = 7;

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
    /// The build it ran as when compiled for this machine (`native`, `break-in`); none: the
    /// generic build, as sweeps run by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled: Option<String>,
    pub repeat: u32,
    pub error: Option<String>,
    /// The lanes of its pass that failed part-way (its timings shared the machine with them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beside_failed: Vec<String>,
    /// What the plan asked of it that it lacks and ran without.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_supported: Vec<String>,
    /// What was measured, by metric (`crate::metrics`): missing = not measured; a name this
    /// build does not know is kept and shown raw. `methods` holds each one's method version,
    /// so a changed way of measuring is told apart from a changed speed (version 7).
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub methods: BTreeMap<String, u32>,
    /// The machine's conditions when its frames ended (clocks, power state, power source; the
    /// numbers are metrics too).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<crate::conditions::Conditions>,
    /// What it is given of the world when less than the reference (its LOD region): the
    /// `*_like` metrics are scored against a reference given the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like: Option<String>,
    /// Its graph's edges that arrive a frame late (the `*_settled` metrics are its frames
    /// once they caught up).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub late: Vec<String>,
    /// The renderer's own counters, averaged over the frames.
    pub counters: BTreeMap<String, f64>,
    /// Time per stage (version 4).
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
        let (metrics, methods) = crate::metrics::measure(&serde_json::to_value(r).unwrap_or_default());
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
            compiled: r.compiled.clone(),
            repeat: r.repeat,
            error: r.error.clone(),
            beside_failed: r.beside_failed.clone(),
            not_supported: r.not_supported.clone(),
            metrics,
            methods,
            conditions: r.conditions.clone(),
            like: r.like.clone(),
            late: r.late.clone(),
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
    /// The pipeline it measured (`None` for standalone renderers).
    pub fn pipeline(&self) -> Option<mr_components::graph::Graph> {
        // Records keep the knobs that spell timings (discover-cpu `pipelined`): they load as the
        // graphs they ran.
        let options: toml::Table = serde_json::from_value(self.options.clone()).unwrap_or_default();
        self.graph.clone().map(|g| g.with_options(&options))
    }

    /// The record in `line`; `None` for one this build no longer reads: a pipeline's from
    /// before version 6 (named in a form since removed). A standalone renderer's reads at
    /// any version; records before version 7 read with their metrics moved into the map
    /// (`metrics::from_v6`, method 1). A record newer than this build ([`VERSION`]) is an
    /// error naming both versions: a field may mean something this build would misread (a
    /// new metric is not a new version, and reads).
    pub fn parse(line: &str) -> serde_json::Result<Option<Self>> {
        if let Some(why) = newer(line) {
            return Err(serde::de::Error::custom(why));
        }
        let mut v: serde_json::Value = serde_json::from_str(line)?;
        let version = v["version"].as_u64().unwrap_or(0);
        if version < 6 && !(v["graph"].is_null() && v["composition"].is_null()) {
            return Ok(None);
        }
        if version < 7 {
            crate::metrics::from_v6(&mut v);
        }
        serde_json::from_value(v).map(Some)
    }

    /// The value of metric `name`, if measured.
    pub fn metric(&self, name: &str) -> Option<f64> {
        self.metrics.get(name).copied()
    }
}

/// Why the record in `line` is refused, if it is newer than this build ([`VERSION`]).
pub fn newer(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let n = v["version"].as_u64().filter(|n| *n > VERSION as u64)?;
    Some(format!("an observation of version {n}, newer than this build reads (version {VERSION}): rebuild"))
}

/// Writes the run's observations (every pass except the reference's) as JSON lines.
/// A report's observation as its line (no newline); none for a reference.
pub fn line(r: &PassReport, plan: &Plan, run: &str, machine: &Machine, features: Option<&SceneFeatures>) -> anyhow::Result<Option<String>> {
    if r.label.ends_with("(ref)") {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&Observation::new(r, plan, run, machine, features))?))
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
