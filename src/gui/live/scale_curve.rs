//! A manual measurement of quality against render scale.

use std::time::Duration;

use super::*;

/// Prints quality vs render scale for a renderer: `RENDERER=gpu-compute:bricks+refloor cargo test
/// --release --bin multirenderer scale_curve -- --ignored --nocapture`.
#[test]
#[ignore]
fn scale_curve() {
    let renderer = std::env::var("RENDERER").unwrap_or("gpu-compute:bricks+refloor".into());
    let mut plan = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/terrain.toml")), &["run.isolation=\"thread\"".into(), "run.resolution=[1280,720]".into()]).unwrap();
    plan.renderers = vec![RendererSpec::from_cli(&renderer).unwrap()];
    let control = Control { camera: CameraControl::Path, mode: PassMode::LIT, scene_frame: 150, events: false, fps_cap: 0.0, display_hz: 10.0, probe_interval: 0.0, windows: 0, paper_white: 1.0 };
    let live = LiveHandle::start(plan, vec![], control, || {});
    let wait = |f: &dyn Fn(&Shared) -> bool| {
        let t = Instant::now();
        while !f(&live.shared.lock().unwrap()) {
            assert!(t.elapsed().as_secs() < 120, "timeout");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait(&|s| s.lanes.first().is_some_and(|l| l.samples.len() > 100 && l.pending_chunks == 0));
    for scale in [1.0, 0.98, 0.9, 0.75, 0.5, 0.35] {
        live.send(Cmd::SetParam { lane: 0, name: "host.render_scale".into(), value: ParamValue::Float(scale) });
        std::thread::sleep(Duration::from_millis(600));
        let n = live.shared.lock().unwrap().lanes[0].quality.len();
        live.send(Cmd::Compare);
        wait(&|s| s.lanes[0].quality.len() > n);
        let sh = live.shared.lock().unwrap();
        if let (Ok(dir), Some(c)) = (std::env::var("DUMP"), &sh.compare) {
            let _ = image::save_buffer(format!("{dir}/ref.png"), &c.reference, c.width, c.height, image::ExtendedColorType::Rgba8);
            let _ = image::save_buffer(format!("{dir}/lane-{scale:.2}.png"), &c.lanes[0].image, c.width, c.height, image::ExtendedColorType::Rgba8);
        }
        let q = sh.lanes[0].quality.back().unwrap();
        println!("scale {scale:.2} ({}x{}): PSNR {:.1} dB, SSIM {:.1} dB, MS-SSIM {:.1} dB, {:.2}% bad", sh.lanes[0].resolution[0], sh.lanes[0].resolution[1], q.psnr, q.ssim_db, q.ms_ssim_db, q.bad_pct);
    }
}
