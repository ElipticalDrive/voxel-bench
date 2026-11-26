//! The metrics: everything a pass's record says was measured, named and described in one
//! place. An observation carries an open map of them (`metrics`, with each one's `methods`
//! version beside it), so a new metric is one entry here: its name, unit, which way is
//! better, the version of how it is measured, a description, and where the pass's report
//! holds it. Every consumer (the catalogue, the GUI's report table, `multirenderer metrics`
//! and through it the guard and `records.py`) iterates this list; a record holding a metric
//! this build does not know is still read, and its value shown raw.
//!
//! `method` is bumped when how a metric is measured changes (another timer, another
//! reference, another averaging), so a changed method is not mistaken for a changed
//! machine or a faster pipeline: the guard compares a metric only between records that
//! measured it the same way. Records before observation version 7 held each metric as a
//! field of its own (`v6`, a dotted path); they read as method 1.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

/// Which way a metric is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Better {
    Higher,
    Lower,
    /// Neither: a property of the pass rather than a merit (paths accumulated, bytes arrived).
    Neither,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Metric {
    pub name: &'static str,
    pub unit: &'static str,
    pub better: Better,
    /// The version of how it is measured (1 for each metric as it stood at observation v6).
    pub method: u32,
    pub description: &'static str,
    /// Where the pass's report (`PassReport` as JSON) holds it.
    pub report: &'static [&'static str],
    /// Multiplies the report's value (bytes to MB).
    pub scale: f64,
    /// A zero in the report means "not measured" (memory the renderer did not count).
    pub zero_unmeasured: bool,
    /// Where an observation of version 6 or older held it (a dotted path; empty: it did not).
    pub v6: &'static str,
    /// Decimals when shown.
    pub decimals: usize,
}

const MB: f64 = 1.0 / 1048576.0;

const fn m(name: &'static str, unit: &'static str, better: Better, report: &'static [&'static str], v6: &'static str, decimals: usize, description: &'static str) -> Metric {
    Metric { name, unit, better, method: 1, description, report, scale: 1.0, zero_unmeasured: false, v6, decimals }
}

use Better::*;

/// Every metric, in the order they are shown.
pub const METRICS: &[Metric] = &[
    m("fps", "frames/s", Higher, &["fps"], "fps", 1, "Frames completed per second of wall time over the pass"),
    m("frame_ms", "ms", Lower, &["frame_interval_ms", "mean"], "frame_ms", 2, "Mean time between completed frames"),
    m("frame_p50_ms", "ms", Lower, &["frame_interval_ms", "p50"], "frame_p50_ms", 2, "Median time between completed frames"),
    m("frame_p99_ms", "ms", Lower, &["frame_interval_ms", "p99"], "frame_p99_ms", 2, "99th-percentile time between completed frames (hitches)"),
    m("gpu_ms", "ms", Lower, &["gpu_ms", "mean"], "gpu_ms", 2, "Mean GPU execution time per frame (timestamp queries)"),
    m("cpu_ms", "ms", Lower, &["cpu_ms", "mean"], "cpu_ms", 2, "Mean renderer CPU time per frame"),
    m("latency_ms", "ms", Lower, &["latency_ms", "mean"], "latency_ms", 2, "Mean time from a frame's request to its completion, measured by the host"),
    m("init_ms", "ms", Lower, &["init_ms"], "init_ms", 0, "Renderer start-up time (device, pipelines)"),
    m("shader_module_ms", "ms", Lower, &["shaders", "module_ms"], "", 0, "The part of start-up spent composing and validating shader modules"),
    m("shader_pipeline_ms", "ms", Lower, &["shaders", "pipeline_ms"], "", 0, "The part of start-up spent creating pipelines (the driver's compile)"),
    m("initial_load_ms", "ms", Lower, &["initial_load_ms"], "initial_load_ms", 0, "Time to load the world before the first frame"),
    m("update_ms", "ms", Lower, &["update_ms", "mean"], "", 2, "Mean time applying world updates per frame (chunk uploads, edits, lights)"),
    m("missed_deadlines", "frames", Lower, &["missed_deadlines"], "", 0, "Frames not done by their deadline in a paced pass"),
    m("peak_rss_mb", "MB", Lower, &["peak_rss_mb"], "peak_rss_mb", 0, "Peak resident memory of the renderer process"),
    Metric { scale: MB, zero_unmeasured: true, ..m("gpu_mb", "MB", Lower, &["memory", "gpu_bytes"], "gpu_mb", 0, "GPU memory the renderer counts holding") },
    Metric {
        scale: MB,
        zero_unmeasured: true,
        ..m("gpu_device_mb", "MB", Lower, &["memory", "gpu_device_bytes"], "gpu_device_mb", 0, "Measured: the most its GPU devices' allocators held (rounding and transient memory included)")
    },
    m("psnr", "dB", Higher, &["quality", "psnr", "mean"], "psnr", 1, "Mean PSNR against the reference (99 = exact)"),
    m("psnr_min", "dB", Higher, &["quality", "psnr", "min"], "psnr_min", 1, "Worst frame's PSNR against the reference"),
    m("bad_pct", "%", Lower, &["quality", "bad_pct", "mean"], "bad_pct", 2, "Mean share of pixels off by more than 16/255 against the reference"),
    m("psnr_like", "dB", Higher, &["quality_like", "psnr", "mean"], "psnr_like", 1, "Mean PSNR against a reference given what the pipeline was given (its LOD region)"),
    m("bad_pct_like", "%", Lower, &["quality_like", "bad_pct", "mean"], "bad_pct_like", 2, "Bad pixels against the reference given the same"),
    m("psnr_settled", "dB", Higher, &["quality_settled", "psnr", "mean"], "psnr_settled", 1, "PSNR of the frames rendered with the camera held still long enough to settle (accumulating pipelines: converged)"),
    m("bad_pct_settled", "%", Lower, &["quality_settled", "bad_pct", "mean"], "bad_pct_settled", 2, "Bad pixels of the settled frames"),
    m("converged_spp", "paths/pixel", Neither, &["converged_spp"], "converged_spp", 0, "Paths per pixel its settled frames accumulated"),
    m("wire_bytes", "bytes", Lower, &["wire_bytes"], "stream.wire_bytes", 0, "Bytes of the world's stream sent to its client in the pass, warm-up included: each change's size, or through gates into the client the first gate's line (serialized, per-packet resends included)"),
    m("arrived_bytes", "bytes", Neither, &["arrived_bytes"], "stream.arrived_bytes", 0, "Bytes of the world's stream that came out of the gates into its client (each change's size)"),
    m("update_ns_per_byte", "ns/byte", Lower, &["update_ns_per_byte"], "stream.update_ns_per_byte", 2, "World-update time per byte arrived"),
    m("holes", "chunks", Lower, &["holes", "mean"], "stream.holes", 1, "Mean chunks of its region missing when a frame was requested"),
    m("holes_max", "chunks", Lower, &["holes", "max"], "stream.holes_max", 0, "Most chunks of its region missing at one frame"),
    m("frames_with_holes", "frames", Lower, &["frames_with_holes"], "stream.frames_with_holes", 0, "Frames requested with any of their region missing"),
    m("first_complete_frame", "frame", Lower, &["first_complete", "frame"], "stream.first_complete_frame", 0, "The first frame requested with nothing of its region missing"),
    m("first_complete_ms", "ms", Lower, &["first_complete", "ms"], "stream.first_complete_ms", 0, "Wall time from the pass's start to its first complete frame"),
    m("gpu_sm_mhz", "MHz", Neither, &["conditions", "gpu_sm_mhz"], "", 0, "The GPU's shader clock when the pass's frames ended (a condition: what could explain a jump)"),
    m("gpu_mem_mhz", "MHz", Neither, &["conditions", "gpu_mem_mhz"], "", 0, "The GPU's memory clock when the pass's frames ended"),
    m("gpu_pstate", "P", Neither, &["conditions", "gpu_pstate"], "", 0, "The GPU's performance state when the pass's frames ended (0 the highest)"),
    m("gpu_power_w", "W", Neither, &["conditions", "gpu_power_w"], "", 1, "The GPU's power draw when the pass's frames ended"),
    m("gpu_temp_c", "°C", Neither, &["conditions", "gpu_temp_c"], "", 0, "The GPU's temperature when the pass's frames ended"),
    m("on_ac", "1 = mains", Neither, &["conditions", "on_ac"], "", 0, "On mains (1) or battery (0) when the pass's frames ended"),
    m("cpu_mhz", "MHz", Neither, &["conditions", "cpu_mhz"], "", 0, "The CPU cores' mean clock when the pass's frames ended"),
];

/// The metric named `name`, if this build knows it.
pub fn get(name: &str) -> Option<&'static Metric> {
    METRICS.iter().find(|m| m.name == name)
}

fn at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |cur, p| cur.get(p))
}

impl Metric {
    /// Its value in a pass's report (`PassReport` as JSON), if the pass measured it.
    pub fn of(&self, report: &Value) -> Option<f64> {
        let x = at(report, self.report)?.as_f64()?;
        (!(self.zero_unmeasured && x == 0.0)).then_some(x * self.scale)
    }

    /// Shown with its decimals.
    pub fn show(&self, x: f64) -> String {
        format!("{x:.*}", self.decimals)
    }
}

/// What a pass's report measured, by metric, with each one's method version. A failed pass
/// keeps only what is not zero (its zero fps and start-up were never measured).
pub fn measure(report: &Value) -> (BTreeMap<String, f64>, BTreeMap<String, u32>) {
    let failed = report.get("error").is_some_and(|e| !e.is_null());
    let (mut values, mut methods) = (BTreeMap::new(), BTreeMap::new());
    for m in METRICS {
        if let Some(x) = m.of(report).filter(|x| x.is_finite() && !(failed && *x == 0.0)) {
            values.insert(m.name.to_string(), x);
            methods.insert(m.name.to_string(), m.method);
        }
    }
    (values, methods)
}

/// A record of version 6 or older, its metrics moved from their fields into `metrics` and
/// `methods` (method 1), as version 7 holds them. The fields are removed.
pub fn from_v6(record: &mut Value) {
    let failed = record.get("error").is_some_and(|e| !e.is_null());
    let (mut values, mut methods) = (serde_json::Map::new(), serde_json::Map::new());
    for m in METRICS.iter().filter(|m| !m.v6.is_empty()) {
        let path: Vec<&str> = m.v6.split('.').collect();
        let Some(x) = at(record, &path).and_then(Value::as_f64) else { continue };
        if failed && x == 0.0 {
            continue;
        }
        values.insert(m.name.into(), x.into());
        methods.insert(m.name.into(), 1.into());
    }
    if let Some(o) = record.as_object_mut() {
        for m in METRICS.iter().filter(|m| !m.v6.is_empty() && !m.v6.contains('.')) {
            o.remove(m.v6);
        }
        o.remove("stream");
        o.insert("metrics".into(), Value::Object(values));
        o.insert("methods".into(), Value::Object(methods));
    }
}

/// The registry as JSON, for scripts (`multirenderer metrics`).
pub fn json() -> Value {
    serde_json::to_value(METRICS).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_are_unique_and_described() {
        let mut seen = std::collections::BTreeSet::new();
        for m in METRICS {
            assert!(seen.insert(m.name), "{} twice", m.name);
            assert!(!m.description.is_empty() && !m.unit.is_empty() && !m.report.is_empty(), "{}", m.name);
            assert!(m.method >= 1, "{}", m.name);
        }
    }

    #[test]
    fn a_report_measures_what_it_holds() {
        let r = json!({ "fps": 60.0, "gpu_ms": { "mean": 2.5 }, "memory": { "gpu_bytes": 0, "gpu_device_bytes": 2097152 }, "first_complete": null, "error": null });
        let (v, m) = measure(&r);
        assert_eq!(v.get("fps"), Some(&60.0));
        assert_eq!(v.get("gpu_ms"), Some(&2.5));
        assert_eq!(v.get("gpu_device_mb"), Some(&2.0));
        assert!(!v.contains_key("gpu_mb"), "a zero counted is not measured");
        assert!(!v.contains_key("first_complete_ms"));
        assert_eq!(m.keys().collect::<Vec<_>>(), v.keys().collect::<Vec<_>>());
        let failed = json!({ "fps": 0.0, "init_ms": 0.0, "error": "boom" });
        assert!(measure(&failed).0.is_empty(), "a failed pass measured nothing it reports as zero");
    }

    #[test]
    fn v6_fields_move_into_the_map() {
        let mut r = json!({ "version": 6, "fps": 30.0, "gpu_ms": null, "init_ms": 12.0, "stream": { "wire_bytes": 100, "holes": 0.5 }, "label": "x" });
        from_v6(&mut r);
        assert_eq!(r["metrics"], json!({ "fps": 30.0, "init_ms": 12.0, "wire_bytes": 100.0, "holes": 0.5 }));
        assert_eq!(r["methods"]["fps"], 1);
        assert!(r.get("fps").is_none() && r.get("stream").is_none() && r["label"] == "x");
    }
}
