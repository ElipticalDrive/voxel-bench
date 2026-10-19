//! End-to-end tests of live sessions.

use std::time::Duration;
use crate::tune;

use super::*;

fn wait(shared: &Arc<Mutex<Shared>>, what: &str, secs: u64, f: impl Fn(&Shared) -> bool) {
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
        compose = { backend = "gpu-compute" }
        [[renderer]]
        compose = { backend = "gpu-raster" }
        [[renderer]]
        compose = { backend = "gpu-compute" }
        label = "lod"
        sites = { client = "pick(radius=2, max_lod=2)" }
        "#,
        &[],
    )
    .unwrap();
    let control = Control { camera: CameraControl::Path, mode: PassMode::UNLIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 60.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan.clone(), vec![], control, || {});
    wait(&live.shared, "frames from all lanes", 60, |s| s.lanes.len() == 3 && s.lanes.iter().all(|l| l.frame.is_some() && l.samples.len() > 20));
    assert!(live.shared.lock().unwrap().lanes[0].params.iter().any(|(p, _)| p.name == "max_steps"));
    // Stage times: GPU stages from timestamps, CPU stages timed by the back end.
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

    // Live compile-time knobs (lane 1, gpu-raster): the drawer is rebuilt at the next frame.
    for (name, value) in [("geometry", "naive"), ("draw", "visbuffer"), ("geometry", "greedy"), ("draw", "raster")] {
        let frames = live.shared.lock().unwrap().lanes[1].frames;
        live.send(Cmd::SetParam { lane: 1, name: name.into(), value: ParamValue::Choice(value.into()) });
        wait(&live.shared, &format!("{name} = {value}"), 20, |s| s.lanes[1].frames > frames + 3 && s.lanes[1].status == LaneStatus::Running);
    }

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
    let given = RendererSpec::from_cli("gpu-compute:bricks+refloor").unwrap();
    let mut now = given.clone();
    now.options.insert("max_steps".into(), toml::Value::Integer(300));
    now.options.insert("accumulate".into(), toml::Value::Boolean(false));
    let value = |s: &RendererSpec, k: &str| s.options.get(k).cloned();
    // The plan unchanged: both kept.
    let kept = keep_knobs(&given, &now, given.clone());
    assert_eq!(value(&kept, "max_steps"), Some(toml::Value::Integer(300)));
    assert_eq!(value(&kept, "accumulate"), Some(toml::Value::Boolean(false)));
    // The plan changes one knob and the storage: its knob and components win.
    let mut new = RendererSpec::from_cli("gpu-compute:prefix+refloor").unwrap();
    new.options.insert("max_steps".into(), toml::Value::Integer(500));
    let kept = keep_knobs(&given, &now, new);
    assert_eq!((value(&kept, "max_steps"), value(&kept, "accumulate")), (Some(toml::Value::Integer(500)), Some(toml::Value::Boolean(false))));
    assert_eq!(kept.options.get("storage").and_then(|v| v.as_str()), Some("prefix"));
    // Another back end: its own knobs only.
    let kept = keep_knobs(&given, &now, RendererSpec::from_cli("gpu-raster").unwrap());
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
        compose = { backend = "gpu-compute" }
        sites = { post = "hdr" }
        "#,
        &[],
    )
    .unwrap();
    let exe = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join(format!("multirenderer{}", std::env::consts::EXE_SUFFIX));
    assert!(exe.exists(), "build the binary first ({})", exe.display());
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
/// A `cpu` lane with `compile = "native"`: the generic build runs first, the pipeline
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
        compose = { backend = "cpu", storage = "prefix" }
        "#,
        &[],
    )
    .unwrap();
    let control = Control { camera: CameraControl::Path, mode: PassMode::UNLIT, scene_frame: 0, events: true, fps_cap: 0.0, display_hz: 60.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    // Process workers start from the real executable, not this test harness
    // (target/<profile>/deps/<test> -> target/<profile>/multirenderer; build it first).
    let exe = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join(format!("multirenderer{}", std::env::consts::EXE_SUFFIX));
    assert!(exe.exists(), "build the binary first ({})", exe.display());
    // SAFETY: set before any worker thread of this test starts; no other test reads it.
    unsafe { std::env::set_var("MR_WORKER_EXE", &exe) };
    let live = LiveHandle::start(plan, vec![], control, || {});
    // The generic build first, then the compiled pipeline swaps in.
    wait(&live.shared, "the swap to start", 30, |s| s.lanes.len() == 1 && s.lanes[0].events.iter().any(|e| e.1.starts_with("hot swap →")));
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
        "renderer=[{compose={backend=\"gpu-raster\"}},{compose={backend=\"cpu\"}}]".into(),
        "run.resolution=[480,270]".into(),
    ])
    .unwrap();
    let mode = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
    let control = Control { camera: CameraControl::Path, mode, scene_frame: 50, events: true, fps_cap: 0.0, display_hz: 30.0, probe_interval: 0.3, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    wait(&live.shared, "probes on both lanes", 120, |s| s.lanes.len() == 2 && s.lanes.iter().all(|l| !l.quality.is_empty()));
    let (with, without) = {
        let sh = live.shared.lock().unwrap();
        (sh.lanes[0].quality.back().unwrap().psnr, sh.lanes[1].quality.back().unwrap().psnr)
    };
    println!("mesh mode probe: gpu-raster {with:.1} dB, cpu (no meshes) {without:.1} dB");
    // The forward drawer draws the meshes in its stage.
    let stages = live.shared.lock().unwrap().lanes[0].stages.clone();
    assert!(stages.iter().any(|s| s.name == "draw+shade" && s.nodes.iter().any(|n| n == "meshes")), "{stages:?}");
    assert!(with > 45.0, "gpu-raster with meshes vs reference: {with:.1} dB");
    assert!(with > without + 5.0, "meshes should matter: gpu-raster {with:.1} dB, cpu (no meshes) {without:.1} dB");
    drop(live);
}
