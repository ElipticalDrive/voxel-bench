//! Command-line spellings: graph edits, renderer lists and `-s` overrides.

use anyhow::{Context, bail};

/// `s` split at its commas outside brackets, braces, parentheses and quotes.
pub(super) fn split_top(s: &str) -> Vec<&str> {
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

pub(super) fn apply_override(table: &mut toml::Table, ov: &str) -> anyhow::Result<()> {
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
