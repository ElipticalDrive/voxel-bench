//! Tests of plan loading, renderers, sweeps and command-line spellings.

use super::cli::cli_edit;
use super::*;
use mr_components::graph::RegionId;
use mr_core::PassMode;

/// The region that builds a pipeline's graph (`None` for a standalone renderer).
fn region(r: &RendererSpec) -> Option<RegionId> {
    r.pipeline().and_then(|g| g.region()).map(|b| b.id)
}

fn renderer(text: &str) -> anyhow::Result<RendererSpec> {
    Ok(toml::from_str(text)?)
}

fn key(r: &RendererSpec) -> String {
    r.pipeline().unwrap().key().unwrap()
}

/// A renderer written as a graph file plus edits is that pipeline, and is written back
/// as it was read until its graph changes.
#[test]
fn graph_files_and_edits() {
    use mr_components::graph::{Change, REGIONS, SHIPPED};
    // Every shipped file, bare: the default pipeline of its name, written back as the file.
    assert_eq!(SHIPPED.len(), REGIONS.len());
    for (name, _) in SHIPPED {
        let r = renderer(&format!("graph = \"{name}\"")).unwrap();
        let d = RendererSpec::from_cli(name).unwrap();
        assert_eq!(key(&r), key(&d), "{name}");
        assert_eq!((region(&r), r.label()), (region(&d), d.label()), "{name}");
        assert_eq!(toml::to_string(&r).unwrap().trim(), format!("graph = \"{name}\"\n\n[options]"), "{name}");
    }
    // Edits: a component, a new node bound, an API, a timing, a node knob.
    for (edits, label) in [
        (r#"[{ node = "storage", component = "tree64" }, { node = "kernel", component = "incremental" }]"#, "gpu: tree64 + incremental + composite + raycast + direct"),
        (r#"[{ node = "shading", component = "pathtrace" }, { add = "far", role = "storage", component = "tree64" }, { node = "far", input = "changes", from = "world" }, { add = "sphere", role = "traversal", component = "sphere" }, { node = "sphere", input = "storage", from = "far" }, { node = "shading", input = "bounce", from = "sphere" }]"#, "gpu: bricks + refloor + composite + raycast + tree64 + sphere + pathtrace"),
    ] {
        let text = format!("graph = \"raycast-gpu\"\nedits = {edits}");
        let r = renderer(&text).unwrap_or_else(|e| panic!("{edits}: {e:#}"));
        assert_eq!((region(&r), r.label()), (Some(RegionId::GpuWalk), label), "{edits}");
        assert_ne!(key(&r), key(&renderer("graph = \"raycast-gpu\"").unwrap()), "{edits}");
        // Written as read, and read back as the same renderer.
        let w = toml::to_string(&r).unwrap();
        assert!(w.starts_with("graph = \"raycast-gpu\"\n") && w.contains("[[edits]]"), "{w}");
        assert_eq!(key(&renderer(&w).unwrap()), key(&r));
    }
    // Options stay options beside it; a knob set by an edit is the node's.
    let r = renderer("graph = \"columns-cpu\"\nedits = [{ node = \"shading\", knob = \"shadows\", value = \"sun\" }]\noptions = { threads = 2 }").unwrap();
    let ck = r.worker_graph().unwrap().unwrap().check().unwrap();
    assert_eq!(ck.node(ck.shading()).knobs.get("shadows").and_then(|v| v.as_str()), Some("sun"));
    assert_eq!(r.options.get("threads").and_then(|v| v.as_integer()), Some(2));
    let w = toml::to_string(&r).unwrap();
    assert!(w.contains("graph = \"columns-cpu\"") && w.contains("[[edits]]"), "{w}");
    // The graph changed since (the editor, a slot switch): written by what it is now.
    let mut r = renderer("graph = \"raycast-gpu\"").unwrap();
    r.graph = Some(r.graph.clone().unwrap().edit(&Change::Component("storage".into(), "tree64".into())).unwrap());
    r.resolve().unwrap();
    let w = toml::to_string(&r).unwrap();
    assert!(!w.contains("graph = \"raycast-gpu\"") && w.contains("tree64"), "{w}");
    assert_eq!(key(&renderer(&w).unwrap()), key(&r));
    // An inline graph takes edits too.
    let inline = toml::to_string(&mr_components::graph::shipped("raycast-gpu").unwrap()).unwrap().replace("[nodes", "[graph.nodes").replace("executor =", "[graph]\nexecutor =");
    let r = renderer(&format!("edits = [{{ node = \"storage\", component = \"tree64\" }}]\n{inline}")).unwrap();
    assert_eq!(r.graph.as_ref().unwrap().nodes["storage"].component, "tree64");
    // Refused with the reason.
    for (text, why) in [
        ("graph = \"no-such-graph\"", "neither a shipped graph (raycast-gpu, "),
        ("graph = \"raycast-gpu\"\nedits = [{ node = \"nowhere\", component = \"tree64\" }]", "edit 1 (`"),
        ("graph = \"raycast-gpu\"\nedits = [{ node = \"storage\", component = \"heightmap\" }]", "renderer graph: "),
        ("name = \"cpu-reference\"\nedits = [{ remove = \"storage\" }]", "`edits` change a `graph`"),
        ("graph = \"raycast-gpu\"\nname = \"cpu-reference\"", "not both"),
        // The flat form is gone, and a graph file is not a renderer's name.
        ("compose = { preset = \"raycast-gpu\" }", "unknown field `compose`"),
        ("graph = \"raycast-gpu\"\nsites = { shadow = \"incremental\" }", "unknown field `sites`"),
        ("name = \"raycast-gpu\"", "write `graph = \"raycast-gpu\"`"),
        ("label = \"nothing\"", "needs a `graph`"),
    ] {
        let e = format!("{:#}", renderer(text).unwrap_err());
        assert!(e.contains(why), "{text}: {e}");
    }
}

/// The region is the graph's client node, the network into it a gate; the run's
/// `view_radius` and `max_lod` are the defaults of clients without their own.
/// Options that are node knobs reach the worker on their nodes, where the executors
/// read them; the preset's own (`threads`) stay options.
#[test]
fn node_knobs_reach_the_worker_graph() {
    let mut r = RendererSpec::from_file("columns-cpu").unwrap();
    r.options.insert("shadows".into(), "sun".into());
    r.options.insert("threads".into(), 2.into());
    r.resolve().unwrap();
    let ck = r.worker_graph().unwrap().unwrap().check().unwrap();
    assert_eq!(ck.node(ck.shading()).knobs.get("shadows").and_then(|v| v.as_str()), Some("sun"));
    assert!(ck.graph.nodes.values().all(|n| !n.knobs.contains_key("threads")));
    assert_eq!(r.options.get("threads").and_then(|v| v.as_integer()), Some(2));
}

#[test]
fn region_lives_in_nodes() {
    let scene = "[scene]\ngenerator = { type = \"terrain\", seed = 1 }\ncamera = { type = \"line\", from = [0, 150, 0], to = [300, 150, 75] }\n";
    let r = "[[renderer]]\ngraph = \"raycast-gpu\"\n";
    let link = |knobs: &str| format!("{{ add = \"net\", role = \"gate\", component = \"net\" }}, {{ node = \"net\", input = \"in\", from = \"world\" }}, {{ node = \"net\", knobs = {{ {knobs} }} }}, {{ node = \"storage\", input = \"changes\", from = \"net\" }}");
    let home = link("latency_ms = 30, jitter_ms = 10, bandwidth_mbps = 200, loss = 0.01, reorder = true");
    let fast = link("latency_ms = 1, bandwidth_mbps = 1000");
    let new = format!(
        "{scene}[run]\nview_radius = 8\n\
         {r}\n{r}edits = [{fast}]\n{r}edits = [{home}]\n{r}edits = [{home}, {{ add = \"client\", role = \"client\", component = \"derive\" }}, {{ node = \"client\", knobs = {{ radius = 4, max_lod = 2 }} }}]\n\
         [[renderer]]\ngraph = {{ executor = \"gpu\", nodes = {{ s = {{ role = \"shading\", component = \"direct\", inputs = {{ hits = \"v\" }} }}, v = {{ role = \"visibility\", component = \"raycast\", inputs = {{ primary = \"k\" }} }}, k = {{ role = \"traversal\", component = \"refloor\", inputs = {{ storage = \"b\" }} }}, b = {{ role = \"storage\", component = \"bricks\" }}, \
         w = {{ role = \"world\", component = \"world\" }}, net = {{ role = \"gate\", component = \"net\", inputs = {{ in = \"w\" }}, knobs = {{ latency_ms = 30, jitter_ms = 10, bandwidth_mbps = 200, loss = 0.01, reorder = true }} }}, c = {{ role = \"client\", component = \"pick\", inputs = {{ changes = \"net\" }}, knobs = {{ radius = 6 }} }} }} }}\n\
         [[renderer]]\nname = \"cpu-reference\"\n"
    );
    let new = Plan::parse(&new, &[]).unwrap();
    assert_eq!((new.renderers[0].view_radius(&new.run), new.renderers[4].view_radius(&new.run)), (8, 6));
    assert_eq!(new.renderers[3].max_lod(&new.run, 8), 2);
    assert_eq!(new.renderers[2].world_gates(), new.renderers[4].world_gates());
    assert_eq!(new.renderers[2].world_gates()[0].1.latency_ms, 30.0);
    // The fields of the region and link outside the graph are gone.
    for old in [format!("{scene}[run]\nlink = {{ latency_ms = 30 }}\n{r}"), format!("{scene}{r}view_radius = 4\n"), format!("{scene}{r}link = {{ latency_ms = 1 }}\n")] {
        assert!(Plan::parse(&old, &[]).is_err(), "{old}");
    }
    let free = &new.renderers[0];
    assert!(free.world_gates().is_empty() && free.client_knob("radius").is_none());
    assert_eq!(new.renderers[3].view_radius(&new.run), 4);
    assert_ne!(new.renderers[1].pipeline().unwrap().key(), free.pipeline().unwrap().key(), "each network its own key");
    let mut far = new.renderers[2].clone();
    far.set_client_knob("radius", Some(3)).unwrap();
    assert_eq!(far.pipeline().unwrap().key(), new.renderers[2].pipeline().unwrap().key(), "the radius is not in the key");
    // Written back as it was read.
    let again = Plan::parse(&toml::to_string(&new).unwrap(), &[]).unwrap();
    for (a, b) in again.renderers.iter().zip(&new.renderers) {
        assert_eq!((a.label(), a.pipeline().map(|g| g.key().unwrap()), a.world_gates(), a.view_radius(&again.run)), (b.label(), b.pipeline().map(|g| g.key().unwrap()), b.world_gates(), b.view_radius(&new.run)));
    }
    // The CLI: commas inside an edit's braces are its own.
    let cli = RendererSpec::from_cli("raycast-gpu,+net=gate:net,{node=\"net\",input=\"in\",from=\"world\"},{node=\"net\",knobs={latency_ms=30,jitter_ms=10,bandwidth_mbps=200,loss=0.01,reorder=true}},{node=\"storage\",input=\"changes\",from=\"net\"},+client=client:pick,client.radius=4").unwrap();
    assert_eq!((cli.world_gates(), cli.view_radius(&new.run)), (new.renderers[2].world_gates(), 4));
    // The GUI's knob setter, both ways.
    let mut s = new.renderers[0].clone();
    s.set_client_knob("max_lod", Some(2)).unwrap();
    assert_eq!((s.max_lod(&new.run, 8), s.client_knob("max_lod"), s.levels()), (2, Some(2), mr_core::client::Levels::Pick));
    s.set_client_knob("max_lod", None).unwrap();
    // The client the setter added stays, without the knob: the same pipeline by its key.
    assert_eq!((s.client_knob("max_lod"), key(&s)), (None, key(free)));
    let mut none = free.clone();
    none.set_client_knob("max_lod", None).unwrap();
    assert_eq!(none.graph, free.graph, "nothing to unset: nothing added");
}

/// A plan past a size the pipelines can address is refused at load, naming the limit: a
/// resolution past `u32::MAX` pixels, a camera past the world's reach (2^30 blocks: a camera
/// 64 blocks short of `i32::MAX` rendered the far side of the world, with no error), a
/// generator size past it.
#[test]
fn sizes_past_the_limits_are_refused() {
    let text = r#"
        [scene]
        generator = { type = "flat" }
        camera = { type = "static", position = [1073741824.0, 10.0, -1073741824.0] }
        [[renderer]]
        name = "cpu-reference"
    "#;
    assert!(Plan::parse(text, &[]).is_ok(), "at the reach");
    let e = Plan::parse(text, &["run.resolution=[65536, 65536]".into()]).unwrap_err().to_string();
    assert!(e.contains("u32"), "{e}");
    let e = Plan::parse(&text.replace("1073741824.0, 10", "1073741825.0, 10"), &[]).unwrap_err().to_string();
    assert!(e.contains("2^30"), "{e}");
    let e = Plan::parse(&text.replace("camera = { type = \"static\", ", "camera = { type = \"orbit\", center = [0.0, 0.0, 0.0], radius = 2e9, period = 9, ").replace("position = [1073741824.0, 10.0, -1073741824.0] ", ""), &[]).unwrap_err().to_string();
    assert!(e.contains("2^30"), "{e}");
    let e = Plan::parse(&text.replace("{ type = \"flat\" }", "{ type = \"menger\", level = 19 }"), &[]).unwrap_err().to_string();
    assert!(e.contains("2^30"), "{e}");
}

#[test]
fn parse_with_overrides() {
    let text = r#"
        [scene]
        generator = { type = "flat" }
        camera = { type = "static" }
        [run]
        pacing = { fps = 60 }
        streaming = { budget = 16 }
        [[renderer]]
        name = "cpu-reference"
    "#;
    let p = Plan::parse(text, &["run.frames=5".into(), "run.modes=[\"lit\"]".into(), "output.window=true".into()]).unwrap();
    assert_eq!(p.run.frames, 5);
    assert_eq!(p.run.modes, vec![PassMode::LIT]);
    assert_eq!(p.run.pacing, Pacing::Fps { fps: 60.0 });
    assert_eq!(p.run.streaming, Streaming::Budget { budget: 16 });
    assert!(p.output.window);
    let text = text.replace("[run]", "[output]\nsave = { every = 5 }\n[run]");
    assert!(Plan::parse(&text, &[]).unwrap().output.save.is_some());
    assert!(Plan::parse_with(&text, &[], &["output.save".into()]).unwrap().output.save.is_none());
    assert!(Plan::parse_with(&text, &[], &["output.nope".into(), "nope.x".into()]).is_ok(), "unsetting what is not there is fine");
}

#[test]
fn distinct_labels() {
    let text = r#"
        [scene]
        generator = { type = "flat" }
        camera = { type = "static" }
        [[renderer]]
        name = "cpu-reference"
        [[renderer]]
        name = "cpu-reference"
        [[renderer]]
        name = "cpu-reference"
        label = "cpu-reference #2"
        [[renderer]]
        name = "cpu-reference"
    "#;
    let mut p = Plan::parse(text, &[]).unwrap();
    p.distinct_labels();
    let labels: Vec<&str> = p.renderers.iter().map(|r| r.label()).collect();
    assert_eq!(labels, ["cpu-reference", "cpu-reference #3", "cpu-reference #2", "cpu-reference #4"]);
}

#[test]
fn shipped_plans_roundtrip() {
    for e in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/plans")).unwrap() {
        let path = e.unwrap().path();
        let plan = Plan::load(&path, &[]).unwrap();
        let text = plan.to_toml().unwrap();
        let again = Plan::parse(&text, &[]).unwrap_or_else(|e| panic!("{}: {e:#}\n{text}", path.display()));
        assert_eq!(text, again.to_toml().unwrap(), "{}", path.display());
    }
}

/// The components a node may be instead are knobs of the host's, asked of the graph;
/// setting one is the graph edit.
#[test]
fn component_knobs_are_the_graphs() {
    let text = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n\n[[renderer]]\ngraph = \"raycast-gpu\"\nedits = [{ node = \"storage\", component = \"prefix\" }, { node = \"kernel\", component = \"incremental\" }]\noptions = { exact = false }\n";
    let p = Plan::parse(text, &[]).unwrap();
    let r = &p.renderers[0];
    assert_eq!(region(r), Some(RegionId::GpuWalk));
    // The host's own list of what each node may be (the executors list no component).
    let knobs = r.component_knobs();
    let storage = knobs.iter().find(|k| k.name == "storage.component").expect("a storage choice");
    assert!(!storage.live && storage.default.as_str() == Some("prefix") && matches!(&storage.kind, mr_core::ParamKind::Choice(c) if c.iter().any(|x| x == "tree64") && c.len() > 4), "{:?}", storage.kind);
    assert!(knobs.iter().all(|k| component_knob(&k.name).is_some_and(|id| !matches!(id, "world" | "client" | "post"))), "{:?}", knobs.iter().map(|k| &k.name).collect::<Vec<_>>());
    assert!(crate::pipelines::executor_of(&r.pipeline().unwrap()).unwrap().1.iter().all(|k| knobs.iter().all(|c| c.name != k.name)), "no knob twice");
    assert!(r.options.get("storage").is_none() && r.knob_values().get("storage.component").and_then(|v| v.as_str()) == Some("prefix"));
    // Setting one is the graph edit; every listed choice is one the graph takes, and a
    // refused one changes nothing.
    for k in &knobs {
        let mr_core::ParamKind::Choice(cs) = &k.kind else { panic!("{k:?}") };
        for c in cs {
            let mut e = r.clone();
            e.set_component(component_knob(&k.name).unwrap(), c).unwrap_or_else(|err| panic!("{} = {c}: {err:#}", k.name));
            assert_eq!(e.knob_values().get(&k.name).and_then(|v| v.as_str()), Some(c.as_str()));
        }
    }
    // Asked on every publish of a lane's knobs: quick enough for that.
    let t = std::time::Instant::now();
    for f in mr_components::graph::SHIPPED {
        RendererSpec::from_cli(f.0).unwrap().component_knobs();
    }
    assert!(t.elapsed().as_millis() < 500, "{:?} for the ten shipped graphs", t.elapsed());
    let mut e = r.clone();
    assert!(e.set_component("storage", "heightmap").is_err() && e.set_component("nope", "bricks").is_err());
    assert_eq!((e.graph.as_ref(), &e.options, e.label()), (r.graph.as_ref(), &r.options, r.label()));
    assert_eq!(r.label(), "gpu: prefix + incremental + composite + raycast + direct");
    // Saved as it was read, options beside it.
    let saved = p.to_toml().unwrap();
    assert!(saved.contains("graph = \"raycast-gpu\"") && saved.contains("[[renderer.edits]]") && !saved.contains("compose"), "{saved}");
    let again = &Plan::parse(&saved, &[]).unwrap().renderers[0];
    assert_eq!((key(again), &again.options), (key(r), &r.options));
    // A component is no option: one named so stays an option no executor reads.
    let opt = Plan::parse(&text.replace("{ exact = false }", "{ exact = false, traversal = \"sphere\" }"), &[]).unwrap();
    assert_eq!((key(&opt.renderers[0]), opt.renderers[0].options.get("traversal").and_then(|v| v.as_str())), (key(r), Some("sphere")));
    // Another region's file; what a graph cannot be is refused with the reason.
    let raster = RendererSpec::from_cli("raster-gpu,geometry=cubes").unwrap();
    assert_eq!((region(&raster), raster.label()), (Some(RegionId::GpuMeshing), "gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct"));
    assert_eq!(region(&RendererSpec::from_cli("raycast-cpu,storage=palette").unwrap()), Some(RegionId::CpuRaycast));
    for (bad, why) in [("raycast-cpu,storage=rows", "renderer graph: "), ("rayquery-gpu,storage=prefix", "no node `storage`"), ("raycast-gpu,geometry=greedy", "no node `geometry`")] {
        let e = format!("{:#}", RendererSpec::from_cli(bad).unwrap_err());
        assert!(e.contains(why), "{bad}: {e}");
    }
    // A graph file is named as the graph, not as the renderer.
    let named = text.replace("graph = \"raycast-gpu\"", "name = \"discover-gpu\"").replace("edits = [{ node = \"storage\", component = \"prefix\" }, { node = \"kernel\", component = \"incremental\" }]\n", "").replace("options = { exact = false }", "");
    assert!(named.contains("name = \"discover-gpu\"") && !named.contains("edits") && !named.contains("exact"), "{named}");
    let e = format!("{:#}", Plan::parse(&named, &[]).unwrap_err());
    assert!(e.contains("write `graph = \"discover-gpu\"`"), "{e}");
    let r = &Plan::parse(&named.replace("name = \"discover-gpu\"", "graph = \"discover-gpu\""), &[]).unwrap().renderers[0];
    assert_eq!(r.label(), "gpu: bricks + refloor + quadtree + raster + direct");
}

/// The command line: a graph file, then edits in their short spellings or as tables.
#[test]
fn command_line() {
    let cli = |s: &str| RendererSpec::from_cli(s).map_err(|e| format!("{e:#}"));
    let plan = |s: &str| renderer(s).unwrap();
    // A bare file is that graph; a name that is no file is a standalone renderer's.
    for (name, _) in mr_components::graph::SHIPPED {
        assert_eq!(key(&cli(name).unwrap()), key(&plan(&format!("graph = \"{name}\""))), "{name}");
    }
    assert_eq!(key(&cli(concat!(env!("CARGO_MANIFEST_DIR"), "/pipelines/columns-cpu.toml")).unwrap()), key(&cli("columns-cpu").unwrap()));
    let r = cli("cpu-reference").unwrap();
    assert!(r.graph.is_none() && r.name == "cpu-reference");
    // Each short spelling is the plan's edit.
    for (short, edits) in [
        ("raycast-gpu,storage=tree64,kernel=incremental", r#"[{ node = "storage", component = "tree64" }, { node = "kernel", component = "incremental" }]"#),
        ("raycast-gpu, shading = pathtrace, shading.bounces = 2", r#"[{ node = "shading", component = "pathtrace" }, { node = "shading", knob = "bounces", value = 2 }]"#),
        ("raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage", r#"[{ node = "shading", component = "pathtrace" }, { node = "shading", input = "bounce", from = { role = "traversal", component = "sphere" } }, { node = "traversal", input = "storage", from = "storage" }]"#),
        ("raycast-gpu,-meshes", r#"[{ remove = "meshes" }]"#),
        ("raycast-gpu,+far=storage:tree64,+far.kernel=traversal:sphere,far.kernel.storage=far,shading.shadow=far.kernel", r#"[{ add = "far", role = "storage", component = "tree64" }, { add = "far.kernel", role = "traversal", component = "sphere" }, { node = "far.kernel", input = "storage", from = "far" }, { node = "shading", input = "shadow", from = "far.kernel" }]"#),
        ("discover-cpu,shadow.kernel=sphere,shadow.storage=distance", r#"[{ node = "shadow.kernel", component = "sphere" }, { node = "shadow.storage", component = "distance" }]"#),
        ("raycast-gpu,{node=\"kernel\",component=\"sphere\"},{ node = \"storage\", component = \"distance\" }", r#"[{ node = "kernel", component = "sphere" }, { node = "storage", component = "distance" }]"#),
        ("raycast-gpu,+client=client:derive,{node=\"client\",knobs={radius=4,max_lod=2}}", r#"[{ add = "client", role = "client", component = "derive" }, { node = "client", knobs = { radius = 4, max_lod = 2 } }]"#),
    ] {
        let (c, p) = (cli(short).unwrap_or_else(|e| panic!("{short}: {e}")), plan(&format!("graph = \"{}\"\nedits = {edits}", short.split(',').next().unwrap())));
        assert_eq!(c.graph, p.graph, "{short}");
        assert_eq!((c.label(), c.view_radius(&RunSpec::default())), (p.label(), p.view_radius(&RunSpec::default())), "{short}");
        // Written into a plan as its file and edits.
        assert_eq!(toml::to_string(&c).unwrap(), toml::to_string(&p).unwrap(), "{short}");
    }
    // A device, set and unset; an input unbound.
    use mr_components::graph::{Change, Device, shipped};
    let g = shipped("raycast-gpu").unwrap();
    assert_eq!(cli_edit(&g, "shading@gpu1").unwrap(), Change::Device("shading".into(), Some(Device::Gpu(1))));
    assert_eq!(cli_edit(&g, "shading @ default").unwrap(), Change::Device("shading".into(), None));
    assert_eq!(key(&cli("discover-gpu,visibility@cpu,visibility@default").unwrap()), key(&cli("discover-gpu").unwrap()));
    assert_eq!(key(&cli("raycast-gpu,visibility.meshes=,-meshes").unwrap()), key(&cli("raycast-gpu,-meshes").unwrap()));
    // No meshes stage: no mesh passes.
    let lit_meshes = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
    let bare = cli("raycast-gpu,-meshes").unwrap();
    let info = |s: &RendererSpec| s.info(&RunSpec::default(), &[]).unwrap();
    assert!(!bare.supports(&info(&bare), lit_meshes) && bare.supports(&info(&bare), PassMode::LIT));
    let walk = cli("raycast-gpu").unwrap();
    assert!(walk.supports(&info(&walk), lit_meshes) && info(&walk).features.left_out(Some(lit_meshes), 0, &[]).is_empty());
    // Forward raster-gpu draws meshes but its shadow rays miss them: said, not hidden.
    let forward = cli("raster-gpu").unwrap();
    assert!(forward.supports(&info(&forward), lit_meshes));
    assert_eq!(info(&forward).features.left_out(Some(lit_meshes), 0, &[]), vec!["mesh rays: draw `raster` (`draw`) lacks mesh rays: its shading does not meet the triangles".to_string()]);
    assert!(info(&forward).features.left_out(Some(PassMode { lighting: mr_core::RenderMode::Unlit, meshes: true }), 0, &[]).is_empty());
    let visbuffer = cli("raster-gpu,draw=visbuffer").unwrap();
    assert!(info(&visbuffer).features.has(mr_core::Feature::MeshRays));
    // Knobs asked of a graph that leaves them out: said, not dropped silently.
    let left_out = |s: &str, knobs: &[(&str, toml::Value)]| {
        let mut r = cli(s).unwrap();
        r.options.extend(knobs.iter().map(|(k, v)| (k.to_string(), v.clone())));
        r.not_supported(&RunSpec::default(), Some(PassMode::LIT), &[], &info(&r))
    };
    assert_eq!(left_out("raycast-gpu", &[("bounces", 3.into())]), vec!["knob `bounces`: no shading node of `pathtrace` (its shading: `direct`)".to_string()]);
    assert!(left_out("raycast-gpu,shading=pathtrace", &[("bounces", 3.into()), ("view", "steps".into())]).is_empty());
    assert_eq!(left_out("rayquery-gpu", &[("view", "steps".into())]), vec!["knob `view`: this pipeline has no such knob".to_string()]);
    assert_eq!(left_out("discover-cpu", &[("view", "kinds".into())]), vec!["`view = kinds`: visibility `quadtree` (`visibility`) lacks clusters".to_string()]);
    assert!(left_out("discover-cpu,visibility=cluster", &[("view", "kinds".into()), ("seed_step", 8.into())]).is_empty());
    assert_eq!(left_out("raycast-gpu", &[("nonsense", 1.into())]), vec!["knob `nonsense`: this pipeline has no such knob".to_string()]);
    // Refused with the reason.
    for (s, why) in [
        ("raycast-gpu,nowhere=tree64", "no node `nowhere` (the graph has "),
        ("raycast-gpu,storage=nope", "unknown storage component `nope`"),
        ("raycast-gpu,shading.shadow=nowhere", "no node `nowhere` to read"),
        ("raycast-gpu,shading.shadow=moon:sphere", "edit `shading.shadow=moon:sphere`"),
        ("raycast-gpu,storage", "`node=component`, `node.input=source`"),
        ("raycast-gpu,storage@moon", "unknown device `moon`"),
        ("raycast-gpu,+far=tree64", "`+id=role:component`"),
        ("raycast-gpu,-nowhere", "no node `nowhere`"),
        ("raycast-gpu,{node=\"storage\"}", "what changes on the node"),
        ("no-such-graph,storage=tree64", "neither a shipped graph"),
        ("raycast-gpu:prefix+incremental,storage=tree64", "neither a shipped graph"),
        ("raycast-gpu:prefix+incremental", "a renderer is written `<graph file>[,<edit>,...]`"),
        ("raycast-cpu,storage=rows", "renderer graph: "),
    ] {
        let e = cli(s).err().unwrap_or_else(|| panic!("{s}: accepted"));
        assert!(e.contains(why), "{s}: {e}");
    }
}

/// A renderer written as an inline graph is the pipeline a file and edits give; its
/// stages' knobs stay on its nodes.
#[test]
fn graph_renderers() {
    let base = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n";
    let graph = r#"
[[renderer]]
[renderer.graph]
executor = "gpu"
nodes.s = { role = "storage", component = "prefix" }
nodes.k = { role = "traversal", component = "incremental", inputs = { storage = "s" }, knobs = { exact = false } }
nodes.v = { role = "visibility", component = "raycast", inputs = { primary = "k" } }
nodes.out = { role = "shading", component = "direct", inputs = { hits = "v", shadow = "k" } }
"#;
    let g = Plan::parse(&format!("{base}{graph}"), &[]).unwrap();
    let mut c = RendererSpec::from_cli("raycast-gpu,storage=prefix,kernel=incremental,visibility.meshes=,-meshes").unwrap();
    c.options.insert("exact".into(), false.into());
    c.resolve().unwrap();
    let (g, c) = (&g.renderers[0], &c);
    assert_eq!(g.name, c.name);
    assert_eq!((g.label(), c.label()), ("gpu: prefix + incremental(exact=false) + raycast + direct", "gpu: prefix + incremental + raycast + direct"), "a node's knob is in the label, an option is not");
    assert!(g.options.is_empty());
    // It runs as it is written: its nodes keep their ids.
    assert_eq!(g.graph.as_ref().unwrap().nodes.keys().map(String::as_str).collect::<Vec<_>>(), ["k", "out", "s", "v"]);
    assert_eq!(g.worker_graph().unwrap().unwrap().key(), c.worker_graph().unwrap().unwrap().key(), "the same pipeline to the worker");
    assert_eq!(g.knob_values().get("exact"), c.knob_values().get("exact"));
    let bare = Plan::parse(&format!("{base}{}", graph.replace(", knobs = { exact = false }", "")), &[]).unwrap();
    assert_eq!(key(&bare.renderers[0]), key(c));
    let bad = graph.replace("primary = \"k\"", "primary = \"s\"");
    let e = Plan::parse(&format!("{base}{bad}"), &[]).unwrap_err().to_string();
    assert!(e.contains("takes ray queries"), "{e}");
    // Shadow rays through a kernel of their own (its knob stays with it) run as
    // the graph, labelled by its nodes.
    let own = graph.replace("shadow = \"k\" } }", "shadow = \"k2\" } }\nnodes.k2 = { role = \"traversal\", component = \"incremental\", inputs = { storage = \"s\" }, knobs = { exact = true, skip8 = false } }");
    let p = Plan::parse(&format!("{base}{own}"), &[]).unwrap();
    let r = &p.renderers[0];
    assert_eq!(r.label(), "gpu: prefix + incremental(exact=false) + raycast + incremental(skip8=false) + direct", "a knob at its default says nothing");
    assert_eq!(r.knob_values().get("exact"), Some(&toml::Value::Boolean(false)), "the primary kernel's knob");
    assert!(r.options.is_empty());
    assert!(r.pipeline().unwrap().nodes.contains_key("k2"));
    // Saved as the graph it is (its knobs in its nodes), and loads back the same.
    let saved = p.to_toml().unwrap();
    assert!(saved.contains("[renderer.graph]") && !saved.contains("compose"), "{saved}");
    let again = Plan::parse(&saved, &[]).unwrap();
    assert_eq!(again.renderers[0].pipeline().unwrap().key(), r.pipeline().unwrap().key());
    assert_eq!(again.renderers[0].label(), r.label());
    // Without knobs on its kernels the same graph is saved as the graph too (no other form).
    let plain = own.replace(", knobs = { exact = true, skip8 = false }", "").replace(", knobs = { exact = false }", "");
    let p = Plan::parse(&format!("{base}{plain}"), &[]).unwrap();
    let r = &p.renderers[0];
    assert_eq!(r.label(), "gpu: prefix + incremental + raycast + incremental + direct");
    let saved = p.to_toml().unwrap();
    assert!(saved.contains("[renderer.graph]") && !saved.contains("sites") && !saved.contains("compose"), "{saved}");
    assert_eq!(key(&Plan::parse(&saved, &[]).unwrap().renderers[0]), key(r));
    let both = graph.replace("[[renderer]]\n", "[[renderer]]\nname = \"cpu-reference\"\n");
    assert!(Plan::parse(&format!("{base}{both}"), &[]).unwrap_err().to_string().contains("not both"));
}

/// What a pass can tell apart, read off the graph: unlit, the shading's component and
/// the rays it casts are moot; without meshes, the meshes node and its shadow rays.
#[test]
fn passes_tell_apart() {
    let modes = [PassMode { lighting: mr_core::RenderMode::Unlit, meshes: false }, PassMode { lighting: mr_core::RenderMode::Unlit, meshes: true }, PassMode { lighting: mr_core::RenderMode::Lit, meshes: false }, PassMode { lighting: mr_core::RenderMode::Lit, meshes: true }];
    let [unlit, unlit_meshes, lit, lit_meshes] = modes;
    let cli = |s: &str| RendererSpec::from_cli(s).unwrap_or_else(|e| panic!("{s}: {e:#}"));
    let same = |a: &str, b: &str, m: PassMode| cli(a).told_apart(m) == cli(b).told_apart(m);
    // Path tracing with bounce rays of its own against the plain pipeline.
    let (plain, pt) = ("raycast-gpu", "raycast-gpu,shading=pathtrace,shading.bounce=traversal:sphere,traversal.storage=storage:tree64");
    assert!(cli(pt).pipeline().unwrap().nodes.values().any(|n| n.component == "tree64"));
    assert!(same(plain, pt, unlit) && same(plain, pt, unlit_meshes) && !same(plain, pt, lit) && !same(plain, pt, lit_meshes));
    // What primary rays walk is told apart in every pass.
    assert!(modes.iter().all(|&m| !same(plain, "raycast-gpu,storage=tree64", m)));
    // The rasteriser's storage serves shadow rays alone (the shading's and the mesh pass's).
    let (raster, other) = ("raster-gpu", "raster-gpu,storage=tree64");
    assert!(same(raster, other, unlit) && same(raster, other, unlit_meshes) && !same(raster, other, lit) && !same(raster, other, lit_meshes));
    // The meshes node and what it alone reads: told apart only by passes with meshes.
    let no_meshes = "raster-gpu,draw.meshes=,-meshes";
    assert!(cli(no_meshes).pipeline().unwrap().nodes.values().all(|n| n.role != mr_components::graph::Role::Meshes));
    assert!(same(raster, no_meshes, unlit) && same(raster, no_meshes, lit) && !same(raster, no_meshes, unlit_meshes) && !same(raster, no_meshes, lit_meshes));
    // An option is told apart always; a renderer from itself never.
    let mut o = cli(plain);
    o.options.insert("exact".into(), false.into());
    assert!(modes.iter().all(|&m| o.told_apart(m) != cli(plain).told_apart(m) && cli(pt).told_apart(m) == cli(pt).told_apart(m)));
    // Nothing is left out of a lit pass with meshes: the graph as it is.
    let g = cli(pt).graph.unwrap();
    assert_eq!(g.told_apart(lit_meshes), g);
}

/// The scoring class takes the lighting model: path tracing in lit passes, with its
/// bounces; direct otherwise (unlit, no bounces, other shading).
#[test]
fn lighting_class() {
    use mr_core::PassMode;
    let pt = RendererSpec::from_cli("raycast-gpu,shading=pathtrace").unwrap();
    assert_eq!(pt.lighting(PassMode::LIT), Lighting::Paths { bounces: 1 });
    assert_eq!(pt.lighting(PassMode::UNLIT), Lighting::Direct);
    let mut more = pt.clone();
    more.options.insert("bounces".into(), 3.into());
    assert_eq!(more.lighting(PassMode::LIT), Lighting::Paths { bounces: 3 });
    more.options.insert("bounces".into(), 0.into());
    assert_eq!(more.lighting(PassMode::LIT), Lighting::Direct);
    assert_eq!(RendererSpec::from_cli("raycast-gpu").unwrap().lighting(PassMode::LIT), Lighting::Direct);
    assert_eq!(RendererSpec::new("cpu-reference").lighting(PassMode::LIT), Lighting::Direct);
    let ao = RendererSpec::from_cli("raycast-gpu,shading=ao").unwrap();
    assert_eq!(ao.lighting(PassMode::LIT), Lighting::Occlusion { reach: 4 });
    assert_eq!(ao.lighting(PassMode::UNLIT), Lighting::Direct);
    let near = RendererSpec::from_cli("raycast-gpu,shading=ao,shading.reach=2").unwrap();
    assert_eq!(near.lighting(PassMode::LIT), Lighting::Occlusion { reach: 2 });
    // Late edges are read off the graph: discover-cpu's faces, unless discovered in step.
    assert!(pt.late().is_empty() && RendererSpec::new("cpu-reference").late().is_empty());
    let late = RendererSpec::from_cli("discover-cpu").unwrap();
    assert_eq!(late.late().len(), 1, "{:?}", late.late());
    assert!(late.late()[0].ends_with(".faces"), "{:?}", late.late());
    let mut sync = late.clone();
    sync.options.insert("pipelined".into(), false.into());
    assert!(sync.late().is_empty(), "{:?}", sync.late());
    let run = RunSpec::default();
    let region = pt.region(&run, 0);
    assert_eq!(Like::of(&region, Lighting::Direct, Leaves::default(), &run), None);
    let like = Like::of(&region, pt.lighting(PassMode::LIT), Leaves::default(), &run).unwrap();
    assert_eq!(like.to_string(), format!("pick(radius={}), pathtrace(bounces=1)", run.view_radius));
    // What a pipeline declares it leaves out, its reference leaves out too.
    use mr_core::Feature;
    let lit_meshes = PassMode { lighting: mr_core::RenderMode::Lit, meshes: true };
    let mut forward = mr_core::Features::of(&[Feature::Lit, Feature::Meshes, Feature::MeshRays, Feature::Volume]);
    forward.lack(Feature::MeshRays, "draw `raster` shades forward");
    let leaves = Leaves::of(&forward, Some(lit_meshes), &[Feature::Transparency]);
    assert_eq!(leaves, Leaves { mesh_rays: true, transparency: true, volume: false });
    assert_eq!(Leaves::of(&forward, Some(PassMode::UNLIT), &[]), Leaves::default(), "no shading rays unlit");
    assert_eq!(Leaves::of(&forward, None, &[]), Leaves::default());
    let columns = mr_core::Features::of(&[Feature::Lit]).lacking(Feature::Volume, "2.5D");
    assert_eq!(Leaves::of(&columns, Some(PassMode::UNLIT), &[]), Leaves { volume: true, ..Default::default() });
    let like = Like::of(&region, Lighting::Direct, leaves, &run).unwrap();
    assert_eq!(like.to_string(), format!("pick(radius={}), no mesh rays, no transparency", run.view_radius));
    // Accumulating pipelines are held still until `converge` paths per pixel; late ones
    // for `settle` renders; the larger where both.
    let c = CompareSpec { reference: "cpu-reference".into(), reference_options: Default::default(), every: 10, settle: 2, converge: 64 };
    assert_eq!((pt.accumulates(PassMode::LIT), pt.accumulates(PassMode::UNLIT)), (Some(1), None));
    assert_eq!((c.held(&pt, PassMode::LIT), c.held(&pt, PassMode::UNLIT)), (63, 0));
    let mut four = pt.clone();
    four.options.insert("spp".into(), 4.into());
    assert_eq!(c.held(&four, PassMode::LIT), 15);
    four.options.insert("accumulate".into(), false.into());
    assert_eq!((four.accumulates(PassMode::LIT), c.held(&four, PassMode::LIT)), (None, 0));
    assert_eq!((c.held(&late, PassMode::UNLIT), c.held(&RendererSpec::from_cli("raycast-gpu").unwrap(), PassMode::LIT)), (2, 0));
}

/// Sweeps as edits (`vary`): every combination that is a pipeline, each once, with
/// options only where they apply; and what is refused.
#[test]
fn varied_sweeps() {
    use std::collections::BTreeSet;
    let base = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n";
    let plan = |r: &str| Plan::parse(&format!("{base}[[renderer]]\n{r}\n"), &[]);
    // What a renderer is to a run: its pipeline, its region, its link, where its world is made.
    let set = |p: &Plan| p.renderers.iter().map(|r| (key(r), r.label().to_string(), r.view_radius(&p.run), r.max_lod(&p.run, 8), format!("{:?}", r.world_gates()), r.generates())).collect::<BTreeSet<_>>();
    let each_once = |p: &Plan, n: usize, what: &str| {
        assert_eq!((p.renderers.len(), set(p).len()), (n, n), "{what}");
        assert!(p.renderers.iter().all(|r| r.swept && r.pipeline().unwrap().accept().is_ok()), "{what}");
    };
    // Lists and `*` (every component the graph takes there); options as before.
    let new = plan("graph = \"raycast-gpu\"\nlabel = \"gone\"\nvary = [{ node = \"storage\", component = \"*\" }, { node = \"kernel\", component = [\"refloor\", \"sphere\"] }]\noptions = { exact = false, merge = \"rows\" }").unwrap();
    // How many components `file`'s `node` takes (each alone a pipeline): what a `*` there gives.
    let takes = |file: &str, node: &str| {
        let g = mr_components::graph::shipped(file).unwrap();
        g.choices(node).into_iter().filter(|c| g.edit(&mr_components::graph::Change::component(node, c)).and_then(|h| h.accept()).is_ok()).count()
    };
    let storages = takes("raycast-gpu", "storage");
    each_once(&new, storages * 2, "storages x two kernels");
    assert!(new.renderers.iter().all(|r| region(r) == Some(RegionId::GpuWalk) && r.label != Some("gone".into()) && r.options.get("exact").is_some() && r.options.get("merge").is_none()));
    // Each is written as its file and edits (the entry's own first), and reads back.
    let r = new.renderers.iter().find(|r| r.label().contains("tree64") && r.label().contains("sphere")).unwrap();
    let w = toml::to_string(r).unwrap();
    assert!(w.starts_with("graph = \"raycast-gpu\"\n") && w.contains("[[edits]]") && !w.contains("vary"), "{w}");
    assert_eq!(key(&renderer(&w).unwrap()), key(r));
    // The full sweep the scripts run, per region.
    let full = include_str!("../../scripts/full-sweep.toml").split_once("renderer = ").unwrap().1;
    let all = Plan::parse(base, &[format!("renderer={full}")]).unwrap();
    each_once(&all, 3512, "the full sweep");
    let n = |b: RegionId| all.renderers.iter().filter(|r| region(r) == Some(b)).count();
    // raycast-gpu: storages x 2 (prepass or not) x kernels x shadings x 2 (meshes or not); the
    // rasteriser's shadow rays: storages x kernels x geometries x draws x shadings x 2 (meshes);
    // ray query: geometries x shadings, then the bounce walk's storages x kernels.
    let (walk, raster, rq) = (|n: &str| takes("raycast-gpu", n), |n: &str| takes("raster-gpu", n), |n: &str| takes("rayquery-gpu", n));
    assert_eq!(
        (n(RegionId::GpuWalk), n(RegionId::GpuRayQuery), n(RegionId::GpuMeshing), n(RegionId::CpuColumns)),
        (
            walk("storage") * 2 * walk("kernel") * walk("shading") * 2,
            rq("geometry") * rq("shading") + walk("storage") * walk("kernel"),
            raster("storage") * raster("kernel") * raster("geometry") * raster("draw") * raster("shading") * 2,
            // The column march's shadings, and raycast-cpu's sweep's heightmap under the
            // heightfield march (a ray caster over a march is the column march region's).
            takes("columns-cpu", "shading") + 1,
        )
    );
    assert_eq!(mr_components::graph::REGIONS.iter().map(|b| n(b.id)).sum::<usize>(), all.renderers.len(), "every one a region's");
    assert!(mr_components::graph::REGIONS.iter().all(|b| n(b.id) > 0), "every region swept");
    // `plans/stream.toml`: four networks (none and three gates), the generator's two places,
    // three clients.
    let stream = Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/stream.toml")), &[]).unwrap();
    each_once(&stream, 4 * 2 * 3, "stream");
    assert_eq!(stream.renderers.iter().map(|r| (r.view_radius(&stream.run), r.max_lod(&stream.run, 8), r.levels() == mr_core::client::Levels::Derive)).collect::<BTreeSet<_>>().len(), 3, "three client regions");
    assert_eq!((stream.renderers.iter().filter(|r| r.world_gates().is_empty()).count(), stream.renderers.iter().filter(|r| r.generates()).count()), (6, 12));
    // The post's tone mappers, each its own key (a CPU pipeline takes all four too).
    for file in ["raycast-gpu", "raycast-cpu"] {
        let p = plan(&format!("graph = \"{file}\"\nedits = [{{ add = \"post\", role = \"post\", component = \"clamp\" }}, {{ node = \"post\", input = \"image\", from = \"shading\" }}]\nvary = [{{ node = \"post\", component = \"*\" }}]")).unwrap();
        each_once(&p, 4, file);
        assert_eq!(p.renderers.iter().map(|r| r.pipeline().unwrap().tone().name()).collect::<Vec<_>>(), ["clamp", "reinhard", "aces", "hdr"], "{file}");
    }
    // A knob's values; the client's region is no part of the key but a pass of its own.
    let p = plan("graph = \"raycast-gpu\"\nedits = [{ add = \"client\", role = \"client\", component = \"pick\" }]\nvary = [{ node = \"client\", knob = \"radius\", value = [3, 6] }]").unwrap();
    assert_eq!(p.renderers.iter().map(|r| r.view_radius(&p.run)).collect::<Vec<_>>(), [3, 6]);
    assert_eq!(key(&p.renderers[0]), key(&p.renderers[1]));
    // What the bounce rays may read: asked of the graph, kept where `accept` takes it.
    let p = plan("graph = \"raycast-gpu\"\nedits = [{ node = \"shading\", component = \"pathtrace\" }]\nvary = [{ node = \"shading\", input = \"bounce\", from = \"*\" }]").unwrap();
    assert!(!p.renderers.is_empty() && p.renderers.iter().all(|r| r.pipeline().unwrap().accept().is_ok()));
    // Refused with the reason.
    for (r, why) in [
        ("vary = [{ node = \"storage\", component = \"*\" }]", "this renderer has none"),
        ("graph = \"raycast-gpu\"\nvary = [{ node = \"nowhere\", component = \"*\" }]", "no node `nowhere` to vary"),
        ("graph = \"raycast-gpu\"\nvary = [{ node = \"nowhere\", component = [\"tree64\"] }]", "gives no pipeline (the first combination: edit 1"),
        ("graph = \"raycast-gpu\"\nvary = [{ node = \"storage\", component = [\"heightmap\"] }]", "gives no pipeline (the first combination: "),
        ("graph = \"raycast-gpu\"\nvary = [{ node = \"shading\", knob = \"bounces\", value = \"*\" }]", "give the values as a list"),
        ("graph = \"raycast-gpu\"\nvary = [{ any = [[]], node = \"storage\" }]", "`any` stands alone"),
    ] {
        let e = format!("{:#}", plan(r).unwrap_err());
        assert!(e.contains(why), "{r}: {e}");
    }
}

/// The covering selection itself: of combinations some of which are no pipeline, the chosen
/// hold every value and every pair the valid ones hold, are valid, fewer, and the same each time.
#[test]
fn pairs_cover_what_the_valid_hold() {
    use std::collections::HashSet;
    use super::sweep::{Held, cover, held};
    let lens = [4, 3, 3, 2, 2];
    let total: usize = lens.iter().product();
    // No pipeline: entry 0's value 1 with entry 1's value 2, and every seventh combination.
    let valid: Vec<usize> = (0..total).filter(|&n| !(held(n, &lens).contains(&(0, 1, 1, 2)) || n % 7 == 0)).collect();
    let candidates: Vec<HashSet<Held>> = valid.iter().map(|&n| held(n, &lens).into_iter().collect()).collect();
    let chosen = cover(&candidates, None);
    let want: HashSet<Held> = candidates.iter().flatten().copied().collect();
    let got: HashSet<Held> = chosen.iter().flat_map(|&i| candidates[i].iter().copied()).collect();
    assert_eq!(got, want, "every value and every pair the valid combinations hold");
    assert!(!want.contains(&(0, 1, 1, 2)), "a pair no pipeline holds is not asked for");
    assert!(want.contains(&(0, 1, 0, 1)) && want.contains(&(1, 2, 1, 2)), "its values alone are");
    assert!(chosen.windows(2).all(|w| w[0] < w[1]), "in order, each once");
    assert!(chosen.len() < valid.len() / 4, "{} of {}", chosen.len(), valid.len());
    assert_eq!(cover(&candidates, None), chosen, "deterministic");
    // One entry: each value once; none: nothing to hold, one pipeline.
    let one: Vec<HashSet<Held>> = (0..5).map(|n| held(n, &[5]).into_iter().collect()).collect();
    assert_eq!(cover(&one, None), [0, 1, 2, 3, 4]);
    assert_eq!(held(0, &[]), []);
    // In parts: each with every value, together with every pair.
    let values: HashSet<Held> = want.iter().copied().filter(|h| h.0 == h.2).collect();
    let mut union = HashSet::new();
    for k in 0..6 {
        let part = cover(&candidates, Some((k, 6)));
        let got: HashSet<Held> = part.iter().flat_map(|&i| candidates[i].iter().copied()).collect();
        assert!(values.is_subset(&got), "part {k}: every value");
        assert!(part.windows(2).all(|w| w[0] < w[1]) && part.len() < chosen.len(), "part {k}: {} of {}", part.len(), chosen.len());
        assert_eq!(cover(&candidates, Some((k, 6))), part, "deterministic");
        union.extend(got);
    }
    assert_eq!(union, want, "the parts together: every pair");
}

/// `cover = "pairs"` over the full sweep, entry by entry: a few of the full product's
/// renderers, written alike and in its order, all pipelines, the same each time, and
/// together with every node's every component (or absence) and every two of them that the
/// full product has together.
#[test]
fn pair_covering_sweeps() {
    use std::collections::BTreeSet;
    let base = "[scene]\ngenerator = { type = \"flat\" }\ncamera = { type = \"static\" }\n";
    let sweep: toml::Table = toml::from_str(include_str!("../../scripts/full-sweep.toml")).unwrap();
    let written = |p: &Plan| p.renderers.iter().map(|r| toml::to_string(r).unwrap()).collect::<Vec<_>>();
    let (mut full_n, mut pairs_n, mut parts_n) = (0, 0, [0; 15]);
    for e in sweep["renderer"].as_array().unwrap() {
        let one = format!("renderer=[{e}]");
        let full = Plan::parse(base, std::slice::from_ref(&one)).unwrap();
        let pairs = Plan::parse(base, &[one.clone(), "sweep.cover=\"pairs\"".into()]).unwrap();
        assert_eq!(written(&Plan::parse(base, &[one.clone(), "sweep.cover=\"all\"".into()]).unwrap()), written(&full), "{e}: `all` is the default");
        assert_eq!(written(&Plan::parse(base, &[one.clone(), "sweep.cover=\"pairs\"".into()]).unwrap()), written(&pairs), "{e}: deterministic");
        // A subsequence of the full product.
        let (w, mut rest) = (written(&full), written(&pairs).into_iter().peekable());
        for x in &w {
            if rest.peek() == Some(x) {
                rest.next();
            }
        }
        assert!(rest.next().is_none(), "{e}: the pairs are some of the full product, in its order");
        assert!(pairs.renderers.iter().all(|r| r.swept && r.pipeline().unwrap().accept().is_ok()), "{e}");
        // What each graph has: every node's component, or that it lacks a node others have.
        let ids: BTreeSet<String> = full.renderers.iter().flat_map(|r| r.pipeline().unwrap().nodes.into_keys()).collect();
        let has = |r: &RendererSpec| {
            let g = r.pipeline().unwrap();
            ids.iter().map(|id| format!("{id}={}", g.nodes.get(id).map_or("none", |n| n.component.as_str()))).collect::<Vec<_>>()
        };
        let together = |p: &Plan| {
            p.renderers.iter().flat_map(|r| {
                let f = has(r);
                (0..f.len()).flat_map(move |i| (i..f.len()).map({
                    let f = f.clone();
                    move |j| (f[i].clone(), f[j].clone())
                })).collect::<Vec<_>>()
            }).collect::<BTreeSet<_>>()
        };
        assert_eq!(together(&pairs), together(&full), "{e}: every value and pair of values the full product has");
        // Spread over 15 runs (one per world): each with every value, together every pair.
        let values: BTreeSet<_> = together(&full).into_iter().filter(|(a, b)| a == b).collect();
        let mut union = BTreeSet::new();
        for (k, n) in parts_n.iter_mut().enumerate() {
            let part = Plan::parse(base, &[one.clone(), "sweep.cover=\"pairs\"".into(), format!("sweep.part={k}"), "sweep.parts=15".into()]).unwrap();
            let t = together(&part);
            assert!(values.is_subset(&t), "{e}: part {k} has every value");
            assert!(part.renderers.iter().all(|r| r.pipeline().unwrap().accept().is_ok()), "{e}");
            *n += part.renderers.len();
            union.extend(t);
        }
        assert_eq!(union, together(&full), "{e}: the parts together have every pair");
        (full_n, pairs_n) = (full_n + full.renderers.len(), pairs_n + pairs.renderers.len());
    }
    assert_eq!(full_n, 3512, "the full sweep");
    eprintln!("the full sweep: {full_n} renderers, pairs: {pairs_n}, in 15 parts: {parts_n:?}");
    assert!(pairs_n * 4 < full_n, "{pairs_n} of {full_n}");
    assert!(parts_n.iter().all(|&n| n * 2 < pairs_n), "{parts_n:?} of {pairs_n}");
    // Per entry, over the default; refused with the reason.
    let one = |r: &str, more: &str| Plan::parse(&format!("{base}{more}[[renderer]]\n{r}\n"), &[]);
    let vary = "graph = \"raycast-gpu\"\nvary = [{ node = \"storage\", component = \"*\" }, { node = \"kernel\", component = \"*\" }, { node = \"shading\", component = \"*\" }]";
    let n = |r: &str, more: &str| one(r, more).unwrap().renderers.len();
    assert!(n(&format!("{vary}\ncover = \"pairs\""), "") < n(vary, ""));
    assert_eq!(n(&format!("{vary}\ncover = \"all\""), "[sweep]\ncover = \"pairs\"\n"), n(vary, ""));
    // `except`: `*` but those.
    let every = |except: &str| n(&format!("graph = \"raycast-gpu\"\nvary = [{{ node = \"storage\", component = \"*\"{except} }}]"), "");
    assert_eq!(every(", except = [\"bricks\", \"hier\"]") + 2, every(""));
    for (r, more, why) in [
        (format!("{vary}\ncover = \"some\""), "", "\"all\" (every pipeline"),
        ("graph = \"raycast-gpu\"\ncover = \"pairs\"".to_string(), "", "this renderer has no `vary`"),
        (vary.to_string(), "[sweep]\nlimit = 3\n", "`sweep.limit`"),
        (vary.to_string(), "[sweep]\ncover = \"pairs\"\npart = 15\nparts = 15\n", "the part counted from 0"),
        (vary.to_string(), "[sweep]\ncover = \"pairs\"\npart = 1\n", "both"),
        ("graph = \"raycast-gpu\"\nvary = [{ node = \"storage\", component = \"*\", except = [\"nope\"] }]".to_string(), "", "which `\"*\"` does not offer"),
        ("graph = \"raycast-gpu\"\nvary = [{ node = \"storage\", component = [\"bricks\", \"hier\"], except = [\"bricks\"] }]".to_string(), "", "not a list"),
    ] {
        let e = format!("{:#}", one(&r, more).unwrap_err());
        assert!(e.contains(why), "{r}: {e}");
    }
}

/// Every shipped plan parses, and its compositions validate and resolve.
#[test]
fn shipped_plans_load() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans"));
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        if path.extension().is_some_and(|x| x == "toml") {
            let p = Plan::load(&path, &[]).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
            assert!(p.renderers.iter().all(|r| r.info(&p.run, &mr_worker::infos()).is_some()));
            // No knob a plan sets is unknown to its pipeline or left out by its graph.
            for r in &p.renderers {
                let info = r.info(&p.run, &mr_worker::infos()).unwrap();
                let knobs: Vec<String> = r.not_supported(&p.run, None, &[], &info).into_iter().filter(|w| w.starts_with("knob ") || w.starts_with('`')).collect();
                assert!(knobs.is_empty(), "{}: {}: {knobs:?}", path.display(), r.label());
            }
            n += 1;
        }
    }
    assert!(n >= 10);
}

/// A knob set on a node is one its executor declares for that node's role and component:
/// an unknown name, a knob of another node and a knob of the pipeline's are refused, each
/// naming node and knob; the world and generator have none.
#[test]
fn unowned_knobs_are_refused() {
    let with = |edit: &str| renderer(&format!("graph = \"raster-cpu\"\nedits = [{edit}]")).and_then(|mut r| r.resolve().map(|_| r));
    with(r#"{ node = "draw", knobs = { tile_width = 32 } }"#).unwrap();
    let e = with(r#"{ node = "draw", knobs = { bogus = 1 } }"#).unwrap_err().to_string();
    assert!(e.contains("`draw` (draw `raster`): no knob `bogus`") && e.contains("`tile_width`"), "{e}");
    let e = with(r#"{ node = "storage", knobs = { tile_width = 32 } }"#).unwrap_err().to_string();
    assert!(e.contains("`storage` (storage `bricks`): knob `tile_width` belongs to a draw node of `raster`"), "{e}");
    let e = with(r#"{ node = "world", knobs = { radius = 3 } }"#).unwrap_err().to_string();
    assert!(e.contains("`world`: the world has no knobs (not `radius`)"), "{e}");
}
