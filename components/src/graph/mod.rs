//! Pipelines as graphs: a pipeline is a graph of component
//! instances (nodes, each with its own knobs and device) joined by edges from an output to
//! an input *port*, not a fixed list of stages. Any graph whose every required input is
//! bound to a compatible output, and that leads from the world to one image, is a valid
//! pipeline; pipelines need not share the same kinds of components.
//!
//! Ports carry typed data ([`Data`]): the world's change log, a storage, a face set, ray
//! starts, a ray-query service, hits, mesh surfaces, the image. Every place rays are cast
//! is an input port of the component that casts them ([`Port::casts`]: primary rays,
//! discovery rays, shadow and bounce rays, mesh shadows), bound to its own kernel and
//! storage nodes or to shared ones; discovery's exposure tests read a storage port (they
//! look blocks up, they cast no rays). Unbound secondary rays follow the rays before them
//! (shadow rays the strategy's own on the same device, bounces the shadow rays, mesh
//! shadows the shading's). Storages and ray queries are *fused* data: compiled together
//! with their caller, so they never cross devices; everything else may (the two
//! kinds of composition).
//!
//! The model, validation with reasons, a canonical form (hashable: the cache key),
//! and the conversion from today's flat [`Composition`](crate::Composition)s (so every stored record loads as a
//! graph). Back ends consume graphs ([`of_init`], [`Checked::site`]). Each back end
//! has a *lead* site (its strategy's own rays, or a rasteriser's shadow rays: the storage
//! and traversal stages of its compositions) and binds its other sites freely, over the
//! lead's storage only, or only to the lead ([`site_binding`]). [`Graph::accept`] checks
//! the free sites' components are implemented where their rays run and that the rest (the
//! skeleton) is one of the back end's compositions; it returns that composition and whether
//! it is exactly the graph.

mod accept;
mod canonical;
mod check;
mod compose;
mod model;
mod schedule;
mod sites;

pub use accept::*;
pub use check::*;
pub use model::*;
pub use schedule::*;
pub use sites::*;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;
    use crate::{Api, BACKENDS, Composition, Slot, backend, names_on};

    /// Every composition of every back end (each slot through all its components).
    fn every_composition() -> Vec<Composition> {
        let mut all = Vec::new();
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let mut combos = vec![Composition::new(b.name)];
            for slot in Slot::ALL.into_iter().filter(|&s| b.has(s)) {
                combos = combos
                    .iter()
                    .flat_map(|c| {
                        names_on(b.name, slot).into_iter().map(move |x| {
                            let mut c = c.clone();
                            c.set(slot, x);
                            c
                        })
                    })
                    .collect();
            }
            all.extend(combos.into_iter().filter(|c| c.validate().is_ok()));
        }
        all
    }

    /// Today's compositions load as graphs that validate, and convert back to a composition
    /// with the same graph.
    #[test]
    fn compositions_round_trip() {
        let all = every_composition();
        assert!(all.len() > 1000, "{}", all.len());
        let mut keys = HashMap::new();
        for c in &all {
            let g = Graph::from_composition(c).unwrap();
            assert_eq!(g.preset(), Some(c.backend.as_str()), "{}", c.label());
            let back = g.to_composition().unwrap_or_else(|e| panic!("{}: {e}\n{g:#?}", c.label()));
            assert_eq!(Graph::from_composition(&back).unwrap().canonical(), g.canonical(), "{}", c.label());
            // Exactly, except `same` and an explicit name meaning one graph.
            let same = |c: &Composition| {
                let mut c = c.clone();
                for s in [Slot::ShadowStorage, Slot::ShadowTraversal] {
                    if b_has(&c, s) {
                        let r = c.resolved(s).to_string();
                        c.set(s, &r);
                    }
                }
                c
            };
            assert_eq!(same(&back), same(c), "{}", c.label());
            keys.entry(g.key().unwrap()).or_insert_with(Vec::new).push(same(c));
        }
        // Distinct compositions are distinct graphs, unless they differ only in `same`.
        let shared = keys.values().filter(|v| v.len() > 1).count();
        for (k, cs) in &keys {
            assert!(cs.iter().all(|c| *c == cs[0]), "{k}: {cs:?}");
        }
        eprintln!("{} compositions, {} graphs, {shared} reached by several spellings", all.len(), keys.len());
    }

    fn b_has(c: &Composition, s: Slot) -> bool {
        backend(&c.backend).is_some_and(|b| b.has(s))
    }

    #[test]
    fn canonical_ignores_ids() {
        let g = Graph::from_composition(&Composition { storage: "prefix".into(), shadow_storage: "rows".into(), ..Default::default() }).unwrap();
        let mut renamed = Graph { executor: g.executor.clone(), nodes: BTreeMap::new() };
        let rename = |id: &str| format!("x_{id}");
        for (id, n) in &g.nodes {
            let mut n = n.clone();
            n.inputs.values_mut().for_each(|s| *s = rename(s));
            renamed.nodes.insert(rename(id), n);
        }
        assert_eq!(renamed.canonical(), g.canonical());
        assert_eq!(renamed.key(), g.key());
        assert_ne!(Graph::from_composition(&Composition::default()).unwrap().key(), g.key());
    }

    /// A graph written by hand, without the world (implicit), loads and runs as the
    /// composition it is.
    #[test]
    fn written_graph() {
        let g: Graph = toml::from_str(
            r#"
            executor = "gpu-compute"
            [nodes.s]
            role = "storage"
            component = "prefix"
            [nodes.k]
            role = "traversal"
            component = "incremental"
            inputs = { storage = "s" }
            knobs = { exact = true }
            [nodes.v]
            role = "visibility"
            component = "raycast"
            inputs = { primary = "k" }
            [nodes.out]
            role = "shading"
            component = "direct"
            inputs = { hits = "v", shadow = "k" }
            "#,
        )
        .unwrap();
        let c = g.to_composition().unwrap();
        assert_eq!(c, Composition { storage: "prefix".into(), traversal: "incremental".into(), meshes: "none".into(), ..Default::default() });
        assert_eq!(g.flat_knobs().unwrap().get("exact"), Some(&toml::Value::Boolean(true)));
        // Shadow rays through their own kernel over the same storage.
        let mut sep = g.clone();
        sep.nodes.insert("k2".into(), Node::new(Role::Traversal, "sphere").with(&[("storage", "s")]));
        sep.nodes.get_mut("out").unwrap().inputs.insert("shadow".into(), "k2".into());
        let c = sep.to_composition().unwrap();
        assert_eq!((c.shadow_storage.as_str(), c.shadow_traversal.as_str()), ("same", "sphere"));
    }

    /// Invalid graphs say why.
    #[test]
    fn reasons() {
        let base = Graph::from_composition(&Composition::default()).unwrap();
        let err = |f: &dyn Fn(&mut Graph)| {
            let mut g = base.clone();
            f(&mut g);
            g.validate().unwrap_err()
        };
        let e = err(&|g| {
            g.nodes.get_mut("visibility").unwrap().inputs.insert("primary".into(), "storage".into());
        });
        assert!(e.contains("takes ray queries, but `storage`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().inputs.clear();
        });
        assert!(e.contains("input `storage`") && e.contains("not bound"), "{e}");
        let e = err(&|g| {
            g.nodes.insert("extra".into(), Node::new(Role::Storage, "rows"));
        });
        assert!(e.contains("lead to no image: extra"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().device = Some(Device::Cpu);
        });
        assert!(e.contains("compiled into their caller"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().inputs.insert("storage".into(), "kernel".into());
        });
        assert!(e.contains("takes storage"), "{e}");
        // A shape gpu-compute has no composition for: two primary-ray storages.
        let e = err(&|g| {
            g.nodes.insert("s2".into(), Node::new(Role::Storage, "rows"));
            g.nodes.insert("k2".into(), Node::new(Role::Traversal, "refloor").with(&[("storage", "s2")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "k2".into());
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "kernel".into());
        });
        assert!(e.contains("no input `bounce`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("shading").unwrap().component = "pathtrace".into();
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "visibility".into());
        });
        assert!(e.contains("takes ray queries"), "{e}");
        // Capability needs: sphere needs distances; storages get them through an adapter,
        // so only a storage-less source would fail, which types already reject.
        assert!(Graph::from_composition(&Composition { traversal: "sphere".into(), ..Default::default() }).unwrap().validate().is_ok());
        // Discovery across devices: CPU discovery rays and GPU shadow rays cannot share a kernel.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "kernel".into());
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(e.contains("two devices"), "{e}");
    }

    /// Sites off the lead bind their own kernel and storage where the back end
    /// runs them so; elsewhere they are rejected with the reason.
    /// A node's `api`, checked against its device, its component's implementations
    /// and the executor; written into the canonical form only where it is a choice.
    #[test]
    fn apis() {
        let set = |c: &Composition, at: &[(&str, Api)]| {
            let mut g = Graph::from_composition(c).unwrap();
            for (id, a) in at {
                g.nodes.get_mut(*id).unwrap().api = Some(*a);
            }
            g
        };
        // cpu: one API for its one loop, read as its `isa` knob; fused nodes follow.
        let cpu = Composition::new("cpu");
        let g = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx2)]);
        let (c, exact) = g.accept().unwrap();
        assert!(exact && c == Composition::new("cpu"));
        assert_eq!(g.flat_knobs().unwrap().get("isa"), Some(&toml::Value::String("avx2".into())));
        let ck = g.check().unwrap();
        assert_eq!(ck.apis.get("kernel"), Some(&Api::Avx2), "the kernel runs with its caller");
        // A node without an API of its own or its callers' runs its component's only one on
        // its device (what the ray-casting executor picks its lane by).
        // A walk's kernel has two (compute, raster: `dispatch`), so it has none.
        for (backend, api) in [("gpu-rt", Some(Api::RayQuery)), ("gpu-compute", None)] {
            let ck = Graph::from_composition(&Composition::new(backend)).unwrap().check().unwrap();
            assert_eq!(ck.api(ck.input(Role::Visibility, "primary").unwrap()), api, "{backend}");
        }
        assert!(g.canonical().unwrap().contains(":avx2") && g.key().unwrap() != Graph::from_composition(&cpu).unwrap().key().unwrap());
        let e = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx512)]).validate().unwrap_err();
        assert!(e.contains("with one api, its `isa`"), "{e}");
        let e = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx2), ("kernel", Api::Scalar)]).validate().unwrap_err();
        assert!(e.contains("`kernel` (api scalar) is compiled into callers running avx2"), "{e}");
        let e = set(&cpu, &[("shading", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("runs on a gpu"), "{e}");
        // GPU back ends: the API they run the stage with, and only that.
        let gc = Composition::default();
        assert!(set(&gc, &[("visibility", Api::Compute)]).validate().is_ok());
        assert_eq!(set(&gc, &[("visibility", Api::Compute)]).key(), Graph::from_composition(&gc).unwrap().key(), "the default: not in the key");
        // gpu-compute's walk: a compute thread or a fragment shader per pixel (`dispatch`).
        let frag = set(&gc, &[("visibility", Api::Raster)]);
        assert_eq!(frag.flat_knobs().unwrap().get("dispatch"), Some(&toml::Value::String("fragment".into())));
        assert!(frag.validate().is_ok() && frag.key() != Graph::from_composition(&gc).unwrap().key());
        let e = set(&gc, &[("visibility", Api::Raster), ("shading", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("with one api, its `dispatch`"), "{e}");
        let e = set(&Composition { prepass: "beam".into(), ..Default::default() }, &[("prepass", Api::Raster)]).validate().unwrap_err();
        assert!(e.contains("`beam` has no raster implementation"), "{e}");
        let e = set(&Composition::new("gpu-rt"), &[("visibility", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("runs its visibility with rayquery here, not compute"), "{e}");
        let e = set(&Composition::new("gpu-raster"), &[("shading", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("runs its shading with raster here, not compute"), "{e}");
        let e = set(&gc, &[("kernel", Api::RayQuery)]).validate().unwrap_err();
        assert!(e.contains("`kernel` (in `meshes`): `gpu-compute` runs its meshes with raster here, not rayquery"), "{e}");
        // A face drawer's shading: in the fragment shader for `raster`, else deferred compute.
        let r = Composition::new("gpu-raster");
        assert!(set(&r, &[("shading", Api::Raster)]).validate().is_ok());
        assert_eq!(set(&r, &[("shading", Api::Raster)]).key(), Graph::from_composition(&r).unwrap().key(), "no choice: not in the key");
        assert!(set(&r, &[("shading", Api::Compute)]).validate().is_err());
        let vb = Composition { draw: "visbuffer".into(), ..Composition::new("gpu-raster") };
        assert!(set(&vb, &[("shading", Api::Compute)]).validate().is_ok());
        // Implementations: cpu-raster is scalar only; pathtrace has no CPU implementation.
        let e = set(&Composition::new("cpu-raster"), &[("shading", Api::Avx2)]).validate().unwrap_err();
        assert!(e.contains("runs its shading with scalar here, not avx2"), "{e}");
        let mut g = Graph::from_composition(&Composition::new("cpu")).unwrap();
        g.nodes.get_mut("shading").unwrap().component = "pathtrace".into();
        assert!(g.validate().is_err());
        // Devices follow where the back end runs a role: rasterisers build faces on the CPU.
        let ck = Graph::from_composition(&r).unwrap().check().unwrap();
        assert_eq!((ck.devices["geometry"], ck.devices["draw"], ck.devices["kernel"]), (Device::Cpu, Device::Gpu(0), Device::Gpu(0)));
        // Plans and records: `api` round-trips through TOML.
        let t = toml::to_string(&set(&cpu, &[("visibility", Api::Avx512)])).unwrap();
        assert!(t.contains("api = \"avx512\""), "{t}");
        assert_eq!(toml::from_str::<Graph>(&t).unwrap(), set(&cpu, &[("visibility", Api::Avx512)]));
    }

    #[test]
    fn free_sites() {
        let with = |c: Composition, f: &dyn Fn(&mut Graph)| {
            let mut g = Graph::from_composition(&c).unwrap();
            f(&mut g);
            g
        };
        let own = |g: &mut Graph, id: &str, st: &str, k: &str, at: &[(&str, &str)]| {
            g.nodes.insert(format!("{id}.storage"), Node::new(Role::Storage, st));
            g.nodes.insert(format!("{id}.kernel"), Node::new(Role::Traversal, k).with(&[("storage", &format!("{id}.storage"))]));
            for (node, port) in at {
                g.nodes.get_mut(*node).unwrap().inputs.insert(port.to_string(), format!("{id}.kernel"));
            }
        };
        // gpu-compute: primary, shadow, bounce and mesh shadows each their own.
        let pt = Composition { shading: "pathtrace".into(), ..Default::default() };
        let g = with(pt.clone(), &|g| {
            own(g, "s", "distance", "sphere", &[("shading", "shadow")]);
            own(g, "b", "rows", "rowrun", &[("shading", "bounce")]);
            own(g, "m", "tree64", "refloor", &[("meshes", "shadow")]);
        });
        let (c, exact) = g.accept().unwrap();
        assert!(!exact && c.shadow_storage == "distance" && c.shadow_traversal == "sphere", "{c:?}");
        // The same shadow and bounce walk is a composition again.
        let g = with(pt.clone(), &|g| own(g, "s", "distance", "sphere", &[("shading", "shadow"), ("shading", "bounce"), ("meshes", "shadow")]));
        assert!(g.accept().unwrap().1);
        // Unbound bounces follow the shadow rays.
        let mut h = g.clone();
        h.nodes.get_mut("shading").unwrap().inputs.remove("bounce");
        assert_eq!(h.canonical(), g.canonical());
        // gpu-raster: bounces apart from the shadow rays, also as ray queries over the
        // geometry it draws (before, a kernel it lacked).
        let r = Composition { shading: "pathtrace".into(), ..Composition::new("gpu-raster") };
        assert!(!with(r.clone(), &|g| own(g, "b", "prefix", "incremental", &[("shading", "bounce")])).accept().unwrap().1);
        let q = with(r, &|g| {
            g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "rq".into());
        });
        assert!(!q.accept().unwrap().1);
        // gpu-discover: exposure tests read another storage than the discovery rays walk.
        let d = with(Composition { storage: "distance".into(), traversal: "sphere".into(), ..Composition::new("gpu-discover") }, &|g| {
            g.nodes.insert("x".into(), Node::new(Role::Storage, "bricks"));
            g.nodes.get_mut("visibility").unwrap().inputs.insert("exposure".into(), "x".into());
        });
        assert!(!d.accept().unwrap().1);
        // cpu-discover: exposure is the discovery storage; GPU shadow rays their own.
        let cd = Composition::new("cpu-discover");
        let e = with(cd.clone(), &|g| {
            g.nodes.insert("x".into(), Node::new(Role::Storage, "hier"));
            g.nodes.get_mut("visibility").unwrap().inputs.insert("exposure".into(), "x".into());
        })
        .validate()
        .unwrap_err();
        assert!(e.contains("reads exposure from the storage its rays walk"), "{e}");
        assert!(with(Composition { shading: "pathtrace".into(), ..cd }, &|g| own(g, "b", "rows", "rowrun", &[("shading", "bounce")])).validate().is_ok());
        // cpu: the shadow kernel of its own over the primary storage, not another storage.
        let cpu = Composition::new("cpu");
        let g = with(cpu.clone(), &|g| {
            g.nodes.insert("sk".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "storage")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "sk".into());
        });
        assert!(!g.accept().unwrap().1);
        let e = with(cpu, &|g| own(g, "s", "hier", "refloor", &[("shading", "shadow")])).validate().unwrap_err();
        assert!(e.contains("over the storage of its primary rays"), "{e}");
        // gpu-rt: its shading's rays free: a kernel node of their own over its
        // one acceleration structure.
        let g = with(Composition { shading: "pathtrace".into(), ..Composition::new("gpu-rt") }, &|g| {
            g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "rq".into());
        });
        assert!(!g.accept().unwrap().1);
        // Knobs of free-site nodes stay with their nodes.
        let mut g = with(Composition::default(), &|g| own(g, "s", "prefix", "incremental", &[("shading", "shadow")]));
        g.nodes.get_mut("s.kernel").unwrap().knobs.insert("exact".into(), toml::Value::Boolean(false));
        g.nodes.get_mut("kernel").unwrap().knobs.insert("exact".into(), toml::Value::Boolean(true));
        assert_eq!(g.flat_knobs().unwrap().get("exact"), Some(&toml::Value::Boolean(true)));
    }

    /// Every graph one or two site bindings away from a back end's compositions
    /// has a sites form, and that form gives the graph back.
    #[test]
    fn sites_form_round_trips() {
        let mut bases = Vec::new();
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let mut c = Composition::new(b.name);
            c.fill_defaults();
            bases.push(c.clone());
            if b.has(Slot::Shading) && names_on(b.name, Slot::Shading).contains(&"pathtrace") {
                c.shading = "pathtrace".into();
                if b.has(Slot::Meshes) && names_on(b.name, Slot::Meshes).len() > 1 {
                    c.meshes = names_on(b.name, Slot::Meshes).into_iter().find(|m| *m != "none").unwrap().into();
                }
                bases.push(c);
            }
        }
        let (mut graphs, mut keys) = (0, std::collections::HashSet::new());
        let check = |c: &Composition, sites: &BTreeMap<String, String>, keys: &mut std::collections::HashSet<String>| -> bool {
            let base = Graph::from_composition(c).unwrap();
            let Ok(g) = base.with_sites(sites) else { return false };
            if g.accept().is_err() {
                return false;
            }
            let (c2, s2) = g.sites_form().unwrap().unwrap_or_else(|| panic!("{}: {sites:?} has no sites form", c.label()));
            let again = Graph::from_composition(&c2).unwrap().with_sites(&s2).unwrap();
            assert_eq!(again.canonical(), g.canonical(), "{} {sites:?} → {} {s2:?}", c.label(), c2.label());
            assert!(g.label().unwrap().starts_with(&c2.label()));
            keys.insert(g.key().unwrap());
            true
        };
        for c in &bases {
            let base = Graph::from_composition(c).unwrap();
            let names: Vec<String> = base.site_ports().into_iter().map(|(n, ..)| n).collect();
            for n in &names {
                for v in base.site_choices(n) {
                    graphs += check(c, &BTreeMap::from([(n.clone(), v.clone())]), &mut keys) as usize;
                }
            }
            // Pairs: shadow and bounce rays on gpu-compute, a sample of values.
            if c.backend == "gpu-compute" && c.shading == "pathtrace" {
                for a in ["primary", "sphere", "rows/rowrun", "tree64/refloor"] {
                    for b in ["shadow", "incremental", "distance/sphere", "shadow/sphere", "primary"] {
                        graphs += check(c, &BTreeMap::from([("shadow".into(), a.into()), ("bounce".into(), b.into())]), &mut keys) as usize;
                    }
                }
            }
        }
        eprintln!("{graphs} site bindings run, {} distinct graphs", keys.len());
        assert!(graphs > 300, "{graphs}");
        // Labels and errors.
        let pt = Composition { shading: "pathtrace".into(), ..Default::default() };
        let g = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("bounce".into(), "tree64/sphere".into())])).unwrap();
        assert_eq!(g.label().unwrap(), "gpu-compute: bricks + refloor + pathtrace, bounce = tree64/sphere");
        // Rebinding the shadow rays takes the bounces along: a composition again.
        let g = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("shadow".into(), "rows/rowrun".into())])).unwrap();
        assert_eq!(g.label().unwrap(), "gpu-compute: bricks + refloor + shadow rows + shadow rowrun + pathtrace");
        let e = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("nope".into(), "x".into())])).unwrap_err();
        assert!(e.contains("no site `nope`"), "{e}");
        let e = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("bounce".into(), "x/refloor".into())])).unwrap_err();
        assert!(e.contains("unknown storage component `x`"), "{e}");
        let e = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("bounce".into(), "shadow".into()), ("shadow".into(), "bounce".into())])).unwrap_err();
        assert!(e.contains("cycle"), "{e}");
    }

    /// The back ends' names are presets, named by a graph's shape and devices,
    /// whatever executor it is written for (or none).
    #[test]
    fn presets() {
        let key = |c: Composition| Graph::from_composition(&c).unwrap().key().unwrap();
        let of = |b: &str| Graph::from_composition(&Composition::new(b)).unwrap();
        // Discovery's sampler moved to the CPU: the shading's shadow rays get their own
        // instances on the GPU, the drawer its faces a frame late or now.
        let mut g = of("gpu-discover");
        g.nodes.get_mut("visibility").unwrap().device = Some(Device::Cpu);
        for (id, n) in [("shadow.kernel", g.nodes["kernel"].clone().with(&[("storage", "shadow.storage")])), ("shadow.storage", g.nodes["storage"].clone())] {
            g.nodes.insert(id.into(), Node { device: Some(Device::Gpu(0)), ..n });
        }
        g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "shadow.kernel".into());
        assert_eq!(g.placed().executor, "cpu-discover");
        let now: toml::Table = toml::from_str("pipelined = false").unwrap();
        assert_eq!(g.key().unwrap(), of("cpu-discover").with_options(&now).key().unwrap());
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Prev);
        assert_eq!(g.key().unwrap(), key(Composition::new("cpu-discover")));
        assert_eq!(g.to_composition().unwrap().backend, "cpu-discover");
        // The drawer's face source a choice: discovery in place of meshing.
        let mut g = Graph::from_composition(&Composition { meshes: "none".into(), ..Composition::new("gpu-raster") }).unwrap();
        g.nodes.retain(|k, _| k != "geometry");
        g.nodes.insert("visibility".into(), Node::new(Role::Visibility, "quadtree").with(&[("rays", "kernel"), ("exposure", "storage")]));
        assert_eq!(g.key().unwrap(), key(Composition::new("gpu-discover")));
        // Ray queries for the primary rays, written for gpu-compute or for no executor.
        let mut g = of("gpu-rt");
        g.executor = "gpu-compute".into();
        assert_eq!(g.key().unwrap(), of("gpu-rt").key().unwrap());
        for b in ["gpu-compute", "gpu-splat", "gpu-voxelspace", "gpu-raster", "gpu-discover"] {
            let mut g = of(b);
            g.executor.clear();
            assert_eq!(g.key().unwrap(), of(b).key().unwrap(), "{b}");
        }
        // Without an executor, nodes run on the GPU unless they say otherwise.
        let mut g = of("cpu");
        g.executor.clear();
        assert_eq!(g.placed().executor, "gpu-compute");
        g.nodes.values_mut().filter(|n| matches!(n.role, Role::Visibility | Role::Shading)).for_each(|n| n.device = Some(Device::Cpu));
        assert_eq!(g.key().unwrap(), of("cpu").key().unwrap());
        // A shape no preset has keeps its reasons.
        let mut g = of("cpu-raster");
        g.nodes.get_mut("draw").unwrap().device = Some(Device::Gpu(0));
        assert_eq!(g.placed().executor, "gpu-raster");
        assert!(g.validate().is_err());
    }
    /// A pipeline on one GPU runs on any slot; over two it is split at the hit records (ray
    /// The client's component is a choice of the sites form (`client = "derive"`): the back
    /// end runs the graph as before, its key and label say so; `pick` stays out of both.
    #[test]
    fn client_in_the_sites_form() {
        let c = Composition::new("gpu-compute");
        let pick = Graph::from_composition(&c).unwrap();
        let sites: BTreeMap<String, String> = [(CLIENT.to_string(), "derive".to_string())].into();
        let derive = pick.with_sites(&sites).unwrap();
        assert_eq!(derive.client(), "derive");
        assert_eq!(derive.to_composition().unwrap().label(), pick.to_composition().unwrap().label());
        assert_ne!(derive.key().unwrap(), pick.key().unwrap());
        assert_eq!(derive.sites_form().unwrap().unwrap().1, sites);
        assert!(derive.label().unwrap().ends_with(", client = derive"), "{}", derive.label().unwrap());
        assert!(pick.sites_form().unwrap().unwrap().1.is_empty());
        let with_shadow: BTreeMap<String, String> = [(CLIENT.to_string(), "derive".to_string()), ("shadow".to_string(), "tree64/sphere".to_string())].into();
        let g = pick.with_sites(&with_shadow).unwrap();
        let (c, form) = g.sites_form().unwrap().unwrap();
        assert_eq!(form.get(CLIENT).map(String::as_str), Some("derive"));
        assert_eq!(Graph::from_composition(&c).unwrap().with_sites(&form).unwrap().key().unwrap(), g.key().unwrap());
        let bad: BTreeMap<String, String> = [(CLIENT.to_string(), "fetch".to_string())].into();
        assert!(pick.with_sites(&bad).unwrap_err().contains("unknown client"));
    }

    /// The link as a node: `link = "net(..)"` in the sites form; the free
    /// link and the client's region leave the key alone, each network has its own; the
    /// back end runs the graph as its composition.
    #[test]
    fn link_and_client_knobs() {
        let pick = Graph::from_composition(&Composition::new("gpu-compute")).unwrap();
        let with = |kv: &[(&str, &str)]| pick.with_sites(&kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        let key = |g: &Graph| g.key().unwrap();
        assert_eq!(key(&with(&[(LINK, "free")]).unwrap()), key(&pick));
        assert_eq!(key(&with(&[(CLIENT, "pick(radius=4, max_lod=3)")]).unwrap()), key(&pick), "the region is not in the key");
        let home = with(&[(LINK, "net(latency_ms=30, jitter_ms=10, loss=0.01, reorder=true)")]).unwrap();
        assert_ne!(key(&home), key(&pick));
        assert_eq!(key(&with(&[(LINK, "net(reorder=true, latency_ms=30.0, loss=0.01, jitter_ms=10)")]).unwrap()), key(&home), "knob order and 30 / 30.0");
        assert_ne!(key(&with(&[(LINK, "net(latency_ms=31, jitter_ms=10, loss=0.01, reorder=true)")]).unwrap()), key(&home));
        assert_eq!(home.link().unwrap(), mr_core::link::LinkSpec { latency_ms: 30.0, jitter_ms: 10.0, loss: 0.01, reorder: true, ..Default::default() });
        assert_eq!(home.to_composition().unwrap().label(), pick.to_composition().unwrap().label());
        let (_, form) = home.sites_form().unwrap().unwrap();
        assert_eq!(form[LINK], "net(jitter_ms=10.0, latency_ms=30.0, loss=0.01, reorder=true)");
        assert_eq!(key(&pick.with_sites(&form).unwrap()), key(&home));
        let region = with(&[(CLIENT, "derive(radius=4)"), (LINK, "net(latency_ms=1)")]).unwrap();
        assert_eq!((region.client(), region.client_knob("radius"), region.client_knob("max_lod")), ("derive", Some(4), None));
        assert_eq!(region.sites_form().unwrap().unwrap().1[CLIENT], "derive(radius=4)");
        assert!(region.label().unwrap().ends_with(", client = derive(radius=4), link = net(latency_ms=1.0)"), "{}", region.label().unwrap());
        // The chain: world -> link -> client -> the renderer's readers.
        let ck = home.check().unwrap();
        let reads = |id: &str| ck.graph.nodes[id].inputs["changes"].as_str();
        assert_eq!((reads("link"), reads("client")), ("world", "link"));
        assert_eq!(ck.devices["link"], Device::Cpu);
        for (bad, why) in [("net(latency=3)", "unknown field"), ("free(latency_ms=3)", "no knobs"), ("udp", "unknown link"), ("net(latency_ms=3", "knobs are written")] {
            assert!(with(&[(LINK, bad)]).and_then(|g| g.check().map(|_| ())).unwrap_err().contains(why), "{bad}");
        }
        for bad in ["pick(radius=0)", "pick(levels=2)", "pick(max_lod=99)"] {
            assert!(with(&[(CLIENT, bad)]).unwrap().check().is_err(), "{bad}");
        }
        // An old graph with the client reading the world gets the link in between.
        let mut old = pick.normalised().unwrap();
        old.nodes.remove("link");
        old.nodes.get_mut("client").unwrap().inputs.insert("changes".into(), "world".into());
        assert_eq!(old.normalised().unwrap().nodes["client"].inputs["changes"], "link");
        assert_eq!(key(&old), key(&pick));
    }

    /// The generator: on the server it feeds the world and is left out of the key; on the
    /// client's cpu it feeds the client, a pipeline of its own.
    /// The post after the shading, its tone mapper a choice; `clamp` keeps the key.
    #[test]
    fn post_node() {
        for backend in ["gpu-compute", "cpu", "gpu-raster", "cpu-discover", "gpu-voxelspace"] {
            let g = Graph::from_composition(&Composition::new(backend)).unwrap();
            let with = |v: &str| g.with_sites(&[(POST.to_string(), v.to_string())].into());
            let key = |g: &Graph| g.key().unwrap();
            let ck = g.check().unwrap();
            assert_eq!((ck.graph.nodes[&ck.output].role, ck.shading(), ck.tone()), (Role::Post, "shading", crate::post::Tone::Clamp), "{backend}");
            assert_eq!(ck.devices[&ck.output], ck.devices["shading"], "{backend}: with the shading");
            assert!(!g.canonical().unwrap().contains("post"), "{backend}: clamp is left out");
            assert_eq!(key(&with("clamp").unwrap()), key(&g));
            let aces = with("aces").unwrap();
            assert_ne!(key(&aces), key(&g));
            assert_eq!(aces.tone(), crate::post::Tone::Aces);
            assert_eq!(aces.accept().unwrap(), g.accept().unwrap(), "{backend}: every executor compiles any tone in");
            assert_eq!(aces.sites_value(POST), "aces");
            assert_eq!(aces.sites_form().unwrap().unwrap().1[POST], "aces");
            assert!(with("filmic").unwrap_err().contains("reinhard"));
        }
        let mut g = Graph::from_composition(&Composition::new("gpu-compute")).unwrap().check().unwrap().graph;
        g.nodes.get_mut("post").unwrap().device = Some(Device::Gpu(1));
        assert!(g.check().unwrap_err().contains("compiled into the shading's output"));
        g.nodes.get_mut("post").unwrap().device = None;
        g.nodes.get_mut("post").unwrap().knobs.insert("white".into(), toml::Value::Float(4.0));
        assert!(g.check().unwrap_err().contains("no knobs"));
    }

    #[test]
    fn generator_placement() {
        let pick = Graph::from_composition(&Composition::new("gpu-compute")).unwrap();
        let with = |kv: &[(&str, &str)]| pick.with_sites(&kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        let key = |g: &Graph| g.key().unwrap();
        assert!(!pick.generates());
        assert_eq!(key(&with(&[(GENERATOR, "server")]).unwrap()), key(&pick));
        let ck = pick.check().unwrap();
        assert_eq!((ck.graph.nodes["world"].inputs["chunks"].as_str(), ck.devices["generator"]), ("generator", Device::Server));
        assert!(!ck.graph.nodes["client"].inputs.contains_key("chunks"));
        let local = with(&[(GENERATOR, "cpu")]).unwrap();
        assert!(local.generates());
        assert_ne!(key(&local), key(&pick));
        let ck = local.check().unwrap();
        assert_eq!((ck.graph.nodes["client"].inputs["chunks"].as_str(), ck.devices["generator"]), ("generator", Device::Cpu));
        assert!(!ck.graph.nodes["world"].inputs.contains_key("chunks"));
        assert_eq!(local.sites_value(GENERATOR), "cpu");
        assert_eq!(pick.sites_value(GENERATOR), "server");
        let (_, form) = local.sites_form().unwrap().unwrap();
        assert_eq!(form[GENERATOR], "cpu");
        assert_eq!(key(&pick.with_sites(&form).unwrap()), key(&local));
        assert!(local.label().unwrap().ends_with(", generator = cpu"), "{}", local.label().unwrap());
        assert_eq!(local.to_composition().unwrap().label(), pick.to_composition().unwrap().label());
        // With the other world nodes set.
        let both = with(&[(GENERATOR, "cpu"), (CLIENT, "derive")]).unwrap();
        assert_ne!(key(&both), key(&with(&[(CLIENT, "derive")]).unwrap()));
        assert_ne!(key(&both), key(&local));
        assert!(with(&[(GENERATOR, "gpu")]).unwrap_err().contains("server"));
        let mut gpu = local.clone();
        gpu.nodes.get_mut("generator").unwrap().device = Some(Device::Gpu(0));
        assert!(gpu.check().unwrap_err().contains("the generator runs on the server"), "{}", gpu.check().unwrap_err());
        // Moved back to the server, it feeds the world again.
        assert_eq!(key(&local.with_sites(&[(GENERATOR.to_string(), "server".to_string())].into()).unwrap()), key(&pick));
    }

    /// Graphs split over GPUs are accepted where an executor runs them (ray casting's deferred
    /// shading, with its walks, on the second GPU); else the reasons.
    #[test]
    fn two_gpus() {
        let split = |b: &str, placement: &str| {
            let mut g = Graph::from_composition(&Composition::new(b)).unwrap();
            g.nodes.retain(|_, n| n.role != Role::Meshes);
            g.nodes.values_mut().for_each(|n| drop(n.inputs.remove("meshes")));
            g.nodes.insert("s.storage".into(), Node::new(Role::Storage, "prefix").with(&[("changes", "world")]));
            g.nodes.insert("s.kernel".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "s.storage")]));
            let sh = g.nodes.get_mut("shading").unwrap();
            sh.inputs.insert("shadow".into(), "s.kernel".into());
            sh.device = Some(Device::Gpu(1));
            sh.knobs.insert("placement".into(), toml::Value::String(placement.into()));
            g
        };
        for b in ["gpu-compute", "gpu-rt"] {
            let g = split(b, "deferred");
            let (_, exact) = g.accept().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert!(!exact, "{b}: the split is no flat composition (its devices would be lost)");
            let ck = g.check().unwrap();
            assert_eq!((ck.devices["s.kernel"], ck.devices["visibility"]), (Device::Gpu(1), Device::Gpu(0)));
            let sched = ck.schedule().unwrap().to_string();
            assert!(sched.contains("gpu -> gpu1"), "{b}: {sched}");
            let why = split(b, "inline").accept().unwrap_err();
            assert!(why.contains("placement = deferred"), "{b}: {why}");
            // The shading a frame behind, the GPUs overlapping; not on one GPU.
            let mut late = split(b, "deferred");
            late.nodes.get_mut("shading").unwrap().timing.insert("hits".into(), When::Prev);
            late.accept().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert_ne!(late.key().unwrap(), g.key().unwrap());
            let sched = late.check().unwrap().schedule().unwrap();
            assert!(sched.pipelined(), "{b}: {sched}");
            late.nodes.get_mut("shading").unwrap().device = None;
            let why = late.accept().unwrap_err();
            assert!(why.contains("hits prev") && why.contains("another GPU"), "{b}: {why}");
        }
        // The whole pipeline on the second GPU.
        let mut g = Graph::from_composition(&Composition::new("gpu-compute")).unwrap();
        g.nodes.values_mut().filter(|n| n.role != Role::World).for_each(|n| n.device = Some(Device::Gpu(1)));
        g.accept().unwrap();
        // Meshes on the first GPU with the shading on the second; three GPUs; a drawer's shading.
        let mut g = split("gpu-compute", "deferred");
        g.nodes.insert("m.kernel".into(), Node::new(Role::Traversal, "refloor").with(&[("storage", "storage")]));
        g.nodes.insert("meshes".into(), Node::new(Role::Meshes, "composite").with(&[("changes", "world"), ("shadow", "m.kernel")]));
        g.nodes.get_mut("visibility").unwrap().inputs.insert("meshes".into(), "meshes".into());
        let why = g.accept().unwrap_err();
        assert!(why.contains("meshes = none"), "{why}");
        let mut g = split("gpu-compute", "deferred");
        g.nodes.get_mut("storage").unwrap().device = Some(Device::Gpu(2));
        assert!(g.accept().is_err());
        let mut g = Graph::from_composition(&Composition::new("gpu-raster")).unwrap();
        g.nodes.insert("s.storage".into(), Node::new(Role::Storage, "prefix").with(&[("changes", "world")]));
        g.nodes.insert("s.kernel".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "s.storage")]));
        let sh = g.nodes.get_mut("shading").unwrap();
        sh.inputs.insert("shadow".into(), "s.kernel".into());
        sh.device = Some(Device::Gpu(1));
        let why = g.accept().unwrap_err();
        assert!(why.contains("runs on one GPU"), "{why}");
    }


    /// Any GPU ray-casting site binds any kernel with a GPU implementation: ray
    /// queries over a geometry node's acceleration structures (one per pipeline) beside
    /// walks, walks beside ray queries; the reasons where not.
    #[test]
    fn ray_query_sites() {
        let of = |b: &str| Graph::from_composition(&Composition { shading: "pathtrace".into(), ..Composition::new(b) }).unwrap();
        let rq = |g: &mut Graph, geo: &str, k: &str| {
            g.nodes.insert(geo.into(), Node::new(Role::Geometry, "greedy"));
            g.nodes.insert(k.into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", geo)]));
        };
        let walk = |g: &mut Graph| {
            g.nodes.insert("st".into(), Node::new(Role::Storage, "prefix"));
            g.nodes.insert("wk".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "st")]));
        };
        let bind = |g: &mut Graph, port: &str, k: &str| drop(g.nodes.get_mut("shading").unwrap().inputs.insert(port.into(), k.into()));
        // Shadow rays as ray queries in a walk, in GPU discovery, in CPU discovery's drawing.
        for b in ["gpu-compute", "gpu-discover", "cpu-discover"] {
            let mut g = of(b);
            rq(&mut g, "geo", "rq");
            bind(&mut g, "shadow", "rq");
            bind(&mut g, "bounce", "rq");
            let g = g.pruned();
            g.validate().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert_eq!(g.check().unwrap().devices["geo"], Device::Cpu, "{b}");
        }
        // Bounces as ray queries in the rasterisers; walks for gpu-rt's shadow rays.
        for b in ["gpu-raster", "gpu-splat"] {
            let mut g = of(b);
            rq(&mut g, "geo2", "rq");
            bind(&mut g, "bounce", "rq");
            g.validate().unwrap_or_else(|e| panic!("{b}: {e}"));
        }
        let mut g = of("gpu-rt");
        walk(&mut g);
        bind(&mut g, "shadow", "wk");
        g.validate().unwrap();
        // Reasons: geometry without acceleration structures, two of them, mesh shadows of the
        // face drawer, a CPU-only storage on the GPU (its capabilities checked first).
        let mut g = of("gpu-compute");
        g.nodes.insert("pts".into(), Node::new(Role::Geometry, "points"));
        g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "pts")]));
        bind(&mut g, "shadow", "rq");
        assert!(g.validate().unwrap_err().contains("builds no acceleration structure"));
        let mut g = of("gpu-rt");
        rq(&mut g, "geo2", "rq2");
        bind(&mut g, "shadow", "rq2");
        assert!(g.validate().unwrap_err().contains("one acceleration structure per pipeline"));
        let mut g = of("gpu-raster");
        rq(&mut g, "geo2", "rq");
        g.nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), "rq".into());
        assert!(g.validate().unwrap_err().contains("its mesh pipelines have no ray queries"));
        let mut g = of("gpu-compute");
        g.nodes.insert("st".into(), Node::new(Role::Storage, "palette"));
        g.nodes.insert("wk".into(), Node::new(Role::Traversal, "refloor").with(&[("storage", "st")]));
        bind(&mut g, "shadow", "wk");
        let e = g.validate().unwrap_err();
        assert!(e.contains("storage `palette` lacks it on the gpu"), "{e}");
    }

    #[test]
    fn cpu_discover_needs_shadow_rays() {
        // cpu-discover without GPU shadow rays cannot shade lit.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("shading").unwrap().inputs.remove("shadow");
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(e.contains("input `shadow`"), "{e}");
    }

    /// Edge timings. Hole detection is a loop through the drawer (it draws last
    /// frame's faces, the holes node finds where they leave holes, the sampler looks there,
    /// the drawer draws again) plus last frame's image read `prev`; and the reasons for what
    /// is not a pipeline.
    #[test]
    fn timings() {
        let it = Graph::from_composition(&Composition { holes: "coverage".into(), ..Composition::new("gpu-discover") }).unwrap();
        assert_eq!(it.nodes["draw"].when("faces"), When::Iterate(2));
        assert_eq!(it.nodes["holes"].when("sky"), When::Prev);
        assert_eq!(it.nodes["visibility"].inputs.get("where").map(String::as_str), Some("holes"));
        // Written as TOML and as JSON (the worker's form), canonical with its timings.
        let toml_text = toml::to_string(&it).unwrap();
        assert!(toml_text.contains("iterate = 2") && toml_text.contains("sky = \"prev\""), "{toml_text}");
        assert_eq!(toml::from_str::<Graph>(&toml_text).unwrap(), it);
        assert_eq!(serde_json::from_str::<Graph>(&serde_json::to_string(&it).unwrap()).unwrap(), it);
        let canon = it.canonical().unwrap();
        assert!(canon.contains("~iterate2") && canon.contains("~prev"), "{canon}");
        // An explicit `now` is the same pipeline as none.
        let base = Graph::from_composition(&Composition::new("gpu-discover")).unwrap();
        let mut now = base.clone();
        now.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Now);
        assert_eq!(now.key().unwrap(), base.key().unwrap());
        // The schedule: one loop, the consumer first, the shading after it.
        let sched = it.check().unwrap().schedule().unwrap();
        let names = |ps: &[Pass]| ps.iter().map(|p| p.node.clone()).collect::<Vec<_>>();
        let loops: Vec<&Step> = sched.steps.iter().filter(|s| matches!(s, Step::Loop { .. })).collect();
        let [Step::Loop { passes, body }] = loops.as_slice() else { panic!("one loop\n{sched}") };
        assert_eq!((*passes, names(body)), (2, vec!["draw".to_string(), "holes".to_string(), "visibility".to_string()]), "{sched}");
        assert!(matches!(sched.steps.last(), Some(Step::Pass(p)) if p.node == "shading"), "{sched}");
        assert!(sched.kept.iter().any(|e| e.to == "draw" && e.port == "faces") && sched.kept.iter().any(|e| e.to == "holes" && e.port == "sky"), "{sched}");
        // gpu-discover runs it, and it is the composition.
        assert_eq!(it.accept().unwrap(), (Composition { holes: "coverage".into(), ..Composition::new("gpu-discover") }, true));
        // Reasons. A cycle within the frame says how to break it.
        let mut g = it.clone();
        g.nodes.get_mut("draw").unwrap().timing.clear();
        let e = g.check().unwrap_err();
        assert!(e.contains("cycle through") && e.contains("`prev`"), "{e}");
        // Faces a frame late instead of the loop: a valid graph gpu-discover does not run.
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Prev);
        let e = g.validate().unwrap_err();
        assert!(e.contains("runs `gpu-discover: bricks + quadtree + coverage` with") && e.contains("iterate 2"), "{e}");
        let mut g = base.clone();
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Iterate(2));
        let e = g.check().unwrap_err();
        assert!(e.contains("no cycle to iterate"), "{e}");
        let mut g = it.clone();
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Iterate(1));
        let e = g.check().unwrap_err();
        assert!(e.contains("at least twice"), "{e}");
        let mut g = base.clone();
        g.nodes.get_mut("kernel").unwrap().timing.insert("storage".into(), When::Prev);
        let e = g.check().unwrap_err();
        assert!(e.contains("compiled into their caller") && e.contains("no timing"), "{e}");
        let mut g = base.clone();
        g.nodes.get_mut("visibility").unwrap().timing.insert("where".into(), When::Prev);
        let e = g.check().unwrap_err();
        assert!(e.contains("not a bound input"), "{e}");
        // A free timing outside what the back end runs.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Iterate(2));
        g.nodes.get_mut("visibility").unwrap().inputs.insert("where".into(), "draw".into());
        assert!(g.validate().is_err());
    }

    /// The schedule across devices. cpu-discover: discovery on the CPU, drawing on the GPU;
    /// faces and the change log cross devices; by default the faces arrive a frame late and
    /// the devices overlap (`pipelined = true`); `pipelined = false` spells `now`.
    #[test]
    fn schedule_across_devices() {
        let g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        let sched = g.check().unwrap().schedule().unwrap();
        let crosses = |s: &Schedule, from: &str, to: &str| s.transfers.iter().find(|e| e.from == from && e.to == to).map(|e| (e.from_device, e.to_device, e.when));
        assert_eq!(crosses(&sched, "visibility", "draw"), Some((Device::Cpu, Device::Gpu(0), When::Prev)), "{sched}");
        // The link: the server's stream into the link (in the client's process), the
        // client's messages to the GPU.
        assert_eq!(crosses(&sched, "world", "link"), Some((Device::Server, Device::Cpu, When::Now)), "{sched}");
        assert_eq!(crosses(&sched, "link", "client"), None, "{sched}");
        assert_eq!(crosses(&sched, "client", "shadow.storage"), Some((Device::Cpu, Device::Gpu(0), When::Now)), "{sched}");
        assert_eq!(crosses(&sched, "client", "storage"), None, "{sched}");
        assert!(sched.pipelined(), "{sched}");
        let vis = sched.passes().find(|p| p.node == "visibility").unwrap();
        assert_eq!((vis.device, vis.fused.clone()), (Device::Cpu, vec!["kernel".to_string(), "storage".to_string()]));
        assert!(vis.state.is_some());
        assert_eq!(g.timing_options(), vec![("pipelined", true)]);
        let off: toml::Table = toml::from_str("pipelined = false").unwrap();
        let sync = g.clone().with_options(&off);
        let sched = sync.check().unwrap().schedule().unwrap();
        assert_eq!(crosses(&sched, "visibility", "draw"), Some((Device::Cpu, Device::Gpu(0), When::Now)), "{sched}");
        assert!(!sched.pipelined());
        assert_eq!(sync.timing_options(), vec![("pipelined", false)]);
        // Both run, as the same composition; the knob carries the difference.
        assert_eq!(sync.accept().unwrap(), (Composition::new("cpu-discover"), true));
        assert_eq!(sync.flat_knobs().unwrap().get("pipelined"), Some(&toml::Value::Boolean(false)));
        assert_ne!(sync.key().unwrap(), g.key().unwrap());
    }

    /// Every composition schedules: the shading after what it reads, walking kernels and the
    /// post only compiled in, every other node a pass of its own, fused nodes compiled into a
    /// pass on their device.
    #[test]
    fn every_composition_schedules() {
        for c in every_composition() {
            let ck = Graph::from_composition(&c).unwrap().check().unwrap();
            let s = ck.schedule().unwrap_or_else(|e| panic!("{}: {e}", c.label()));
            let passes: Vec<&Pass> = s.passes().collect();
            // The output after everything it reads within the frame (discovery a frame ahead
            // runs after it).
            let at = |id: &str| passes.iter().position(|p| p.node == id);
            let out = at(ck.shading()).unwrap();
            assert!(ck.graph.nodes.keys().filter(|id| ck.graph.depends(ck.shading(), id)).filter_map(|id| at(id)).all(|i| i <= out), "{}\n{s}", c.label());
            assert!(passes[out].fused.contains(&ck.output), "{}: the post compiled into the shading\n{s}", c.label());
            // Kept values and loops only where discovery's timings are.
            let looped = s.steps.iter().any(|x| matches!(x, Step::Loop { .. }));
            assert_eq!(looped, c.holes == "coverage", "{}\n{s}", c.label());
            assert_eq!(s.kept.is_empty(), c.holes != "coverage" && c.backend != "cpu-discover", "{}\n{s}", c.label());
            for (id, n) in &ck.graph.nodes {
                let is_pass = passes.iter().any(|p| p.node == *id);
                // Ray queries build their acceleration structures from faces: a pass too.
                assert_eq!(is_pass, (n.role != Role::Traversal || n.component == "rayquery") && n.role != Role::Post, "{}: `{id}`\n{s}", c.label());
                if output(n.role, &n.component).fused() {
                    assert!(passes.iter().any(|p| p.fused.contains(id) && p.device == ck.devices[id.as_str()]), "{}: `{id}` compiled into nothing\n{s}", c.label());
                }
            }
            for e in &s.transfers {
                assert!(!e.data.fused() && e.from_device != e.to_device);
            }
        }
    }
}
