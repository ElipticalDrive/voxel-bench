//! Adding a technique is registering it: a component known to the registry alone (no other
//! edit) is offered everywhere components are listed. The test registers a trivial storage on
//! its own thread (`mr_components::register_on_this_thread`) and looks for it in each
//! listing; a listing that misses it needs a hand edit for every new component, and goes in
//! `DEBT` until that is fixed.

use mr_components::graph::{self, Graph, Role};
use mr_components::impls::{ISAS, SHADER};
use mr_components::{Cap, ComponentInfo, Impl, Kind};
use mr_core::PassMode;

use crate::plan::{Leaves, Like, Plan, RendererSpec, RunSpec};

/// The new component: a storage with GPU and CPU implementations (and LOD on the GPU, as
/// `bricks`), no code behind it; nothing here runs a frame.
const PROBE: &str = "probe";
static REGISTERED: [ComponentInfo; 1] = [ComponentInfo {
    role: Role::Storage,
    name: PROBE,
    description: "A storage only a test registers.",
    impls: &[
        Impl { kind: Kind::Gpu, apis: SHADER, provides: &[Cap::Voxels, Cap::Lod] },
        Impl { kind: Kind::Cpu, apis: ISAS, provides: &[Cap::Voxels] },
    ],
    needs: &[],
}];

/// The listings that do not offer a newly registered component yet (each needs a hand edit
/// per component): fixed, they leave this list, and the test says so.
const DEBT: &[&str] = &[];

/// The shipped graphs the probe should be offered in, one per executor: their `storage` node.
const FILES: [&str; 2] = ["raycast-gpu", "raycast-cpu"];

fn with_probe(file: &str) -> Graph {
    graph::shipped(file).unwrap().edit(&graph::Change::component("storage", PROBE)).unwrap()
}

/// The reference class of `spec`'s lit frames: what its reference is given and computes.
fn class(spec: &RendererSpec, run: &RunSpec) -> Option<Like> {
    let info = spec.info(run, &[]).expect("a pipeline's info");
    let region = spec.region(run, info.features.max_lod());
    Like::of(&region, spec.lighting(PassMode::LIT), Leaves::of(&info.features, Some(PassMode::LIT), &[]), run)
}

#[test]
fn a_registered_component_is_offered_everywhere() {
    mr_components::register_on_this_thread(&REGISTERED);
    let mut missing: Vec<(&str, String)> = vec![];
    let mut check = |place: &'static str, ok: bool, why: String| {
        if !ok {
            missing.push((place, why));
        }
    };

    // The registry.
    check("registry", graph::role_components(Role::Storage).contains(&PROBE), "`role_components(storage)` lacks it".into());

    // What a node may be.
    for file in FILES {
        let g = graph::shipped(file).unwrap();
        let choices = g.choices("storage");
        check("Graph::choices", choices.contains(&PROBE), format!("{file} `storage` may be {choices:?}"));
        if let Err(why) = g.edit(&graph::Change::component("storage", PROBE)).and_then(|h| h.accept()) {
            check("Graph::choices", false, format!("{file} with it: {why}"));
        }
    }

    // A sweep over every storage.
    let base = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n";
    for file in FILES {
        let swept = Plan::parse(&format!("{base}[[renderer]]\ngraph = \"{file}\"\nvary = [{{ node = \"storage\", component = \"*\" }}]\n"), &[]);
        let has = swept.as_ref().is_ok_and(|p| p.renderers.iter().any(|r| r.pipeline().is_some_and(|g| g.nodes["storage"].component == PROBE)));
        check("sweep expansion", has, format!("{file} `storage = *`: {:?}", swept.map(|p| p.renderers.iter().map(|r| r.label().to_string()).collect::<Vec<_>>())));
    }

    // `multirenderer list`: the component, and the files that offer it.
    let list = crate::list().unwrap();
    let line = list.lines().find(|l| l.trim_start().starts_with(&format!("{PROBE} "))).unwrap_or_default().to_string();
    check("list", !line.is_empty(), "no line".into());
    for file in FILES {
        check("list", line.contains("(in ") && line.contains(file), format!("not offered in {file}: `{line}`"));
    }

    // `multirenderer audit`: implemented and reached on both kinds (the two files suffice).
    let rows = graph::audit_of(FILES, false);
    match rows.iter().find(|r| r.role == Role::Storage && r.name == PROBE) {
        None => check("audit", false, "no row".into()),
        Some(r) => {
            for (k, file) in [(Kind::Gpu, FILES[0]), (Kind::Cpu, FILES[1])] {
                let u = &r.on[graph::kind_index(k)];
                check("audit", r.registered && !u.apis.is_empty() && u.files.contains(file), format!("{}: {u:?}", k.name()));
            }
        }
    }

    // The catalogue: the effect of choosing it for the storage node.
    let obs = |g: Graph, fps: f64| crate::observe::Observation {
        version: crate::observe::VERSION,
        run: "r".into(),
        plan: "terrain".into(),
        resolution: [64, 64],
        mode: "unlit".into(),
        label: g.label().unwrap(),
        renderer: g.runs_on().into(),
        graph_key: g.key().ok(),
        graph: Some(g),
        options: serde_json::json!({}),
        fps,
        ..Default::default()
    };
    let cat = crate::catalogue::Catalogue::build(&[obs(graph::shipped("raycast-gpu").unwrap(), 100.0), obs(with_probe("raycast-gpu"), 120.0)], 35.0);
    let row = cat.components.iter().find(|c| c.role == "storage").and_then(|c| c.rows.iter().find(|r| r.0 == PROBE));
    check("catalogue", row.is_some_and(|r| r.2 == 1), format!("{:?}", cat.components));

    // A reference class: its lit frames are scored like those of a registered storage with
    // the same capabilities (`bricks`: LOD on the GPU), from what it declares.
    let run = RunSpec { max_lod: 2, ..RunSpec::default() };
    for file in FILES {
        let spec = |storage: &str| RendererSpec::from_cli(&format!("{file},storage={storage}"));
        match (spec(PROBE), spec("bricks")) {
            (Ok(probe), Ok(bricks)) => {
                let (a, b) = (class(&probe, &run), class(&bricks, &run));
                check("reference class", a == b, format!("{file}: {a:?}, bricks {b:?}"));
            }
            (probe, _) => check("reference class", false, format!("{file}: {:?}", probe.err())),
        }
    }

    // The GUI: the editor's choices for the node, not greyed (it runs); the tuner's knob.
    for file in FILES {
        let g = graph::shipped(file).unwrap();
        let options = crate::gui::graph::component_options(&g, "storage");
        let pick = options.iter().find(|o| o.0 == PROBE);
        check("GUI choices", pick.is_some(), format!("{file}: {:?}", options.iter().map(|o| &o.0).collect::<Vec<_>>()));
        if let Some((.., change, _, _)) = pick {
            let spec = RendererSpec::from_cli(file).unwrap();
            let mut trial = crate::gui::graph::model::Trial::new(spec.clone(), g.clone());
            check("GUI choices", trial.try_change(change, &[]).is_ok(), format!("{file}: greyed: {:?}", trial.try_change(change, &[]).err()));
            let knob = spec.component_knobs().into_iter().find(|k| k.name == "storage.component");
            check("GUI choices", knob.as_ref().is_some_and(|k| matches!(&k.kind, mr_core::ParamKind::Choice(v) if v.iter().any(|c| c == PROBE))), format!("{file}: the tuner's `storage.component`: {knob:?}"));
        }
    }

    let mut places: Vec<&str> = missing.iter().map(|(p, _)| *p).collect();
    places.dedup();
    assert_eq!(places, DEBT, "a newly registered component is missing from (place, why): {missing:#?}");
}

/// Baselines stay comparable across turns: the guard matches passes by graph key, recomputed
/// by the current binary from each record's graph, so a turn that registers a component or
/// gives a component a knob (with its default) leaves every existing pipeline's key alone.
/// The probe: with a component registered, no shipped graph's key or label moves; a node
/// that sets a knob to its default ([`mr_components::knobs`]) keys as the node leaving it
/// unset, and off its default apart; a node that is its role's identity
/// ([`graph::identity`]), set to nothing else, keys as the graph without it.
#[test]
fn keys_stay_across_turns() {
    use mr_components::knobs;
    let shipped = || graph::SHIPPED.iter().map(|(name, _)| (*name, graph::shipped(name).unwrap()));
    let seen = |g: &Graph| (g.key().unwrap(), g.label().unwrap());
    let before: Vec<_> = shipped().map(|(_, g)| seen(&g)).collect();
    mr_components::register_on_this_thread(&REGISTERED);
    assert_eq!(shipped().map(|(_, g)| seen(&g)).collect::<Vec<_>>(), before, "registering a component moved a shipped graph's key or label");

    // Knobs at their defaults, on every node of every shipped graph as every component it may be.
    let mut tried = 0;
    for (name, g) in shipped() {
        for id in g.nodes.keys() {
            for c in g.choices(id) {
                let Ok(h) = g.edit(&graph::Change::component(id, c)) else { continue };
                if h.key().is_err() {
                    continue;
                }
                let role = h.nodes[id].role;
                for (knob, d) in knobs::DEFAULTS.iter().filter(|(r, n, _)| *r == role && *n == c).flat_map(|(_, _, ks)| ks.iter()) {
                    let set = |v: toml::Value| {
                        let mut s = h.clone();
                        s.nodes.get_mut(id).unwrap().knobs.insert(knob.to_string(), v);
                        s
                    };
                    assert_eq!(seen(&set(d.value())), seen(&h), "{name} `{id}` = {c}: {knob} at its default keys apart");
                    let off = match *d {
                        knobs::Value::Bool(b) => (!b).into(),
                        knobs::Value::Int(i) => (i + 1).into(),
                        knobs::Value::Float(f) => (f + 1.0).into(),
                        knobs::Value::Str(s) => format!("{s}-not").into(),
                    };
                    assert_ne!(set(off).key().unwrap(), h.key().unwrap(), "{name} `{id}` = {c}: {knob} off its default keys as unset");
                    tried += 1;
                }
            }
        }
    }
    assert!(tried > 30, "the knobs probed: {tried}");

    // The identities: written out, the same graph; another component there, another.
    for (name, g) in shipped() {
        for role in Role::ALL.iter().copied().filter(|r| graph::identity(*r).is_some()) {
            if g.nodes.values().any(|n| n.role == role) {
                continue;
            }
            let with = |c: &str| {
                let mut edits = vec![graph::Change::add(role.name(), role, c)];
                if role == Role::Post {
                    let shading = g.nodes.iter().find(|(_, n)| n.role == Role::Shading).map(|(k, _)| k.clone()).unwrap();
                    edits.push(graph::Change::bind(role.name(), "image", &shading));
                }
                g.with_edits(&edits).unwrap()
            };
            let identity = graph::identity(role).unwrap();
            assert_eq!(seen(&with(identity)), seen(&g), "{name} with its {} `{identity}` written out", role.name());
            if let Some(other) = graph::role_components(role).into_iter().find(|c| *c != identity && with(c).key().is_ok()) {
                assert_ne!(with(other).key().unwrap(), g.key().unwrap(), "{name} with its {} `{other}`", role.name());
            }
        }
    }
}

/// The knob defaults the key reads ([`mr_components::knobs::DEFAULTS`]) are the executors':
/// a component's knob is listed exactly when every region declaring it (`ParamSpec` with an
/// owner) gives it one default, and with that default.
#[test]
fn knob_defaults_are_the_executors() {
    use std::collections::{BTreeMap, BTreeSet};
    let mut declared: BTreeMap<(String, String, String), BTreeSet<String>> = BTreeMap::new();
    for b in graph::REGIONS {
        let info = if b.executor() == "cpu" { mr_exec_cpu::info(b.id) } else { mr_exec_gpu::info(b.id) };
        for p in &info.params {
            let Some((role, components)) = &p.owner else { continue };
            let v = toml::Value::try_from(p.default.to_json()).unwrap().to_string();
            for c in components {
                declared.entry((role.clone(), c.clone(), p.name.clone())).or_default().insert(v.clone());
            }
        }
    }
    let want: BTreeMap<_, _> = declared.into_iter().filter(|(_, v)| v.len() == 1).map(|(k, v)| (k, v.into_iter().next().unwrap())).collect();
    let have: BTreeMap<_, _> = mr_components::knobs::DEFAULTS
        .iter()
        .flat_map(|(r, c, ks)| ks.iter().map(move |(k, d)| ((r.name().to_string(), c.to_string(), k.to_string()), d.value().to_string())))
        .collect();
    let missing: Vec<_> = want.iter().filter(|(k, v)| have.get(*k) != Some(v)).collect();
    let extra: Vec<_> = have.iter().filter(|(k, _)| !want.contains_key(*k)).collect();
    assert!(missing.is_empty() && extra.is_empty(), "knobs::DEFAULTS differs from the executors' knobs: to add or correct (role, component, knob) = default: {missing:#?}; to remove: {extra:#?}");
}
