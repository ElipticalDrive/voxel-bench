//! Pipelines compiled on demand: for one graph, generate a
//! small Rust crate whose worker serves only that graph, built with `cargo` for this machine
//! (`-C target-cpu=native`). For the CPU executor (any of its lanes: ray casting,
//! rasterisation, columns) the lane's entry is instantiated for the graph's storage type with
//! its choices baked in as constants (`backend_cpu::Spec`); for the GPU executor its worker's
//! CPU work (client, storage encoding, meshing, discovery on the CPU in the graph's storage
//! alone) is built for this CPU (`backend_gpu::Spec`). The build runs in its own process,
//! the result is an ordinary worker executable spoken to over the usual protocol, so a bad
//! build only fails its worker and the host can swap it for the generic build.
//!
//! `break-in` spends what it takes on the same crate:
//! it builds it natively, with whole-program LTO, profile-guided (trained on a short flight of
//! the trust check's scene) and with both, times each here and keeps the fastest.
//!
//! Builds are cached by the pipeline's graph key (the renderer's graph with the
//! values of the knobs the generated code bakes placed on their nodes, [`Bake::graph`]) and
//! a hash over the generated source, the sources it compiles against, the toolchain
//! (`rustc -vV`) and the CPU (`/proc/cpuinfo` model and flags): a pipeline is compiled once
//! per machine and source state. Generated crates share one target directory, so the
//! workspace's crates are compiled once for all of them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::plan::{Compile, RendererSpec};

/// What a generated pipeline bakes in: the pipeline (its graph, with the values of the knobs
/// in [`bakes`] on the nodes they belong to: its identity, the cache key), the executor, its
/// entry and the `Spec` constants read off it.
#[derive(Clone, Debug, PartialEq)]
pub struct Bake {
    pub graph: mr_components::graph::Graph,
    pub executor: Executor,
    /// The renderer entry, a Rust expression over the spec `Baked`.
    pub entry: String,
    /// The `Spec` items (`const ...;`), `KNOBS` first.
    pub consts: Vec<String>,
}

/// The executor a generated pipeline instantiates.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Executor {
    Cpu,
    Gpu,
}

impl Executor {
    /// Its crate (package name, path in the repository) and the worker's devices.
    fn parts(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Executor::Cpu => ("backend-cpu", "backends/cpu", "mr_serve::cpu_devices"),
            Executor::Gpu => ("backend-gpu", "backends/gpu", "mr_gpu::devices"),
        }
    }

    /// The trait the generated `Baked` implements.
    fn spec(self) -> &'static str {
        match self {
            Executor::Cpu => "backend_cpu::Spec",
            Executor::Gpu => "backend_gpu::Spec",
        }
    }
}

/// The knobs a generated pipeline of CPU executor `backend` bakes in (each declared
/// specialisable and belonging to a node, a test checks): those its lane folds into code; the
/// others stay run-time values in it.
pub fn bakes(backend: &str) -> &'static [&'static str] {
    match backend {
        "cpu" => &["exact", "packet", "skip2", "skip4", "skip8", "skip16", "shadows"],
        "cpu-voxelspace" => &["shadows"],
        _ => &[],
    }
}

/// The executor of `backend` and its knobs (`None`: no generated pipeline runs it).
fn params_of(backend: &str) -> Option<(Executor, Vec<mr_core::ParamSpec>)> {
    let (executor, entry) = match backend {
        "cpu" => (Executor::Cpu, backend_cpu::cpu_entry()),
        "cpu-raster" => (Executor::Cpu, backend_cpu::cpu_raster_entry()),
        "cpu-voxelspace" => (Executor::Cpu, backend_cpu::cpu_voxelspace_entry()),
        "gpu-raster" => (Executor::Gpu, backend_gpu::gpu_raster_entry()),
        "gpu-discover" => (Executor::Gpu, backend_gpu::gpu_discover_entry()),
        "cpu-discover" => (Executor::Gpu, backend_gpu::cpu_discover_entry()),
        "gpu-compute" => (Executor::Gpu, backend_gpu::gpu_compute_entry()),
        "gpu-rt" => (Executor::Gpu, backend_gpu::gpu_rt_entry()),
        "gpu-splat" => (Executor::Gpu, backend_gpu::gpu_splat_entry()),
        "gpu-voxelspace" => (Executor::Gpu, backend_gpu::gpu_voxelspace_entry()),
        _ => return None,
    };
    Some((executor, (entry.info)().params))
}

/// A built pipeline worker.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub exe: PathBuf,
    pub key: String,
    /// Build time if it was built now (`None`: from the cache).
    pub build_s: Option<f64>,
}

/// What a renderer run by an executor bakes in (`None` for other renderers).
pub fn bake(spec: &RendererSpec) -> anyhow::Result<Option<Bake>> {
    use mr_components::graph::{Device, Role};
    let Some((executor, params)) = params_of(&spec.name) else { return Ok(None) };
    let graph = spec.pipeline().context("the renderer has no graph")?;
    // A knob's value: the renderer's, else the back end's default.
    let value = |k: &str| -> anyhow::Result<(&mr_core::ParamSpec, toml::Value)> {
        let p = params.iter().find(|p| p.name == k).with_context(|| format!("`{}` has no knob `{k}`", spec.name))?;
        Ok((p, spec.options.get(k).cloned().map_or_else(|| toml::Value::try_from(p.default.to_json()), Ok)?))
    };
    let knobs: Vec<_> = bakes(&spec.name).iter().map(|k| value(k)).collect::<anyhow::Result<_>>()?;
    let graph = graph.with_knobs(&knobs).map_err(anyhow::Error::msg)?;
    let ck = graph.check().map_err(anyhow::Error::msg)?;
    // The knobs compiled in: the baked ones that are on a node (not moot), and on the ray
    // casting lane the components the flat options name.
    let on_node = |k: &str| graph.nodes.values().any(|n| n.knobs.contains_key(k));
    let mut baked: Vec<(String, toml::Value)> = knobs.iter().filter(|(p, _)| on_node(&p.name)).map(|(p, v)| (p.name.clone(), v.clone())).collect();
    let rust = |v: &toml::Value| -> anyhow::Result<String> { Ok(format!("{:?}", serde_json::to_string(v)?)) };
    let variant = |s: &str| s[..1].to_uppercase() + &s[1..];
    let mut consts = vec![];
    if executor == Executor::Gpu {
        // The GPU executor's CPU work, built for this CPU; discovery on the CPU in the graph's
        // storage alone (the storage its rays walk).
        consts.push(format!("const BACKEND: &'static str = {:?};", spec.name));
        let entry = "backend_gpu::compiled_entry::<Baked>()".to_string();
        if ck.find(Role::Draw).is_some() && ck.find(Role::Visibility).is_some_and(|v| ck.devices[v] == Device::Cpu) {
            let kernel = ck.input(Role::Visibility, "rays").context("discovery casts no rays")?;
            let storage = ck.node(ck.node(kernel).inputs.get("storage").context("the discovery rays walk no storage")?).component.clone();
            let ty = backend_cpu::storage_type(&storage).with_context(|| format!("storage `{storage}` has no CPU implementation"))?;
            consts.push(format!("const CPU_WORLD: Option<fn() -> backend_gpu::CpuWorld> = Some(backend_gpu::CpuWorld::of::<{ty}>);"));
            baked.insert(0, ("storage".into(), storage.as_str().into()));
        }
        let knobs = baked.iter().map(|(k, v)| Ok(format!("({k:?}, {})", rust(v)?))).collect::<anyhow::Result<Vec<_>>>()?;
        consts.insert(0, format!("const KNOBS: &'static [(&'static str, &'static str)] = &[{}];", knobs.join(", ")));
        consts.push("const NATIVE: bool = true;".into());
        return Ok(Some(Bake { graph, executor, entry, consts }));
    }
    consts.push(format!("const TONE: Option<backend_cpu::Tone> = Some(backend_cpu::Tone::{:?});", ck.tone()));
    if let Some((_, v)) = baked.iter().find(|(k, _)| k == "shadows") {
        consts.push(format!("const SHADOWS: Option<backend_cpu::Shadows> = Some(backend_cpu::Shadows::{});", variant(v.as_str().context("shadows: a choice")?)));
    }
    // The output's pixel (the post's format).
    let px = match ck.tone().format() {
        mr_core::lighting::PixelFormat::Rgba8 => "[u8; 4]",
        mr_core::lighting::PixelFormat::Rgba16F => "backend_cpu::Half4",
    };
    let entry = if ck.find(Role::Draw).is_some() {
        // The raster lane's shading is the reference model's (tone and shadows read at run time).
        consts.clear();
        format!("backend_cpu::raster_entry::<Baked, {px}>()")
    } else if ck.find(Role::Visibility).is_some_and(|v| ck.node(v).component == "columns") {
        format!("backend_cpu::columns_entry::<Baked, {px}>()")
    } else {
        // Ray casting: the primary walk, the strategy's kernel and its storage.
        let kernel = ck.input(Role::Visibility, "primary").context("no primary rays")?;
        let k = ck.node(kernel);
        let storage = ck.node(k.inputs.get("storage").context("the kernel walks no storage")?).component.clone();
        let ty = backend_cpu::storage_type(&storage).with_context(|| format!("storage `{storage}` has no CPU implementation"))?;
        let knob = |n: &str| k.knobs.get(n);
        let on = |n: &str| knob(n).and_then(|v| v.as_bool()).unwrap_or(true);
        let traversal = backend_cpu::traversal(&k.component, on("exact"))?;
        consts.push(format!("const TRAVERSAL: Option<backend_cpu::Traversal> = Some(backend_cpu::Traversal::{traversal:?});"));
        if let Some(p) = knob("packet") {
            let n: usize = p.as_str().and_then(|p| p.parse().ok()).context("packet: a width")?;
            consts.push(format!("const PACKET: Option<usize> = Some({n});"));
        }
        if traversal != backend_cpu::Traversal::Refloor {
            consts.push(format!("const LEVELS: Option<u32> = Some({:#b});", backend_cpu::level_mask(["skip2", "skip4", "skip8", "skip16"].map(on))));
        }
        let shadow = backend_cpu::shadow_kernel(&ck)?.map(|(t, l)| format!("Some((backend_cpu::Traversal::{t:?}, {l:#b}))"));
        consts.push(format!("const SHADOW: Option<Option<(backend_cpu::Traversal, u32)>> = Some({});", shadow.as_deref().unwrap_or("None")));
        baked.insert(0, ("storage".into(), storage.as_str().into()));
        baked.insert(1, ("traversal".into(), k.component.as_str().into()));
        format!("backend_cpu::raycast_entry::<{ty}, Baked, {px}>()")
    };
    let knobs = baked.iter().map(|(k, v)| Ok(format!("({k:?}, {})", rust(v)?))).collect::<anyhow::Result<Vec<_>>>()?;
    consts.insert(0, format!("const KNOBS: &'static [(&'static str, &'static str)] = &[{}];", knobs.join(", ")));
    consts.push("const NATIVE: bool = true;".into());
    Ok(Some(Bake { graph, executor, entry, consts }))
}

/// The generated crate's `src/main.rs`.
pub fn source(b: &Bake) -> anyhow::Result<String> {
    let consts: String = b.consts.iter().map(|c| format!("    {c}\n")).collect();
    let (_, _, devices) = b.executor.parts();
    Ok(format!(
        "// Generated by multirenderer: an executor compiled for one graph.\n\
         struct Baked;\n\
         impl {spec} for Baked {{\n{consts}}}\n\
         \n\
         fn main() -> anyhow::Result<()> {{\n\
         \x20   let socket = mr_serve::socket_arg(std::env::args().skip(1))?;\n\
         \x20   mr_serve::run(&socket, vec![{entry}], {devices})\n\
         }}\n",
        spec = b.executor.spec(),
        entry = b.entry,
    ))
}

/// The repository the generated crates compile against (this build's sources).
fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn manifest(repo: &Path, executor: Executor) -> String {
    let dep = |name: &str, path: &str| format!("{name} = {{ path = {} }}\n", toml::Value::String(repo.join(path).to_string_lossy().into_owned()));
    let (name, path, _) = executor.parts();
    let gpu = if executor == Executor::Gpu { dep("mr-gpu", "crates/mr-gpu") } else { String::new() };
    format!(
        "[package]\nname = \"mr-pipeline\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\n{}{gpu}{}{}{}\n\
         [profile.release]\ndebug = \"line-tables-only\"\ncodegen-units = 16\n\n\
         [profile.break-in]\ninherits = \"release\"\nlto = \"fat\"\ncodegen-units = 1\n",
        dep(name, path),
        dep("mr-serve", "crates/mr-serve"),
        dep("mr-components", "components"),
        "anyhow = \"1\"\n",
    )
}

use mr_core::cache_dir;

/// FNV-1a: stable across runs and builds.
fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// A hash over every source file the generated crate of `executor` compiles against (and the
/// lock file).
fn sources_hash(repo: &Path, executor: Executor) -> anyhow::Result<u64> {
    let mut files = vec![repo.join("Cargo.lock"), repo.join("Cargo.toml"), repo.join(".cargo/config.toml")];
    for dir in [executor.parts().1, "components", "crates", "renderers/cpu-reference"] {
        collect(&repo.join(dir), &mut files)?;
    }
    files.sort();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for f in files {
        h = fnv(h, f.strip_prefix(repo).unwrap_or(&f).to_string_lossy().as_bytes());
        h = fnv(h, &std::fs::read(&f).unwrap_or_default());
    }
    Ok(h)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let p = e?.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                collect(&p, out)?;
            }
        } else if p.extension().is_some_and(|x| x == "rs" || x == "toml" || x == "wgsl") {
            out.push(p);
        }
    }
    Ok(())
}

/// The toolchain and CPU parts of the cache key.
fn machine_id() -> anyhow::Result<String> {
    let rustc = Command::new(std::env::var("RUSTC").unwrap_or("rustc".into())).arg("-vV").output().context("running rustc -vV (a Rust toolchain builds the pipelines)")?;
    let cpu = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model: Vec<&str> = cpu.lines().filter(|l| l.starts_with("model name") || l.starts_with("flags")).take(2).collect();
    Ok(format!("{}\n{}", String::from_utf8_lossy(&rustc.stdout), model.join("\n")))
}

/// The cache key of `b` built at `level` on this machine and source state: the pipeline's
/// graph key, then a hash of what builds it here (`-break-in` after it for that level).
pub fn key(b: &Bake, level: Compile) -> anyhow::Result<String> {
    let mut h = fnv(0xcbf2_9ce4_8422_2325, source(b)?.as_bytes());
    h = fnv(h, manifest(repo(), b.executor).as_bytes());
    h = fnv(h, &sources_hash(repo(), b.executor)?.to_le_bytes());
    h = fnv(h, machine_id()?.as_bytes());
    h = fnv(h, FLAGS.as_bytes());
    if level == Compile::BreakIn {
        h = fnv(h, format!("{BREAK_IN:?}").as_bytes());
    }
    let prefix = match b.executor {
        Executor::Cpu => "cpu",
        Executor::Gpu => "gpu",
    };
    let suffix = if level == Compile::BreakIn { "-break-in" } else { "" };
    Ok(format!("{prefix}-{}-{h:016x}{suffix}", b.graph.key().map_err(anyhow::Error::msg)?))
}

/// The same forced code alignment as the workspace (`.cargo/config.toml`), plus this CPU.
const FLAGS: &str = "-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=5 -C target-cpu=native";

/// Builds the crate in `src` with `profile` into `target` (`RUSTFLAGS` = [`FLAGS`] + `extra`)
/// and returns the worker executable.
fn cargo(key: &str, src: &Path, target: &Path, profile: &str, extra: &str) -> anyhow::Result<PathBuf> {
    tracing::info!("compiling pipeline {key} (cargo, profile {profile}, target {})", target.display());
    let out = Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
        .args(["build", "--quiet", "--profile", profile, "--manifest-path"])
        .arg(src.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", target)
        .env("RUSTFLAGS", format!("{FLAGS} {extra}"))
        .output()
        .context("running cargo (a Rust toolchain builds the pipelines)")?;
    if !out.status.success() {
        bail!("building pipeline {key} ({profile}) failed:\n{}", tail(&out.stderr, 30));
    }
    Ok(target.join(profile).join(exe_name("mr-pipeline")))
}

/// `name` as an executable's file name on this system.
fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// The last `n` lines of a child's output.
fn tail(out: &[u8], n: usize) -> String {
    let s = String::from_utf8_lossy(out);
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// The toolchain's `llvm-profdata` (rustup component `llvm-tools`: its LLVM is rustc's).
fn profdata() -> anyhow::Result<PathBuf> {
    let rustc = std::env::var("RUSTC").unwrap_or("rustc".into());
    let sysroot = Command::new(&rustc).args(["--print", "sysroot"]).output().context("running rustc --print sysroot")?;
    let vv = Command::new(&rustc).arg("-vV").output().context("running rustc -vV")?;
    let vv = String::from_utf8_lossy(&vv.stdout);
    let host = vv.lines().find_map(|l| l.strip_prefix("host: ")).context("rustc -vV names no host")?;
    let exe = PathBuf::from(String::from_utf8_lossy(&sysroot.stdout).trim()).join("lib").join("rustlib").join(host).join("bin").join(exe_name("llvm-profdata"));
    if !exe.exists() {
        bail!("break-in needs llvm-profdata ({} is missing): rustup component add llvm-tools", exe.display());
    }
    Ok(exe)
}

/// The training run's knobs: one thread where the executor has the knob. The profile's
/// counters are shared, and threads writing them take turns at the cache lines (measured,
/// bricks + refloor, 960x540: 71 ms a frame on one thread, 547 on four, 850 on all 32).
fn train_options(b: &Bake) -> toml::Table {
    let mut options = toml::Table::new();
    if params_of(&b.graph.executor).is_some_and(|(_, ps)| ps.iter().any(|p| p.name == "threads")) {
        options.insert("threads".into(), 1.into());
    }
    options
}

/// Break-in's candidates: name, target directory, cargo profile, profile-guided.
const BREAK_IN: &[(&str, &str, &str, bool)] = &[
    ("native", "target", "release", false),
    ("lto", "target-lto", "break-in", false),
    ("pgo", "target-pgo-use", "release", true),
    ("lto+pgo", "target-break-in", "break-in", true),
];

/// Flies `b`'s graph (lit and unlit, `frames` frames, 960x540) under `dir` with worker `exe`
/// (and `env`) and returns the run's report.
fn fly(b: &Bake, dir: &Path, exe: &Path, options: toml::Table, frames: u32, env: &[(&str, &std::ffi::OsStr)]) -> anyhow::Result<serde_json::Value> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("plan.toml"), check_plan(b, &dir.join("run"), &["off"], options, frames, [960, 540], false)?)?;
    let mut cmd = Command::new(host_exe()?);
    cmd.arg("run").arg(dir.join("plan.toml")).env("MR_PIPELINE_CHECK", "1").env("MR_WORKER_EXE", exe);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        bail!("the run in {} failed:\n{}", dir.display(), tail(&out.stderr, 20));
    }
    let run = std::fs::read_dir(dir.join("run"))?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.join("report.json").exists()).context("the run wrote no report")?;
    Ok(serde_json::from_slice(&std::fs::read(run.join("report.json"))?)?)
}

/// The break-in builds of the crate in `src`, each timed here, the fastest kept: the native
/// build, whole-program LTO (one codegen unit), profile-guided (an instrumented build flies a
/// training run, `llvm-profdata` merges the profile), and both. Which wins depends on the
/// pipeline (measured: LTO alone +4-8 % for raster and host-bound gpu-compute, where the
/// profile costs 4 %; the profile alone +8-10 % for prefix + incremental, where LTO costs
/// 2-4 %; both together lost to the better one alone in each).
fn break_in(b: &Bake, key: &str, src: &Path, root: &Path) -> anyhow::Result<PathBuf> {
    let profdata = profdata()?;
    let train = root.join("train").join(key);
    let _ = std::fs::remove_dir_all(&train);
    std::fs::create_dir_all(&train)?;
    // The instrumented build: one fixed profile path for every pipeline (the training run
    // names its own files), so the shared target keeps its dependencies.
    let scratch = root.join("pgo-scratch");
    let instrumented = cargo(key, src, &root.join("target-pgo"), "release", &format!("-C profile-generate={}", scratch.display()))?;
    let t = Instant::now();
    let profiles = train.join("profiles");
    std::fs::create_dir_all(&profiles)?;
    fly(b, &train.join("train"), &instrumented, train_options(b), 90, &[("LLVM_PROFILE_FILE", profiles.join("%p-%m.profraw").as_os_str())])?;
    let raw: Vec<PathBuf> = std::fs::read_dir(&profiles).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    if raw.is_empty() {
        bail!("the training run wrote no profile");
    }
    let merged = train.join("merged.profdata");
    let out = Command::new(profdata).arg("merge").arg("-o").arg(&merged).args(&raw).output().context("running llvm-profdata")?;
    if !out.status.success() {
        bail!("llvm-profdata merge failed:\n{}", tail(&out.stderr, 20));
    }
    tracing::info!("pipeline {key}: trained in {:.1} s ({} profiles)", t.elapsed().as_secs_f64(), raw.len());
    // The candidates, each copied out of its target (the targets are shared).
    let use_profile = format!("-C profile-use={}", merged.display());
    let mut candidates = vec![];
    for &(name, target, profile, pgo) in BREAK_IN {
        let exe = train.join(exe_name(&format!("worker-{name}")));
        if pgo {
            // The profile's path is in the flags, so every dependency is built anew for this
            // pipeline (1.5 GB); nothing else can use the target.
            let target = root.join(target).join(key);
            let built = cargo(key, src, &target, profile, &use_profile);
            let copied = built.and_then(|b| Ok(std::fs::copy(b, &exe)?));
            let _ = std::fs::remove_dir_all(&target);
            copied?;
        } else {
            std::fs::copy(cargo(key, src, &root.join(target), profile, "")?, &exe)?;
        }
        candidates.push((name, exe));
    }
    // Timed in interleaved rounds, all threads: the mean frame time over lit and unlit.
    let t = Instant::now();
    let mut ms: Vec<Vec<f64>> = vec![vec![]; candidates.len()];
    for round in 0..3 {
        for (i, (name, exe)) in candidates.iter().enumerate() {
            let report = fly(b, &train.join(format!("time-{name}-{round}")), exe, toml::Table::new(), 150, &[])?;
            let passes = report.as_array().context("a report is a list of passes")?;
            let fps: Vec<f64> = passes.iter().filter_map(|p| p["fps"].as_f64()).collect();
            if fps.is_empty() || fps.iter().any(|&f| f <= 0.0) {
                bail!("timing {name}: no frame rate in its report");
            }
            ms[i].push(fps.iter().map(|f| 1e3 / f).sum::<f64>() / fps.len() as f64);
        }
    }
    let median = |v: &[f64]| {
        let mut v = v.to_vec();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let times: Vec<f64> = ms.iter().map(|v| median(v)).collect();
    let best = (0..times.len()).min_by(|&a, &b| times[a].total_cmp(&times[b])).unwrap();
    let table: Vec<String> = candidates.iter().zip(&times).map(|((n, _), t)| format!("{n} {t:.3} ms")).collect();
    let summary = format!("chose {} ({}; median of 3 rounds, timed in {:.1} s)", candidates[best].0, table.join(", "), t.elapsed().as_secs_f64());
    tracing::info!("pipeline {key}: {summary}");
    std::fs::write(train.join("chosen.txt"), &summary)?;
    // Kept: the choice and the profile; the runs and the other candidates go.
    let chosen = train.join(exe_name("worker"));
    std::fs::rename(&candidates[best].1, &chosen)?;
    for e in std::fs::read_dir(&train)?.filter_map(|e| e.ok()).map(|e| e.path()) {
        if e.is_dir() {
            let _ = std::fs::remove_dir_all(&e);
        } else if e.file_name().is_some_and(|n| n.to_string_lossy().starts_with("worker-")) {
            let _ = std::fs::remove_file(&e);
        }
    }
    Ok(chosen)
}

/// Returns the worker for `b` at `level`, building it first if it is not cached (blocking;
/// native: minutes the first time, while the shared target directory fills, seconds after;
/// break-in: minutes every time).
pub fn ensure(b: &Bake, level: Compile) -> anyhow::Result<Compiled> {
    let key = key(b, level)?;
    let root = cache_dir();
    let exe = root.join("bin").join(exe_name(&key));
    if exe.exists() {
        // Its use, for the cache's least-recently-used order.
        let _ = std::fs::File::options().write(true).open(&exe).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        return Ok(Compiled { exe, key, build_s: None });
    }
    let t = Instant::now();
    let src = root.join("src").join(&key);
    std::fs::create_dir_all(src.join("src"))?;
    std::fs::write(src.join("Cargo.toml"), manifest(repo(), b.executor))?;
    std::fs::write(src.join("src/main.rs"), source(b)?)?;
    // The workspace's lock file, so the generated crate builds the same dependency versions.
    std::fs::copy(repo().join("Cargo.lock"), src.join("Cargo.lock"))?;
    let built = match level {
        Compile::BreakIn => break_in(b, &key, &src, &root)?,
        _ => cargo(&key, &src, &root.join("target"), "release", "")?,
    };
    std::fs::create_dir_all(root.join("bin"))?;
    // Copy, then rename: a crash mid-copy never leaves a truncated worker under the key.
    let tmp = root.join("bin").join(format!(".{key}.tmp"));
    std::fs::copy(&built, &tmp)?;
    if level == Compile::BreakIn {
        // The chosen candidate, out of its training directory.
        let _ = std::fs::remove_file(&built);
    }
    std::fs::rename(&tmp, &exe)?;
    let build_s = t.elapsed().as_secs_f64();
    tracing::info!("compiled pipeline {key} in {build_s:.1} s");
    prune(&root, &key, cache_limit());
    Ok(Compiled { exe, key, build_s: Some(build_s) })
}

/// The cache's size limit: `$MR_PIPELINE_CACHE_GB`, else 4 GB.
fn cache_limit() -> u64 {
    let gb = std::env::var("MR_PIPELINE_CACHE_GB").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(4.0);
    (gb * 1e9) as u64
}

/// The bytes under `p` (0 if it is missing).
pub fn size(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map(|d| d.filter_map(|e| e.ok()).map(|e| size(&e.path())).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// Removes worker `key` and everything kept for it.
fn forget(root: &Path, key: &str) {
    let exe = root.join("bin").join(exe_name(key));
    for f in [exe.with_extension("trusted"), exe.with_extension("rejected"), exe] {
        let _ = std::fs::remove_file(f);
    }
    for d in ["src", "train", "check"] {
        let _ = std::fs::remove_dir_all(root.join(d).join(key));
    }
}

/// Keeps the cache bounded after `key` was built: other builds of the same pipeline and
/// level (older sources, toolchain or CPU: never asked for again) go; then, while the cache
/// is over `limit` bytes, the least recently used workers, and last the shared target
/// directories (the next build fills them again).
fn prune(root: &Path, key: &str, limit: u64) {
    let level = |k: &str| k.ends_with("-break-in");
    let pipeline = |k: &str| k.trim_end_matches("-break-in").rsplit_once('-').map(|(p, _)| p.to_string());
    // Workers: the files without an extension (`.exe` on Windows); not `.trusted` and the like.
    let mut workers: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(root.join("bin"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_none_or(|x| x == "exe"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path().file_stem()?.to_string_lossy().into_owned())))
        .filter(|(_, k)| !k.starts_with('.'))
        .collect();
    workers.retain(|(_, k)| {
        let stale = k != key && level(k) == level(key) && pipeline(k) == pipeline(key);
        if stale {
            tracing::info!("pipeline cache: {k} replaced by {key}");
            forget(root, k);
        }
        !stale
    });
    workers.sort();
    let mut total = size(root);
    for (_, k) in workers.iter().filter(|(_, k)| k != key) {
        if total <= limit {
            return;
        }
        tracing::info!("pipeline cache over {:.1} GB: {k} removed (least recently used)", limit as f64 / 1e9);
        forget(root, k);
        total = size(root);
    }
    for d in ["target-pgo", "target-lto", "target-break-in", "target-pgo-use", "target"] {
        if total <= limit {
            return;
        }
        tracing::info!("pipeline cache over {:.1} GB: {d} removed", limit as f64 / 1e9);
        let _ = std::fs::remove_dir_all(root.join(d));
        total = size(root);
    }
}

/// Returns the worker for `b` once it is trusted: a freshly built pipeline must render
/// the same pixels as the generic build of the same composition ([`verify`]) before it is
/// used; the result is kept next to the worker (`.trusted`, or `.rejected` with the
/// reason, which fails every later request until the sources or toolchain change).
pub fn trusted(b: &Bake, level: Compile) -> anyhow::Result<Compiled> {
    let c = ensure(b, level)?;
    // The check itself runs the pipeline: no check inside the check.
    if std::env::var_os("MR_PIPELINE_CHECK").is_some() {
        return Ok(c);
    }
    let ok = c.exe.with_extension("trusted");
    let bad = c.exe.with_extension("rejected");
    if ok.exists() {
        return Ok(c);
    }
    if let Ok(why) = std::fs::read_to_string(&bad) {
        bail!("pipeline {} was rejected: {why}", c.key);
    }
    let t = Instant::now();
    match verify(b, &c, level) {
        Ok(summary) => {
            // The check's frames are not needed once it passed (a failure keeps them).
            let _ = std::fs::remove_dir_all(cache_dir().join("check").join(&c.key));
            tracing::info!("pipeline {} trusted in {:.1} s: {summary}", c.key, t.elapsed().as_secs_f64());
            std::fs::write(&ok, summary)?;
            Ok(c)
        }
        Err(e) => {
            std::fs::write(&bad, format!("{e:#}"))?;
            Err(e.context(format!("pipeline {} differs from the generic build; not used", c.key)))
        }
    }
}

/// This executable (`MR_WORKER_EXE` where the current one is a test harness, as for workers).
fn host_exe() -> anyhow::Result<PathBuf> {
    Ok(std::env::var_os("MR_WORKER_EXE").map(PathBuf::from).map_or_else(std::env::current_exe, Ok)?)
}

/// A terrain flight (lit and unlit) of `b`'s graph under `dir`, one lane per `compile` level
/// with knobs `options`; with `save`, every frame is saved (the check).
fn check_plan(b: &Bake, dir: &Path, lanes: &[&str], options: toml::Table, frames: u32, resolution: [u32; 2], save: bool) -> anyhow::Result<String> {
    let lane = |compile: &&str| -> anyhow::Result<toml::Value> {
        let mut t = toml::Table::new();
        t.insert("graph".into(), toml::Value::try_from(&b.graph)?);
        t.insert("compile".into(), (*compile).into());
        t.insert("options".into(), options.clone().into());
        Ok(toml::Value::Table(t))
    };
    let mut t = toml::Table::new();
    t.insert("renderer".into(), toml::Value::Array(lanes.iter().map(lane).collect::<anyhow::Result<_>>()?));
    let renderers = toml::to_string(&t)?;
    let save = if save { "save = { every = 1 }" } else { "" };
    Ok(format!(
        "name = \"pipeline-check\"\n\n[scene]\ngenerator = {{ type = \"terrain\", seed = 7 }}\n\
         camera = {{ type = \"flythrough\", points = [[0, 110, 0], [260, 140, 120], [520, 95, -80]] }}\n\
         lights = [{{ type = \"scatter\", count = 40, seed = 1, center = [260, 80, 0], extent = [300, 60, 200], on_surface = true, intensity = 25.0, radius = 18.0 }}]\n\n\
         [run]\nframes = {frames}\nwarmup = 1\nresolution = {resolution:?}\nmodes = [\"unlit\", \"lit\"]\nview_radius = 6\n\n\
         [output]\ndir = {:?}\n{save}\n\n{renderers}",
        dir,
    ))
}

/// Renders a short terrain flight (lit and unlit) with the generic build and the compiled
/// pipeline `c` (built at `level`), in a child process of this executable, and compares every
/// saved frame pixel by pixel.
fn verify(b: &Bake, c: &Compiled, level: Compile) -> anyhow::Result<String> {
    let dir = cache_dir().join("check").join(&c.key);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    // The pipeline twice, as its graph: the generic build and the compiled one.
    let plan = check_plan(b, &dir.join("run"), &["off", level.name()], toml::Table::new(), 4, [480, 270], true)?;
    std::fs::write(dir.join("check.toml"), &plan)?;
    let out = Command::new(host_exe()?).arg("run").arg(dir.join("check.toml")).env("MR_PIPELINE_CHECK", "1").output()?;
    if !out.status.success() {
        bail!("check run failed:\n{}", tail(&out.stderr, 20));
    }
    // frames/<label>-<mode>/<frame>.png: the generic lane and the compiled one (`<label> [<level>]`).
    let marker = format!("__{}_", level.name());
    let run = std::fs::read_dir(dir.join("run"))?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.join("frames").is_dir()).context("check run saved no frames")?;
    let lanes: Vec<PathBuf> = std::fs::read_dir(run.join("frames"))?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    let (mut frames, mut differ) = (0, vec![]);
    for native in lanes.iter().filter(|p| p.to_string_lossy().contains(&marker)) {
        let name = native.file_name().unwrap().to_string_lossy().replace(&marker, "");
        let generic = run.join("frames").join(&name);
        for f in std::fs::read_dir(native)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| !p.to_string_lossy().ends_with("-diff.png")) {
            let a = image::open(&f)?.to_rgb8();
            let g = image::open(generic.join(f.file_name().unwrap())).with_context(|| format!("generic frame for {}", f.display()))?.to_rgb8();
            frames += 1;
            let n = a.pixels().zip(g.pixels()).filter(|(x, y)| x != y).count();
            if n > 0 || a.dimensions() != g.dimensions() {
                differ.push(format!("{}: {n} pixels differ", f.strip_prefix(&run).unwrap_or(&f).display()));
            }
        }
    }
    if frames == 0 {
        bail!("the check compared no frames (lanes: {lanes:?})");
    }
    if !differ.is_empty() {
        let more = if differ.len() > 4 { format!("; ... ({} more)", differ.len() - 4) } else { String::new() };
        bail!("{} of {frames} frames differ: {}{more}", differ.len(), differ[..differ.len().min(4)].join("; "));
    }
    Ok(format!("{frames} frames pixel-identical to the generic build"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Older builds of the same pipeline and level go at once; over the limit, the least
    /// recently used workers, then the shared targets; the new worker always stays.
    #[test]
    fn the_cache_stays_bounded() {
        let root = std::env::temp_dir().join(format!("mr-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("target/lib"), [0; 1000]).unwrap();
        let t0 = std::time::SystemTime::now() - std::time::Duration::from_secs(1000);
        let worker = |k: &str, age: u64| {
            let p = root.join("bin").join(exe_name(k));
            std::fs::write(&p, [0; 1000]).unwrap();
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t0 + std::time::Duration::from_secs(age)).unwrap();
            std::fs::write(p.with_extension("trusted"), "").unwrap();
        };
        let (old, new) = ("cpu-aaaa-1111", "cpu-aaaa-2222");
        worker(old, 0);
        worker("cpu-aaaa-1111-break-in", 1);
        worker("cpu-bbbb-1111", 2);
        worker("cpu-cccc-1111", 3);
        worker(new, 4);
        let has = |k: &str| root.join("bin").join(exe_name(k)).exists();
        prune(&root, new, 100_000);
        assert!(!has(old) && !root.join("bin").join(old).with_extension("trusted").exists(), "the older build of the same pipeline went");
        assert!(has("cpu-aaaa-1111-break-in") && has("cpu-bbbb-1111") && has("cpu-cccc-1111") && has(new));
        prune(&root, new, 3_000);
        assert!(!has("cpu-aaaa-1111-break-in") && !has("cpu-bbbb-1111") && has("cpu-cccc-1111") && has(new), "least recently used first");
        assert!(root.join("target").exists());
        prune(&root, new, 1_500);
        assert!(!has("cpu-cccc-1111") && has(new) && !root.join("target").exists(), "then the target; never the new worker");
        let _ = std::fs::remove_dir_all(&root);
    }

    fn spec(r: &str, options: &[(&str, toml::Value)]) -> RendererSpec {
        let mut s = RendererSpec::from_cli(r).unwrap();
        s.options.extend(options.iter().map(|(k, v)| (k.to_string(), v.clone())));
        s.resolve().unwrap();
        s
    }

    /// Every CPU storage and kernel of the `cpu` back end and every other lane of the CPU
    /// executor has generated code; the knobs it bakes are declared specialisable.
    #[test]
    fn every_cpu_graph_has_a_source() {
        for st in mr_components::names_on("cpu", mr_components::Slot::Storage) {
            for k in mr_components::names_on("cpu", mr_components::Slot::Traversal) {
                let b = bake(&spec(&format!("cpu:{st}+{k}"), &[("packet", "4".into())])).unwrap().unwrap();
                let s = source(&b).unwrap();
                assert!(s.contains("NATIVE: bool = true") && s.contains(&format!("raycast_entry::<{}, Baked, [u8; 4]>", backend_cpu::storage_type(st).unwrap())), "{s}");
                assert_eq!(s.contains("PACKET: Option<usize> = Some(4)"), k == "refloor", "{s}");
                assert_eq!(s.contains("LEVELS"), k != "refloor", "{s}");
                assert!(s.contains(&format!("(\"storage\", \"\\\"{st}\\\"\")")), "{s}");
            }
        }
        let s = source(&bake(&spec("cpu:prefix+incremental,shadow=refloor,post=aces", &[("shadows", "sun".into()), ("skip4", false.into())])).unwrap().unwrap()).unwrap();
        for want in ["SHADOW: Option<Option<(backend_cpu::Traversal, u32)>> = Some(Some((backend_cpu::Traversal::Refloor, 0b111111)))", "LEVELS: Option<u32> = Some(0b111011)", "Tone::Aces", "Shadows::Sun", "(\"skip4\", \"false\")"] {
            assert!(s.contains(want), "{want} in {s}");
        }
        let s = source(&bake(&spec("cpu-voxelspace", &[])).unwrap().unwrap()).unwrap();
        assert!(s.contains("columns_entry::<Baked, [u8; 4]>") && s.contains("Shadows::All") && s.contains("Tone::Clamp"), "{s}");
        let s = source(&bake(&spec("cpu-raster:naive+raster", &[])).unwrap().unwrap()).unwrap();
        assert!(s.contains("raster_entry::<Baked, [u8; 4]>") && s.contains("KNOBS: &'static [(&'static str, &'static str)] = &[];") && !s.contains("TONE"), "{s}");
        // `hdr`: every lane writes float pixels.
        for (r, entry) in [("cpu,post=hdr", "raycast_entry::<mr_components::storage::bricks::cpu::U16, Baked, backend_cpu::Half4>"), ("cpu-voxelspace,post=hdr", "columns_entry::<Baked, backend_cpu::Half4>"), ("cpu-raster,post=hdr", "raster_entry::<Baked, backend_cpu::Half4>")] {
            let s = source(&bake(&spec(r, &[])).unwrap().unwrap()).unwrap();
            assert!(s.contains(entry), "{entry} in {s}");
        }
        for b in ["cpu", "cpu-raster", "cpu-voxelspace"] {
            let (_, params) = params_of(b).unwrap();
            for k in bakes(b) {
                assert!(params.iter().any(|p| p.name == *k && p.baked && p.owner.is_some()), "{b}: `{k}` is declared specialisable and belongs to a node");
            }
        }
        assert!(manifest(repo(), Executor::Cpu).contains("[workspace]"), "the generated crate is its own workspace");
    }

    /// Every GPU-executor graph has a generated worker, built for this CPU; discovery on the
    /// CPU walks the graph's storage alone, baked (refused otherwise).
    #[test]
    fn every_gpu_graph_has_a_source() {
        for b in ["gpu-raster", "gpu-discover", "cpu-discover", "gpu-compute", "gpu-rt", "gpu-splat", "gpu-voxelspace"] {
            let bk = bake(&spec(b, &[])).unwrap().unwrap();
            assert_eq!(bk.executor, Executor::Gpu, "{b}");
            let s = source(&bk).unwrap();
            for want in ["impl backend_gpu::Spec for Baked", &format!("BACKEND: &'static str = {b:?}"), "backend_gpu::compiled_entry::<Baked>()", "mr_gpu::devices", "NATIVE: bool = true"] {
                assert!(s.contains(want), "{b}: {want} in {s}");
            }
            assert_eq!(s.contains("CPU_WORLD"), b == "cpu-discover", "{s}");
        }
        let prefix = RendererSpec::composed(mr_components::Composition { storage: "prefix".into(), ..mr_components::Composition::new("cpu-discover") });
        let s = source(&bake(&prefix).unwrap().unwrap()).unwrap();
        assert!(s.contains("CpuWorld::of::<mr_components::storage::prefix::cpu::Prefix>") && s.contains("(\"storage\", \"\\\"prefix\\\"\")"), "{s}");
        let m = manifest(repo(), Executor::Gpu);
        assert!(m.contains("backend-gpu") && m.contains("mr-gpu") && !m.contains("backend-cpu"), "{m}");
        let key = |r: &str, o: &[(&str, toml::Value)]| self::key(&bake(&spec(r, o)).unwrap().unwrap(), Compile::Native).unwrap();
        assert!(key("gpu-compute", &[]).starts_with("gpu-"));
        assert_ne!(self::key(&bake(&prefix).unwrap().unwrap(), Compile::Native).unwrap(), key("cpu-discover", &[]));
    }

    /// Break-in is a level of the same pipeline: its own worker under the same graph key, its training run on one thread where the executor has the knob.
    #[test]
    fn break_in_is_its_own_build() {
        let b = bake(&spec("cpu", &[])).unwrap().unwrap();
        let (native, broken) = (key(&b, Compile::Native).unwrap(), key(&b, Compile::BreakIn).unwrap());
        assert!(broken.ends_with("-break-in") && broken != format!("{native}-break-in"), "its recipe in the hash: {broken}");
        assert_eq!(broken.split('-').nth(1), native.split('-').nth(1), "the same graph");
        let m = manifest(repo(), Executor::Cpu);
        assert!(m.contains("[profile.break-in]\ninherits = \"release\"\nlto = \"fat\"\ncodegen-units = 1"), "{m}");
        let p: crate::plan::Plan = crate::plan::Plan::parse("[scene]\ngenerator = { type = \"terrain\" }\ncamera = { type = \"line\", from = [0, 1, 0], to = [1, 1, 0] }\n[run]\ncompile = \"break-in\"\n[[renderer]]\ncompose = { backend = \"cpu\" }\n", &[]).unwrap();
        assert_eq!(p.renderers[0].compiled(&p.run), Some(Compile::BreakIn));
        let plan = check_plan(&b, Path::new("/x"), &["off"], train_options(&b), 90, [960, 540], false).unwrap();
        assert!(plan.contains("threads = 1") && !plan.contains("save"), "{plan}");
        assert!(train_options(&bake(&spec("gpu-compute", &[])).unwrap().unwrap()).is_empty(), "no threads knob");
    }

    /// The cache key is the pipeline's graph with its baked values: knobs it does not use
    /// leave it alone, and the check run's plan (the graph twice) bakes the same pipeline.
    #[test]
    fn the_graph_is_the_key() {
        let key = |r: &str, o: &[(&str, toml::Value)]| bake(&spec(r, o)).unwrap().unwrap().graph.key().unwrap();
        assert_ne!(key("cpu:prefix+refloor", &[("packet", "4".into())]), key("cpu:prefix+refloor", &[("packet", "8".into())]));
        assert_eq!(key("cpu:prefix+incremental", &[("packet", "4".into())]), key("cpu:prefix+incremental", &[("packet", "8".into())]), "packet is moot for incremental");
        assert_ne!(key("cpu:prefix+incremental", &[("exact", false.into())]), key("cpu:prefix+incremental", &[]));
        assert_eq!(key("cpu:prefix+refloor", &[("exact", false.into())]), key("cpu:prefix+refloor", &[]), "exact is moot for refloor");
        assert_eq!(key("cpu:prefix+refloor", &[("view", "steps".into())]), key("cpu:prefix+refloor", &[]), "not baked by the generated code");
        assert_ne!(key("cpu:prefix+refloor", &[]), key("cpu:bricks+refloor", &[]));
        assert_ne!(key("cpu:prefix+refloor", &[]), key("cpu:prefix+refloor,shadow=incremental", &[]), "the shadow rays' own kernel");
        assert_ne!(key("cpu:prefix+incremental", &[("skip8", false.into())]), key("cpu:prefix+incremental", &[]));
        assert_eq!(key("cpu:prefix+refloor", &[("skip8", false.into())]), key("cpu:prefix+refloor", &[]), "levels are moot for refloor");
        assert_ne!(key("cpu:prefix+refloor", &[("shadows", "sun".into())]), key("cpu:prefix+refloor", &[]));
        assert_ne!(key("cpu-voxelspace", &[("shadows", "none".into())]), key("cpu-voxelspace", &[]));
        // The check run's lanes: the bake's graph in a plan bakes the same pipeline.
        for (r, o) in [("cpu:hier+incremental", vec![("exact", toml::Value::from(false))]), ("cpu:bits+refloor,shadow=incremental", vec![("packet", "8".into())])] {
            let b = bake(&spec(r, &o)).unwrap().unwrap();
            let mut s = RendererSpec::new("cpu");
            s.graph = Some(b.graph.clone());
            s.resolve().unwrap();
            assert_eq!(bake(&s).unwrap().unwrap(), b, "{r}");
        }
    }
}
