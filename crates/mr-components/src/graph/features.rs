//! What a graph's pipeline supports: its region's features ([`mr_core::Features`], declared
//! by the executor) less what this graph's nodes and components lack; and which of the knobs
//! asked of it it leaves out ([`Checked::knobs_left_out`]).

use mr_core::{Feature, Features};

use super::{Checked, Role};
use crate::Kind;
use crate::caps::{Cap, provides};

impl Checked {
    /// Narrows `f`, its region's features, to this graph: meshes need a meshes node that
    /// draws them; mesh rays need meshes and drawers whose shading meets the triangles
    /// ([`Cap::MeshRays`]).
    pub fn narrow(&self, f: &mut Features) {
        let nodes = &self.graph.nodes;
        if f.has(Feature::Meshes) && !nodes.values().any(|n| n.role == Role::Meshes && n.component != "none") {
            f.lack(Feature::Meshes, "no meshes node");
        }
        if !f.has(Feature::Meshes) && f.has(Feature::MeshRays) {
            f.lack(Feature::MeshRays, "no meshes node");
        }
        let mut ids: Vec<&String> = nodes.keys().collect();
        ids.sort();
        for id in ids.into_iter().filter(|id| nodes[*id].role == Role::Draw) {
            let (n, kind) = (&nodes[id], self.devices.get(id).map_or(Kind::Gpu, |d| d.kind()));
            if f.has(Feature::MeshRays) && !provides(kind, Role::Draw, &n.component, Cap::MeshRays) {
                f.lack(Feature::MeshRays, format!("draw `{}` (`{id}`) lacks mesh rays: its shading does not meet the triangles", n.component));
            }
        }
    }

    /// The knobs in `asked` (name → value) this graph leaves out, each with the reason, given
    /// its executor's knobs `params`: a knob it has no node for (its owner's role and
    /// components), a value whose need ([`mr_core::ValueNeed`]) no node meets, a knob it does
    /// not have at all. Names with a `.` (`<node>.component`) are the host's, not asked here.
    pub fn knobs_left_out(&self, params: &[mr_core::ParamSpec], asked: &toml::Table) -> Vec<String> {
        let g = &self.graph;
        let stages = g.stage_nodes();
        let mut ids: Vec<&String> = g.nodes.keys().filter(|id| stages.contains(*id)).collect();
        ids.sort();
        let of_role = |role: &str| -> Vec<&String> { ids.iter().copied().filter(|id| g.nodes[*id].role.name() == role).collect() };
        let mut out = vec![];
        for (k, v) in asked.iter().filter(|(k, _)| !k.contains('.')) {
            let Some(p) = params.iter().find(|p| p.name == *k) else {
                out.push(format!("knob `{k}`: this pipeline has no such knob"));
                continue;
            };
            if let Some((role, comps)) = &p.owner
                && !of_role(role).into_iter().any(|id| comps.contains(&g.nodes[id].component))
            {
                out.push(format!("knob `{k}`: no {role} node of {} (its {role}: {})", names(comps), names(&of_role(role).into_iter().map(|id| g.nodes[id].component.clone()).collect::<Vec<_>>())));
                continue;
            }
            let value = v.as_str().map_or_else(|| v.to_string(), str::to_string);
            for need in p.needs.iter().filter(|n| n.value == value) {
                // The nodes the need is about: of its role, or what feeds their `port`.
                let at: Vec<&String> = of_role(&need.role).into_iter().flat_map(|id| match &need.port {
                    Some(port) => g.nodes[id].inputs.get(port).into_iter().collect::<Vec<_>>(),
                    None => vec![id],
                }).collect();
                if at.iter().any(|id| g.nodes.get(*id).is_some_and(|n| need.components.contains(&n.component))) {
                    continue;
                }
                let what = match &need.port {
                    Some(port) => format!("{}.{port}", need.role),
                    None => need.role.clone(),
                };
                let why = match at.first().and_then(|id| g.nodes.get(*id).map(|n| (id, n))) {
                    Some((id, n)) => format!("{} `{}` (`{id}`) lacks {}", n.role.name(), n.component, need.what),
                    None => format!("no node at {what}"),
                };
                out.push(format!("`{k} = {value}`: {why}"));
            }
        }
        out
    }
}

/// `a`, `b` / none.
fn names(v: &[String]) -> String {
    if v.is_empty() { "none".into() } else { v.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ") }
}

#[cfg(test)]
mod tests {
    use mr_core::ParamSpec;

    use crate::graph::shipped;

    #[test]
    fn value_needs_follow_ports() {
        // A need of the node feeding `visibility.primary`: `kernel` here (refloor).
        let p = |comps: &[&str]| ParamSpec::choice("view", "", &["image", "steps"], "image", true).needs("steps", "visibility", Some("primary"), comps, "steps");
        let c = shipped("raycast-gpu").unwrap().check().unwrap();
        let asked: toml::Table = toml::from_str("view = \"steps\"").unwrap();
        assert!(c.knobs_left_out(&[p(&["refloor"])], &asked).is_empty());
        assert_eq!(c.knobs_left_out(&[p(&["sphere"])], &asked), vec!["`view = steps`: traversal `refloor` (`kernel`) lacks steps".to_string()]);
        assert!(c.knobs_left_out(&[p(&["sphere"])], &toml::from_str("view = \"image\"").unwrap()).is_empty());
    }
}
