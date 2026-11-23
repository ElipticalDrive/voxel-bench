//! What each component can be used for: per device kind and API, where it is implemented and
//! which shipped graph accepts it there (as the node is, or with that one node moved to the
//! other device kind). A gap is an implementation no shipped graph reaches, or a kind with no
//! implementation; `multirenderer audit` prints the table.

use std::collections::BTreeSet;

use super::{Change, Device, Graph, Role, SHIPPED, edits, role_components, shipped};
use crate::{Api, Kind, component};

/// One component on one device kind.
#[derive(Debug, Default)]
pub struct Use {
    /// The APIs it is implemented with there (empty: no implementation).
    pub apis: Vec<Api>,
    /// The shipped files whose graph accepts it on this kind.
    pub files: BTreeSet<&'static str>,
    /// The shipped files one node-adding or -removing edit from a graph that accepts it here
    /// (and not already in `files`).
    pub by_edit: BTreeSet<&'static str>,
    /// The implemented APIs some shipped graph accepts it with when the node asks for them.
    pub apis_taken: BTreeSet<Api>,
}

#[derive(Debug)]
pub struct Row {
    pub role: Role,
    pub name: &'static str,
    /// Whether the registry describes it (components without an entry have no per-kind
    /// implementations to compare).
    pub registered: bool,
    /// Indexed by [`kind_index`].
    pub on: [Use; 2],
}

pub fn kind_index(k: Kind) -> usize {
    match k {
        Kind::Cpu => 0,
        Kind::Gpu => 1,
    }
}

const KINDS: [Kind; 2] = [Kind::Cpu, Kind::Gpu];

impl Row {
    /// What is missing on `k`, if anything: `None` = usable there, or neither implemented nor
    /// reachable (nothing to fill without new code: see [`Row::unimplemented`]).
    pub fn unreached(&self, k: Kind) -> Option<String> {
        let u = &self.on[kind_index(k)];
        if u.apis.is_empty() {
            return None;
        }
        if u.files.is_empty() && u.by_edit.is_empty() {
            return Some("implemented, no shipped graph (or one edit from one) takes it".into());
        }
        let left: Vec<&str> = u
            .apis
            .iter()
            .filter(|a| !u.apis_taken.contains(a))
            .map(|a| a.name())
            .collect();
        (!left.is_empty() && u.apis.len() > 1)
            .then(|| format!("API {} not asked for by any of them", left.join("/")))
    }

    /// Whether it lacks an implementation on `k` (registered components only).
    pub fn unimplemented(&self, k: Kind) -> bool {
        self.registered && self.on[kind_index(k)].apis.is_empty()
    }
}

/// The table, in role order, over every shipped file and the graphs one node-adding or
/// -removing edit from one.
pub fn audit() -> Vec<Row> {
    audit_of(SHIPPED.iter().map(|(f, _)| *f), true)
}

/// The table over `files` (shipped names), with the one-edit graphs if `one_edit`.
pub fn audit_of<'a>(files: impl IntoIterator<Item = &'a str>, one_edit: bool) -> Vec<Row> {
    let mut rows: Vec<Row> = Role::ALL
        .into_iter()
        .flat_map(|role| {
            role_components(role)
                .into_iter()
                .map(move |name| (role, name))
        })
        .map(|(role, name)| {
            let info = component(role, name);
            let on = KINDS.map(|k| Use {
                apis: info
                    .into_iter()
                    .flat_map(|i| i.impls)
                    .filter(|i| i.kind == k)
                    .flat_map(|i| i.apis.iter().copied())
                    .collect(),
                ..Use::default()
            });
            Row {
                role,
                name,
                registered: info.is_some(),
                on,
            }
        })
        .collect();
    for file in files {
        let (file, _) = SHIPPED
            .iter()
            .find(|(f, _)| *f == file)
            .expect("a shipped file");
        let base = shipped(file).expect("a shipped file");
        let mut graphs = vec![(base.clone(), false)];
        let mut seen = std::collections::HashSet::new();
        for h in edits(&base)
            .into_iter()
            .filter(|_| one_edit)
            .map(Graph::pruned)
        {
            if !h.nodes.keys().eq(base.nodes.keys())
                && h.accept().is_ok()
                && h.key().is_ok_and(|k| seen.insert(k))
            {
                graphs.push((h, true));
            }
        }
        for (g, edited) in graphs {
            let Ok(checked) = g.check() else { continue };
            for (id, n) in &g.nodes {
                let Some(&here) = checked.devices.get(id.as_str()) else {
                    continue;
                };
                let there = match here.kind() {
                    Kind::Cpu => Device::Gpu(0),
                    Kind::Gpu => Device::Cpu,
                };
                let moved = g.edit(&Change::Device(id.clone(), Some(there))).ok();
                for (g, kind) in [(Some(g.clone()), here.kind()), (moved, there.kind())] {
                    let Some(g) = g else { continue };
                    for c in g.choices(id) {
                        let Some(row) = rows.iter_mut().find(|r| r.role == n.role && r.name == c)
                        else {
                            continue;
                        };
                        let u = &mut row.on[kind_index(kind)];
                        if edited {
                            u.by_edit.insert(file);
                        } else {
                            u.files.insert(file);
                        }
                        for &api in &u.apis.clone() {
                            if u.apis_taken.contains(&api) {
                                continue;
                            }
                            let ok = g
                                .with_edits(&[
                                    Change::Component(id.clone(), c.into()),
                                    Change::Api(id.clone(), Some(api)),
                                ])
                                .is_ok_and(|g: Graph| g.accept().is_ok());
                            if ok {
                                u.apis_taken.insert(api);
                            }
                        }
                    }
                }
            }
        }
    }
    for r in &mut rows {
        for u in &mut r.on {
            let files = u.files.clone();
            u.by_edit.retain(|f| !files.contains(f));
        }
    }
    rows
}

/// The table as Markdown, then the gaps.
pub fn audit_markdown() -> String {
    let rows = audit();
    let mut s = String::from(
        "| role | component | cpu impl | cpu: in | gpu impl | gpu: in |\n|---|---|---|---|---|---|\n",
    );
    let cell = |u: &Use| -> (String, String) {
        let apis = if u.apis.is_empty() {
            "-".into()
        } else {
            u.apis
                .iter()
                .map(|a| {
                    if u.apis_taken.contains(a) {
                        a.name().to_string()
                    } else {
                        format!("({})", a.name())
                    }
                })
                .collect::<Vec<_>>()
                .join("/")
        };
        let all: Vec<String> = u
            .files
            .iter()
            .map(|f| f.to_string())
            .chain(u.by_edit.iter().map(|f| format!("{f}+")))
            .collect();
        let files = if all.is_empty() {
            "-".into()
        } else {
            all.join(", ")
        };
        (apis, files)
    };
    for r in &rows {
        let (ca, cf) = cell(&r.on[0]);
        let (ga, gf) = cell(&r.on[1]);
        let reg = if r.registered {
            ""
        } else {
            " (no registry entry)"
        };
        s += &format!(
            "| {} | {}{reg} | {ca} | {cf} | {ga} | {gf} |\n",
            r.role.name(),
            r.name
        );
    }
    s += "\n(api): implemented, but no graph here accepts the node asking for it (code compiled into another node's pass runs that pass's API without asking: the walk and storage in a ray-query pass take two added nodes, beyond this search). file+: reached only one node-adding or -removing edit from that file.\n\nUnreached implementations:\n";
    for r in &rows {
        for k in KINDS {
            if let Some(why) = r.unreached(k) {
                s += &format!("- {} `{}` on {}: {why}\n", r.role.name(), r.name, k.name());
            }
        }
    }
    s += "\nNo implementation on one kind (the other has one):\n";
    for r in &rows {
        for k in KINDS {
            let other = match k {
                Kind::Cpu => Kind::Gpu,
                Kind::Gpu => Kind::Cpu,
            };
            if r.unimplemented(k) && !r.unimplemented(other) {
                s += &format!(
                    "- {} `{}`: no {} implementation\n",
                    r.role.name(),
                    r.name,
                    k.name()
                );
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn audit_reaches_the_shipped_components() {
        let rows = super::audit_of(["raycast-gpu", "raycast-cpu"], false);
        // Every component a shipped file names is reached on its own device kind.
        assert!(
            rows.iter()
                .any(|r| r.name == "refloor" && !r.on[1].files.is_empty())
        );
        assert!(
            rows.iter().all(|r| r
                .on
                .iter()
                .all(|u| (u.files.is_empty() && u.by_edit.is_empty())
                    || !u.apis.is_empty()
                    || !r.registered)),
            "accepted where nothing implements it"
        );
    }
}
