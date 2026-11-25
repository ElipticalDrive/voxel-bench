//! Run plans: TOML files describing what to render, with what, and where results go.
//! See `plans/` for examples. Any value can be overridden from the command line with
//! `-s dotted.path=value` (value parsed as TOML, falling back to a string).
//!
//! **Sweeps:** a renderer written as a graph may `vary` its edits: a list of edits whose
//! values are lists, or `"*"` for everything the graph offers there, or a choice between
//! lists of edits (`mr_components::graph::varied`). It expands into one renderer per
//! combination that can be made and that `accept` takes, each distinct pipeline once
//! (`RendererSpec::swept`); the entry's `options` go to each pipeline whose executor
//! declares them. `cover = "pairs"` (on the entry, or `[sweep] cover` for all) keeps only
//! some, holding every value and every pair of two entries' values (`sweep::cover`;
//! `[sweep] part`, `parts`: the pairs spread over several runs, each with every value).
//! In each pass, swept combinations that the pass
//! mode cannot tell apart run once (`RendererSpec::told_apart`).

mod cli;
mod like;
mod output;
mod renderer;
mod run;
mod sweep;
mod written;

#[cfg(test)]
mod tests;

use std::path::Path;

use anyhow::{Context, bail};
use mr_world::SceneSpec;
use serde::{Deserialize, Serialize};

pub use like::{Leaves, Lighting, Like};
pub use output::{CompareSpec, OutputSpec, Readback, SaveSpec};
pub use renderer::{RendererSpec, component_knob};
pub use run::{Compile, Isolation, Pacing, RunSpec, Streaming, Watchdog};
pub use written::{GraphRef, Written, graph_file};

use cli::apply_override;
use sweep::expand_sweeps;

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

    /// The renderers a pass of mode `m` runs, in order: those that support it (`standalone`:
    /// the standalone renderers' infos), and of the swept pipelines it cannot tell apart
    /// ([`RendererSpec::told_apart`]) the first; with how many swept ones that leaves out.
    pub fn passes(&self, m: mr_core::PassMode, standalone: &[mr_core::RendererInfo]) -> (Vec<RendererSpec>, usize) {
        let mut seen = std::collections::HashSet::new();
        let mut moot = 0;
        let specs = self
            .renderers
            .iter()
            .filter(|s| s.info(&self.run, standalone).is_some_and(|i| s.supports(&i, m)))
            .filter(|s| {
                let first = !s.swept || seen.insert(s.told_apart(m));
                moot += usize::from(!first);
                first
            })
            .cloned()
            .collect();
        (specs, moot)
    }

    /// TOML of the plan, every renderer as [`RendererSpec::written`].
    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}
