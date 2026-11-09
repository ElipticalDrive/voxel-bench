//! Conversion from flat compositions and their options to graphs, and the presets a graph's shape names.

use std::collections::BTreeMap;

use crate::{Composition, Kind, Slot, preset_info};

use super::*;

impl Graph {
    /// The graph of a flat composition (its preset's defaults filled in). Primary and
    /// secondary rays share kernel and storage nodes where the composition says `same`;
    /// secondary rays on another device than the primary ones get their own nodes.
    pub fn from_composition(c: &Composition) -> Result<Graph, String> {
        let mut c = c.clone();
        c.fill_defaults();
        let b = preset_info(&c.preset).ok_or_else(|| format!("unknown preset `{}`", c.preset))?;
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
        if let (true, Some(w)) = (drawn, faces_timing(&c.preset, holes)) {
            nodes.get_mut("draw").unwrap().timing.insert("faces".into(), w);
        }
        // Mesh shadows are the shading's shadow rays.
        if let (true, Some(k)) = (meshes, shadow) {
            nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), k.into());
        }
        // Run by the preset's executor; nodes it would place elsewhere than the preset
        // does get their device written (fused ones follow their callers, the rest of the
        // world's side and the post have their own rules).
        let executor = executor_of(&c.preset);
        for n in nodes.values_mut().filter(|n| n.role.slot().is_some() && known(n.role, &n.component) && !output(n.role, &n.component).fused()) {
            let at = placed_on(&c.preset, n.role, &n.component);
            if at != default_device(executor, n.role, &n.component) {
                n.device = Some(at);
            }
        }
        Ok(Graph { executor: executor.into(), nodes })
    }

    /// The preset this graph's shape and devices name (the region of its executor that
    /// builds it, and the tables keyed by the presets' names): the face drawer's
    /// source (meshing: `raster-gpu` / `raster-cpu` by the drawer's device; discovery:
    /// `discover-gpu` / `discover-cpu` by the sampler's), else the visibility node's
    /// component and device (ray casting: `raycast-cpu`, `rayquery-gpu` when its primary rays are ray
    /// queries (the kernel's component or API), else `raycast-gpu`; `columns`; `splat`). `None` for a
    /// shape no preset has.
    pub fn preset(&self) -> Option<&'static str> {
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role, &n.component)).kind();
        let shading = self.nodes.values().find(|n| n.role == Role::Shading)?;
        let hits = self.nodes.get(shading.inputs.get("hits")?)?;
        let gpu = |n: &Node| dev(n) == Kind::Gpu;
        if hits.role == Role::Draw {
            let src = self.nodes.get(hits.inputs.get("faces")?)?;
            return match (src.role, src.component.as_str()) {
                (Role::Visibility, "quadtree" | "cluster") if gpu(hits) => Some(if gpu(src) { "discover-gpu" } else { "discover-cpu" }),
                (Role::Visibility, "faces") => Some(if gpu(hits) { "raster-gpu" } else { "raster-cpu" }),
                _ => None,
            };
        }
        if hits.role != Role::Visibility {
            return None;
        }
        match (hits.component.as_str(), gpu(hits)) {
            ("columns", g) => Some(if g { "columns-gpu" } else { "columns-cpu" }),
            ("splat", true) => Some("splat-gpu"),
            ("raycast", false) => Some("raycast-cpu"),
            ("raycast", true) => {
                let rq = hits.inputs.get("primary").and_then(|k| self.nodes.get(k)).is_some_and(|k| k.component == "rayquery" || k.api == Some(crate::Api::RayQuery));
                Some(if rq { "rayquery-gpu" } else { "raycast-gpu" })
            }
            _ => None,
        }
    }

    /// The graph with the edge timings its knobs spell set (`TIMING_KNOBS`: discover-cpu's
    /// `pipelined = false` makes the drawer's faces arrive `now`). Knobs not given keep the
    /// graph's timing.
    pub fn with_options(self, options: &toml::Table) -> Graph {
        let mut g = self;
        let preset = g.preset();
        for &(_, knob, role, port, on, off) in TIMING_KNOBS.iter().filter(|k| Some(k.0) == preset) {
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
        let preset = g.preset();
        let mut v = Vec::new();
        for &(_, knob, role, port, on, _) in TIMING_KNOBS.iter().filter(|k| Some(k.0) == preset) {
            if let Some(n) = g.nodes.values().find(|n| n.role == role && n.inputs.contains_key(port)) {
                v.push((knob, n.when(port) == on));
            }
        }
        v
    }

    /// The preset's own knobs this graph spells (not node knobs, which the executors read
    /// from the nodes): the API choice (`isa`, `dispatch`) and the timings (`pipelined`).
    pub fn preset_options(&self) -> Result<toml::Table, String> {
        let preset = self.preset().unwrap_or_default();
        let mut t = toml::Table::new();
        if let Some((b, ch)) = preset_info(preset).and_then(|b| b.api_choice.map(|ch| (b, ch)))
            && let Some(v) = self.nodes.values().filter(|n| n.role.slot().is_some_and(|sl| b.chooses(sl))).find_map(|n| n.api).and_then(|a| ch.value(a)) {
            t.insert(ch.knob.into(), v.into());
        }
        for (k, v) in self.normalised()?.timing_options() {
            t.insert(k.into(), v.into());
        }
        Ok(t)
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
