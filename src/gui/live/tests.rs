//! End-to-end tests of live sessions.

use std::time::Duration;
use crate::tune;

use super::*;

/// Waits up to `secs`, times `MR_TEST_SLOW` (a factor; `check.py lavapipe` sets it: software
/// Vulkan on a CI runner's few cores shares them with the CPU reference and every worker).
fn wait(shared: &Arc<Mutex<Shared>>, what: &str, secs: u64, f: impl Fn(&Shared) -> bool) {
    let slow = std::env::var("MR_TEST_SLOW").ok().and_then(|v| v.parse::<f64>().ok()).filter(|f| *f >= 1.0).unwrap_or(1.0);
    let secs = (secs as f64 * slow).ceil() as u64;
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if f(&shared.lock().unwrap()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let msg = {
        let sh = shared.lock().unwrap();
        format!("timed out waiting for {what}; status {}; log:\n{}", sh.status, sh.log.join("\n"))
    };
    panic!("{msg}");
}

/// The built host executable, for process workers (they start from the real executable, not
/// this test harness): the nearest ancestor of the harness that holds one, `target/<profile>/`,
/// whatever cargo's layout for test binaries is (cargo 1.100 moved them under `build/`).
pub(crate) fn built_host() -> std::path::PathBuf {
    let me = std::env::current_exe().unwrap();
    let name = format!("multirenderer{}", std::env::consts::EXE_SUFFIX);
    me.ancestors().map(|d| d.join(&name)).find(|p| p.is_file()).unwrap_or_else(|| panic!("build the binary first (none above {})", me.display()))
}

/// Drives a whole live session headlessly (needs a GPU; run with `--ignored`).
#[test]
#[ignore]
fn live_session_end_to_end() {
    let plan = Plan::parse(
        r#"
        [scene]
        generator = { type = "showcase" }
        camera = { type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }
        [run]
        resolution = [320, 180]
        view_radius = 3
        isolation = "thread"
        [[renderer]]
        graph = "raycast-gpu"
        [[renderer]]
        graph = "raster-gpu"
        [[renderer]]
        graph = "raycast-gpu"
        label = "lod"
        edits = [{ add = "client", role = "client", component = "pick" }, { node = "client", knobs = { radius = 2, max_lod = 2 } }]
        "#,
        &[],
    )
    .unwrap();
    let control = Control { camera: CameraControl::Path, mode: PassMode::UNLIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 60.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan.clone(), vec![], control, || {});
    wait(&live.shared, "frames from all lanes", 60, |s| s.lanes.len() == 3 && s.lanes.iter().all(|l| l.frame.is_some() && l.samples.len() > 20));
    assert!(live.shared.lock().unwrap().lanes[0].params.iter().any(|(p, _)| p.name == "max_steps"));
    // Stage times: GPU stages from timestamps, CPU stages timed by the preset.
    {
        let sh = live.shared.lock().unwrap();
        let has = |lane: usize, name: &str, nodes: &[&str], gpu: bool| sh.lanes[lane].stages.iter().any(|s| s.name == name && s.nodes == nodes && if gpu { s.gpu_ms.is_some_and(|x| x > 0.0) } else { s.cpu_ms.is_some() });
        assert!(has(0, "cast+shade", &["visibility", "shading"], true), "{:?}", sh.lanes[0].stages);
        assert!(has(1, "geometry", &["geometry"], false) && has(1, "draw+shade", &["visibility", "draw", "shading"], true), "{:?}", sh.lanes[1].stages);
    }

    let cam = live.shared.lock().unwrap().camera.unwrap();
    live.control.lock().unwrap().camera = CameraControl::Free(Camera { position: cam.position + DVec3::new(0.0, 2.0, 0.0), ..cam });
    live.control.lock().unwrap().mode = PassMode::LIT;

    live.send(Cmd::Edit { origin: DVec3::new(20.5, 6.0, 0.5), dir: Vec3::NEG_Y, place: None });
    live.send(Cmd::AddLight { position: DVec3::new(20.0, 4.0, 2.0), color: Vec3::ONE, intensity: 30.0, radius: 20.0 });
    wait(&live.shared, "light", 10, |s| s.lights == 1);

    // Live knobs: a renderer parameter and the render scale (resize).
    live.send(Cmd::SetParam { lane: 0, name: "shadows".into(), value: ParamValue::Choice("sun".into()) });
    live.send(Cmd::SetParam { lane: 0, name: "host.render_scale".into(), value: ParamValue::Float(0.5) });
    wait(&live.shared, "resize", 20, |s| s.lanes[0].frame.as_ref().is_some_and(|f| f.width == 160 && f.height == 90));

    // Live component switches as graph edits (lane 1, raster-gpu): the drawer is rebuilt at
    // the next frame.
    for (node, value) in [("geometry", "naive"), ("draw", "visbuffer"), ("geometry", "greedy"), ("draw", "raster")] {
        let (frames, spec, params) = {
            let sh = live.shared.lock().unwrap();
            (sh.lanes[1].frames, sh.lanes[1].spec.clone(), sh.lanes[1].params.clone())
        };
        let mut new = spec.clone();
        new.set_component(node, value).unwrap();
        let knobs = crate::gui::graph::live_edits(&spec, &new, &params).unwrap_or_else(|| panic!("{node} = {value} live"));
        live.send(Cmd::Edits { lane: 1, spec: new, knobs });
        wait(&live.shared, &format!("{node} = {value}"), 20, |s| s.lanes[1].frames > frames + 3 && s.lanes[1].status == LaneStatus::Running && s.lanes[1].spec.graph.as_ref().is_some_and(|g| g.nodes[node].component == value));
    }
    // The same through the host's knob for a node's component (the tuner's way): live here,
    // no hot swap.
    let frames = live.shared.lock().unwrap().lanes[1].frames;
    live.send(Cmd::SetParam { lane: 1, name: "geometry.component".into(), value: ParamValue::Choice("naive".into()) });
    wait(&live.shared, "geometry.component = naive", 20, |s| s.lanes[1].frames > frames + 3 && s.lanes[1].status == LaneStatus::Running && s.lanes[1].spec.graph.as_ref().is_some_and(|g| g.nodes["geometry"].component == "naive") && s.lanes[1].params.iter().any(|(p, v)| p.name == "geometry.component" && v.as_str() == Some("naive")));
    assert!(live.shared.lock().unwrap().lanes[1].events.iter().all(|(_, e)| !e.starts_with("hot swap")), "a live edit, not a swap");
    // The worker took every edit (a refusal is an event of the lane, not an error).
    let refused: Vec<String> = live.shared.lock().unwrap().lanes[1].events.iter().filter(|(_, e)| e.starts_with("refused")).map(|(_, e)| e.clone()).collect();
    assert!(refused.is_empty(), "{refused:?}");

    // Hot swap (restart-only knob): lane 1 must keep producing frames throughout.
    let frames_before = live.shared.lock().unwrap().lanes[1].frames;
    live.send(Cmd::SetParam { lane: 1, name: "frames_in_flight".into(), value: ParamValue::Int(2) });
    wait(&live.shared, "hot swap start", 10, |s| s.lanes[1].swap.is_some());
    wait(&live.shared, "hot swap complete", 60, |s| s.lanes[1].swap.is_none() && s.lanes[1].events.iter().any(|e| e.1 == "hot swap complete"));
    assert!(live.shared.lock().unwrap().lanes[1].frames > frames_before + 5, "lane kept rendering during the swap");

    // LOD depth is a restart knob: sliding it hot-swaps the LOD lane.
    live.send(Cmd::SetParam { lane: 2, name: "host.max_lod".into(), value: ParamValue::Int(1) });
    wait(&live.shared, "lod hot swap", 60, |s| s.lanes[2].events.iter().filter(|e| e.1 == "hot swap complete").count() >= 1 && s.lanes[2].swap.is_none());
    assert!(live.shared.lock().unwrap().lanes[2].params.iter().any(|(p, v)| p.name == "host.max_lod" && *v == ParamValue::Int(1)));

    // ↻ from the plan keeps the knob values changed live (lane 0's shadows and render
    // scale, lane 2's LOD depth).
    for lane in [0, 2] {
        let swaps = live.shared.lock().unwrap().lanes[lane].events.iter().filter(|e| e.1 == "hot swap complete").count();
        live.send(Cmd::Restart { lane, spec: plan.renderers[lane].clone() });
        wait(&live.shared, "restart", 60, |s| s.lanes[lane].swap.is_none() && s.lanes[lane].events.iter().filter(|e| e.1 == "hot swap complete").count() > swaps);
    }
    {
        let sh = live.shared.lock().unwrap();
        let value = |lane: usize, name: &str| sh.lanes[lane].params.iter().find(|(p, _)| p.name == name).map(|(_, v)| v.clone());
        assert_eq!(value(0, "shadows"), Some(ParamValue::Choice("sun".into())));
        assert_eq!(value(0, "host.render_scale"), Some(ParamValue::Float(0.5)));
        assert_eq!(value(2, "host.max_lod"), Some(ParamValue::Int(1)));
        assert!(sh.lanes[0].spec.options.get("shadows").is_some_and(|v| v.as_str() == Some("sun")));
    }

    // Comparison (keeps images, includes a lane at half resolution).
    live.send(Cmd::Compare);
    wait(&live.shared, "comparison", 60, |s| s.compare.is_some());
    {
        let sh = live.shared.lock().unwrap();
        let c = sh.compare.as_ref().unwrap();
        assert_eq!(c.lanes.len(), 3);
        assert!(c.lanes[2].quality.psnr > 15.0, "LOD lane (2-block cells cannot show the 1-block floor checker): {}", c.lanes[2].quality.psnr);
        assert!(c.lanes[1].quality.psnr > 35.0, "{} psnr {}", c.lanes[1].label, c.lanes[1].quality.psnr);
        assert!(c.lanes[0].quality.psnr > 9.0, "half-res lane still comparable (aliasing at 160x90 is heavy): {:?} {:?} ref {}x{}", c.lanes[0].quality, sh.lanes[0].resolution, c.width, c.height);
        assert!(sh.lanes.iter().all(|l| l.status == LaneStatus::Running), "{:?}", sh.log);
    }

    // Tuner: max fps with quality ignored; it must act and record history.
    live.send(Cmd::Tune { lane: 1, config: Some(TunerConfig { goal: tune::Goal::MaxFps, settle_s: 0.1, window_s: 0.3, ..Default::default() }) });
    wait(&live.shared, "tuner trials", 30, |s| s.lanes[1].tuner.as_ref().is_some_and(|t| t.trials >= 2));
    // Stopping restores the best configuration found.
    live.send(Cmd::Tune { lane: 1, config: None });
    wait(&live.shared, "tuner stopped", 10, |s| s.lanes[1].tuner.is_none() && s.lanes[1].events.iter().any(|(_, e)| e.starts_with("tuner stopped")));
    let restored: Vec<(String, String)> = {
        let sh = live.shared.lock().unwrap();
        let (_, e) = sh.lanes[1].events.iter().rev().find(|(_, e)| e.starts_with("tuner stopped")).unwrap();
        e.split_once(": ").unwrap().1.split(", ").filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
    };
    wait(&live.shared, "best restored", 10, |s| restored.iter().all(|(k, v)| s.lanes[1].params.iter().any(|(p, x)| &p.name == k && &x.display() == v)));

    live.send(Cmd::RemoveLane(0));
    wait(&live.shared, "remove", 10, |s| s.lanes.len() == 2);
    // A restart of the session (F5) keeps them too; with a lane removed, each lane
    // finds its predecessor by the spec the plan gave it.
    let carry = Carry::of(&live.shared.lock().unwrap().lanes);
    drop(live);
    let live = LiveHandle::start(plan, carry, control, || {});
    wait(&live.shared, "restarted session", 60, |s| s.lanes.len() == 3 && s.lanes.iter().all(|l| l.frame.is_some()));
    {
        let sh = live.shared.lock().unwrap();
        let value = |lane: usize, name: &str| sh.lanes[lane].params.iter().find(|(p, _)| p.name == name).map(|(_, v)| v.clone());
        assert_eq!(value(0, "shadows"), Some(ParamValue::Choice("all".into())), "the removed lane's values stay with it");
        assert_eq!(value(1, "frames_in_flight"), Some(ParamValue::Int(2)));
        assert_eq!(value(2, "host.max_lod"), Some(ParamValue::Int(1)));
    }
    drop(live);
}

/// A restart from the plan keeps the knobs changed since, unless the plan changes them.
#[test]
fn restarts_keep_knobs() {
    let given = RendererSpec::from_cli("raycast-gpu").unwrap();
    let mut now = given.clone();
    now.options.insert("max_steps".into(), toml::Value::Integer(300));
    now.options.insert("accumulate".into(), toml::Value::Boolean(false));
    let value = |s: &RendererSpec, k: &str| s.options.get(k).cloned();
    // The plan unchanged: both kept.
    let kept = keep_knobs(&given, &now, given.clone());
    assert_eq!(value(&kept, "max_steps"), Some(toml::Value::Integer(300)));
    assert_eq!(value(&kept, "accumulate"), Some(toml::Value::Boolean(false)));
    // The plan changes one knob and the storage: its knob and components win.
    let mut new = RendererSpec::from_cli("raycast-gpu,storage=prefix").unwrap();
    new.options.insert("max_steps".into(), toml::Value::Integer(500));
    let kept = keep_knobs(&given, &now, new);
    assert_eq!((value(&kept, "max_steps"), value(&kept, "accumulate")), (Some(toml::Value::Integer(500)), Some(toml::Value::Boolean(false))));
    assert_eq!(kept.graph.as_ref().unwrap().nodes["storage"].component, "prefix");
    // Another preset: its own knobs only.
    let kept = keep_knobs(&given, &now, RendererSpec::from_cli("raster-gpu").unwrap());
    assert_eq!(value(&kept, "max_steps"), None);
    // The LOD depth (a client knob).
    let mut now = given.clone();
    now.set_client_knob("max_lod", Some(2)).unwrap();
    assert_eq!(keep_knobs(&given, &now, given.clone()).client_knob("max_lod"), Some(2));
}

/// A lane in a presenter window of its own (needs a GPU and a display; run with
/// `--ignored`): the window opens, reports how it presents, shows frames, and closes.
#[test]
#[ignore]
fn live_hdr_window() {
    let plan = Plan::parse(
        r#"
        [scene]
        generator = { type = "showcase" }
        camera = { type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }
        [run]
        resolution = [320, 180]
        view_radius = 3
        isolation = "thread"
        [[renderer]]
        graph = "raycast-gpu"
        edits = [{ add = "post", role = "post", component = "hdr" }, { node = "post", input = "image", from = "shading" }]
        "#,
        &[],
    )
    .unwrap();
    let exe = built_host();
    // SAFETY: set before any thread of this test starts; the other tests that read it set the same.
    unsafe { std::env::set_var("MR_WORKER_EXE", &exe) };
    let control = Control { camera: CameraControl::Path, mode: PassMode::LIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 30.0, probe_interval: 0.0, windows: 1, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    wait(&live.shared, "the window's status", 30, |s| s.lanes.first().is_some_and(|l| l.window.is_some()));
    let frames = live.shared.lock().unwrap().lanes[0].frames;
    wait(&live.shared, "frames while the window is open", 30, |s| s.lanes[0].frames > frames + 30);
    assert!(live.shared.lock().unwrap().closed_windows.is_empty(), "the window went: {:?}", live.shared.lock().unwrap().log);
    eprintln!("window: {}", live.shared.lock().unwrap().lanes[0].window.as_deref().unwrap());
    live.control.lock().unwrap().windows = 0;
    wait(&live.shared, "the window to close", 10, |s| s.lanes[0].window.is_none());
}

/// Mesh mode through a live session (needs a GPU; run with `--ignored`): the probe's
/// reference draws the meshes, so a mesh-capable lane matches it closely and a lane
/// without mesh support does not.
/// A `raycast-cpu` lane with `compile = "native"`: the generic build runs first, the pipeline
/// compiled for this machine (built and checked in the background) hot-swaps in.
#[test]
#[ignore]
fn live_compiled_pipeline() {
    let plan = Plan::parse(
        r#"
        [scene]
        generator = { type = "showcase" }
        camera = { type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }
        [run]
        resolution = [320, 180]
        view_radius = 3
        isolation = "process"
        compile = "native"
        [[renderer]]
        graph = "raycast-cpu"
        edits = [{ node = "storage", component = "prefix" }]
        "#,
        &[],
    )
    .unwrap();
    let control = Control { camera: CameraControl::Path, mode: PassMode::UNLIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 60.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    let exe = built_host();
    // SAFETY: set before any worker thread of this test starts; no other test reads it.
    unsafe { std::env::set_var("MR_WORKER_EXE", &exe) };
    let live = LiveHandle::start(plan, vec![], control, || {});
    // The generic build first, then the compiled pipeline swaps in.
    wait(&live.shared, "the swap to start", 30, |s| s.lanes.len() == 1 && s.lanes[0].events.iter().any(|e| e.1.starts_with("hot swap ➡")));
    wait(&live.shared, "the compiled pipeline swapped in", 600, |s| s.lanes[0].events.iter().any(|e| e.1 == "hot swap complete" || e.1.starts_with("hot swap failed")));
    let sh = live.shared.lock().unwrap();
    assert!(!sh.log.iter().any(|l| l.contains("failed")), "{:?}", sh.log);
    assert!(sh.lanes[0].events.iter().any(|e| e.1 == "hot swap complete"), "{:?}", sh.lanes[0].events);
    assert!(sh.lanes[0].frame.is_some() && !sh.lanes[0].samples.is_empty());
    drop(sh);
    live.send(Cmd::Stop);
}

#[test]
#[ignore]
fn live_meshes_end_to_end() {
    let plan = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/meshes.toml")), &[
        "run.isolation=\"thread\"".into(),
        "renderer=[{graph=\"raster-gpu\"},{graph=\"raycast-cpu\"},{graph=\"raster-gpu\",edits=[{node=\"draw\",component=\"visbuffer\"}]}]".into(),
        "run.resolution=[480,270]".into(),
    ])
    .unwrap();
    let mode = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
    let control = Control { camera: CameraControl::Path, mode, scene_frame: 50, events: true, fps_cap: 0.0, display_hz: 30.0, probe_interval: 0.3, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    wait(&live.shared, "probes on every lane", 120, |s| s.lanes.len() == 3 && s.lanes.iter().all(|l| !l.quality.is_empty()));
    let [forward, without, deferred] = {
        let sh = live.shared.lock().unwrap();
        std::array::from_fn(|i| sh.lanes[i].quality.back().unwrap().psnr)
    };
    println!("mesh mode probe: raster-gpu {forward:.1} dB, visbuffer {deferred:.1} dB, cpu (no meshes) {without:.1} dB");
    // The forward drawer draws the meshes in its stage.
    let stages = live.shared.lock().unwrap().lanes[0].stages.clone();
    assert!(stages.iter().any(|s| s.name == "draw+shade" && s.nodes.iter().any(|n| n == "meshes")), "{stages:?}");
    assert!(forward > without + 5.0, "meshes should matter: raster-gpu {forward:.1} dB, cpu (no meshes) {without:.1} dB");
    // The deferred shading's rays meet the meshes (shadows by meshes, as the reference's);
    // the forward drawer's do not yet.
    assert!(deferred > 45.0, "raster-gpu visbuffer with meshes vs reference: {deferred:.1} dB");
    drop(live);
}

/// Pipelines split over two GPUs at the hit records (needs a GPU; run with `--ignored`): ray
/// casting's deferred records, the three face executors' gbuffer and visbuffer and the splats' ids, shaded on `gpu1` (the same GPU
/// when there is one), match the same graph on one GPU, and the transfer's stages show.
#[test]
#[ignore]
fn live_split_end_to_end() {
    // The shading's own storage and kernel (a node compiled into callers on two GPUs is
    // refused); meshes stay on one GPU.
    let own = r#"{ add = "s.storage", role = "storage", component = "bricks" }, { node = "s.storage", input = "changes", from = "world" }, { add = "s.kernel", role = "traversal", component = "refloor" }, { node = "s.kernel", input = "storage", from = "s.storage" }, { node = "shading", input = "shadow", from = "s.kernel" }"#;
    let graphs = [
        ("raycast-gpu", format!(r#"{{ remove = "meshes" }}, {{ node = "shading", knob = "placement", value = "deferred" }}, {own}"#)),
        ("raster-gpu", r#"{ remove = "meshes" }, { node = "draw", component = "gbuffer" }"#.to_string()),
        ("discover-gpu", format!(r#"{{ node = "draw", component = "gbuffer" }}, {own}"#)),
        // Its shading's walks are its own already.
        ("discover-cpu", r#"{ node = "draw", component = "gbuffer" }"#.to_string()),
        // Visbuffer: its ids decoded into records on the first GPU.
        ("raster-gpu", r#"{ remove = "meshes" }, { node = "draw", component = "visbuffer" }"#.to_string()),
        ("discover-cpu", r#"{ node = "draw", component = "visbuffer" }"#.to_string()),
        // Splats: their ids decoded into records on the first GPU (the shading's walks its own).
        ("splat-gpu", String::new()),
    ];
    let renderers: String = graphs
        .iter()
        .flat_map(|(g, e)| {
            let split = [e.as_str(), r#"{ node = "shading", device = "gpu1" }"#].into_iter().filter(|e| !e.is_empty()).collect::<Vec<_>>().join(", ");
            [format!("[[renderer]]\ngraph = \"{g}\"\nedits = [{e}]\n"), format!("[[renderer]]\ngraph = \"{g}\"\nedits = [{split}]\n")]
        })
        .collect();
    let plan = Plan::parse(
        &format!(
            r#"
        [scene]
        generator = {{ type = "showcase" }}
        camera = {{ type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }}
        [run]
        resolution = [320, 180]
        view_radius = 3
        isolation = "thread"
        {renderers}"#
        ),
        &[],
    )
    .unwrap();
    let mode = PassMode { lighting: mr_core::RenderMode::Lit, meshes: false };
    let control = Control { camera: CameraControl::Path, mode, scene_frame: 50, events: true, fps_cap: 0.0, display_hz: 30.0, probe_interval: 0.3, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    let n = graphs.len() * 2;
    // Each lane's best of five probes: discovery converges over the first frames, and
    // discover-cpu's probes dip now and then on a still view (one GPU or two alike; three
    // were not enough on lavapipe, one run in five 2 dB apart).
    wait(&live.shared, "probes on every lane", 120, |s| s.lanes.len() == n && s.lanes.iter().all(|l| l.quality.len() >= 5));
    let sh = live.shared.lock().unwrap();
    assert!(!sh.log.iter().any(|l| l.contains("failed")), "{:?}", sh.log);
    for (i, (g, edits)) in graphs.iter().enumerate() {
        let (one, two) = (&sh.lanes[2 * i], &sh.lanes[2 * i + 1]);
        let best = |l: &LaneView| l.quality.iter().map(|q| q.psnr).fold(f64::MIN, f64::max);
        let (a, b) = (best(one), best(two));
        let ids = edits.contains("visbuffer") || *g == "splat-gpu";
        let draw = match *g {
            "splat-gpu" => "splats",
            "raycast-gpu" => "records",
            _ if ids => "visbuffer",
            _ => "gbuffer",
        };
        println!("{g} {draw}: one GPU {a:.1} dB, split {b:.1} dB");
        // Near the reference a pixel or two swing the score by 10 dB: splats' records shade
        // a shadow edge pixel apart from their local resolve (the hit point rebuilt from the
        // record's distance), either way.
        let near_perfect = a.min(b) > 60.0;
        assert!(a > 35.0 && ((a - b).abs() < 1.0 || near_perfect), "{g} {draw}: one GPU {a:.1} dB, split {b:.1} dB");
        for stage in ["copy", "transfer"] {
            assert!(two.stages.iter().any(|s| s.name == stage), "{g} split: no `{stage}` stage in {:?}", two.stages);
            assert!(!one.stages.iter().any(|s| s.name == stage), "{g} on one GPU: a `{stage}` stage");
        }
        let decodes = two.stages.iter().any(|s| s.name == "decode");
        assert_eq!(decodes, ids, "{g} split: `decode` stage in {:?}", two.stages);
    }
    drop(sh);
    drop(live);
}

/// The live GUI outlives any renderer (`MR_FAULT_INJECTION`): one session per isolation, a
/// faulty lane per moment x way beside a healthy one. Both run at once, each in a child of this test
/// binary (`gui_fault_session`), where the environment allows injection (no `set_var` in a
/// threaded harness). CPU only: runs everywhere.
#[test]
fn gui_survives_faults() {
    let host = built_host();
    let me = std::env::current_exe().unwrap();
    let runs: Vec<_> = ["process", "thread"]
        .into_iter()
        .map(|iso| {
            let c = std::process::Command::new(&me)
                .args(["--exact", "gui::live::tests::gui_fault_session", "--ignored", "--nocapture", "--test-threads=1"])
                .env("MR_FAULT_INJECTION", "1")
                .env("MR_WORKER_EXE", &host)
                .env("MR_GUI_FAULT_ISOLATION", iso)
                .env("RUST_LOG", "warn")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            (iso, c)
        })
        .collect();
    for (iso, c) in runs {
        let o = c.wait_with_output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success() && out.contains("1 passed"), "{iso}: {}\n{out}", o.status);
    }
}

/// One live session with injected faults (run by [`gui_survives_faults`] with the
/// environment it needs; does nothing without it).
#[test]
#[ignore]
fn gui_fault_session() {
    let Ok(iso) = std::env::var("MR_GUI_FAULT_ISOLATION") else { return };
    let slow = std::env::var("MR_TEST_SLOW").ok().and_then(|v| v.parse::<f64>().ok()).filter(|f| *f >= 1.0).unwrap_or(1.0);
    // A thread shares the host's process: abort and segfault end it (the defined cost, the
    // scripted survival tests show it), so a thread session leaves them out.
    let ways: &[&str] = if iso == "process" { &["error", "panic", "abort", "segv", "hang"] } else { &["error", "panic", "hang"] };
    let moments = ["build", "first", "mid", "knob", "world", "shutdown"];
    let faults: Vec<(&str, &str)> = moments.iter().flat_map(|a| ways.iter().map(move |h| (*a, *h))).collect();
    let mut plan = format!(
        r#"[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
resolution = [32, 18]
view_radius = 1
isolation = "{iso}"
watchdog = {{ build_s = {}, factor = 20, min_s = {} }}
[[renderer]]
graph = "raycast-cpu"
label = "healthy"
"#,
        10.0 * slow,
        3.0 * slow
    );
    for (at, how) in &faults {
        plan += &format!("[[renderer]]\ngraph = \"raycast-cpu\"\nlabel = \"{at}:{how}\"\noptions = {{ fault = \"{at}:{how}\" }}\n");
    }
    // Last, a lane whose hot swap brings a replacement that hangs warming up.
    plan += "[[renderer]]\ngraph = \"raycast-cpu\"\nlabel = \"swap\"\n";
    let plan = Plan::parse(&plan, &[]).unwrap();
    let control = Control { camera: CameraControl::Path, mode: PassMode::UNLIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 30.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    let shared = live.shared.clone();
    let swap = faults.len() + 1;
    let n = swap + 1;
    let lane = |at: &str, how: &str| 1 + faults.iter().position(|f| *f == (at, how)).unwrap();
    let says = |how: &str| match (how, iso.as_str()) {
        ("error", _) => "injected fault",
        ("panic", _) => "panicked: injected fault",
        ("abort", _) => "SIGABRT",
        ("segv", _) => "SIGSEGV",
        ("hang", "process") => "killed",
        ("hang", _) => "abandoned",
        _ => unreachable!(),
    };
    // The lanes faulting at build, the first or the tenth frame failed, each saying why.
    let settled = |s: &Shared, ats: &[&str]| {
        s.lanes.len() == n && faults.iter().all(|(at, how)| !ats.contains(at) || matches!(&s.lanes[lane(at, how)].status, LaneStatus::Failed(m) if m.contains(says(how))))
    };
    wait(&shared, "build, first and mid faults to fail their lanes", 60, |s| settled(s, &["build", "first", "mid"]));
    // A live knob of the renderer's (a bool toggled, else an integer stepped) changed on every
    // lane, and a world edit.
    let (name, value) = {
        let sh = shared.lock().unwrap();
        let knob = |want: fn(&ParamValue) -> Option<ParamValue>| sh.lanes[0].params.iter().find_map(|(p, v)| (p.live && !p.name.starts_with("host.")).then(|| want(v)).flatten().map(|w| (p.name.clone(), w)));
        knob(|v| v.as_bool().map(|b| ParamValue::Bool(!b))).or_else(|| knob(|v| v.as_i64().map(|x| ParamValue::Int(x + 1)))).expect("a live knob on raycast-cpu")
    };
    for i in 0..n {
        live.send(Cmd::SetParam { lane: i, name: name.clone(), value: value.clone() });
    }
    live.send(Cmd::Edit { origin: DVec3::new(5.5, 30.0, 0.5), dir: Vec3::NEG_Y, place: None });
    let mut hangs = shared.lock().unwrap().lanes[swap].spec.clone();
    hangs.options.insert("fault".into(), toml::Value::String("first:hang".into()));
    live.send(Cmd::Swap { lane: swap, spec: hangs });
    // A knob's error is a refusal (an event; the lane goes on); anything worse fails the lane.
    let knob_ok = |s: &Shared, how: &str| {
        let l = &s.lanes[lane("knob", how)];
        matches!(&l.status, LaneStatus::Failed(m) if m.contains(says(how))) || (how == "error" && l.events.iter().any(|(_, e)| e.contains(says(how))))
    };
    wait(&shared, "knob and world faults", 60, |s| settled(s, &["build", "first", "mid", "world"]) && ways.iter().all(|h| knob_ok(s, h)));
    // The hung replacement: the watchdog ends the swap, the lane keeps its own worker.
    wait(&shared, "the hung hot swap to be given up", 60, |s| s.lanes[swap].events.iter().any(|(_, e)| e.starts_with("hot swap") && e.contains(says("hang"))));
    // The healthy lane and the shutdown ones render on.
    let frames: Vec<u64> = shared.lock().unwrap().lanes.iter().map(|l| l.frames).collect();
    wait(&shared, "the healthy and shutdown lanes rendering on", 30, |s| {
        [0, swap].into_iter().chain(ways.iter().map(|h| lane("shutdown", h))).all(|i| s.lanes[i].status == LaneStatus::Running && s.lanes[i].frames > frames[i] + 5)
    });
    // Stopping ends, and says what went wrong while stopping.
    let t = Instant::now();
    drop(live);
    let sh = shared.lock().unwrap();
    eprintln!("stopped in {:.1?}; log:\n{}", t.elapsed(), sh.log.join("\n"));
    for how in ways {
        let i = lane("shutdown", how);
        let said = sh.log.iter().any(|l| l.starts_with(&format!("lane {} while stopping", i + 1)));
        assert!(said, "shutdown:{how}: nothing said while stopping");
    }
    assert!(!sh.log.iter().any(|l| l.starts_with("lane 1 ")), "the healthy lane: {:?}", sh.log);
}
