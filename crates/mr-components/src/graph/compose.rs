//! The region a graph's shape names, the region's own knobs a graph spells, and what a pass can tell apart.

use crate::Kind;

use super::*;

impl Graph {
    /// The region of an executor this graph's shape and devices name (what builds it:
    /// [`Builds`]): the face drawer's source (meshing, by the drawer's device; discovery, by
    /// the sampler's), else the visibility node's component and device (ray casting: on the
    /// CPU, by ray queries when its primary rays are ray queries (the kernel's component or
    /// API), else the GPU walk; columns; splats). `None` for a shape no region has.
    pub fn region(&self) -> Option<&'static Builds> {
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role, &n.component)).kind();
        let shading = self.nodes.values().find(|n| n.role == Role::Shading)?;
        let hits = self.nodes.get(shading.inputs.get("hits")?)?;
        let gpu = |n: &Node| dev(n) == Kind::Gpu;
        if hits.role == Role::Draw {
            let src = self.nodes.get(hits.inputs.get("faces")?)?;
            return match (src.role, src.component.as_str()) {
                (Role::Visibility, "quadtree" | "cluster") if gpu(hits) => Some(if gpu(src) { &GPU_SAMPLER } else { &CPU_SAMPLER }),
                (Role::Visibility, "faces") => Some(if gpu(hits) { &GPU_MESHING } else { &CPU_RASTER }),
                _ => None,
            };
        }
        if hits.role != Role::Visibility {
            return None;
        }
        match (hits.component.as_str(), gpu(hits)) {
            ("columns", g) => Some(if g { &GPU_COLUMNS } else { &CPU_COLUMNS }),
            ("splat", true) => Some(&GPU_SPLAT),
            ("raycast", false) => Some(&CPU_RAYCAST),
            ("raycast", true) => {
                let rq = hits.inputs.get("primary").and_then(|k| self.nodes.get(k)).is_some_and(|k| k.component == "rayquery" || k.api == Some(crate::Api::RayQuery));
                Some(if rq { &GPU_RAYQUERY } else { &GPU_WALK })
            }
            _ => None,
        }
    }

    /// The name of this graph's region ([`Self::region`]).
    pub fn preset(&self) -> Option<&'static str> {
        self.region().map(|b| b.name)
    }

    /// The graph with the edge timings its knobs spell set ([`Builds::timing_knobs`]: discover-cpu's
    /// `pipelined = false` makes the drawer's faces arrive `now`). Knobs not given keep the
    /// graph's timing.
    pub fn with_options(self, options: &toml::Table) -> Graph {
        let mut g = self;
        let knobs = g.region().map_or(&[][..], |b| b.timing_knobs);
        for &(knob, role, port, on, off) in knobs {
            let Some(v) = options.get(knob).and_then(|v| v.as_bool()) else { continue };
            for n in g.nodes.values_mut().filter(|n| n.role == role && n.inputs.contains_key(port)) {
                n.timing.insert(port.into(), if v { on } else { off });
                n.timing.retain(|_, w| *w != When::Now);
            }
        }
        g
    }

    /// The knobs spelling this graph's timings, as `with_options` reads them.
    pub fn timing_options(&self) -> Vec<(&'static str, bool)> {
        let g = self;
        let mut v = Vec::new();
        for &(knob, role, port, on, _) in g.region().map_or(&[][..], |b| b.timing_knobs) {
            if let Some(n) = g.nodes.values().find(|n| n.role == role && n.inputs.contains_key(port)) {
                v.push((knob, n.when(port) == on));
            }
        }
        v
    }

    /// The region's own knobs this graph spells (not node knobs, which the executors read
    /// from the nodes): the API choice (`isa`, `dispatch`) and the timings (`pipelined`).
    pub fn preset_options(&self) -> Result<toml::Table, String> {
        let mut t = toml::Table::new();
        if let Some((b, ch)) = self.region().and_then(|b| b.api_choice.map(|ch| (b, ch)))
            && let Some(v) = self.nodes.values().filter(|n| n.role.slot().is_some_and(|sl| b.chooses(sl))).find_map(|n| n.api).and_then(|a| ch.value(a)) {
            t.insert(ch.knob.into(), v.into());
        }
        for (k, v) in self.normalised()?.timing_options() {
            t.insert(k.into(), v.into());
        }
        Ok(t)
    }

    /// The graph as far as a pass of mode `m` can tell it from another: what the pass does
    /// not evaluate is left out. Unlit, a shading node shows the texel of its hits, whatever
    /// its component, and casts no rays (its other inputs go, and the mesh pass's shadow
    /// rays); without meshes nothing reads a meshes node. The nodes nothing reads any more
    /// go too. Two graphs equal here render the same frames in such a pass (a sweep runs one
    /// of them); the result is for comparing, not for running.
    pub fn told_apart(&self, m: mr_core::PassMode) -> Graph {
        let unlit = m.lighting == mr_core::RenderMode::Unlit;
        let read = |g: &Graph| g.nodes.values().flat_map(|n| n.inputs.values().cloned()).collect::<std::collections::HashSet<_>>();
        // What nothing reads in the graph as it is stays (the output, an unread world).
        let ends: Vec<String> = self.nodes.keys().filter(|k| !read(self).contains(*k)).cloned().collect();
        let mut g = self.clone();
        let meshes: Vec<String> = g.nodes.iter().filter(|(_, n)| n.role == Role::Meshes).map(|(k, _)| k.clone()).collect();
        for n in g.nodes.values_mut() {
            if unlit && n.role == Role::Shading {
                n.inputs.retain(|p, _| p == "hits");
                n.component.clear();
            }
            if unlit && n.role == Role::Meshes {
                n.inputs.remove("shadow");
            }
            if !m.meshes {
                n.inputs.retain(|_, from| !meshes.contains(from));
            }
            let Node { inputs, timing, .. } = n;
            timing.retain(|p, _| inputs.contains_key(p));
        }
        let mut kept = std::collections::HashSet::new();
        let mut todo = ends;
        while let Some(k) = todo.pop() {
            if let Some(n) = g.nodes.get(&k)
                && kept.insert(k) {
                todo.extend(n.inputs.values().cloned());
            }
        }
        g.nodes.retain(|k, _| kept.contains(k));
        g
    }
}

impl Graph {
    /// The knobs set on the stage nodes (`stage_nodes`; the first value of each),
    /// as a preset-wide knob list shows them.
    pub fn stage_knobs(&self) -> Result<toml::Table, String> {
        let g = self.normalised()?;
        let stages = g.stage_nodes();
        let mut t = toml::Table::new();
        for (_, n) in g.nodes.iter().filter(|(id, n)| stages.contains(*id) && n.role.slot().is_some()) {
            for (k, v) in &n.knobs {
                t.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        Ok(t)
    }

    /// The graph with the options that are node knobs placed on the nodes they belong to
    /// (what the executors read, and what a compiled build bakes in, for its cache key): each
    /// `(spec, value)` replaces the knob on the stage nodes its `owner` names (`stage_nodes`;
    /// nodes on sites of their own keep their own knobs). A knob whose owner is not
    /// in the graph is moot here and left out;
    /// a knob without an owner (the preset's own) has no node and is an error.
    pub fn with_knobs(&self, knobs: &[(&mr_core::ParamSpec, toml::Value)]) -> Result<Graph, String> {
        let mut g = self.normalised()?;
        let stages = g.stage_nodes();
        for (spec, v) in knobs {
            let Some((slot, comps)) = &spec.owner else {
                return Err(format!("knob `{}` belongs to the preset, not to a node", spec.name));
            };
            for (_, n) in g.nodes.iter_mut().filter(|(id, _)| stages.contains(*id)) {
                if n.role.name() == slot && comps.contains(&n.component) {
                    n.knobs.insert(spec.name.clone(), v.clone());
                }
            }
        }
        Ok(g)
    }
}
