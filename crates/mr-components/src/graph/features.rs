//! What a graph's pipeline supports: its region's features ([`mr_core::Features`], declared
//! by the executor) less what this graph's nodes and components lack; and which of the knobs
//! asked of it it leaves out ([`Checked::knobs_left_out`]).

use mr_core::{Feature, Features};

use super::{Checked, Role};
use crate::Kind;
use crate::caps::{Cap, provides};

impl Checked {
    /// Narrows `f`, its region's features, to this graph: meshes need a meshes node that
    /// draws them; mesh rays need meshes and drawers whose shading meets the triangles
    /// ([`Cap::MeshRays`]); LOD needs every storage, traversal and prepass (its sites' too) to
    /// handle LOD chunks ([`Cap::Lod`]; a prepass of `tile = 1`, which does not run, still
    /// counts).
    /// Lit passes need, where stateful shading runs in a forward drawer's fragment shader,
    /// forced early depth tests on that drawer's GPU (the adapter `devices` names for its
    /// slot; see [`Checked::narrow_on`]).
    /// Transparency needs every visibility and draw node to give more than one surface per
    /// pixel ([`Cap::Layers`]); their reasons are said even where the region does not build it,
    /// so a report names the node that stands in the way.
    pub fn narrow(&self, f: &mut Features, devices: &mr_core::DeviceConfig) {
        self.narrow_on(f, &|slot| mr_gpu::adapter_features(devices.gpu(slot as usize)));
    }

    /// [`Checked::narrow`] with the features of each GPU slot from `gpu` (`None`: unknown,
    /// taken to have what is asked). Only graphs that shade statefully in a fragment shader
    /// ask it.
    pub fn narrow_on(&self, f: &mut Features, gpu: &dyn Fn(u8) -> Option<mr_gpu::wgpu::Features>) {
        let nodes = &self.graph.nodes;
        if f.has(Feature::Lit) {
            let mut ids: Vec<&String> = nodes.keys().collect();
            ids.sort();
            for id in ids {
                let n = &nodes[id];
                let Some(draw) = n.inputs.get("hits").filter(|_| n.role == Role::Shading).and_then(|d| nodes.get(d).map(|x| (d, x))) else { continue };
                let forward = draw.1.role == Role::Draw && crate::draw::drawer::DrawMode::parse(&draw.1.component).is_some_and(|m| !m.deferred());
                let Some(crate::graph::Device::Gpu(slot)) = self.devices.get(draw.0).copied().filter(|_| forward && crate::shading::state::stateful(&n.component)) else { continue };
                if let Some(what) = crate::shading::state::fragment_lacks(&n.component, gpu(slot)) {
                    let dev = crate::graph::Device::Gpu(slot);
                    f.lack(Feature::Lit, format!("shading `{}` (`{id}`) in draw `{}` (`{}`)'s fragment shader needs {what}, which {dev} lacks", n.component, draw.1.component, draw.0));
                }
            }
        }
        if f.has(Feature::Lod) {
            let mut ids: Vec<&String> = nodes.keys().collect();
            ids.sort();
            let why: Vec<String> = ids
                .into_iter()
                .filter(|id| matches!(nodes[*id].role, Role::Storage | Role::Traversal | Role::Prepass) && nodes[*id].component != "none")
                .filter(|id| !provides(self.devices.get(*id).map_or(Kind::Gpu, |d| d.kind()), nodes[*id].role, &nodes[*id].component, Cap::Lod))
                .map(|id| format!("{} `{}` (`{id}`) lacks lod", nodes[id].role.name(), nodes[id].component))
                .collect();
            if !why.is_empty() {
                f.lack(Feature::Lod, why.join("; "));
            }
        }
        if f.has(Feature::Meshes) && !nodes.values().any(|n| n.role == Role::Meshes && n.component != "none") {
            f.lack(Feature::Meshes, "no meshes node");
        }
        if !f.has(Feature::Meshes) && f.has(Feature::MeshRays) {
            f.lack(Feature::MeshRays, "no meshes node");
        }
        let mut ids: Vec<&String> = nodes.keys().collect();
        ids.sort();
        let layers: Vec<String> = ids
            .iter()
            .filter(|id| matches!(nodes[**id].role, Role::Visibility | Role::Draw) && nodes[**id].component != "none")
            .filter(|id| !provides(self.devices.get(**id).map_or(Kind::Gpu, |d| d.kind()), nodes[**id].role, &nodes[**id].component, Cap::Layers))
            .map(|id| format!("{} `{}` (`{id}`) lacks layers: one surface per pixel", nodes[*id].role.name(), nodes[*id].component))
            .collect();
        if !layers.is_empty() {
            f.lack(Feature::Transparency, layers.join("; "));
        }
        for id in ids.into_iter().filter(|id| nodes[*id].role == Role::Draw) {
            let (n, kind) = (&nodes[id], self.devices.get(id).map_or(Kind::Gpu, |d| d.kind()));
            if f.has(Feature::MeshRays) && !provides(kind, Role::Draw, &n.component, Cap::MeshRays) {
                f.lack(Feature::MeshRays, format!("draw `{}` (`{id}`) lacks mesh rays: its shading does not meet the triangles", n.component));
            }
        }
    }

    /// The knobs in `asked` (name → value) this graph leaves out, each with the reason, given
    /// its executor's knobs `params`: a knob it has no node for (its owner's role and
    /// components), a value whose need ([`mr_core::ValueNeed`]) no node meets, a knob it does
    /// not have at all. Names with a `.` (`<node>.component`) are the host's, not asked here.
    pub fn knobs_left_out(&self, params: &[mr_core::ParamSpec], asked: &toml::Table) -> Vec<String> {
        let mut out = vec![];
        for (k, v) in asked.iter().filter(|(k, _)| !k.contains('.')) {
            match params.iter().find(|p| p.name == *k) {
                None => out.push(format!("knob `{k}`: this pipeline has no such knob")),
                Some(p) => out.extend(self.value_left_out(p, &v.as_str().map_or_else(|| v.to_string(), str::to_string))),
            }
        }
        out
    }

    /// Knob `p` as far as this graph takes it (what a tuner may vary): `None` without a node
    /// to take it, a choice without the values it leaves out (none left: `None`).
    pub fn taken(&self, p: &mr_core::ParamSpec) -> Option<mr_core::ParamSpec> {
        let mut p = p.clone();
        if let mr_core::ParamKind::Choice(opts) = &mut p.kind {
            let all = std::mem::take(opts);
            let kept: Vec<String> = all.into_iter().filter(|o| self.value_left_out(&p, o).is_empty()).collect();
            if kept.is_empty() {
                return None;
            }
            if !p.default.as_str().is_some_and(|d| kept.iter().any(|o| o == d)) {
                p.default = mr_core::ParamValue::Choice(kept[0].clone());
            }
            p.kind = mr_core::ParamKind::Choice(kept);
            return Some(p);
        }
        self.value_left_out(&p, &p.default.display()).is_empty().then_some(p)
    }

    /// Why this graph leaves out knob `p` set to `value` (as text: a choice's name, a number):
    /// no node to take the knob (its owner's role and components), or the value's need
    /// ([`mr_core::ValueNeed`]) met by no node. Empty: it takes it.
    pub fn value_left_out(&self, p: &mr_core::ParamSpec, value: &str) -> Vec<String> {
        let g = &self.graph;
        let stages = g.stage_nodes();
        let mut ids: Vec<&String> = g.nodes.keys().filter(|id| stages.contains(*id)).collect();
        ids.sort();
        let of_role = |role: &str| -> Vec<&String> { ids.iter().copied().filter(|id| g.nodes[*id].role.name() == role).collect() };
        let k = &p.name;
        if let Some((role, comps)) = &p.owner
            && !of_role(role).into_iter().any(|id| comps.contains(&g.nodes[id].component))
        {
            return vec![format!("knob `{k}`: no {role} node of {} (its {role}: {})", names(comps), names(&of_role(role).into_iter().map(|id| g.nodes[id].component.clone()).collect::<Vec<_>>()))];
        }
        let mut out = vec![];
        for need in p.needs.iter().filter(|n| n.value == value) {
            // The nodes the need is about: of its role, or what feeds their `port`.
            let at: Vec<&String> = of_role(&need.role).into_iter().flat_map(|id| match &need.port {
                Some(port) => g.nodes[id].inputs.get(port).into_iter().collect::<Vec<_>>(),
                None => vec![id],
            }).collect();
            if at.iter().any(|id| g.nodes.get(*id).is_some_and(|n| need.components.contains(&n.component))) {
                continue;
            }
            let what = match &need.port {
                Some(port) => format!("{}.{port}", need.role),
                None => need.role.clone(),
            };
            let why = match at.first().and_then(|id| g.nodes.get(*id).map(|n| (id, n))) {
                Some((id, n)) => format!("{} `{}` (`{id}`) lacks {}", n.role.name(), n.component, need.what),
                None => format!("no node at {what}"),
            };
            out.push(format!("`{k} = {value}`: {why}"));
        }
        out
    }
}

/// `a`, `b` / none.
fn names(v: &[String]) -> String {
    if v.is_empty() { "none".into() } else { v.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ") }
}

#[cfg(test)]
mod tests {
    use mr_core::ParamSpec;

    use crate::graph::shipped;

    #[test]
    fn value_needs_follow_ports() {
        // A need of the node feeding `visibility.primary`: `kernel` here (refloor).
        let p = |comps: &[&str]| ParamSpec::choice("view", "", &["image", "steps"], "image", true).needs("steps", "visibility", Some("primary"), comps, "steps");
        let c = shipped("raycast-gpu").unwrap().check().unwrap();
        let asked: toml::Table = toml::from_str("view = \"steps\"").unwrap();
        assert!(c.knobs_left_out(&[p(&["refloor"])], &asked).is_empty());
        assert_eq!(c.knobs_left_out(&[p(&["sphere"])], &asked), vec!["`view = steps`: traversal `refloor` (`kernel`) lacks steps".to_string()]);
        assert!(c.knobs_left_out(&[p(&["sphere"])], &toml::from_str("view = \"image\"").unwrap()).is_empty());
        // A tuner varies the values it takes.
        assert_eq!(c.taken(&p(&["sphere"])).map(|t| t.kind), Some(mr_core::ParamKind::Choice(vec!["image".into()])));
        assert_eq!(c.taken(&p(&["refloor"])).map(|t| t.kind), Some(mr_core::ParamKind::Choice(vec!["image".into(), "steps".into()])));
        let owned = ParamSpec::int("bounces", "", 1, 8, 1, true).of("shading", &["pathtrace"]);
        assert!(c.taken(&owned).is_none());
    }

    /// No shipped graph has transparency yet: each says which visibility or draw node lacks
    /// layers, even where its region does not build it.
    #[test]
    fn transparency_names_the_nodes_without_layers() {
        use mr_core::{Feature, Features};
        for (name, _) in crate::graph::SHIPPED {
            let c = shipped(name).unwrap().check().unwrap();
            for mut f in [Features::of(&[Feature::Lit, Feature::Transparency]), Features::of(&[Feature::Lit])] {
                c.narrow_on(&mut f, &|_| None);
                let why = f.why_not(Feature::Transparency).unwrap();
                assert!(why.contains("lacks layers: one surface per pixel") && (why.starts_with("visibility `") || why.starts_with("draw `")), "{name}: {why}");
            }
        }
        let c = shipped("raster-gpu").unwrap().check().unwrap();
        let mut f = Features::of(&[Feature::Transparency]);
        c.narrow_on(&mut f, &|_| None);
        assert_eq!(f.why_not(Feature::Transparency).unwrap(), "draw `raster` (`draw`) lacks layers: one surface per pixel; visibility `faces` (`visibility`) lacks layers: one surface per pixel");
    }

    /// Stateful shading in a forward drawer's fragment shader takes lit passes away on a GPU
    /// without forced early depth tests, with the reason; deferred drawers, stateless shading
    /// and devices with the feature (or unknown ones) keep them.
    #[test]
    fn fragment_pathtrace_needs_its_device_feature() {
        use mr_core::{Feature, Features};

        use crate::graph::Change;
        use crate::shading::state::FRAGMENT_FEATURE;
        let lit = |name: &str, edits: &[Change], gpu: Option<mr_gpu::wgpu::Features>| {
            let c = shipped(name).unwrap().with_edits(edits).unwrap().check().unwrap();
            let mut f = Features::of(&[Feature::Lit]);
            c.narrow_on(&mut f, &|_| gpu);
            f.why_not(Feature::Lit)
        };
        let pt = Change::component("shading", "pathtrace");
        let (none, has) = (Some(mr_gpu::wgpu::Features::empty()), Some(FRAGMENT_FEATURE));
        for name in ["raster-gpu", "discover-gpu"] {
            assert_eq!(
                lit(name, std::slice::from_ref(&pt), none).as_deref(),
                Some("shading `pathtrace` (`shading`) in draw `raster` (`draw`)'s fragment shader needs forced early depth tests (wgpu SHADER_EARLY_DEPTH_TEST), which gpu lacks"),
                "{name}"
            );
            assert_eq!(lit(name, std::slice::from_ref(&pt), has), None, "{name}");
            assert_eq!(lit(name, std::slice::from_ref(&pt), None), None, "{name}");
            assert_eq!(lit(name, &[], none), None, "{name}: direct shading");
        }
        assert_eq!(lit("raster-gpu", &[pt, Change::component("draw", "visbuffer")], none), None, "deferred");
    }
}
