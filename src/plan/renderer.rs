//! One renderer of a plan: its graph or standalone name, and what follows from it.

use std::path::Path;

use anyhow::{Context, bail};
use mr_core::PassMode;
use serde::{Deserialize, Serialize};

use super::cli::{cli_edit, split_top};
use super::{Compile, GraphRef, Lighting, Like, RunSpec, Written, graph_file};

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
    pub(super) start: Option<(String, Vec<mr_components::graph::Change>)>,
    /// Cache of the label (derived).
    pub(super) compose_label: std::sync::OnceLock<String>,
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
        for (id, n) in g.nodes.iter().filter(|(_, n)| !matches!(n.role, Role::World | Role::Generator | Role::Client | Role::Post)) {
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
        let n = g.nodes.values().find(|n| n.role == Role::Shading && mr_components::shading::state::stateful(&n.component))?;
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
            let n = g.nodes.values().find(|n| n.role == Role::Shading && mr_components::shading::state::stateful(&n.component))?;
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

    /// The gates on the world's stream into its client (the server-to-client crossing), in
    /// order from the world.
    pub fn world_gates(&self) -> Vec<mr_core::gate::GateNode> {
        self.gates_where(|e| e.data == mr_components::graph::Data::Changes && e.into_client)
    }

    /// The gates on its client's changes into the renderer (its graph's gates on the
    /// renderer's edges of changes, which all have the same gates), in order from the client.
    pub fn client_gates(&self) -> Vec<mr_core::gate::GateNode> {
        self.gates_where(|e| e.data == mr_components::graph::Data::Changes && !e.into_client)
    }

    /// The gates on the chunks its client's generator gives it.
    pub fn chunk_gates(&self) -> Vec<mr_core::gate::GateNode> {
        self.gates_where(|e| e.data == mr_components::graph::Data::Chunks)
    }

    /// Whether any gate crosses the world's side of its graph (its client then may hold less
    /// than the world: holes).
    pub fn gated_world(&self) -> bool {
        !(self.world_gates().is_empty() && self.client_gates().is_empty() && self.chunk_gates().is_empty())
    }

    /// The gates behind its graph's output (the frame crosses them), in order from it.
    pub fn frame_gates(&self) -> Vec<mr_core::gate::GateNode> {
        self.gates_where(|e| e.behind_output())
    }

    /// The gates of the first gated edge `which` picks.
    fn gates_where(&self, which: impl Fn(&mr_components::graph::EdgeGates) -> bool) -> Vec<mr_core::gate::GateNode> {
        self.pipeline().and_then(|g| g.check().ok()).and_then(|c| c.gates.into_iter().find(|e| which(e)).map(|e| e.gates)).unwrap_or_default()
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
