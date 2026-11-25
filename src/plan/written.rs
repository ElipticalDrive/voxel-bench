//! Renderers as plans write them (a graph file and edits, or the graph itself) and back.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use super::{Compile, RendererSpec};

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
    let shipped = mr_components::graph::SHIPPED
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ");
    let text = std::fs::read_to_string(f).with_context(|| {
        format!("graph `{f}`: neither a shipped graph ({shipped}) nor a file that can be read")
    })?;
    toml::from_str(&text).with_context(|| format!("graph file `{f}`"))
}

impl TryFrom<Written> for RendererSpec {
    type Error = anyhow::Error;

    fn try_from(w: Written) -> anyhow::Result<Self> {
        if w.graph.is_some() && !w.name.is_empty() {
            bail!(
                "a renderer has `graph` (a pipeline) or `name` (a standalone renderer), not both"
            );
        }
        if !w.edits.is_empty() && w.graph.is_none() {
            bail!(
                "`edits` change a `graph` (a file's name or the graph itself); this renderer has none"
            );
        }
        let (graph, start) = match w.graph {
            None => (None, None),
            Some(r) => {
                let (g, file) = match r {
                    GraphRef::Inline(g) => (*g, None),
                    GraphRef::File(f) => (graph_file(&f)?, Some(f)),
                };
                let g = g
                    .with_edits(&w.edits)
                    .map_err(|e| anyhow::anyhow!("renderer graph: {e}"))?;
                (Some(g), file.map(|f| (f, w.edits)))
            }
        };
        let mut s = Self {
            graph,
            start,
            label: w.label,
            options: w.options,
            devices: w.devices,
            compile: w.compile,
            swept: w.swept,
            ..Self::new(w.name)
        };
        s.resolve()?;
        Ok(s)
    }
}

impl From<RendererSpec> for Written {
    fn from(s: RendererSpec) -> Self {
        s.written()
    }
}
