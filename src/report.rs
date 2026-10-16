use std::fmt::Write as _;

use mr_core::{MemoryReport, PassMode, StageTime};
use serde::Serialize;

use crate::compare::Quality;

#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Summary {
    pub mean: f64,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

impl Summary {
    pub fn of(values: impl IntoIterator<Item = f64>) -> Option<Self> {
        let mut v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(f64::total_cmp);
        let pct = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        Some(Self {
            mean: v.iter().sum::<f64>() / v.len() as f64,
            min: v[0],
            p50: pct(0.5),
            p95: pct(0.95),
            p99: pct(0.99),
            max: v[v.len() - 1],
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FrameRecord {
    pub frame: u32,
    /// Host-measured: Render sent -> FrameDone received.
    pub latency_ms: f64,
    pub cpu_ms: f32,
    pub gpu_ms: Option<f32>,
    pub update_ms: f32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub counters: Vec<(String, f64)>,
    /// Time per stage and the graph nodes each ran (PLAN step 11).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<StageTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

/// One stage over a pass: its times over the frames that ran it (a stage's CPU and GPU
/// parts, and repeats within a frame, summed per frame).
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct StageSummary {
    pub name: String,
    pub nodes: Vec<String>,
    /// Frames that ran the stage (some run only in some frames, e.g. a storage rebuild).
    pub frames: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_ms: Option<Summary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_ms: Option<Summary>,
}

impl StageSummary {
    /// The stages of `frames`, in the order they first ran.
    pub fn of<'a>(frames: impl IntoIterator<Item = &'a [StageTime]>) -> Vec<StageSummary> {
        type Times = (Vec<f64>, Vec<f64>);
        let mut order: Vec<(String, Vec<String>)> = Vec::new();
        let mut times: std::collections::HashMap<(String, Vec<String>), Times> = Default::default();
        for stages in frames {
            let mut frame: std::collections::HashMap<(String, Vec<String>), (Option<f64>, Option<f64>)> = Default::default();
            for s in stages {
                let k = (s.name.clone(), s.nodes.clone());
                if !times.contains_key(&k) {
                    order.push(k.clone());
                    times.insert(k.clone(), Default::default());
                }
                let e = frame.entry(k).or_default();
                let add = |t: &mut Option<f64>, v: Option<f32>| {
                    if let Some(v) = v {
                        *t = Some(t.unwrap_or(0.0) + v as f64);
                    }
                };
                add(&mut e.0, s.cpu_ms);
                add(&mut e.1, s.gpu_ms);
            }
            for (k, (c, g)) in frame {
                let t = times.get_mut(&k).unwrap();
                t.0.extend(c);
                t.1.extend(g);
            }
        }
        order
            .into_iter()
            .map(|k| {
                let (c, g) = &times[&k];
                StageSummary { frames: c.len().max(g.len()), cpu_ms: Summary::of(c.iter().copied()), gpu_ms: Summary::of(g.iter().copied()), name: k.0, nodes: k.1 }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct QualitySummary {
    pub frames: usize,
    pub psnr: Summary,
    pub mae: Summary,
    pub bad_pct: Summary,
    pub worst_frame: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PassReport {
    pub label: String,
    pub renderer: String,
    /// The composition and the options beyond it (knobs), when composed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composition: Option<mr_components::Composition>,
    /// Sites bound otherwise than the composition binds them (PLAN step 9d).
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub sites: std::collections::BTreeMap<String, String>,
    /// The graph run, when it has no sites form (else composition and sites give it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<mr_components::graph::Graph>,
    /// The canonical graph's key (`Graph::key`): what the pass ran, for comparisons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_key: Option<String>,
    #[serde(skip_serializing_if = "toml::Table::is_empty")]
    pub options: toml::Table,
    pub view_radius: u32,
    pub mode: Option<PassMode>,
    pub tags: Vec<String>,
    pub resolution: [u32; 2],
    pub init_ms: f32,
    pub initial_load_ms: f64,
    pub frames: usize,
    pub wall_s: f64,
    pub fps: f64,
    /// Time between consecutive frame completions.
    pub frame_interval_ms: Option<Summary>,
    pub latency_ms: Option<Summary>,
    pub cpu_ms: Option<Summary>,
    pub gpu_ms: Option<Summary>,
    pub update_ms: Option<Summary>,
    /// Time per stage (PLAN step 11).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<StageSummary>,
    pub missed_deadlines: Option<u32>,
    pub chunks_sent: u64,
    pub peak_rss_mb: Option<f64>,
    pub memory: MemoryReport,
    pub quality: Option<QualitySummary>,
    pub error: Option<String>,
    /// Which repetition this pass is (0-based) when `run.repeats > 1`.
    pub repeat: u32,
    pub per_frame: Vec<FrameRecord>,
}

impl PassReport {
    pub fn finish(&mut self, completions: &[f64]) {
        let f = &self.per_frame;
        self.frames = f.len();
        self.latency_ms = Summary::of(f.iter().map(|r| r.latency_ms));
        self.cpu_ms = Summary::of(f.iter().map(|r| r.cpu_ms as f64));
        self.gpu_ms = Summary::of(f.iter().filter_map(|r| r.gpu_ms.map(f64::from)));
        self.update_ms = Summary::of(f.iter().map(|r| r.update_ms as f64));
        self.stages = StageSummary::of(f.iter().map(|r| &r.stages[..]));
        self.frame_interval_ms = Summary::of(completions.windows(2).map(|w| w[1] - w[0]));
        if self.wall_s > 0.0 {
            self.fps = self.frames as f64 / self.wall_s;
        }
        let q: Vec<(u32, Quality)> = f.iter().filter_map(|r| r.quality.map(|q| (r.frame, q))).collect();
        if !q.is_empty() {
            let worst = q.iter().max_by(|a, b| a.1.mae.total_cmp(&b.1.mae)).unwrap().0;
            self.quality = Some(QualitySummary {
                frames: q.len(),
                // Identical frames have infinite PSNR; cap at 99 dB so summaries stay finite.
                psnr: Summary::of(q.iter().map(|(_, x)| x.psnr.min(99.0))).unwrap(),
                mae: Summary::of(q.iter().map(|(_, x)| x.mae)).unwrap(),
                bad_pct: Summary::of(q.iter().map(|(_, x)| x.bad_pct)).unwrap(),
                worst_frame: worst,
            });
        }
    }
}

pub fn table(reports: &[PassReport]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "{:<22} {:<6} {:>8} {:>9} {:>9} {:>9} {:>8} {:>9} {:>8} {:>7} {:>7} {:>8}",
        "renderer", "mode", "fps", "frame ms", "p99 ms", "gpu ms", "init ms", "load ms", "PSNR dB", "bad %", "RSS MB", "GPU MB"
    );
    for r in reports {
        let mode = r.mode.map(|m| m.name()).unwrap_or("-");
        if let Some(e) = &r.error {
            let _ = writeln!(s, "{:<22} {:<6} FAILED: {}", r.label, mode, e.lines().next().unwrap_or(""));
            continue;
        }
        let o = |v: Option<f64>, p: usize| v.map(|x| format!("{x:.p$}")).unwrap_or_else(|| "-".into());
        let interval = r.frame_interval_ms.as_ref();
        let _ = writeln!(
            s,
            "{:<22} {:<6} {:>8.1} {:>9} {:>9} {:>9} {:>8.0} {:>9.0} {:>8} {:>7} {:>7} {:>8}",
            r.label,
            mode,
            r.fps,
            o(interval.map(|x| x.mean), 2),
            o(interval.map(|x| x.p99), 2),
            o(r.gpu_ms.as_ref().map(|x| x.mean), 2),
            r.init_ms,
            r.initial_load_ms,
            o(r.quality.as_ref().map(|q| q.psnr.mean), 1),
            o(r.quality.as_ref().map(|q| q.bad_pct.mean), 2),
            o(r.peak_rss_mb, 0),
            o((r.memory.gpu_bytes > 0).then(|| r.memory.gpu_bytes as f64 / 1048576.0), 0),
        );
    }
    s
}

/// Mean time per stage of each pass (PLAN step 11), `None` when no pass reported stages.
pub fn stage_table(reports: &[PassReport]) -> Option<String> {
    let mut s = String::new();
    for r in reports.iter().filter(|r| r.error.is_none() && !r.stages.is_empty()) {
        let _ = writeln!(s, "{} ({}{})", r.label, r.mode.map(|m| m.name()).unwrap_or("-"), if r.repeat > 0 { format!(", run {}", r.repeat + 1) } else { String::new() });
        for st in &r.stages {
            let ms = |x: &Option<Summary>, what| x.as_ref().map(|x| format!(" {what} {:>7.3} ms", x.mean)).unwrap_or_default();
            let part = if st.frames < r.frames { format!(" ({} of {} frames)", st.frames, r.frames) } else { String::new() };
            let _ = writeln!(s, "  {:<14} {:<28}{}{}{part}", st.name, st.nodes.join(" + "), ms(&st.gpu_ms, "gpu"), ms(&st.cpu_ms, "cpu"));
        }
    }
    (!s.is_empty()).then_some(s)
}

/// Medians and spread over repeated passes of the same (renderer, mode).
pub fn repeat_table(reports: &[PassReport]) -> Option<String> {
    if reports.iter().all(|r| r.repeat == 0) {
        return None;
    }
    let mut keys: Vec<(String, Option<PassMode>)> = vec![];
    for r in reports.iter().filter(|r| r.error.is_none() && !r.label.ends_with("(ref)")) {
        if !keys.contains(&(r.label.clone(), r.mode)) {
            keys.push((r.label.clone(), r.mode));
        }
    }
    let med = |mut v: Vec<f64>| -> Option<(f64, f64, f64)> {
        if v.is_empty() {
            return None;
        }
        v.sort_by(f64::total_cmp);
        Some((v[v.len() / 2], v[0], v[v.len() - 1]))
    };
    let mut s = String::new();
    let _ = writeln!(s, "{:<22} {:<6} {:>4} {:>24} {:>24} {:>8}", "renderer", "mode", "runs", "fps median [min–max]", "gpu ms median [min–max]", "spread");
    for (label, mode) in keys {
        let runs: Vec<&PassReport> = reports.iter().filter(|r| r.label == label && r.mode == mode && r.error.is_none()).collect();
        let fps = med(runs.iter().map(|r| r.fps).collect());
        let gpu = med(runs.iter().filter_map(|r| r.gpu_ms.as_ref().map(|g| g.mean)).collect());
        let f = |x: Option<(f64, f64, f64)>, p: usize| x.map(|(m, lo, hi)| format!("{m:.p$} [{lo:.p$}–{hi:.p$}]")).unwrap_or("-".into());
        let spread = fps.map(|(m, lo, hi)| format!("±{:.0}%", 50.0 * (hi - lo) / m.max(1e-9))).unwrap_or_default();
        let _ = writeln!(s, "{:<22} {:<6} {:>4} {:>24} {:>24} {:>8}", label, mode.map(|m| m.name()).unwrap_or("-"), runs.len(), f(fps, 1), f(gpu, 2), spread);
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(name: &str, cpu: Option<f32>, gpu: Option<f32>) -> StageTime {
        StageTime { name: name.into(), nodes: vec![name.into()], cpu_ms: cpu, gpu_ms: gpu }
    }

    /// A stage's CPU and GPU parts and its repeats within a frame add up per frame; stages
    /// keep the order they first ran in and count the frames that ran them.
    #[test]
    fn stage_summaries() {
        let frames = [
            vec![st("cast", Some(1.0), None), st("geometry", Some(2.0), None), st("geometry", None, Some(0.5))],
            vec![st("cast", Some(3.0), None), st("storage", Some(1.0), None), st("storage", Some(1.0), None)],
        ];
        let s = StageSummary::of(frames.iter().map(|f| &f[..]));
        let got: Vec<(&str, usize, Option<f64>, Option<f64>)> = s.iter().map(|s| (s.name.as_str(), s.frames, s.cpu_ms.as_ref().map(|x| x.mean), s.gpu_ms.as_ref().map(|x| x.mean))).collect();
        assert_eq!(got, [("cast", 2, Some(2.0), None), ("geometry", 1, Some(2.0), Some(0.5)), ("storage", 1, Some(2.0), None)]);
    }
}
