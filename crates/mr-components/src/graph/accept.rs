//! Acceptance by the executors (validity, timings, declarations, device and API blockers)
//! and the nearest flat composition (the skeleton).

use crate::{Api, Composition, Slot, backend};

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
        let preset = g.preset().unwrap_or_default();
        let lead = g.lead();
        let lead_storage = lead.as_ref().and_then(|(c, p)| g.nodes[c].inputs.get(*p)).and_then(|k| g.walked(k)).map(String::from);
        for (caster, port, _) in g.sites() {
            if lead.as_ref().is_some_and(|(c, p)| *c == caster && *p == port) || follows_lead(preset, g.nodes[&caster].role, port) {
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

    /// Whether the graph's executor builds it (`Graph::check`, its timings, the nodes it
    /// declares: [`Checked::builder`], its GPUs and APIs), with the nearest flat composition
    /// (its skeleton's, else the back end's default; for labels and the flat options) and
    /// whether that composition is exactly the graph. Fails with the reason.
    pub fn accept(&self) -> Result<(Composition, bool), String> {
        let ck = self.check()?;
        let preset = ck.preset.ok_or_else(|| ck.built().err().unwrap_or_else(|| "no preset has this shape".into()))?;
        // Timings an executor lets the graph choose; every other edge runs now.
        let free = |n: &Node, p: &str| FREE_TIMINGS.iter().find(|(e, r, port, _)| *e == preset && *r == n.role && *port == p).map(|f| f.3);
        for (id, n) in &ck.graph.nodes {
            for p in n.inputs.keys() {
                let w = &n.when(p);
                if let Some(ok) = free(n, p).or(Some(&[When::Now])).filter(|ok| !ok.contains(w)) {
                    return Err(format!("`{id}`.{p}: `{preset}` runs this edge {}, not {}", ok.iter().map(|w| w.label()).collect::<Vec<_>>().join(" or "), w.label()));
                }
            }
        }
        ck.built()?;
        if let Some(why) = device_blocker(&ck) {
            return Err(why);
        }
        let mut c = ck.graph.skeleton().skeleton_composition().unwrap_or_else(|_| {
            let mut c = Composition::new(preset);
            c.fill_defaults();
            c
        });
        // Without knobs and APIs (the composition has neither; `cpu` reads its API as `isa`)
        // and with free timings as `prev` (their knobs: see `TIMING_KNOBS`).
        let strip = |g: &Graph| {
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
                // A free timing compares as `prev` (whichever it is; a cycle stays closed).
                for (_, _, port, _) in FREE_TIMINGS.iter().filter(|(e, r, _, _)| *e == preset && *r == role) {
                    if n.inputs.contains_key(*port) {
                        n.timing.insert(port.to_string(), When::Prev);
                    }
                }
            });
            g.canonical()
        };
        // The secondary-ray stages where the composition has them.
        let b = backend(preset).unwrap();
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
        let preset = ck.preset.ok_or("no preset has this shape")?;
        let mut c = Composition { backend: preset.into(), ..Composition::new("") };
        let b = backend(preset).unwrap();
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
            return Err(format!("`{}` draws faces straight from `{vis}`: this back end needs a visibility stage between them", preset));
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
    if !matches!(ck.preset, Some("gpu-compute" | "gpu-rt")) {
        return Some(format!("`{}` runs on one GPU (here: {used}); a pipeline over two GPUs is split at the hit records, ray casting's deferred shading on the second", ck.preset.unwrap_or("this pipeline")));
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
    let preset = g.preset();
    anyhow::ensure!(preset == Some(backend), "a `{}` graph handed to `{backend}`", preset.unwrap_or("shapeless"));
    g.validate().map_err(anyhow::Error::msg)?;
    g.check().map_err(anyhow::Error::msg)
}

/// Whether back end `executor`'s composition casts the `port` site of its `role` node
/// through its lead kernel (or for exposure tests, reads the lead kernel's storage), so the
/// skeleton rebinds it there; the other sites are the graph's own.
fn follows_lead(executor: &str, role: Role, port: &str) -> bool {
    !matches!(
        (executor, role, port),
        ("gpu-compute", Role::Shading | Role::Meshes, _)
            | ("gpu-rt", Role::Shading, _)
            | ("gpu-raster" | "gpu-splat", Role::Shading, "bounce")
            | ("gpu-raster", Role::Meshes, "shadow")
            | ("gpu-discover", Role::Visibility, "exposure")
            | ("gpu-discover" | "cpu-discover", Role::Shading, _)
            | ("cpu", Role::Shading, "shadow")
    )
}
