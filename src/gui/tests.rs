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
            compose = { backend = "gpu-compute", storage = "prefix", traversal = "incremental", shading = "pathtrace" }
            sites = { bounce = "tree64/sphere" }
            [[renderer]]
            compose = { backend = "gpu-raster", geometry = "naive", draw = "visbuffer" }
            "#,
            &[],
        )
        .unwrap(),
    };
    app.option_text = option_texts(&app.plan);
    app.start_live(&ctx);
    app.right_tab = RightTab::Metrics;
    // `SHOT_SCHEMES=storage/bits.md` (or empty): the Schemes tab, on that entry.
    if let Ok(v) = std::env::var("SHOT_SCHEMES") {
        app.right_tab = RightTab::Schemes;
        app.schemes.open = (!v.is_empty()).then_some(v);
    }
    // `SHOT_GRAPH=lane0` / `plan1:shading`: the pipeline editor open on a renderer (and node).
    if let Ok(v) = std::env::var("SHOT_GRAPH") {
        let (of, sel) = v.split_once(':').map_or((v.as_str(), None), |(a, b)| (a, Some(b.to_string())));
        let n: usize = of.trim_start_matches(|c: char| c.is_alphabetic()).parse().unwrap_or(0);
        app.graph_win = Some(graph::Window::new(&ctx, if of.starts_with("plan") { graph::Of::Plan(n) } else { graph::Of::Lane(n) }, sel));
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
        compose = { backend = "gpu-compute" }
        [[renderer]]
        compose = { backend = "gpu-raster" }
        "#,
        &[],
    )
    .unwrap();
    app.option_text = option_texts(&app.plan);
    let mut t = 0.0;
    for tab in [RightTab::Metrics, RightTab::Compare, RightTab::Benchmark, RightTab::Help] {
        app.right_tab = tab;
        frame(&ctx, &mut app, &[], &mut t);
    }
    frame(&ctx, &mut app, &[Key::F5], &mut t);
    assert!(app.live.is_some());
    let start = std::time::Instant::now();
    while app.textures.iter().flatten().count() < 2 {
        if start.elapsed().as_secs() >= 60 {
            let msg = {
                let sh = app.live.as_ref().unwrap().shared.lock().unwrap();
                let lanes: Vec<_> = sh.lanes.iter().map(|l| (l.spec.name.clone(), l.status.clone(), l.frames, l.frame.is_some())).collect();
                format!("no frames displayed; status {}; lanes {lanes:?}; log:\n{}", sh.status, sh.log.join("\n"))
            };
            panic!("{msg}");
        }
        frame(&ctx, &mut app, &[], &mut t);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.right_tab = RightTab::Metrics;
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
    app.right_tab = RightTab::Metrics;
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
    app.left_tab = LeftTab::Toml;
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
