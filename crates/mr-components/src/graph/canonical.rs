//! The canonical form of a graph, its cache key and its label.

use std::collections::{HashMap, HashSet};

use crate::Api;

use super::*;

/// One node as the canonical form and the label write it.
struct Entry {
    id: String,
    role: Role,
    component: String,
    device: Device,
    /// Its API, where it is a choice off the default.
    api: Option<Api>,
    /// Port, the node read (through the nodes left out), when.
    ins: Vec<(String, String, When)>,
    knobs: toml::Table,
}

impl Graph {
    /// The nodes that say something, in visiting order from the output (inputs by port
    /// name). Left out: the default link (`free`) and client (`pick`), nothing else set
    /// (their readers are written as reading what they read, as before the link and client
    /// were nodes, which keeps the keys), the generator on the server, and the default post
    /// (`clamp`: the shading's image is the output, as before). `region`: the client's
    /// region knobs count (the workload, not the pipeline: the key leaves them out).
    fn entries(&self, region: bool) -> Result<Vec<Entry>, String> {
        let c = self.check()?;
        let g = &c.graph;
        let knobs = |n: &Node| -> toml::Table {
            match n.role {
                Role::Client if !region => n.knobs.iter().filter(|(k, _)| !CLIENT_KNOBS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect(),
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
        let mut seen: HashSet<String> = HashSet::new();
        let mut order: Vec<String> = Vec::new();
        let out = &g.nodes[&c.output];
        let start = if out.role == Role::Post && out.component == "clamp" && out.device.is_none_or(|d| d == c.devices[&out.inputs["image"]]) && out.api.is_none() && out.timing.is_empty() { out.inputs["image"].clone() } else { c.output.clone() };
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            // Reversed so the first port by name is visited first.
            stack.extend(g.nodes[&id].inputs.values().filter(|s| !served(s)).rev().map(|s| src(s)));
            order.push(id);
        }
        // APIs only where the executor offers a choice (elsewhere a set API is checked to be
        // the one it runs, so it says nothing more).
        // (and not the choice's default: an API set to it is the same pipeline as one unset).
        let chosen = |n: &Node, a: &Api| c.region.is_some_and(|b| b.chooses(n.role) && b.api_choice.is_some_and(|ch| ch.default != Some(*a)));
        Ok(order
            .into_iter()
            .map(|id| {
                let n = &g.nodes[&id];
                Entry {
                    role: n.role,
                    component: n.component.clone(),
                    device: c.devices[&id],
                    api: c.apis.get(&id).filter(|a| chosen(n, a)).copied(),
                    ins: n.inputs.iter().filter(|(_, s)| !served(s)).map(|(p, s)| (p.clone(), src(s), n.when(p))).collect(),
                    knobs: knobs(n),
                    id,
                }
            })
            .collect())
    }

    /// The canonical form: nodes renamed in visiting order from the output (inputs by port
    /// name), devices resolved, APIs where set and a choice, knobs sorted; equal for graphs
    /// that differ only in node ids.
    pub fn canonical(&self) -> Result<String, String> {
        let entries = self.entries(false)?;
        let ids: HashMap<&str, usize> = entries.iter().enumerate().map(|(i, e)| (e.id.as_str(), i)).collect();
        let mut s = format!("{}|", self.runs_on());
        for e in &entries {
            let ins: Vec<String> = e.ins.iter().map(|(p, s, w)| format!("{p}=n{}{}", ids[s.as_str()], late(*w))).collect();
            let knobs: Vec<String> = e.knobs.iter().map(|(k, v)| format!("{k}={v}")).collect();
            let api = e.api.map_or(String::new(), |a| format!(":{}", a.name()));
            // (The server is written `cpu`, the world's device before the server was modelled, which keeps
            // the keys: only the world runs there.)
            let dev = match e.device {
                Device::Server => Device::Cpu,
                d => d,
            };
            s += &format!("n{}={}.{}@{}{api}({}){{{}}};", ids[e.id.as_str()], e.role.name(), e.component, dev, ins.join(","), knobs.join(","));
        }
        Ok(s)
    }

    /// A stable hash of the canonical form (FNV-1a, 64 bits): the cache key.
    pub fn key(&self) -> Result<String, String> {
        let h = self.canonical()?.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        Ok(format!("{h:016x}"))
    }

    /// A label, read off the graph alone: the executor's device, then the nodes that say
    /// something (those of `entries`, the client's region counted; the world is every
    /// graph's), each after the nodes it reads (inputs in the order its component declares
    /// them, so the main path comes first), written as its
    /// component, with `@device` where it runs elsewhere than on the executor's (the link
    /// and the client are always on the client's CPU), `:api` where that is a choice off the
    /// default, and in parentheses its inputs that arrive late and its knobs:
    /// `gpu: bricks + refloor + composite + raycast + direct`,
    /// `gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct`.
    /// Graphs that differ only in which of two like nodes an input reads share a label.
    pub fn label(&self) -> Result<String, String> {
        let entries = self.entries(true)?;
        let by: HashMap<&str, &Entry> = entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let on = self.runs_on();
        fn visit<'a>(e: &'a Entry, by: &HashMap<&str, &'a Entry>, seen: &mut HashSet<&'a str>, order: &mut Vec<&'a Entry>) {
            if seen.insert(&e.id) {
                // Inputs in the order the component declares them: the main path first.
                let ports = inputs(e.role, &e.component);
                let mut ins: Vec<&(String, String, When)> = e.ins.iter().collect();
                ins.sort_by_key(|(p, ..)| ports.iter().position(|x| x.name == p));
                ins.into_iter().for_each(|(_, s, _)| visit(by[s.as_str()], by, seen, order));
                order.push(e);
            }
        }
        let (mut seen, mut order) = (HashSet::new(), Vec::new());
        entries.first().into_iter().for_each(|e| visit(e, &by, &mut seen, &mut order));
        let parts: Vec<String> = order
            .into_iter()
            .filter(|e| e.role != Role::World)
            .map(|e| {
                let dev = if matches!(e.role, Role::Link | Role::Client) || e.device.to_string() == on { String::new() } else { format!("@{}", e.device) };
                let api = e.api.map_or(String::new(), |a| format!(":{}", a.name()));
                let ins = e.ins.iter().filter(|(_, _, w)| *w != When::Now).map(|(p, _, w)| format!("{p}{}", late(*w)));
                let inner: Vec<String> = ins.chain(e.knobs.iter().map(|(k, v)| format!("{k}={v}"))).collect();
                let inner = if inner.is_empty() { String::new() } else { format!("({})", inner.join(", ")) };
                format!("{}{dev}{api}{inner}", e.component)
            })
            .collect();
        Ok(format!("{on}: {}", parts.join(" + ")))
    }
}

/// An input's timing as the canonical form and labels write it after the port.
fn late(w: When) -> String {
    match w {
        When::Now => String::new(),
        When::Prev => "~prev".into(),
        When::Iterate(k) => format!("~iterate{k}"),
    }
}
