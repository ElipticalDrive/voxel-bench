//! Embeds the encyclopedia (`docs/schemes/**/*.md`) for the GUI: `$OUT_DIR/schemes.rs` is a
//! list of `(path under docs/schemes, text)`.

use std::path::{Path, PathBuf};

fn entries(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            entries(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") && p.file_name().is_some_and(|n| n != "README.md") {
            out.push(p);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("docs/schemes");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = vec![];
    entries(&root, &mut files);
    files.sort();
    let mut src = String::from("pub static ENTRIES: &[(&str, &str)] = &[\n");
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        src += &format!("    ({rel:?}, include_str!({:?})),\n", f.display().to_string());
    }
    src += "];\n";
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("schemes.rs"), src).unwrap();
}
