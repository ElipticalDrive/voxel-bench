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
mod present;
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
    /// `[renderer.graph]`), label, canonical form, cache key and schedule. The renderer as for
    /// `view -r`: a graph file and edits (`raycast-gpu,storage=prefix,kernel=incremental`).
    /// With `--keys`: reads observations (JSON lines) on stdin and prints each one's graph
    /// key (`-` if it has none), for records written before they carried one.
    Graph {
        renderer: Option<String>,
        #[arg(long)]
        keys: bool,
    },
    /// The cache of pipelines compiled for this machine: where it is and what it holds;
    /// `--clear` empties it (each pipeline compiles again when next used).
    Cache {
        #[arg(long)]
        clear: bool,
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
    /// Internal: present one lane's frames in a window of its own (spawned by the GUI).
    #[command(hide = true)]
    Present {
        #[arg(long)]
        socket: String,
        #[arg(long)]
        title: String,
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
        Cmd::Present { socket, title } => present::run(&socket, &title),
        Cmd::Graph { keys: true, .. } => {
            for line in std::io::stdin().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let g = observe::Observation::parse(&line).ok().flatten().and_then(|o| o.pipeline());
                let key = g.as_ref().and_then(|g| g.key().ok());
                println!("{}\t{}", key.as_deref().unwrap_or("-"), g.and_then(|g| g.label().ok()).unwrap_or_default());
            }
            Ok(())
        }
        Cmd::Graph { renderer, .. } => {
            let renderer = renderer.context("which renderer? e.g. `graph raycast-gpu,storage=prefix,kernel=incremental`")?;
            let mut spec = plan::RendererSpec::from_cli(&renderer)?;
            spec.resolve()?;
            let g = spec.pipeline().with_context(|| format!("`{renderer}` is not a pipeline (a graph file, then edits)"))?;
            println!("# {}\n{}", spec.label(), toml::to_string(&g)?);
            println!("# canonical: {}\n# key: {}", g.canonical().map_err(anyhow::Error::msg)?, g.key().map_err(anyhow::Error::msg)?);
            let schedule = g.check().and_then(|c| c.schedule()).map_err(anyhow::Error::msg)?;
            println!("# schedule (passes in frame order, [fused nodes compiled in]):");
            print!("{}", schedule.to_string().lines().map(|l| format!("#{l}\n")).collect::<String>());
            Ok(())
        }
        Cmd::List => {
            println!("A renderer is a pipeline graph: a graph file and the edits made to it.\n  plan: graph = \"raycast-gpu\", edits = [{{ node = \"storage\", component = \"tree64\" }}] (`vary` sweeps them; docs/plans.md)\n  CLI:  -r <graph>[,<edit>,...] with <graph> a shipped file's name or a path, and an edit one of\n          node=component          -r raycast-gpu,storage=prefix,kernel=incremental\n          node.input=source       an existing node, or a new one as role:component (its id: the role's name); empty: unbound\n                                  -r raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage\n          node.knob=value         -r raycast-gpu,shading=pathtrace,shading.bounces=2\n          node@device             -r raycast-gpu,shading@gpu1\n          -node                   -r raycast-gpu,-meshes\n          {{...}}                   any edit as the plan's inline table: -r 'raycast-gpu,{{node=\"kernel\",api=\"rayquery\"}}'\n  `multirenderer graph <renderer>` prints the graph, its label, key and schedule.\n\nShipped graph files (pipelines/*.toml; starting points: the pipeline is the graph, which may take any valid shape), each with its label:");
            // Each file's graph and, per node, what the node may be instead.
            let files: Vec<_> = mr_components::graph::start_files().map(|(name, about)| (name, about, mr_components::graph::shipped(name).expect("a shipped file"))).collect();
            let choices: Vec<Vec<(mr_components::graph::Role, Vec<&str>)>> = files.iter().map(|(.., g)| g.nodes.iter().map(|(id, n)| (n.role, g.choices(id))).collect()).collect();
            for (name, about, g) in &files {
                println!("  {name:<12} {about}\n               {}", g.label().map_err(anyhow::Error::msg)?);
            }
            println!("\nComponents, by the role of the node they fill (in: the shipped files with a node that may be it; others take it by an edit that adds the node):");
            for role in mr_components::graph::Role::ALL {
                let names = mr_components::graph::role_components(role);
                if !names.is_empty() {
                    println!("\n{}:", role.name());
                }
                for c in names {
                    let on: Vec<&str> = files.iter().zip(&choices).filter(|(_, ch)| ch.iter().any(|(r, of)| *r == role && of.contains(&c))).map(|((name, ..), _)| *name).collect();
                    let on = if on.is_empty() { String::new() } else { format!(" (in {})", on.join(",")) };
                    match mr_components::component(role, c) {
                        Some(i) => println!("  {c:<12} [{}]{on} {}", i.impls_label(), i.description),
                        None => println!("{}", format!("  {c:<12}{on}").trim_end()),
                    }
                }
            }
            println!("\nStandalone renderers (-r name; cpu-reference defines the reference images):");
            for i in mr_worker::infos() {
                println!("  {:<16} [{}] {}\n      {}", i.name, i.tags.join(","), i.features.describe(), i.description);
            }
            Ok(())
        }
        Cmd::Cache { clear } => {
            let root = mr_core::cache_dir();
            if clear {
                std::fs::remove_dir_all(&root).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }).with_context(|| format!("removing {}", root.display()))?;
                println!("cleared {}", root.display());
                return Ok(());
            }
            let gb = |p: &std::path::Path| pipelines::size(p) as f64 / 1e9;
            println!("{}: {:.2} GB (limit: MR_PIPELINE_CACHE_GB, default 4)", root.display(), gb(&root));
            for e in std::fs::read_dir(&root).into_iter().flatten().filter_map(|e| e.ok()) {
                println!("  {:<16} {:>7.2} GB", e.file_name().to_string_lossy(), gb(&e.path()));
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
            let (obs, left_out) = observe::read(&files)?;
            if obs.is_empty() {
                bail!("no readable observations under {paths:?} ({left_out} of older versions this build no longer reads)");
            }
            let out = out.or_else(|| paths.first().filter(|p| p.is_dir()).cloned()).unwrap_or_else(|| PathBuf::from("."));
            let cat = catalogue::Catalogue::build(&obs, min_psnr);
            std::fs::create_dir_all(&out)?;
            std::fs::write(out.join("catalogue.md"), cat.markdown())?;
            std::fs::write(out.join("catalogue.csv"), cat.csv())?;
            println!("{}", cat.summary());
            let older = if left_out > 0 { format!(" ({left_out} more of older versions this build no longer reads, left out)") } else { String::new() };
            println!("{} observations from {} files{older} → {}", obs.len(), files.len(), out.join("catalogue.md").display());
            Ok(())
        }
        Cmd::Run { plan: path, overrides, unset } => {
            let mut plan = Plan::load_with(&path, &overrides, &unset)?;
            plan.distinct_labels();
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
    /// A renderer's label in reports: pipelines compiled for this machine are told apart
    /// from the generic build.
    fn label(&self, spec: &RendererSpec) -> String {
        spec.run_label(&self.plan.run)
    }

    fn spawn_lanes(&self, specs: &[RendererSpec], mode: Option<PassMode>) -> (Vec<Lane>, Vec<PassReport>) {
        let run = &self.plan.run;
        let [w, h] = run.resolution;
        let (mut lanes, mut failed) = (vec![], vec![]);
        for spec in specs {
            let label = self.label(spec);
            let mut report = PassReport {
                label: label.clone(),
                renderer: spec.entry().to_string(),
                graph_key: spec.pipeline().and_then(|g| g.key().ok()),
                graph: spec.pipeline(),
                options: spec.options.clone(),
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
                    let region = spec.region(run, worker.info.features.max_lod());
                    // What the plan asks beyond what the pipeline reports is not asked of it.
                    report.not_supported = spec.not_supported(run, mode, &worker.info);
                    let mut lane = Lane::new(worker, region, report, i, w, h);
                    // A reference records for the region it was given; a pipeline given another
                    // region than the full-resolution reference is scored against that one too.
                    lane.like = spec.like.or(plan::Like::of(&lane.region, mode.map(|m| spec.lighting(m)).unwrap_or_default(), run).filter(|_| self.plan.output.compare.is_some() && spec.name != self.plan.output.compare.as_ref().unwrap().reference));
                    if spec.like.is_none() {
                        lane.report.like = lane.like.map(|l| l.to_string());
                    }
                    lanes.push(lane);
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

    /// The reference's pass: its sampled frames recorded for the comparisons. With `like`, the
    /// reference given that region (its LOD levels) instead of everything at full resolution,
    /// computing that lighting model (path tracing, converged) instead of direct lighting.
    fn reference(&self, like: Option<plan::Like>, mode: PassMode, base: &World, out: &mut Outputs) -> anyhow::Result<Vec<PassReport>> {
        let Some(c) = &self.plan.output.compare else { return Ok(vec![]) };
        let mut spec = RendererSpec::new(c.reference.clone());
        spec.options = c.reference_options.clone();
        spec.label = Some(match like {
            Some(l) => {
                spec.options.insert("lod".into(), true.into());
                if let plan::Lighting::Paths { bounces } = l.lighting {
                    spec.options.insert("bounces".into(), (bounces as i64).into());
                }
                format!("{} as {l} (ref)", c.reference)
            }
            None => format!("{} (ref)", c.reference),
        });
        spec.like = like;
        out.recording_reference = true;
        let reports = self.pass(&[spec], mode, base, out, Some(c.every), 0, 0);
        out.recording_reference = false;
        reports
    }

    /// The settled score of a pipeline with late edges: a pass of its own over the same
    /// frames, the compared ones held still for `compare.settle` more renders and the last
    /// one scored (against the like reference where it has one). `None` with `settle = 0`.
    fn settled(&self, spec: &RendererSpec, mode: PassMode, base: &World, out: &mut Outputs) -> anyhow::Result<Option<report::QualitySummary>> {
        let Some(c) = self.plan.output.compare.as_ref().filter(|c| c.settle > 0) else { return Ok(None) };
        let mut held = spec.clone();
        held.label = Some(format!("{} (settled)", spec.label()));
        let label = self.label(&held);
        let reports = self.pass(&[held], mode, base, out, None, self.plan.run.warmup, c.settle)?;
        Ok(reports.into_iter().find(|r| r.label == label).and_then(|r| r.quality_like.or(r.quality)))
    }

    /// One pass: the given renderers run together through the scripted timeline. With
    /// `only_every`, only the compared frames are rendered; with `settle`, the compared
    /// frames that many more times, held still, and the last recorded.
    fn pass(&self, specs: &[RendererSpec], mode: PassMode, base: &World, out: &mut Outputs, only_every: Option<u32>, warmup: u32, settle: u32) -> anyhow::Result<Vec<PassReport>> {
        let run = &self.plan.run;
        let mut world = base.clone();
        let scene = Scene::new(self.plan.scene.clone(), self.blocks.clone(), &mut world, run.frames)?;
        let lights = scene.initial_lights().to_vec();
        let mesh_data = if mode.meshes { scene.mesh_data() } else { vec![] };
        let (mut lanes, mut reports) = self.spawn_lanes(specs, Some(mode));
        if lanes.is_empty() {
            return Ok(reports);
        }
        // The references given what these lanes are given, recorded before the lanes run.
        if !out.recording_reference && out.compare_every.is_some() {
            for like in lanes.iter().filter_map(|l| l.like).collect::<Vec<_>>() {
                if !out.references.keys().any(|k| k.0 == Some(like) && k.1 == mode) {
                    reports.extend(self.reference(Some(like), mode, base, out)?);
                }
            }
        }
        let names: Vec<_> = lanes.iter().map(|l| l.report.label.clone()).collect();
        tracing::info!("pass: {} [{}]", names.join(", "), mode.name());
        let env = scene.environment;
        let mut driver = ScriptedDriver::new(scene, mode, warmup, run.frames, only_every, run.clock_fps);
        driver.settle = out.compare_every.filter(|_| settle > 0).map(|every| (every, settle));
        let result = run_pass(&mut lanes, &mut driver, &mut world, env, &lights, &mesh_data, run, out);
        if let Err(e) = &result {
            tracing::error!("pass failed: {e:#}");
        }
        for lane in lanes {
            let mut r = lane.report;
            if let (None, Err(e)) = (&r.error, &result) {
                r.error = Some(format!("{e:#}"));
            }
            lane.worker.shutdown();
            reports.push(r);
        }
        // Pipelines with an edge a frame late are scored again on settled frames.
        if only_every.is_none() && settle == 0 && out.compare_every.is_some() {
            for spec in specs {
                let late = spec.late();
                let label = self.label(spec);
                if late.is_empty() || !reports.iter().any(|r| r.label == label && r.error.is_none()) {
                    continue;
                }
                let settled = self.settled(spec, mode, base, out)?;
                let r = reports.iter_mut().find(|r| r.label == label).unwrap();
                (r.late, r.quality_settled) = (late, settled);
            }
        }
        println!("{}", report::table(&reports).lines().skip(1).collect::<Vec<_>>().join("\n"));
        Ok(reports)
    }
}

/// Runs every pass of `plan`; returns the reports and the scene's measured features.
fn run_plan(plan: &Plan, run_dir: &Path, ui: Option<UiHandle>) -> anyhow::Result<(Vec<PassReport>, Option<features::SceneFeatures>)> {
    let infos = mr_worker::infos();
    for r in &plan.renderers {
        if r.info(&infos).is_none() {
            bail!("unknown renderer `{}` (see `multirenderer list`)", r.label());
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
            .map(|r| r.region(run, r.info(&infos).map_or(0, |i| i.features.max_lod())).streamed())
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
        if plan.output.compare.is_some() {
            out.references.retain(|k, _| k.1 != mode);
            reports.extend(ctx.reference(None, mode, &base, &mut out)?);
        }
        // Of the swept pipelines this mode cannot tell apart, the first runs.
        let mut seen = std::collections::HashSet::new();
        let mut moot = 0;
        let specs: Vec<RendererSpec> = plan
            .renderers
            .iter()
            .filter(|s| {
                let ok = s.info(&infos).is_some_and(|i| s.supports(&i, mode));
                if !ok && !s.swept {
                    tracing::warn!("{} does not support {} mode; skipped", s.label(), mode.name());
                }
                ok
            })
            .filter_map(|s| {
                if !s.swept {
                    return Some(s.clone());
                }
                if seen.insert(s.told_apart(mode)) {
                    Some(s.clone())
                } else {
                    moot += 1;
                    None
                }
            })
            .collect();
        if moot > 0 {
            tracing::info!("{} mode: {} swept pipelines run, {moot} more are the same in this mode", mode.name(), specs.iter().filter(|s| s.swept).count());
        }
        // Repeats go round-robin (A B C A B C ...) so slow drifts in clocks and
        // temperature spread over all renderers instead of favouring whichever ran first.
        for repeat in 0..run.repeats.max(1) {
            let mut batch = vec![];
            if run.parallel {
                batch.extend(ctx.pass(&specs, mode, &base, &mut out, None, run.warmup, 0)?);
            } else {
                for s in &specs {
                    batch.extend(ctx.pass(std::slice::from_ref(s), mode, &base, &mut out, None, run.warmup, 0)?);
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
    if lanes.iter().all(|l| l.report.error.is_some()) {
        bail!("every renderer failed");
    }
    for l in lanes {
        l.worker.shutdown();
    }
    Ok(())
}
