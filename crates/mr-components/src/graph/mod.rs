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
//! The model, validation with reasons, a canonical form (hashable: the cache key) and a
//! label, edits ([`Change`]) and the shipped graph files plans start from ([`SHIPPED`]).
//! Executors consume graphs ([`of_init`], [`Checked::site`]). Each executor declares the
//! nodes it builds ([`Builds`]); [`Graph::accept`] takes a valid graph whose nodes its
//! executor declares.

mod accept;
mod audit;
mod builds;
mod canonical;
mod check;
mod compose;
mod edit;
mod features;
mod gates;
mod model;
mod schedule;
mod sites;

pub use accept::*;
pub use audit::*;
pub use builds::*;
pub use check::*;
pub use edit::*;
pub use gates::*;
pub use model::*;
pub use schedule::*;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::Api;

    /// The shipped graph file `name`.
    fn file(name: &str) -> Graph {
        shipped(name).unwrap_or_else(|| panic!("no pipelines/{name}.toml"))
    }

    /// `g` with its shading path traced, the bounce rays walking what its shadow rays walk.
    fn pathtrace(mut g: Graph) -> Graph {
        let sh = g
            .nodes
            .values_mut()
            .find(|n| n.role == Role::Shading)
            .unwrap();
        sh.component = "pathtrace".into();
        if let Some(k) = sh.inputs.get("shadow").cloned() {
            sh.inputs.insert("bounce".into(), k);
        }
        g
    }

    /// The graph's world side written out (link, client, generator, post), node `id` set by `f`.
    fn world_side(g: &Graph, id: &str, f: &dyn Fn(&mut Node)) -> Graph {
        let mut g = g.normalised().unwrap();
        f(g.nodes.get_mut(id).unwrap());
        g.normalised().unwrap()
    }

    /// The bases of [`every_choice_schedules`]: each shipped file, and each accepted graph one
    /// edit from it that adds or removes a node.
    fn bases() -> Vec<Graph> {
        let mut bases = vec![];
        for (name, _) in SHIPPED {
            let g = file(name);
            bases.push(g.clone());
            let mut seen = std::collections::HashSet::new();
            for h in edits(&g).into_iter().map(Graph::pruned) {
                if !h.nodes.keys().eq(g.nodes.keys())
                    && h.accept().is_ok()
                    && seen.insert(h.key().unwrap())
                {
                    bases.push(h);
                }
            }
        }
        bases
    }

    /// Every accepted graph that is a base with each node through the components it may be
    /// (`Graph::choices`, all nodes at once) is accepted and schedules: the shading after what it reads, walking
    /// kernels and the post only compiled in, every other node a pass of its own, fused nodes
    /// compiled into a pass on their device, loops and kept values only where timings say so.
    #[test]
    fn every_choice_schedules() {
        let (mut graphs, mut keys) = (0, std::collections::HashSet::new());
        for base in bases() {
            let mut all = vec![base.clone()];
            for id in base.nodes.keys() {
                let ch = base.choices(id);
                if ch.len() > 1 {
                    all = all
                        .iter()
                        .flat_map(|g| {
                            ch.iter().map(move |c| {
                                let mut g = g.clone();
                                g.nodes.get_mut(id).unwrap().component = c.to_string();
                                g
                            })
                        })
                        .collect();
                }
            }
            for g in all {
                graphs += 1;
                if !keys.insert(g.key().unwrap()) {
                    continue;
                }
                g.accept()
                    .unwrap_or_else(|e| panic!("{}: {e}", g.label().unwrap_or_default()));
                let label = g.label().unwrap();
                assert!(label.starts_with(&format!("{}: ", g.runs_on())), "{label}");
                let ck = g.check().unwrap();
                let s = ck.schedule().unwrap_or_else(|e| panic!("{label}: {e}"));
                let passes: Vec<&Pass> = s.passes().collect();
                // The output after everything it reads within the frame (discovery a frame ahead
                // runs after it).
                let at = |id: &str| passes.iter().position(|p| p.node == id);
                let out = at(ck.shading()).unwrap();
                assert!(
                    ck.graph
                        .nodes
                        .keys()
                        .filter(|id| ck.graph.depends(ck.shading(), id))
                        .filter_map(|id| at(id))
                        .all(|i| i <= out),
                    "{label}\n{s}"
                );
                assert!(
                    passes[out].fused.contains(&ck.output),
                    "{label}: the post compiled into the shading\n{s}"
                );
                // Loops where an edge iterates, kept values where one is not `now`.
                let timed = |f: &dyn Fn(When) -> bool| {
                    ck.graph
                        .nodes
                        .values()
                        .any(|n| n.timing.values().any(|w| f(*w)))
                };
                assert_eq!(
                    s.steps.iter().any(|x| matches!(x, Step::Loop { .. })),
                    timed(&|w| matches!(w, When::Iterate(_))),
                    "{label}\n{s}"
                );
                assert_eq!(
                    s.kept.is_empty(),
                    !timed(&|w| w != When::Now),
                    "{label}\n{s}"
                );
                for (id, n) in &ck.graph.nodes {
                    let is_pass = passes.iter().any(|p| p.node == *id);
                    // Ray queries build their acceleration structures from faces: a pass too.
                    assert_eq!(
                        is_pass,
                        (n.role != Role::Traversal || n.component == "rayquery")
                            && n.role != Role::Post,
                        "{label}: `{id}`\n{s}"
                    );
                    if output(n.role, &n.component).fused() {
                        assert!(
                            passes.iter().any(
                                |p| p.fused.contains(id) && p.device == ck.devices[id.as_str()]
                            ),
                            "{label}: `{id}` compiled into nothing\n{s}"
                        );
                    }
                }
                for e in &s.transfers {
                    assert!(!e.data.fused() && e.from_device != e.to_device);
                }
            }
        }
        assert!(keys.len() > 20_000, "{graphs} graphs, {} keys", keys.len());
    }

    #[test]
    fn canonical_ignores_ids() {
        let mut g = file("raycast-gpu");
        g.nodes.get_mut("storage").unwrap().component = "prefix".into();
        g.nodes.insert(
            "shadow.storage".into(),
            Node::new(Role::Storage, "rows").with(&[("changes", "world")]),
        );
        g.nodes.insert(
            "shadow.kernel".into(),
            Node::new(Role::Traversal, "refloor").with(&[("storage", "shadow.storage")]),
        );
        for id in ["shading", "meshes"] {
            g.nodes
                .get_mut(id)
                .unwrap()
                .inputs
                .insert("shadow".into(), "shadow.kernel".into());
        }
        g.accept().unwrap();
        let mut renamed = Graph {
            executor: g.executor.clone(),
            nodes: BTreeMap::new(),
        };
        let rename = |id: &str| format!("x_{id}");
        for (id, n) in &g.nodes {
            let mut n = n.clone();
            n.inputs.values_mut().for_each(|s| *s = rename(s));
            renamed.nodes.insert(rename(id), n);
        }
        assert_eq!(renamed.canonical(), g.canonical());
        assert_eq!(renamed.key(), g.key());
        assert_ne!(file("raycast-gpu").key(), g.key());
    }

    /// A graph written by hand, without the world (implicit), is the shipped file it edits.
    #[test]
    fn written_graph() {
        let g: Graph = toml::from_str(
            r#"
            executor = "gpu"
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
        let edits = [
            Change::Remove("meshes".into()),
            Change::component("storage", "prefix"),
            Change::component("kernel", "incremental"),
            Change::Knob(
                "kernel".into(),
                "exact".into(),
                Some(toml::Value::Boolean(true)),
            ),
        ];
        assert_eq!(
            g.key().unwrap(),
            file("raycast-gpu")
                .with_edits(&edits)
                .unwrap()
                .key()
                .unwrap()
        );
        g.accept().unwrap();
        // Shadow rays through their own kernel over the same storage.
        let mut sep = g.clone();
        sep.nodes.insert(
            "k2".into(),
            Node::new(Role::Traversal, "sphere").with(&[("storage", "s")]),
        );
        sep.nodes
            .get_mut("out")
            .unwrap()
            .inputs
            .insert("shadow".into(), "k2".into());
        sep.accept().unwrap();
        assert_ne!(sep.key(), g.key());
    }

    /// Invalid graphs say why.
    #[test]
    fn reasons() {
        let base = file("raycast-gpu");
        let err = |f: &dyn Fn(&mut Graph)| {
            let mut g = base.clone();
            f(&mut g);
            g.validate().unwrap_err()
        };
        let e = err(&|g| {
            g.nodes
                .get_mut("visibility")
                .unwrap()
                .inputs
                .insert("primary".into(), "storage".into());
        });
        assert!(e.contains("takes ray queries, but `storage`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().inputs.clear();
        });
        assert!(
            e.contains("input `storage`") && e.contains("not bound"),
            "{e}"
        );
        let e = err(&|g| {
            g.nodes
                .insert("extra".into(), Node::new(Role::Storage, "rows"));
        });
        assert!(e.contains("lead to no image: `extra`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().device = Some(Device::Cpu);
        });
        assert!(e.contains("compiled into their caller"), "{e}");
        let e = err(&|g| {
            g.nodes
                .get_mut("kernel")
                .unwrap()
                .inputs
                .insert("storage".into(), "kernel".into());
        });
        assert!(e.contains("takes storage"), "{e}");
        // Two primary-ray storages, a bounce input direct shading lacks.
        let e = err(&|g| {
            g.nodes
                .insert("s2".into(), Node::new(Role::Storage, "rows"));
            g.nodes.insert(
                "k2".into(),
                Node::new(Role::Traversal, "refloor").with(&[("storage", "s2")]),
            );
            g.nodes
                .get_mut("shading")
                .unwrap()
                .inputs
                .insert("shadow".into(), "k2".into());
            g.nodes
                .get_mut("shading")
                .unwrap()
                .inputs
                .insert("bounce".into(), "kernel".into());
        });
        assert!(e.contains("no input `bounce`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("shading").unwrap().component = "pathtrace".into();
            g.nodes
                .get_mut("shading")
                .unwrap()
                .inputs
                .insert("bounce".into(), "visibility".into());
        });
        assert!(e.contains("takes ray queries"), "{e}");
        // Capability needs: sphere needs distances; storages get them through an adapter,
        // so only a storage-less source would fail, which types already reject.
        assert!(
            base.edit(&Change::component("kernel", "sphere"))
                .unwrap()
                .validate()
                .is_ok()
        );
        // Discovery across devices: CPU discovery rays and GPU shadow rays cannot share a kernel.
        let mut g = file("discover-cpu");
        g.nodes
            .get_mut("shading")
            .unwrap()
            .inputs
            .insert("shadow".into(), "kernel".into());
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(e.contains("two devices"), "{e}");
    }

    /// A node's `api`, checked against its device, its component's implementations
    /// and the executor; written into the canonical form only where it is a choice.
    #[test]
    fn apis() {
        let set = |g: &Graph, at: &[(&str, Api)]| {
            let mut g = g.clone();
            for (id, a) in at {
                g.nodes.get_mut(*id).unwrap().api = Some(*a);
            }
            g
        };
        // cpu: one API for its one loop, read as its `isa` knob; fused nodes follow.
        let cpu = file("raycast-cpu");
        let g = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx2)]);
        g.accept().unwrap();
        assert_eq!(
            g.preset_options().unwrap().get("isa"),
            Some(&toml::Value::String("avx2".into()))
        );
        let ck = g.check().unwrap();
        assert_eq!(
            ck.apis.get("kernel"),
            Some(&Api::Avx2),
            "the kernel runs with its caller"
        );
        // A node without an API of its own or its callers' runs its component's only one on
        // its device (what the ray-casting region picks its caster by).
        // A walk's kernel has two (compute, raster: `dispatch`), so it has none.
        for (name, api) in [("rayquery-gpu", Some(Api::RayQuery)), ("raycast-gpu", None)] {
            let ck = file(name).check().unwrap();
            assert_eq!(
                ck.api(ck.input(Role::Visibility, "primary").unwrap()),
                api,
                "{name}"
            );
        }
        assert!(g.canonical().unwrap().contains(":avx2") && g.key().unwrap() != cpu.key().unwrap());
        let e = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx512)])
            .validate()
            .unwrap_err();
        assert!(e.contains("with one api, its `isa`"), "{e}");
        let e = set(
            &cpu,
            &[
                ("visibility", Api::Avx2),
                ("shading", Api::Avx2),
                ("kernel", Api::Scalar),
            ],
        )
        .validate()
        .unwrap_err();
        assert!(
            e.contains("`kernel` (api scalar) is compiled into callers running avx2"),
            "{e}"
        );
        let e = set(&cpu, &[("shading", Api::Compute)])
            .validate()
            .unwrap_err();
        assert!(e.contains("runs on a gpu"), "{e}");
        // GPU executors: the API they run the stage with, and only that.
        let gc = file("raycast-gpu");
        assert!(set(&gc, &[("visibility", Api::Compute)]).validate().is_ok());
        assert_eq!(
            set(&gc, &[("visibility", Api::Compute)]).key(),
            gc.key(),
            "the default: not in the key"
        );
        // The GPU walk: a compute thread or a fragment shader per pixel (`dispatch`).
        let frag = set(&gc, &[("visibility", Api::Raster)]);
        assert_eq!(
            frag.preset_options().unwrap().get("dispatch"),
            Some(&toml::Value::String("fragment".into()))
        );
        assert!(frag.validate().is_ok() && frag.key() != gc.key());
        let e = set(
            &gc,
            &[("visibility", Api::Raster), ("shading", Api::Compute)],
        )
        .validate()
        .unwrap_err();
        assert!(e.contains("with one api, its `dispatch`"), "{e}");
        let beam = gc
            .with_edits(&[
                Change::add("prepass", Role::Prepass, "beam"),
                Change::bind("prepass", "storage", "storage"),
                Change::bind("visibility", "start", "prepass"),
            ])
            .unwrap();
        beam.accept().unwrap();
        let e = set(&beam, &[("prepass", Api::Raster)])
            .validate()
            .unwrap_err();
        assert!(e.contains("`beam` has no raster implementation"), "{e}");
        let e = set(&file("rayquery-gpu"), &[("visibility", Api::Compute)])
            .validate()
            .unwrap_err();
        assert!(
            e.contains("runs its visibility with rayquery here, not compute"),
            "{e}"
        );
        let r = file("raster-gpu");
        let e = set(&r, &[("shading", Api::Compute)])
            .validate()
            .unwrap_err();
        assert!(
            e.contains("runs its shading with raster here, not compute"),
            "{e}"
        );
        let e = set(&gc, &[("kernel", Api::RayQuery)])
            .validate()
            .unwrap_err();
        // Primary rays as ray queries run on the ray-query caster, which draws no meshes.
        assert!(
            e.contains("`meshes`: the gpu executor (for this shape) builds no meshes node"),
            "{e}"
        );
        // A face drawer's shading: in the fragment shader for `raster`, else deferred compute.
        assert!(set(&r, &[("shading", Api::Raster)]).validate().is_ok());
        assert_eq!(
            set(&r, &[("shading", Api::Raster)]).key(),
            r.key(),
            "no choice: not in the key"
        );
        let vb = r.edit(&Change::component("draw", "visbuffer")).unwrap();
        assert!(set(&vb, &[("shading", Api::Compute)]).validate().is_ok());
        // A role's API is where its executor declares it on the node's device: the walk's
        // ray-query geometry is built on the CPU.
        let mut g = gc.clone();
        g.nodes.insert(
            "geo".into(),
            Node::new(Role::Geometry, "greedy").with(&[("changes", "world")]),
        );
        g.nodes.insert(
            "rq".into(),
            Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geo")]),
        );
        g.nodes
            .get_mut("shading")
            .unwrap()
            .inputs
            .insert("shadow".into(), "rq".into());
        g.nodes.get_mut("geo").unwrap().api = Some(Api::Scalar);
        g.validate().unwrap();
        // Implementations: raster-cpu is scalar only; pathtrace has no CPU implementation.
        let e = set(&file("raster-cpu"), &[("shading", Api::Avx2)])
            .validate()
            .unwrap_err();
        assert!(
            e.contains("runs its shading with scalar here, not avx2"),
            "{e}"
        );
        assert!(
            cpu.edit(&Change::component("shading", "pathtrace"))
                .unwrap()
                .validate()
                .is_err()
        );
        // Devices follow where the executor runs a role: rasterisers build faces on the CPU.
        let ck = r.check().unwrap();
        assert_eq!(
            (
                ck.devices["geometry"],
                ck.devices["draw"],
                ck.devices["kernel"]
            ),
            (Device::Cpu, Device::Gpu(0), Device::Gpu(0))
        );
        // Plans and records: `api` round-trips through TOML.
        let t = toml::to_string(&set(&cpu, &[("visibility", Api::Avx512)])).unwrap();
        assert!(t.contains("api = \"avx512\""), "{t}");
        assert_eq!(
            toml::from_str::<Graph>(&t).unwrap(),
            set(&cpu, &[("visibility", Api::Avx512)])
        );
    }

    /// Rays off the lead bind their own kernel and storage where the executor runs them
    /// so; elsewhere they are refused with the reason.
    #[test]
    fn free_sites() {
        let with = |g: Graph, f: &dyn Fn(&mut Graph)| {
            let mut g = g;
            f(&mut g);
            g
        };
        let own = |g: &mut Graph, id: &str, st: &str, k: &str, at: &[(&str, &str)]| {
            g.nodes
                .insert(format!("{id}.storage"), Node::new(Role::Storage, st));
            g.nodes.insert(
                format!("{id}.kernel"),
                Node::new(Role::Traversal, k).with(&[("storage", &format!("{id}.storage"))]),
            );
            for (node, port) in at {
                g.nodes
                    .get_mut(*node)
                    .unwrap()
                    .inputs
                    .insert(port.to_string(), format!("{id}.kernel"));
            }
        };
        // The GPU walk: primary, shadow, bounce and mesh shadows each their own.
        let pt = pathtrace(file("raycast-gpu"));
        let g = with(pt.clone(), &|g| {
            own(g, "s", "distance", "sphere", &[("shading", "shadow")]);
            own(g, "b", "rows", "rowrun", &[("shading", "bounce")]);
            own(g, "m", "tree64", "refloor", &[("meshes", "shadow")]);
        });
        g.accept().unwrap();
        assert_eq!(
            g.label().unwrap(),
            "gpu: bricks + refloor + tree64 + refloor + composite + raycast + distance + sphere + rows + rowrun + pathtrace"
        );
        // Unbound bounces follow the shadow rays.
        let g = with(pt.clone(), &|g| {
            own(
                g,
                "s",
                "distance",
                "sphere",
                &[
                    ("shading", "shadow"),
                    ("shading", "bounce"),
                    ("meshes", "shadow"),
                ],
            )
        });
        let mut h = g.clone();
        h.nodes.get_mut("shading").unwrap().inputs.remove("bounce");
        assert_eq!(h.canonical(), g.canonical());
        let g = with(pt.clone(), &|g| {
            own(g, "b", "tree64", "sphere", &[("shading", "bounce")])
        });
        assert_eq!(
            g.label().unwrap(),
            "gpu: bricks + refloor + composite + raycast + tree64 + sphere + pathtrace"
        );
        // raster-gpu: bounces apart from the shadow rays, also as ray queries over the
        // geometry it draws.
        let r = pathtrace(file("raster-gpu"));
        with(r.clone(), &|g| {
            own(g, "b", "prefix", "incremental", &[("shading", "bounce")])
        })
        .accept()
        .unwrap();
        with(r, &|g| {
            g.nodes.insert(
                "rq".into(),
                Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]),
            );
            g.nodes
                .get_mut("shading")
                .unwrap()
                .inputs
                .insert("bounce".into(), "rq".into());
        })
        .accept()
        .unwrap();
        // discover-gpu: exposure tests read another storage than the discovery rays walk.
        let d = file("discover-gpu")
            .with_edits(&[
                Change::component("storage", "distance"),
                Change::component("kernel", "sphere"),
            ])
            .unwrap();
        with(d, &|g| {
            g.nodes
                .insert("x".into(), Node::new(Role::Storage, "bricks"));
            g.nodes
                .get_mut("visibility")
                .unwrap()
                .inputs
                .insert("exposure".into(), "x".into());
        })
        .accept()
        .unwrap();
        // raster-gpu: faces merged across block types look their blocks up in the face
        // source's storage (any GPU storage); unbound, merge = exposure is refused; the CPU
        // rasteriser reads none there.
        let exposure = |g: &mut Graph| {
            g.nodes
                .get_mut("geometry")
                .unwrap()
                .knobs
                .insert("merge".into(), "exposure".into());
        };
        let rg = file("raster-gpu");
        let bind = |g: &mut Graph, st: &str| {
            g.nodes
                .get_mut("visibility")
                .unwrap()
                .inputs
                .insert("storage".into(), st.into());
        };
        let e = with(rg.clone(), &exposure).accept().unwrap_err();
        assert!(e.contains("`visibility`.storage: not bound, and `geometry` merges"), "{e}");
        with(rg.clone(), &|g| bind(g, "storage")).accept().unwrap();
        with(rg.clone(), &|g| {
            exposure(g);
            bind(g, "storage");
        })
        .accept()
        .unwrap();
        with(rg, &|g| {
            exposure(g);
            g.nodes.insert("l".into(), Node::new(Role::Storage, "rows"));
            bind(g, "l");
        })
        .accept()
        .unwrap();
        let e = with(file("raster-cpu"), &|g| {
            g.nodes
                .get_mut("visibility")
                .unwrap()
                .inputs
                .insert("storage".into(), "storage".into());
        })
        .accept()
        .unwrap_err();
        assert!(e.contains("`visibility`.storage → `storage`: the cpu executor (for this shape) reads nothing there"), "{e}");
        // discover-cpu: exposure is the discovery storage; GPU shadow rays their own.
        let cd = file("discover-cpu");
        let e = with(cd.clone(), &|g| {
            g.nodes.insert("x".into(), Node::new(Role::Storage, "hier"));
            g.nodes
                .get_mut("visibility")
                .unwrap()
                .inputs
                .insert("exposure".into(), "x".into());
        })
        .validate()
        .unwrap_err();
        assert!(e.contains("the gpu executor (for this shape) builds 1 storage node on cpu (here: `storage`, `x`)"), "{e}");
        assert!(
            with(pathtrace(cd), &|g| own(
                g,
                "b",
                "rows",
                "rowrun",
                &[("shading", "bounce")]
            ))
            .validate()
            .is_ok()
        );
        // CPU ray casting: a shadow kernel of its own over the primary storage, not another storage.
        let cpu = file("raycast-cpu");
        with(cpu.clone(), &|g| {
            g.nodes.insert(
                "sk".into(),
                Node::new(Role::Traversal, "incremental").with(&[("storage", "storage")]),
            );
            g.nodes
                .get_mut("shading")
                .unwrap()
                .inputs
                .insert("shadow".into(), "sk".into());
        })
        .accept()
        .unwrap();
        let e = with(cpu, &|g| {
            own(g, "s", "hier", "refloor", &[("shading", "shadow")])
        })
        .validate()
        .unwrap_err();
        assert!(
            e.contains("the cpu executor (for this shape) builds 1 storage node"),
            "{e}"
        );
        // rayquery-gpu: its shading's rays free: a kernel node of their own over its
        // one acceleration structure.
        with(pathtrace(file("rayquery-gpu")), &|g| {
            g.nodes.insert(
                "rq".into(),
                Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]),
            );
            g.nodes
                .get_mut("shading")
                .unwrap()
                .inputs
                .insert("bounce".into(), "rq".into());
        })
        .accept()
        .unwrap();
    }

    /// The regions are named by a graph's shape and devices; the executor (a device kind)
    /// is where its nodes without a device run.
    #[test]
    fn regions() {
        // Discovery's sampler moved to the CPU: the shading's shadow rays get their own
        // instances on the GPU, the drawer its faces a frame late or now.
        let mut g = file("discover-gpu");
        g.nodes.get_mut("visibility").unwrap().device = Some(Device::Cpu);
        for (id, n) in [
            (
                "shadow.kernel",
                g.nodes["kernel"]
                    .clone()
                    .with(&[("storage", "shadow.storage")]),
            ),
            ("shadow.storage", g.nodes["storage"].clone()),
        ] {
            g.nodes.insert(
                id.into(),
                Node {
                    device: Some(Device::Gpu(0)),
                    ..n
                },
            );
        }
        g.nodes
            .get_mut("shading")
            .unwrap()
            .inputs
            .insert("shadow".into(), "shadow.kernel".into());
        assert_eq!(
            (g.region().map(|b| b.id), g.executor.as_str()),
            (Some(RegionId::CpuSampler), "gpu")
        );
        let now: toml::Table = toml::from_str("pipelined = false").unwrap();
        assert_eq!(
            g.key().unwrap(),
            file("discover-cpu").with_options(&now).key().unwrap()
        );
        g.nodes
            .get_mut("draw")
            .unwrap()
            .timing
            .insert("faces".into(), When::Prev);
        assert_eq!(g.key().unwrap(), file("discover-cpu").key().unwrap());
        // The drawer's face source a choice: discovery in place of meshing.
        let mut g = file("raster-gpu")
            .edit(&Change::Remove("meshes".into()))
            .unwrap();
        g.nodes.retain(|k, _| k != "geometry");
        g.nodes.insert(
            "visibility".into(),
            Node::new(Role::Visibility, "quadtree")
                .with(&[("rays", "kernel"), ("exposure", "storage")]),
        );
        assert_eq!(g.key().unwrap(), file("discover-gpu").key().unwrap());
        // The GPU's files: the same graph with or without the executor written.
        for b in [
            "raycast-gpu",
            "rayquery-gpu",
            "splat-gpu",
            "columns-gpu",
            "raster-gpu",
            "discover-gpu",
            "discover-cpu",
        ] {
            let mut g = file(b);
            assert_eq!(g.executor, "gpu", "{b}");
            g.executor.clear();
            assert_eq!(g.key().unwrap(), file(b).key().unwrap(), "{b}");
        }
        // The executor says where nodes without a device run: CPU ray casting's graph on the
        // GPU executor is GPU ray casting, which the CPU executor does not build.
        let mut g = file("raycast-cpu");
        g.executor = "gpu".into();
        assert_eq!(g.region().map(|b| b.id), Some(RegionId::GpuWalk));
        g.executor = "cpu".into();
        g.nodes
            .values_mut()
            .filter(|n| matches!(n.role, Role::Visibility | Role::Shading))
            .for_each(|n| n.device = Some(Device::Gpu(0)));
        assert_eq!(g.region().map(|b| b.id), Some(RegionId::GpuWalk));
        assert!(
            g.check().unwrap().built().unwrap_err().contains(
                "the cpu executor builds no pipeline of this shape (the gpu executor does)"
            )
        );
        // A shape no region has keeps its reasons.
        let mut g = file("raster-cpu");
        g.nodes.get_mut("draw").unwrap().device = Some(Device::Gpu(0));
        assert_eq!(g.region().map(|b| b.id), Some(RegionId::GpuMeshing));
        assert!(g.validate().is_err());
    }

    /// The client's component a choice: the executor runs the graph as before, its key and
    /// label say so; `pick` stays out of both.
    #[test]
    fn client_node() {
        let pick = file("raycast-gpu");
        let client = |c: &str| world_side(&pick, "client", &|n| n.component = c.into());
        let derive = client("derive");
        assert_eq!(derive.client(), "derive");
        assert_ne!(derive.key().unwrap(), pick.key().unwrap());
        assert_eq!(client("pick").key().unwrap(), pick.key().unwrap());
        assert_eq!(
            derive.label().unwrap(),
            "gpu: derive + bricks + refloor + composite + raycast + direct"
        );
        derive.accept().unwrap();
        assert!(client("fetch").check().unwrap_err().contains("fetch"));
    }

    /// The client's region leaves the key alone; the network between server and client is
    /// a gate on the client's `changes`, keyed by its knobs (order and 30 / 30.0 alike), and
    /// the chain stays world -> client -> the renderer's readers.
    #[test]
    fn client_knobs_and_the_stream_gate() {
        let pick = file("raycast-gpu");
        let key = |g: &Graph| g.key().unwrap();
        let knobs = |s: &str| -> toml::Table { toml::from_str(s).unwrap() };
        let client = |k: &str| world_side(&pick, "client", &|n| n.knobs = knobs(k));
        assert_eq!(key(&client("radius = 4\nmax_lod = 3")), key(&pick), "the region is not in the key");
        let gated = |k: &str| {
            let mut g = pick.normalised().unwrap();
            let mut n = Node::new(Role::Gate, "net");
            n.inputs.insert("in".into(), "world".into());
            n.knobs = knobs(k);
            g.nodes.insert("net".into(), n);
            g.nodes.get_mut("client").unwrap().inputs.insert("changes".into(), "net".into());
            g
        };
        let home = gated("latency_ms = 30\njitter_ms = 10\nloss = 0.01\nreorder = true");
        assert_ne!(key(&home), key(&pick));
        assert_eq!(key(&gated("reorder = true\nlatency_ms = 30.0\nloss = 0.01\njitter_ms = 10")), key(&home), "knob order and 30 / 30.0");
        assert_ne!(key(&gated("latency_ms = 31\njitter_ms = 10\nloss = 0.01\nreorder = true")), key(&home));
        let ck = home.check().unwrap();
        let e = ck.gates_on("client", "changes").unwrap();
        assert!(e.into_client && e.data == Data::Changes && e.source == "world");
        assert_eq!(e.gates[0].1.latency_ms, 30.0);
        let reads = |id: &str| ck.graph.nodes[id].inputs["changes"].as_str();
        assert_eq!((reads("client"), reads("storage")), ("world", "client"));
        assert!(gated("latency = 3").check().unwrap_err().contains("unknown field"));
        for bad in ["radius = 0", "levels = 2", "max_lod = 99"] {
            assert!(client(bad).check().is_err(), "{bad}");
        }
        let region = world_side(&pick, "client", &|n| (n.component, n.knobs) = ("derive".into(), knobs("radius = 4")));
        assert_eq!((region.client(), region.client_knob("radius"), region.client_knob("max_lod")), ("derive", Some(4), None));
        assert_eq!(region.label().unwrap(), "gpu: derive(radius=4) + bricks + refloor + composite + raycast + direct");
    }

    /// Gates on the generated chunks: on the client's cpu they cross into the client; with
    /// the generator on the server the edge is the host's, refused naming the gate.
    #[test]
    fn gates_on_the_generated_chunks() {
        let pick = file("raycast-gpu");
        let gated = |device: Option<Device>| {
            let mut g = pick.normalised().unwrap();
            g.nodes.get_mut("generator").unwrap().device = device;
            let mut n = Node::new(Role::Gate, "net");
            n.inputs.insert("in".into(), "generator".into());
            g.nodes.insert("g".into(), n);
            g.nodes.get_mut("world").unwrap().inputs.remove("chunks");
            g.nodes.get_mut("client").unwrap().inputs.insert("chunks".into(), "g".into());
            g
        };
        let ck = gated(Some(Device::Cpu)).check().unwrap();
        let e = ck.gates_on("client", "chunks").unwrap();
        assert!(e.into_client && e.data == Data::Chunks, "{e:?}");
        let why = gated(None).check().unwrap_err();
        assert!(why.contains("`g` on `client.chunks`") && why.contains("on the server"), "{why}");
    }

    /// The post after the shading, its tone mapper a choice; `clamp` keeps the key.
    #[test]
    fn post_node() {
        for name in [
            "raycast-gpu",
            "raycast-cpu",
            "raster-gpu",
            "discover-cpu",
            "columns-gpu",
        ] {
            let g = file(name);
            let with = |v: &str| world_side(&g, "post", &|n| n.component = v.into());
            let key = |g: &Graph| g.key().unwrap();
            let ck = g.check().unwrap();
            assert_eq!(
                (ck.graph.nodes[&ck.output].role, ck.shading(), ck.tone()),
                (Role::Post, "shading", crate::post::Tone::Clamp),
                "{name}"
            );
            assert_eq!(
                ck.devices[&ck.output], ck.devices["shading"],
                "{name}: with the shading"
            );
            assert!(
                !g.canonical().unwrap().contains("post"),
                "{name}: clamp is left out"
            );
            assert_eq!(key(&with("clamp")), key(&g));
            let aces = with("aces");
            assert_ne!(key(&aces), key(&g));
            assert_eq!(aces.tone(), crate::post::Tone::Aces);
            aces.accept()
                .unwrap_or_else(|e| panic!("{name}: every executor compiles any tone in: {e}"));
            assert!(with("filmic").check().unwrap_err().contains("filmic"));
        }
        let mut g = file("raycast-gpu").check().unwrap().graph;
        g.nodes.get_mut("post").unwrap().device = Some(Device::Gpu(1));
        assert!(
            g.check()
                .unwrap_err()
                .contains("compiled into the shading's output")
        );
        g.nodes.get_mut("post").unwrap().device = None;
        g.nodes
            .get_mut("post")
            .unwrap()
            .knobs
            .insert("white".into(), toml::Value::Float(4.0));
        assert!(g.check().unwrap_err().contains("the post's knobs are `view`"));
        // Its diagnostic view's knobs are its own.
        let knobs = &mut g.nodes.get_mut("post").unwrap().knobs;
        knobs.clear();
        knobs.insert("view".into(), toml::Value::String("steps".into()));
        knobs.insert("view_max".into(), toml::Value::Integer(64));
        assert!(g.check().is_ok());
    }

    /// The generator: on the server it feeds the world and is left out of the key; on the
    /// client's cpu it feeds the client, a pipeline of its own.
    #[test]
    fn generator_placement() {
        let pick = file("raycast-gpu");
        let on = |g: &Graph, d: Device| {
            world_side(g, "generator", &|n| {
                n.device = (d != Device::Server).then_some(d)
            })
        };
        let key = |g: &Graph| g.key().unwrap();
        assert!(!pick.generates());
        assert_eq!(key(&on(&pick, Device::Server)), key(&pick));
        let ck = pick.check().unwrap();
        assert_eq!(
            (
                ck.graph.nodes["world"].inputs["chunks"].as_str(),
                ck.devices["generator"]
            ),
            ("generator", Device::Server)
        );
        assert!(!ck.graph.nodes["client"].inputs.contains_key("chunks"));
        let local = on(&pick, Device::Cpu);
        assert!(local.generates());
        assert_ne!(key(&local), key(&pick));
        let ck = local.check().unwrap();
        assert_eq!(
            (
                ck.graph.nodes["client"].inputs["chunks"].as_str(),
                ck.devices["generator"]
            ),
            ("generator", Device::Cpu)
        );
        assert!(!ck.graph.nodes["world"].inputs.contains_key("chunks"));
        assert_eq!(
            local.label().unwrap(),
            "gpu: procedural@cpu + pick + bricks + refloor + composite + raycast + direct"
        );
        // With the other world nodes set.
        let derive = world_side(&pick, "client", &|n| n.component = "derive".into());
        let both = on(&derive, Device::Cpu);
        assert_ne!(key(&both), key(&derive));
        assert_ne!(key(&both), key(&local));
        let mut gpu = local.clone();
        gpu.nodes.get_mut("generator").unwrap().device = Some(Device::Gpu(0));
        assert!(
            gpu.check()
                .unwrap_err()
                .contains("the generator runs on the server"),
            "{}",
            gpu.check().unwrap_err()
        );
        // Moved back to the server, it feeds the world again.
        assert_eq!(key(&on(&local, Device::Server)), key(&pick));
    }

    /// Graphs split over GPUs are accepted where an executor runs them (ray casting's deferred
    /// shading, with its walks, on the second GPU); else the reasons.
    #[test]
    fn two_gpus() {
        let split = |b: &str, placement: &str| {
            let mut g = file(b);
            g.nodes.retain(|_, n| n.role != Role::Meshes);
            g.nodes
                .values_mut()
                .for_each(|n| drop(n.inputs.remove("meshes")));
            g.nodes.insert(
                "s.storage".into(),
                Node::new(Role::Storage, "prefix").with(&[("changes", "world")]),
            );
            g.nodes.insert(
                "s.kernel".into(),
                Node::new(Role::Traversal, "incremental").with(&[("storage", "s.storage")]),
            );
            let sh = g.nodes.get_mut("shading").unwrap();
            sh.inputs.insert("shadow".into(), "s.kernel".into());
            sh.device = Some(Device::Gpu(1));
            sh.knobs
                .insert("placement".into(), toml::Value::String(placement.into()));
            g
        };
        for b in ["raycast-gpu", "rayquery-gpu"] {
            let g = split(b, "deferred");
            g.accept().unwrap_or_else(|e| panic!("{b}: {e}"));
            let ck = g.check().unwrap();
            assert_eq!(
                (ck.devices["s.kernel"], ck.devices["visibility"]),
                (Device::Gpu(1), Device::Gpu(0))
            );
            let sched = ck.schedule().unwrap().to_string();
            assert!(sched.contains("gpu -> gpu1"), "{b}: {sched}");
            let why = split(b, "inline").accept().unwrap_err();
            assert!(why.contains("placement = deferred"), "{b}: {why}");
            // The shading a frame behind, the GPUs overlapping; not on one GPU.
            let mut late = split(b, "deferred");
            late.nodes
                .get_mut("shading")
                .unwrap()
                .timing
                .insert("hits".into(), When::Prev);
            late.accept().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert_ne!(late.key().unwrap(), g.key().unwrap());
            let sched = late.check().unwrap().schedule().unwrap();
            assert!(sched.pipelined(), "{b}: {sched}");
            late.nodes.get_mut("shading").unwrap().device = None;
            let why = late.accept().unwrap_err();
            assert!(
                why.contains("hits prev") && why.contains("another GPU"),
                "{b}: {why}"
            );
        }
        // The whole pipeline on the second GPU.
        let mut g = file("raycast-gpu");
        g.nodes
            .values_mut()
            .filter(|n| n.role != Role::World)
            .for_each(|n| n.device = Some(Device::Gpu(1)));
        g.accept().unwrap();
        // Meshes on the first GPU with the shading on the second; three GPUs; a drawer's shading.
        let mut g = split("raycast-gpu", "deferred");
        g.nodes.insert(
            "m.kernel".into(),
            Node::new(Role::Traversal, "refloor").with(&[("storage", "storage")]),
        );
        g.nodes.insert(
            "meshes".into(),
            Node::new(Role::Meshes, "composite")
                .with(&[("changes", "world"), ("shadow", "m.kernel")]),
        );
        g.nodes
            .get_mut("visibility")
            .unwrap()
            .inputs
            .insert("meshes".into(), "meshes".into());
        let why = g.accept().unwrap_err();
        assert!(why.contains("meshes = none"), "{why}");
        let mut g = split("raycast-gpu", "deferred");
        g.nodes.get_mut("storage").unwrap().device = Some(Device::Gpu(2));
        assert!(g.accept().is_err());
        let mut g = file("raster-gpu");
        g.nodes.insert(
            "s.storage".into(),
            Node::new(Role::Storage, "prefix").with(&[("changes", "world")]),
        );
        g.nodes.insert(
            "s.kernel".into(),
            Node::new(Role::Traversal, "incremental").with(&[("storage", "s.storage")]),
        );
        let sh = g.nodes.get_mut("shading").unwrap();
        sh.inputs.insert("shadow".into(), "s.kernel".into());
        sh.device = Some(Device::Gpu(1));
        // A drawer's shading on the second GPU: the gbuffer's records cross (and the
        // visbuffer's, decoded from its ids; forward shading has none), meshes stay on one GPU.
        let why = g.accept().unwrap_err();
        assert!(why.contains("draw = gbuffer") && why.contains("meshes = none"), "{why}");
        let g = g
            .with_edits(&["meshes", "kernel", "storage"].map(|n| Change::Remove(n.into())).into_iter().chain([Change::component("draw", "gbuffer")]).collect::<Vec<_>>())
            .unwrap();
        g.accept().unwrap();
        let sched = g.check().unwrap().schedule().unwrap().to_string();
        assert!(sched.contains("gpu -> gpu1"), "{sched}");
        g.with_edits(&[Change::component("draw", "visbuffer")]).unwrap().accept().unwrap();
        let why = g
            .with_edits(&[Change::component("draw", "raster")])
            .unwrap()
            .accept()
            .unwrap_err();
        assert!(why.contains("draw = gbuffer or visbuffer"), "{why}");
        // Splats: their ids are deferred by construction (decoded into records on the first
        // GPU); the shading's walks go with it.
        let mut g = file("splat-gpu");
        g.nodes.get_mut("shading").unwrap().device = Some(Device::Gpu(1));
        g.accept().unwrap();
        let ck = g.check().unwrap();
        assert_eq!((ck.devices["kernel"], ck.devices["visibility"]), (Device::Gpu(1), Device::Gpu(0)));
        let sched = ck.schedule().unwrap().to_string();
        assert!(sched.contains("gpu -> gpu1"), "{sched}");
    }

    /// Any GPU ray-casting site binds any kernel with a GPU implementation: ray
    /// queries over a geometry node's acceleration structures (one per pipeline) beside
    /// walks, walks beside ray queries; the reasons where not.
    #[test]
    fn ray_query_sites() {
        let of = |b: &str| pathtrace(file(b));
        let rq = |g: &mut Graph, geo: &str, k: &str| {
            g.nodes
                .insert(geo.into(), Node::new(Role::Geometry, "greedy"));
            g.nodes.insert(
                k.into(),
                Node::new(Role::Traversal, "rayquery").with(&[("geometry", geo)]),
            );
        };
        let walk = |g: &mut Graph| {
            g.nodes
                .insert("st".into(), Node::new(Role::Storage, "prefix"));
            g.nodes.insert(
                "wk".into(),
                Node::new(Role::Traversal, "incremental").with(&[("storage", "st")]),
            );
        };
        let bind = |g: &mut Graph, port: &str, k: &str| {
            drop(
                g.nodes
                    .get_mut("shading")
                    .unwrap()
                    .inputs
                    .insert(port.into(), k.into()),
            )
        };
        // Shadow rays as ray queries in a walk, in GPU discovery, in CPU discovery's drawing.
        for b in ["raycast-gpu", "discover-gpu", "discover-cpu"] {
            let mut g = of(b);
            rq(&mut g, "geo", "rq");
            bind(&mut g, "shadow", "rq");
            bind(&mut g, "bounce", "rq");
            let g = g.pruned();
            g.validate().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert_eq!(g.check().unwrap().devices["geo"], Device::Cpu, "{b}");
        }
        // Bounces as ray queries in the rasterisers; walks for rayquery-gpu's shadow rays.
        for b in ["raster-gpu", "splat-gpu"] {
            let mut g = of(b);
            rq(&mut g, "geo2", "rq");
            bind(&mut g, "bounce", "rq");
            g.validate().unwrap_or_else(|e| panic!("{b}: {e}"));
        }
        let mut g = of("rayquery-gpu");
        walk(&mut g);
        bind(&mut g, "shadow", "wk");
        g.validate().unwrap();
        // Reasons: geometry without acceleration structures, two of them, mesh shadows of the
        // face drawer, a CPU-only storage on the GPU (its capabilities checked first).
        let mut g = of("raycast-gpu");
        g.nodes
            .insert("pts".into(), Node::new(Role::Geometry, "points"));
        g.nodes.insert(
            "rq".into(),
            Node::new(Role::Traversal, "rayquery").with(&[("geometry", "pts")]),
        );
        bind(&mut g, "shadow", "rq");
        let e = g.validate().unwrap_err();
        assert!(e.contains("`rq` (rayquery) needs accel from `pts`: geometry `points` lacks it"), "{e}");
        let mut g = of("rayquery-gpu");
        rq(&mut g, "geo2", "rq2");
        bind(&mut g, "shadow", "rq2");
        let e = g.validate().unwrap_err();
        assert!(
            e.contains("the gpu executor (for this shape) builds 1 geometry node"),
            "{e}"
        );
        let mut g = of("raster-gpu");
        rq(&mut g, "geo2", "rq");
        g.nodes
            .get_mut("meshes")
            .unwrap()
            .inputs
            .insert("shadow".into(), "rq".into());
        let e = g.validate().unwrap_err();
        assert!(e.contains("`meshes`.shadow → `rq`: the gpu executor (for this shape) reads `refloor`, `incremental`, `sphere`, `rowrun` here, not `rayquery`"), "{e}");
        let mut g = of("raycast-gpu");
        g.nodes
            .insert("st".into(), Node::new(Role::Storage, "palette"));
        g.nodes.insert(
            "wk".into(),
            Node::new(Role::Traversal, "refloor").with(&[("storage", "st")]),
        );
        bind(&mut g, "shadow", "wk");
        let e = g.validate().unwrap_err();
        assert!(e.contains("storage `palette` lacks it on the gpu"), "{e}");
    }

    #[test]
    fn cpu_discover_needs_shadow_rays() {
        // discover-cpu without GPU shadow rays cannot shade lit.
        let mut g = file("discover-cpu");
        g.nodes.get_mut("shading").unwrap().inputs.remove("shadow");
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(
            e.contains("builds at least 1 traversal node on gpu (here: none)"),
            "{e}"
        );
    }

    /// Edge timings. Hole detection is a loop through the drawer (it draws last
    /// frame's faces, the holes node finds where they leave holes, the sampler looks there,
    /// the drawer draws again) plus last frame's image read `prev`; and the reasons for what
    /// is not a pipeline.
    #[test]
    fn timings() {
        let base = file("discover-gpu");
        let it = base
            .with_edits(&[
                Change::add("holes", Role::Holes, "coverage"),
                Change::bind("holes", "covered", "draw"),
                Change::bind("holes", "sky", "draw"),
                Change::bind("holes", "changes", "world"),
                Change::Timing("holes".into(), "sky".into(), When::Prev),
                Change::bind("visibility", "where", "holes"),
                Change::Timing("draw".into(), "faces".into(), When::Iterate(2)),
            ])
            .unwrap();
        // Written as TOML and as JSON (the worker's form), canonical with its timings.
        let toml_text = toml::to_string(&it).unwrap();
        assert!(
            toml_text.contains("iterate = 2") && toml_text.contains("sky = \"prev\""),
            "{toml_text}"
        );
        assert_eq!(toml::from_str::<Graph>(&toml_text).unwrap(), it);
        assert_eq!(
            serde_json::from_str::<Graph>(&serde_json::to_string(&it).unwrap()).unwrap(),
            it
        );
        let canon = it.canonical().unwrap();
        assert!(
            canon.contains("~iterate2") && canon.contains("~prev"),
            "{canon}"
        );
        // An explicit `now` is the same pipeline as none.
        let mut now = base.clone();
        now.nodes
            .get_mut("draw")
            .unwrap()
            .timing
            .insert("faces".into(), When::Now);
        assert_eq!(now.key().unwrap(), base.key().unwrap());
        // The schedule: one loop, the consumer first, the shading after it.
        let sched = it.check().unwrap().schedule().unwrap();
        let names = |ps: &[Pass]| ps.iter().map(|p| p.node.clone()).collect::<Vec<_>>();
        let loops: Vec<&Step> = sched
            .steps
            .iter()
            .filter(|s| matches!(s, Step::Loop { .. }))
            .collect();
        let [Step::Loop { passes, body }] = loops.as_slice() else {
            panic!("one loop\n{sched}")
        };
        assert_eq!(
            (*passes, names(body)),
            (
                2,
                vec![
                    "draw".to_string(),
                    "holes".to_string(),
                    "visibility".to_string()
                ]
            ),
            "{sched}"
        );
        assert!(
            matches!(sched.steps.last(), Some(Step::Pass(p)) if p.node == "shading"),
            "{sched}"
        );
        assert!(
            sched
                .kept
                .iter()
                .any(|e| e.to == "draw" && e.port == "faces")
                && sched
                    .kept
                    .iter()
                    .any(|e| e.to == "holes" && e.port == "sky"),
            "{sched}"
        );
        // discover-gpu runs it.
        it.accept().unwrap();
        // Reasons. A cycle within the frame says how to break it.
        let mut g = it.clone();
        g.nodes.get_mut("draw").unwrap().timing.clear();
        let e = g.check().unwrap_err();
        assert!(e.contains("cycle through") && e.contains("`prev`"), "{e}");
        // Faces a frame late instead of the loop: a valid graph discover-gpu does not build.
        g.nodes
            .get_mut("draw")
            .unwrap()
            .timing
            .insert("faces".into(), When::Prev);
        let e = g.validate().unwrap_err();
        assert!(e.contains("`draw`.faces: the gpu executor (for this shape) runs this edge now or iterate 2, not prev"), "{e}");
        let mut g = base.clone();
        g.nodes
            .get_mut("draw")
            .unwrap()
            .timing
            .insert("faces".into(), When::Iterate(2));
        let e = g.check().unwrap_err();
        assert!(e.contains("no cycle to iterate"), "{e}");
        let mut g = it.clone();
        g.nodes
            .get_mut("draw")
            .unwrap()
            .timing
            .insert("faces".into(), When::Iterate(1));
        let e = g.check().unwrap_err();
        assert!(e.contains("at least twice"), "{e}");
        let mut g = base.clone();
        g.nodes
            .get_mut("kernel")
            .unwrap()
            .timing
            .insert("storage".into(), When::Prev);
        let e = g.check().unwrap_err();
        assert!(
            e.contains("compiled into their caller") && e.contains("no timing"),
            "{e}"
        );
        let mut g = base.clone();
        g.nodes
            .get_mut("visibility")
            .unwrap()
            .timing
            .insert("where".into(), When::Prev);
        let e = g.check().unwrap_err();
        assert!(e.contains("not a bound input"), "{e}");
        // A free timing outside what the region runs.
        let mut g = file("discover-cpu");
        g.nodes
            .get_mut("draw")
            .unwrap()
            .timing
            .insert("faces".into(), When::Iterate(2));
        g.nodes
            .get_mut("visibility")
            .unwrap()
            .inputs
            .insert("where".into(), "draw".into());
        assert!(g.validate().is_err());
    }

    /// The schedule across devices. discover-cpu: discovery on the CPU, drawing on the GPU;
    /// faces and the change log cross devices; by default the faces arrive a frame late and
    /// the devices overlap (`pipelined = true`); `pipelined = false` spells `now`.
    #[test]
    fn schedule_across_devices() {
        let g = file("discover-cpu");
        let sched = g.check().unwrap().schedule().unwrap();
        let crosses = |s: &Schedule, from: &str, to: &str| {
            s.transfers
                .iter()
                .find(|e| e.from == from && e.to == to)
                .map(|e| (e.from_device, e.to_device, e.when))
        };
        assert_eq!(
            crosses(&sched, "visibility", "draw"),
            Some((Device::Cpu, Device::Gpu(0), When::Prev)),
            "{sched}"
        );
        // The server's stream into the client (in the client's process), the client's
        // messages to the GPU.
        assert_eq!(
            crosses(&sched, "world", "client"),
            Some((Device::Server, Device::Cpu, When::Now)),
            "{sched}"
        );
        assert_eq!(
            crosses(&sched, "client", "shadow.storage"),
            Some((Device::Cpu, Device::Gpu(0), When::Now)),
            "{sched}"
        );
        assert_eq!(crosses(&sched, "client", "storage"), None, "{sched}");
        assert!(sched.pipelined(), "{sched}");
        let vis = sched.passes().find(|p| p.node == "visibility").unwrap();
        assert_eq!(
            (vis.device, vis.fused.clone()),
            (
                Device::Cpu,
                vec!["kernel".to_string(), "storage".to_string()]
            )
        );
        assert!(vis.state.is_some());
        assert_eq!(g.timing_options(), vec![("pipelined", true)]);
        let off: toml::Table = toml::from_str("pipelined = false").unwrap();
        let sync = g.clone().with_options(&off);
        let sched = sync.check().unwrap().schedule().unwrap();
        assert_eq!(
            crosses(&sched, "visibility", "draw"),
            Some((Device::Cpu, Device::Gpu(0), When::Now)),
            "{sched}"
        );
        assert!(!sched.pipelined());
        assert_eq!(sync.timing_options(), vec![("pipelined", false)]);
        // Both run; the knob carries the difference.
        sync.accept().unwrap();
        assert_eq!(
            sync.preset_options().unwrap().get("pipelined"),
            Some(&toml::Value::Boolean(false))
        );
        assert_ne!(sync.key().unwrap(), g.key().unwrap());
    }

    /// Every node needs an implementation on its device's kind, whatever its API.
    #[test]
    fn nodes_need_an_implementation_on_their_device() {
        let g = file("raycast-cpu")
            .edit(&Change::component("kernel", "rowrun"))
            .unwrap();
        let why = g.check().unwrap_err();
        assert!(why.contains("`rowrun` has no cpu implementation"), "{why}");
        assert!(!why.contains("refused"), "{why}");
    }

    /// A kind refused by nature says why; every refusal names a registered component that
    /// really lacks that kind.
    #[test]
    fn refusals_give_their_reason() {
        for &(role, name, kind, _) in crate::REFUSED {
            let c = crate::component(role, name).expect("a registered component");
            assert!(
                c.on(kind).is_none(),
                "{name} is implemented on {}",
                kind.name()
            );
        }
        // The ray-query graph with its passes moved to the CPU.
        let mut g = file("rayquery-gpu");
        for id in ["kernel", "shading", "visibility"] {
            g = g
                .edit(&Change::Device(id.into(), Some(Device::Cpu)))
                .unwrap();
        }
        let why = g.check().unwrap_err();
        assert!(
            why.contains("`rayquery` has no cpu implementation"),
            "{why}"
        );
        assert!(
            why.contains("refused: it is the GPU's ray-tracing API"),
            "{why}"
        );
    }
}
