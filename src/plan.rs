//! Run plans: TOML files describing what to render, with what, and where results go.
//! See `plans/` for examples. Any value can be overridden from the command line with
//! `-s dotted.path=value` (value parsed as TOML, falling back to a string).
//!
//! **Sweeps:** a renderer written as a graph may `vary` its edits: a list of edits whose
//! values are lists, or `"*"` for everything the graph offers there, or a choice between
//! lists of edits (`mr_components::graph::varied`). It expands into one renderer per
//! combination that can be made and that `accept` takes, each distinct pipeline once
//! (`RendererSpec::swept`); the entry's `options` go to each pipeline whose executor
//! declares them. In each pass, swept combinations that the pass
//! mode cannot tell apart run once (`RendererSpec::told_apart`).

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
    /// The devices renderers run on (GPU adapters by slot, CPU threads), e.g.
    /// `devices = { gpus = ["nvidia"] }`; a renderer's own `devices` overrides it.
    pub devices: mr_core::DeviceConfig,
    /// `native`: run every renderer an executor runs (CPU or GPU) as a pipeline compiled for
    /// its graph and this machine (`pipelines`), built before their pass
    /// if not cached; `break-in`: the same, built for speed whatever the build takes (native, whole-program LTO, profile-guided and both, each timed here, the fastest
    /// kept);
    /// `off`: the generic build.
    pub compile: Compile,
    /// Time every stage of every frame: each GPU stage costs a timestamp
    /// (measured: +0.1–0.6% GPU time); `false` times nothing.
    pub stage_times: bool,
    /// The rate of the simulated clock scripted runs keep: frame `n` (warm-up included) is
    /// at `n / clock_fps` seconds, the time the link's delays are measured in.
    pub clock_fps: f64,
}

impl RunSpec {
    /// The server's chunk loads per frame (`None`: every chunk in range before each frame).
    pub fn budget(&self) -> Option<usize> {
        match self.streaming {
            Streaming::Complete => None,
            Streaming::Budget { budget } => Some(budget),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compile {
    #[default]
    Off,
    Native,
    #[serde(rename = "break-in")]
    BreakIn,
}

impl Compile {
    pub fn name(self) -> &'static str {
        match self {
            Compile::Off => "off",
            Compile::Native => "native",
            Compile::BreakIn => "break-in",
        }
    }
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
            devices: Default::default(),
            compile: Compile::Off,
            stage_times: true,
            clock_fps: 60.0,
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

/// The suffix of the host's knob for a node's component (`RendererSpec::component_knobs`).
const COMPONENT_KNOB: &str = ".component";

/// The node whose component the knob `name` is, if it is such a knob.
pub fn component_knob(name: &str) -> Option<&str> {
    name.strip_suffix(COMPONENT_KNOB)
}

/// One renderer of a plan: a pipeline graph (written `graph`: a graph file, or the graph
/// itself, with `edits`), or a standalone renderer (cpu-reference) by `name`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "Written", into = "Written")]
pub struct RendererSpec {
    /// A standalone renderer's name; empty for a pipeline (its graph says what runs it:
    /// [`Self::entry`], [`Self::info`]).
    pub name: String,
    /// The pipeline (`mr_components::graph`; `None` for standalone renderers), run as it
    /// is written.
    pub graph: Option<mr_components::graph::Graph>,
    /// Display label, to run the same renderer twice with different options.
    pub label: Option<String>,
    pub options: toml::Table,
    /// Overrides `run.devices` for this renderer.
    pub devices: Option<mr_core::DeviceConfig>,
    /// Overrides `run.compile` for this renderer.
    pub compile: Option<Compile>,
    /// Expanded from a sweep (see the module docs).
    pub swept: bool,
    /// The region it is given instead of the run's (a reference standing in for pipelines
    /// with that region; standalone renderers have no client node to say it).
    pub like: Option<Like>,
    /// The graph file and the edits it was written as (`graph = "<file>"`, `edits`); it is
    /// written so again while they still give its graph.
    start: Option<(String, Vec<mr_components::graph::Change>)>,
    /// Cache of the label (derived).
    compose_label: std::sync::OnceLock<String>,
}

/// What a pipeline is given of the world and the lighting model it computes, when they are
/// not the full-resolution reference's (another radius, LOD levels; path tracing; features
/// it declares it leaves out): a reference given the same tells a wrong pipeline from a
/// lossy, a noisy or a declared-partial one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Like {
    pub radius: u32,
    pub max_lod: u8,
    pub levels: mr_core::client::Levels,
    pub lighting: Lighting,
    pub leaves: Leaves,
}

/// What a pass asks that a pipeline declares it leaves out and its frames still show
/// otherwise: the reference draws them the same way, so the score judges what it claims.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Leaves {
    /// Lit `+meshes` frames whose shading rays see voxels only (meshes cast no shadows).
    pub mesh_rays: bool,
    /// See-through blocks drawn opaque in a world asking transparency.
    pub transparency: bool,
    /// The world as a heightmap: each column solid below its top block (2.5D).
    pub volume: bool,
}

impl Leaves {
    /// What a pipeline with `features` leaves out of a pass of mode `m` over a world asking
    /// `world`.
    pub fn of(features: &mr_core::Features, m: Option<PassMode>, world: &[mr_core::Feature]) -> Self {
        use mr_core::Feature;
        let lit_meshes = m.is_some_and(|m| m.lighting == mr_core::RenderMode::Lit && m.meshes);
        Leaves {
            mesh_rays: lit_meshes && features.has(Feature::Meshes) && !features.has(Feature::MeshRays),
            transparency: world.contains(&Feature::Transparency) && !features.has(Feature::Transparency),
            volume: !features.has(Feature::Volume),
        }
    }
}

/// The lighting model of a lit frame: the reference's direct lighting, or diffuse path
/// tracing (the `pathtrace` shading) with its bounces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lighting {
    #[default]
    Direct,
    Paths {
        bounces: u8,
    },
}

impl Like {
    /// The class of a client's region, if it differs from the full-resolution reference's
    /// (the run's radius at LOD 0).
    pub fn of(region: &crate::stream::Region, lighting: Lighting, leaves: Leaves, run: &RunSpec) -> Option<Self> {
        // Without coarse levels it does not matter how they would be made.
        let levels = if region.max_lod > 0 { region.levels } else { Default::default() };
        (region.max_lod > 0 || region.radius != run.view_radius || lighting != Lighting::Direct || leaves != Leaves::default()).then_some(Self { radius: region.radius, max_lod: region.max_lod, levels, lighting, leaves })
    }
}

impl std::fmt::Display for Like {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let levels = match self.levels {
            mr_core::client::Levels::Pick => "pick",
            mr_core::client::Levels::Derive => "derive",
        };
        match self.max_lod {
            0 => write!(f, "{levels}(radius={})", self.radius),
            l => write!(f, "{levels}(radius={}, max_lod={l})", self.radius),
        }?;
        match self.lighting {
            Lighting::Direct => Ok(()),
            Lighting::Paths { bounces } => write!(f, ", pathtrace(bounces={bounces})"),
        }?;
        if self.leaves.mesh_rays {
            write!(f, ", no mesh rays")?;
        }
        if self.leaves.transparency {
            write!(f, ", no transparency")?;
        }
        if self.leaves.volume {
            write!(f, ", heightmap")?;
        }
        Ok(())
    }
}

/// A renderer as plans write it.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Written {
    /// A standalone renderer (`name = "cpu-reference"`); a pipeline has `graph` instead.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The pipeline: a graph file (a shipped one's name,
    /// `graph = "raycast-gpu"`, or a path) or the graph itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<GraphRef>,
    /// Changes made to `graph`, in order (`mr_components::graph::Change`, the editor's own):
    /// `edits = [{ node = "storage", component = "tree64" }]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<mr_components::graph::Change>,
    pub label: Option<String>,
    #[serde(default)]
    pub options: toml::Table,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<mr_core::DeviceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile: Option<Compile>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swept: bool,
}

/// A renderer's graph as plans write it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GraphRef {
    /// A graph file: the name of one shipped with the bench (`pipelines/<name>.toml`, built
    /// in), else a path (relative to the working directory).
    File(String),
    Inline(Box<mr_components::graph::Graph>),
}

/// The graph of file `f` ([`GraphRef::File`]).
pub fn graph_file(f: &str) -> anyhow::Result<mr_components::graph::Graph> {
    if let Some(g) = mr_components::graph::shipped(f) {
        return Ok(g);
    }
    let shipped = mr_components::graph::SHIPPED.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
    let text = std::fs::read_to_string(f).with_context(|| format!("graph `{f}`: neither a shipped graph ({shipped}) nor a file that can be read"))?;
    toml::from_str(&text).with_context(|| format!("graph file `{f}`"))
}

impl TryFrom<Written> for RendererSpec {
    type Error = anyhow::Error;

    fn try_from(w: Written) -> anyhow::Result<Self> {
        if w.graph.is_some() && !w.name.is_empty() {
            bail!("a renderer has `graph` (a pipeline) or `name` (a standalone renderer), not both");
        }
        if !w.edits.is_empty() && w.graph.is_none() {
            bail!("`edits` change a `graph` (a file's name or the graph itself); this renderer has none");
        }
        let (graph, start) = match w.graph {
            None => (None, None),
            Some(r) => {
                let (g, file) = match r {
                    GraphRef::Inline(g) => (*g, None),
                    GraphRef::File(f) => (graph_file(&f)?, Some(f)),
                };
                let g = g.with_edits(&w.edits).map_err(|e| anyhow::anyhow!("renderer graph: {e}"))?;
                (Some(g), file.map(|f| (f, w.edits)))
            }
        };
        let mut s = Self { graph, start, label: w.label, options: w.options, devices: w.devices, compile: w.compile, swept: w.swept, ..Self::new(w.name) };
        s.resolve()?;
        Ok(s)
    }
}

impl From<RendererSpec> for Written {
    fn from(s: RendererSpec) -> Self {
        s.written()
    }
}

impl RendererSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), graph: None, label: None, options: Default::default(), devices: None, compile: None, swept: false, like: None, start: None, compose_label: Default::default() }
    }

    /// The pipeline of graph file `file` (a shipped one's name or a path, as a plan's `graph`).
    pub fn from_file(file: &str) -> anyhow::Result<Self> {
        Self::try_from(Written { graph: Some(GraphRef::File(file.into())), ..Default::default() })
    }

    /// Checks the pipeline and derives what follows from it: into the options, the knobs the
    /// graph spells (the API choice, timings) where they are not the knob's default.
    pub fn resolve(&mut self) -> anyhow::Result<()> {
        self.compose_label = Default::default();
        let Some(g) = &self.graph else {
            if self.name.is_empty() {
                bail!("a renderer needs a `graph` (a pipeline: a graph file's name or the graph itself) or a `name` (a standalone renderer)");
            }
            if mr_components::graph::shipped(&self.name).is_some() {
                bail!("`{0}` is a graph file: write `graph = \"{0}\"`, not `name`", self.name);
            }
            return Ok(());
        };
        g.accept().map_err(|e| anyhow::anyhow!("renderer graph: {e}"))?;
        g.region().context("renderer graph: no executor builds a graph of this shape")?;
        let params = crate::pipelines::executor_of(g).map(|(_, p)| p).unwrap_or_default();
        for (k, v) in g.preset_options().map_err(|e| anyhow::anyhow!("renderer graph: {e}"))? {
            let default = params.iter().find(|p| p.name == k).and_then(|p| toml::Value::try_from(p.default.to_json()).ok());
            if default.as_ref() == Some(&v) {
                continue;
            }
            if self.options.get(&k).is_some_and(|o| *o != v) {
                bail!("renderer graph: knob `{k}` is set in the graph and in `options`");
            }
            self.options.insert(k, v);
        }
        self.name.clear();
        Ok(())
    }

    /// The worker's entry that runs it: a pipeline's executor (`Graph::runs_on`), else its name.
    pub fn entry(&self) -> &str {
        self.graph.as_ref().map_or(self.name.as_str(), |g| g.runs_on())
    }

    /// Whether `other` is run by the same code with the same knobs: the same standalone
    /// renderer, or pipelines of one shape (the same region of the same executor).
    pub fn same_kind(&self, other: &RendererSpec) -> bool {
        let region = |s: &RendererSpec| s.graph.as_ref().and_then(|g| g.region()).map(|b| b.id);
        self.entry() == other.entry() && self.name == other.name && region(self) == region(other)
    }

    /// What it supports and its knobs: a pipeline's from its graph (the region of the executor
    /// that builds its shape), a standalone renderer's from `standalone` by name.
    pub fn info(&self, run: &RunSpec, standalone: &[mr_core::RendererInfo]) -> Option<mr_core::RendererInfo> {
        match &self.graph {
            Some(g) => crate::pipelines::graph_info(g, &self.devices(run)),
            None => standalone.iter().find(|i| i.name == self.name).cloned(),
        }
    }

    /// The components its nodes may be instead, as knobs of the host's (`<node>.component`;
    /// the executors list none: a node's component is the graph's): for each node of its
    /// stages with more than one answer, what the graph says it may be (`Graph::choices`),
    /// starting from what it is. None is live as a knob; setting one is a graph edit
    /// (`set_component`), which the session sends live where the executor takes it.
    pub fn component_knobs(&self) -> Vec<mr_core::ParamSpec> {
        use mr_components::graph::Role;
        let Some(g) = &self.graph else { return vec![] };
        let mut v = vec![];
        for (id, n) in g.nodes.iter().filter(|(_, n)| !matches!(n.role, Role::World | Role::Generator | Role::Link | Role::Client | Role::Post)) {
            let choices = g.choices(id);
            if choices.len() > 1 {
                v.push(mr_core::ParamSpec::choice(&format!("{id}{COMPONENT_KNOB}"), &format!("The component of node `{id}` ({}): an edit of the graph (restart where its executor cannot replace the node while running)", n.role.name()), &choices, &n.component, false));
            }
        }
        v
    }

    /// Node `node` runs `component` (the graph edit; on failure nothing changes).
    pub fn set_component(&mut self, node: &str, component: &str) -> anyhow::Result<()> {
        let old = self.clone();
        let g = self.graph.as_ref().with_context(|| format!("`{}` is not a pipeline", self.label()))?;
        let r = g.edit(&mr_components::graph::Change::component(node, component)).map_err(anyhow::Error::msg).and_then(|g| {
            self.graph = Some(g);
            self.resolve()
        });
        if r.is_err() {
            *self = old;
        }
        r
    }

    /// Written as it was read, a graph file and edits, if reading that back gives this
    /// renderer still (its graph was not changed since).
    fn start_form(&self) -> Option<Written> {
        let (file, edits) = self.start.as_ref()?;
        let w = Written { graph: Some(GraphRef::File(file.clone())), edits: edits.clone(), label: self.label.clone(), options: self.options.clone(), devices: self.devices.clone(), compile: self.compile, swept: self.swept, ..Default::default() };
        let back = Self::try_from(w.clone()).ok()?;
        (back.graph == self.graph && back.options == self.options).then_some(w)
    }

    /// As plans write it: the graph file and edits it was read from while they still say
    /// it, else the graph itself (the knobs it spells left out of the options); a
    /// standalone renderer by its name.
    pub fn written(&self) -> Written {
        if let Some(w) = self.start_form() {
            return w;
        }
        let mut w = Written { label: self.label.clone(), options: self.options.clone(), devices: self.devices.clone(), compile: self.compile, swept: self.swept, ..Default::default() };
        match &self.graph {
            Some(g) => {
                for k in g.preset_options().unwrap_or_default().keys() {
                    w.options.remove(k);
                }
                w.graph = Some(GraphRef::Inline(Box::new(g.clone())));
            }
            None => w.name = self.name.clone(),
        }
        w
    }

    /// From the command line: `<graph>[,<edit>,...]`, a graph file (a shipped one's name or
    /// a path, as a plan's `graph`) and the edits made to it in order ([`cli_edit`]:
    /// `raycast-gpu,storage=tree64,kernel=incremental`); or the name of a standalone
    /// renderer (`cpu-reference`). Commas inside brackets, braces, parentheses and quotes
    /// are the edit's own.
    pub fn from_cli(s: &str) -> anyhow::Result<Self> {
        let mut parts = split_top(s).into_iter().map(str::trim);
        let file = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.filter(|p| !p.is_empty()).collect();
        if rest.is_empty() && mr_components::graph::shipped(file).is_none() && !Path::new(file).is_file() {
            if file.contains([':', '+']) {
                bail!("`{s}`: a renderer is written `<graph file>[,<edit>,...]` (e.g. `raycast-gpu,storage=prefix,kernel=incremental`; `multirenderer list` shows the spellings)");
            }
            return Ok(Self::new(file));
        }
        let mut g = graph_file(file)?;
        let mut edits = vec![];
        for p in rest {
            let c = cli_edit(&g, p)?;
            g = g.edit(&c).map_err(|e| anyhow::anyhow!("`{p}`: {e}"))?;
            edits.push(c);
        }
        Self::try_from(Written { graph: Some(GraphRef::File(file.into())), edits, ..Default::default() }).with_context(|| format!("`{s}`"))
    }

    /// What a pass of mode `m` can tell of this renderer (`Graph::told_apart` and its
    /// options): renderers equal here draw the same frames in such a pass, so a sweep runs
    /// one of them.
    pub fn told_apart(&self, m: PassMode) -> String {
        let g = self.graph.as_ref().map(|g| serde_json::to_string(&g.told_apart(m)).unwrap_or_default());
        format!("{}\n{}\n{}", self.name, g.unwrap_or_default(), toml::to_string(&self.options).unwrap_or_default())
    }

    /// The graph it runs (`None` for standalone renderers).
    pub fn pipeline(&self) -> Option<mr_components::graph::Graph> {
        // Knobs that spell timings (discover-cpu `pipelined`) set them on the graph.
        self.graph.clone().map(|g| g.with_options(&self.options))
    }

    /// The graph the worker runs: `pipeline()` with the options that are node knobs (a
    /// knob with an `owner`) placed on their nodes, where the executors read them; the
    /// preset's own knobs (`threads`, ...) stay options.
    pub fn worker_graph(&self) -> anyhow::Result<Option<mr_components::graph::Graph>> {
        let Some(g) = self.pipeline() else { return Ok(None) };
        let Some((_, params)) = crate::pipelines::executor_of(&g) else { return Ok(Some(g)) };
        let knobs: Vec<_> = self.options.iter().filter_map(|(k, v)| params.iter().find(|p| p.name == *k && p.owner.is_some()).map(|p| (p, v.clone()))).collect();
        g.with_knobs(&knobs).map(Some).map_err(|e| anyhow::anyhow!("{}: {e}", self.label()))
    }

    /// The knob values it runs with as far as they are set: its stages' node knobs, its
    /// nodes' components (`component_knobs`), then its options (which win, as
    /// `worker_graph` places them).
    pub fn knob_values(&self) -> toml::Table {
        let mut t = self.graph.as_ref().and_then(|g| g.stage_knobs().ok()).unwrap_or_default();
        if let Some(g) = &self.graph {
            t.extend(g.nodes.iter().map(|(id, n)| (format!("{id}{COMPONENT_KNOB}"), n.component.clone().into())));
        }
        t.extend(self.options.clone());
        t
    }

    /// How this renderer runs compiled for this machine (`None`: the generic build).
    pub fn compiled(&self, run: &RunSpec) -> Option<Compile> {
        Some(self.compile.unwrap_or(run.compile)).filter(|&c| c != Compile::Off && matches!(crate::pipelines::bake(self), Ok(Some(_))))
    }

    /// The label of its lanes in a run: [`Self::label`], with the compiled level when it runs
    /// compiled (` [native]`), so the generic build and a compiled one stay apart.
    pub fn run_label(&self, run: &RunSpec) -> String {
        match self.compiled(run) {
            Some(c) => format!("{} [{}]", self.label(), c.name()),
            None => self.label().to_string(),
        }
    }

    /// Display label: the explicit one, else the graph's (`Graph::label`), else the
    /// standalone renderer's name.
    pub fn label(&self) -> &str {
        if let Some(l) = &self.label {
            return l;
        }
        match &self.graph {
            Some(g) => self.compose_label.get_or_init(|| g.label().unwrap_or_else(|e| format!("{} [invalid graph: {e}]", g.runs_on()))),
            None => &self.name,
        }
    }

    /// Whether this renderer draws passes of mode `m` (`info`'s features, narrowed to its
    /// graph by [`RendererSpec::info`]).
    pub fn supports(&self, info: &mr_core::RendererInfo, m: PassMode) -> bool {
        info.features.supports(m)
    }

    /// What the plan asks of it in mode `m` (`None`: a view), over a world asking `world`
    /// (`mr_world::World::asks`: transparency), that the pipeline (`info`, as its worker
    /// reported it for this graph) lacks, each with the reason: it is not asked for these, or
    /// (mesh rays, transparency) its frames leave them out; then the knobs in its options its
    /// graph leaves out (`Checked::knobs_left_out`: no node to take them, a value's need unmet).
    pub fn not_supported(&self, run: &RunSpec, m: Option<PassMode>, world: &[mr_core::Feature], info: &mr_core::RendererInfo) -> Vec<String> {
        let mut out = info.features.left_out(m, self.max_lod(run, u8::MAX), world);
        if let Some(c) = self.pipeline().and_then(|g| g.check().ok()) {
            out.extend(c.knobs_left_out(&info.params, &self.options));
        }
        out
    }

    /// The edges of its graph whose value arrives a frame late (`node.port`): its frames show
    /// a moving camera or a changing world partly as they were a frame ago.
    pub fn late(&self) -> Vec<String> {
        let Ok(Some(g)) = self.worker_graph() else { return vec![] };
        g.nodes.iter().flat_map(|(id, n)| n.timing.iter().filter(|t| *t.1 == mr_components::graph::When::Prev).map(move |(port, _)| format!("{id}.{port}"))).collect()
    }

    /// The paths per pixel per frame it accumulates in a pass of mode `m`, if it averages
    /// frames held still: its shading node path traces a lit frame with `accumulate` on.
    pub fn accumulates(&self, m: PassMode) -> Option<u32> {
        use mr_components::graph::Role;
        if m.lighting != mr_core::RenderMode::Lit {
            return None;
        }
        let g = self.worker_graph().ok()??;
        let n = g.nodes.values().find(|n| n.role == Role::Shading && n.component == "pathtrace")?;
        let defaults = mr_components::shading::state::params();
        let default = |name: &str| defaults.iter().find(|p| p.name == name).map(|p| p.default.clone());
        let on = n.knobs.get("accumulate").and_then(|v| v.as_bool()).or_else(|| default("accumulate")?.as_bool()).unwrap_or(true);
        let spp = n.knobs.get("spp").and_then(|v| v.as_integer()).or_else(|| default("spp")?.as_i64()).unwrap_or(1);
        on.then_some(spp.clamp(1, 64) as u32)
    }

    /// The lighting model of its frames in a pass of mode `m`: path tracing where its
    /// shading node path traces a lit frame (with the node's `bounces`), else direct.
    pub fn lighting(&self, m: PassMode) -> Lighting {
        use mr_components::graph::Role;
        let traced = || {
            let g = self.worker_graph().ok()??;
            let n = g.nodes.values().find(|n| n.role == Role::Shading && n.component == "pathtrace")?;
            let default = mr_components::shading::state::params().iter().find(|p| p.name == "bounces").and_then(|p| p.default.as_i64()).unwrap_or(1);
            // Without bounces it is the direct model.
            match n.knobs.get("bounces").and_then(|v| v.as_integer()).unwrap_or(default) as u8 {
                0 => None,
                bounces => Some(Lighting::Paths { bounces }),
            }
        };
        if m.lighting == mr_core::RenderMode::Lit { traced().unwrap_or_default() } else { Lighting::Direct }
    }

    /// Chunks streamed around the camera: its client's `radius`, else the run's.
    pub fn view_radius(&self, run: &RunSpec) -> u32 {
        self.like.map_or_else(|| self.client_knob("radius").map_or(run.view_radius, |r| r as u32), |l| l.radius)
    }

    /// Its client's knob `name` (`radius`, `max_lod`), if set.
    pub fn client_knob(&self, name: &str) -> Option<i64> {
        self.pipeline().and_then(|g| g.client_knob(name))
    }

    /// Sets (or with `None` clears) its client's knob `name` and re-resolves (on failure
    /// nothing changes).
    pub fn set_client_knob(&mut self, name: &str, value: Option<i64>) -> anyhow::Result<()> {
        use mr_components::graph::{CLIENT, Change, Role};
        let g = self.graph.clone().with_context(|| format!("`{}` has no client (not a pipeline)", self.label()))?;
        // The edits: the knob on its client node, the node added first where the graph
        // leaves it to be filled in (then nothing is unset on it).
        let mut edits = vec![];
        let id = match g.nodes.iter().find(|(_, n)| n.role == Role::Client) {
            Some((id, _)) => id.clone(),
            None if value.is_none() => return Ok(()),
            None => {
                let filled = g.check().map_err(anyhow::Error::msg)?.graph;
                let component = filled.nodes.values().find(|n| n.role == Role::Client).map(|n| n.component.clone()).context("no client in the checked graph")?;
                edits.push(Change::add(CLIENT, Role::Client, &component));
                CLIENT.to_string()
            }
        };
        edits.push(Change::Knob(id, name.into(), value.map(Into::into)));
        self.graph = Some(g.with_edits(&edits).map_err(anyhow::Error::msg)?);
        if let Err(e) = self.resolve() {
            self.graph = Some(g);
            let _ = self.resolve();
            return Err(e);
        }
        Ok(())
    }

    /// The link its client receives the stream over (its graph's link node).
    pub fn link(&self) -> mr_core::link::LinkSpec {
        self.pipeline().and_then(|g| g.link().ok()).unwrap_or_default()
    }

    pub fn devices(&self, run: &RunSpec) -> mr_core::DeviceConfig {
        self.devices.clone().unwrap_or_else(|| run.devices.clone())
    }

    /// LOD levels to stream, given what the renderer supports.
    pub fn max_lod(&self, run: &RunSpec, supported: u8) -> u8 {
        self.like.map_or_else(|| self.client_knob("max_lod").map_or(run.max_lod, |l| l as u8), |l| l.max_lod).min(supported)
    }

    /// How its client gets the LOD levels (its graph's `client` node).
    pub fn levels(&self) -> mr_core::client::Levels {
        self.like.map_or_else(|| self.pipeline().and_then(|g| mr_core::client::Levels::of(g.client())).unwrap_or_default(), |l| l.levels)
    }

    /// What its client takes of the server's stream, given the levels the renderer supports.
    pub fn region(&self, run: &RunSpec, supported: u8) -> crate::stream::Region {
        crate::stream::Region { radius: self.view_radius(run), max_lod: self.max_lod(run, supported), levels: self.levels(), generates: self.generates() }
    }

    /// Whether its client generates the world itself (its graph's generator on the client).
    pub fn generates(&self) -> bool {
        self.pipeline().is_some_and(|g| g.generates())
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
    /// Frames a compared frame is held still (camera and world) before a pipeline with
    /// late edges is scored once more, settled (0: no settled score).
    #[serde(default = "default_settle")]
    pub settle: u32,
    /// Paths per pixel a compared frame accumulates, held still, before a pipeline that
    /// accumulates lit frames (`pathtrace` with `accumulate`) is scored settled (0: none).
    #[serde(default = "default_converge")]
    pub converge: u32,
}

impl CompareSpec {
    /// Renders a compared frame of `spec` is held still in its settled pass in mode `m`
    /// (0: no settled pass): `settle` for late edges, enough for `converge` paths per pixel
    /// where it accumulates, the larger.
    pub fn held(&self, spec: &RendererSpec, m: PassMode) -> u32 {
        let late = if spec.late().is_empty() { 0 } else { self.settle };
        // The compared frame's own render is the first of them.
        let converge = spec.accumulates(m).map_or(0, |spp| self.converge.div_ceil(spp).saturating_sub(1));
        late.max(converge)
    }
}

fn default_settle() -> u32 {
    2
}

fn default_converge() -> u32 {
    64
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
        Self::load_with(path, overrides, &[])
    }

    /// Loads with overrides (`dotted.path=value`) and removals (`dotted.path`; absent keys
    /// are fine, so scripts can unset across plans).
    pub fn load_with(path: &Path, overrides: &[String], unset: &[String]) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse_with(&text, overrides, unset).with_context(|| format!("in plan {}", path.display()))
    }

    pub fn parse(text: &str, overrides: &[String]) -> anyhow::Result<Self> {
        Self::parse_with(text, overrides, &[])
    }

    pub fn parse_with(text: &str, overrides: &[String], unset: &[String]) -> anyhow::Result<Self> {
        let mut table: toml::Table = toml::from_str(text)?;
        for o in overrides {
            apply_override(&mut table, o)?;
        }
        for u in unset {
            let mut parts: Vec<&str> = u.trim().split('.').collect();
            let last = parts.pop().unwrap_or_default();
            let mut cur = Some(&mut table);
            for p in parts {
                cur = cur.and_then(|t| t.get_mut(p)).and_then(|v| v.as_table_mut());
            }
            if cur.and_then(|t| t.remove(last)).is_none() {
                tracing::debug!("unset `{u}`: not in the plan");
            }
        }
        expand_sweeps(&mut table)?;
        // Each renderer resolves as it loads (`Written`).
        let plan: Plan = table.try_into()?;
        if plan.run.frames == 0 {
            bail!("run.frames must be > 0");
        }
        Ok(plan)
    }
}

impl Plan {
    /// Gives renderers whose lanes would share a label ([`RendererSpec::run_label`]) distinct
    /// ones (the second `label #2`, ...), so their frames, logs and report rows stay apart.
    pub fn distinct_labels(&mut self) {
        let run = self.run.clone();
        let mut seen: std::collections::HashMap<String, usize> = Default::default();
        let mut taken: std::collections::HashSet<String> = self.renderers.iter().map(|r| r.run_label(&run)).collect();
        for r in &mut self.renderers {
            let n = seen.entry(r.run_label(&run)).or_insert(0);
            *n += 1;
            if *n == 1 {
                continue;
            }
            let label = r.label().to_string();
            for k in *n.. {
                r.label = Some(format!("{label} #{k}"));
                if taken.insert(r.run_label(&run)) {
                    *n = k;
                    break;
                }
            }
        }
    }

    /// TOML of the plan, every renderer as [`RendererSpec::written`].
    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

/// Expands every swept `[[renderer]]` (one with `vary`) into one renderer per pipeline it
/// stands for ([`expand_varied`]).
fn expand_sweeps(table: &mut toml::Table) -> anyhow::Result<()> {
    let Some(toml::Value::Array(entries)) = table.get_mut("renderer") else { return Ok(()) };
    if !entries.iter().any(|e| e.get("vary").is_some()) {
        return Ok(());
    }
    let mut out = vec![];
    for e in entries.drain(..) {
        match e.get("vary") {
            Some(_) => out.extend(expand_varied(e.as_table().context("a renderer is a table")?)?),
            None => out.push(e),
        }
    }
    *entries = out;
    Ok(())
}

/// The renderers a swept entry stands for (`vary`, after `graph` and `edits`): one per
/// combination of its choices (`mr_components::graph::varied`) that can be made and that
/// `accept` takes, each distinct pipeline once, written as the entry with the combination's
/// edits after its own. A combination that is no pipeline does not exist; none at all is an
/// error, with the first reason.
fn expand_varied(e: &toml::Table) -> anyhow::Result<Vec<toml::Value>> {
    use mr_components::graph::{CLIENT_KNOBS, Change, varied};
    let graph: GraphRef = e.get("graph").cloned().context("`vary` sweeps the edits of a `graph` (a file's name or the graph itself); this renderer has none")?.try_into()?;
    let edits: Vec<Change> = e.get("edits").cloned().map(|v| v.try_into()).transpose()?.unwrap_or_default();
    let base = match graph {
        GraphRef::File(f) => graph_file(&f)?,
        GraphRef::Inline(g) => *g,
    };
    let base = base.with_edits(&edits).map_err(|e| anyhow::anyhow!("renderer graph: {e}"))?;
    let vary = e["vary"].as_array().context("`vary` is a list of edits whose values are lists")?;
    let given = e.get("options").and_then(|o| o.as_table()).cloned().unwrap_or_default();
    let mut keys = std::collections::HashSet::new();
    let (mut out, mut first) = (vec![], None);
    for combo in varied(&base, vary).map_err(|e| anyhow::anyhow!("vary: {e}"))? {
        let g = match base.with_edits(&combo).and_then(|g| g.accept().map(|_| g)) {
            Ok(g) => g,
            Err(why) => {
                first.get_or_insert(why);
                continue;
            }
        };
        // The options this pipeline's executor declares.
        let known = crate::pipelines::executor_of(&g).map(|(_, p)| p).unwrap_or_default();
        let options: toml::Table = given.iter().filter(|(k, _)| known.iter().any(|p| p.name == **k)).map(|(k, v)| (k.clone(), v.clone())).collect();
        // The client's region is the workload, not in the key, but a pass of its own.
        let region: Vec<Option<i64>> = CLIENT_KNOBS.iter().map(|k| g.client_knob(k)).collect();
        if !keys.insert((g.with_options(&options).key().map_err(anyhow::Error::msg)?, region)) {
            continue;
        }
        let mut entry = e.clone();
        entry.remove("vary");
        entry.remove("label");
        let all: Vec<Change> = edits.iter().chain(&combo).cloned().collect();
        entry.insert("edits".into(), toml::Value::try_from(&all)?);
        entry.insert("options".into(), toml::Value::Table(options));
        entry.insert("swept".into(), toml::Value::Boolean(true));
        out.push(toml::Value::Table(entry));
    }
    if out.is_empty() {
        bail!("the sweep `vary = {}` gives no pipeline{}", e["vary"], first.map(|w| format!(" (the first combination: {w})")).unwrap_or_default());
    }
    Ok(out)
}

/// `s` split at its commas outside brackets, braces, parentheses and quotes.
fn split_top(s: &str) -> Vec<&str> {
    let (mut depth, mut quote, mut from, mut out) = (0i32, None, 0, vec![]);
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => {
                out.push(&s[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[from..]);
    out
}

/// One edit to graph `g` as the command line writes it: the plan's inline table
/// (`{node="kernel",api="rayquery"}`: every edit), or one of the short spellings
/// - `node=component`: the node's component;
/// - `node.input=source`: the input bound to node `source`, or to a new node written
///   `role:component` (its id: the role's name); `node.input=` unbinds it;
/// - `node.knob=value`: a knob of the node (the value as TOML, else a string);
/// - `node@device`: where it runs (`node@default`: unset);
/// - `+id=role:component`: a new node under that id, unbound;
/// - `-node`: the node removed.
///
/// Whether the name after the last dot is an input or a knob is read off the node's
/// component.
pub fn cli_edit(g: &mr_components::graph::Graph, p: &str) -> anyhow::Result<mr_components::graph::Change> {
    use mr_components::graph::{Change, Device, Role, Target, inputs};
    let value = |v: &str| toml::from_str::<toml::Table>(&format!("v = {v}")).ok().and_then(|mut t| t.remove("v"));
    let nodes = || g.nodes.keys().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ");
    if p.starts_with('{') {
        let t = value(p).and_then(|v| v.as_table().cloned()).with_context(|| format!("edit `{p}`: not an inline table"))?;
        return Change::try_from(t).map_err(anyhow::Error::msg);
    }
    if let Some(id) = p.strip_prefix('-') {
        return Ok(Change::Remove(id.trim().into()));
    }
    let role = |r: &str| toml::Value::from(r.trim()).try_into::<Role>().map_err(|e| anyhow::anyhow!("edit `{p}`: {}", e.to_string().trim()));
    if let Some(new) = p.strip_prefix('+') {
        let (id, (r, c)) = new.split_once('=').and_then(|(id, v)| Some((id, v.split_once(':')?))).with_context(|| format!("edit `{p}`: a new node is written `+id=role:component`"))?;
        return Ok(Change::Add(id.trim().into(), role(r)?, c.trim().into()));
    }
    let Some((k, v)) = p.split_once('=') else {
        let (id, d) = p.split_once('@').with_context(|| format!("edit `{p}`: `node=component`, `node.input=source`, `node.knob=value`, `node@device`, `+id=role:component`, `-node`, or an inline table (`{{node=\"kernel\",api=\"rayquery\"}}`)"))?;
        let d = match d.trim() {
            "default" => None,
            d => Some(Device::try_from(d.to_string()).map_err(|e| anyhow::anyhow!("edit `{p}`: {e}"))?),
        };
        return Ok(Change::Device(id.trim().into(), d));
    };
    let (k, v) = (k.trim(), v.trim());
    if g.nodes.contains_key(k) {
        return Ok(Change::Component(k.into(), v.into()));
    }
    let (id, name) = k.rsplit_once('.').filter(|(id, _)| g.nodes.contains_key(*id)).with_context(|| format!("edit `{p}`: no node `{k}` (the graph has {})", nodes()))?;
    let n = &g.nodes[id];
    if !inputs(n.role, &n.component).iter().any(|i| i.name == name) {
        return Ok(Change::Knob(id.into(), name.into(), Some(value(v).unwrap_or_else(|| v.into()))));
    }
    let target = match v.split_once(':') {
        _ if v.is_empty() => Target::Unbound,
        _ if g.nodes.contains_key(v) => Target::Node(v.into()),
        Some((r, c)) => Target::New(role(r)?, c.trim().into()),
        None => bail!("edit `{p}`: no node `{v}` to read (the graph has {}; a new one is written `role:component`)", nodes()),
    };
    Ok(Change::Bind(id.into(), name.into(), target))
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
    use mr_components::graph::RegionId;

    /// The region that builds a pipeline's graph (`None` for a standalone renderer).
    fn region(r: &RendererSpec) -> Option<RegionId> {
        r.pipeline().and_then(|g| g.region()).map(|b| b.id)
    }

    fn renderer(text: &str) -> anyhow::Result<RendererSpec> {
        Ok(toml::from_str(text)?)
    }

    fn key(r: &RendererSpec) -> String {
        r.pipeline().unwrap().key().unwrap()
    }

    /// A renderer written as a graph file plus edits is that pipeline, and is written back
    /// as it was read until its graph changes.
    #[test]
    fn graph_files_and_edits() {
        use mr_components::graph::{Change, REGIONS, SHIPPED};
        // Every shipped file, bare: the default pipeline of its name, written back as the file.
        assert_eq!(SHIPPED.len(), REGIONS.len());
        for (name, _) in SHIPPED {
            let r = renderer(&format!("graph = \"{name}\"")).unwrap();
            let d = RendererSpec::from_cli(name).unwrap();
            assert_eq!(key(&r), key(&d), "{name}");
            assert_eq!((region(&r), r.label()), (region(&d), d.label()), "{name}");
            assert_eq!(toml::to_string(&r).unwrap().trim(), format!("graph = \"{name}\"\n\n[options]"), "{name}");
        }
        // Edits: a component, a new node bound, an API, a timing, a node knob.
        for (edits, label) in [
            (r#"[{ node = "storage", component = "tree64" }, { node = "kernel", component = "incremental" }]"#, "gpu: tree64 + incremental + composite + raycast + direct"),
            (r#"[{ node = "shading", component = "pathtrace" }, { add = "far", role = "storage", component = "tree64" }, { node = "far", input = "changes", from = "world" }, { add = "sphere", role = "traversal", component = "sphere" }, { node = "sphere", input = "storage", from = "far" }, { node = "shading", input = "bounce", from = "sphere" }]"#, "gpu: bricks + refloor + composite + raycast + tree64 + sphere + pathtrace"),
        ] {
            let text = format!("graph = \"raycast-gpu\"\nedits = {edits}");
            let r = renderer(&text).unwrap_or_else(|e| panic!("{edits}: {e:#}"));
            assert_eq!((region(&r), r.label()), (Some(RegionId::GpuWalk), label), "{edits}");
            assert_ne!(key(&r), key(&renderer("graph = \"raycast-gpu\"").unwrap()), "{edits}");
            // Written as read, and read back as the same renderer.
            let w = toml::to_string(&r).unwrap();
            assert!(w.starts_with("graph = \"raycast-gpu\"\n") && w.contains("[[edits]]"), "{w}");
            assert_eq!(key(&renderer(&w).unwrap()), key(&r));
        }
        // Options stay options beside it; a knob set by an edit is the node's.
        let r = renderer("graph = \"columns-cpu\"\nedits = [{ node = \"shading\", knob = \"shadows\", value = \"sun\" }]\noptions = { threads = 2 }").unwrap();
        let ck = r.worker_graph().unwrap().unwrap().check().unwrap();
        assert_eq!(ck.node(ck.shading()).knobs.get("shadows").and_then(|v| v.as_str()), Some("sun"));
        assert_eq!(r.options.get("threads").and_then(|v| v.as_integer()), Some(2));
        let w = toml::to_string(&r).unwrap();
        assert!(w.contains("graph = \"columns-cpu\"") && w.contains("[[edits]]"), "{w}");
        // The graph changed since (the editor, a slot switch): written by what it is now.
        let mut r = renderer("graph = \"raycast-gpu\"").unwrap();
        r.graph = Some(r.graph.clone().unwrap().edit(&Change::Component("storage".into(), "tree64".into())).unwrap());
        r.resolve().unwrap();
        let w = toml::to_string(&r).unwrap();
        assert!(!w.contains("graph = \"raycast-gpu\"") && w.contains("tree64"), "{w}");
        assert_eq!(key(&renderer(&w).unwrap()), key(&r));
        // An inline graph takes edits too.
        let inline = toml::to_string(&mr_components::graph::shipped("raycast-gpu").unwrap()).unwrap().replace("[nodes", "[graph.nodes").replace("executor =", "[graph]\nexecutor =");
        let r = renderer(&format!("edits = [{{ node = \"storage\", component = \"tree64\" }}]\n{inline}")).unwrap();
        assert_eq!(r.graph.as_ref().unwrap().nodes["storage"].component, "tree64");
        // Refused with the reason.
        for (text, why) in [
            ("graph = \"no-such-graph\"", "neither a shipped graph (raycast-gpu, "),
            ("graph = \"raycast-gpu\"\nedits = [{ node = \"nowhere\", component = \"tree64\" }]", "edit 1 (`"),
            ("graph = \"raycast-gpu\"\nedits = [{ node = \"storage\", component = \"heightmap\" }]", "renderer graph: "),
            ("name = \"cpu-reference\"\nedits = [{ remove = \"storage\" }]", "`edits` change a `graph`"),
            ("graph = \"raycast-gpu\"\nname = \"cpu-reference\"", "not both"),
            // The flat form is gone, and a graph file is not a renderer's name.
            ("compose = { preset = \"raycast-gpu\" }", "unknown field `compose`"),
            ("graph = \"raycast-gpu\"\nsites = { shadow = \"incremental\" }", "unknown field `sites`"),
            ("name = \"raycast-gpu\"", "write `graph = \"raycast-gpu\"`"),
            ("label = \"nothing\"", "needs a `graph`"),
        ] {
            let e = format!("{:#}", renderer(text).unwrap_err());
            assert!(e.contains(why), "{text}: {e}");
        }
    }

    /// The region and the link are the graph's client and link nodes; the run's
    /// `view_radius` and `max_lod` are the defaults of clients without their own.
    /// Options that are node knobs reach the worker on their nodes, where the executors
    /// read them; the preset's own (`threads`) stay options.
    #[test]
    fn node_knobs_reach_the_worker_graph() {
        let mut r = RendererSpec::from_file("columns-cpu").unwrap();
        r.options.insert("shadows".into(), "sun".into());
        r.options.insert("threads".into(), 2.into());
        r.resolve().unwrap();
        let ck = r.worker_graph().unwrap().unwrap().check().unwrap();
        assert_eq!(ck.node(ck.shading()).knobs.get("shadows").and_then(|v| v.as_str()), Some("sun"));
        assert!(ck.graph.nodes.values().all(|n| !n.knobs.contains_key("threads")));
        assert_eq!(r.options.get("threads").and_then(|v| v.as_integer()), Some(2));
    }

    #[test]
    fn region_lives_in_nodes() {
        let scene = "[scene]\ngenerator = { type = \"terrain\", seed = 1 }\ncamera = { type = \"line\", from = [0, 150, 0], to = [300, 150, 75] }\n";
        let r = "[[renderer]]\ngraph = \"raycast-gpu\"\n";
        let link = |knobs: &str| format!("{{ add = \"link\", role = \"link\", component = \"net\" }}, {{ node = \"link\", knobs = {{ {knobs} }} }}");
        let home = link("latency_ms = 30, jitter_ms = 10, bandwidth_mbps = 200, loss = 0.01, reorder = true");
        let fast = link("latency_ms = 1, bandwidth_mbps = 1000");
        let new = format!(
            "{scene}[run]\nview_radius = 8\n\
             {r}\n{r}edits = [{fast}]\n{r}edits = [{home}]\n{r}edits = [{home}, {{ add = \"client\", role = \"client\", component = \"derive\" }}, {{ node = \"client\", knobs = {{ radius = 4, max_lod = 2 }} }}]\n\
             [[renderer]]\ngraph = {{ executor = \"gpu\", nodes = {{ s = {{ role = \"shading\", component = \"direct\", inputs = {{ hits = \"v\" }} }}, v = {{ role = \"visibility\", component = \"raycast\", inputs = {{ primary = \"k\" }} }}, k = {{ role = \"traversal\", component = \"refloor\", inputs = {{ storage = \"b\" }} }}, b = {{ role = \"storage\", component = \"bricks\" }}, \
             net = {{ role = \"link\", component = \"net\", knobs = {{ latency_ms = 30, jitter_ms = 10, bandwidth_mbps = 200, loss = 0.01, reorder = true }} }}, c = {{ role = \"client\", component = \"pick\", inputs = {{ changes = \"net\" }}, knobs = {{ radius = 6 }} }} }} }}\n\
             [[renderer]]\nname = \"cpu-reference\"\n"
        );
        let new = Plan::parse(&new, &[]).unwrap();
        assert_eq!((new.renderers[0].view_radius(&new.run), new.renderers[4].view_radius(&new.run)), (8, 6));
        assert_eq!(new.renderers[3].max_lod(&new.run, 8), 2);
        assert_eq!(new.renderers[2].link(), new.renderers[4].link());
        // The fields of the region and link outside the graph are gone.
        for old in [format!("{scene}[run]\nlink = {{ latency_ms = 30 }}\n{r}"), format!("{scene}{r}view_radius = 4\n"), format!("{scene}{r}link = {{ latency_ms = 1 }}\n")] {
            assert!(Plan::parse(&old, &[]).is_err(), "{old}");
        }
        let free = &new.renderers[0];
        assert!(free.link().free() && free.client_knob("radius").is_none());
        assert_eq!(new.renderers[3].view_radius(&new.run), 4);
        assert_ne!(new.renderers[1].pipeline().unwrap().key(), free.pipeline().unwrap().key(), "each network its own key");
        let mut far = new.renderers[2].clone();
        far.set_client_knob("radius", Some(3)).unwrap();
        assert_eq!(far.pipeline().unwrap().key(), new.renderers[2].pipeline().unwrap().key(), "the radius is not in the key");
        // Written back as it was read.
        let again = Plan::parse(&toml::to_string(&new).unwrap(), &[]).unwrap();
        for (a, b) in again.renderers.iter().zip(&new.renderers) {
            assert_eq!((a.label(), a.pipeline().map(|g| g.key().unwrap()), a.link(), a.view_radius(&again.run)), (b.label(), b.pipeline().map(|g| g.key().unwrap()), b.link(), b.view_radius(&new.run)));
        }
        // The CLI: commas inside an edit's braces are its own.
        let cli = RendererSpec::from_cli("raycast-gpu,+link=link:net,{node=\"link\",knobs={latency_ms=30,jitter_ms=10,bandwidth_mbps=200,loss=0.01,reorder=true}},+client=client:pick,client.radius=4").unwrap();
        assert_eq!((cli.link(), cli.view_radius(&new.run)), (new.renderers[2].link(), 4));
        // The GUI's knob setter, both ways.
        let mut s = new.renderers[0].clone();
        s.set_client_knob("max_lod", Some(2)).unwrap();
        assert_eq!((s.max_lod(&new.run, 8), s.client_knob("max_lod"), s.levels()), (2, Some(2), mr_core::client::Levels::Pick));
        s.set_client_knob("max_lod", None).unwrap();
        // The client the setter added stays, without the knob: the same pipeline by its key.
        assert_eq!((s.client_knob("max_lod"), key(&s)), (None, key(free)));
        let mut none = free.clone();
        none.set_client_knob("max_lod", None).unwrap();
        assert_eq!(none.graph, free.graph, "nothing to unset: nothing added");
    }

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
        let text = text.replace("[run]", "[output]\nsave = { every = 5 }\n[run]");
        assert!(Plan::parse(&text, &[]).unwrap().output.save.is_some());
        assert!(Plan::parse_with(&text, &[], &["output.save".into()]).unwrap().output.save.is_none());
        assert!(Plan::parse_with(&text, &[], &["output.nope".into(), "nope.x".into()]).is_ok(), "unsetting what is not there is fine");
    }

    #[test]
    fn distinct_labels() {
        let text = r#"
            [scene]
            generator = { type = "flat" }
            camera = { type = "static" }
            [[renderer]]
            name = "cpu-reference"
            [[renderer]]
            name = "cpu-reference"
            [[renderer]]
            name = "cpu-reference"
            label = "cpu-reference #2"
            [[renderer]]
            name = "cpu-reference"
        "#;
        let mut p = Plan::parse(text, &[]).unwrap();
        p.distinct_labels();
        let labels: Vec<&str> = p.renderers.iter().map(|r| r.label()).collect();
        assert_eq!(labels, ["cpu-reference", "cpu-reference #3", "cpu-reference #2", "cpu-reference #4"]);
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

    /// The components a node may be instead are knobs of the host's, asked of the graph;
    /// setting one is the graph edit.
    #[test]
    fn component_knobs_are_the_graphs() {
        let text = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n\n[[renderer]]\ngraph = \"raycast-gpu\"\nedits = [{ node = \"storage\", component = \"prefix\" }, { node = \"kernel\", component = \"incremental\" }]\noptions = { exact = false }\n";
        let p = Plan::parse(text, &[]).unwrap();
        let r = &p.renderers[0];
        assert_eq!(region(r), Some(RegionId::GpuWalk));
        // The host's own list of what each node may be (the executors list no component).
        let knobs = r.component_knobs();
        let storage = knobs.iter().find(|k| k.name == "storage.component").expect("a storage choice");
        assert!(!storage.live && storage.default.as_str() == Some("prefix") && matches!(&storage.kind, mr_core::ParamKind::Choice(c) if c.iter().any(|x| x == "tree64") && c.len() > 4), "{:?}", storage.kind);
        assert!(knobs.iter().all(|k| component_knob(&k.name).is_some_and(|id| !matches!(id, "world" | "client" | "link" | "post"))), "{:?}", knobs.iter().map(|k| &k.name).collect::<Vec<_>>());
        assert!(crate::pipelines::executor_of(&r.pipeline().unwrap()).unwrap().1.iter().all(|k| knobs.iter().all(|c| c.name != k.name)), "no knob twice");
        assert!(r.options.get("storage").is_none() && r.knob_values().get("storage.component").and_then(|v| v.as_str()) == Some("prefix"));
        // Setting one is the graph edit; every listed choice is one the graph takes, and a
        // refused one changes nothing.
        for k in &knobs {
            let mr_core::ParamKind::Choice(cs) = &k.kind else { panic!("{k:?}") };
            for c in cs {
                let mut e = r.clone();
                e.set_component(component_knob(&k.name).unwrap(), c).unwrap_or_else(|err| panic!("{} = {c}: {err:#}", k.name));
                assert_eq!(e.knob_values().get(&k.name).and_then(|v| v.as_str()), Some(c.as_str()));
            }
        }
        // Asked on every publish of a lane's knobs: quick enough for that.
        let t = std::time::Instant::now();
        for f in mr_components::graph::SHIPPED {
            RendererSpec::from_cli(f.0).unwrap().component_knobs();
        }
        assert!(t.elapsed().as_millis() < 500, "{:?} for the ten shipped graphs", t.elapsed());
        let mut e = r.clone();
        assert!(e.set_component("storage", "heightmap").is_err() && e.set_component("nope", "bricks").is_err());
        assert_eq!((e.graph.as_ref(), &e.options, e.label()), (r.graph.as_ref(), &r.options, r.label()));
        assert_eq!(r.label(), "gpu: prefix + incremental + composite + raycast + direct");
        // Saved as it was read, options beside it.
        let saved = p.to_toml().unwrap();
        assert!(saved.contains("graph = \"raycast-gpu\"") && saved.contains("[[renderer.edits]]") && !saved.contains("compose"), "{saved}");
        let again = &Plan::parse(&saved, &[]).unwrap().renderers[0];
        assert_eq!((key(again), &again.options), (key(r), &r.options));
        // A component is no option: one named so stays an option no executor reads.
        let opt = Plan::parse(&text.replace("{ exact = false }", "{ exact = false, traversal = \"sphere\" }"), &[]).unwrap();
        assert_eq!((key(&opt.renderers[0]), opt.renderers[0].options.get("traversal").and_then(|v| v.as_str())), (key(r), Some("sphere")));
        // Another region's file; what a graph cannot be is refused with the reason.
        let raster = RendererSpec::from_cli("raster-gpu,geometry=cubes").unwrap();
        assert_eq!((region(&raster), raster.label()), (Some(RegionId::GpuMeshing), "gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct"));
        assert_eq!(region(&RendererSpec::from_cli("raycast-cpu,storage=palette").unwrap()), Some(RegionId::CpuRaycast));
        for (bad, why) in [("raycast-cpu,storage=rows", "renderer graph: "), ("rayquery-gpu,storage=prefix", "no node `storage`"), ("raycast-gpu,geometry=greedy", "no node `geometry`")] {
            let e = format!("{:#}", RendererSpec::from_cli(bad).unwrap_err());
            assert!(e.contains(why), "{bad}: {e}");
        }
        // A graph file is named as the graph, not as the renderer.
        let named = text.replace("graph = \"raycast-gpu\"", "name = \"discover-gpu\"").replace("edits = [{ node = \"storage\", component = \"prefix\" }, { node = \"kernel\", component = \"incremental\" }]\n", "").replace("options = { exact = false }", "");
        assert!(named.contains("name = \"discover-gpu\"") && !named.contains("edits") && !named.contains("exact"), "{named}");
        let e = format!("{:#}", Plan::parse(&named, &[]).unwrap_err());
        assert!(e.contains("write `graph = \"discover-gpu\"`"), "{e}");
        let r = &Plan::parse(&named.replace("name = \"discover-gpu\"", "graph = \"discover-gpu\""), &[]).unwrap().renderers[0];
        assert_eq!(r.label(), "gpu: bricks + refloor + quadtree + raster + direct");
    }

    /// The command line: a graph file, then edits in their short spellings or as tables.
    #[test]
    fn command_line() {
        let cli = |s: &str| RendererSpec::from_cli(s).map_err(|e| format!("{e:#}"));
        let plan = |s: &str| renderer(s).unwrap();
        // A bare file is that graph; a name that is no file is a standalone renderer's.
        for (name, _) in mr_components::graph::SHIPPED {
            assert_eq!(key(&cli(name).unwrap()), key(&plan(&format!("graph = \"{name}\""))), "{name}");
        }
        assert_eq!(key(&cli(concat!(env!("CARGO_MANIFEST_DIR"), "/pipelines/columns-cpu.toml")).unwrap()), key(&cli("columns-cpu").unwrap()));
        let r = cli("cpu-reference").unwrap();
        assert!(r.graph.is_none() && r.name == "cpu-reference");
        // Each short spelling is the plan's edit.
        for (short, edits) in [
            ("raycast-gpu,storage=tree64,kernel=incremental", r#"[{ node = "storage", component = "tree64" }, { node = "kernel", component = "incremental" }]"#),
            ("raycast-gpu, shading = pathtrace, shading.bounces = 2", r#"[{ node = "shading", component = "pathtrace" }, { node = "shading", knob = "bounces", value = 2 }]"#),
            ("raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage", r#"[{ node = "shading", component = "pathtrace" }, { node = "shading", input = "bounce", from = { role = "traversal", component = "sphere" } }, { node = "traversal", input = "storage", from = "storage" }]"#),
            ("raycast-gpu,-meshes", r#"[{ remove = "meshes" }]"#),
            ("raycast-gpu,+far=storage:tree64,+far.kernel=traversal:sphere,far.kernel.storage=far,shading.shadow=far.kernel", r#"[{ add = "far", role = "storage", component = "tree64" }, { add = "far.kernel", role = "traversal", component = "sphere" }, { node = "far.kernel", input = "storage", from = "far" }, { node = "shading", input = "shadow", from = "far.kernel" }]"#),
            ("discover-cpu,shadow.kernel=sphere,shadow.storage=distance", r#"[{ node = "shadow.kernel", component = "sphere" }, { node = "shadow.storage", component = "distance" }]"#),
            ("raycast-gpu,{node=\"kernel\",component=\"sphere\"},{ node = \"storage\", component = \"distance\" }", r#"[{ node = "kernel", component = "sphere" }, { node = "storage", component = "distance" }]"#),
            ("raycast-gpu,+client=client:derive,{node=\"client\",knobs={radius=4,max_lod=2}}", r#"[{ add = "client", role = "client", component = "derive" }, { node = "client", knobs = { radius = 4, max_lod = 2 } }]"#),
        ] {
            let (c, p) = (cli(short).unwrap_or_else(|e| panic!("{short}: {e}")), plan(&format!("graph = \"{}\"\nedits = {edits}", short.split(',').next().unwrap())));
            assert_eq!(c.graph, p.graph, "{short}");
            assert_eq!((c.label(), c.view_radius(&RunSpec::default())), (p.label(), p.view_radius(&RunSpec::default())), "{short}");
            // Written into a plan as its file and edits.
            assert_eq!(toml::to_string(&c).unwrap(), toml::to_string(&p).unwrap(), "{short}");
        }
        // A device, set and unset; an input unbound.
        use mr_components::graph::{Change, Device, shipped};
        let g = shipped("raycast-gpu").unwrap();
        assert_eq!(cli_edit(&g, "shading@gpu1").unwrap(), Change::Device("shading".into(), Some(Device::Gpu(1))));
        assert_eq!(cli_edit(&g, "shading @ default").unwrap(), Change::Device("shading".into(), None));
        assert_eq!(key(&cli("discover-gpu,visibility@cpu,visibility@default").unwrap()), key(&cli("discover-gpu").unwrap()));
        assert_eq!(key(&cli("raycast-gpu,visibility.meshes=,-meshes").unwrap()), key(&cli("raycast-gpu,-meshes").unwrap()));
        // No meshes stage: no mesh passes.
        let lit_meshes = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
        let bare = cli("raycast-gpu,-meshes").unwrap();
        let info = |s: &RendererSpec| s.info(&RunSpec::default(), &[]).unwrap();
        assert!(!bare.supports(&info(&bare), lit_meshes) && bare.supports(&info(&bare), PassMode::LIT));
        let walk = cli("raycast-gpu").unwrap();
        assert!(walk.supports(&info(&walk), lit_meshes) && info(&walk).features.left_out(Some(lit_meshes), 0, &[]).is_empty());
        // Forward raster-gpu draws meshes but its shadow rays miss them: said, not hidden.
        let forward = cli("raster-gpu").unwrap();
        assert!(forward.supports(&info(&forward), lit_meshes));
        assert_eq!(info(&forward).features.left_out(Some(lit_meshes), 0, &[]), vec!["mesh rays: draw `raster` (`draw`) lacks mesh rays: its shading does not meet the triangles".to_string()]);
        assert!(info(&forward).features.left_out(Some(PassMode { lighting: mr_core::RenderMode::Unlit, meshes: true }), 0, &[]).is_empty());
        let visbuffer = cli("raster-gpu,draw=visbuffer").unwrap();
        assert!(info(&visbuffer).features.has(mr_core::Feature::MeshRays));
        // Knobs asked of a graph that leaves them out: said, not dropped silently.
        let left_out = |s: &str, knobs: &[(&str, toml::Value)]| {
            let mut r = cli(s).unwrap();
            r.options.extend(knobs.iter().map(|(k, v)| (k.to_string(), v.clone())));
            r.not_supported(&RunSpec::default(), Some(PassMode::LIT), &[], &info(&r))
        };
        assert_eq!(left_out("raycast-gpu", &[("bounces", 3.into())]), vec!["knob `bounces`: no shading node of `pathtrace` (its shading: `direct`)".to_string()]);
        assert!(left_out("raycast-gpu,shading=pathtrace", &[("bounces", 3.into()), ("view", "steps".into())]).is_empty());
        assert_eq!(left_out("rayquery-gpu", &[("view", "steps".into())]), vec!["knob `view`: this pipeline has no such knob".to_string()]);
        assert_eq!(left_out("discover-cpu", &[("view", "kinds".into())]), vec!["`view = kinds`: visibility `quadtree` (`visibility`) lacks clusters".to_string()]);
        assert!(left_out("discover-cpu,visibility=cluster", &[("view", "kinds".into()), ("seed_step", 8.into())]).is_empty());
        assert_eq!(left_out("raycast-gpu", &[("nonsense", 1.into())]), vec!["knob `nonsense`: this pipeline has no such knob".to_string()]);
        // Refused with the reason.
        for (s, why) in [
            ("raycast-gpu,nowhere=tree64", "no node `nowhere` (the graph has "),
            ("raycast-gpu,storage=nope", "unknown storage component `nope`"),
            ("raycast-gpu,shading.shadow=nowhere", "no node `nowhere` to read"),
            ("raycast-gpu,shading.shadow=moon:sphere", "edit `shading.shadow=moon:sphere`"),
            ("raycast-gpu,storage", "`node=component`, `node.input=source`"),
            ("raycast-gpu,storage@moon", "unknown device `moon`"),
            ("raycast-gpu,+far=tree64", "`+id=role:component`"),
            ("raycast-gpu,-nowhere", "no node `nowhere`"),
            ("raycast-gpu,{node=\"storage\"}", "what changes on the node"),
            ("no-such-graph,storage=tree64", "neither a shipped graph"),
            ("raycast-gpu:prefix+incremental,storage=tree64", "neither a shipped graph"),
            ("raycast-gpu:prefix+incremental", "a renderer is written `<graph file>[,<edit>,...]`"),
            ("raycast-cpu,storage=rows", "renderer graph: "),
        ] {
            let e = cli(s).err().unwrap_or_else(|| panic!("{s}: accepted"));
            assert!(e.contains(why), "{s}: {e}");
        }
    }

    /// A renderer written as an inline graph is the pipeline a file and edits give; its
    /// stages' knobs stay on its nodes.
    #[test]
    fn graph_renderers() {
        let base = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n";
        let graph = r#"
[[renderer]]
[renderer.graph]
executor = "gpu"
nodes.s = { role = "storage", component = "prefix" }
nodes.k = { role = "traversal", component = "incremental", inputs = { storage = "s" }, knobs = { exact = false } }
nodes.v = { role = "visibility", component = "raycast", inputs = { primary = "k" } }
nodes.out = { role = "shading", component = "direct", inputs = { hits = "v", shadow = "k" } }
"#;
        let g = Plan::parse(&format!("{base}{graph}"), &[]).unwrap();
        let mut c = RendererSpec::from_cli("raycast-gpu,storage=prefix,kernel=incremental,visibility.meshes=,-meshes").unwrap();
        c.options.insert("exact".into(), false.into());
        c.resolve().unwrap();
        let (g, c) = (&g.renderers[0], &c);
        assert_eq!(g.name, c.name);
        assert_eq!((g.label(), c.label()), ("gpu: prefix + incremental(exact=false) + raycast + direct", "gpu: prefix + incremental + raycast + direct"), "a node's knob is in the label, an option is not");
        assert!(g.options.is_empty());
        // It runs as it is written: its nodes keep their ids.
        assert_eq!(g.graph.as_ref().unwrap().nodes.keys().map(String::as_str).collect::<Vec<_>>(), ["k", "out", "s", "v"]);
        assert_eq!(g.worker_graph().unwrap().unwrap().key(), c.worker_graph().unwrap().unwrap().key(), "the same pipeline to the worker");
        assert_eq!(g.knob_values().get("exact"), c.knob_values().get("exact"));
        let bare = Plan::parse(&format!("{base}{}", graph.replace(", knobs = { exact = false }", "")), &[]).unwrap();
        assert_eq!(key(&bare.renderers[0]), key(c));
        let bad = graph.replace("primary = \"k\"", "primary = \"s\"");
        let e = Plan::parse(&format!("{base}{bad}"), &[]).unwrap_err().to_string();
        assert!(e.contains("takes ray queries"), "{e}");
        // Shadow rays through a kernel of their own (its knob stays with it) run as
        // the graph, labelled by its nodes.
        let own = graph.replace("shadow = \"k\" } }", "shadow = \"k2\" } }\nnodes.k2 = { role = \"traversal\", component = \"incremental\", inputs = { storage = \"s\" }, knobs = { exact = true, skip8 = false } }");
        let p = Plan::parse(&format!("{base}{own}"), &[]).unwrap();
        let r = &p.renderers[0];
        assert_eq!(r.label(), "gpu: prefix + incremental(exact=false) + raycast + incremental(exact=true, skip8=false) + direct");
        assert_eq!(r.knob_values().get("exact"), Some(&toml::Value::Boolean(false)), "the primary kernel's knob");
        assert!(r.options.is_empty());
        assert!(r.pipeline().unwrap().nodes.contains_key("k2"));
        // Saved as the graph it is (its knobs in its nodes), and loads back the same.
        let saved = p.to_toml().unwrap();
        assert!(saved.contains("[renderer.graph]") && !saved.contains("compose"), "{saved}");
        let again = Plan::parse(&saved, &[]).unwrap();
        assert_eq!(again.renderers[0].pipeline().unwrap().key(), r.pipeline().unwrap().key());
        assert_eq!(again.renderers[0].label(), r.label());
        // Without knobs on its kernels the same graph is saved as the graph too (no other form).
        let plain = own.replace(", knobs = { exact = true, skip8 = false }", "").replace(", knobs = { exact = false }", "");
        let p = Plan::parse(&format!("{base}{plain}"), &[]).unwrap();
        let r = &p.renderers[0];
        assert_eq!(r.label(), "gpu: prefix + incremental + raycast + incremental + direct");
        let saved = p.to_toml().unwrap();
        assert!(saved.contains("[renderer.graph]") && !saved.contains("sites") && !saved.contains("compose"), "{saved}");
        assert_eq!(key(&Plan::parse(&saved, &[]).unwrap().renderers[0]), key(r));
        let both = graph.replace("[[renderer]]\n", "[[renderer]]\nname = \"cpu-reference\"\n");
        assert!(Plan::parse(&format!("{base}{both}"), &[]).unwrap_err().to_string().contains("not both"));
    }

    /// What a pass can tell apart, read off the graph: unlit, the shading's component and
    /// the rays it casts are moot; without meshes, the meshes node and its shadow rays.
    #[test]
    fn passes_tell_apart() {
        let modes = [PassMode { lighting: mr_core::RenderMode::Unlit, meshes: false }, PassMode { lighting: mr_core::RenderMode::Unlit, meshes: true }, PassMode { lighting: mr_core::RenderMode::Lit, meshes: false }, PassMode { lighting: mr_core::RenderMode::Lit, meshes: true }];
        let [unlit, unlit_meshes, lit, lit_meshes] = modes;
        let cli = |s: &str| RendererSpec::from_cli(s).unwrap_or_else(|e| panic!("{s}: {e:#}"));
        let same = |a: &str, b: &str, m: PassMode| cli(a).told_apart(m) == cli(b).told_apart(m);
        // Path tracing with bounce rays of its own against the plain pipeline.
        let (plain, pt) = ("raycast-gpu", "raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage:tree64");
        assert!(cli(pt).pipeline().unwrap().nodes.values().any(|n| n.component == "tree64"));
        assert!(same(plain, pt, unlit) && same(plain, pt, unlit_meshes) && !same(plain, pt, lit) && !same(plain, pt, lit_meshes));
        // What primary rays walk is told apart in every pass.
        assert!(modes.iter().all(|&m| !same(plain, "raycast-gpu,storage=tree64", m)));
        // The rasteriser's storage serves shadow rays alone (the shading's and the mesh pass's).
        let (raster, other) = ("raster-gpu", "raster-gpu,storage=tree64");
        assert!(same(raster, other, unlit) && same(raster, other, unlit_meshes) && !same(raster, other, lit) && !same(raster, other, lit_meshes));
        // The meshes node and what it alone reads: told apart only by passes with meshes.
        let no_meshes = "raster-gpu,draw.meshes=,-meshes";
        assert!(cli(no_meshes).pipeline().unwrap().nodes.values().all(|n| n.role != mr_components::graph::Role::Meshes));
        assert!(same(raster, no_meshes, unlit) && same(raster, no_meshes, lit) && !same(raster, no_meshes, unlit_meshes) && !same(raster, no_meshes, lit_meshes));
        // An option is told apart always; a renderer from itself never.
        let mut o = cli(plain);
        o.options.insert("exact".into(), false.into());
        assert!(modes.iter().all(|&m| o.told_apart(m) != cli(plain).told_apart(m) && cli(pt).told_apart(m) == cli(pt).told_apart(m)));
        // Nothing is left out of a lit pass with meshes: the graph as it is.
        let g = cli(pt).graph.unwrap();
        assert_eq!(g.told_apart(lit_meshes), g);
    }

    /// The scoring class takes the lighting model: path tracing in lit passes, with its
    /// bounces; direct otherwise (unlit, no bounces, other shading).
    #[test]
    fn lighting_class() {
        use mr_core::PassMode;
        let pt = RendererSpec::from_cli("raycast-gpu,shading=pathtrace").unwrap();
        assert_eq!(pt.lighting(PassMode::LIT), Lighting::Paths { bounces: 1 });
        assert_eq!(pt.lighting(PassMode::UNLIT), Lighting::Direct);
        let mut more = pt.clone();
        more.options.insert("bounces".into(), 3.into());
        assert_eq!(more.lighting(PassMode::LIT), Lighting::Paths { bounces: 3 });
        more.options.insert("bounces".into(), 0.into());
        assert_eq!(more.lighting(PassMode::LIT), Lighting::Direct);
        assert_eq!(RendererSpec::from_cli("raycast-gpu").unwrap().lighting(PassMode::LIT), Lighting::Direct);
        assert_eq!(RendererSpec::new("cpu-reference").lighting(PassMode::LIT), Lighting::Direct);
        // Late edges are read off the graph: discover-cpu's faces, unless discovered in step.
        assert!(pt.late().is_empty() && RendererSpec::new("cpu-reference").late().is_empty());
        let late = RendererSpec::from_cli("discover-cpu").unwrap();
        assert_eq!(late.late().len(), 1, "{:?}", late.late());
        assert!(late.late()[0].ends_with(".faces"), "{:?}", late.late());
        let mut sync = late.clone();
        sync.options.insert("pipelined".into(), false.into());
        assert!(sync.late().is_empty(), "{:?}", sync.late());
        let run = RunSpec::default();
        let region = pt.region(&run, 0);
        assert_eq!(Like::of(&region, Lighting::Direct, Leaves::default(), &run), None);
        let like = Like::of(&region, pt.lighting(PassMode::LIT), Leaves::default(), &run).unwrap();
        assert_eq!(like.to_string(), format!("pick(radius={}), pathtrace(bounces=1)", run.view_radius));
        // What a pipeline declares it leaves out, its reference leaves out too.
        use mr_core::Feature;
        let lit_meshes = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
        let mut forward = mr_core::Features::of(&[Feature::Lit, Feature::Meshes, Feature::MeshRays, Feature::Volume]);
        forward.lack(Feature::MeshRays, "draw `raster` shades forward");
        let leaves = Leaves::of(&forward, Some(lit_meshes), &[Feature::Transparency]);
        assert_eq!(leaves, Leaves { mesh_rays: true, transparency: true, volume: false });
        assert_eq!(Leaves::of(&forward, Some(PassMode::UNLIT), &[]), Leaves::default(), "no shading rays unlit");
        assert_eq!(Leaves::of(&forward, None, &[]), Leaves::default());
        let columns = mr_core::Features::of(&[Feature::Lit]).lacking(Feature::Volume, "2.5D");
        assert_eq!(Leaves::of(&columns, Some(PassMode::UNLIT), &[]), Leaves { volume: true, ..Default::default() });
        let like = Like::of(&region, Lighting::Direct, leaves, &run).unwrap();
        assert_eq!(like.to_string(), format!("pick(radius={}), no mesh rays, no transparency", run.view_radius));
        // Accumulating pipelines are held still until `converge` paths per pixel; late ones
        // for `settle` renders; the larger where both.
        let c = CompareSpec { reference: "cpu-reference".into(), reference_options: Default::default(), every: 10, settle: 2, converge: 64 };
        assert_eq!((pt.accumulates(PassMode::LIT), pt.accumulates(PassMode::UNLIT)), (Some(1), None));
        assert_eq!((c.held(&pt, PassMode::LIT), c.held(&pt, PassMode::UNLIT)), (63, 0));
        let mut four = pt.clone();
        four.options.insert("spp".into(), 4.into());
        assert_eq!(c.held(&four, PassMode::LIT), 15);
        four.options.insert("accumulate".into(), false.into());
        assert_eq!((four.accumulates(PassMode::LIT), c.held(&four, PassMode::LIT)), (None, 0));
        assert_eq!((c.held(&late, PassMode::UNLIT), c.held(&RendererSpec::from_cli("raycast-gpu").unwrap(), PassMode::LIT)), (2, 0));
    }

    /// Sweeps as edits (`vary`): every combination that is a pipeline, each once, with
    /// options only where they apply; and what is refused.
    #[test]
    fn varied_sweeps() {
        use std::collections::BTreeSet;
        let base = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n";
        let plan = |r: &str| Plan::parse(&format!("{base}[[renderer]]\n{r}\n"), &[]);
        // What a renderer is to a run: its pipeline, its region, its link, where its world is made.
        let set = |p: &Plan| p.renderers.iter().map(|r| (key(r), r.label().to_string(), r.view_radius(&p.run), r.max_lod(&p.run, 8), format!("{:?}", r.link()), r.generates())).collect::<BTreeSet<_>>();
        let each_once = |p: &Plan, n: usize, what: &str| {
            assert_eq!((p.renderers.len(), set(p).len()), (n, n), "{what}");
            assert!(p.renderers.iter().all(|r| r.swept && r.pipeline().unwrap().accept().is_ok()), "{what}");
        };
        // Lists and `*` (every component the graph takes there); options as before.
        let new = plan("graph = \"raycast-gpu\"\nlabel = \"gone\"\nvary = [{ node = \"storage\", component = \"*\" }, { node = \"kernel\", component = [\"refloor\", \"sphere\"] }]\noptions = { exact = false, merge = \"rows\" }").unwrap();
        // raycast-gpu: 8 storages x 2 kernels.
        each_once(&new, 16, "storages x two kernels");
        assert!(new.renderers.iter().all(|r| region(r) == Some(RegionId::GpuWalk) && r.label != Some("gone".into()) && r.options.get("exact").is_some() && r.options.get("merge").is_none()));
        // Each is written as its file and edits (the entry's own first), and reads back.
        let r = new.renderers.iter().find(|r| r.label().contains("tree64") && r.label().contains("sphere")).unwrap();
        let w = toml::to_string(r).unwrap();
        assert!(w.starts_with("graph = \"raycast-gpu\"\n") && w.contains("[[edits]]") && !w.contains("vary"), "{w}");
        assert_eq!(key(&renderer(&w).unwrap()), key(r));
        // The full sweep the scripts run, per region.
        let full = include_str!("../scripts/full-sweep.toml").split_once("renderer = ").unwrap().1;
        let all = Plan::parse(base, &[format!("renderer={full}")]).unwrap();
        each_once(&all, 1883, "the full sweep");
        let n = |b: RegionId| all.renderers.iter().filter(|r| region(r) == Some(b)).count();
        // raycast-gpu: 8 storages x 2 prepasses x 4 kernels x 2 shadings x 2 meshes; the
        // rasteriser's shadow rays: 8 storages x 4 kernels x 3 geometries x 3 draws x 2 shadings x 2 meshes;
        // ray query: 3 geometries x 2 shadings, then the bounce walk's 8 storages x 4 kernels.
        assert_eq!((n(RegionId::GpuWalk), n(RegionId::GpuRayQuery), n(RegionId::GpuMeshing), n(RegionId::CpuColumns)), (8 * 2 * 4 * 2 * 2, 3 * 2 + 8 * 4, 8 * 4 * 3 * 3 * 2 * 2, 1));
        assert_eq!(mr_components::graph::REGIONS.iter().map(|b| n(b.id)).sum::<usize>(), all.renderers.len(), "every one a region's");
        assert!(mr_components::graph::REGIONS.iter().all(|b| n(b.id) > 0), "every region swept");
        // `plans/stream.toml`: four links, the generator's two places, three clients.
        let stream = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/stream.toml")), &[]).unwrap();
        each_once(&stream, 4 * 2 * 3, "stream");
        assert_eq!(stream.renderers.iter().map(|r| (r.view_radius(&stream.run), r.max_lod(&stream.run, 8), r.levels() == mr_core::client::Levels::Derive)).collect::<BTreeSet<_>>().len(), 3, "three client regions");
        assert_eq!((stream.renderers.iter().filter(|r| r.link().free()).count(), stream.renderers.iter().filter(|r| r.generates()).count()), (6, 12));
        // The post's tone mappers, each its own key (a CPU pipeline takes all four too).
        for file in ["raycast-gpu", "raycast-cpu"] {
            let p = plan(&format!("graph = \"{file}\"\nedits = [{{ add = \"post\", role = \"post\", component = \"clamp\" }}, {{ node = \"post\", input = \"image\", from = \"shading\" }}]\nvary = [{{ node = \"post\", component = \"*\" }}]")).unwrap();
            each_once(&p, 4, file);
            assert_eq!(p.renderers.iter().map(|r| r.pipeline().unwrap().tone().name()).collect::<Vec<_>>(), ["clamp", "reinhard", "aces", "hdr"], "{file}");
        }
        // A knob's values; the client's region is no part of the key but a pass of its own.
        let p = plan("graph = \"raycast-gpu\"\nedits = [{ add = \"client\", role = \"client\", component = \"pick\" }]\nvary = [{ node = \"client\", knob = \"radius\", value = [3, 6] }]").unwrap();
        assert_eq!(p.renderers.iter().map(|r| r.view_radius(&p.run)).collect::<Vec<_>>(), [3, 6]);
        assert_eq!(key(&p.renderers[0]), key(&p.renderers[1]));
        // What the bounce rays may read: asked of the graph, kept where `accept` takes it.
        let p = plan("graph = \"raycast-gpu\"\nedits = [{ node = \"shading\", component = \"pathtrace\" }]\nvary = [{ node = \"shading\", input = \"bounce\", from = \"*\" }]").unwrap();
        assert!(!p.renderers.is_empty() && p.renderers.iter().all(|r| r.pipeline().unwrap().accept().is_ok()));
        // Refused with the reason.
        for (r, why) in [
            ("vary = [{ node = \"storage\", component = \"*\" }]", "this renderer has none"),
            ("graph = \"raycast-gpu\"\nvary = [{ node = \"nowhere\", component = \"*\" }]", "no node `nowhere` to vary"),
            ("graph = \"raycast-gpu\"\nvary = [{ node = \"nowhere\", component = [\"tree64\"] }]", "gives no pipeline (the first combination: edit 1"),
            ("graph = \"raycast-gpu\"\nvary = [{ node = \"storage\", component = [\"heightmap\"] }]", "gives no pipeline (the first combination: "),
            ("graph = \"raycast-gpu\"\nvary = [{ node = \"shading\", knob = \"bounces\", value = \"*\" }]", "give the values as a list"),
            ("graph = \"raycast-gpu\"\nvary = [{ any = [[]], node = \"storage\" }]", "`any` stands alone"),
        ] {
            let e = format!("{:#}", plan(r).unwrap_err());
            assert!(e.contains(why), "{r}: {e}");
        }
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
                assert!(p.renderers.iter().all(|r| r.info(&p.run, &mr_worker::infos()).is_some()));
                // No knob a plan sets is unknown to its pipeline or left out by its graph.
                for r in &p.renderers {
                    let info = r.info(&p.run, &mr_worker::infos()).unwrap();
                    let knobs: Vec<String> = r.not_supported(&p.run, None, &[], &info).into_iter().filter(|w| w.starts_with("knob ") || w.starts_with('`')).collect();
                    assert!(knobs.is_empty(), "{}: {}: {knobs:?}", path.display(), r.label());
                }
                n += 1;
            }
        }
        assert!(n >= 10);
    }
}
