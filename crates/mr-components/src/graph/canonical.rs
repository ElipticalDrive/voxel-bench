//! The canonical form of a graph and its cache key.

use std::collections::HashMap;

use crate::{Api, backend};

use super::*;

impl Graph {
    /// The canonical form: nodes renamed in visiting order from the output (inputs by port
    /// name), devices resolved, APIs where set and a choice, knobs sorted; equal for graphs
    /// that differ only in node ids.
    pub fn canonical(&self) -> Result<String, String> {
        let c = self.check()?;
        let g = &c.graph;
        // The client's region is the workload, not the pipeline (its knobs left out); the
        // default link (`free`) and client (`pick`), nothing else set, are left out: their
        // readers are written as reading what they read, as before the link and client were nodes,
        // which keeps the keys. So is the generator on the server.
        let knobs = |n: &Node| -> toml::Table {
            match n.role {
                Role::Client => n.knobs.iter().filter(|(k, _)| !CLIENT_KNOBS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect(),
                Role::Link => link_spec(n).map(|l| link_knobs(&l)).unwrap_or_default(),
                _ => n.knobs.clone(),
            }
        };
        let plain = |id: &str| {
            let n = &g.nodes[id];
            matches!((n.role, n.component.as_str()), (Role::Client, "pick") | (Role::Link, "free")) && knobs(n).is_empty() && n.timing.is_empty() && n.api.is_none() && n.device.is_none_or(|d| d == Device::Cpu) && !n.inputs.contains_key("chunks")
        };
        let served = |id: &str| g.nodes[id].role == Role::Generator && c.devices[id] == Device::Server && g.nodes[id].component == "procedural" && knobs(&g.nodes[id]).is_empty();
        let src = |s: &'_ str| -> String {
            let mut s = s;
            while plain(s) {
                s = &g.nodes[s].inputs["changes"];
            }
            s.to_string()
        };
        let mut ids: HashMap<String, usize> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        // The default post (`clamp`), nothing else set, is left out too: the
        // shading's image is the output, as before.
        let out = &g.nodes[&c.output];
        let start = if out.role == Role::Post && out.component == "clamp" && out.device.is_none_or(|d| d == c.devices[&out.inputs["image"]]) && out.api.is_none() && out.timing.is_empty() { out.inputs["image"].clone() } else { c.output.clone() };
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            if ids.contains_key(&id) {
                continue;
            }
            ids.insert(id.clone(), order.len());
            // Reversed so the first port by name is visited first.
            stack.extend(g.nodes[&id].inputs.values().filter(|s| !served(s)).rev().map(|s| src(s)));
            order.push(id);
        }
        // APIs only where the executor offers a choice (elsewhere a set API is checked to be
        // the one it runs, so it says nothing more).
        // (and not the choice's default: an API set to it is the same pipeline as one unset).
        let chosen = |n: &Node, a: &Api| backend(&g.executor).is_some_and(|b| n.role.slot().is_some_and(|sl| b.chooses(sl)) && b.api_choice.is_some_and(|ch| ch.default != Some(*a)));
        let mut s = format!("{}|", g.executor);
        for id in &order {
            let n = &g.nodes[id];
            let when = |p: &str| match n.when(p) {
                When::Now => String::new(),
                When::Prev => "~prev".into(),
                When::Iterate(k) => format!("~iterate{k}"),
            };
            let ins: Vec<String> = n.inputs.iter().filter(|(_, s)| !served(s)).map(|(p, s)| format!("{p}=n{}{}", ids[&src(s)], when(p))).collect();
            let knobs: Vec<String> = knobs(n).iter().map(|(k, v)| format!("{k}={v}")).collect();
            let api = c.apis.get(id).filter(|a| chosen(n, a)).map_or(String::new(), |a| format!(":{}", a.name()));
            // (The server is written `cpu`, the world's device before the server was modelled, which keeps
            // the keys: only the world runs there.)
            let dev = match c.devices[id] {
                Device::Server => Device::Cpu,
                d => d,
            };
            s += &format!("n{}={}.{}@{}{api}({}){{{}}};", ids[id], n.role.name(), n.component, dev, ins.join(","), knobs.join(","));
        }
        Ok(s)
    }

    /// A stable hash of the canonical form (FNV-1a, 64 bits): the cache key.
    pub fn key(&self) -> Result<String, String> {
        let h = self.canonical()?.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        Ok(format!("{h:016x}"))
    }
}
