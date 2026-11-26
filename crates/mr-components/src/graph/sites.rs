//! Sites (the ports where rays are cast, and discovery's exposure tests), the stage nodes, pruning, and the world side's nodes read off a graph.

use super::*;

/// Whether `p` is a site: a ray-casting port, or discovery's exposure tests (their storage).
fn is_site(p: &Port) -> bool {
    p.casts || p.name == "exposure"
}

impl Graph {
    /// Without the nodes that lead to no image (the world stays).
    pub fn pruned(mut self) -> Graph {
        let read = |k: &String| {
            self.nodes
                .values()
                .any(|r| r.inputs.values().any(|s| s == k))
        };
        let Some(out) = self
            .nodes
            .iter()
            .find(|(k, n)| {
                known(n.role, &n.component)
                    && output(n.role, &n.component) == Data::Image
                    && !read(k)
            })
            .map(|(k, _)| k.clone())
        else {
            return self;
        };
        let mut keep = std::collections::HashSet::new();
        let mut stack = vec![out];
        while let Some(id) = stack.pop() {
            if keep.insert(id.clone()) {
                stack.extend(
                    self.nodes
                        .get(&id)
                        .into_iter()
                        .flat_map(|n| n.inputs.values().cloned()),
                );
            }
        }
        self.nodes.retain(|k, n| {
            keep.contains(k)
                || matches!(
                    n.role,
                    Role::World | Role::Generator | Role::Client
                )
        });
        self
    }

    /// The stage nodes: those the output reads other than through a site off the lead
    /// (the nodes a site of its own adds are not stages; their knobs are their own).
    pub(super) fn stage_nodes(&self) -> std::collections::HashSet<String> {
        let read = |k: &String| {
            self.nodes
                .values()
                .any(|r| r.inputs.values().any(|s| s == k))
        };
        let lead = self.lead();
        let mut keep = std::collections::HashSet::new();
        let mut stack: Vec<String> = self
            .nodes
            .iter()
            .filter(|(k, n)| {
                known(n.role, &n.component)
                    && output(n.role, &n.component) == Data::Image
                    && !read(k)
            })
            .map(|(k, _)| k.clone())
            .take(1)
            .collect();
        while let Some(id) = stack.pop() {
            if !keep.insert(id.clone()) {
                continue;
            }
            let Some(n) = self.nodes.get(&id) else {
                continue;
            };
            for (port, src) in &n.inputs {
                let off_lead = inputs(n.role, &n.component)
                    .iter()
                    .any(|p| p.name == port && is_site(p))
                    && lead.as_ref().is_none_or(|(c, p)| *c != id || p != port);
                if !off_lead {
                    stack.push(src.clone());
                }
            }
        }
        keep
    }

    /// Whether the world is generated on the client (the generator node off the server).
    pub fn generates(&self) -> bool {
        self.nodes
            .values()
            .any(|n| n.role == Role::Generator && n.device.is_some_and(|d| d != Device::Server))
    }

    /// The client node's component (its identity, `pick`, when the graph has none).
    pub fn client(&self) -> &str {
        self.nodes
            .values()
            .find(|n| n.role == Role::Client)
            .map_or(identity(Role::Client).unwrap(), |n| n.component.as_str())
    }

    /// The client's knob `name` ([`CLIENT_KNOBS`]), if set.
    pub fn client_knob(&self, name: &str) -> Option<i64> {
        self.nodes
            .values()
            .find(|n| n.role == Role::Client)
            .and_then(|n| n.knobs.get(name))
            .and_then(|v| v.as_integer())
    }

    /// The post node's tone mapper (`clamp` when the graph has none).
    pub fn tone(&self) -> crate::post::Tone {
        self.nodes
            .values()
            .find(|n| n.role == Role::Post)
            .and_then(|n| crate::post::Tone::parse(&n.component))
            .unwrap_or_default()
    }
}
