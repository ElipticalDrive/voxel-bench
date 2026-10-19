//! Conversion from flat compositions and their options to graphs, and the presets a graph's shape names.

use std::collections::BTreeMap;

use crate::{Composition, Kind, Slot, backend};

use super::*;

impl Graph {
    /// The graph of a flat composition (its back end's defaults filled in). Primary and
    /// secondary rays share kernel and storage nodes where the composition says `same`;
    /// secondary rays on another device than the primary ones get their own nodes.
    pub fn from_composition(c: &Composition) -> Result<Graph, String> {
        let mut c = c.clone();
        c.fill_defaults();
        let b = backend(&c.backend).ok_or_else(|| format!("unknown back end `{}`", c.backend))?;
        let has = |s: Slot| b.has(s) && !c.get(s).is_empty();
        let mut nodes = BTreeMap::new();
        let mut add = |id: &str, role: Role, name: &str, inputs: &[(&str, &str)]| {
            nodes.insert(id.to_string(), Node::new(role, name).with(inputs));
        };
        add("world", Role::World, "world", &[]);
        let changes = [("changes", "world")];
        for (slot, role) in [(Slot::Storage, Role::Storage), (Slot::Geometry, Role::Geometry)] {
            if has(slot) {
                add(slot.name(), role, c.get(slot), &changes);
            }
        }
        let prepass = has(Slot::Prepass) && c.prepass != "none";
        if prepass {
            add("prepass", Role::Prepass, &c.prepass, &[("storage", "storage")]);
        }
        let meshes = has(Slot::Meshes) && c.meshes != "none";
        if meshes {
            add("meshes", Role::Meshes, &c.meshes, &changes);
        }
        if has(Slot::Traversal) {
            let on = if c.traversal == "rayquery" { ("geometry", "geometry") } else { ("storage", "storage") };
            add("kernel", Role::Traversal, &c.traversal, &[on]);
        }
        let drawn = has(Slot::Draw);
        let mut vis: Vec<(&str, &str)> = match c.visibility.as_str() {
            "raycast" => vec![("primary", "kernel")],
            "quadtree" | "cluster" => vec![("rays", "kernel"), ("exposure", "storage")],
            "splat" => vec![("points", "geometry")],
            "columns" => vec![("storage", "storage")],
            _ => vec![("faces", "geometry")],
        };
        if prepass {
            vis.push(("start", "prepass"));
        }
        if meshes && !drawn {
            vis.push(("meshes", "meshes"));
        }
        // Hole detection: a loop through the drawer (it draws last frame's faces, discovery
        // looks where they leave holes, it draws again), and the last frame's image.
        let holes = has(Slot::Holes) && c.holes != "none";
        if holes {
            vis.push(("where", "holes"));
            add("holes", Role::Holes, &c.holes, &[("covered", "draw"), ("sky", "draw"), ("changes", "world")]);
        }
        add("visibility", Role::Visibility, &c.visibility, &vis);
        if drawn {
            add("draw", Role::Draw, &c.draw, if meshes { &[("faces", "visibility"), ("meshes", "meshes")] } else { &[("faces", "visibility")] });
        }
        let shadow = if b.has(Slot::ShadowTraversal) {
            // On the primary rays' device, a secondary stage naming the primary's component is `same`.
            let apart = b.place(Slot::ShadowTraversal).kind != b.place(Slot::Traversal).kind;
            let shared = |s: Slot| !apart && c.resolved(s) == c.get(s.source());
            if shared(Slot::ShadowStorage) && shared(Slot::ShadowTraversal) {
                Some("kernel")
            } else {
                let st = if shared(Slot::ShadowStorage) {
                    "storage"
                } else {
                    add("shadow.storage", Role::Storage, c.resolved(Slot::ShadowStorage), &changes);
                    "shadow.storage"
                };
                add("shadow.kernel", Role::Traversal, c.resolved(Slot::ShadowTraversal), &[("storage", st)]);
                Some("shadow.kernel")
            }
        } else {
            has(Slot::Traversal).then_some("kernel")
        };
        let mut shade = vec![("hits", if drawn { "draw" } else { "visibility" })];
        if let Some(k) = shadow {
            shade.extend(inputs(Role::Shading, &c.shading).iter().filter(|p| p.casts).map(|p| (p.name, k)));
        }
        add("shading", Role::Shading, &c.shading, &shade);
        if holes {
            nodes.get_mut("holes").unwrap().timing.insert("sky".into(), When::Prev);
        }
        if let (true, Some(w)) = (drawn, faces_timing(&c.backend, holes)) {
            nodes.get_mut("draw").unwrap().timing.insert("faces".into(), w);
        }
        // Mesh shadows are the shading's shadow rays.
        if let (true, Some(k)) = (meshes, shadow) {
            nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), k.into());
        }
        Ok(Graph { executor: c.backend, nodes })
    }

    /// The preset this graph's shape and devices name (the back ends'
    /// names are presets of graphs, not what a graph is written for): the face drawer's
    /// source (meshing: `gpu-raster` / `cpu-raster` by the drawer's device; discovery:
    /// `gpu-discover` / `cpu-discover` by the sampler's), else the visibility node's
    /// component and device (ray casting: `cpu`, `gpu-rt` when its primary rays are ray
    /// queries, else `gpu-compute`; `columns`; `splat`). Nodes without a device run where
    /// the written executor runs their role. `None` for a shape no preset has.
    pub fn preset(&self) -> Option<&'static str> {
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role, &n.component)).kind();
        let shading = self.nodes.values().find(|n| n.role == Role::Shading)?;
        let hits = self.nodes.get(shading.inputs.get("hits")?)?;
        let gpu = |n: &Node| dev(n) == Kind::Gpu;
        if hits.role == Role::Draw {
            let src = self.nodes.get(hits.inputs.get("faces")?)?;
            return match (src.role, src.component.as_str()) {
                (Role::Visibility, "quadtree" | "cluster") if gpu(hits) => Some(if gpu(src) { "gpu-discover" } else { "cpu-discover" }),
                (Role::Visibility, "faces") => Some(if gpu(hits) { "gpu-raster" } else { "cpu-raster" }),
                _ => None,
            };
        }
        if hits.role != Role::Visibility {
            return None;
        }
        match (hits.component.as_str(), gpu(hits)) {
            ("columns", g) => Some(if g { "gpu-voxelspace" } else { "cpu-voxelspace" }),
            ("splat", true) => Some("gpu-splat"),
            ("raycast", false) => Some("cpu"),
            ("raycast", true) => {
                let rq = hits.inputs.get("primary").and_then(|k| self.nodes.get(k)).is_some_and(|k| k.component == "rayquery");
                Some(if rq { "gpu-rt" } else { "gpu-compute" })
            }
            _ => None,
        }
    }

    /// The graph run by the preset its shape names ([`Self::preset`]; unchanged if it names
    /// none or the written one). Where the preset changes, nodes in stages the written back
    /// end has keep the device it runs them on (fused nodes still follow their callers), so
    /// the graph means what it was written as; the others run where the preset runs them.
    pub fn placed(&self) -> Graph {
        let Some(p) = self.preset().filter(|p| *p != self.executor) else { return self.clone() };
        let mut g = self.clone();
        if let Some(b) = backend(&self.executor) {
            for n in g.nodes.values_mut().filter(|n| n.role.slot().is_some_and(|s| b.has(s)) && known(n.role, &n.component) && !output(n.role, &n.component).fused()) {
                n.device.get_or_insert_with(|| default_device(&self.executor, n.role, &n.component));
            }
        }
        g.executor = p.into();
        g
    }

    /// The graph without the devices that only repeat where its executor runs a node, so
    /// another preset its shape names places them.
    pub fn unpinned(&self) -> Graph {
        let mut g = self.clone();
        for n in g.nodes.values_mut() {
            if n.device == Some(default_device(&self.executor, n.role, &n.component)) {
                n.device = None;
            }
        }
        g
    }

    /// The graph with the edge timings its knobs spell set (`TIMING_KNOBS`: cpu-discover's
    /// `pipelined = false` makes the drawer's faces arrive `now`). Knobs not given keep the
    /// graph's timing.
    pub fn with_options(self, options: &toml::Table) -> Graph {
        let mut g = self.placed();
        for &(_, knob, role, port, on, off) in TIMING_KNOBS.iter().filter(|k| k.0 == g.executor) {
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
        let g = self.placed();
        let mut v = Vec::new();
        for &(_, knob, role, port, on, _) in TIMING_KNOBS.iter().filter(|k| k.0 == g.executor) {
            if let Some(n) = g.nodes.values().find(|n| n.role == role && n.inputs.contains_key(port)) {
                v.push((knob, n.when(port) == on));
            }
        }
        v
    }

    /// The knobs of the skeleton's nodes as the back end's flat options (the nodes on free
    /// sites keep theirs: back ends read them per instance). Two skeleton nodes with
    /// different values of one knob cannot be expressed yet.
    pub fn flat_knobs(&self) -> Result<toml::Table, String> {
        let placed = self.placed();
        if placed.executor != self.executor {
            return placed.flat_knobs();
        }
        let skel = self.normalised()?.skeleton();
        let mut t = toml::Table::new();
        if let Some((b, ch)) = backend(&self.executor).and_then(|b| b.api_choice.map(|ch| (b, ch)))
            && let Some(v) = self.nodes.values().filter(|n| n.role.slot().is_some_and(|sl| b.chooses(sl))).find_map(|n| n.api).and_then(|a| ch.value(a)) {
            t.insert(ch.knob.into(), v.into());
        }
        for (k, v) in self.normalised()?.timing_options() {
            t.insert(k.into(), v.into());
        }
        // (The link's and the client's knobs are theirs: the worker's, not the back end's.)
        for (id, n) in self.nodes.iter().filter(|(id, n)| skel.nodes.contains_key(*id) && n.role.slot().is_some()) {
            for (k, v) in &n.knobs {
                if let Some(old) = t.insert(k.clone(), v.clone()).filter(|old| old != v) {
                    return Err(format!("knob `{k}` set to {old} and {v} (`{id}`): `{}` has one `{k}` for its stages", self.executor));
                }
            }
        }
        Ok(t)
    }
}

impl Graph {
    /// The graph with flat knob values placed on the nodes they belong to (what a
    /// compiled build bakes in, for its cache key): each `(spec, value)` goes to the
    /// skeleton's nodes its `owner` names (the back end's stages; nodes on free sites keep
    /// their own knobs). A knob whose owner is not in the graph is moot here and left out;
    /// a knob without an owner (the back end's own) has no node and is an error.
    pub fn with_knobs(&self, knobs: &[(&mr_core::ParamSpec, toml::Value)]) -> Result<Graph, String> {
        let mut g = self.normalised()?;
        let skel = g.skeleton();
        for (spec, v) in knobs {
            let Some((slot, comps)) = &spec.owner else {
                return Err(format!("knob `{}` belongs to the back end, not to a node", spec.name));
            };
            for (_, n) in g.nodes.iter_mut().filter(|(id, _)| skel.nodes.contains_key(*id)) {
                if n.role.name() == slot && comps.contains(&n.component) {
                    n.knobs.entry(spec.name.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        Ok(g)
    }
}
