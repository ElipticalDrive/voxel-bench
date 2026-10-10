//! Run plans: TOML files describing what to render, with what, and where results go.
//! See `plans/` for examples. Any value can be overridden from the command line with
//! `-s dotted.path=value` (value parsed as TOML, falling back to a string).

use std::path::Path;

use anyhow::{Context, bail};
use mr_core::PassMode;
use mr_world::SceneSpec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    #[serde(default = "default_name")]
    pub name: String,
    pub scene: SceneSpec,
    #[serde(default)]
    pub run: RunSpec,
    #[serde(default, rename = "renderer")]
    pub renderers: Vec<RendererSpec>,
    #[serde(default)]
    pub output: OutputSpec,
}

fn default_name() -> String {
    "unnamed".into()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RunSpec {
    /// Measured frames (camera paths are parametrised over this length).
    pub frames: u32,
    /// Unmeasured frames rendered first at the frame-0 camera.
    pub warmup: u32,
    pub resolution: [u32; 2],
    pub modes: Vec<PassMode>,
    /// Chunks streamed around the camera (Chebyshev radius, per LOD level).
    pub view_radius: u32,
    /// Highest LOD level streamed (capped by each renderer's own maximum).
    pub max_lod: u8,
    /// Frames a renderer may have in flight before the host waits.
    pub in_flight: u32,
    pub pacing: Pacing,
    pub streaming: Streaming,
    /// Run all renderers at the same time (e.g. side by side in the window) instead of
    /// one after another. Sequential is fairer for benchmarking.
    pub parallel: bool,
    pub isolation: Isolation,
    /// Command prefix for worker processes, e.g. `["systemd-run", "--user", "--scope",
    /// "-p", "MemoryMax=8G"]` or `["nice", "-n", "5"]`.
    pub launcher: Vec<String>,
    /// Run every renderer's pass this many times, round-robin across renderers, and report
    /// medians and spread. GPU boost clocks and temperature drift between runs (notably on
    /// laptops), so single runs can differ by tens of percent.
    pub repeats: u32,
}

impl Default for RunSpec {
    fn default() -> Self {
        Self {
            frames: 300,
            warmup: 10,
            resolution: [1280, 720],
            modes: vec![PassMode::UNLIT],
            view_radius: 12,
            max_lod: 0,
            in_flight: 2,
            pacing: Pacing::Max,
            streaming: Streaming::Complete,
            parallel: false,
            isolation: Isolation::Process,
            launcher: vec![],
            repeats: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Pacing {
    /// As fast as possible (bounded by `in_flight`).
    Max,
    /// Submit at a fixed rate, measuring whether renderers keep up.
    #[serde(untagged)]
    Fps { fps: f64 },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Streaming {
    /// Every chunk in range is delivered before each frame: deterministic, comparable.
    Complete,
    /// At most `budget` chunk loads per frame, nearest first: realistic pop-in.
    #[serde(untagged)]
    Budget { budget: usize },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Isolation {
    /// Each renderer in its own process (default; crash- and resource-isolated).
    Process,
    /// Renderer on a thread of the host process (for debugging/profiling in the IDE).
    Thread,
}

/// One renderer of a plan: a composition of components (`compose`, IDEAS §12), or a
/// standalone renderer (cpu-reference) by `name`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSpec {
    /// Renderer entry to run. For compositions this is derived (the back end's renderer).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The components this renderer is assembled from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose: Option<mr_components::Composition>,
    /// Display label, to run the same renderer twice with different options.
    pub label: Option<String>,
    #[serde(default)]
    pub options: toml::Table,
    /// Overrides `run.view_radius` for this renderer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_radius: Option<u32>,
    /// Overrides `run.max_lod` for this renderer (still capped by what it supports).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lod: Option<u8>,
    /// Cache of the composition's label (derived; not part of the plan).
    #[serde(skip)]
    pub compose_label: std::sync::OnceLock<String>,
}

impl RendererSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), compose: None, label: None, options: Default::default(), view_radius: None, max_lod: None, compose_label: Default::default() }
    }

    /// A renderer assembled from components.
    pub fn composed(c: mr_components::Composition) -> Self {
        let mut s = Self::new("");
        s.compose = Some(c);
        s.resolve().expect("valid composition");
        s
    }

    /// For compositions: checks them and sets `name` (the back end's renderer) and the
    /// options that select the components.
    pub fn resolve(&mut self) -> anyhow::Result<()> {
        // `name = "<back end's renderer>"` (older plans, the CLI): the back end's
        // composition, with stages taken from the options where given.
        if self.compose.is_none() {
            if let Some(b) = mr_components::BACKENDS.iter().find(|b| b.renderer == Some(self.name.as_str())) {
                let mut c = mr_components::Composition::new(b.name);
                for slot in c.slots().collect::<Vec<_>>() {
                    if let Some(v) = self.options.get(slot.name()).and_then(|v| v.as_str()) {
                        c.set(slot, v);
                    }
                }
                self.compose = Some(c);
            }
        }
        if let Some(c) = &mut self.compose {
            let (name, opts) = c.resolve().map_err(|e| anyhow::anyhow!("renderer {}: {e}", c.label()))?;
            self.name = name.to_string();
            for (k, v) in opts {
                self.options.insert(k.into(), toml::Value::String(v));
            }
        }
        if self.name.is_empty() {
            bail!("a renderer needs `compose = {{ ... }}` or a `name`");
        }
        Ok(())
    }

    /// From the command line: `backend:slot+slot+...` for a composition (a bare back-end
    /// name gives its default composition): the back end's
    /// required slots in order, then optionally shading, prepass and meshes
    /// (`gpu-compute:prefix+incremental`, `gpu-compute:tree64+refloor+direct+beam`,
    /// `gpu-raster:bricks+greedy+visbuffer`); otherwise a renderer name (a back end's, giving its default composition, or a
    /// standalone one).
    pub fn from_cli(s: &str) -> anyhow::Result<Self> {
        use mr_components::Slot;
        let Some((backend, rest)) = s.split_once(':') else { return Ok(Self::new(s)) };
        let parts: Vec<&str> = rest.split('+').collect();
        let mut c = mr_components::Composition::new(backend);
        let all: Vec<Slot> = c.slots().collect();
        let mut order: Vec<Slot> = all.iter().copied().filter(|s| !s.optional()).collect();
        let required = order.len();
        order.extend([Slot::Shading, Slot::Prepass, Slot::Meshes].into_iter().filter(|s| all.contains(s)));
        if all.is_empty() || !(required..=order.len()).contains(&parts.len()) {
            let names = |sl: &[Slot]| sl.iter().map(|s| s.name()).collect::<Vec<_>>().join("+");
            bail!("composition `{s}` should be {backend}:{}[+{}]", names(&order[..required]), names(&order[required..]));
        }
        for (slot, name) in order.into_iter().zip(parts) {
            c.set(slot, name);
        }
        let mut spec = Self::new("");
        spec.set_compose(c)?;
        Ok(spec)
    }

    /// Replaces the composition (and the cached label) and re-resolves.
    pub fn set_compose(&mut self, c: mr_components::Composition) -> anyhow::Result<()> {
        self.compose = Some(c);
        self.compose_label = Default::default();
        self.resolve()
    }

    /// Options as the user sees them: without the keys a composition derives.
    pub fn extra_options(&self) -> toml::Table {
        let mut t = self.options.clone();
        if self.compose.is_some() {
            for k in mr_components::Slot::ALL.map(|s| s.name()) {
                t.remove(k);
            }
        }
        t
    }

    /// Display label: the explicit one, else the composition, else the renderer name.
    pub fn label(&self) -> &str {
        if let Some(l) = &self.label {
            return l;
        }
        match &self.compose {
            Some(c) => self.compose_label.get_or_init(|| c.label()),
            None => &self.name,
        }
    }

    /// Whether this renderer draws passes of mode `m`: its lighting modes from `info`, mesh
    /// support from the composition's meshes stage (standalone renderers: the `meshes` tag).
    pub fn supports(&self, info: &mr_core::RendererInfo, m: PassMode) -> bool {
        match &self.compose {
            Some(c) => info.modes.contains(&m.lighting) && c.supports(m),
            None => info.supports(m),
        }
    }

    pub fn view_radius(&self, run: &RunSpec) -> u32 {
        self.view_radius.unwrap_or(run.view_radius)
    }

    /// LOD levels to stream, given what the renderer supports.
    pub fn max_lod(&self, run: &RunSpec, supported: u8) -> u8 {
        self.max_lod.unwrap_or(run.max_lod).min(supported)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct OutputSpec {
    /// Show frames live in a window.
    pub window: bool,
    /// `auto`: read frames back only when an output needs them. `always`: every frame
    /// (frames delivered to host memory; measures read-back cost). `never`.
    pub readback: Readback,
    pub save: Option<SaveSpec>,
    pub compare: Option<CompareSpec>,
    /// Where run directories go (default `runs/`).
    pub dir: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Readback {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSpec {
    /// Save every n-th measured frame (0 = only the last).
    #[serde(default = "default_every")]
    pub every: u32,
    /// Also save amplified difference images against the reference.
    #[serde(default = "yes")]
    pub diffs: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompareSpec {
    /// Renderer producing the reference images (rendered in its own pass first).
    #[serde(default = "default_reference")]
    pub reference: String,
    #[serde(default)]
    pub reference_options: toml::Table,
    /// Compare every n-th measured frame.
    #[serde(default = "default_every")]
    pub every: u32,
}

fn default_every() -> u32 {
    60
}
fn yes() -> bool {
    true
}
fn default_reference() -> String {
    "cpu-reference".into()
}

impl Plan {
    pub fn load(path: &Path, overrides: &[String]) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text, overrides).with_context(|| format!("in plan {}", path.display()))
    }

    pub fn parse(text: &str, overrides: &[String]) -> anyhow::Result<Self> {
        let mut table: toml::Table = toml::from_str(text)?;
        for o in overrides {
            apply_override(&mut table, o)?;
        }
        let mut plan: Plan = table.try_into()?;
        for r in &mut plan.renderers {
            r.resolve()?;
        }
        if plan.run.frames == 0 {
            bail!("run.frames must be > 0");
        }
        Ok(plan)
    }
}

impl Plan {
    /// TOML of the plan. Compositions are written as their `compose` table only (the
    /// derived renderer name and component options are left out).
    pub fn to_toml(&self) -> anyhow::Result<String> {
        let mut p = self.clone();
        for r in &mut p.renderers {
            if r.compose.is_some() {
                r.name.clear();
                for k in mr_components::Slot::ALL.map(|s| s.name()) {
                    r.options.remove(k);
                }
            }
        }
        Ok(toml::to_string_pretty(&p)?)
    }
}

fn apply_override(table: &mut toml::Table, ov: &str) -> anyhow::Result<()> {
    let (path, value) = ov.split_once('=').with_context(|| format!("override `{ov}` is not key=value"))?;
    let value: toml::Value = toml::from_str::<toml::Table>(&format!("v = {value}"))
        .ok()
        .and_then(|mut t| t.remove("v"))
        .unwrap_or_else(|| toml::Value::String(value.to_string()));
    let mut parts: Vec<&str> = path.trim().split('.').collect();
    let last = parts.pop().unwrap();
    let mut cur = table;
    for p in parts {
        cur = cur
            .entry(p)
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .with_context(|| format!("override `{ov}`: `{p}` is not a table"))?;
    }
    cur.insert(last.to_string(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_overrides() {
        let text = r#"
            [scene]
            generator = { type = "flat" }
            camera = { type = "static" }
            [run]
            pacing = { fps = 60 }
            streaming = { budget = 16 }
            [[renderer]]
            name = "cpu-reference"
        "#;
        let p = Plan::parse(text, &["run.frames=5".into(), "run.modes=[\"lit\"]".into(), "output.window=true".into()]).unwrap();
        assert_eq!(p.run.frames, 5);
        assert_eq!(p.run.modes, vec![PassMode::LIT]);
        assert_eq!(p.run.pacing, Pacing::Fps { fps: 60.0 });
        assert_eq!(p.run.streaming, Streaming::Budget { budget: 16 });
        assert!(p.output.window);
    }

    #[test]
    fn shipped_plans_roundtrip() {
        for e in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/plans")).unwrap() {
            let path = e.unwrap().path();
            let plan = Plan::load(&path, &[]).unwrap();
            let text = plan.to_toml().unwrap();
            let again = Plan::parse(&text, &[]).unwrap_or_else(|e| panic!("{}: {e:#}\n{text}", path.display()));
            assert_eq!(text, again.to_toml().unwrap(), "{}", path.display());
        }
    }

    /// Compositions resolve to their back end's renderer and component options, are
    /// validated, and save as just the `compose` table.
    #[test]
    fn compositions() {
        let text = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n\n[[renderer]]\ncompose = { storage = \"prefix\", visibility = \"incremental\" }\noptions = { exact = false }\n";
        let p = Plan::parse(text, &[]).unwrap();
        let r = &p.renderers[0];
        assert_eq!(r.name, "gpu-compute");
        assert_eq!(r.options.get("storage").and_then(|v| v.as_str()), Some("prefix"));
        assert_eq!(r.options.get("visibility").and_then(|v| v.as_str()), Some("incremental"));
        assert_eq!(r.label(), "gpu-compute: prefix + incremental");
        let saved = p.to_toml().unwrap();
        assert!(!saved.contains("name = \"gpu-compute\"") && saved.contains("compose") && !saved.contains("[renderer.options]\nstorage"), "{saved}");
        assert_eq!(Plan::parse(&saved, &[]).unwrap().renderers[0].options, r.options);
        // Unsupported combinations are rejected with the reason.
        let bad = text.replace("storage = \"prefix\"", "backend = \"gpu-rt\", storage = \"prefix\"");
        assert!(Plan::parse(&bad, &[]).unwrap_err().to_string().contains("no storage slot"));
        let cpu = text.replace("storage = \"prefix\"", "backend = \"cpu\", storage = \"palette\"");
        assert_eq!(Plan::parse(&cpu, &[]).unwrap().renderers[0].name, "cpu");
        let bad = text.replace("storage = \"prefix\"", "backend = \"cpu\", storage = \"rows\"");
        assert!(Plan::parse(&bad, &[]).unwrap_err().to_string().contains("no cpu implementation"));
        let raster = text.replace("storage = \"prefix\", visibility = \"incremental\"", "backend = \"gpu-raster\", geometry = \"cubes\"");
        let r = &Plan::parse(&raster, &[]).unwrap().renderers[0];
        assert_eq!((r.name.as_str(), r.label()), ("gpu-raster", "gpu-raster: bricks + cubes + raster"));
        assert!(Plan::parse(&text.replace("visibility = \"incremental\"", "geometry = \"greedy\""), &[]).unwrap_err().to_string().contains("no geometry slot"));
        let spec = RendererSpec::from_cli("gpu-raster:hier+naive+visbuffer").unwrap();
        assert_eq!(spec.options.get("geometry").and_then(|v| v.as_str()), Some("naive"));
        assert!(RendererSpec::from_cli("gpu-raster:hier+naive").is_err());
        // Naming a back end's renderer gives its composition, stages from the options.
        let named = text.replace("compose = { storage = \"prefix\", visibility = \"incremental\" }", "name = \"gpu-discover\"").replace("options = { exact = false }", "options = { storage = \"prefix\" }");
        let r = &Plan::parse(&named, &[]).unwrap().renderers[0];
        assert_eq!(r.label(), "gpu-discover: prefix + quadtree");
        assert!(Plan::parse(&named, &[]).unwrap().to_toml().unwrap().contains("compose"));
        let spec = RendererSpec::from_cli("gpu-compute:tree64+refloor+direct+beam").unwrap();
        assert_eq!(spec.options.get("prepass").and_then(|v| v.as_str()), Some("beam"));
        let spec = RendererSpec::from_cli("gpu-compute:distance+sphere").unwrap();
        assert_eq!(spec.options.get("storage").and_then(|v| v.as_str()), Some("distance"));
        assert!(RendererSpec::from_cli("gpu-compute:nope+sphere").is_err());
        // The meshes stage: last in the CLI form; `none` drops mesh passes.
        let spec = RendererSpec::from_cli("gpu-compute:bricks+refloor+direct+none+none").unwrap();
        assert_eq!(spec.options.get("meshes").and_then(|v| v.as_str()), Some("none"));
        assert_eq!(spec.label(), "gpu-compute: bricks + refloor + no meshes");
        let info = mr_core::RendererInfo { name: "gpu-compute".into(), description: String::new(), tags: vec!["meshes".into()], modes: vec![mr_core::RenderMode::Unlit, mr_core::RenderMode::Lit], max_lod: 0, params: vec![] };
        let lit_meshes = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
        assert!(!spec.supports(&info, lit_meshes) && spec.supports(&info, PassMode::LIT));
        assert!(RendererSpec::from_cli("gpu-compute:bricks+refloor").unwrap().supports(&info, lit_meshes));
        assert_eq!(RendererSpec::from_cli("gpu-raster:bricks+greedy+raster+direct+none").unwrap().options.get("meshes").and_then(|v| v.as_str()), Some("none"));
    }

    /// Every shipped plan parses, and its compositions validate and resolve.
    #[test]
    fn shipped_plans_load() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans"));
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_some_and(|x| x == "toml") {
                let p = Plan::load(&path, &[]).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
                assert!(p.renderers.iter().all(|r| !r.name.is_empty()));
                n += 1;
            }
        }
        assert!(n >= 10);
    }
}
