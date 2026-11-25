//! GUI tests: scrolling, plans, and headless end-to-end runs.

use egui::{Key, Pos2, Rect, Vec2};

use crate::tune;

use super::*;

/// A pane that shrinks for a while (a lane restarting) and grows back returns to where the
/// user had scrolled it.
#[test]
fn steady_scroll_survives_shrinking() {
    let ctx = egui::Context::default();
    // A frame of content `height` in a 500 px pane; `scroll_to`: the user's scroll before it.
    let frame = |height: f32, scroll_to: Option<f32>| {
        let mut offset = 0.0;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 500.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            let id = ui.make_persistent_id("probe");
            if let (Some(y), Some(area)) = (scroll_to, ui.data(|d| d.get_temp::<egui::Id>(id))) {
                let mut st = egui::scroll_area::State::load(ui.ctx(), area).unwrap();
                st.offset.y = y;
                st.store(ui.ctx(), area);
            }
            let out = steady_scroll_area(ui, "t", |ui| ui.allocate_space(Vec2::new(100.0, height)));
            ui.data_mut(|d| d.insert_temp(id, out.id));
            offset = out.state.offset.y;
        });
        out.textures_delta.clear();
        offset
    };
    frame(3000.0, None);
    frame(3000.0, Some(900.0));
    assert_eq!(frame(3000.0, None), 900.0);
    for _ in 0..3 {
        assert_eq!(frame(200.0, None), 0.0);
    }
    frame(3000.0, None);
    assert_eq!(frame(3000.0, None), 900.0);
    // Scrolling up while it is shorter is the user's choice: kept when it grows.
    assert_eq!(frame(1000.0, None), 500.0);
    frame(1000.0, Some(100.0));
    frame(3000.0, None);
    assert_eq!(frame(3000.0, None), 100.0);
}

/// New starts plain (one renderer, no scattered lights); every demo says what it shows.
/// Every non-ASCII character in the GUI's source is one egui's fonts can draw (a missing one
/// shows as a box: ⧉ and ▼ did).
#[test]
fn every_glyph_in_the_gui_is_in_the_fonts() {
    let ctx = egui::Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    // `has_glyph` calls a glyph in the last fallback font missing; compare atlas cells instead.
    let cell = |c: char| {
        let g = ctx.fonts_mut(|f| f.layout_no_wrap(c.to_string(), egui::FontId::proportional(14.0), egui::Color32::WHITE));
        g.rows[0].row.glyphs[0].uv_rect
    };
    let replacement = cell('\u{E000}');
    let mut missing = std::collections::BTreeSet::new();
    let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gui"))];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            // Comments are not drawn.
            for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
                for c in line.chars().filter(|c| !c.is_ascii()) {
                    if cell(c) == replacement {
                        missing.insert(format!("{c} (U+{:04X}) in {}", c as u32, path.display()));
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "glyphs egui's fonts lack: {missing:#?}");
}

#[test]
fn default_plan_and_demos() {
    let plan = Plan::parse(DEFAULT_PLAN, &[]).unwrap();
    assert_eq!(plan.renderers.len(), 1);
    assert!(plan.scene.lights.is_empty() && plan.scene.events.is_empty());
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans"));
    let demos: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| is_demo(p)).collect();
    assert!(demos.len() >= 5, "{demos:?}");
    for p in demos {
        assert!(lead_comment(&p).starts_with("Demo: "), "{}", p.display());
    }
}

/// Ctrl+S saves the plan, except when the canvas has the keyboard (the last click landed on
/// it): there it flies fast backwards and saves nothing.
#[test]
fn ctrl_s_saves_or_flies_backwards() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, None);
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/target/test-tmp/ctrl_s"));
    let _ = std::fs::remove_dir_all(dir);
    let path = dir.join("plan.toml");
    app.plan_path = Some(path.clone());
    let ctrl = egui::Modifiers { ctrl: true, command: true, ..Default::default() };
    let mut t = 0.0;
    let mut run = |app: &mut App, mut events: Vec<egui::Event>, modifiers: egui::Modifiers| {
        t += 1.0 / 60.0;
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1700.0, 1000.0))), time: Some(t), events, ..Default::default() };
        ctx.run_ui(input, |ui| app.draw(ui)).textures_delta.clear();
    };
    let key = |pressed| egui::Event::Key { key: Key::S, physical_key: None, pressed, repeat: false, modifiers: ctrl };
    run(&mut app, vec![], Default::default());
    app.follow_path = false;
    let start = app.cam.position;
    run(&mut app, vec![key(true), key(false)], ctrl);
    assert!(path.exists(), "Ctrl+S off the canvas saves");
    assert_eq!(app.cam.position, start, "and does not fly");
    std::fs::remove_file(&path).unwrap();

    // A click on the canvas (the middle of the window) gives it the keyboard.
    let at = Pos2::new(850.0, 500.0);
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    run(&mut app, vec![egui::Event::PointerMoved(at), button(true)], Default::default());
    run(&mut app, vec![button(false)], Default::default());
    assert!(app.canvas_focus, "the click landed on the canvas");
    run(&mut app, vec![key(true)], ctrl);
    for _ in 0..5 {
        run(&mut app, vec![], ctrl);
    }
    run(&mut app, vec![key(false)], ctrl);
    assert!(!path.exists(), "Ctrl+S on the canvas saves nothing");
    let back = -app.cam.forward().with_y(0.0).normalize().as_dvec3();
    assert!((app.cam.position - start).dot(back) > 0.0, "it flies backwards: {start} -> {}", app.cam.position);
}

fn frame(ctx: &egui::Context, app: &mut App, keys: &[Key], t: &mut f64) {
    *t += 1.0 / 60.0;
    let mut events = vec![];
    for &key in keys {
        for pressed in [true, false] {
            events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
        }
    }
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1700.0, 1000.0))),
        time: Some(*t),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| app.draw(ui));
    out.textures_delta.clear(); // no renderer in tests
}

/// Renders the GUI offscreen to `target/gui-shot.png` (needs a GPU; `--ignored`): a live
/// session with composed renderers, so the pipeline summaries of the plan editor and of the
/// Metrics tab can be looked at. `SHOT_PLAN` may name a plan file instead.
#[test]
#[ignore]
fn gui_screenshot() {
    use mr_gpu::wgpu;
    let (w, h) = (1800u32, 2300u32);
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, None);
    app.plan = match std::env::var("SHOT_PLAN") {
        Ok(path) => Plan::load(Path::new(&path), &[]).unwrap(),
        Err(_) => Plan::parse(
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
            edits = [{ node = "storage", component = "prefix" }, { node = "kernel", component = "incremental" }, { node = "shading", component = "pathtrace" }, { add = "far", role = "storage", component = "tree64" }, { node = "far", input = "changes", from = "world" }, { add = "sphere", role = "traversal", component = "sphere" }, { node = "sphere", input = "storage", from = "far" }, { node = "shading", input = "bounce", from = "sphere" }]
            [[renderer]]
            graph = "raster-gpu"
            edits = [{ node = "geometry", component = "naive" }, { node = "draw", component = "visbuffer" }]
            "#,
            &[],
        )
        .unwrap(),
    };
    app.option_text = option_texts(&app.plan);
    app.start_live(&ctx);
    app.docks.show(&ctx, dock::Pane::Metrics);
    // `SHOT_DOCKS=keys:bottom,log:window,left:collapsed`: panes moved, torn off, docks collapsed.
    if let Ok(v) = std::env::var("SHOT_DOCKS") {
        app.docks.arrange(&v).unwrap();
    }
    // `SHOT_GRAPH=lane0` / `plan1:shading`: the pipeline editor open on a renderer (and node).
    if let Ok(v) = std::env::var("SHOT_GRAPH") {
        let (of, sel) = v.split_once(':').map_or((v.as_str(), None), |(a, b)| (a, Some(b.to_string())));
        let n: usize = of.trim_start_matches(|c: char| c.is_alphabetic()).parse().unwrap_or(0);
        app.open_graph(&ctx, if of.starts_with("plan") { graph::Of::Plan(n) } else { graph::Of::Lane(n) }, sel);
    }
    // Every texture update, in order: the font atlas only arrives in the first frames.
    let mut deltas = vec![];
    let mut last = None;
    let mut t = 0.0;
    let start = std::time::Instant::now();
    let mut frames = 0;
    while frames < 30 || start.elapsed().as_secs_f32() < 4.0 {
        t += 1.0 / 60.0;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w as f32, h as f32))), time: Some(t), ..Default::default() };
        // `SHOT_DRAFT=storage` (with `SHOT_GRAPH`): that node removed in the editor, as a draft.
        if frames == 10 && let (Ok(node), Some(t)) = (std::env::var("SHOT_DRAFT"), app.trials.shown("Lane(0)")) {
            t.make_draft(&graph::model::Change::Remove(node), &[]);
        }
        let mut out = ctx.run_ui(input, |ui| app.draw(ui));
        deltas.extend(std::mem::take(&mut out.textures_delta.set));
        last = Some(out);
        frames += 1;
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let out = last.unwrap();
    let gpu = mr_gpu::Gpu::new(mr_gpu::GpuRequest { experimental: false, adapter: None, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::empty(), limits: &|a| a.clone() }).unwrap();
    let fmt = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut r = eframe::egui_wgpu::Renderer::new(&gpu.device, fmt, Default::default());
    for (id, ds) in &deltas {
        for d in ds {
            r.update_texture(&gpu.device, &gpu.queue, *id, d);
        }
    }
    let jobs = ctx.tessellate(out.shapes, out.pixels_per_point);
    let sd = eframe::egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: out.pixels_per_point };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("gui shot"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fmt,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    let extra = r.update_buffers(&gpu.device, &gpu.queue, &mut enc, &jobs, &sd);
    {
        let mut pass = enc
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        r.render(&mut pass, &jobs, &sd);
    }
    let row = (w * 4).next_multiple_of(256);
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: row as u64 * h as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: Default::default(), aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    gpu.queue.submit(extra.into_iter().chain([enc.finish()]));
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().unwrap();
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        let src = &data[(y * row) as usize..(y * row + w * 4) as usize];
        img.as_mut()[(y * w * 4) as usize..((y + 1) * w * 4) as usize].copy_from_slice(src);
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/gui-shot.png");
    img.save(path).unwrap();
    println!("saved {path} after {frames} frames");
}

/// Draws the real GUI headlessly through a live session (needs a GPU; `--ignored`).
#[test]
#[ignore]
fn gui_headless_smoke() {
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, None);
    app.plan = Plan::parse(
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
        "#,
        &[],
    )
    .unwrap();
    app.option_text = option_texts(&app.plan);
    let mut t = 0.0;
    for pane in [dock::Pane::Metrics, dock::Pane::Compare, dock::Pane::Benchmark, dock::Pane::Keys] {
        app.docks.show(&ctx, pane);
        frame(&ctx, &mut app, &[], &mut t);
    }
    // Panes torn off into windows of their own, moved between docks, and docked again.
    app.docks.tear(dock::Pane::Metrics);
    app.docks.tear(dock::Pane::Log);
    app.docks.move_to(dock::Pane::Keys, dock::Side::Bottom);
    frame(&ctx, &mut app, &[], &mut t);
    app.docks = dock::Docks::default();
    frame(&ctx, &mut app, &[Key::F5], &mut t);
    assert!(app.live.is_some());
    let start = std::time::Instant::now();
    while app.textures.iter().flatten().count() < 2 {
        if start.elapsed().as_secs() >= 60 {
            let msg = {
                let sh = app.live.as_ref().unwrap().shared.lock().unwrap();
                let lanes: Vec<_> = sh.lanes.iter().map(|l| (l.spec.label().to_string(), l.status.clone(), l.frames, l.frame.is_some())).collect();
                format!("no frames displayed; status {}; lanes {lanes:?}; log:\n{}", sh.status, sh.log.join("\n"))
            };
            panic!("{msg}");
        }
        frame(&ctx, &mut app, &[], &mut t);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.docks.show(&ctx, dock::Pane::Metrics);
    for keys in [&[Key::L][..], &[Key::F], &[Key::W], &[Key::E], &[Key::Num1], &[Key::Escape], &[Key::C]] {
        for _ in 0..5 {
            frame(&ctx, &mut app, keys, &mut t);
        }
    }
    assert!(!app.follow_path, "W should switch to free fly");
    assert_eq!(app.mode, PassMode::LIT);
    let start = std::time::Instant::now();
    while app.live.as_ref().unwrap().shared.lock().unwrap().compare.is_none() {
        assert!(start.elapsed().as_secs() < 60, "compare never finished");
        frame(&ctx, &mut app, &[], &mut t);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    frame(&ctx, &mut app, &[], &mut t);
    assert!(app.compare_tex.is_some(), "compare tab should have uploaded images");

    // Tuner + periodic probes: the Metrics tab must draw knobs, tuner and quality plots.
    app.docks.show(&ctx, dock::Pane::Metrics);
    app.probe_interval = 0.3;
    app.send(Cmd::Tune { lane: 0, config: Some(tune::TunerConfig { goal: tune::Goal::MaxFps, settle_s: 0.1, window_s: 0.3, ..Default::default() }) });
    app.send(Cmd::SetParam { lane: 1, name: "depth_prepass".into(), value: mr_core::ParamValue::Bool(false) });
    let start = std::time::Instant::now();
    loop {
        frame(&ctx, &mut app, &[], &mut t);
        let done = {
            let sh = app.live.as_ref().unwrap().shared.lock().unwrap();
            sh.lanes[0].tuner.as_ref().is_some_and(|t| t.trials >= 1) && sh.lanes.iter().all(|l| l.quality.len() >= 2)
        };
        if done {
            break;
        }
        assert!(start.elapsed().as_secs() < 60, "tuner/probes did not progress");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.probe_interval = 0.0;
    frame(&ctx, &mut app, &[], &mut t);
    app.docks.show(&ctx, dock::Pane::Toml);
    frame(&ctx, &mut app, &[], &mut t);

    // Fullscreen + 1:1 + match resolution: the viewport becomes the whole screen and
    // the render resolution follows it exactly.
    frame(&ctx, &mut app, &[Key::F11], &mut t);
    assert!(app.fullscreen);
    frame(&ctx, &mut app, &[Key::Num1], &mut t);
    frame(&ctx, &mut app, &[], &mut t);
    assert_eq!(app.tile_px, Some([1700, 1000]));
    frame(&ctx, &mut app, &[Key::M], &mut t);
    assert_eq!(app.plan.run.resolution, [1700, 1000]);
    assert!(app.pixel_exact);
    // Lane 1: the tuner on lane 0 may have lowered its render scale.
    let start = std::time::Instant::now();
    while app.textures.get(1).and_then(|t| t.as_ref()).is_none_or(|t| t.tex.size() != [1700, 1000]) {
        let size = app.textures.get(1).and_then(|t| t.as_ref()).map(|t| t.tex.size());
        assert!(start.elapsed().as_secs() < 60, "lane 1 has no 1700×1000 frame after the resolution change: {size:?}");
        frame(&ctx, &mut app, &[], &mut t);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    frame(&ctx, &mut app, &[Key::O, Key::Z], &mut t);
    frame(&ctx, &mut app, &[Key::Escape], &mut t);
    assert!(!app.fullscreen);
    frame(&ctx, &mut app, &[], &mut t);
    app.live = None;
}

/// The GUI's own GPU device lost (destroyed by fault injection) while a live session runs, both
/// ways it can be found: by the GUI's next frame (`device-lost`) and by egui-wgpu panicking
/// while it draws (`device-lost-unseen`, here with unsaved changes). Either way the GUI stops
/// the session, hands over to a fresh GUI on the plan (the unsaved one recovered) and exits
/// normally. Runs the real GUI in a headless compositor of its own (`kwin_wayland --virtual`,
/// through its Xwayland: on its Wayland socket the GUI draws its first frame and then gets no
/// frame callbacks), so nothing shows on the desktop; needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gui_device_lost_hands_over() {
    use std::io::Read;
    use std::time::{Duration, Instant};
    let host = super::live::tests::built_host();
    let slow = std::env::var("MR_TEST_SLOW").ok().and_then(|v| v.parse::<f64>().ok()).filter(|f| *f >= 1.0).unwrap_or(1.0);
    let dir = std::env::temp_dir().join(format!("mr-gui-device-lost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).expect("XDG_RUNTIME_DIR");
    let socket = format!("mr-test-{}", std::process::id());
    let display_file = dir.join("display");
    let mut kwin = std::process::Command::new("kwin_wayland")
        .args(["--virtual", "--xwayland", "--socket", &socket, "--width", "1280", "--height", "800"])
        .arg(format!("sh -c 'echo $DISPLAY > {}'", display_file.display()))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("this test needs kwin_wayland (its --virtual backend is the headless compositor)");
    let deadline = Instant::now() + Duration::from_secs_f64(20.0 * slow);
    let display = loop {
        if let Some(d) = std::fs::read_to_string(&display_file).ok().map(|d| d.trim().to_string()).filter(|d| !d.is_empty()) {
            break d;
        }
        if Instant::now() > deadline {
            let _ = kwin.kill();
            panic!("kwin_wayland --virtual --xwayland started no X display");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let plan = dir.join("plan.toml");
    let text = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"line\", from = [0, 20, 0], to = [40, 20, 0] }\n\
        [run]\nresolution = [64, 36]\nview_radius = 1\n[[renderer]]\ngraph = \"raycast-cpu\"\n";
    std::fs::write(&plan, text).unwrap();
    let (lost, quit) = (3.0 * slow, 2.0 * slow);
    let runs = [
        (format!("live:0,device-lost:{lost},quit:{quit}"), "the GUI's GPU device was destroyed: stopping the live session"),
        (format!("live:0,dirty:0,device-lost-unseen:{lost},quit:{quit}"), "the GUI's GPU device was destroyed and drawing on it failed"),
    ];
    let mut failures = vec![];
    for (faults, how) in &runs {
        let mut gui = std::process::Command::new(&host)
            .arg("gui")
            .arg(&plan)
            .env("DISPLAY", &display)
            .env_remove("WAYLAND_DISPLAY")
            .env("XDG_CONFIG_HOME", dir.join("config"))
            .env("MR_FAULT_INJECTION", "1")
            .env("MR_GUI_FAULT", faults)
            .env("RUST_LOG", "warn")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // The pipe ends when both GUIs have closed it (the fresh one inherits it).
        let mut err = gui.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            let _ = tx.send(s);
        });
        let Ok(log) = rx.recv_timeout(Duration::from_secs_f64(60.0 * slow)) else {
            failures.push(format!("{faults}: the GUIs did not close within the deadline"));
            break;
        };
        let status = gui.wait().unwrap();
        let unsaved = faults.contains("dirty");
        let mut want = vec![
            "fault injection: starting the live session",
            "fault injection: destroying the GUI's GPU device",
            how,
            "GUI restarted: the previous GUI's GPU device was destroyed",
            "fault injection: closing the GUI",
        ];
        if unsaved {
            want.extend(["unsaved plan written to", "unsaved changes of"]);
        }
        let missing: Vec<_> = want.iter().filter(|w| !log.contains(**w)).collect();
        // The unseen loss is egui-wgpu's panic, by design; anything else panicking is not.
        let panicked = log.contains("panicked") && !(faults.contains("unseen") && log.matches("panicked").count() == 1 && log.contains("egui-wgpu"));
        if !status.success() || !missing.is_empty() || panicked {
            failures.push(format!("{faults}: status {status}, missing {missing:?}, panicked {panicked}\n{log}"));
        }
    }
    // A terminated compositor removes its X display's socket and lock (killed, it leaves them).
    let _ = std::process::Command::new("kill").arg(kwin.id().to_string()).status();
    let _ = kwin.wait();
    for f in [socket.clone(), format!("{socket}.lock")] {
        let _ = std::fs::remove_file(runtime.join(f));
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The benchmark table shows a column only where a pass that ran reports it: a plain pass
/// fills the fixed ones; the like score, convergence, world updates, missed deadlines and
/// streaming (wire bytes, holes, first complete) appear with a pass that has them, never for
/// a failed pass's leftovers; the table draws headless.
#[test]
fn report_columns_follow_what_passes_report() {
    use serde_json::json;
    let names = |passes: &[serde_json::Value]| bench::columns(passes).iter().map(|c| c.name).collect::<Vec<_>>();
    let fixed = ["fps", "frame ms", "p99 ms", "gpu ms", "cpu ms", "latency ms", "PSNR", "settled dB", "bad %", "init ms", "shaders ms", "RSS MB", "GPU MB", "device MB"];
    // Complete at its first frame, no link, no updates: nothing optional.
    let plain = json!({ "label": "a", "mode": "unlit", "fps": 60.0, "update_ms": { "mean": 0.0 }, "missed_deadlines": null, "first_complete": { "frame": 0, "ms": 12.0, "clock_s": 0.0 }, "per_frame": [{ "frame": 0 }], "wire_bytes": 1000 });
    assert_eq!(names(std::slice::from_ref(&plain)), fixed);
    let rich = json!({
        "label": "b", "mode": "lit", "fps": 30.0,
        "quality_like": { "psnr": { "mean": 41.0 } }, "like": "lod 2",
        "converged_spp": 64, "update_ms": { "mean": 0.5 }, "update_ns_per_byte": 2.0, "missed_deadlines": 3,
        "link": { "bandwidth_mbps": 10.0 }, "wire_bytes": 3145728, "arrived_bytes": 2097152, "chunks_sent": 40,
        "holes": { "mean": 1.5, "max": 7.0 }, "frames_with_holes": 4,
        "first_complete": { "frame": 9, "ms": 900.0, "clock_s": 0.15 }, "per_frame": [{ "frame": 0 }],
    });
    let both = [plain.clone(), rich.clone()];
    let extra = ["like dB", "spp", "update ms", "missed", "wire MB", "holes", "first complete"];
    assert_eq!(names(&both), fixed.iter().chain(&extra).copied().collect::<Vec<_>>());
    let cols = bench::columns(&both);
    let cell = |name: &str, p: &serde_json::Value| cols.iter().find(|c| c.name == name).and_then(|c| (c.cell)(p)).map(|c| (c.text, c.hover));
    assert_eq!(cell("like dB", &rich), Some(("41.0".into(), Some("like lod 2".into()))));
    assert_eq!(cell("wire MB", &rich).unwrap().0, "3.0");
    assert_eq!(cell("holes", &rich).unwrap(), ("1.5 / 7".into(), Some("4 frames with holes".into())));
    assert_eq!(cell("first complete", &rich).unwrap().0, "#9 900 ms");
    assert_eq!(cell("holes", &plain), None, "a pass with nothing streamed late shows -");
    // Never complete: the streaming columns show, saying so.
    let never = json!({ "label": "c", "mode": "unlit", "first_complete": null, "per_frame": [{ "frame": 0 }] });
    assert_eq!(bench::columns(std::slice::from_ref(&never)).iter().find(|c| c.name == "first complete").and_then(|c| (c.cell)(&never)).map(|c| c.text), Some("never".into()));
    // A failed pass's figures add nothing.
    let mut failed = rich.clone();
    failed["error"] = json!("boom");
    assert_eq!(names(&[plain.clone(), failed.clone()]), fixed);
    // Drawn headless.
    let ctx = egui::Context::default();
    let report = bench::Report { path: "runs/x/report.json".into(), passes: vec![plain, rich, never, failed] };
    ctx.run_ui(Default::default(), |ui| bench::report_ui(ui, &report)).textures_delta.clear();
}
