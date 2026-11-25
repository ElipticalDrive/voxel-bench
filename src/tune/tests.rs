//! Tuner tests against a synthetic renderer.

use super::*;

/// Synthetic renderer: quality rises with scale and steps, cost rises with both.
/// `slowdown` multiplies frame cost (a scene change).
fn simulate(values: &BTreeMap<String, ParamValue>, slowdown: f64) -> (f64, f64) {
    let scale = values["host.render_scale"].as_f64().unwrap();
    let steps = values["max_steps"].as_i64().unwrap() as f64;
    let shadows = values["shadows"].as_str().unwrap() == "all";
    let cost_ms = (0.5 + 6.0 * scale * scale + steps / 2000.0 + if shadows { 2.0 } else { 0.0 }) * slowdown;
    let psnr = 30.0 + 20.0 * scale + (steps.min(512.0) / 512.0) * 5.0 + if shadows { 6.0 } else { 0.0 };
    (1000.0 / cost_ms, psnr)
}

struct Run {
    t: Tuner,
    values: BTreeMap<String, ParamValue>,
    now: f64,
    rng: u64,
}

impl Run {
    fn new(cfg: TunerConfig) -> Self {
        Self::seeded(cfg, 0.0)
    }

    /// The tuner's random seed derives from its start time.
    fn seeded(cfg: TunerConfig, start: f64) -> Self {
        let mut specs = host_params(8);
        specs.push(ParamSpec::int("max_steps", "", 32, 4096, 4096, true));
        specs.push(ParamSpec::choice("shadows", "", &["none", "sun", "all"], "none", true));
        let mut values: BTreeMap<String, ParamValue> = specs.iter().map(|s| (s.name.clone(), s.default.clone())).collect();
        values.insert("host.render_scale".into(), ParamValue::Float(1.0));
        Self { t: Tuner::new(cfg, specs, values.clone(), start), values, now: start, rng: 12345 ^ start.to_bits() }
    }

    /// Runs for `seconds` with per-frame time jitter of `jitter` (fraction).
    fn run(&mut self, seconds: f64, slowdown: f64, jitter: f64) {
        let end = self.now + seconds;
        while self.now < end {
            let (fps, psnr) = simulate(&self.values, slowdown);
            self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r = (self.rng >> 33) as f64 / (1u64 << 31) as f64 - 0.5;
            self.now += (1.0 + r * 2.0 * jitter) / fps;
            self.t.on_frame(self.now, 1000.0 / fps);
            match self.t.tick(self.now) {
                Action::Set(changes) => self.values.extend(changes),
                Action::NeedProbe => self.t.on_probe(self.now, psnr, psnr),
                Action::Idle => {}
            }
        }
    }
}

#[test]
fn local_search_meets_fps_target_and_improves_quality() {
    let mut r = Run::new(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 150.0 }, patience: 0, ..Default::default() });
    r.run(120.0, 1.0, 0.0);
    let best = r.t.best_values().unwrap().clone();
    let (fps, psnr) = simulate(&best, 1.0);
    assert!(fps >= 150.0 * 0.95, "fps {fps} best {best:?}\n{:?}", r.t.log);
    assert!(psnr > 45.0, "psnr {psnr} best {best:?}\n{:?}", r.t.log);
    assert!(r.t.accepted > 0);
}

#[test]
fn bayesian_meets_fps_target_and_improves_quality() {
    let mut r = Run::new(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 150.0 }, algorithm: Algorithm::Bayesian, patience: 0, ..Default::default() });
    r.run(90.0, 1.0, 0.0);
    let best = r.t.best_values().unwrap().clone();
    let (fps, psnr) = simulate(&best, 1.0);
    assert!(fps >= 147.0 && psnr > 50.0, "best {fps:.0} fps {psnr:.1} dB\n{:?}", r.t.log);
}

#[test]
fn locked_knobs_are_never_touched() {
    let mut cfg = TunerConfig { goal: Goal::MaxFps, ..Default::default() };
    cfg.locked = ["host.render_scale".to_string(), "shadows".to_string()].into();
    let mut r = Run::new(cfg);
    r.run(40.0, 1.0, 0.0);
    assert_eq!(r.values["host.render_scale"], ParamValue::Float(1.0));
    assert_eq!(r.values["shadows"], ParamValue::Choice("none".into()));
}

#[test]
fn controller_converges_on_target() {
    let mut r = Run::new(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 200.0 }, algorithm: Algorithm::ResolutionController, ..Default::default() });
    r.run(20.0, 1.0, 0.0);
    let (fps, _) = simulate(&r.values, 1.0);
    assert!((190.0..260.0).contains(&fps), "fps {fps} values {:?}", r.values);
}

#[test]
fn settles_on_best_then_resumes_when_the_scene_changes() {
    for (algorithm, seed) in [Algorithm::LocalSearch, Algorithm::Bayesian].into_iter().flat_map(|a| (1..=4).map(move |s| (a, s as f64 * 0.37))) {
        let mut r = Run::seeded(TunerConfig { goal: Goal::MaxQualityAtFps { fps: 150.0 }, algorithm, patience: 10, ..Default::default() }, seed);
        r.run(200.0, 1.0, 0.05);
        assert!(r.t.holding(), "{algorithm:?} seed {seed} should have settled\n{:?}", r.t.log);
        // Holding means the best configuration is applied.
        assert_eq!(Some(&r.values), r.t.best_values(), "{algorithm:?}");
        let (fps, _) = simulate(&r.values, 1.0);
        assert!(fps >= 145.0, "{algorithm:?} seed {seed} settled at {fps:.0} fps\n{:?}", r.t.log);
        // The scene gets 60 % more expensive: the target is missed, so it explores again.
        r.run(20.0, 1.6, 0.05);
        assert!(r.t.log.iter().any(|l| l.contains("exploring again")), "{algorithm:?} seed {seed}\n{:?}", r.t.log);
        r.run(200.0, 1.6, 0.05);
        let best = r.t.best_values().unwrap().clone();
        let (fps, _) = simulate(&best, 1.6);
        assert!(fps >= 145.0, "{algorithm:?} seed {seed} re-tuned best {fps:.0} fps\n{:?}", r.t.log);
    }
}

#[test]
fn noise_does_not_produce_false_improvements() {
    // Only one knob, and it does nothing: every "improvement" would be noise.
    let mut specs = vec![ParamSpec::int("useless", "", 0, 100, 50, true)];
    specs.extend(host_params(8));
    let cfg = TunerConfig { goal: Goal::MaxFps, locked: ["host.render_scale", "host.view_radius", "host.in_flight"].iter().map(|s| s.to_string()).collect(), patience: 0, ..Default::default() };
    let values: BTreeMap<String, ParamValue> = specs.iter().map(|s| (s.name.clone(), s.default.clone())).collect();
    let mut t = Tuner::new(cfg, specs, values, 0.0);
    let (mut now, mut rng) = (0.0, 7u64);
    while now < 120.0 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = (rng >> 33) as f64 / (1u64 << 31) as f64 - 0.5;
        now += (1.0 + r * 0.6) / 200.0; // 200 fps, ±30 % frame-time jitter
        t.on_frame(now, 5.0);
        t.tick(now);
    }
    assert!(t.trials > 20);
    assert!(t.accepted <= 2, "accepted {} of {} trials on pure noise\n{:?}", t.accepted, t.trials, t.log);
}
