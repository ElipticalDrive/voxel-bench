//! Architecture guard (IDEAS §16 step 4): catches code of redundant purpose creeping back.
//!
//! 1. No WGSL function is defined by more than one back end (`backends/*`), except the
//!    functions a component interface asks every implementer for (`INTERFACE`) and entry
//!    points.
//! 2. No back end defines a WGSL function that shared code (`components`, `crates`)
//!    already defines.
//! 3. No two places define the same WGSL function body under different names (a renamed
//!    copy, as `face_axes_d` once was).
//! 4. Lighting goes through the shading component: back ends neither call the lighting
//!    model's internals nor compute light themselves (`LIGHTING_WGSL`, `LIGHTING_RUST`),
//!    and each back end uses its kind's shading entry point (`SHADING_USE`). Exceptions are
//!    listed with their reason.
//!
//! WGSL is read from `.wgsl` files and from string literals in Rust sources (WGSL composed
//! in code).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Functions a component interface asks of every implementer (one per back end by design).
const INTERFACE: &[(&str, &str)] = &[
    ("src_vertex", "face source (visibility/faces.wgsl): one per face record format"),
    ("src_origin", "face source"),
    ("src_block", "face source"),
    ("src_lookup", "face source"),
    ("src_view", "face source: each source's diagnostic views"),
];

/// Lighting internals only the shading components (and the lighting model) may call.
const LIGHTING_WGSL: &[&str] = &["shade_lit(", "direct_light(", "attenuation(", "lookup_light_cell("];
/// Light computed in Rust outside the shading component and the reference model.
const LIGHTING_RUST: &[&str] = &["point_light(", "light_at(", "env.ambient", "env.sun", ".sun_shadows"];

/// Places allowed to use the lighting internals, with the reason.
const LIGHTING_ALLOWED: &[(&str, &str)] = &[
    ("components/src/shading/", "the shading components"),
    ("crates/mr-gpu/src/lighting.wgsl", "the lighting model itself"),
    ("crates/mr-gpu/src/voxels.rs", "uploads the environment for the lighting model"),
    ("crates/mr-gpu/src/mesh.wgsl", "the meshes stage shades vertex-coloured mesh fragments, not a voxel Surface (until meshes feed the shading component)"),
    ("crates/mr-world/", "scene description (sun direction, ambient), not lighting"),
    ("crates/mr-core/", "the lighting model's shared math and the environment type"),
    ("renderers/cpu-reference/", "the reference renderer's f64 model"),
];

/// Each back end's shading entry point (a string its sources must contain).
const SHADING_USE: &[(&str, &str)] = &[
    ("gpu-compute", "shading::gpu_wgsl("),
    ("gpu-raster", "shading::gpu_wgsl("),
    ("gpu-rt", "shading::gpu_wgsl("),
    ("gpu-discover", "shading::gpu_wgsl("),
    ("cpu-discover", "shading::gpu_wgsl("),
    ("gpu-splat", "shading::gpu_wgsl("),
    ("gpu-voxelspace", "shading::gpu_wgsl("),
    ("cpu", "direct::cpu::"),
    ("cpu-voxelspace", "direct::cpu::"),
    // The reference's f64 model on purpose: the same algorithm at a higher precision (the
    // f32 component cost 4-22 dB of lit PSNR; PLAN, step 1).
    ("cpu-raster", "shade_surface("),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                files(&p, ext, out);
            }
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap().to_string_lossy().into_owned()
}

/// Owner of a source file: the back end's name, or "shared".
fn owner(rel: &str) -> String {
    match rel.strip_prefix("backends/") {
        Some(r) => r.split('/').next().unwrap().to_string(),
        None => "shared".into(),
    }
}

/// The WGSL text of a file: the whole `.wgsl`, or the string literals of a `.rs` (with
/// `\n` escapes and line continuations undone, `{{` `}}` format escapes collapsed).
fn wgsl_text(path: &Path) -> String {
    let src = fs::read_to_string(path).unwrap();
    if path.extension().is_some_and(|x| x == "wgsl") {
        return src;
    }
    let mut out = String::new();
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if !in_str {
            if c == '"' {
                in_str = true;
            } else if c == '/' && chars.peek() == Some(&'/') {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            continue;
        }
        match c {
            '"' => {
                in_str = false;
                out.push('\n');
            }
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('\n') => {
                    while chars.peek().is_some_and(|c| c.is_whitespace()) {
                        chars.next();
                    }
                }
                Some(o) => out.push(o),
                None => {}
            },
            _ => out.push(c),
        }
    }
    out.replace("{{", "{").replace("}}", "}")
}

/// WGSL functions: (name, normalised body, entry point?).
fn functions(text: &str) -> Vec<(String, String, bool)> {
    let mut out = vec![];
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(off) = text[i..].find("fn ") {
        let at = i + off;
        i = at + 3;
        // A definition: at a line start (after indentation) or after an attribute.
        let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
        let before = text[line_start..at].trim();
        if !(before.is_empty() || before.ends_with(')')) || !before.chars().all(|c| c.is_whitespace() || c == '@' || c.is_alphanumeric() || "()_,".contains(c)) {
            continue;
        }
        let name: String = text[at + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if name.is_empty() || !text[at + 3 + name.len()..].starts_with('(') {
            continue;
        }
        let Some(open) = text[at..].find('{').map(|o| at + o) else { continue };
        let mut depth = 0;
        let mut end = open;
        for (k, &b) in bytes.iter().enumerate().skip(open) {
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = k;
                    break;
                }
            }
        }
        let body: String = text[open..=end].lines().map(|l| l.split("//").next().unwrap().trim()).collect::<Vec<_>>().join(" ");
        let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
        // An entry point: an attribute on this line or the previous one.
        let prev = text[..line_start.saturating_sub(1)].rsplit('\n').next().unwrap_or("");
        let entry = [before, prev].iter().any(|l| l.contains("@compute") || l.contains("@vertex") || l.contains("@fragment"));
        out.push((name, body, entry));
        i = end.max(i);
    }
    out
}

struct Def {
    file: String,
    owner: String,
    body: String,
    entry: bool,
}

fn definitions() -> BTreeMap<String, Vec<Def>> {
    let mut paths = vec![];
    for d in ["backends", "components", "crates"] {
        files(&root().join(d), "wgsl", &mut paths);
        files(&root().join(d), "rs", &mut paths);
    }
    let mut defs: BTreeMap<String, Vec<Def>> = BTreeMap::new();
    for p in paths {
        let file = rel(&p);
        let text = wgsl_text(&p);
        // Rust sources: only literals that look like WGSL functions count.
        for (name, body, entry) in functions(&text) {
            defs.entry(name).or_default().push(Def { owner: owner(&file), file: file.clone(), body, entry });
        }
    }
    defs
}

#[test]
fn no_wgsl_function_is_defined_twice() {
    let defs = definitions();
    let interface: BTreeSet<&str> = INTERFACE.iter().map(|&(n, _)| n).collect();
    let mut problems = vec![];
    for (name, ds) in &defs {
        if interface.contains(name.as_str()) || ds.iter().all(|d| d.entry) {
            continue;
        }
        let backends: BTreeSet<&str> = ds.iter().filter(|d| d.owner != "shared").map(|d| d.owner.as_str()).collect();
        if backends.len() > 1 {
            problems.push(format!("`{name}` is defined by several back ends: {}", ds.iter().map(|d| d.file.as_str()).collect::<Vec<_>>().join(", ")));
        }
        if ds.iter().any(|d| d.owner == "shared") && !backends.is_empty() {
            problems.push(format!("`{name}` is shared but redefined by a back end: {}", ds.iter().map(|d| d.file.as_str()).collect::<Vec<_>>().join(", ")));
        }
    }
    assert!(problems.is_empty(), "redundant WGSL functions (IDEAS §16):\n  {}", problems.join("\n  "));
}

#[test]
fn no_wgsl_body_is_copied_under_another_name() {
    let defs = definitions();
    let mut by_body: BTreeMap<&str, BTreeSet<(String, String)>> = BTreeMap::new();
    for (name, ds) in &defs {
        for d in ds.iter().filter(|d| !d.entry && d.body.len() >= 80) {
            by_body.entry(d.body.as_str()).or_default().insert((name.clone(), d.file.clone()));
        }
    }
    let problems: Vec<String> = by_body
        .values()
        .filter(|fs| fs.iter().map(|(n, _)| n).collect::<BTreeSet<_>>().len() > 1)
        .map(|fs| fs.iter().map(|(n, f)| format!("{n} ({f})")).collect::<Vec<_>>().join(" = "))
        .collect();
    assert!(problems.is_empty(), "identical WGSL bodies under different names:\n  {}", problems.join("\n  "));
}

#[test]
fn lighting_goes_through_the_shading_component() {
    let mut paths = vec![];
    for d in ["backends", "components", "crates", "renderers"] {
        files(&root().join(d), "wgsl", &mut paths);
        files(&root().join(d), "rs", &mut paths);
    }
    let mut problems = vec![];
    for p in &paths {
        let file = rel(p);
        if LIGHTING_ALLOWED.iter().any(|&(prefix, _)| file.starts_with(prefix)) {
            continue;
        }
        let src = fs::read_to_string(p).unwrap();
        let needles = if file.ends_with(".wgsl") { LIGHTING_WGSL } else { LIGHTING_RUST };
        // Rust sources may also carry WGSL in literals.
        let wgsl = if file.ends_with(".rs") { wgsl_text(p) } else { String::new() };
        for n in needles {
            if src.lines().any(|l| !l.trim_start().starts_with("//") && l.contains(n) && !l.contains(&format!("fn {n}"))) {
                problems.push(format!("{file} uses `{n}`"));
            }
        }
        for n in LIGHTING_WGSL {
            if wgsl.contains(n) {
                problems.push(format!("{file} composes WGSL calling `{n}`"));
            }
        }
    }
    for &(backend, needle) in SHADING_USE {
        let mut srcs = vec![];
        files(&root().join("backends").join(backend), "rs", &mut srcs);
        if !srcs.iter().any(|p| fs::read_to_string(p).unwrap().contains(needle)) {
            problems.push(format!("back end `{backend}` does not shade through `{needle}`"));
        }
    }
    let mut backends = vec![];
    if let Ok(rd) = fs::read_dir(root().join("backends")) {
        backends = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    }
    for b in backends {
        if !SHADING_USE.iter().any(|&(n, _)| n == b) {
            problems.push(format!("back end `{b}` has no entry in SHADING_USE (how does it shade?)"));
        }
    }
    assert!(problems.is_empty(), "lighting outside the shading component:\n  {}", problems.join("\n  "));
}

/// The guard itself: catches a planted duplicate, a renamed copy and an own lighting call.
#[test]
fn the_guard_catches_what_it_should() {
    let a = "fn face_axes(face: u32) -> vec3<u32> {\n    switch face { default: { return vec3<u32>(0u, 1u, 2u); } }\n}\n";
    let b = a.replace("face_axes", "face_axes_d");
    let fa = functions(a);
    let fb = functions(&b);
    assert_eq!(fa.len(), 1);
    assert_eq!(fa[0].0, "face_axes");
    assert_eq!(fb[0].0, "face_axes_d");
    assert_eq!(fa[0].1, fb[0].1, "renamed copies have equal bodies");
    let entry = functions("@compute @workgroup_size(8)\nfn main(@builtin(global_invocation_id) g: vec3<u32>) {\n    let x = 1u;\n}\n");
    assert!(entry[0].2, "entry points are recognised");
    let rs = "let s = \"fn occluded(p: vec3<f32>) -> bool {{ return ctrace(p).hit; }}\\n\";";
    let dir = std::env::temp_dir().join(format!("guard-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let f = dir.join("x.rs");
    fs::write(&f, rs).unwrap();
    let w = wgsl_text(&f);
    fs::remove_dir_all(&dir).ok();
    assert_eq!(functions(&w).first().map(|f| f.0.as_str()), Some("occluded"), "WGSL in Rust literals is read: {w}");
}
