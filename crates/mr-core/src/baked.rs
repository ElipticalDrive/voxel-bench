//! The rule every pipeline compiled for one graph keeps: the knob
//! values it baked in (name, value as JSON) cannot change, at creation or live; another value
//! needs another build. The executors' `Spec`s list them; the generic builds bake none.

use crate::{InitParams, ParamSpec};

/// The knob values a compiled pipeline baked in (name, value as JSON).
pub type Knobs = &'static [(&'static str, &'static str)];

/// Fails if `value` (JSON) of knob `name` is not the one `baked` compiled in.
pub fn refuse(baked: &[(&str, &str)], name: &str, value: &serde_json::Value) -> anyhow::Result<()> {
    match baked.iter().find(|(k, _)| *k == name) {
        Some((_, v)) if serde_json::from_str::<serde_json::Value>(v).ok().as_ref() != Some(value) => {
            anyhow::bail!("this pipeline was compiled for {name} = {v} (not {value}); a change needs another build")
        }
        _ => Ok(()),
    }
}

/// Checks the creation against what `baked` compiled in: each baked knob's value on every
/// node of its `owner` in the graph (given there, else its default in `params`); for a
/// component (a name that is no knob but a role) a node of that role running it; for a knob
/// of the executor's own, the option (given, else its default).
pub fn check(baked: &[(&str, &str)], p: &InitParams, params: &[ParamSpec]) -> anyhow::Result<()> {
    if baked.is_empty() {
        return Ok(());
    }
    let options: serde_json::Map<String, serde_json::Value> = if p.options_json.trim().is_empty() { Default::default() } else { serde_json::from_str(&p.options_json)? };
    let graph: serde_json::Value = if p.graph_json.trim().is_empty() { Default::default() } else { serde_json::from_str(&p.graph_json)? };
    check_values(baked, &options, &graph, params)
}

/// [`check`] on the parsed options and graph.
fn check_values(baked: &[(&str, &str)], options: &serde_json::Map<String, serde_json::Value>, graph: &serde_json::Value, params: &[ParamSpec]) -> anyhow::Result<()> {
    let nodes: Vec<&serde_json::Value> = graph.get("nodes").and_then(|n| n.as_object()).map(|m| m.values().collect()).unwrap_or_default();
    let field = |n: &serde_json::Value, f: &str| n.get(f).and_then(|v| v.as_str()).map(String::from);
    for (name, _) in baked {
        let Some(q) = params.iter().find(|q| q.name == *name) else {
            // A component the build is made of, named by its role (`storage`, `traversal`): a
            // node of that role runs it (the build's own graph is its cache key; this is the
            // safety net).
            let named: Vec<String> = nodes.iter().filter(|n| field(n, "role").as_deref() == Some(name)).filter_map(|n| field(n, "component")).collect();
            anyhow::ensure!(!named.is_empty(), "baked `{name}` is neither a knob of the executor nor a role in the graph");
            if !named.iter().any(|c| refuse(baked, name, &c.clone().into()).is_ok()) {
                refuse(baked, name, &named.join(", ").into())?;
            }
            continue;
        };
        let default = q.default.to_json();
        match &q.owner {
            Some((role, comps)) => {
                for n in nodes.iter().filter(|n| field(n, "role").as_ref() == Some(role) && field(n, "component").is_some_and(|c| comps.contains(&c))) {
                    refuse(baked, name, n.get("knobs").and_then(|k| k.get(*name)).unwrap_or(&default))?;
                }
            }
            None => refuse(baked, name, options.get(*name).unwrap_or(&default))?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compiled pipeline refuses another value of a knob it baked in, and only that.
    #[test]
    fn baked_knobs_are_refused() {
        let knobs = &[("packet", "\"4\""), ("exact", "true")];
        assert!(refuse(knobs, "packet", &"4".into()).is_ok());
        let e = refuse(knobs, "packet", &"8".into()).unwrap_err().to_string();
        assert!(e.contains("compiled for packet = \"4\""), "{e}");
        assert!(refuse(knobs, "exact", &false.into()).is_err());
        assert!(refuse(knobs, "tile", &16.into()).is_ok(), "not baked: live");
    }

    /// A baked component (a name that is a role, not a knob) needs a node of that role
    /// running it; a baked knob of a node its value there, else its default.
    #[test]
    fn baked_components_are_checked_by_role() {
        let params = [ParamSpec::bool("exact", "", true, true).of("traversal", &["incremental"])];
        let baked = &[("storage", "\"prefix\""), ("traversal", "\"incremental\""), ("exact", "false")];
        let graph = |storage: &str, exact: &str| -> serde_json::Value {
            serde_json::from_str(&format!(r#"{{"nodes": {{"s": {{"role": "storage", "component": "{storage}"}}, "far": {{"role": "storage", "component": "tree64"}}, "k": {{"role": "traversal", "component": "incremental", "knobs": {{{exact}}}}}}}}}"#)).unwrap()
        };
        let check = |g: serde_json::Value, baked: &[(&str, &str)]| check_values(baked, &Default::default(), &g, &params).map_err(|e| e.to_string());
        assert_eq!(check(graph("prefix", "\"exact\": false"), baked), Ok(()), "one of the storages is the baked one");
        let e = check(graph("bricks", "\"exact\": false"), baked).unwrap_err();
        assert!(e.contains("compiled for storage = \"prefix\"") && e.contains("tree64, bricks"), "every storage named, in id order: {e}");
        let e = check(graph("prefix", ""), baked).unwrap_err();
        assert!(e.contains("compiled for exact = false (not true)"), "the default counts: {e}");
        let e = check(graph("prefix", ""), &[("geometry", "\"greedy\"")]).unwrap_err();
        assert!(e.contains("neither a knob of the executor nor a role in the graph"), "{e}");
    }
}
