//! Acceptance by back ends: the skeleton, site bindings, device and API blockers.

use crate::{Api, Composition, Kind, Slot, backend, component};

use super::*;

impl Graph {
    /// The ray-casting sites (and exposure tests' storages) bound in this graph: caster
    /// node, port, the node bound to it.
    fn sites(&self) -> Vec<(String, &'static str, String)> {
        let mut v = Vec::new();
        for (id, n) in &self.nodes {
            for p in inputs(n.role, &n.component).iter().filter(|p| p.casts || p.name == "exposure") {
                if let Some(src) = n.inputs.get(p.name) {
                    v.push((id.clone(), p.name, src.clone()));
                }
            }
        }
        v
    }

    /// The lead site: the strategy's own rays (primary or discovery rays), else the shading's
    /// shadow rays (rasterisers, whose storage and kernel stages are the shadow rays').
    pub(super) fn lead(&self) -> Option<(String, &'static str)> {
        let sites = self.sites();
        let of = |role: Role| sites.iter().find(|(c, p, _)| self.nodes[c].role == role && *p != "exposure").map(|(c, p, _)| (c.clone(), *p));
        of(Role::Visibility).or_else(|| of(Role::Shading).filter(|(_, p)| *p == "shadow"))
    }

    /// The node a kernel walks (its storage, or geometry for ray queries).
    pub(super) fn walked(&self, kernel: &str) -> Option<&str> {
        let n = self.nodes.get(kernel)?;
        n.inputs.get("storage").or(n.inputs.get("geometry")).map(String::as_str)
    }

    /// The skeleton: free sites rebound as a flat composition binds them (exposure tests
    /// to the lead kernel's storage, secondary rays unbound so they follow the lead), and
    /// the nodes only they reached dropped.
    pub(super) fn skeleton(&self) -> Graph {
        let mut g = self.clone();
        let lead = g.lead();
        let lead_storage = lead.as_ref().and_then(|(c, p)| g.nodes[c].inputs.get(*p)).and_then(|k| g.walked(k)).map(String::from);
        for (caster, port, _) in g.sites() {
            if lead.as_ref().is_some_and(|(c, p)| *c == caster && *p == port) || site_binding(&g.executor, g.nodes[&caster].role, port) == Bind::Lead {
                continue;
            }
            let n = g.nodes.get_mut(&caster).unwrap();
            match (port, &lead_storage) {
                ("exposure", Some(s)) => drop(n.inputs.insert(port.into(), s.clone())),
                ("exposure", None) => {}
                _ => drop(n.inputs.remove(port)),
            }
        }
        g.pruned()
    }

    /// Why a site bound away from the lead cannot run on this back end, if it can't.
    fn site_blocker(&self, ck: &Checked) -> Option<String> {
        let g = &ck.graph;
        let b = backend(&g.executor)?;
        let lead = g.lead();
        let lead_kernel = lead.as_ref().and_then(|(c, p)| g.nodes[c].inputs.get(*p)).cloned();
        let lead_storage = lead_kernel.as_deref().and_then(|k| g.walked(k)).map(String::from);
        let mut why = Vec::new();
        for (caster, port, src) in g.sites() {
            if lead.as_ref().is_some_and(|(c, p)| *c == caster && *p == port) {
                continue;
            }
            let role = g.nodes[&caster].role;
            let at = format!("`{caster}`.{port}");
            match site_binding(&g.executor, role, port) {
                Bind::Lead if port == "exposure" => {
                    if Some(&src) != lead_storage.as_ref() {
                        why.push(format!("{at}: `{}` reads exposure from the storage its rays walk (`{}`)", g.executor, lead_storage.as_deref().unwrap_or("-")));
                    }
                }
                Bind::Lead => {
                    if Some(&src) != lead_kernel.as_ref() {
                        why.push(format!("{at}: `{}` casts these rays through its {} kernel (`{}`)", g.executor, lead.as_ref().map_or("own", |l| l.1), lead_kernel.as_deref().unwrap_or("-")));
                    }
                }
                bind => {
                    // On the GPU, any GPU implementation (the GPU executor compiles walks into
                    // any of its lanes and builds ray queries' acceleration structures). Else
                    // secondary rays run where the back end runs its shadow stages, the others
                    // with its storage and kernel.
                    let secondary = matches!(role, Role::Shading | Role::Meshes);
                    let need = |slot: Slot, id: &str| {
                        let comp = &g.nodes[id].component;
                        if ck.devices[id].kind() == Kind::Gpu && slot != Slot::Geometry {
                            let c = component(slot, comp)?;
                            return (!c.impls.iter().any(|i| i.kind == Kind::Gpu)).then(|| format!("{at} → `{id}`: `{comp}` has no GPU implementation (has: {})", c.impls_label()));
                        }
                        let at_slot = match slot {
                            Slot::Storage if secondary => Slot::ShadowStorage,
                            Slot::Traversal if secondary => Slot::ShadowTraversal,
                            s => s,
                        };
                        crate::unimplemented_at(&b.place(at_slot), slot, comp).map(|w| format!("{at} → `{id}`: {w}"))
                    };
                    if port == "exposure" {
                        why.extend(need(Slot::Storage, &src));
                        continue;
                    }
                    why.extend(need(Slot::Traversal, &src));
                    // Ray queries trace acceleration structures the GPU executor builds on the
                    // CPU from a geometry node (one per pipeline), not in the face drawer's
                    // mesh pipelines.
                    let query = g.walked(&src).filter(|w| g.nodes[*w].role == Role::Geometry);
                    if let Some(geo) = query {
                        let name = &g.nodes[geo].component;
                        if crate::geometry::rt(name).is_none() {
                            why.push(format!("{at} → `{geo}`: geometry `{name}` builds no acceleration structure for ray queries (have: greedy, naive, boxes)"));
                        }
                        if ck.devices[geo] != Device::Cpu {
                            why.push(format!("{at} → `{geo}`: acceleration structures are built from geometry meshed on the CPU (here: {})", ck.devices[geo]));
                        }
                        if role == Role::Meshes && b.has(Slot::Draw) {
                            why.push(format!("{at}: `{}` lights meshes through walks (its mesh pipelines have no ray queries)", g.executor));
                        }
                        let other = g.sites().into_iter().find_map(|(_, _, k)| g.walked(&k).filter(|w| g.nodes[*w].role == Role::Geometry && *w != geo).map(String::from));
                        if let Some(o) = other {
                            why.push(format!("{at}: ray queries over two geometry nodes (`{geo}`, `{o}`): one acceleration structure per pipeline"));
                        }
                        continue;
                    }
                    if let Some(st) = g.walked(&src) {
                        if let Some(slot) = g.nodes[st].role.slot() {
                            why.extend(need(slot, st));
                        }
                        if bind == Bind::SameStorage && Some(st) != lead_storage.as_deref() {
                            why.push(format!("{at}: `{}` walks these rays over the storage of its {} rays (`{}`; another storage needs a pipeline compiled for it)", g.executor, lead.as_ref().map_or("own", |l| l.1), lead_storage.as_deref().unwrap_or("-")));
                        }
                    }
                }
            }
        }
        // Back ends that cast shadow rays through a kernel need one bound.
        if b.has(Slot::Traversal)
            && let Some(n) = g.nodes.values().find(|n| n.role == Role::Shading)
            && !n.inputs.contains_key("shadow") {
            why.push(format!("shading `{}`: input `shadow` (shadow rays) is not bound, and `{}` has no rays of its own on the shading's device", n.component, g.executor));
        }
        (!why.is_empty()).then(|| why.join("; "))
    }

    /// The flat composition of the graph's skeleton (everything but its free sites), and
    /// whether it is exactly the graph (else the graph is run as such: the composition is
    /// the nearest, for labels and the flat options). Fails with the reasons if the back end
    /// cannot run the graph.
    pub fn accept(&self) -> Result<(Composition, bool), String> {
        let placed = self.placed();
        if placed.executor != self.executor {
            return placed.accept();
        }
        let ck = self.check()?;
        // Timings a back end lets the graph choose; the others must be its composition's.
        let free = |n: &Node, p: &str| FREE_TIMINGS.iter().find(|(e, r, port, _)| *e == self.executor && *r == n.role && *port == p).map(|f| f.3);
        for (id, n) in &ck.graph.nodes {
            for (p, w) in &n.timing {
                if let Some(ok) = free(n, p).filter(|ok| !ok.contains(w)) {
                    return Err(format!("`{id}`.{p}: `{}` runs this edge {}, not {}", self.executor, ok.iter().map(|w| w.label()).collect::<Vec<_>>().join(" or "), w.label()));
                }
            }
        }
        if let Some(why) = self.site_blocker(&ck).or_else(|| device_blocker(&ck)) {
            return Err(why);
        }
        let skel = ck.graph.skeleton();
        let mut c = skel.skeleton_composition()?;
        // Without knobs and APIs (the composition has neither; `cpu` reads its API as `isa`)
        // and free timings (its knobs: see `TIMING_KNOBS`); `all` makes every timed edge
        // `prev` (the graphs' shapes compared, whatever their timings).
        let strip_with = |g: &Graph, all: bool| {
            let mut g = g.clone();
            g.nodes.values_mut().for_each(|n| {
                n.knobs.clear();
                n.api = None;
                // The worker runs the link, the client and a generator on the client, whichever
                // they are; the back end sees the client's messages.
                match n.role {
                    Role::Client => n.component = "pick".into(),
                    Role::Link => n.component = "free".into(),
                    Role::Generator => n.device = None,
                    // Every executor compiles in any tone mapper.
                    Role::Post => n.component = "clamp".into(),
                    _ => {}
                }
                let role = n.role;
                n.timing.retain(|p, _| all || !FREE_TIMINGS.iter().any(|(e, r, port, _)| *e == self.executor && *r == role && port == p));
                if all {
                    n.timing.values_mut().for_each(|w| *w = When::Prev);
                }
            });
            g.canonical()
        };
        let strip = |g: &Graph| strip_with(g, false);
        // The shape on one GPU: which GPU each node runs on is `device_blocker`'s.
        let one_gpu = |g: &Graph| {
            let mut g = g.clone();
            g.nodes.values_mut().filter(|n| matches!(n.device, Some(Device::Gpu(_)))).for_each(|n| n.device = Some(Device::Gpu(0)));
            g
        };
        let expected = Graph::from_composition(&c)?;
        if strip(&expected.skeleton())? != strip(&one_gpu(&skel))? {
            if strip_with(&expected.skeleton(), true)? == strip_with(&one_gpu(&skel), true)? {
                let timed = |g: &Graph| {
                    let v: Vec<String> = g.nodes.iter().flat_map(|(id, n)| n.timing.iter().map(move |(p, w)| format!("`{id}`.{p} {}", w.label()))).collect();
                    if v.is_empty() { "every edge now".to_string() } else { v.join(", ") }
                };
                return Err(format!("`{}` runs `{}` with {} (here: {})", self.executor, c.label(), timed(&expected), timed(&ck.graph)));
            }
            return Err(format!("`{}` cannot run this graph; the nearest it runs is `{}`", self.executor, c.label()));
        }
        // The secondary-ray stages where the composition has them.
        let b = backend(&self.executor).unwrap();
        let g = &ck.graph;
        let shading = ck.shading();
        if let (true, Some(s), Some(k)) = (b.has(Slot::ShadowTraversal), g.nodes[shading].inputs.get("shadow"), g.lead().and_then(|(c, p)| g.nodes[&c].inputs.get(p).cloned())) {
            let mut e = c.clone();
            if *s == k {
                e.shadow_storage = "same".into();
                e.shadow_traversal = "same".into();
            } else {
                e.shadow_traversal = if g.nodes[s].component == g.nodes[&k].component { "same".into() } else { g.nodes[s].component.clone() };
                let (ss, ps) = (g.walked(s).unwrap_or(""), g.walked(&k).unwrap_or(""));
                e.shadow_storage = if ss == ps { "same".into() } else { g.nodes.get(ss).map_or(String::new(), |n| n.component.clone()) };
            }
            if e.validate().is_ok() {
                c = e;
            }
        }
        if let Some(why) = api_blocker(b, &ck, &c) {
            return Err(why);
        }
        let exact = strip(&Graph::from_composition(&c)?)? == strip(self)?;
        Ok((c, exact))
    }

    /// The composition the back end runs the graph as (see [`Self::accept`]).
    pub fn to_composition(&self) -> Result<Composition, String> {
        self.accept().map(|(c, _)| c)
    }

    /// A skeleton's flat composition: each stage node in its slot.
    fn skeleton_composition(&self) -> Result<Composition, String> {
        let ck = self.check()?;
        let g = &ck.graph;
        let n = |id: &str| &g.nodes[id];
        let input = |id: &str, p: &str| n(id).inputs.get(p).map(String::as_str);
        let mut c = Composition { backend: g.executor.clone(), ..Composition::new("") };
        let b = backend(&g.executor).unwrap();
        let shading = ck.shading();
        c.shading = n(shading).component.clone();
        let hits = input(shading, "hits").unwrap();
        let vis = if n(hits).role == Role::Draw {
            c.draw = n(hits).component.clone();
            input(hits, "faces").unwrap()
        } else {
            hits
        };
        if n(vis).role != Role::Visibility {
            return Err(format!("`{}` draws faces straight from `{vis}`: this back end needs a visibility stage between them", g.executor));
        }
        c.visibility = n(vis).component.clone();
        let take = |c: &mut Composition, id: &str| {
            if let Some(slot) = n(id).role.slot() {
                c.set(slot, &n(id).component);
            }
        };
        n(vis).inputs.values().for_each(|src| take(&mut c, src));
        if n(hits).role == Role::Draw {
            input(hits, "meshes").into_iter().for_each(|m| take(&mut c, m));
        }
        // The lead kernel's storage; rasterisers' only rays are the shadow rays.
        if let Some((caster, port)) = g.lead() {
            let k = input(&caster, port).unwrap();
            take(&mut c, k);
            if let Some(st) = g.walked(k) {
                take(&mut c, st);
            }
        }
        // Stages a back end has but the graph leaves out are at their "off" component.
        for (slot, off) in [(Slot::Prepass, "none"), (Slot::Meshes, "none")] {
            if b.has(slot) && c.get(slot).is_empty() {
                c.set(slot, off);
            }
        }
        c.fill_defaults();
        c.validate()?;
        Ok(c)
    }

    /// Checks the graph and that its back end runs it.
    pub fn validate(&self) -> Result<(), String> {
        self.accept().map(|_| ())
    }
}

/// Why the GPUs `ck` places its nodes on cannot run, if they can't: a
/// pipeline runs on one GPU (any slot), or split over two at the hit records: ray casting's
/// shading (`placement = deferred`, with the walks compiled into it) on another GPU than the
/// rest, the records crossing between them (the schedule's transfer), this frame's or the
/// last frame's (`hits` timed `prev`, which only a split runs).
fn device_blocker(ck: &Checked) -> Option<String> {
    let g = &ck.graph;
    let mut slots: Vec<u8> = ck.devices.values().filter_map(|d| if let Device::Gpu(i) = d { Some(*i) } else { None }).collect();
    slots.sort();
    slots.dedup();
    if slots.len() < 2 {
        // Hit records a frame late are history worth keeping only between GPUs (they overlap).
        let late = g.nodes.iter().find(|(_, n)| n.role == Role::Shading && n.when("hits") == When::Prev);
        return late.map(|(id, _)| format!("`{id}`.hits prev: the last frame's hit records are shaded on a second GPU while the first casts this frame's (on one GPU a frame late is only latency: put `{id}` on another GPU, or hits now)"));
    }
    let on = |d: Device| {
        let mut v: Vec<&str> = g.nodes.keys().map(String::as_str).filter(|id| ck.devices[*id] == d).collect();
        v.sort();
        v.iter().map(|id| format!("`{id}`")).collect::<Vec<_>>().join(", ")
    };
    let used = slots.iter().map(|&i| format!("{} ({})", Device::Gpu(i), on(Device::Gpu(i)))).collect::<Vec<_>>().join("; ");
    if !matches!(g.executor.as_str(), "gpu-compute" | "gpu-rt") {
        return Some(format!("`{}` runs on one GPU (here: {used}); a pipeline over two GPUs is split at the hit records, ray casting's deferred shading on the second", g.executor));
    }
    let (Some(vis), Some(shading)) = (g.nodes.iter().find(|(_, n)| n.role == Role::Visibility), g.nodes.iter().find(|(_, n)| n.role == Role::Shading)) else {
        return Some(format!("a pipeline over two GPUs is split between its visibility and shading nodes (here: {used})"));
    };
    let (near, far) = (ck.devices[vis.0], ck.devices[shading.0]);
    // The nodes on the shading's GPU are the shading and what is compiled into it (the post;
    // fused nodes follow their callers), the rest on the visibility's.
    let stray: Vec<String> = g.nodes.iter().filter(|(id, n)| matches!(ck.devices[*id], Device::Gpu(_)) && ck.devices[*id] != near && (far == near || (*id != shading.0 && n.role != Role::Post && !output(n.role, &n.component).fused()))).map(|(id, _)| format!("`{id}` on {}", ck.devices[id])).collect();
    if slots.len() > 2 || far == near || !stray.is_empty() {
        return Some(format!("only the shading and the walks compiled into it run on a second GPU (here: {used}{})", if stray.is_empty() { String::new() } else { format!("; not so: {}", stray.join(", ")) }));
    }
    let mut why = Vec::new();
    if shading.1.knobs.get("placement").and_then(|v| v.as_str()) != Some("deferred") {
        why.push(format!("`{}` on {far} shades the hit records `{}` writes on {near}: placement = deferred", shading.0, vis.0));
    }
    if let Some((id, n)) = g.nodes.iter().find(|(_, n)| n.role == Role::Meshes && n.component != "none") {
        why.push(format!("`{id}` ({}): mesh surfaces are rasterised on {near}, and the shading on {far} has not their triangles (meshes = none)", n.component));
    }
    (!why.is_empty()).then(|| why.join("; "))
}

/// Why the APIs set in `ck` cannot run on back end `b` running composition `c`, if they
/// can't: a non-fused node's API must be the one the back end runs its stage with, or one
/// of its choices; the stages its `api_choice` sets run with one API.
fn api_blocker(b: &crate::BackendInfo, ck: &Checked, c: &Composition) -> Option<String> {
    let g = &ck.graph;
    let mut why = Vec::new();
    for (id, &a) in ck.apis.iter().filter(|(id, _)| g.nodes[*id].api.is_some()) {
        // A fused node runs as the stages calling it (through other fused nodes) do.
        let mut stages = vec![id.as_str()];
        while let Some(i) = stages.iter().position(|s| output(g.nodes[*s].role, &g.nodes[*s].component).fused()) {
            let s = stages.remove(i);
            stages.extend(g.nodes.iter().filter(|(_, n)| n.inputs.values().any(|x| x == s)).map(|(k, _)| k.as_str()));
        }
        stages.sort();
        stages.dedup();
        for st in stages {
            let Some(slot) = g.nodes[st].role.slot() else { continue };
            let place = b.place(slot);
            let at = if st == id { format!("`{id}`") } else { format!("`{id}` (in `{st}`)") };
            match b.api(slot, c) {
                Some(x) if x != a => why.push(format!("{at}: `{}` runs its {} with {} here, not {}", b.name, slot.name(), x.name(), a.name())),
                None if !place.apis.contains(&a) => why.push(format!("{at}: `{}` runs its {} with {}, not {}", b.name, slot.name(), place.label(), a.name())),
                _ => {}
            }
        }
    }
    if let Some(ch) = b.api_choice {
        let mut set: Vec<Api> = ck.apis.iter().filter(|(id, _)| g.nodes[*id].role.slot().is_some_and(|sl| b.chooses(sl))).map(|(_, a)| *a).collect();
        set.sort();
        set.dedup();
        if set.len() > 1 {
            why.push(format!("`{}` runs its {} with one api, its `{}` (have: {})", b.name, ch.slots.iter().map(|s| s.name()).collect::<Vec<_>>().join(", "), ch.knob, set.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")));
        }
    }
    (!why.is_empty()).then(|| why.join("; "))
}

/// The graph back end `backend` runs: the host's
/// ([`mr_core::InitParams::graph_json`]), else the graph of the flat composition in its
/// options (its slots by name), checked and accepted.
pub fn of_init(backend: &str, p: &mr_core::InitParams) -> anyhow::Result<Checked> {
    let g: Graph = if p.graph_json.trim().is_empty() {
        let opts: serde_json::Map<String, serde_json::Value> = if p.options_json.trim().is_empty() { Default::default() } else { serde_json::from_str(&p.options_json)? };
        let mut c = Composition::new(backend);
        for s in c.slots().collect::<Vec<_>>() {
            if let Some(v) = opts.get(s.name()).and_then(|v| v.as_str()) {
                c.set(s, v);
            }
        }
        let knobs: toml::Table = serde_json::from_value(serde_json::Value::Object(opts)).unwrap_or_default();
        Graph::from_composition(&c).map_err(anyhow::Error::msg)?.with_options(&knobs)
    } else {
        serde_json::from_str(&p.graph_json)?
    };
    let g = g.placed();
    anyhow::ensure!(g.executor == backend, "a `{}` graph handed to `{backend}`", g.executor);
    g.validate().map_err(anyhow::Error::msg)?;
    g.check().map_err(anyhow::Error::msg)
}

/// How a back end binds a site that is not its lead.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Bind {
    /// Any kernel and storage implemented where the rays run.
    Free,
    /// Its own kernel over the lead's storage (the `cpu` back end's precompiled build: one
    /// storage type per instance).
    SameStorage,
    /// The lead's kernel (or for exposure tests, the lead kernel's storage).
    Lead,
}

/// How back end `executor` binds the `port` site of its `role` node.
fn site_binding(executor: &str, role: Role, port: &str) -> Bind {
    match (executor, role, port) {
        ("gpu-compute", Role::Shading | Role::Meshes, _) | ("gpu-rt", Role::Shading, _) => Bind::Free,
        ("gpu-raster" | "gpu-splat", Role::Shading, "bounce") | ("gpu-raster", Role::Meshes, "shadow") => Bind::Free,
        ("gpu-discover", Role::Visibility, "exposure") | ("gpu-discover" | "cpu-discover", Role::Shading, _) => Bind::Free,
        ("cpu", Role::Shading, "shadow") => Bind::SameStorage,
        _ => Bind::Lead,
    }
}
