//! `multirenderer`: run, benchmark and compare voxel renderers, each sandboxed in its own
//! worker process.

mod bayes;
mod compare;
mod conditions;
#[cfg(test)]
mod contract;
mod features;
mod journal;
mod metrics;
mod catalogue;
mod gui;
mod observe;
mod pause;
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
        /// Render nothing: list the plan's renderers (sweeps expanded), then per mode how
        /// many passes it would run.
        #[arg(long)]
        list: bool,
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
    /// The metrics a pass's record can hold: name, unit, which way is better, method version,
    /// description, where the report holds it and where version-6 records held it. Markdown,
    /// or JSON with `--json` (read by the guard and `records.py`).
    Metrics {
        #[arg(long)]
        json: bool,
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
    /// What each component can be used for: per device kind, its implemented APIs and the
    /// shipped graphs that accept it there (Markdown), then the gaps.
    Audit,
    /// A pipeline as a graph: its nodes (TOML, for a plan's
    /// `[renderer.graph]`), label, canonical form, cache key and schedule. The renderer as for
    /// `view -r`: a graph file and edits (`raycast-gpu,storage=prefix,kernel=incremental`).
    /// With `--keys`: reads observations (JSON lines) on stdin and prints each one's graph
    /// key and label, tab-separated (`-`, nothing and the reason if it has none: a form or a
    /// component this build no longer reads).
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
        // Its stderr is a pipe to the host, gone when the host dies: a failed write is dropped,
        // not reported (the report is an `eprintln!` to the same pipe, which panics).
        sub.with_ansi(false).without_time().log_internal_errors(false).init();
    } else {
        sub.init();
    }

    match cmd {
        Cmd::Gui { plan } => gui::run(plan),
        Cmd::Worker { socket } => {
            mr_serve::own_process();
            mr_worker::run(&socket)
        }
        Cmd::Present { socket, title } => present::run(&socket, &title),
        Cmd::Graph { keys: true, .. } => {
            for (i, line) in std::io::stdin().lines().enumerate() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                // A record newer than this build is refused (by name), not left out.
                if let Some(why) = observe::newer(&line) {
                    bail!("record {}: {why}", i + 1);
                }
                // No key: `-`, no label, and why (the third column), e.g. a component since removed.
                let o = observe::Observation::parse(&line).map_err(|e| e.to_string());
                let g = match o {
                    Ok(Some(o)) => o.pipeline().ok_or_else(|| "a standalone renderer".to_string()),
                    Ok(None) => Err(format!("a pipeline record older than version {} (a form this build no longer reads)", observe::VERSION)),
                    Err(e) => Err(e),
                };
                match g.and_then(|g| Ok((g.key()?, g.label()?))) {
                    Ok((key, label)) => println!("{key}\t{label}"),
                    Err(why) => println!("-\t\t{}", why.replace(['\t', '\n'], " ")),
                }
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
        Cmd::Audit => {
            print!("{}", mr_components::graph::audit_markdown());
            Ok(())
        }
        Cmd::List => {
            print!("{}", list()?);
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
        Cmd::Metrics { json } => {
            if json {
                println!("{}", metrics::json());
            } else {
                println!("| metric | unit | better | method | description |\n|---|---|---|---:|---|");
                for m in metrics::METRICS {
                    println!("| `{}` | {} | {:?} | {} | {} |", m.name, m.unit, m.better, m.method, m.description);
                }
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
        Cmd::Run { plan: path, overrides, unset, list: true } => {
            let mut plan = Plan::load_with(&path, &overrides, &unset)?;
            plan.distinct_labels();
            let infos = mr_worker::infos();
            for r in &plan.renderers {
                println!("{}", r.label());
            }
            let swept = plan.renderers.iter().filter(|s| s.swept).count();
            println!("# {}: {} renderers ({swept} swept)", plan.name, plan.renderers.len());
            for &mode in &plan.run.modes {
                let (specs, moot) = plan.passes(mode, &infos);
                println!("# {} mode: {} passes ({moot} more swept pipelines the same in this mode)", mode.name(), specs.len());
            }
            Ok(())
        }
        Cmd::Run { plan: path, overrides, unset, .. } => {
            let mut plan = Plan::load_with(&path, &overrides, &unset)?;
            plan.distinct_labels();
            let resumed = plan.output.resume.then(|| journal::unfinished(&run_base(&plan), &sanitize(&plan.name))).flatten();
            let run_dir = match &resumed {
                Some(d) => d.clone(),
                None => make_run_dir(&plan)?,
            };
            let text = std::fs::read_to_string(&path)?;
            if resumed.is_none() {
                std::fs::write(run_dir.join("plan.toml"), format!("# overrides: {overrides:?}, unset: {unset:?}\n{text}"))?;
            }
            tracing::info!("run directory {}{}", run_dir.display(), if resumed.is_some() { " (resumed)" } else { "" });
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
            std::fs::write(run_dir.join("summary.txt"), &table).with_context(|| format!("{}: the run stays unfinished: with output.resume a later run writes its records", run_dir.join("summary.txt").display()))?;
            drop(reports);
            let run_name = run_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let n = finish_run(&run_dir, &plan, &run_name, scene_features.as_ref())?;
            println!("report: {} ({n} observations)", run_dir.join("report.json").display());
            // What the run held at most (its workers apart, under process isolation): flat in
            // the passes it runs, as their frames live in the journal.
            if let Some(kb) = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmHWM:")?.trim().strip_suffix(" kB")?.parse::<u64>().ok())) {
                println!("host peak memory: {:.0} MB", kb as f64 / 1024.0);
            }
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

/// What `multirenderer list` prints: how a renderer is written, the shipped graph files, the
/// components by role (with the files that offer each) and the standalone renderers.
fn list() -> anyhow::Result<String> {
    use std::fmt::Write as _;
    let mut s = String::new();
    writeln!(s, "A renderer is a pipeline graph: a graph file and the edits made to it.\n  plan: graph = \"raycast-gpu\", edits = [{{ node = \"storage\", component = \"tree64\" }}] (`vary` sweeps them; docs/plans.md)\n  CLI:  -r <graph>[,<edit>,...] with <graph> a shipped file's name or a path, and an edit one of\n          node=component          -r raycast-gpu,storage=prefix,kernel=incremental\n          node.input=source       an existing node, or a new one as role:component (its id: the role's name); empty: unbound\n                                  -r raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage\n          node.knob=value         -r raycast-gpu,shading=pathtrace,shading.bounces=2\n          node@device             -r raycast-gpu,shading@gpu1\n          -node                   -r raycast-gpu,-meshes\n          {{...}}                   any edit as the plan's inline table: -r 'raycast-gpu,{{node=\"kernel\",api=\"rayquery\"}}'\n  `multirenderer graph <renderer>` prints the graph, its label, key and schedule.\n\nShipped graph files (pipelines/*.toml; starting points: the pipeline is the graph, which may take any valid shape), each with its label:")?;
    // Each file's graph and, per node, what the node may be instead.
    let files: Vec<_> = mr_components::graph::start_files().map(|(name, about)| (name, about, mr_components::graph::shipped(name).expect("a shipped file"))).collect();
    let choices: Vec<Vec<(mr_components::graph::Role, Vec<&str>)>> = files.iter().map(|(.., g)| g.nodes.iter().map(|(id, n)| (n.role, g.choices(id))).collect()).collect();
    for (name, about, g) in &files {
        writeln!(s, "  {name:<12} {about}\n               {}", g.label().map_err(anyhow::Error::msg)?)?;
    }
    writeln!(s, "\nComponents, by the role of the node they fill (in: the shipped files with a node that may be it; others take it by an edit that adds the node):")?;
    for role in mr_components::graph::Role::ALL {
        let names = mr_components::graph::role_components(role);
        if !names.is_empty() {
            writeln!(s, "\n{}:", role.name())?;
        }
        for c in names {
            let on: Vec<&str> = files.iter().zip(&choices).filter(|(_, ch)| ch.iter().any(|(r, of)| *r == role && of.contains(&c))).map(|((name, ..), _)| *name).collect();
            let on = if on.is_empty() { String::new() } else { format!(" (in {})", on.join(",")) };
            match mr_components::component(role, c) {
                Some(i) => writeln!(s, "  {c:<12} [{}]{on} {}", i.impls_label(), i.description)?,
                None => writeln!(s, "{}", format!("  {c:<12}{on}").trim_end())?,
            }
        }
    }
    writeln!(s, "\nStandalone renderers (-r name; cpu-reference defines the reference images):")?;
    for i in mr_worker::infos() {
        writeln!(s, "  {:<16} [{}] {}\n      {}", i.name, i.tags.join(","), i.features.describe(), i.description)?;
    }
    Ok(s)
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
    let dir = run_base(plan).join(format!("{stamp}-{}", sanitize(&plan.name)));
    std::fs::create_dir_all(dir.join("logs"))?;
    Ok(dir)
}

/// Where the plan's run directories go.
fn run_base(plan: &Plan) -> PathBuf {
    PathBuf::from(plan.output.dir.clone().unwrap_or_else(|| "runs".into()))
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

    /// A pass's report before it runs: what it is.
    fn base_report(&self, spec: &RendererSpec, mode: Option<PassMode>) -> PassReport {
        let run = &self.plan.run;
        PassReport {
            label: self.label(spec),
            renderer: spec.entry().to_string(),
            graph_key: spec.pipeline().and_then(|g| g.key().ok()),
            graph: spec.pipeline(),
            options: spec.options.clone(),
            compiled: spec.compiled(run).map(|c| c.name().to_string()),
            view_radius: spec.view_radius(run),
            world_gated: spec.gated_world(),
            mode,
            resolution: run.resolution,
            ..Default::default()
        }
    }

    fn spawn_lanes(&self, specs: &[RendererSpec], mode: Option<PassMode>, asks: &[mr_core::Feature]) -> (Vec<Lane>, Vec<PassReport>) {
        let run = &self.plan.run;
        let [w, h] = run.resolution;
        let (mut lanes, mut failed) = (vec![], vec![]);
        for spec in specs {
            let label = self.label(spec);
            let mut report = self.base_report(spec, mode);
            let mode_tag = mode.map(|m| m.name()).unwrap_or("view");
            let log = self.run_dir.map(|d| d.join("logs").join(format!("{}-{mode_tag}.log", sanitize(&label))));
            let spawn = || worker::spawn_renderer(run, spec, &self.plan.scene.generator, self.blocks.clone(), log, run.resolution, run.in_flight + 2);
            match spawn() {
                Ok(worker) => {
                    report.init_ms = worker.init_ms;
                    report.shaders = worker.shaders;
                    report.tags = worker.info.tags.clone();
                    let i = lanes.len();
                    let region = spec.region(run, worker.info.features.max_lod());
                    // What the plan asks beyond what the pipeline reports is not asked of it.
                    report.not_supported = spec.not_supported(run, mode, asks, &worker.info);
                    let mut lane = Lane::new(worker, region, report, i, w, h);
                    // A reference records for the region it was given; a pipeline given another
                    // region than the full-resolution reference is scored against that one too.
                    lane.like = spec.like.or(plan::Like::of(&lane.region, mode.map(|m| spec.lighting(m)).unwrap_or_default(), plan::Leaves::of(&lane.worker.info.features, mode, asks), run).filter(|_| self.plan.output.compare.is_some() && spec.name != self.plan.output.compare.as_ref().unwrap().reference));
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
                match l.lighting {
                    plan::Lighting::Paths { bounces } => _ = spec.options.insert("bounces".into(), (bounces as i64).into()),
                    plan::Lighting::Occlusion { reach } => _ = spec.options.insert("occlusion".into(), (reach as f64).into()),
                    plan::Lighting::Direct => {}
                }
                if l.leaves.mesh_rays {
                    spec.options.insert("mesh_rays".into(), false.into());
                }
                if l.leaves.transparency {
                    spec.options.insert("transparency".into(), false.into());
                }
                if l.leaves.volume {
                    spec.options.insert("heightmap".into(), true.into());
                }
                format!("{} as {l} (ref)", c.reference)
            }
            None => format!("{} (ref)", c.reference),
        });
        spec.like = like;
        out.recording_reference = true;
        let reports = self.pass(&[spec], mode, base, out, Some(c.every), 0);
        out.recording_reference = false;
        reports
    }

    /// The settled score of a pipeline with late edges or accumulated frames: a pass of its
    /// own over the same frames, the compared ones held still for `held` more renders
    /// (`CompareSpec::held`) and the last one scored (against the like reference where it
    /// has one).
    fn settled(&self, spec: &RendererSpec, mode: PassMode, base: &World, out: &mut Outputs, held: u32) -> anyhow::Result<Option<report::QualitySummary>> {
        let mut still = spec.clone();
        still.label = Some(format!("{} (settled)", spec.label()));
        let label = self.label(&still);
        let reports = self.pass(&[still], mode, base, out, None, held)?;
        Ok(reports.into_iter().find(|r| r.label == label).and_then(|r| r.quality_like.or(r.quality)))
    }

    /// One pass: the given renderers run together through the scripted timeline. With
    /// `only_every`, only the compared frames are rendered; with `settle`, the compared
    /// frames that many more times, held still, and the last recorded. Every pass has the
    /// plan's warm-up: what a budgeted stream has sent by a frame depends on it.
    /// A pause asked for while it runs (`pause`) drops it; it runs again on resume.
    fn pass(&self, specs: &[RendererSpec], mode: PassMode, base: &World, out: &mut Outputs, only_every: Option<u32>, settle: u32) -> anyhow::Result<Vec<PassReport>> {
        // The references these lanes recorded, kept when the pass is redone (not recorded again).
        let mut refs = vec![];
        loop {
            pause::hold();
            if let Some(mut reports) = self.attempt(specs, mode, base, out, only_every, settle, &mut refs)? {
                refs.append(&mut reports);
                return Ok(refs);
            }
            tracing::info!("pass dropped by a pause; it runs again");
        }
    }

    /// One attempt at `pass`; None when a pause dropped it.
    #[allow(clippy::too_many_arguments)]
    fn attempt(&self, specs: &[RendererSpec], mode: PassMode, base: &World, out: &mut Outputs, only_every: Option<u32>, settle: u32, refs: &mut Vec<PassReport>) -> anyhow::Result<Option<Vec<PassReport>>> {
        let run = &self.plan.run;
        let mut world = base.clone();
        let scene = Scene::new(self.plan.scene.clone(), self.blocks.clone(), &mut world, run.frames)?;
        let lights = scene.initial_lights().to_vec();
        let mesh_data = if mode.meshes { scene.mesh_data() } else { vec![] };
        let (mut lanes, mut reports) = self.spawn_lanes(specs, Some(mode), &world.asks(&self.blocks));
        if lanes.is_empty() {
            return Ok(Some(reports));
        }
        // The references given what these lanes are given, recorded before the lanes run.
        if !out.recording_reference && out.compare_every.is_some() {
            for like in lanes.iter().filter_map(|l| l.like).collect::<Vec<_>>() {
                if !out.references.keys().any(|k| k.0 == Some(like) && k.1 == mode) {
                    refs.extend(self.reference(Some(like), mode, base, out)?);
                }
            }
        }
        let names: Vec<_> = lanes.iter().map(|l| l.report.label.clone()).collect();
        tracing::info!("pass: {} [{}]", names.join(", "), mode.name());
        let env = scene.environment;
        let mut driver = ScriptedDriver::new(scene, mode, run.warmup, run.frames, only_every, run.clock_fps);
        driver.settle = out.compare_every.filter(|_| settle > 0).map(|every| (every, settle));
        let result = run_pass(&mut lanes, &mut driver, &mut world, env, &lights, &mesh_data, &self.blocks, run, out);
        if result.as_ref().is_err_and(pause::is) {
            for lane in lanes {
                lane.worker.shutdown();
            }
            return Ok(None);
        }
        if let Err(e) = &result {
            tracing::error!("pass failed: {e:#}");
        }
        // What the world asked by the end (edits may have placed see-through blocks).
        let asks = world.asks(&self.blocks);
        for lane in lanes {
            let mut r = lane.report;
            if let Some(spec) = specs.iter().find(|s| self.label(s) == r.label) {
                r.not_supported = spec.not_supported(run, Some(mode), &asks, &lane.worker.info);
            }
            if let (None, Err(e)) = (&r.error, &result) {
                r.error = Some(format!("{e:#}"));
            }
            r.shutdown_error = lane.worker.shutdown();
            reports.push(r);
        }
        // Pipelines with an edge a frame late, or that accumulate frames held still, are
        // scored again on settled frames.
        if let Some(c) = self.plan.output.compare.as_ref().filter(|_| only_every.is_none() && settle == 0 && out.compare_every.is_some()) {
            for spec in specs {
                let held = c.held(spec, mode);
                let label = self.label(spec);
                if held == 0 || !reports.iter().any(|r| r.label == label && r.error.is_none()) {
                    continue;
                }
                let settled = self.settled(spec, mode, base, out, held)?;
                let r = reports.iter_mut().find(|r| r.label == label).unwrap();
                let converged = spec.accumulates(mode).map(|spp| spp * (held + 1));
                (r.late, r.converged_spp, r.quality_settled) = (spec.late(), converged, settled);
            }
        }
        println!("{}", report::table(&reports).lines().skip(1).collect::<Vec<_>>().join("\n"));
        Ok(Some(reports))
    }
}

/// Runs every pass of `plan`; returns the reports and the scene's measured features.
fn run_plan(plan: &Plan, run_dir: &Path, ui: Option<UiHandle>) -> anyhow::Result<(Vec<PassReport>, Option<features::SceneFeatures>)> {
    let infos = mr_worker::infos();
    for r in &plan.renderers {
        if r.info(&plan.run, &infos).is_none() {
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
            .map(|r| r.region(run, r.info(&plan.run, &infos).map_or(0, |i| i.features.max_lod())).streamed())
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
    let mut journal = journal::Journal::open(run_dir)?;
    if !journal.kept.is_empty() {
        tracing::info!("resumed: {} reports of passes an earlier run finished stand", journal.kept.len());
    }
    let mut reports = vec![];
    for &mode in &run.modes {
        if plan.output.compare.is_some() {
            out.references.retain(|k, _| k.1 != mode);
            let refs = ctx.reference(None, mode, &base, &mut out)?;
            journal.beside(&refs)?;
            reports.extend(refs.into_iter().map(PassReport::slim));
        }
        let (specs, moot) = plan.passes(mode, &infos);
        for s in plan.renderers.iter().filter(|s| !s.swept && !s.info(&plan.run, &infos).is_some_and(|i| s.supports(&i, mode))) {
            tracing::warn!("{} does not support {} mode; skipped", s.label(), mode.name());
        }
        if moot > 0 {
            tracing::info!("{} mode: {} swept pipelines run, {moot} more are the same in this mode", mode.name(), specs.iter().filter(|s| s.swept).count());
        }
        // Repeats go round-robin (A B C A B C ...) so slow drifts in clocks and
        // temperature spread over all renderers instead of favouring whichever ran first.
        for repeat in 0..run.repeats.max(1) {
            let mut batch = vec![];
            let groups: Vec<&[RendererSpec]> = if run.parallel { vec![&specs[..]] } else { specs.chunks(1).collect() };
            for group in groups {
                // The passes an earlier run finished stand; one it died in too often is failed.
                let key = |s: &RendererSpec| journal::Key { label: ctx.label(s), mode: mode.name().to_string(), repeat };
                let mut todo = vec![];
                for s in group {
                    let k = key(s);
                    if journal.done(&k) {
                        continue;
                    }
                    if let Some(why) = journal.given_up(&k) {
                        tracing::error!("[{}] {why}", k.label);
                        let mut r = ctx.base_report(s, Some(mode));
                        (r.error, r.repeat) = (Some(why), repeat);
                        journal.finished(&k, std::slice::from_ref(&r))?;
                        batch.push(r);
                        continue;
                    }
                    journal.started(&k)?;
                    todo.push(s.clone());
                }
                if todo.is_empty() {
                    continue;
                }
                let mut done = ctx.pass(&todo, mode, &base, &mut out, None, 0)?;
                for r in &mut done {
                    r.repeat = repeat;
                }
                // Run side by side, they end together: their reports written once.
                for (i, s) in todo.iter().enumerate() {
                    journal.finished(&key(s), if i == 0 { &done } else { &[] })?;
                }
                batch.extend(done.into_iter().map(PassReport::slim));
                if out.ui.as_ref().is_some_and(|u| u.closed()) {
                    reports.extend(batch);
                    return Ok((with_kept(journal.kept, reports), scene_features));
                }
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
    Ok((with_kept(journal.kept, reports), scene_features))
}

/// Writes a finished run's records from its journal, one report at a time (the run held only
/// slim copies): the observations, then `report.json` (compact: every frame of every pass), each
/// written beside and renamed into place, so a failed write (a full disk) leaves no partial
/// file and the run stays unfinished, resumable; then the journal goes. Returns the
/// observations written.
fn finish_run(run_dir: &Path, plan: &Plan, run_name: &str, features: Option<&features::SceneFeatures>) -> anyhow::Result<usize> {
    use std::io::Write as _;
    let create = |name: &str| -> anyhow::Result<(PathBuf, std::io::BufWriter<std::fs::File>)> {
        let part = run_dir.join(format!("{name}.part"));
        let f = std::fs::File::create(&part).with_context(|| part.display().to_string())?;
        Ok((part, std::io::BufWriter::new(f)))
    };
    let close = |part: PathBuf, w: std::io::BufWriter<std::fs::File>, name: &str| -> anyhow::Result<()> {
        let f = w.into_inner().map_err(|e| e.into_error()).with_context(|| part.display().to_string())?;
        f.sync_data().with_context(|| part.display().to_string())?;
        std::fs::rename(&part, run_dir.join(name)).with_context(|| part.display().to_string())
    };
    let machine = observe::Machine::detect();
    let (op, rp) = (run_dir.join("observations.jsonl.part"), run_dir.join("report.json.part"));
    let write = || -> anyhow::Result<usize> {
        let ((_, mut ow), (_, mut rw)) = (create("observations.jsonl")?, create("report.json")?);
        let (mut n, mut first) = (0, true);
        rw.write_all(b"[")?;
        journal::each(run_dir, |r| {
            if let Some(line) = observe::line(&r, plan, run_name, &machine, features)? {
                ow.write_all(line.as_bytes())?;
                ow.write_all(b"\n")?;
                n += 1;
            }
            if !first {
                rw.write_all(b",")?;
            }
            first = false;
            serde_json::to_writer(&mut rw, &r)?;
            Ok(())
        })?;
        rw.write_all(b"]")?;
        close(op.clone(), ow, "observations.jsonl")?;
        close(rp.clone(), rw, "report.json")?;
        Ok(n)
    };
    let n = write().map_err(|e| {
        // What was written of them goes, so the space is free for the resumed run.
        let _ = std::fs::remove_file(&op);
        let _ = std::fs::remove_file(&rp);
        e.context(format!("writing the run's records ({}, report.json) from its journal failed; the run stays unfinished: with output.resume a later run writes them", run_dir.join("observations.jsonl").display()))
    })?;
    let _ = std::fs::remove_file(run_dir.join(journal::FILE));
    Ok(n)
}

/// The reports of the passes earlier runs finished (`kept`, a resumed run's), then this run's;
/// a reference both recorded, once (this run's).
fn with_kept(kept: Vec<PassReport>, reports: Vec<PassReport>) -> Vec<PassReport> {
    let fresh: std::collections::HashSet<_> = reports.iter().map(|r| (r.label.clone(), r.mode, r.repeat)).collect();
    let mut all: Vec<PassReport> = kept.into_iter().filter(|r| !fresh.contains(&(r.label.clone(), r.mode, r.repeat))).collect();
    all.extend(reports);
    all
}

fn view(plan: &Plan, ui: UiHandle) -> anyhow::Result<()> {
    let ctx = Ctx { plan, blocks: Arc::new(default_registry()), run_dir: None, ui: Some(ui.clone()) };
    let mut world = World::new(plan.scene.generator.build());
    let scene = Scene::new(plan.scene.clone(), ctx.blocks.clone(), &mut world, plan.run.frames)?;
    let lights = scene.initial_lights().to_vec();
    let mesh_data = scene.mesh_data();
    let (mut lanes, failed) = ctx.spawn_lanes(&plan.renderers, None, &world.asks(&ctx.blocks));
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
    run_pass(&mut lanes, &mut driver, &mut world, env, &lights, &mesh_data, &ctx.blocks, &plan.run, &mut out).context("view session")?;
    if lanes.iter().all(|l| l.report.error.is_some()) {
        bail!("every renderer failed");
    }
    for l in lanes {
        l.worker.shutdown();
    }
    Ok(())
}
