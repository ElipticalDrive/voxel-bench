//! The sites form: site names, bindings, labels and site knob syntax.

use std::collections::{BTreeMap, HashMap};

use crate::{Composition, Slot};

use super::*;

/// `component(knob=value, ...)`, the sites form of a node with knobs (the link, the
/// client): the component and the knobs (values in TOML).
pub fn parse_site_knobs(value: &str) -> Result<(String, toml::Table), String> {
    let Some((name, rest)) = value.split_once('(') else { return Ok((value.trim().to_string(), toml::Table::new())) };
    let body = rest.trim_end().strip_suffix(')').ok_or_else(|| format!("`{value}`: knobs are written `{name}(knob=value, ...)`"))?;
    let mut t = toml::Table::new();
    for kv in body.split(',').map(str::trim).filter(|kv| !kv.is_empty()) {
        let (k, v) = kv.split_once('=').ok_or_else(|| format!("`{value}`: `{kv}` is not `knob=value`"))?;
        let v: toml::Table = toml::from_str(&format!("v = {}", v.trim())).map_err(|e| format!("`{value}`: `{kv}`: {}", e.message()))?;
        t.insert(k.trim().to_string(), v["v"].clone());
    }
    Ok((name.trim().to_string(), t))
}

/// The sites form of a component with knobs (see [`parse_site_knobs`]).
pub fn site_with_knobs(component: &str, knobs: &toml::Table) -> String {
    if knobs.is_empty() {
        return component.to_string();
    }
    format!("{component}({})", knobs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", "))
}

/// A link's sites form (`free`, or `net(..)` with the knobs off their defaults).
pub fn link_site(spec: &mr_core::link::LinkSpec) -> String {
    if spec.free() && !spec.reorder { "free".into() } else { site_with_knobs("net", &link_knobs(spec)) }
}

/// The link node `n` models (its knobs read as a `LinkSpec`; `free`: the free link).
pub(super) fn link_spec(n: &Node) -> Result<mr_core::link::LinkSpec, String> {
    match n.component.as_str() {
        "free" if n.knobs.is_empty() => Ok(Default::default()),
        "free" => Err("the free link has no knobs (`net` has them)".into()),
        _ => toml::Value::Table(n.knobs.clone()).try_into().map_err(|e: toml::de::Error| format!("link `net`: {}", e.message())),
    }
}

/// A link's knobs as the canonical form writes them: the `LinkSpec` read back, its fields
/// off their defaults, so `latency_ms = 30` and `30.0` are one network.
pub(super) fn link_knobs(spec: &mr_core::link::LinkSpec) -> toml::Table {
    let mut t = toml::Table::try_from(spec).expect("a LinkSpec is a table");
    let none = toml::Table::try_from(mr_core::link::LinkSpec::default()).unwrap();
    t.retain(|k, v| none.get(k) != Some(v));
    t
}

/// Whether `p` is a site: a ray-casting port, or discovery's exposure tests (their storage).
fn is_site(p: &Port) -> bool {
    p.casts || p.name == "exposure"
}

/// The short name of `role`'s site `port`: the port's name for the strategy and the
/// shading (`primary`, `rays`, `exposure`, `shadow`, `bounce`), else `<role>-<port>`
/// (`meshes-shadow`).
pub fn site_name(role: Role, port: &str) -> String {
    match role {
        Role::Visibility | Role::Shading => port.to_string(),
        r => format!("{}-{port}", r.name()),
    }
}

/// The *sites form* of a graph, its flat composition plus a binding per site that
/// the composition binds otherwise (`shadow = "tree64/sphere"`), for the CLI
/// (`gpu-compute:prefix+incremental,bounce=tree64/sphere`), plans (`sites = { ... }`),
/// labels, sweeps and the GUI. A site's value is one of:
/// - another site's name: the same kernel (an exposure site: the same storage);
/// - `<kernel>`: a kernel of its own over the lead's storage;
/// - `<storage>/<kernel>`: a kernel of its own over a storage of its own, or `<site>/<kernel>`
///   over that site's storage;
/// - for exposure tests, `<storage>`: a storage of their own.
///
/// Unlisted sites follow the rays before them, as unbound ports do: rebinding the shadow
/// rays takes the bounces and mesh shadows along unless they are listed too.
impl Graph {
    /// The sites of the graph's nodes (bound or not): name, caster node, port. The lead
    /// first, then by role and port order.
    pub fn site_ports(&self) -> Vec<(String, String, &'static Port)> {
        let mut v: Vec<(String, String, &'static Port)> = Vec::new();
        for (id, n) in &self.nodes {
            for p in inputs(n.role, &n.component).iter().filter(|p| is_site(p)) {
                v.push((site_name(n.role, p.name), id.clone(), p));
            }
        }
        let lead = self.lead();
        let rank = |(_, c, p): &(String, String, &'static Port)| {
            let n = &self.nodes[c];
            let at = inputs(n.role, &n.component).iter().position(|x| x.name == p.name).unwrap_or(0);
            (lead.as_ref().is_none_or(|(lc, lp)| lc != c || *lp != p.name), n.role, at)
        };
        v.sort_by_key(|s| rank(s));
        v
    }

    /// The node bound to site `name` (a kernel; for exposure tests a storage).
    pub fn site_node(&self, name: &str) -> Option<&str> {
        let (_, caster, port) = self.site_ports().into_iter().find(|(n, ..)| n == name)?;
        self.nodes.get(&caster)?.inputs.get(port.name).map(String::as_str)
    }

    /// The storage site `name` reads: its kernel's storage, or the exposure tests' own.
    fn site_storage(&self, name: &str) -> Option<String> {
        let (_, caster, port) = self.site_ports().into_iter().find(|(n, ..)| n == name)?;
        let b = self.nodes.get(&caster)?.inputs.get(port.name)?;
        if port.data == Data::Rays { self.walked(b).map(String::from) } else { Some(b.clone()) }
    }

    /// The site whose rays a site follows when it is not bound (or listed) itself.
    fn followed(&self, name: &str) -> Option<String> {
        match name {
            "bounce" | "meshes-shadow" => Some("shadow".into()),
            "shadow" | "exposure" => self.lead().map(|(c, p)| site_name(self.nodes[&c].role, p)).filter(|l| l != name),
            _ => None,
        }
    }

    /// The graph with `sites` bound as given (see the sites form above).
    pub fn with_sites(&self, sites: &BTreeMap<String, String>) -> Result<Graph, String> {
        let mut g = self.normalised()?;
        let mut sites = sites.clone();
        for (key, role, have) in [(CLIENT, Role::Client, CLIENTS), (LINK, Role::Link, LINKS)] {
            if let Some(v) = sites.remove(key) {
                let (c, knobs) = parse_site_knobs(&v)?;
                if !have.contains(&c.as_str()) {
                    return Err(format!("unknown {key} `{c}` (have: {})", have.join(", ")));
                }
                for n in g.nodes.values_mut().filter(|n| n.role == role) {
                    n.component = c.clone();
                    n.knobs = knobs.clone();
                }
            }
        }
        if let Some(v) = sites.remove(POST) {
            if !POSTS.contains(&v.as_str()) {
                return Err(format!("unknown {POST} `{v}` (have: {})", POSTS.join(", ")));
            }
            g.nodes.values_mut().filter(|n| n.role == Role::Post).for_each(|n| n.component = v.clone());
        }
        if let Some(v) = sites.remove(GENERATOR) {
            let d = match v.as_str() {
                "server" => Device::Server,
                "cpu" => Device::Cpu,
                _ => return Err(format!("{GENERATOR} = `{v}`: it runs on the `server` (streamed) or the client's `cpu`")),
            };
            for n in g.nodes.values_mut().filter(|n| n.role == Role::Generator) {
                n.device = (d != Device::Server).then_some(d);
            }
            g = g.normalised()?;
        }
        let sites = &sites;
        let names: Vec<String> = g.site_ports().into_iter().map(|(n, ..)| n).collect();
        if let Some(bad) = sites.keys().find(|k| !names.contains(k)) {
            return Err(format!("no site `{bad}` in this pipeline (have: {}, {LINK}, {CLIENT}, {GENERATOR}, {POST})", names.join(", ")));
        }
        // A value naming another listed site waits until that one is bound.
        let refers = |v: &str, n: &str| v == n || v.split_once('/').is_some_and(|(s, _)| s == n);
        let mut pending: Vec<(&String, &String)> = sites.iter().collect();
        while !pending.is_empty() {
            let Some(i) = pending.iter().position(|(_, v)| !pending.iter().any(|(n, _)| refers(v, n))) else {
                return Err(format!("sites refer to each other in a cycle: {}", pending.iter().map(|(n, v)| format!("{n} = {v}")).collect::<Vec<_>>().join(", ")));
            };
            let (name, value) = pending.remove(i);
            g.bind_site(name, value, sites)?;
        }
        Ok(g.pruned())
    }

    /// Binds site `name` to `value`; the sites following it (not in `listed`) and bound as
    /// it was come along.
    fn bind_site(&mut self, name: &str, value: &str, listed: &BTreeMap<String, String>) -> Result<(), String> {
        let ports = self.site_ports();
        let (_, caster, port) = ports.iter().find(|(n, ..)| n == name).cloned().unwrap();
        let is_site = |s: &str| ports.iter().any(|(n, ..)| n == s);
        let lead_storage = self.lead().and_then(|(c, p)| self.site_storage(&site_name(self.nodes[&c].role, p)));
        let fresh = |g: &Graph, id: String| {
            let mut out = id.clone();
            let mut i = 2;
            while g.nodes.contains_key(&out) {
                out = format!("{id}{i}");
                i += 1;
            }
            out
        };
        let add = |g: &mut Graph, id: String, node: Node| {
            let id = fresh(g, id);
            g.nodes.insert(id.clone(), node);
            id
        };
        let unknown = |role: Role, c: &str| format!("site `{name}`: unknown {} component `{c}` (have: {})", role.name(), crate::components(role.slot().unwrap()).map(|c| c.name).collect::<Vec<_>>().join(", "));
        let old = self.nodes[&caster].inputs.get(port.name).cloned();
        let target = if port.data == Data::Storage {
            if is_site(value) {
                self.site_storage(value).ok_or_else(|| format!("site `{name}` = `{value}`: that site is not bound"))?
            } else if known(Role::Storage, value) {
                add(self, format!("{name}.storage"), Node::new(Role::Storage, value).with(&[("changes", "world")]))
            } else {
                return Err(unknown(Role::Storage, value));
            }
        } else if is_site(value) {
            self.site_node(value).map(String::from).ok_or_else(|| format!("site `{name}` = `{value}`: that site is not bound"))?
        } else {
            let (st, k) = match value.split_once('/') {
                Some((s, k)) => (Some(s), k),
                None => (None, value),
            };
            if !known(Role::Traversal, k) {
                return Err(unknown(Role::Traversal, k));
            }
            let walks = inputs(Role::Traversal, k).iter().find(|p| p.required).unwrap();
            let src = match st {
                None => lead_storage.clone().ok_or_else(|| format!("site `{name}` = `{value}`: no lead storage to walk; name one (`<storage>/{k}`)"))?,
                Some(s) if is_site(s) => self.site_storage(s).ok_or_else(|| format!("site `{name}` = `{value}`: site `{s}` is not bound"))?,
                Some(s) => {
                    let role = if walks.data == Data::Faces { Role::Geometry } else { Role::Storage };
                    if !known(role, s) {
                        return Err(unknown(role, s));
                    }
                    add(self, format!("{name}.{}", role.name()), Node::new(role, s).with(&[("changes", "world")]))
                }
            };
            add(self, format!("{name}.kernel"), Node::new(Role::Traversal, k).with(&[(walks.name, &src)]))
        };
        self.nodes.get_mut(&caster).unwrap().inputs.insert(port.name.into(), target.clone());
        // Followers bound as this site was come along (unless listed themselves).
        if let Some(old) = old {
            for (f, fc, fp) in &ports {
                if f != name && !listed.contains_key(f) && self.followed(f).as_deref() == Some(name) && self.nodes[fc].inputs.get(fp.name) == Some(&old) && fp.data == port.data {
                    self.bind_site(f, name, listed)?;
                }
            }
        }
        Ok(())
    }

    /// Without the nodes that lead to no image (the world stays).
    pub fn pruned(mut self) -> Graph {
        let read = |k: &String| self.nodes.values().any(|r| r.inputs.values().any(|s| s == k));
        let Some(out) = self.nodes.iter().find(|(k, n)| known(n.role, &n.component) && output(n.role, &n.component) == Data::Image && !read(k)).map(|(k, _)| k.clone()) else { return self };
        let mut keep = std::collections::HashSet::new();
        let mut stack = vec![out];
        while let Some(id) = stack.pop() {
            if keep.insert(id.clone()) {
                stack.extend(self.nodes.get(&id).into_iter().flat_map(|n| n.inputs.values().cloned()));
            }
        }
        self.nodes.retain(|k, n| keep.contains(k) || matches!(n.role, Role::World | Role::Generator | Role::Link | Role::Client));
        self
    }

    /// Whether the world is generated on the client (the generator node off the server).
    pub fn generates(&self) -> bool {
        self.nodes.values().any(|n| n.role == Role::Generator && n.device.is_some_and(|d| d != Device::Server))
    }

    /// The client node's component (`pick` when the graph has none).
    pub fn client(&self) -> &str {
        self.nodes.values().find(|n| n.role == Role::Client).map_or("pick", |n| n.component.as_str())
    }

    /// The client's knob `name` ([`CLIENT_KNOBS`]), if set.
    pub fn client_knob(&self, name: &str) -> Option<i64> {
        self.nodes.values().find(|n| n.role == Role::Client).and_then(|n| n.knobs.get(name)).and_then(|v| v.as_integer())
    }

    /// The post node's tone mapper (`clamp` when the graph has none).
    pub fn tone(&self) -> crate::post::Tone {
        self.nodes.values().find(|n| n.role == Role::Post).and_then(|n| crate::post::Tone::parse(&n.component)).unwrap_or_default()
    }

    /// The network the link node models (free when the graph has none).
    pub fn link(&self) -> Result<mr_core::link::LinkSpec, String> {
        self.nodes.values().find(|n| n.role == Role::Link).map_or(Ok(Default::default()), link_spec)
    }

    /// The sites form's value for the link, the client, the generator or the post (`key`:
    /// [`LINK`], [`CLIENT`], [`GENERATOR`], [`POST`]), the default written out.
    pub fn sites_value(&self, key: &str) -> String {
        let g = self.normalised().unwrap_or_else(|_| self.clone());
        g.world_sites().remove(key).unwrap_or_else(|| match key {
            LINK => "free".into(),
            GENERATOR => "server".into(),
            POST => "clamp".into(),
            _ => "pick".into(),
        })
    }

    /// The link, the client, the generator and the post in the sites form (`link =
    /// "net(latency_ms=30)"`, `client = "derive(radius=4)"`, `generator = "cpu"`, `post =
    /// "aces"`): those other than the default, nothing set.
    fn world_sites(&self) -> BTreeMap<String, String> {
        let mut t = BTreeMap::new();
        for n in self.nodes.values() {
            let (key, default) = match n.role {
                Role::Link => (LINK, "free"),
                Role::Client => (CLIENT, "pick"),
                Role::Post => (POST, "clamp"),
                Role::Generator => {
                    if let Some(d) = n.device.filter(|d| *d != Device::Server) {
                        t.insert(GENERATOR.to_string(), d.to_string());
                    }
                    continue;
                }
                _ => continue,
            };
            let knobs = if n.role == Role::Link { link_spec(n).map(|l| link_knobs(&l)).unwrap_or_else(|_| n.knobs.clone()) } else { n.knobs.clone() };
            if n.component != default || !knobs.is_empty() {
                t.insert(key.to_string(), site_with_knobs(&n.component, &knobs));
            }
        }
        t
    }

    /// The lead site's name (the strategy's own rays, or a rasteriser's shadow rays).
    pub fn lead_site(&self) -> Option<String> {
        self.lead().map(|(c, p)| site_name(self.nodes[&c].role, p))
    }

    /// Every bound site but the lead, in site order, described as in the sites form
    /// (nothing left out: what the composition binds so too).
    pub fn site_values(&self) -> Vec<(String, String)> {
        let me = self;
        // Every site described, the lead first so the others can name what they share.
        let ports = me.site_ports();
        let lead = me.lead().map(|(c, p)| site_name(me.nodes[&c].role, p));
        let lead_storage = lead.as_deref().and_then(|l| me.site_storage(l));
        let mut kernels: HashMap<String, String> = HashMap::new(); // kernel node → first site using it
        let mut storages: HashMap<String, String> = HashMap::new(); // storage node → first site reading it
        let mut sites = Vec::new();
        for (name, caster, port) in &ports {
            let Some(b) = me.nodes[caster].inputs.get(port.name) else { continue };
            let st = me.site_storage(name);
            if Some(name) == lead.as_ref() {
                kernels.insert(b.clone(), name.clone());
                st.into_iter().for_each(|s| drop(storages.insert(s, name.clone())));
                continue;
            }
            let value = if port.data == Data::Storage {
                storages.get(b).cloned().unwrap_or_else(|| me.nodes[b].component.clone())
            } else if let Some(owner) = kernels.get(b) {
                owner.clone()
            } else {
                let k = me.nodes[b].component.clone();
                match &st {
                    s if *s == lead_storage => k,
                    Some(s) => format!("{}/{k}", storages.get(s).cloned().unwrap_or_else(|| me.nodes[s].component.clone())),
                    None => k,
                }
            };
            if port.data == Data::Rays {
                kernels.entry(b.clone()).or_insert(name.clone());
            }
            if let Some(s) = st {
                storages.entry(s).or_insert(name.clone());
            }
            sites.push((name.clone(), value));
        }
        sites
    }

    /// The graph in its sites form: the composition the back end runs it as and the sites
    /// bound otherwise (`None` if it cannot be written so: nodes on free sites have knobs,
    /// or a shape the form has no words for).
    pub fn sites_form(&self) -> Result<Option<(Composition, BTreeMap<String, String>)>, String> {
        let (c, exact) = self.accept()?;
        // A link or client other than the default (which `accept` leaves to the worker) is
        // written out.
        let client = self.normalised()?.world_sites();
        if exact && client.is_empty() {
            return Ok(Some((c, client)));
        }
        let me = self.normalised()?.pruned();
        let skel = me.skeleton();
        if me.nodes.iter().any(|(id, n)| !skel.nodes.contains_key(id) && !n.knobs.is_empty()) {
            return Ok(None);
        }
        let strip = |g: &Graph| {
            let mut g = g.clone();
            g.nodes.values_mut().for_each(|n| n.knobs.clear());
            g.canonical()
        };
        let target = strip(&me)?;
        let described = me.site_values();
        let order: Vec<String> = described.iter().map(|(n, _)| n.clone()).collect();
        let mut sites: BTreeMap<String, String> = described.into_iter().chain(client).collect();
        let same_on = |c: &Composition, s: &BTreeMap<String, String>| Graph::from_composition(c).and_then(|b| b.with_sites(s)).and_then(|g| strip(&g)).is_ok_and(|x| x == target);
        if !same_on(&c, &sites) {
            return Ok(None);
        }
        // The composition's secondary-ray stages, where the sites say it all (a shadow walk
        // with a storage of its own, which the stages cannot say), back at their defaults.
        let mut c = c;
        let defaults = crate::backend(&c.backend).map(|b| c.slots().filter(|s| s.secondary()).filter_map(|s| Some((s, b.default_for(s)?))).collect::<Vec<_>>()).unwrap_or_default();
        for &(slot, d) in defaults.iter().rev() {
            let mut fewer = c.clone();
            fewer.set(slot, d);
            if fewer.get(slot) != c.get(slot) && fewer.validate().is_ok() && same_on(&fewer, &sites) {
                c = fewer;
            }
        }
        let same = |s: &BTreeMap<String, String>| same_on(&c, s);
        // Only what differs from the composition: drop the sites it binds so anyway, last first.
        for name in order.iter().rev() {
            let mut fewer = sites.clone();
            fewer.remove(name);
            // Another site naming this one keeps it.
            if !fewer.values().any(|v| v == name || v.split_once('/').is_some_and(|(s, _)| s == name)) && same(&fewer) {
                sites = fewer;
            }
        }
        Ok(Some((c, sites)))
    }

    /// A label: the composition's, then the sites bound otherwise
    /// (`gpu-compute: prefix + incremental, bounce = tree64/sphere`); a graph without a sites
    /// form gets its key.
    pub fn label(&self) -> Result<String, String> {
        Ok(match self.sites_form()? {
            Some((c, s)) => sites_label(&c, &s),
            None => format!("{} [graph {}]", self.accept()?.0.label(), &self.key()?[..8]),
        })
    }

    /// The values site `name` could take on its own (for sweeps' `"*"` and the GUI; not
    /// checked against the back end): the other sites it could share, a kernel over the
    /// lead's storage, and each storage with each kernel (exposure tests: each storage).
    pub fn site_choices(&self, name: &str) -> Vec<String> {
        if name == CLIENT {
            return CLIENTS.iter().map(|c| c.to_string()).collect();
        }
        if name == LINK {
            return LINKS.iter().map(|c| c.to_string()).collect();
        }
        if name == GENERATOR {
            return vec!["server".into(), "cpu".into()];
        }
        if name == POST {
            return POSTS.iter().map(|c| c.to_string()).collect();
        }
        let ports = self.site_ports();
        let Some((_, _, port)) = ports.iter().find(|(n, ..)| n == name) else { return vec![] };
        let others = ports.iter().filter(|(n, _, p)| n != name && p.data == port.data).map(|(n, ..)| n.clone());
        let storages = || crate::components(Slot::Storage).map(|c| c.name.to_string());
        if port.data == Data::Storage {
            return others.chain(storages()).collect();
        }
        let kernels: Vec<String> = crate::components(Slot::Traversal).map(|c| c.name.to_string()).collect();
        let geometries: Vec<String> = crate::components(Slot::Geometry).map(|c| c.name.to_string()).collect();
        let mut v: Vec<String> = others.chain(kernels.iter().cloned()).collect();
        for k in &kernels {
            let on = if inputs(Role::Traversal, k).iter().any(|p| p.data == Data::Faces) { geometries.clone() } else { storages().collect() };
            v.extend(on.iter().map(|s| format!("{s}/{k}")));
        }
        v
    }
}

/// A composition's label with sites bound otherwise.
pub fn sites_label(c: &Composition, sites: &BTreeMap<String, String>) -> String {
    let mut s = c.label();
    for (k, v) in sites {
        s += &format!(", {k} = {v}");
    }
    s
}
