//! Validation with reasons: the checked graph and its accessors.

use std::collections::{HashMap, HashSet};

use crate::{Api, backend, caps, component};

use super::*;

impl Graph {
    /// The world, the link and the client added if missing, in a chain: the link reads the
    /// world, the client the link, and every other `changes` input (unbound, or bound to the
    /// world or the link) the client: the world's change log reaches a renderer only over
    /// its link, through its client.
    pub(super) fn normalised(&self) -> Result<Graph, String> {
        let mut g = self.placed();
        let one = |g: &mut Graph, role: Role, component: &str| -> Result<String, String> {
            let ids: Vec<String> = g.nodes.iter().filter(|(_, n)| n.role == role).map(|(k, _)| k.clone()).collect();
            match ids.as_slice() {
                [] => {
                    let id = role.name();
                    if g.nodes.contains_key(id) {
                        return Err(format!("node `{id}` must be the {id} (role = \"{id}\")"));
                    }
                    g.nodes.insert(id.into(), Node::new(role, component));
                    Ok(id.to_string())
                }
                [one] => Ok(one.clone()),
                _ => Err(format!("one {} per graph (have: {})", role.name(), ids.join(", "))),
            }
        };
        let world = one(&mut g, Role::World, "world")?;
        let generator = one(&mut g, Role::Generator, "procedural")?;
        let link = one(&mut g, Role::Link, "free")?;
        let client = one(&mut g, Role::Client, "pick")?;
        // The generator feeds what runs on its device: the world on the server, else the client.
        let local = g.nodes[&generator].device.is_some_and(|d| d != Device::Server);
        let (feeds, other) = if local { (&client, &world) } else { (&world, &client) };
        g.nodes.get_mut(feeds).unwrap().inputs.insert("chunks".into(), generator.clone());
        g.nodes.get_mut(other).unwrap().inputs.remove("chunks");
        for (id, n) in g.nodes.iter_mut() {
            n.timing.retain(|_, w| *w != When::Now);
            let from = if *id == link {
                &world
            } else if *id == client {
                &link
            } else {
                &client
            };
            for p in inputs(n.role, &n.component).iter().filter(|p| p.data == Data::Changes) {
                let src = n.inputs.entry(p.name.into()).or_insert_with(|| from.clone());
                if *src == world || *src == link {
                    *src = from.clone();
                }
            }
        }
        // Unbound secondary rays follow the rays before them: the shading's shadow rays the
        // strategy's own (on the same device), bounces the shadow rays, mesh shadows the
        // shading's shadow rays.
        if let Some(sid) = g.nodes.iter().find(|(_, n)| n.role == Role::Shading).map(|(k, _)| k.clone()) {
            let has = |g: &Graph, id: &str, port: &str| g.nodes.get(id).is_some_and(|n| inputs(n.role, &n.component).iter().any(|p| p.name == port));
            if has(&g, &sid, "shadow") && !g.nodes[&sid].inputs.contains_key("shadow")
                && let Some(k) = g.lead_kernel(&sid) {
                g.nodes.get_mut(&sid).unwrap().inputs.insert("shadow".into(), k);
            }
            if let Some(k) = g.nodes[&sid].inputs.get("shadow").cloned() {
                if has(&g, &sid, "bounce") {
                    g.nodes.get_mut(&sid).unwrap().inputs.entry("bounce".into()).or_insert(k.clone());
                }
                for n in g.nodes.values_mut().filter(|n| n.role == Role::Meshes) {
                    if inputs(n.role, &n.component).iter().any(|p| p.name == "shadow") {
                        n.inputs.entry("shadow".into()).or_insert(k.clone());
                    }
                }
            }
            // The post after the shading: `clamp` if missing, reading the
            // shading if unbound.
            let post = one(&mut g, Role::Post, "clamp")?;
            g.nodes.get_mut(&post).unwrap().inputs.entry("image".into()).or_insert(sid);
        }
        Ok(g)
    }

    /// The kernel the strategy feeding shading node `shading` casts its own rays with
    /// (primary or discovery rays), if it casts any on the shading's device.
    fn lead_kernel(&self, shading: &str) -> Option<String> {
        let mut id = self.nodes.get(shading)?.inputs.get("hits")?;
        if self.nodes.get(id)?.role == Role::Draw {
            id = self.nodes[id].inputs.get("faces")?;
        }
        let vis = self.nodes.get(id)?;
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role, &n.component));
        if vis.role != Role::Visibility || dev(vis) != dev(&self.nodes[shading]) {
            return None;
        }
        inputs(vis.role, &vis.component).iter().filter(|p| p.casts).find_map(|p| vis.inputs.get(p.name).cloned())
    }

    /// This frame's edges into `id`: inputs with timing `now` (`prev` and `iterate` edges
    /// carry values from before; they close cycles).
    pub(super) fn now_inputs<'a>(&'a self, id: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        let n = &self.nodes[id];
        n.inputs.iter().filter(|(p, _)| n.when(p) == When::Now).map(|(p, s)| (p.as_str(), s.as_str()))
    }

    /// Whether `id` depends on `on` within the frame (through `now` edges; a node on itself).
    pub(super) fn depends(&self, id: &str, on: &str) -> bool {
        let mut seen = HashSet::new();
        let mut stack = vec![id];
        while let Some(x) = stack.pop() {
            if x == on {
                return true;
            }
            if seen.insert(x) {
                stack.extend(self.now_inputs(x).map(|(_, s)| s));
            }
        }
        false
    }

    /// Producers before consumers over this frame's edges (post-order from the output,
    /// then from nodes only reached through `prev` / `iterate` edges); a cycle of `now`
    /// edges is an error.
    pub(super) fn frame_order<'a>(&'a self, out: &'a str) -> Result<Vec<&'a str>, String> {
        let mut order = Vec::new();
        let mut state: HashMap<&str, bool> = HashMap::new(); // false: on the stack, true: done
        fn visit<'a>(g: &'a Graph, id: &'a str, state: &mut HashMap<&'a str, bool>, order: &mut Vec<&'a str>) -> Result<(), String> {
            match state.get(id) {
                Some(true) => return Ok(()),
                Some(false) => return Err(format!("the graph has a cycle through `{id}` within one frame: give one of its edges the timing `prev` (the previous frame's value) or `iterate` (passes within the frame)")),
                None => {}
            }
            state.insert(id, false);
            for (_, src) in g.now_inputs(id) {
                visit(g, src, state, order)?;
            }
            state.insert(id, true);
            order.push(id);
            Ok(())
        }
        visit(self, out, &mut state, &mut order)?;
        // Nodes only reached through earlier values, latest first (from the output's side).
        let mut stack: Vec<&str> = vec![out];
        let mut seen: HashSet<&str> = HashSet::new();
        while let Some(id) = stack.pop() {
            if seen.insert(id) {
                if !state.contains_key(id) {
                    visit(self, id, &mut state, &mut order)?;
                }
                stack.extend(self.nodes[id].inputs.values().map(String::as_str));
            }
        }
        Ok(order)
    }

    /// Checks the graph is well formed (not yet whether its back end runs it: see
    /// [`Self::validate`]), with the reasons when it isn't.
    pub fn check(&self) -> Result<Checked, String> {
        let placed = self.placed();
        if placed.executor != self.executor {
            return placed.check();
        }
        let b = backend(&self.executor).ok_or_else(|| format!("unknown back end `{}`", self.executor))?;
        let g = self.normalised()?;
        let mut why = Vec::new();
        for (id, n) in &g.nodes {
            if !known(n.role, &n.component) {
                let have: Vec<&str> = match n.role {
                    Role::Client => CLIENTS.to_vec(),
                    Role::Link => LINKS.to_vec(),
                    Role::Generator => GENERATORS.to_vec(),
                    Role::Post => POSTS.to_vec(),
                    r => r.slot().map(|s| crate::components(s).map(|c| c.name).collect()).unwrap_or_default(),
                };
                why.push(format!("`{id}`: unknown {} component `{}` (have: {})", n.role.name(), n.component, have.join(", ")));
                continue;
            }
            match n.role {
                Role::Link => {
                    if let Err(e) = link_spec(n) {
                        why.push(format!("`{id}`: {e}"));
                    }
                }
                Role::Client => {
                    for (k, v) in &n.knobs {
                        match (CLIENT_KNOBS.iter().position(|c| c == k), v.as_integer()) {
                            (None, _) => why.push(format!("`{id}`: the client has no knob `{k}` (has: {})", CLIENT_KNOBS.join(", "))),
                            (Some(0), Some(1..=1024)) | (Some(1), Some(0..=15)) => {}
                            (Some(0), _) => why.push(format!("`{id}`.radius: chunks around the camera, 1 to 1024 (not {v})")),
                            _ => why.push(format!("`{id}`.max_lod: LOD levels beyond LOD 0, 0 to 15 (not {v})")),
                        }
                    }
                }
                Role::Post if !n.knobs.is_empty() => why.push(format!("`{id}`: the post has no knobs (its component is the tone mapper: {})", POSTS.join(", "))),
                _ => {}
            }
            let ports = inputs(n.role, &n.component);
            for (p, src) in &n.inputs {
                let Some(port) = ports.iter().find(|x| x.name == p) else {
                    why.push(format!("`{id}` ({} `{}`) has no input `{p}` (has: {})", n.role.name(), n.component, ports.iter().map(|x| x.name).collect::<Vec<_>>().join(", ")));
                    continue;
                };
                let Some(s) = g.nodes.get(src) else {
                    why.push(format!("`{id}`.{p}: no node `{src}`"));
                    continue;
                };
                if !known(s.role, &s.component) {
                    continue;
                }
                let out = output(s.role, &s.component);
                if out != port.data {
                    why.push(format!("`{id}`.{p} takes {}, but `{src}` ({} `{}`) gives {}", port.data.name(), s.role.name(), s.component, out.name()));
                    continue;
                }
            }
            for (p, w) in &n.timing {
                match (ports.iter().find(|x| x.name == p), n.inputs.contains_key(p)) {
                    (Some(port), true) if port.data.fused() => why.push(format!("`{id}`.{p}: {} are compiled into their caller, so they arrive within the frame (no timing)", port.data.name())),
                    (_, true) => {}
                    _ => why.push(format!("`{id}`: a timing for `{p}`, which is not a bound input")),
                }
                if matches!(w, When::Iterate(k) if *k < 2) {
                    why.push(format!("`{id}`.{p}: an iteration runs its consumer at least twice (iterate >= 2)"));
                }
            }
            for p in ports.iter().filter(|p| p.required && !n.inputs.contains_key(p.name)) {
                why.push(format!("`{id}` ({} `{}`): input `{}` ({}) is not bound", n.role.name(), n.component, p.name, p.description));
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        let outs: Vec<&String> = g.nodes.iter().filter(|(k, n)| output(n.role, &n.component) == Data::Image && !g.nodes.values().any(|r| r.inputs.values().any(|s| s == *k))).map(|(k, _)| k).collect();
        let [out] = outs.as_slice() else {
            return Err(format!("a graph has one image output (have: {})", if outs.is_empty() { "none".into() } else { outs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") }));
        };
        let out = out.to_string();
        // Every node leads to the image, through edges of any timing.
        let mut reached: HashSet<&str> = HashSet::new();
        let mut stack = vec![out.as_str()];
        while let Some(id) = stack.pop() {
            if reached.insert(id) {
                stack.extend(g.nodes[id].inputs.values().map(String::as_str));
            }
        }
        let dangling: Vec<&str> = g.nodes.keys().map(|k| k.as_str()).filter(|k| !reached.contains(k)).collect();
        if !dangling.is_empty() {
            return Err(format!("nodes that lead to no image: {}", dangling.join(", ")));
        }
        let order = g.frame_order(&out)?;
        // An iteration closes a cycle of this frame's edges.
        for (id, n) in &g.nodes {
            for (p, src) in n.inputs.iter().filter(|(p, _)| matches!(n.when(p), When::Iterate(_))) {
                if !g.depends(src, id) {
                    why.push(format!("`{id}`.{p} iterates, but `{src}` does not depend on `{id}` within the frame (no cycle to iterate: use `prev`)"));
                }
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        // Devices, consumers first: a fused node runs with its callers.
        let mut why = Vec::new();
        let mut devices: HashMap<String, Device> = HashMap::new();
        for &id in order.iter().rev() {
            let n = &g.nodes[id];
            let d = match n.device {
                Some(d) => d,
                None if output(n.role, &n.component).fused() => {
                    let callers: Vec<(&str, Device)> = g.nodes.iter().filter(|(_, c)| c.inputs.values().any(|s| s == id)).map(|(k, _)| (k.as_str(), devices[k.as_str()])).collect();
                    let d = callers[0].1;
                    if let Some((other, od)) = callers.iter().find(|(_, x)| *x != d) {
                        return Err(format!("`{id}` is compiled into callers on two devices (`{}` on {d}, `{other}` on {od}): give each device its own instance", callers[0].0));
                    }
                    d
                }
                // The post runs where the image it maps is made (compiled into its output).
                None if n.role == Role::Post => {
                    let s = &g.nodes[&n.inputs["image"]];
                    s.device.unwrap_or_else(|| default_device(&g.executor, s.role, &s.component))
                }
                None => default_device(&g.executor, n.role, &n.component),
            };
            devices.insert(id.to_string(), d);
            if n.role == Role::Generator {
                if !matches!(d, Device::Server | Device::Cpu) {
                    why.push(format!("`{id}`: the generator runs on the server (streamed) or the client's cpu, not on {d}"));
                }
            } else if (n.role == Role::World) != (d == Device::Server) {
                why.push(if n.role == Role::World { format!("`{id}`: the world runs on the server, not on {d}") } else { format!("`{id}`: only the world and its generator run on the server") });
            } else if n.role == Role::Client && d != Device::Cpu {
                why.push(format!("`{id}`: the client runs on the cpu (the renderer's process), not on {d}"));
            } else if n.role == Role::Link && d != Device::Cpu {
                why.push(format!("`{id}`: the link runs on the cpu (the client's process, in front of the client), not on {d}"));
            }
        }
        for (id, n) in &g.nodes {
            for (p, src) in &n.inputs {
                let s = &g.nodes[src];
                if n.role == Role::Post && devices[src] != devices[id] {
                    return Err(format!("`{id}` on {} cannot map `{src}`'s image on {}: the post is compiled into the shading's output, on its device", devices[id], devices[src]));
                }
                if output(s.role, &s.component).fused() && devices[src] != devices[id] {
                    return Err(format!("`{id}`.{p} on {} cannot read `{src}` on {}: {} are compiled into their caller", devices[id], devices[src], output(s.role, &s.component).name()));
                }
            }
        }
        // Capability needs on what each port reads (on the reader's device): met natively,
        // by an adapter, or not at all.
        for (id, n) in &g.nodes {
            let needs = n.role.slot().and_then(|sl| component(sl, &n.component)).map_or(&[][..], |c| c.needs);
            for src in n.inputs.values() {
                let s = &g.nodes[src];
                for &(slot, cap) in needs.iter().filter(|(slot, _)| s.role.slot() == Some(*slot)) {
                    if !caps::provides(devices[src].kind(), slot, &s.component, cap) && caps::adapter(slot, cap).is_none() {
                        why.push(format!("`{id}` ({}) needs {} from `{src}`: {} `{}` lacks it on the {} and no adapter gives it", n.component, cap.name(), slot.name(), s.component, devices[src].kind().name()));
                    }
                }
            }
        }
        // APIs: a node's own, else (fused) its callers', else the executor's choice.
        let mut apis: HashMap<String, Api> = HashMap::new();
        for &id in order.iter().rev() {
            let n = &g.nodes[id];
            let fused = output(n.role, &n.component).fused();
            // Inherited when every caller runs one API (a module compiled into callers on two
            // APIs of one device runs as each of them).
            let inherited = || {
                let mut callers = g.nodes.iter().filter(|(_, c)| c.inputs.values().any(|s| s == id)).map(|(k, _)| apis.get(k.as_str()).copied());
                let first = callers.next().flatten();
                callers.all(|a| a == first).then_some(first).flatten()
            };
            let api = match n.api {
                Some(a) => {
                    if a.kind() != devices[id].kind() {
                        why.push(format!("`{id}`: api {} runs on a {}, the node is on {}", a.name(), a.kind().name(), devices[id]));
                    } else if let Some(c) = n.role.slot().and_then(|sl| component(sl, &n.component)).filter(|c| !c.impls.iter().any(|i| i.kind == a.kind() && i.apis.contains(&a))) {
                        why.push(format!("`{id}`: {} `{}` has no {} implementation (has: {})", n.role.name(), c.name, a.name(), c.impls_label()));
                    }
                    if let Some(ca) = inherited().filter(|ca| fused && *ca != a) {
                        why.push(format!("`{id}` (api {}) is compiled into callers running {}", a.name(), ca.name()));
                    }
                    Some(a)
                }
                None if fused => inherited(),
                None => None,
            };
            if let Some(a) = api {
                apis.insert(id.to_string(), a);
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        if b.renderer.is_none() {
            return Err(format!("back end `{}` is not available yet", b.name));
        }
        Ok(Checked { graph: g, devices, apis, output: out })
    }
}

/// The ray-casting sites of a checked graph, for the back ends that run it.
impl Checked {
    pub fn node(&self, id: &str) -> &Node {
        &self.graph.nodes[id]
    }

    /// The node making the image the post maps (the output, without a post).
    pub fn shading(&self) -> &str {
        let out = &self.graph.nodes[&self.output];
        if out.role == Role::Post { &out.inputs["image"] } else { &self.output }
    }

    /// The post's tone mapper (`clamp` without a post).
    pub fn tone(&self) -> crate::post::Tone {
        self.graph.tone()
    }

    /// The API node `id` runs with: its own or its callers' (`apis`), else its component's
    /// only one on the node's device (e.g. `rayquery`'s ray queries).
    pub fn api(&self, id: &str) -> Option<Api> {
        self.apis.get(id).copied().or_else(|| {
            let n = self.node(id);
            let kind = self.devices[id].kind();
            let apis: Vec<Api> = component(n.role.slot()?, &n.component)?.impls.iter().filter(|i| i.kind == kind).flat_map(|i| i.apis.iter().copied()).collect();
            match apis[..] {
                [a] => Some(a),
                _ => None,
            }
        })
    }

    /// The node of `role` (strategy, shading, draw, prepass, meshes: one each).
    pub fn find(&self, role: Role) -> Option<&str> {
        self.graph.nodes.iter().find(|(_, n)| n.role == role).map(|(k, _)| k.as_str())
    }

    /// The ids of the nodes of `roles`, in that order: what a timed stage runs (callers only, the storages and kernels fused into them are timed with them).
    /// A role without a node (added live, e.g. a prepass switched on) is named as
    /// [`Graph::from_composition`] names it: by the role.
    pub fn ids(&self, roles: &[Role]) -> Vec<String> {
        roles.iter().map(|&r| self.find(r).unwrap_or(r.name()).to_string()).collect()
    }

    /// The node bound to input `port` of `role`'s node.
    pub fn input(&self, role: Role, port: &str) -> Option<&str> {
        self.graph.nodes.get(self.find(role)?)?.inputs.get(port).map(String::as_str)
    }

    /// Ray-casting site `port` of `role`'s node: its kernel and what the kernel walks.
    pub fn site(&self, role: Role, port: &str) -> Option<Site<'_>> {
        let kernel = self.input(role, port)?;
        Some(Site { kernel, storage: self.graph.walked(kernel)? })
    }
}

/// A ray-casting site's binding: node ids of its kernel and of the storage (or geometry)
/// the kernel walks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Site<'a> {
    pub kernel: &'a str,
    pub storage: &'a str,
}
