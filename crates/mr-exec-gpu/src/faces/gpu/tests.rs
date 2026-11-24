//! The discovery's programs compose and validate (no GPU needed).

use mr_components::shaders::Walks;
use mr_components::visibility::quadtree;

use super::*;

/// The sampler (without and with hole detection's tiles), hole detection, the raster and
/// every drawer compose from their modules and validate for every storage, kernel and
/// shading (no GPU needed), as `GpuSampler::new` and `Core` assemble them.
#[test]
fn programs_compose() {
    use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
    let check = |p: &mr_gpu::program::Program, root: &str, deps: &[String], what: &str| {
        let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
        let m = p.naga(root, &deps).unwrap_or_else(|e| panic!("{what}: {e}"));
        Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    };
    use mr_components::graph::{Change, Node, shipped};
    let base = shipped("discover-gpu").unwrap();
    let (storages, kernels) = (base.choices("storage"), base.choices("kernel"));
    assert!(storages.len() > 1 && kernels.len() > 1, "{storages:?} {kernels:?}");
    for st in storages {
        for &k in &kernels {
            let mut graphs = vec![base.with_edits(&[Change::component("storage", st), Change::component("kernel", k), Change::component("shading", "pathtrace"), Change::bind("shading", "bounce", "kernel")]).unwrap()];
            // Exposure, shadow and bounce rays through storages and kernels of their own.
            if k == "refloor" {
                let mut g = graphs[0].clone();
                let node = |role, c: &str, ins: &[(&str, &str)]| {
                    let mut n = Node::new(role, c);
                    n.inputs.extend(ins.iter().map(|&(p, s)| (p.to_string(), s.to_string())));
                    n
                };
                g.nodes.insert("x".into(), node(Role::Storage, "bricks", &[]));
                g.nodes.insert("s.storage".into(), node(Role::Storage, "rows", &[]));
                g.nodes.insert("s.kernel".into(), node(Role::Traversal, "rowrun", &[("storage", "s.storage")]));
                g.nodes.insert("b.kernel".into(), node(Role::Traversal, "sphere", &[("storage", "storage")]));
                g.nodes.get_mut("visibility").unwrap().inputs.insert("exposure".into(), "x".into());
                g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "s.kernel".into());
                g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "b.kernel".into());
                graphs.push(g);
            }
            for g in graphs {
                g.validate().unwrap_or_else(|e| panic!("{g:?}: {e}"));
                let ck = g.check().unwrap();
                let mut walks = Walks::of(&ck);
                let rays = walks.site(&ck, ck.site(Role::Visibility, "rays").unwrap(), STORAGE_GROUPS, false);
                let exposure = walks.storage(&ck, ck.input(Role::Visibility, "exposure").unwrap(), STORAGE_GROUPS);
                let shadow = walks.site(&ck, ck.site(Role::Shading, "shadow").unwrap(), STORAGE_GROUPS, false);
                // Direct shading, and pathtrace with its bounces.
                let mut with_bounce = walks.clone();
                let bounce = with_bounce.site(&ck, ck.site(Role::Shading, "bounce").unwrap(), STORAGE_GROUPS, false);
                for (walks, bounce, shading) in [(walks, shadow, "direct"), (with_bounce, bounce, "pathtrace")] {
                    let (mut program, deps, _) = crate::faces::programs(&walks, Some((rays, exposure)), None, [shadow, bounce], None, shading);
                    let draw_src = crate::faces::draw_src(include_str!("../set_faces.wgsl"), true);
                    crate::faces::tests::drawers(&program, &deps, None, &draw_src, shading, &format!("discover-gpu {st} {k} {shading}"), &base.choices("draw"), (&walks, Some((rays, exposure)), None));
                    // The overdraw view's counting raster.
                    let mut counting = program.clone();
                    counting.define("VIEW_OVERDRAW").define("EARLY_DEPTH");
                    check(&counting, &format!("{draw_src}\n{}", crate::view::atomic_wgsl()), &deps, &format!("overdraw {st} {k} {shading}"));
                    // Discovery's pieces: the sampler and hole detection over the shared set.
                    program.add(quadtree::SET, quadtree::SET_WGSL, &deps.iter().map(String::as_str).collect::<Vec<_>>());
                    let disc_deps: Vec<String> = std::iter::once(quadtree::SET.to_string()).chain(deps.iter().cloned()).collect();
                    check(&program, quadtree::GPU_WGSL, &disc_deps, &format!("quadtree {st} {k}"));
                    program.define("HOLES");
                    check(&program, quadtree::GPU_WGSL, &disc_deps, &format!("quadtree with holes {st} {k}"));
                    check(&program, mr_components::holes::coverage::GPU_WGSL, &disc_deps, &format!("holes {st} {k}"));
                }
            }
        }
    }
}

/// The view writers validate.
#[test]
fn view_writers_validate() {
    use wgpu::naga::{front::wgsl, valid};
    let m = wgsl::parse_str(&format!("{}{}", crate::view::wgsl(), include_str!("../view.wgsl"))).unwrap_or_else(|e| panic!("{e}"));
    valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::all()).validate(&m).unwrap();
}
