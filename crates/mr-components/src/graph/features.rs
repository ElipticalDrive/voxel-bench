//! What a graph's pipeline supports: its region's features ([`mr_core::Features`], declared
//! by the executor) less what this graph's nodes and components lack.

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
}
