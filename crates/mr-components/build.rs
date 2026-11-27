//! The GPU sources of walks and shadings, by name, from the source tree: a component's WGSL
//! is `src/<role>/<name>/<file>`, so adding one needs no hand-kept table of names. A source
//! may begin with lines `// include <path>` (relative to its folder); those files are put in
//! front of it, in order (the shared sampling of the stateful shadings).

use std::fmt::Write as _;
use std::path::Path;

/// (constant, role folder, file name).
const TABLES: &[(&str, &str, &str)] = &[
    ("TRAVERSAL_GPU", "traversal", "gpu.wgsl"),
    ("TRAVERSAL_RT", "traversal", "rt.wgsl"),
    ("SHADING_GPU", "shading", "gpu.wgsl"),
];

fn main() {
    let src = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let mut out = String::new();
    for &(name, role, file) in TABLES {
        let dir = src.join(role);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut entries: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
        entries.sort();
        writeln!(out, "pub(crate) const {name}: &[(&str, &str)] = &[").unwrap();
        for d in entries {
            println!("cargo:rerun-if-changed={}", d.display());
            let path = d.join(file);
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let mut parts: Vec<String> = text
                .lines()
                .map_while(|l| l.strip_prefix("// include "))
                .map(|inc| {
                    let p = d.join(inc.trim());
                    assert!(p.is_file(), "{}: `// include {inc}` names no file", path.display());
                    format!("include_str!({:?})", p.display().to_string())
                })
                .collect();
            parts.push(format!("include_str!({:?})", path.display().to_string()));
            let component = d.file_name().unwrap().to_str().unwrap();
            writeln!(out, "    ({component:?}, concat!({})),", parts.join(", ")).unwrap();
        }
        writeln!(out, "];").unwrap();
    }
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("gpu_sources.rs"), out).unwrap();
}
