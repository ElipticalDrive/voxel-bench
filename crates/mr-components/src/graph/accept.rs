//! Acceptance by the executors (validity, timings, declarations, device and API blockers).

use crate::Api;

use super::*;

impl Graph {
    /// The ray-casting sites (and exposure tests' storages) bound in this graph: caster
    /// node, port, the node bound to it.
    fn sites(&self) -> Vec<(String, &'static str, String)> {
        let mut v = Vec::new();
        for (id, n) in &self.nodes {
            for p in inputs(n.role, &n.component)
                .iter()
                .filter(|p| p.casts || p.name == "exposure")
            {
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
        let of = |role: Role| {
            sites
                .iter()
                .find(|(c, p, _)| self.nodes[c].role == role && *p != "exposure")
                .map(|(c, p, _)| (c.clone(), *p))
        };
        of(Role::Visibility).or_else(|| of(Role::Shading).filter(|(_, p)| *p == "shadow"))
    }

    /// The node a kernel walks (its storage, or geometry for ray queries).
    pub(super) fn walked(&self, kernel: &str) -> Option<&str> {
        let n = self.nodes.get(kernel)?;
        n.inputs
            .get("storage")
            .or(n.inputs.get("geometry"))
            .map(String::as_str)
    }

    /// Whether the graph's executor builds it (`Graph::check`, its timings, the nodes it
    /// declares: [`Checked::builder`], its GPUs and the APIs it declares). Fails with the reason.
    pub fn accept(&self) -> Result<(), String> {
        let ck = self.check()?;
        let region = ck.region.ok_or_else(|| {
            ck.built()
                .err()
                .unwrap_or_else(|| "no region builds this shape".into())
        })?;
        let who = region.who();
        // Timings a region lets the graph choose; every other edge runs now.
        let free = |n: &Node, p: &str| region.free_at(n.role, p);
        for (id, n) in &ck.graph.nodes {
            for p in n.inputs.keys() {
                let w = &n.when(p);
                if let Some(ok) = free(n, p)
                    .or(Some(&[When::Now][..]))
                    .filter(|ok| !ok.contains(w))
                {
                    return Err(format!(
                        "`{id}`.{p}: {who} runs this edge {}, not {}",
                        ok.iter()
                            .map(|w| w.label())
                            .collect::<Vec<_>>()
                            .join(" or "),
                        w.label()
                    ));
                }
            }
        }
        ck.built()?;
        if let Some(why) = device_blocker(&ck)
            .or_else(|| api_blocker(&ck))
            .or_else(|| lookup_blocker(&ck))
        {
            return Err(why);
        }
        Ok(())
    }

    /// Checks the graph and that its executor runs it.
    pub fn validate(&self) -> Result<(), String> {
        self.accept()
    }
}

/// Why the GPUs `ck` places its nodes on cannot run, if they can't: a
/// pipeline runs on one GPU (any slot), or split over two at the hit records: ray casting's
/// shading (`placement = deferred`, with the walks compiled into it) on another GPU than the
/// rest, the records crossing between them (the schedule's transfer), this frame's or the
/// last frame's (`hits` timed `prev`, which only a split runs).
fn device_blocker(ck: &Checked) -> Option<String> {
    let g = &ck.graph;
    let mut slots: Vec<u8> = ck
        .devices
        .values()
        .filter_map(|d| {
            if let Device::Gpu(i) = d {
                Some(*i)
            } else {
                None
            }
        })
        .collect();
    slots.sort();
    slots.dedup();
    if slots.len() < 2 {
        // Hit records a frame late are history worth keeping only between GPUs (they overlap).
        let late = g
            .nodes
            .iter()
            .find(|(_, n)| n.role == Role::Shading && n.when("hits") == When::Prev);
        return late.map(|(id, _)| format!("`{id}`.hits prev: the last frame's hit records are shaded on a second GPU while the first casts this frame's (on one GPU a frame late is only latency: put `{id}` on another GPU, or hits now)"));
    }
    let on = |d: Device| {
        let mut v: Vec<&str> = g
            .nodes
            .keys()
            .map(String::as_str)
            .filter(|id| ck.devices[*id] == d)
            .collect();
        v.sort();
        v.iter()
            .map(|id| format!("`{id}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let used = slots
        .iter()
        .map(|&i| format!("{} ({})", Device::Gpu(i), on(Device::Gpu(i))))
        .collect::<Vec<_>>()
        .join("; ");
    if !ck.region.is_some_and(|b| b.split) {
        return Some(format!(
            "this pipeline runs on one GPU (here: {used}); a pipeline over two GPUs is split at the hit records, ray casting's deferred shading on the second"
        ));
    }
    // The split's ends: the shading and the node its hits come from (ray casting's
    // visibility, the face drawer).
    let shading = g.nodes.iter().find(|(_, n)| n.role == Role::Shading);
    let Some(((sid, sn), (hid, hn))) = shading.and_then(|s| {
        let h = s.1.inputs.get("hits")?;
        Some((s, g.nodes.get_key_value(h)?))
    }) else {
        return Some(format!(
            "a pipeline over two GPUs is split between its shading and the node its hits come from (here: {used})"
        ));
    };
    let (near, far) = (ck.devices[hid], ck.devices[sid]);
    // The nodes on the shading's GPU are the shading and what is compiled into it (the post;
    // fused nodes follow their callers), the rest on the hits' GPU.
    let stray: Vec<String> = g
        .nodes
        .iter()
        .filter(|(id, n)| {
            matches!(ck.devices[*id], Device::Gpu(_))
                && ck.devices[*id] != near
                && (far == near
                    || (*id != sid
                        && n.role != Role::Post
                        && !output(n.role, &n.component).fused()))
        })
        .map(|(id, _)| format!("`{id}` on {}", ck.devices[id]))
        .collect();
    if slots.len() > 2 || far == near || !matches!(near, Device::Gpu(_)) || !stray.is_empty() {
        return Some(format!(
            "only the shading and the walks compiled into it run on a second GPU (here: {used}{})",
            if stray.is_empty() {
                String::new()
            } else {
                format!("; not so: {}", stray.join(", "))
            }
        ));
    }
    let mut why = Vec::new();
    // What crosses is the records: rays write them deferred, a drawer as `gbuffer` or
    // `visbuffer`, splats always (ids decoded into records on the first GPU; forward shading
    // has none).
    match hn.role {
        Role::Draw if !matches!(hn.component.as_str(), "gbuffer" | "visbuffer") => why.push(format!(
            "`{sid}` on {far} shades the hit records `{hid}` draws on {near}: draw = gbuffer or visbuffer"
        )),
        Role::Draw => {}
        // Splatting settles visibility only once every point is in: its ids are deferred by
        // construction, decoded into records on the first GPU as a visbuffer's.
        Role::Visibility if hn.component == "splat" => {}
        _ if sn.knobs.get("placement").and_then(|v| v.as_str()) != Some("deferred") => why.push(format!(
            "`{sid}` on {far} shades the hit records `{hid}` writes on {near}: placement = deferred"
        )),
        _ => {}
    }
    if let Some((id, n)) = g
        .nodes
        .iter()
        .find(|(_, n)| n.role == Role::Meshes && n.component != "none")
    {
        why.push(format!("`{id}` ({}): mesh surfaces are rasterised on {near}, and the shading on {far} has not their triangles (meshes = none)", n.component));
    }
    (!why.is_empty()).then(|| why.join("; "))
}

/// Why a face source cannot draw its faces, if it can't: quads merged across block types
/// (`merge = exposure`) look their blocks up per pixel in the storage bound to the face
/// source's `storage` input; unbound, nothing is looked up.
fn lookup_blocker(ck: &Checked) -> Option<String> {
    let g = &ck.graph;
    g.nodes.iter().find_map(|(id, n)| {
        let faces = n.inputs.get("faces").filter(|_| n.role == Role::Visibility)?;
        let merge = g.nodes[faces].knobs.get("merge").and_then(|v| v.as_str());
        (merge == Some("exposure") && !n.inputs.contains_key("storage")).then(|| {
            format!("`{id}`.storage: not bound, and `{faces}` merges faces across block types (merge = exposure), whose blocks are looked up per pixel in that storage (bind it, or merge = block)")
        })
    })
}

/// Why the APIs set in `ck` cannot run on its executor, if they can't: a non-fused node's
/// API must be one its executor's declaration runs that role with on that device kind
/// ([`Build::at`]), the one a face drawer fixes for the shading behind it (the fragment shader
/// for `raster`, else a deferred compute pass) when the place has two and the API choice does
/// not set it; the stages its `api_choice` sets run with one API.
fn api_blocker(ck: &Checked) -> Option<String> {
    let g = &ck.graph;
    let b = ck.builder()?;
    let name = b.who();
    let drawer = ck.find(Role::Draw).map(|d| g.nodes[d].component.as_str());
    let mut why = Vec::new();
    for (id, &a) in ck.apis.iter().filter(|(id, _)| g.nodes[*id].api.is_some()) {
        // A fused node runs as the stages calling it (through other fused nodes) do.
        let mut stages = vec![id.as_str()];
        while let Some(i) = stages
            .iter()
            .position(|s| output(g.nodes[*s].role, &g.nodes[*s].component).fused())
        {
            let s = stages.remove(i);
            stages.extend(
                g.nodes
                    .iter()
                    .filter(|(_, n)| n.inputs.values().any(|x| x == s))
                    .map(|(k, _)| k.as_str()),
            );
        }
        stages.sort();
        stages.dedup();
        for st in stages {
            let role = g.nodes[st].role;
            let Some(place) = b
                .nodes
                .iter()
                .find(|d| d.role == role && d.at.kind == ck.devices[st].kind())
                .map(|d| d.at)
            else {
                continue;
            };
            let chosen = b.chooses(role);
            let at = if st == id {
                format!("`{id}`")
            } else {
                format!("`{id}` (in `{st}`)")
            };
            let fixed = match place.apis {
                [x] => Some(*x),
                _ if chosen => None,
                _ => drawer.map(|d| {
                    if d == "raster" {
                        Api::Raster
                    } else {
                        Api::Compute
                    }
                }),
            };
            match fixed {
                Some(x) if x != a => why.push(format!(
                    "{at}: {name} runs its {} with {} here, not {}",
                    role.name(),
                    x.name(),
                    a.name()
                )),
                None if !place.apis.contains(&a) => why.push(format!(
                    "{at}: {name} runs its {} with {}, not {}",
                    role.name(),
                    place.label(),
                    a.name()
                )),
                _ => {}
            }
        }
    }
    if let Some(ch) = b.api_choice {
        let mut set: Vec<Api> = ck
            .apis
            .iter()
            .filter(|(id, _)| b.chooses(g.nodes[*id].role))
            .map(|(_, a)| *a)
            .collect();
        set.sort();
        set.dedup();
        if set.len() > 1 {
            why.push(format!(
                "{name} runs its {} with one api, its `{}` (have: {})",
                ch.roles
                    .iter()
                    .map(|s| s.name())
                    .collect::<Vec<_>>()
                    .join(", "),
                ch.knob,
                set.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    (!why.is_empty()).then(|| why.join("; "))
}

/// The graph the `kind` executor (`cpu`, `gpu`) runs: the host's
/// ([`mr_core::InitParams::graph_json`]), checked; its preset is the region that runs it.
pub fn of_init(kind: &str, p: &mr_core::InitParams) -> anyhow::Result<Checked> {
    anyhow::ensure!(
        !p.graph_json.trim().is_empty(),
        "the {kind} executor runs a graph; none was given"
    );
    let g: Graph = serde_json::from_str(&p.graph_json)?;
    anyhow::ensure!(
        g.runs_on() == kind,
        "a graph for the {} executor handed to the {kind} executor",
        g.runs_on()
    );
    anyhow::ensure!(
        g.region().is_some(),
        "the {kind} executor builds no pipeline of this shape"
    );
    g.validate().map_err(anyhow::Error::msg)?;
    g.check().map_err(anyhow::Error::msg)
}
