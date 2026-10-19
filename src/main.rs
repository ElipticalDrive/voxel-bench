//! `multirenderer`: run, benchmark and compare voxel renderers, each sandboxed in its own
//! worker process.

mod bayes;
mod compare;
mod features;
mod catalogue;
mod gui;
mod observe;
mod pipelines;
mod plan;
mod report;
mod session;
pub(crate) use mr_world::stream;
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
        /// Remove plan values, e.g. `-u output.save` (no saved frames).
        #[arg(short = 'u', long = "unset")]
        unset: Vec<String>,
    },
    /// What each renderer is good at: reads the observations of any number of runs (run
    /// directories, or directories containing them) and writes a catalogue (Markdown and
    /// CSV) ranking the renderers per scenario and mode.
    Catalogue {
        paths: Vec<PathBuf>,
        /// Where `catalogue.md` and `catalogue.csv` go (default: the first path, if a directory).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Frames below this mean PSNR against the reference count as incorrect: such a
        /// renderer is ranked, but cannot be the best or "good at" a scenario.
        #[arg(long, default_value_t = 35.0)]
        min_psnr: f64,
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
    /// A pipeline as a graph: its nodes (TOML, for a plan's
    /// `[renderer.graph]`), sites form, canonical form and cache key. The renderer as for
    /// `view -r` (`gpu-compute:prefix+incremental,shadow=tree64/sphere`).
    /// With `--keys`: reads observations (JSON lines) on stdin and prints each one's graph
    /// key (`-` if it has none), for records written before they carried one.
    Graph {
        renderer: Option<String>,
        #[arg(long)]
        keys: bool,
    },
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
        socket: String,
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
        Cmd::Graph { keys: true, .. } => {
            for line in std::io::stdin().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let key = serde_json::from_str::<observe::Observation>(&line).ok().and_then(|o| o.pipeline()).and_then(|g| g.key().ok());
                println!("{}", key.as_deref().unwrap_or("-"));
            }
            Ok(())
        }
        Cmd::Graph { renderer, .. } => {
            let renderer = renderer.context("which renderer? e.g. `graph gpu-compute:prefix+incremental`")?;
            let mut spec = plan::RendererSpec::from_cli(&renderer)?;
            spec.resolve()?;
            let g = spec.pipeline().with_context(|| format!("`{renderer}` is not a composition"))?;
            println!("# {}\n{}", spec.label(), toml::to_string(&g)?);
            if let Some((c, sites)) = g.sites_form().map_err(anyhow::Error::msg)? {
                let sites: Vec<String> = sites.iter().map(|(k, v)| format!(",{k}={v}")).collect();
                println!("# sites form: compose = {} {}", serde_json::to_string(&c)?, sites.concat());
            }
            println!("# canonical: {}\n# key: {}", g.canonical().map_err(anyhow::Error::msg)?, g.key().map_err(anyhow::Error::msg)?);
            let schedule = g.check().and_then(|c| c.schedule()).map_err(anyhow::Error::msg)?;
            println!("# schedule (passes in frame order, [fused nodes compiled in]):");
            print!("{}", schedule.to_string().lines().map(|l| format!("#{l}\n")).collect::<String>());
            Ok(())
        }
        Cmd::List => {
            println!("Renderers are assembled from components (plan: compose = {{ backend, storage, geometry, prepass, visibility, shading, meshes }};\nCLI: -r backend:<its required slots in order>[+shading[+prepass[+meshes]]], e.g. gpu-compute:prefix+incremental, gpu-compute:bricks+refloor+direct+beam).\nSites bound otherwise (shadow, bounce, exposure, meshes-shadow; plan: sites = {{ ... }}; CLI: after commas):\n  gpu-compute:bricks+refloor+pathtrace,bounce=tree64/sphere — a site's value is another site (shared), <kernel> (over the lead's storage),\n  <storage>/<kernel> or <site>/<kernel>; `multirenderer graph <renderer>` shows the graph.\nThe world's side: link = free | net(latency_ms=30,jitter_ms=10,bandwidth_mbps=200,loss=0.01,reorder=true,seed=1),\n  client = pick | derive, with its region as knobs: client=derive(radius=4,max_lod=2) (unset: run.view_radius, run.max_lod).\n\nBack ends:");
            for b in mr_components::BACKENDS {
                let slots: Vec<String> = b.slots.iter().map(|(s, d)| format!("{}={d}", s.name())).collect();
                let mut at: Vec<String> = vec![format!("{} (default)", b.runs.label())];
                at.extend(b.at.iter().filter(|(s, _)| b.has(*s)).map(|(s, p)| format!("{} {}", s.name(), p.label())));
                println!("  {:<12} {}{}\n               slots: {}\n               runs: {}", b.name, b.description, if b.renderer.is_none() { "  [not available yet]" } else { "" }, slots.join(", "), at.join(", "));
            }
            for slot in mr_components::Slot::ALL {
                println!("\n{}:", slot.name());
                for c in mr_components::components(slot) {
                    let on: Vec<&str> = mr_components::BACKENDS.iter().filter(|b| mr_components::unavailable(b.name, slot, c.name).is_none()).map(|b| b.name).collect();
                    println!("  {:<12} [{}] (on {}) {}", c.name, c.impls_label(), on.join(","), c.description);
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
        Cmd::Catalogue { paths, out, min_psnr } => {
            let files = observe::find(&paths);
            if files.is_empty() {
                bail!("no observations.jsonl under {paths:?}");
            }
            let obs = observe::read(&files)?;
            let out = out.or_else(|| paths.first().filter(|p| p.is_dir()).cloned()).unwrap_or_else(|| PathBuf::from("."));
            let cat = catalogue::Catalogue::build(&obs, min_psnr);
            std::fs::create_dir_all(&out)?;
            std::fs::write(out.join("catalogue.md"), cat.markdown())?;
            std::fs::write(out.join("catalogue.csv"), cat.csv())?;
            println!("{}", cat.summary());
            println!("{} observations from {} files → {}", obs.len(), files.len(), out.join("catalogue.md").display());
            Ok(())
        }
        Cmd::Run { plan: path, overrides, unset } => {
            let plan = Plan::load_with(&path, &overrides, &unset)?;
            let run_dir = make_run_dir(&plan)?;
            let text = std::fs::read_to_string(&path)?;
            std::fs::write(run_dir.join("plan.toml"), format!("# overrides: {overrides:?}, unset: {unset:?}\n{text}"))?;
            tracing::info!("run directory {}", run_dir.display());
            let (reports, scene_features) = if plan.output.window {
                let (p, d) = (plan.clone(), run_dir.clone());
                window::run_with_window(&format!("multirenderer — {}", plan.name), move |ui| run_plan(&p, &d, Some(ui)))?
            } else {
                run_plan(&plan, &run_dir, None)?
            };
            let mut table = report::table(&reports);
            if let Some(t) = report::repeat_table(&reports) {
                table = format!("{table}\n{t}");
            }
            if let Some(t) = report::stage_table(&reports) {
                table = format!("{table}\nstages (mean per frame that ran them):\n{t}");
            }
            if let Some(t) = report::stream_table(&reports) {
                table = format!("{table}\n{t}");
            }
            println!("\n{table}");
            std::fs::write(run_dir.join("summary.txt"), &table)?;
            std::fs::write(run_dir.join("report.json"), serde_json::to_string_pretty(&reports)?)?;
            let run_name = run_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let n = observe::write(&run_dir.join("observations.jsonl"), &reports, &plan, &run_name, &observe::Machine::detect(), scene_features.as_ref())?;
            println!("report: {} ({n} observations)", run_dir.join("report.json").display());
            Ok(())
        }
        Cmd::View { plan: path, renderers, overrides } => {
            let mut plan = Plan::load(&path, &overrides)?;
            if !renderers.is_empty() {
                plan.renderers = renderers
                    .iter()
                    .map(|r| {
                        let mut s = plan::RendererSpec::from_cli(r)?;
                        s.resolve()?;
                        Ok(s)
                    })
                    .collect::<anyhow::Result<_>>()?;
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
    fn spawn_lanes(&self, specs: &[RendererSpec], mode: Option<PassMode>) -> (Vec<Lane>, Vec<PassReport>) {
        let run = &self.plan.run;
        let [w, h] = run.resolution;
        let (mut lanes, mut failed) = (vec![], vec![]);
        for spec in specs {
            // Pipelines compiled for this machine are told apart from the generic build.
            let label = match spec.compiled(run) {
                Some(c) => format!("{} [{}]", spec.label(), c.name()),
                None => spec.label().to_string(),
            };
            let mut report = PassReport {
                label: label.clone(),
                renderer: spec.name.clone(),
                composition: spec.compose.clone(),
                sites: spec.sites.clone(),
                graph: spec.graph.clone(),
                graph_key: spec.pipeline().and_then(|g| g.key().ok()),
                options: spec.extra_options(),
                view_radius: spec.view_radius(run),
                link: spec.link(),
                mode,
                resolution: [w, h],
                ..Default::default()
            };
            let mode_tag = mode.map(|m| m.name()).unwrap_or("view");
            let log = self.run_dir.map(|d| d.join("logs").join(format!("{}-{mode_tag}.log", sanitize(&label))));
            let spawn = || worker::spawn_renderer(run, spec, &self.plan.scene.generator, self.blocks.clone(), log, run.resolution, run.in_flight + 2);
            match spawn() {
                Ok(worker) => {
                    report.init_ms = worker.init_ms;
                    report.tags = worker.info.tags.clone();
                    let i = lanes.len();
                    let region = spec.region(run, worker.info.max_lod);
                    lanes.push(Lane::new(worker, region, report, i, w, h));
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
        let (mut lanes, mut reports) = self.spawn_lanes(specs, Some(mode));
        if lanes.is_empty() {
            return Ok(reports);
        }
        let names: Vec<_> = lanes.iter().map(|l| l.report.label.clone()).collect();
        tracing::info!("pass: {} [{}]", names.join(", "), mode.name());
        let env = scene.environment;
        let mut driver = ScriptedDriver::new(scene, mode, warmup, run.frames, only_every, run.clock_fps);
        let result = run_pass(&mut lanes, &mut driver, &mut world, env, &lights, &mesh_data, run, out);
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

/// Runs every pass of `plan`; returns the reports and the scene's measured features.
fn run_plan(plan: &Plan, run_dir: &Path, ui: Option<UiHandle>) -> anyhow::Result<(Vec<PassReport>, Option<features::SceneFeatures>)> {
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
        // One streamer per distinct region the server streams (none for renderers generating
        // the world themselves).
        let mut configs: Vec<Vec<u32>> = plan
            .renderers
            .iter()
            .map(|r| r.region(run, infos.iter().find(|i| i.name == r.name).map(|i| i.max_lod).unwrap_or(0)).streamed())
            .filter(|r| !r.is_empty())
            .chain(std::iter::once(vec![run.view_radius]))
            .collect();
        configs.sort();
        configs.dedup();
        let mut sink = vec![];
        for radii in configs {
            let mut s = Streamer::new(radii, None);
            for f in 0..run.frames {
                s.update(&mut base, scene.camera(f).position, &mut sink);
                sink.clear();
            }
        }
        tracing::info!("pre-generated {} chunks in {:.1}s", base.cached_chunks(), t.elapsed().as_secs_f32());
    }
    // Renderer-independent properties of the scenario, stored with the observations.
    let t = Instant::now();
    let scene_features = match features::SceneFeatures::measure(&plan.scene, ctx.blocks.clone(), &mut base, run.frames, run.view_radius) {
        Ok(f) => {
            tracing::info!("scene features ({:.1}s): {}", t.elapsed().as_secs_f32(), f.describe());
            Some(f)
        }
        Err(e) => {
            tracing::warn!("scene features: {e:#}");
            None
        }
    };

    let save = plan.output.save.clone().map(|s| (s, run_dir.join("frames")));
    let mut out = Outputs::new(ui, plan.output.readback, save, plan.output.compare.as_ref().map(|c| c.every), run.frames);
    let mut reports = vec![];
    for &mode in &run.modes {
        if let Some(c) = &plan.output.compare {
            out.recording_reference = true;
            out.references.retain(|k, _| k.0 != mode);
            let mut spec = RendererSpec::new(c.reference.clone());
            (spec.label, spec.options) = (Some(format!("{} (ref)", c.reference)), c.reference_options.clone());
            reports.extend(ctx.pass(&[spec], mode, &base, &mut out, Some(c.every), 0)?);
            out.recording_reference = false;
        }
        // Swept compositions run as what this mode can tell apart, once each.
        let mut seen = std::collections::HashSet::new();
        let mut moot = 0;
        let specs: Vec<RendererSpec> = plan
            .renderers
            .iter()
            .filter(|s| {
                let ok = infos.iter().any(|i| i.name == s.name && s.supports(i, mode));
                if !ok && !s.swept {
                    tracing::warn!("{} does not support {} mode; skipped", s.label(), mode.name());
                }
                ok
            })
            .filter_map(|s| {
                if !s.swept {
                    return Some(s.clone());
                }
                let e = s.effective(mode).ok()?;
                if seen.insert((e.label().to_string(), toml::to_string(&e.extra_options()).unwrap_or_default())) {
                    Some(e)
                } else {
                    moot += 1;
                    None
                }
            })
            .collect();
        if moot > 0 {
            tracing::info!("{} mode: {} swept compositions run, {moot} more are the same in this mode", mode.name(), specs.iter().filter(|s| s.swept).count());
        }
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
                        return Ok((reports, scene_features));
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
    Ok((reports, scene_features))
}

fn view(plan: &Plan, ui: UiHandle) -> anyhow::Result<()> {
    let ctx = Ctx { plan, blocks: Arc::new(default_registry()), run_dir: None, ui: Some(ui.clone()) };
    let mut world = World::new(plan.scene.generator.build());
    let scene = Scene::new(plan.scene.clone(), ctx.blocks.clone(), &mut world, plan.run.frames)?;
    let lights = scene.initial_lights().to_vec();
    let mesh_data = scene.mesh_data();
    let (mut lanes, failed) = ctx.spawn_lanes(&plan.renderers, None);
    for f in failed {
        tracing::error!("{}: {}", f.label, f.error.unwrap_or_default());
    }
    if lanes.is_empty() {
        bail!("no renderer started");
    }
    let mode = plan.run.modes.first().copied().unwrap_or(PassMode::UNLIT);
    let env = scene.environment;
    let mut driver = InteractiveDriver::new(ui.clone(), scene, mode);
    let mut out = Outputs::new(Some(ui), plan::Readback::Auto, None, None, u32::MAX);
    run_pass(&mut lanes, &mut driver, &mut world, env, &lights, &mesh_data, &plan.run, &mut out).context("view session")?;
    for l in lanes {
        l.worker.shutdown();
    }
    Ok(())
}
