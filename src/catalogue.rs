//! The catalogue: what each renderer is good at, from the observations of any number of
//! runs (`multirenderer catalogue <dirs>`). Scenarios are (machine, plan, mode, resolution);
//! within one, renderers are ranked by throughput (fps), and a renderer whose frames fall
//! below `min_psnr` against the reference counts as incorrect there (ranked, but never best).
//!
//! Per renderer: relative speed (fps / the best correct renderer's) per scenario, its
//! geometric mean over scenarios, wins, where it is within 10% of the best ("good at"), and
//! where it does unusually well compared with its own average ("stands out": comparative
//! advantage). Per component: the paired effect of choosing it for a node, among pipelines
//! whose graphs differ in that node's component alone (knobs equal).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mr_components::graph::{self, Graph};

use crate::observe::Observation;

/// One renderer in one scenario (repeats and runs merged by median).
#[derive(Clone, Debug)]
pub struct Entry {
    pub label: String,
    /// The pipeline (none for standalone renderers) and its key.
    pub graph: Option<Graph>,
    pub graph_key: Option<String>,
    pub options: String,
    pub fps: f64,
    pub frame_p99_ms: Option<f64>,
    pub gpu_ms: Option<f64>,
    pub cpu_ms: Option<f64>,
    pub psnr: Option<f64>,
    /// Against a reference given the same of the world as it (LOD pipelines; else none).
    pub psnr_like: Option<f64>,
    /// Once its late edges caught up (pipelines with an edge a frame late; else none).
    pub psnr_settled: Option<f64>,
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

/// The effect of a node's component, over the pipelines whose graphs differ in that node's
/// component alone. Which node: its executor, its role and its place.
#[derive(Clone, Debug)]
pub struct ComponentEffect {
    pub executor: String,
    pub role: &'static str,
    /// Where the node is in its graph: every way to it from the output (`nodes`).
    pub place: String,
    /// (component, geomean of fps / best fps among otherwise equal pipelines, wins,
    /// comparisons)
    pub rows: Vec<(String, f64, usize, usize)>,
}

/// Every node of a graph as (role, place, component, the rest of the graph). Its place is
/// every way to it from the output, each the inputs followed (a node's in the order its
/// component declares them: the main path first), so a node is what its readers make of it
/// (a storage the primary rays walk is another place than one only shadow rays walk) in
/// every graph alike, whatever its id; the rest is the graph without that node's component,
/// equal for the graphs that differ in it alone. Nothing for a graph that does not check.
fn nodes(g: &Graph) -> Vec<(&'static str, String, String, String)> {
    let Ok(c) = g.check() else { return vec![] };
    let g = &c.graph;
    fn visit<'a>(g: &'a Graph, id: &'a str, path: &mut Vec<&'a str>, on: &mut Vec<&'a str>, places: &mut BTreeMap<&'a str, Vec<String>>) {
        // (An edge back to a node on the way, a frame late, is not another way to it.)
        if on.contains(&id) {
            return;
        }
        places.entry(id).or_default().push(path.join(" > "));
        let n = &g.nodes[id];
        let ports = graph::inputs(n.role, &n.component);
        let mut ins: Vec<(&String, &String)> = n.inputs.iter().collect();
        ins.sort_by_key(|(p, _)| ports.iter().position(|x| x.name == p.as_str()));
        on.push(id);
        for (p, s) in ins {
            path.push(p);
            visit(g, s, path, on, places);
            path.pop();
        }
        on.pop();
    }
    let mut places = BTreeMap::new();
    visit(g, &c.output, &mut vec![], &mut vec![], &mut places);
    // Each node written once with and once without its component: a rest is the others whole.
    let text = |id: &str, n: &graph::Node| format!("{id}={};", serde_json::to_string(n).unwrap_or_default());
    let whole: Vec<String> = g.nodes.iter().map(|(id, n)| text(id, n)).collect();
    g.nodes
        .iter()
        .enumerate()
        .filter_map(|(i, (id, n))| {
            let blank = text(id, &graph::Node { component: String::new(), ..n.clone() });
            let rest: String = whole.iter().enumerate().map(|(j, w)| if i == j { &blank } else { w }).map(String::as_str).collect();
            Some((n.role.name(), places.get(id.as_str())?.join("; "), n.component.clone(), format!("{}|{rest}", g.runs_on())))
        })
        .collect()
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
                    // A pipeline given less than the full-resolution reference is judged
                    // against the reference given the same: lossy is not incorrect.
                    let psnr_like = m(&|o| o.psnr_like);
                    // A pipeline with an edge a frame late is judged on its settled frames:
                    // late is not incorrect.
                    let psnr_settled = m(&|o| o.psnr_settled);
                    Entry {
                        label,
                        graph: os[0].pipeline(),
                        graph_key: os[0].graph_key.clone(),
                        options: os[0].options.to_string(),
                        fps: m(&|o| Some(o.fps)).unwrap_or(0.0),
                        frame_p99_ms: m(&|o| o.frame_p99_ms),
                        gpu_ms: m(&|o| o.gpu_ms),
                        cpu_ms: m(&|o| o.cpu_ms),
                        psnr,
                        psnr_like,
                        psnr_settled,
                        bad_pct: m(&|o| o.bad_pct),
                        gpu_mb: m(&|o| o.gpu_mb),
                        rss_mb: m(&|o| o.peak_rss_mb),
                        samples: ok.len(),
                        error: if ok.is_empty() { os.iter().find_map(|o| o.error.clone()) } else { None },
                        rel: 0.0,
                        valid: !ok.is_empty() && psnr_settled.or(psnr_like).or(psnr).is_none_or(|p| p >= min_psnr),
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

        // Per component: compare pipelines whose graphs differ only in one node's component.
        type Place = (String, &'static str, String);
        let mut ratios: BTreeMap<Place, BTreeMap<String, (Vec<f64>, usize)>> = BTreeMap::new();
        let mut parts: BTreeMap<String, Vec<(&'static str, String, String, String)>> = BTreeMap::new();
        for s in &scenarios {
            let mut by_rest: BTreeMap<(Place, String), Vec<(String, f64)>> = BTreeMap::new();
            for e in s.entries.iter().filter(|e| e.valid) {
                let Some(g) = &e.graph else { continue };
                for (role, place, component, rest) in parts.entry(e.label.clone()).or_insert_with(|| nodes(g)).iter() {
                    by_rest.entry(((g.runs_on().to_string(), *role, place.clone()), format!("{rest} {}", e.options))).or_default().push((component.clone(), e.fps));
                }
            }
            for ((place, _), group) in by_rest.iter().filter(|(_, g)| g.len() > 1) {
                let top = group.iter().map(|g| g.1).fold(0.0, f64::max);
                for (name, fps) in group {
                    let r = ratios.entry(place.clone()).or_default().entry(name.clone()).or_default();
                    r.0.push(fps / top);
                    if *fps >= top {
                        r.1 += 1;
                    }
                }
            }
        }
        let mut components = vec![];
        for ((executor, role, place), ratios) in ratios {
            if ratios.len() > 1 {
                let mut rows: Vec<(String, f64, usize, usize)> = ratios.into_iter().map(|(n, (v, w))| (n, geomean(&v), w, v.len())).collect();
                rows.sort_by(|a, b| b.1.total_cmp(&a.1));
                components.push(ComponentEffect { executor, role, place, rows });
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
            let _ = writeln!(s, "## Components\n\nThe effect of a node's component, among pipelines whose graphs differ in that node's component alone (knobs equal): the geometric mean of fps relative to the best of each such group, wins, and the number of comparisons. A node is named by its executor, its role and the inputs leading to it from the output.\n");
            for c in &self.components {
                let _ = writeln!(s, "**{} · {} ({})**\n\n| component | geomean rel | wins | comparisons |\n|---|---:|---:|---:|", c.executor, c.role, c.place);
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
        let mut s = String::from("machine,plan,mode,width,height,cam_speed,cam_turn_deg,chunk_loads,solid_fraction,surface_fraction,sky_fraction,hit_distance,edits,lights,mesh_triangles,label,executor,graph_key,options,fps,rel,valid,frame_p99_ms,gpu_ms,cpu_ms,psnr,psnr_like,psnr_settled,bad_pct,gpu_mb,rss_mb,samples,error\n");
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
                let _ = writeln!(
                    s,
                    "{scenario},{},{},{},{},{:.3},{:.4},{},{},{},{},{},{},{},{},{},{},{},{}",
                    q(&e.label),
                    e.graph.as_ref().map_or("", |g| g.runs_on()),
                    e.graph_key.as_deref().unwrap_or(""),
                    q(&e.options),
                    e.fps,
                    e.rel,
                    e.valid,
                    opt(e.frame_p99_ms, 3),
                    opt(e.gpu_ms, 3),
                    opt(e.cpu_ms, 3),
                    opt(e.psnr, 2),
                    opt(e.psnr_like, 2),
                    opt(e.psnr_settled, 2),
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

    /// The ray caster's storage: its primary rays walk it, and the shadow rays of its meshes and its shading.
    const PRIMARY: &str = "image > hits > primary > storage; image > hits > meshes > shadow > storage; image > shadow > storage";

    /// The shipped ray-casting graph with another storage.
    fn with(storage: &str) -> Graph {
        let mut g = graph::shipped("raycast-gpu").unwrap();
        g.nodes.get_mut("storage").unwrap().component = storage.into();
        g
    }

    fn obs(plan: &str, g: Graph, fps: f64, psnr: Option<f64>) -> Observation {
        Observation {
            version: crate::observe::VERSION,
            run: "r".into(),
            machine: Machine::default(),
            plan: plan.into(),
            resolution: [64, 64],
            view_radius: 4,
            frames: 10,
            streaming: "complete".into(),
            mode: "unlit".into(),
            label: g.label().unwrap(),
            renderer: g.runs_on().into(),
            graph_key: g.key().ok(),
            graph: Some(g),
            options: serde_json::json!({}),
            fps,
            psnr,
            ..Default::default()
        }
    }

    #[test]
    fn ranks_and_effects() {
        let c = with;
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
        assert_eq!(terrain.entries[0].label, "gpu: prefix + refloor + composite + raycast + direct", "an incorrect renderer is never best");
        let hier = cat.renderers.iter().find(|r| r.label.contains("hier")).unwrap();
        assert_eq!(hier.incorrect, ["terrain [unlit]"]);
        let prefix = cat.renderers.iter().find(|r| r.label.contains("prefix")).unwrap();
        assert_eq!(prefix.wins, ["terrain [unlit]"]);
        assert!((prefix.geomean.unwrap() - (1.0f64 * 0.5).sqrt()).abs() < 1e-9);
        assert_eq!(hier.geomean, None, "never correct");
        assert_eq!(cat.components.len(), 1, "one node differs: {:?}", cat.components);
        let storage = &cat.components[0];
        assert_eq!((storage.executor.as_str(), storage.role, storage.place.as_str()), ("gpu", "storage", PRIMARY));
        // bricks: 100/120 and 1; prefix: 1 and 0.5.
        let row = |n: &str| storage.rows.iter().find(|r| r.0 == n).unwrap().clone();
        assert!((row("bricks").1 - (100.0f64 / 120.0).sqrt()).abs() < 1e-9 && row("bricks").2 == 1 && row("bricks").3 == 2);
        assert!(cat.markdown().contains(&format!("**gpu · storage ({PRIMARY})**")) && cat.csv().lines().count() == 6);
        assert!(cat.csv().contains(&format!(",gpu,{},", with("prefix").key().unwrap())), "{}", cat.csv());
    }

    /// A node's component is compared only among graphs that differ in it alone, and the
    /// same component at another place is another row.
    #[test]
    fn effects_one_node_apart() {
        let kernel = |st: &str, k: &str| {
            let mut g = with(st);
            g.nodes.get_mut("kernel").unwrap().component = k.into();
            g
        };
        // Two nodes apart (bricks + refloor, prefix + incremental): no comparison.
        let apart = Catalogue::build(&[obs("terrain", kernel("bricks", "refloor"), 100.0, None), obs("terrain", kernel("prefix", "incremental"), 50.0, None)], 35.0);
        assert!(apart.components.is_empty(), "{:?}", apart.components);
        // A third joins each to it: one comparison per node.
        let cat = Catalogue::build(&[obs("terrain", kernel("bricks", "refloor"), 100.0, None), obs("terrain", kernel("prefix", "incremental"), 50.0, None), obs("terrain", kernel("prefix", "refloor"), 80.0, None)], 35.0);
        let at = |role: &str| cat.components.iter().find(|e| e.role == role).unwrap();
        assert_eq!(cat.components.len(), 2);
        assert_eq!(at("storage").rows.iter().map(|r| (r.0.as_str(), r.3)).collect::<Vec<_>>(), [("bricks", 1), ("prefix", 1)]);
        assert_eq!((at("traversal").place.as_str(), at("traversal").rows[0].0.as_str()), ("image > hits > primary; image > hits > meshes > shadow; image > shadow", "refloor"));
        // The rasteriser's storage (its shadow rays') is another place than the ray caster's.
        let raster = |st: &str| {
            let mut g = graph::shipped("raster-gpu").unwrap();
            g.nodes.get_mut("storage").unwrap().component = st.into();
            g
        };
        let both = Catalogue::build(&[obs("terrain", with("bricks"), 100.0, None), obs("terrain", with("prefix"), 50.0, None), obs("terrain", raster("bricks"), 10.0, None), obs("terrain", raster("prefix"), 20.0, None)], 35.0);
        let places: Vec<&str> = both.components.iter().map(|e| e.place.as_str()).collect();
        assert_eq!(places, ["image > hits > meshes > shadow > storage; image > shadow > storage", PRIMARY]);
    }

    /// Records of pipelines from before a pipeline was its graph are left out and counted;
    /// a standalone renderer's older record reads, as does anything of this version.
    #[test]
    fn older_records() {
        let new = obs("terrain", with("prefix"), 100.0, None);
        let line = |f: &dyn Fn(&mut serde_json::Map<String, serde_json::Value>)| {
            let mut v = serde_json::to_value(&new).unwrap();
            f(v.as_object_mut().unwrap());
            v.to_string()
        };
        let flat = line(&|m| {
            m.remove("graph");
            m.insert("version".into(), 5.into());
            m.insert("composition".into(), serde_json::json!({ "storage": "prefix" }));
        });
        let old_graph = line(&|m| drop(m.insert("version".into(), 5.into())));
        let unparsed = line(&|m| {
            m.insert("version".into(), 2.into());
            m.insert("fps".into(), "fast".into());
        });
        let standalone = line(&|m| {
            m.remove("graph");
            m.insert("version".into(), 5.into());
            m.insert("renderer".into(), "cpu-reference".into());
        });
        let now = line(&|_| ());
        let parse = |l: &str| Observation::parse(l).unwrap();
        assert!(parse(&flat).is_none() && parse(&old_graph).is_none() && parse(&unparsed).is_none());
        assert!(parse(&standalone).is_some_and(|o| o.pipeline().is_none() && o.renderer == "cpu-reference"));
        assert_eq!(parse(&now).and_then(|o| o.pipeline()).and_then(|g| g.key().ok()), new.graph_key);
        assert!(Observation::parse(&line(&|m| drop(m.insert("fps".into(), "fast".into())))).is_err(), "a record of this version that does not read is an error");
        let dir = std::env::temp_dir().join(format!("mr-older-records-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("observations.jsonl");
        std::fs::write(&file, [flat, old_graph, unparsed, standalone, now].join("\n")).unwrap();
        let (read, left_out) = crate::observe::read(&[file]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!((read.len(), left_out), (2, 3));
    }

    /// A pipeline with an edge a frame late is held to the floor on its settled frames: late
    /// is not incorrect; one that stays low once settled is.
    #[test]
    fn late_is_not_incorrect() {
        let c = with;
        let settled = |st: &str, s: f64| Observation { late: vec!["draw.faces".into()], psnr_settled: Some(s), ..obs("terrain", c(st), 100.0, Some(30.0)) };
        let cat = Catalogue::build(&[settled("bricks", 50.0), settled("hier", 30.0)], 35.0);
        let valid = |st: &str| cat.scenarios[0].entries.iter().find(|e| e.label.contains(st)).unwrap().valid;
        assert!(valid("bricks") && !valid("hier"));
        assert!(cat.csv().lines().nth(1).unwrap().contains(",30.00,-,50.00,"), "{}", cat.csv());
    }
}
