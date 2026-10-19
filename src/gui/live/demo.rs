//! A manual demo of the tuner on real terrain.

use std::time::Duration;
use crate::tune;

use super::*;

/// Watch the tuner work on real terrain: `cargo test --release --bin multirenderer
/// tuner_demo -- --ignored --nocapture`.
#[test]
#[ignore]
fn tuner_demo() {
    let plan = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/terrain.toml")), &[
        "run.isolation=\"thread\"".into(),
        "renderer=[{name=\"gpu-dda\"}]".into(),
        "run.resolution=[1920,1080]".into(),
    ])
    .unwrap();
    let control = Control { camera: CameraControl::Path, mode: PassMode::LIT, scene_frame: 150, events: false, fps_cap: 0.0, display_hz: 10.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    let t0 = Instant::now();
    while live.shared.lock().unwrap().lanes.first().is_none_or(|l| l.samples.len() < 50) {
        assert!(t0.elapsed().as_secs() < 60);
        std::thread::sleep(Duration::from_millis(50));
    }
    let fps = |s: &Shared| {
        let v = &s.lanes[0].samples;
        let w: Vec<f32> = v.iter().rev().take(200).map(|x| x.interval_ms).filter(|x| *x > 0.0).collect();
        1000.0 / (w.iter().sum::<f32>() / w.len() as f32)
    };
    println!("untuned: {:.0} fps", fps(&live.shared.lock().unwrap()));
    let target = std::env::var("TARGET_FPS").ok().and_then(|s| s.parse().ok()).unwrap_or(900.0);
    let locked = std::env::var("LOCK").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    let algorithm = match std::env::var("ALGO").as_deref() {
        Ok("bayes") => tune::Algorithm::Bayesian,
        Ok("controller") => tune::Algorithm::ResolutionController,
        _ => tune::Algorithm::LocalSearch,
    };
    live.send(Cmd::Tune { lane: 0, config: Some(TunerConfig { goal: tune::Goal::MaxQualityAtFps { fps: target }, locked, algorithm, ..Default::default() }) });
    let secs = std::env::var("SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
    std::thread::sleep(Duration::from_secs(secs));
    let sh = live.shared.lock().unwrap();
    let tv = sh.lanes[0].tuner.as_ref().unwrap();
    println!("target {target} fps → now {:.0} fps; {}; best {:?} {:?}", fps(&sh), tv.status, tv.best, tv.best_diff);
    for l in &tv.log {
        println!("  {l}");
    }
    for (p, v) in &sh.lanes[0].params {
        println!("  knob {} = {}", p.name, v.display());
    }
}
