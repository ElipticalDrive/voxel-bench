//! Pipelines compiled on demand (PLAN steps 8 and 16a, IDEAS §17): for one graph, generate a
//! small Rust crate whose worker serves only that graph, built with `cargo` for this machine
//! (`-C target-cpu=native`). For the CPU executor (any of its lanes: ray casting,
//! rasterisation, columns) the lane's entry is instantiated for the graph's storage type with
//! its choices baked in as constants (`backend_cpu::Spec`); for the GPU executor its worker's
//! CPU work (client, storage encoding, meshing, discovery on the CPU in the graph's storage
//! alone) is built for this CPU (`backend_gpu::Spec`). The build runs in its own process,
//! the result is an ordinary worker executable spoken to over the usual protocol, so a bad
//! build only fails its worker and the host can swap it for the generic build.
//!
//! Builds are cached by the pipeline's graph key (step 10: the renderer's graph with the
//! values of the knobs the generated code bakes placed on their nodes, [`Bake::graph`]) and
//! a hash over the generated source, the sources it compiles against, the toolchain
//! (`rustc -vV`) and the CPU (`/proc/cpuinfo` model and flags): a pipeline is compiled once
//! per machine and source state. Generated crates share one target directory, so the
//! workspace's crates are compiled once for all of them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::plan::RendererSpec;

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
    let entry = if ck.find(Role::Draw).is_some() {
        // The raster lane's shading is the reference model's (tone and shadows read at run time).
        consts.clear();
        "backend_cpu::raster_entry::<Baked>()".to_string()
    } else if ck.find(Role::Visibility).is_some_and(|v| ck.node(v).component == "columns") {
        "backend_cpu::columns_entry::<Baked>()".to_string()
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
        format!("backend_cpu::raycast_entry::<{ty}, Baked>()")
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
        "// Generated by multirenderer (PLAN steps 8 and 16a): an executor compiled for one graph.\n\
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
    let dep = |name: &str, path: &str| format!("{name} = {{ path = {:?} }}\n", repo.join(path));
    let (name, path, _) = executor.parts();
    let gpu = if executor == Executor::Gpu { dep("mr-gpu", "crates/mr-gpu") } else { String::new() };
    format!(
        "[package]\nname = \"mr-pipeline\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\n{}{gpu}{}{}{}\n\
         [profile.release]\ndebug = \"line-tables-only\"\ncodegen-units = 16\n",
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

/// The cache key of `b` on this machine and source state: the pipeline's graph key, then a
/// hash of what builds it here.
pub fn key(b: &Bake) -> anyhow::Result<String> {
    let mut h = fnv(0xcbf2_9ce4_8422_2325, source(b)?.as_bytes());
    h = fnv(h, manifest(repo(), b.executor).as_bytes());
    h = fnv(h, &sources_hash(repo(), b.executor)?.to_le_bytes());
    h = fnv(h, machine_id()?.as_bytes());
    let prefix = match b.executor {
        Executor::Cpu => "cpu",
        Executor::Gpu => "gpu",
    };
    Ok(format!("{prefix}-{}-{h:016x}", b.graph.key().map_err(anyhow::Error::msg)?))
}

/// Returns the worker for `b`, building it first if it is not cached (blocking; minutes
/// the first time, while the shared target directory fills, seconds after).
pub fn ensure(b: &Bake) -> anyhow::Result<Compiled> {
    let key = key(b)?;
    let root = cache_dir();
    let exe = root.join("bin").join(&key);
    if exe.exists() {
        return Ok(Compiled { exe, key, build_s: None });
    }
    let t = Instant::now();
    let src = root.join("src").join(&key);
    std::fs::create_dir_all(src.join("src"))?;
    std::fs::write(src.join("Cargo.toml"), manifest(repo(), b.executor))?;
    std::fs::write(src.join("src/main.rs"), source(b)?)?;
    // The workspace's lock file, so the generated crate builds the same dependency versions.
    std::fs::copy(repo().join("Cargo.lock"), src.join("Cargo.lock"))?;
    // The same forced code alignment as the workspace (`.cargo/config.toml`), plus this CPU.
    let flags = "-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=5 -C target-cpu=native";
    let target = root.join("target");
    tracing::info!("compiling pipeline {key} (cargo, target {})", target.display());
    let out = Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
        .args(["build", "--release", "--quiet", "--manifest-path"])
        .arg(src.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTFLAGS", flags)
        .output()
        .context("running cargo (a Rust toolchain builds the pipelines)")?;
    if !out.status.success() {
        bail!("building pipeline {key} failed:\n{}", String::from_utf8_lossy(&out.stderr).lines().rev().take(30).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));
    }
    std::fs::create_dir_all(root.join("bin"))?;
    // Copy, then rename: a crash mid-copy never leaves a truncated worker under the key.
    let tmp = root.join("bin").join(format!(".{key}.tmp"));
    std::fs::copy(target.join("release/mr-pipeline"), &tmp)?;
    std::fs::rename(&tmp, &exe)?;
    let build_s = t.elapsed().as_secs_f64();
    tracing::info!("compiled pipeline {key} in {build_s:.1} s");
    Ok(Compiled { exe, key, build_s: Some(build_s) })
}

/// Returns the worker for `b` once it is trusted: a freshly built pipeline must render
/// the same pixels as the generic build of the same composition ([`verify`]) before it is
/// used; the result is kept next to the worker (`.trusted`, or `.rejected` with the
/// reason, which fails every later request until the sources or toolchain change).
pub fn trusted(b: &Bake) -> anyhow::Result<Compiled> {
    let c = ensure(b)?;
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
    match verify(b, &c) {
        Ok(summary) => {
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

/// Renders a short terrain flight (lit and unlit) with the generic build and the compiled
/// pipeline `c`, in a child process of this executable, and compares every saved frame
/// pixel by pixel.
fn verify(b: &Bake, c: &Compiled) -> anyhow::Result<String> {
    let dir = cache_dir().join("check").join(&c.key);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    // The pipeline twice, as its graph: the generic build and the compiled one.
    let lane = |compile: &str| -> anyhow::Result<toml::Value> {
        let mut t = toml::Table::new();
        t.insert("graph".into(), toml::Value::try_from(&b.graph)?);
        t.insert("compile".into(), compile.into());
        Ok(toml::Value::Table(t))
    };
    let mut lanes = toml::Table::new();
    lanes.insert("renderer".into(), toml::Value::Array(vec![lane("off")?, lane("native")?]));
    let renderers = toml::to_string(&lanes)?;
    let plan = format!(
        "name = \"pipeline-check\"\n\n[scene]\ngenerator = {{ type = \"terrain\", seed = 7 }}\n\
         camera = {{ type = \"flythrough\", points = [[0, 110, 0], [260, 140, 120], [520, 95, -80]] }}\n\
         lights = [{{ type = \"scatter\", count = 40, seed = 1, center = [260, 80, 0], extent = [300, 60, 200], on_surface = true, intensity = 25.0, radius = 18.0 }}]\n\n\
         [run]\nframes = 4\nwarmup = 1\nresolution = [480, 270]\nmodes = [\"unlit\", \"lit\"]\nview_radius = 6\n\n\
         [output]\ndir = {:?}\nsave = {{ every = 1 }}\n\n{renderers}",
        dir.join("run"),
    );
    std::fs::write(dir.join("check.toml"), &plan)?;
    // This executable (`MR_WORKER_EXE` where the current one is a test harness, as for workers).
    let exe = std::env::var_os("MR_WORKER_EXE").map(PathBuf::from).map_or_else(std::env::current_exe, Ok)?;
    let out = Command::new(exe).arg("run").arg(dir.join("check.toml")).env("MR_PIPELINE_CHECK", "1").output()?;
    if !out.status.success() {
        bail!("check run failed:\n{}", String::from_utf8_lossy(&out.stderr).lines().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));
    }
    // frames/<label>-<mode>/<frame>.png: the generic lane and the `[native]` one.
    let run = std::fs::read_dir(dir.join("run"))?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.join("frames").is_dir()).context("check run saved no frames")?;
    let lanes: Vec<PathBuf> = std::fs::read_dir(run.join("frames"))?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    let (mut frames, mut differ) = (0, vec![]);
    for native in lanes.iter().filter(|p| p.to_string_lossy().contains("_native_")) {
        let name = native.file_name().unwrap().to_string_lossy().replace("__native_", "");
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
                assert!(s.contains("NATIVE: bool = true") && s.contains(&format!("raycast_entry::<{}, Baked>", backend_cpu::storage_type(st).unwrap())), "{s}");
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
        assert!(s.contains("columns_entry::<Baked>") && s.contains("Shadows::All") && s.contains("Tone::Clamp"), "{s}");
        let s = source(&bake(&spec("cpu-raster:naive+raster", &[])).unwrap().unwrap()).unwrap();
        assert!(s.contains("raster_entry::<Baked>") && s.contains("KNOBS: &'static [(&'static str, &'static str)] = &[];") && !s.contains("TONE"), "{s}");
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
        let s = source(&bake(&spec("cpu-discover", &[("storage", "prefix".into())])).unwrap().unwrap()).unwrap();
        assert!(s.contains("CpuWorld::of::<mr_components::storage::prefix::cpu::Prefix>") && s.contains("(\"storage\", \"\\\"prefix\\\"\")"), "{s}");
        let m = manifest(repo(), Executor::Gpu);
        assert!(m.contains("backend-gpu") && m.contains("mr-gpu") && !m.contains("backend-cpu"), "{m}");
        let key = |r: &str, o: &[(&str, toml::Value)]| self::key(&bake(&spec(r, o)).unwrap().unwrap()).unwrap();
        assert!(key("gpu-compute", &[]).starts_with("gpu-"));
        assert_ne!(key("cpu-discover", &[("storage", "prefix".into())]), key("cpu-discover", &[]));
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
