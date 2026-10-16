//! The catalogue: what each renderer is good at, from the observations of any number of
//! runs (`multirenderer catalogue <dirs>`). Scenarios are (machine, plan, mode, resolution);
//! within one, renderers are ranked by throughput (fps), and a renderer whose frames fall
//! below `min_psnr` against the reference counts as incorrect there (ranked, but never best).
//!
//! Per renderer: relative speed (fps / the best correct renderer's) per scenario, its
//! geometric mean over scenarios, wins, where it is within 10% of the best ("good at"), and
//! where it does unusually well compared with its own average ("stands out": comparative
//! advantage). Per component: the paired effect of choosing it for a stage, other stages and
//! knobs equal.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mr_components::{Composition, Slot};

use crate::observe::Observation;

/// One renderer in one scenario (repeats and runs merged by median).
#[derive(Clone, Debug)]
pub struct Entry {
    pub label: String,
    pub composition: Option<Composition>,
    pub options: String,
    pub fps: f64,
    pub frame_p99_ms: Option<f64>,
    pub gpu_ms: Option<f64>,
    pub cpu_ms: Option<f64>,
    pub psnr: Option<f64>,
    pub bad_pct: Option<f64>,
    pub gpu_mb: Option<f64>,
    pub rss_mb: Option<f64>,
    pub samples: usize,
    pub error: Option<String>,
    /// fps / the best correct renderer's fps in this scenario.
    pub rel: f64,
    /// Ran, and its frames meet `min_psnr` (or were not compared).
    pub valid: bool,
}

#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: String,
    pub machine: String,
    pub plan: String,
    pub mode: String,
    pub resolution: [u32; 2],
    /// Measured scene features (from the first observation that has them).
    pub features: Option<crate::features::SceneFeatures>,
    /// Fastest first; failures last.
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug)]
pub struct RendererSummary {
    pub label: String,
    pub scenarios: usize,
    /// Geometric mean of `rel` over the scenarios where it ran correctly (`None`: nowhere).
    pub geomean: Option<f64>,
    pub wins: Vec<String>,
    pub good_at: Vec<String>,
    /// Scenarios where `rel` is well above its own geometric mean, with the ratio.
    pub stands_out: Vec<(String, f64)>,
    pub weak_at: Vec<String>,
    pub incorrect: Vec<String>,
    pub failed: Vec<String>,
}

/// The effect of a component choice for one stage of one back end.
#[derive(Clone, Debug)]
pub struct ComponentEffect {
    pub backend: String,
    pub slot: &'static str,
    /// (component, geomean of fps / best fps among otherwise equal compositions, wins,
    /// comparisons)
    pub rows: Vec<(String, f64, usize, usize)>,
}

pub struct Catalogue {
    pub min_psnr: f64,
    pub machines: Vec<String>,
    pub runs: usize,
    pub observations: usize,
    pub scenarios: Vec<Scenario>,
    pub renderers: Vec<RendererSummary>,
    pub components: Vec<ComponentEffect>,
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn geomean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    (v.iter().map(|x| x.max(1e-6).ln()).sum::<f64>() / v.len() as f64).exp()
}

fn opt(v: Option<f64>, p: usize) -> String {
    v.map(|x| format!("{x:.p$}")).unwrap_or_else(|| "-".into())
}

impl Catalogue {
    pub fn build(obs: &[Observation], min_psnr: f64) -> Self {
        let machines: BTreeSet<String> = obs.iter().map(|o| o.machine.key()).collect();
        let multi = machines.len() > 1;
        // Group by scenario, then by renderer.
        type Key = (String, String, String, [u32; 2]);
        let mut groups: BTreeMap<Key, BTreeMap<String, Vec<&Observation>>> = BTreeMap::new();
        let mut features: BTreeMap<Key, crate::features::SceneFeatures> = BTreeMap::new();
        for o in obs {
            let label = if o.options.as_object().is_some_and(|m| !m.is_empty()) { format!("{} {}", o.label, o.options) } else { o.label.clone() };
            let key = (o.machine.key(), o.plan.clone(), o.mode.clone(), o.resolution);
            if let Some(f) = &o.features {
                features.entry(key.clone()).or_insert_with(|| f.clone());
            }
            groups.entry(key).or_default().entry(label).or_default().push(o);
        }
        let mut scenarios = vec![];
        for (key, by_label) in groups {
            let features = features.get(&key).cloned();
            let (machine, plan, mode, resolution) = key;
            let mut entries: Vec<Entry> = by_label
                .into_iter()
                .map(|(label, os)| {
                    let ok: Vec<&&Observation> = os.iter().filter(|o| o.error.is_none()).collect();
                    let m = |f: &dyn Fn(&Observation) -> Option<f64>| median(ok.iter().filter_map(|o| f(o)).collect());
                    let psnr = m(&|o| o.psnr);
                    Entry {
                        label,
                        composition: os[0].composition.clone(),
                        options: os[0].options.to_string(),
                        fps: m(&|o| Some(o.fps)).unwrap_or(0.0),
                        frame_p99_ms: m(&|o| o.frame_p99_ms),
                        gpu_ms: m(&|o| o.gpu_ms),
                        cpu_ms: m(&|o| o.cpu_ms),
                        psnr,
                        bad_pct: m(&|o| o.bad_pct),
                        gpu_mb: m(&|o| o.gpu_mb),
                        rss_mb: m(&|o| o.peak_rss_mb),
                        samples: ok.len(),
                        error: if ok.is_empty() { os.iter().find_map(|o| o.error.clone()) } else { None },
                        rel: 0.0,
                        valid: !ok.is_empty() && psnr.is_none_or(|p| p >= min_psnr),
                    }
                })
                .collect();
            let best = entries.iter().filter(|e| e.valid).map(|e| e.fps).fold(0.0, f64::max);
            for e in &mut entries {
                e.rel = if best > 0.0 && e.error.is_none() { e.fps / best } else { 0.0 };
            }
            entries.sort_by(|a, b| a.error.is_some().cmp(&b.error.is_some()).then(b.valid.cmp(&a.valid)).then(b.fps.total_cmp(&a.fps)));
            let name = if multi { format!("{plan} [{mode}] @ {machine}") } else { format!("{plan} [{mode}]") };
            scenarios.push(Scenario { name, machine, plan, mode, resolution, features, entries });
        }

        // Per renderer.
        let mut per: BTreeMap<String, Vec<(&Scenario, &Entry)>> = BTreeMap::new();
        for s in &scenarios {
            for e in &s.entries {
                per.entry(e.label.clone()).or_default().push((s, e));
            }
        }
        let mut renderers: Vec<RendererSummary> = per
            .into_iter()
            .map(|(label, list)| {
                let valid: Vec<(&Scenario, &Entry)> = list.iter().copied().filter(|(_, e)| e.valid).collect();
                let g = (!valid.is_empty()).then(|| geomean(&valid.iter().map(|(_, e)| e.rel).collect::<Vec<_>>()));
                let names = |f: &dyn Fn(&Entry) -> bool| list.iter().filter(|(_, e)| f(e)).map(|(s, _)| s.name.clone()).collect::<Vec<_>>();
                let mut stands: Vec<(String, f64)> = valid.iter().filter(|_| valid.len() > 1).map(|(s, e)| (s.name.clone(), e.rel / g.unwrap_or(1.0).max(1e-9))).filter(|&(_, r)| r >= 1.25).collect();
                stands.sort_by(|a, b| b.1.total_cmp(&a.1));
                stands.truncate(3);
                RendererSummary {
                    label,
                    scenarios: list.len(),
                    geomean: g,
                    wins: names(&|e| e.valid && e.rel >= 0.9999),
                    good_at: names(&|e| e.valid && e.rel >= 0.9),
                    stands_out: stands,
                    weak_at: names(&|e| e.valid && e.rel < 0.25),
                    incorrect: names(&|e| e.error.is_none() && !e.valid),
                    failed: names(&|e| e.error.is_some()),
                }
            })
            .collect();
        renderers.sort_by(|a, b| b.geomean.unwrap_or(-1.0).total_cmp(&a.geomean.unwrap_or(-1.0)));

        // Per component: compare compositions that differ only in that stage.
        let mut components = vec![];
        let backends: BTreeSet<String> = scenarios.iter().flat_map(|s| s.entries.iter().filter_map(|e| e.composition.as_ref().map(|c| c.backend.clone()))).collect();
        for b in backends {
            for slot in Slot::ALL {
                let mut ratios: BTreeMap<String, (Vec<f64>, usize)> = BTreeMap::new();
                for s in &scenarios {
                    let mut by_rest: BTreeMap<String, Vec<(&str, f64)>> = BTreeMap::new();
                    for e in s.entries.iter().filter(|e| e.valid) {
                        let Some(c) = e.composition.as_ref().filter(|c| c.backend == b && !c.get(slot).is_empty()) else { continue };
                        let mut rest = c.clone();
                        rest.set(slot, "");
                        by_rest.entry(format!("{rest:?} {}", e.options)).or_default().push((c.get(slot), e.fps));
                    }
                    for group in by_rest.values().filter(|g| g.len() > 1) {
                        let top = group.iter().map(|g| g.1).fold(0.0, f64::max);
                        for &(name, fps) in group {
                            let r = ratios.entry(name.to_string()).or_default();
                            r.0.push(fps / top);
                            if fps >= top {
                                r.1 += 1;
                            }
                        }
                    }
                }
                if ratios.len() > 1 {
                    let mut rows: Vec<(String, f64, usize, usize)> = ratios.into_iter().map(|(n, (v, w))| (n, geomean(&v), w, v.len())).collect();
                    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
                    components.push(ComponentEffect { backend: b.clone(), slot: slot.name(), rows });
                }
            }
        }
        let runs: BTreeSet<&str> = obs.iter().map(|o| o.run.as_str()).collect();
        Self { min_psnr, machines: machines.into_iter().collect(), runs: runs.len(), observations: obs.len(), scenarios, renderers, components }
    }

    /// The best correct renderer of each scenario, and the overall ranking (for the terminal).
    pub fn summary(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "Best per scenario:");
        for sc in &self.scenarios {
            match sc.entries.iter().find(|e| e.valid) {
                Some(e) => {
                    let _ = writeln!(s, "  {:<34} {:<52} {:>8.1} fps", sc.name, e.label, e.fps);
                }
                None => {
                    let _ = writeln!(s, "  {:<34} (no correct renderer)", sc.name);
                }
            }
        }
        let _ = writeln!(s, "\nOverall (geometric mean of speed relative to each scenario's best), top 20:");
        for r in self.renderers.iter().take(20) {
            let _ = writeln!(s, "  {:<52} {:>5}  wins {:>2}  good at {:>2}/{}", r.label, opt(r.geomean, 2), r.wins.len(), r.good_at.len(), r.scenarios);
        }
        s
    }

    pub fn markdown(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Renderer catalogue\n");
        let _ = writeln!(s, "{} observations from {} runs, {} scenarios. Machine{}: {}.\n", self.observations, self.runs, self.scenarios.len(), if self.machines.len() > 1 { "s" } else { "" }, self.machines.join("; "));
        let _ = writeln!(
            s,
            "How to read: a scenario is a plan (scene, camera path, events) in one pass mode. Speed is throughput (completed frames per second; at several hundred fps and more the host's per-frame overhead compresses differences, so GPU ms is shown too). *rel* is fps relative to the fastest correct renderer of the scenario; correct means a mean PSNR against the reference of at least {:.0} dB (or not compared). *good at*: within 10% of the best. *stands out*: relative speed at least 1.25× the renderer's own average, i.e. where it is comparatively strong.\n",
            self.min_psnr
        );
        let _ = writeln!(s, "## Best per scenario\n\n| scenario | best | fps | gpu ms | PSNR | runner-up | rel |\n|---|---|---:|---:|---:|---|---:|");
        for sc in &self.scenarios {
            let mut ok = sc.entries.iter().filter(|e| e.valid);
            let (a, b) = (ok.next(), ok.next());
            match a {
                Some(a) => {
                    let _ = writeln!(s, "| {} | {} | {:.1} | {} | {} | {} | {} |", sc.name, a.label, a.fps, opt(a.gpu_ms, 2), opt(a.psnr, 1), b.map_or("-".into(), |b| b.label.clone()), b.map_or("-".into(), |b| format!("{:.2}", b.rel)));
                }
                None => {
                    let _ = writeln!(s, "| {} | (no correct renderer) | | | | | |", sc.name);
                }
            }
        }
        let _ = writeln!(s, "\n## Renderers\n\nSorted by the geometric mean of *rel* over the scenarios where the renderer ran correctly.\n\n| renderer | scenarios | geomean rel | wins | good at | stands out | weak at (rel < 0.25) | incorrect | failed |\n|---|---:|---:|---:|---|---|---|---|---|");
        for r in &self.renderers {
            let list = |v: &[String]| if v.is_empty() { "".to_string() } else { v.join(", ") };
            let stands: Vec<String> = r.stands_out.iter().map(|(n, x)| format!("{n} ({x:.1}×)")).collect();
            let _ = writeln!(s, "| {} | {} | {} | {} | {} | {} | {} | {} | {} |", r.label, r.scenarios, opt(r.geomean, 2), r.wins.len(), list(&r.good_at), stands.join(", "), list(&r.weak_at), list(&r.incorrect), list(&r.failed));
        }
        let _ = writeln!(s);
        if !self.components.is_empty() {
            let _ = writeln!(s, "## Components\n\nThe effect of a stage's component, other stages and knobs equal: the geometric mean of fps relative to the best of each such group, wins, and the number of comparisons.\n");
            for c in &self.components {
                let _ = writeln!(s, "**{} · {}**\n\n| component | geomean rel | wins | comparisons |\n|---|---:|---:|---:|", c.backend, c.slot);
                for (n, g, w, k) in &c.rows {
                    let _ = writeln!(s, "| {n} | {g:.2} | {w} | {k} |");
                }
                let _ = writeln!(s);
            }
        }
        let _ = writeln!(s, "## Scenarios\n");
        for sc in &self.scenarios {
            let feats = sc.features.as_ref().map(|f| format!(" {}.", f.describe())).unwrap_or_default();
            let _ = writeln!(s, "### {}\n\n{}×{}.{feats}\n\n| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |\n|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|", sc.name, sc.resolution[0], sc.resolution[1]);
            for (i, e) in sc.entries.iter().enumerate() {
                if let Some(err) = &e.error {
                    let _ = writeln!(s, "| {} | {} | FAILED: {} | | | | | | | | |", i + 1, e.label, err.lines().next().unwrap_or("").replace('|', "/"));
                    continue;
                }
                let flag = if e.valid { "" } else { " (incorrect)" };
                let _ = writeln!(
                    s,
                    "| {} | {}{flag} | {:.1} | {:.2} | {} | {} | {} | {} | {} | {} | {} |",
                    i + 1,
                    e.label,
                    e.fps,
                    e.rel,
                    opt(e.frame_p99_ms, 2),
                    opt(e.gpu_ms, 2),
                    opt(e.cpu_ms, 2),
                    opt(e.psnr, 1),
                    opt(e.bad_pct, 2),
                    opt(e.gpu_mb, 0),
                    opt(e.rss_mb, 0)
                );
            }
            let _ = writeln!(s);
        }
        s
    }

    /// One row per (scenario, renderer), for spreadsheets and scripts.
    pub fn csv(&self) -> String {
        let mut s = String::from("machine,plan,mode,width,height,cam_speed,cam_turn_deg,chunk_loads,solid_fraction,surface_fraction,sky_fraction,hit_distance,edits,lights,mesh_triangles,label,backend,storage,geometry,prepass,visibility,shading,meshes,options,fps,rel,valid,frame_p99_ms,gpu_ms,cpu_ms,psnr,bad_pct,gpu_mb,rss_mb,samples,error\n");
        let q = |x: &str| format!("\"{}\"", x.replace('"', "\"\""));
        for sc in &self.scenarios {
            // The scenario's columns, including its measured features (empty if unknown).
            let feats = match &sc.features {
                Some(f) => format!(
                    "{:.3},{:.3},{:.2},{:.5},{:.5},{:.3},{:.1},{:.2},{},{}",
                    f.cam_speed, f.cam_turn_deg, f.chunk_loads, f.solid_fraction, f.surface_fraction, f.sky_fraction, f.hit_distance, f.edits, f.lights, f.mesh_triangles
                ),
                None => ",,,,,,,,,".into(),
            };
            let scenario = format!("{},{},{},{},{},{feats}", q(&sc.machine), q(&sc.plan), sc.mode, sc.resolution[0], sc.resolution[1]);
            for e in &sc.entries {
                let c = e.composition.clone().unwrap_or_else(|| Composition { backend: String::new(), ..Composition::new("") });
                let _ = writeln!(
                    s,
                    "{scenario},{},{},{},{},{},{},{},{},{},{:.3},{:.4},{},{},{},{},{},{},{},{},{},{}",
                    q(&e.label),
                    c.backend,
                    c.storage,
                    c.geometry,
                    c.prepass,
                    c.visibility,
                    c.shading,
                    c.meshes,
                    q(&e.options),
                    e.fps,
                    e.rel,
                    e.valid,
                    opt(e.frame_p99_ms, 3),
                    opt(e.gpu_ms, 3),
                    opt(e.cpu_ms, 3),
                    opt(e.psnr, 2),
                    opt(e.bad_pct, 3),
                    opt(e.gpu_mb, 1),
                    opt(e.rss_mb, 1),
                    e.samples,
                    q(e.error.as_deref().unwrap_or(""))
                );
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::{Machine, Observation};

    fn obs(plan: &str, comp: Composition, fps: f64, psnr: Option<f64>) -> Observation {
        Observation {
            version: 1,
            run: "r".into(),
            machine: Machine::default(),
            plan: plan.into(),
            scene: Default::default(),
            features: None,
            resolution: [64, 64],
            view_radius: 4,
            frames: 10,
            streaming: "complete".into(),
            mode: "unlit".into(),
            label: comp.label(),
            renderer: comp.backend.clone(),
            composition: Some(comp),
            sites: Default::default(),
            graph: None,
            graph_key: None,
            options: serde_json::json!({}),
            repeat: 0,
            error: None,
            fps,
            frame_ms: None,
            frame_p50_ms: None,
            frame_p99_ms: None,
            gpu_ms: None,
            cpu_ms: None,
            latency_ms: None,
            init_ms: 0.0,
            initial_load_ms: 0.0,
            peak_rss_mb: None,
            gpu_mb: None,
            psnr,
            psnr_min: None,
            bad_pct: None,
            counters: Default::default(),
        }
    }

    #[test]
    fn ranks_and_effects() {
        let c = |st: &str| Composition { storage: st.into(), ..Default::default() };
        let all = vec![
            obs("terrain", c("bricks"), 100.0, Some(50.0)),
            obs("terrain", c("prefix"), 120.0, Some(50.0)),
            obs("terrain", c("hier"), 300.0, Some(20.0)), // fast but wrong
            obs("menger", c("bricks"), 200.0, None),
            obs("menger", c("prefix"), 100.0, None),
        ];
        let cat = Catalogue::build(&all, 35.0);
        assert_eq!(cat.scenarios.len(), 2);
        let terrain = cat.scenarios.iter().find(|s| s.plan == "terrain").unwrap();
        assert_eq!(terrain.entries[0].label, "gpu-compute: prefix + refloor", "an incorrect renderer is never best");
        let hier = cat.renderers.iter().find(|r| r.label.contains("hier")).unwrap();
        assert_eq!(hier.incorrect, ["terrain [unlit]"]);
        let prefix = cat.renderers.iter().find(|r| r.label.contains("prefix")).unwrap();
        assert_eq!(prefix.wins, ["terrain [unlit]"]);
        assert!((prefix.geomean.unwrap() - (1.0f64 * 0.5).sqrt()).abs() < 1e-9);
        assert_eq!(hier.geomean, None, "never correct");
        let storage = cat.components.iter().find(|e| e.slot == "storage").unwrap();
        // bricks: 100/120 and 1; prefix: 1 and 0.5.
        let row = |n: &str| storage.rows.iter().find(|r| r.0 == n).unwrap().clone();
        assert!((row("bricks").1 - (100.0f64 / 120.0).sqrt()).abs() < 1e-9 && row("bricks").2 == 1 && row("bricks").3 == 2);
        assert!(cat.markdown().contains("## Components") && cat.csv().lines().count() == 6);
    }
}
