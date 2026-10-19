//! The encyclopedia (`docs/schemes/`) covers the registry: every component has an entry
//! that names it in `components`, and every component an entry names exists.

use mr_components::graph::{CLIENTS, GENERATORS, LINKS, POSTS};
use mr_components::{Slot, COMPONENTS};
use std::collections::BTreeSet;
use std::path::Path;

/// Every component as `slot/name`, the encyclopedia's spelling. Shadow slots reuse the
/// storage and traversal components; `none` and `same` are absences, not schemes.
fn registry() -> BTreeSet<String> {
    let slots = COMPONENTS.iter().filter(|c| !matches!(c.slot, Slot::ShadowStorage | Slot::ShadowTraversal)).map(|c| (c.slot.name(), c.name));
    let roles = [("link", LINKS), ("client", CLIENTS), ("generator", GENERATORS), ("post", POSTS)];
    slots.chain(roles.iter().flat_map(|&(role, names)| names.iter().map(move |&n| (role, n)))).filter(|(_, n)| !matches!(*n, "none" | "same")).map(|(s, n)| format!("{s}/{n}")).collect()
}

/// `(entry path, its header's components)` for every entry.
fn entries(dir: &Path) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        if path.is_dir() {
            out.extend(entries(&path));
        } else if path.extension().is_some_and(|x| x == "md") && path.file_name().unwrap() != "README.md" {
            let text = std::fs::read_to_string(&path).unwrap();
            let header = text.strip_prefix("+++\n").and_then(|t| t.split_once("\n+++\n")).unwrap_or_else(|| panic!("{}: no +++ header", path.display())).0;
            let table: toml::Table = toml::from_str(header).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let comps = table.get("components").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            out.push((path.display().to_string(), comps));
        }
    }
    out
}

#[test]
fn every_component_has_an_entry() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/schemes");
    let registry = registry();
    let entries = entries(&dir);
    let named: BTreeSet<&String> = entries.iter().flat_map(|(_, c)| c).collect();
    let missing: Vec<&String> = registry.iter().filter(|c| !named.contains(c)).collect();
    assert!(missing.is_empty(), "components without an entry in docs/schemes: {missing:?}");
    for (path, comps) in &entries {
        for c in comps {
            assert!(registry.contains(c), "{path}: no component {c} in the registry");
        }
    }
}
