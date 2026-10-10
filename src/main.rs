//! `multirenderer`: run, benchmark and compare voxel renderers, each sandboxed in its own
//! worker process.

mod bayes;
mod compare;
mod gui;
mod plan;
mod report;
mod session;
mod stream;
mod tune;
mod window;
mod worker;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use mr_core::{BlockRegistry, PassMode};
use mr_world::{Scene, World, textures::default_registry};
use plan::{Plan, RendererSpec, Streaming};
use report::PassReport;
use session::{InteractiveDriver, Lane, Outputs, ScriptedDriver, run_pass, sanitize};
use stream::Streamer;
use window::{UiEvent, UiHandle};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Without a subcommand the GUI opens.
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open the interactive GUI (the default), optionally with a plan loaded.
    Gui { plan: Option<PathBuf> },
    /// Run a plan: benchmark renderers, compare them against a reference, write a report.
    Run {
        plan: PathBuf,
        /// Override plan values, e.g. `-s run.frames=100 -s output.window=true`.
        #[arg(short = 's', long = "set")]
        overrides: Vec<String>,
    },
    /// Fly around a plan's scene interactively with one or more renderers side by side.
    View {
        plan: PathBuf,
        /// Renderers to show (default: the plan's).
        #[arg(short, long = "renderer")]
        renderers: Vec<String>,
        #[arg(short = 's', long = "set")]
        overrides: Vec<String>,
    },
    /// List available renderers.
    List,
    /// Compare two PNG images.
    Diff {
        a: PathBuf,
        b: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Internal: serve a renderer over a socket (spawned by the host).
    #[command(hide = true)]
    Worker {
        #[arg(long)]
        socket: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cmd = cli.cmd.unwrap_or(Cmd::Gui { plan: None });
    let worker = matches!(cmd, Cmd::Worker { .. });
    let filter = tracing_subscriber::EnvFilter::try_from_env("MR_LOG")
        .unwrap_or_else(|_| "info,wgpu_core=warn,wgpu_hal=error,naga=warn".into());
    let sub = tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(filter);
    if worker {
        sub.with_ansi(false).without_time().init();
    } else {
        sub.init();
    }

    match cmd {
        Cmd::Gui { plan } => gui::run(plan),
        Cmd::Worker { socket } => mr_worker::run(&socket),
        Cmd::List => {
            println!("Renderers are assembled from components (plan: compose = {{ backend, storage, geometry, prepass, visibility, shading, meshes }};\nCLI: -r backend:<its required slots in order>[+shading[+prepass[+meshes]]], e.g. gpu-compute:prefix+incremental, gpu-compute:bricks+refloor+direct+beam).\n\nBack ends:");
            for b in mr_components::BACKENDS {
                let slots: Vec<String> = b.slots.iter().map(|(s, d)| format!("{}={d}", s.name())).collect();
                println!("  {:<12} {}{}\n               slots: {}", b.name, b.description, if b.renderer.is_none() { "  [not available yet]" } else { "" }, slots.join(", "));
            }
            for slot in mr_components::Slot::ALL {
                println!("\n{}:", slot.name());
                for c in mr_components::components(slot) {
                    println!("  {:<12} [{}] {}", c.name, c.backends.join(","), c.description);
                }
            }
            println!("\nStandalone renderers (-r name; cpu-reference defines the reference images):");
            for i in mr_worker::infos().into_iter().filter(|i| !mr_components::BACKENDS.iter().any(|b| b.renderer == Some(i.name.as_str()))) {
                let modes: Vec<_> = i.modes.iter().map(|m| m.name()).collect();
                println!("  {:<16} [{}] modes={} max_lod={}\n      {}", i.name, i.tags.join(","), modes.join(","), i.max_lod, i.description);
            }
            Ok(())
        }
        Cmd::Diff { a, b, out } => {
            let ia = image::open(&a)?.to_rgba8();
            let ib = image::open(&b)?.to_rgba8();
            if ia.dimensions() != ib.dimensions() {
                bail!("size mismatch: {:?} vs {:?}", ia.dimensions(), ib.dimensions());
            }
            let q = compare::compare(&ia, &ib, ia.width());
            println!("PSNR {:.2} dB, SSIM {:.4} ({:.1} dB), MAE {:.5}, bad pixels {:.3}%", q.psnr, q.ssim, q.ssim_db, q.mae, q.bad_pct);
            if let Some(o) = out {
                let (w, h) = ia.dimensions();
                image::save_buffer(o, &compare::diff_image(&ia, &ib), w, h, image::ExtendedColorType::Rgba8)?;
            }
            Ok(())
        }
        Cmd::Run { plan: path, overrides } => {
            let plan = Plan::load(&path, &overrides)?;
            let run_dir = make_run_dir(&plan)?;
            let text = std::fs::read_to_string(&path)?;
            std::fs::write(run_dir.join("plan.toml"), format!("# overrides: {overrides:?}\n{text}"))?;
            tracing::info!("run directory {}", run_dir.display());
            let reports = if plan.output.window {
                let (p, d) = (plan.clone(), run_dir.clone());
                window::run_with_window(&format!("multirenderer — {}", plan.name), move |ui| run_plan(&p, &d, Some(ui)))?
            } else {
                run_plan(&plan, &run_dir, None)?
            };
            let mut table = report::table(&reports);
            if let Some(t) = report::repeat_table(&reports) {
                table = format!("{table}\n{t}");
            }
            println!("\n{table}");
            std::fs::write(run_dir.join("summary.txt"), &table)?;
            std::fs::write(run_dir.join("report.json"), serde_json::to_string_pretty(&reports)?)?;
            println!("report: {}", run_dir.join("report.json").display());
            Ok(())
        }
        Cmd::View { plan: path, renderers, overrides } => {
            let mut plan = Plan::load(&path, &overrides)?;
            if !renderers.is_empty() {
                plan.renderers = renderers.iter().map(|r| plan::RendererSpec::from_cli(r)).collect::<anyhow::Result<_>>()?;
            }
            if plan.run.streaming == Streaming::Complete {
                // Keep the view responsive when flying fast.
                plan.run.streaming = Streaming::Budget { budget: 512 };
            }
            window::run_with_window("multirenderer", move |ui| view(&plan, ui))
        }
    }
}

fn make_run_dir(plan: &Plan) -> anyhow::Result<PathBuf> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil-from-days (Howard Hinnant), UTC.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    let stamp = format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60);
    let base = PathBuf::from(plan.output.dir.clone().unwrap_or_else(|| "runs".into()));
    let dir = base.join(format!("{stamp}-{}", sanitize(&plan.name)));
    std::fs::create_dir_all(dir.join("logs"))?;
    Ok(dir)
}

struct Ctx<'a> {
    plan: &'a Plan,
    blocks: Arc<BlockRegistry>,
    run_dir: Option<&'a Path>,
    ui: Option<UiHandle>,
}

impl Ctx<'_> {
    fn spawn_lanes(&self, specs: &[RendererSpec], mode: Option<PassMode>, env: mr_core::Environment) -> (Vec<Lane>, Vec<PassReport>) {
        let run = &self.plan.run;
        let [w, h] = run.resolution;
        let budget = match run.streaming {
            Streaming::Complete => None,
            Streaming::Budget { budget } => Some(budget),
        };
        let (mut lanes, mut failed) = (vec![], vec![]);
        for spec in specs {
            let label = spec.label().to_string();
            let mut report = PassReport { label: label.clone(), renderer: spec.name.clone(), mode, resolution: [w, h], ..Default::default() };
            let mode_tag = mode.map(|m| m.name()).unwrap_or("view");
            let log = self.run_dir.map(|d| d.join("logs").join(format!("{}-{mode_tag}.log", sanitize(&label))));
            let spawn = || worker::spawn_renderer(run, spec, self.blocks.clone(), env, log, run.resolution, run.in_flight + 2);
            match spawn() {
                Ok(worker) => {
                    report.init_ms = worker.init_ms;
                    report.tags = worker.info.tags.clone();
                    let max_lod = spec.max_lod(run, worker.info.max_lod);
                    let i = lanes.len();
                    lanes.push(Lane::new(worker, Streamer::new(spec.view_radius(run), max_lod, budget), report, i, w, h));
                }
                Err(e) => {
                    tracing::error!("[{label}] failed to start: {e:#}");
                    report.error = Some(format!("{e:#}"));
                    failed.push(report);
                }
            }
        }
        if let Some(ui) = &self.ui {
            ui.send(UiEvent::Lanes(lanes.len()));
            ui.send(UiEvent::Title(format!("multirenderer — {}", specs.iter().map(|s| s.label()).collect::<Vec<_>>().join(" | "))));
        }
        (lanes, failed)
    }

    /// One pass: the given renderers run together through the scripted timeline.
    fn pass(&self, specs: &[RendererSpec], mode: PassMode, base: &World, out: &mut Outputs, only_every: Option<u32>, warmup: u32) -> anyhow::Result<Vec<PassReport>> {
        let run = &self.plan.run;
        let mut world = base.clone();
        let scene = Scene::new(self.plan.scene.clone(), self.blocks.clone(), &mut world, run.frames)?;
        let lights = scene.initial_lights().to_vec();
        let mesh_data = if mode.meshes { scene.mesh_data() } else { vec![] };
        let (mut lanes, mut reports) = self.spawn_lanes(specs, Some(mode), scene.environment);
        if lanes.is_empty() {
            return Ok(reports);
        }
        let names: Vec<_> = lanes.iter().map(|l| l.report.label.clone()).collect();
        tracing::info!("pass: {} [{}]", names.join(", "), mode.name());
        let mut driver = ScriptedDriver::new(scene, mode, warmup, run.frames, only_every);
        let result = run_pass(&mut lanes, &mut driver, &mut world, &lights, &mesh_data, run, out);
        if let Err(e) = &result {
            tracing::error!("pass failed: {e:#}");
        }
        for lane in lanes {
            let mut r = lane.report;
            if let Err(e) = &result {
                r.error = Some(format!("{e:#}"));
            }
            lane.worker.shutdown();
            reports.push(r);
        }
        println!("{}", report::table(&reports).lines().skip(1).collect::<Vec<_>>().join("\n"));
        Ok(reports)
    }
}

fn run_plan(plan: &Plan, run_dir: &Path, ui: Option<UiHandle>) -> anyhow::Result<Vec<PassReport>> {
    let infos = mr_worker::infos();
    for r in &plan.renderers {
        if !infos.iter().any(|i| i.name == r.name) {
            bail!("unknown renderer `{}` (see `multirenderer list`)", r.name);
        }
    }
    let ctx = Ctx { plan, blocks: Arc::new(default_registry()), run_dir: Some(run_dir), ui: ui.clone() };
    let run = &plan.run;

    // Pre-generate every chunk the camera path will need, so world generation never
    // counts against a renderer.
    let mut base = World::new(plan.scene.generator.build());
    {
        let t = Instant::now();
        let scene = Scene::new(plan.scene.clone(), ctx.blocks.clone(), &mut base, run.frames)?;
        // One streamer per distinct (radius, LOD) configuration among the renderers.
        let mut configs: Vec<(u32, u8)> = plan
            .renderers
            .iter()
            .map(|r| (r.view_radius(run), r.max_lod(run, infos.iter().find(|i| i.name == r.name).map(|i| i.max_lod).unwrap_or(0))))
            .chain(std::iter::once((run.view_radius, 0)))
            .collect();
        configs.sort();
        configs.dedup();
        let mut sink = vec![];
        for (radius, max_lod) in configs {
            let mut s = Streamer::new(radius, max_lod, None);
            for f in 0..run.frames {
                s.update(&mut base, scene.camera(f).position, &mut sink);
                sink.clear();
            }
        }
        tracing::info!("pre-generated {} chunks in {:.1}s", base.cached_chunks(), t.elapsed().as_secs_f32());
    }

    let save = plan.output.save.clone().map(|s| (s, run_dir.join("frames")));
    let mut out = Outputs::new(ui, plan.output.readback, save, plan.output.compare.as_ref().map(|c| c.every), run.frames);
    let mut reports = vec![];
    for &mode in &run.modes {
        if let Some(c) = &plan.output.compare {
            out.recording_reference = true;
            out.references.retain(|k, _| k.0 != mode);
            let spec = RendererSpec { label: Some(format!("{} (ref)", c.reference)), options: c.reference_options.clone(), ..RendererSpec::new(c.reference.clone()) };
            reports.extend(ctx.pass(&[spec], mode, &base, &mut out, Some(c.every), 0)?);
            out.recording_reference = false;
        }
        let specs: Vec<RendererSpec> = plan
            .renderers
            .iter()
            .filter(|s| {
                let ok = infos.iter().any(|i| i.name == s.name && s.supports(i, mode));
                if !ok {
                    tracing::warn!("{} does not support {} mode; skipped", s.label(), mode.name());
                }
                ok
            })
            .cloned()
            .collect();
        // Repeats go round-robin (A B C A B C ...) so slow drifts in clocks and
        // temperature spread over all renderers instead of favouring whichever ran first.
        for repeat in 0..run.repeats.max(1) {
            let mut batch = vec![];
            if run.parallel {
                batch.extend(ctx.pass(&specs, mode, &base, &mut out, None, run.warmup)?);
            } else {
                for s in &specs {
                    batch.extend(ctx.pass(std::slice::from_ref(s), mode, &base, &mut out, None, run.warmup)?);
                    if out.ui.as_ref().is_some_and(|u| u.closed()) {
                        reports.extend(batch);
                        return Ok(reports);
                    }
                }
            }
            for r in &mut batch {
                r.repeat = repeat;
            }
            reports.extend(batch);
            // References are only compared once (first repetition); later ones would only
            // repeat identical quality numbers.
            if repeat == 0 {
                out.compare_every = None;
            }
        }
        out.compare_every = plan.output.compare.as_ref().map(|c| c.every);
    }
    Ok(reports)
}

fn view(plan: &Plan, ui: UiHandle) -> anyhow::Result<()> {
    let ctx = Ctx { plan, blocks: Arc::new(default_registry()), run_dir: None, ui: Some(ui.clone()) };
    let mut world = World::new(plan.scene.generator.build());
    let scene = Scene::new(plan.scene.clone(), ctx.blocks.clone(), &mut world, plan.run.frames)?;
    let lights = scene.initial_lights().to_vec();
    let mesh_data = scene.mesh_data();
    let (mut lanes, failed) = ctx.spawn_lanes(&plan.renderers, None, scene.environment);
    for f in failed {
        tracing::error!("{}: {}", f.label, f.error.unwrap_or_default());
    }
    if lanes.is_empty() {
        bail!("no renderer started");
    }
    let mode = plan.run.modes.first().copied().unwrap_or(PassMode::UNLIT);
    let mut driver = InteractiveDriver::new(ui.clone(), scene, mode);
    let mut out = Outputs::new(Some(ui), plan::Readback::Auto, None, None, u32::MAX);
    run_pass(&mut lanes, &mut driver, &mut world, &lights, &mesh_data, &plan.run, &mut out).context("view session")?;
    for l in lanes {
        l.worker.shutdown();
    }
    Ok(())
}
