//! Sweeps: a renderer that `vary`s its edits expanded into one renderer per pipeline.

use anyhow::{Context, bail};

use super::{GraphRef, graph_file};

/// Expands every swept `[[renderer]]` (one with `vary`) into one renderer per pipeline it
/// stands for ([`expand_varied`]).
pub(super) fn expand_sweeps(table: &mut toml::Table) -> anyhow::Result<()> {
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
