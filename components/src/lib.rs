//! Renderer components (IDEAS §12): instead of choosing a monolithic renderer, a renderer is
//! assembled from one component per slot and run on a back end:
//!
//! `backend` × `storage` × `visibility` × `shading`
//!
//! This crate is the single source of truth for what exists and what combines: component
//! metadata, compatibility (used by the GUI to forbid unsupported combinations, by plan
//! loading and by the back ends), and the component implementations themselves, one
//! directory per piece (`storage/prefix/gpu.wgsl`, ...). Back ends (`backends/`) assemble a
//! working renderer from a [`Composition`].
//!
//! Compatibility rule: a component combines with a back end if it has an implementation for
//! it. Optional fast-path queries never forbid a combination (generic fallbacks exist), so
//! within a back end every storage pairs with every visibility component.

use serde::{Deserialize, Serialize};

pub mod shading;
pub mod storage;
pub mod visibility;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Storage,
    Visibility,
    Shading,
}

impl Slot {
    pub const ALL: [Slot; 3] = [Slot::Storage, Slot::Visibility, Slot::Shading];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Storage => "storage",
            Slot::Visibility => "visibility",
            Slot::Shading => "shading",
        }
    }
}

pub struct BackendInfo {
    pub name: &'static str,
    pub description: &'static str,
    /// Renderer entry the worker runs for this back end (`None`: not available yet).
    pub renderer: Option<&'static str>,
}

/// Every back end, including planned ones (shown disabled, with the reason).
pub const BACKENDS: &[BackendInfo] = &[
    BackendInfo { name: "gpu-compute", description: "GPU compute shaders (wgpu / WGSL, on Vulkan here): one thread per pixel walks the composed storage and visibility", renderer: Some("gpu-compute") },
    BackendInfo { name: "cpu", description: "CPU, multi-threaded (rayon, SIMD where it pays) — planned (phase 2)", renderer: None },
    BackendInfo { name: "gpu-raster", description: "GPU rasterisation of generated or discovered geometry — planned (phase 3)", renderer: None },
    BackendInfo { name: "gpu-rt", description: "GPU hardware ray tracing (ray queries / pipelines) — planned (phase 4)", renderer: None },
];

pub struct ComponentInfo {
    pub slot: Slot,
    pub name: &'static str,
    pub description: &'static str,
    /// Back ends with an implementation of this component.
    pub backends: &'static [&'static str],
    /// Optional queries this storage implements fast / this visibility component relies on
    /// (only advisory: the generic fallbacks cover any gap).
    pub provides: &'static [&'static str],
    pub uses: &'static [&'static str],
}

const GPU: &[&str] = &["gpu-compute"];

pub const COMPONENTS: &[ComponentInfo] = &[
    // Storage.
    ComponentInfo { slot: Slot::Storage, name: "bricks", description: "The shared pool: a u16 id per voxel and one bit per 4^3 brick (levels 1, 4). gpu-dda's data.", backends: GPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "hier", description: "Bricks plus one bit per 8^3 region (levels 1, 4, 8), as cpu-simd-hier.", backends: GPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "prefix", description: "The user's structure (IDEAS §1): Morton-ordered 4^3 masks plus a popcount prefix; any aligned 2..16 cube's emptiness is one compare (levels 1-16).", backends: GPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "distance", description: "Per-voxel Chebyshev distance to the nearest solid voxel (from gpu-sdf). Fast `s_dist`.", backends: GPU, provides: &["s_dist"], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "rows", description: "Bricks plus 32x32 occupancy rows per chunk (from gpu-bitmask). Fast `s_row`.", backends: GPU, provides: &["s_row"], uses: &[] },
    // Visibility.
    ComponentInfo { slot: Slot::Visibility, name: "refloor", description: "Per-pixel DDA: ask the storage for the largest empty cube, step out of it, re-floor (gpu-dda's walk).", backends: GPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Visibility, name: "incremental", description: "Hierarchical DDA keeping per-level exit times, moving between levels incrementally (IDEAS §1). Knobs: exact, skip2..16.", backends: GPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Visibility, name: "sphere", description: "Chebyshev sphere tracing: leave the whole empty box around the cell in one step (from gpu-sdf). Fast with `distance`.", backends: GPU, provides: &[], uses: &["s_dist"] },
    ComponentInfo { slot: Slot::Visibility, name: "rowrun", description: "Refloor plus bit-scan jumps across empty row runs when the ray runs along x (from gpu-bitmask). Fast with `rows`.", backends: GPU, provides: &[], uses: &["s_row"] },
    // Shading.
    ComponentInfo { slot: Slot::Shading, name: "direct", description: "The reference lighting model: texel (unlit); ambient, sun and point lights with shadow rays through the same walk (lit).", backends: GPU, provides: &[], uses: &[] },
];

/// A renderer as an assembly of components (missing fields take the defaults).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Composition {
    pub backend: String,
    pub storage: String,
    pub visibility: String,
    pub shading: String,
}

impl Default for Composition {
    fn default() -> Self {
        Self { backend: "gpu-compute".into(), storage: "bricks".into(), visibility: "refloor".into(), shading: "direct".into() }
    }
}

impl Composition {
    pub fn get(&self, slot: Slot) -> &str {
        match slot {
            Slot::Storage => &self.storage,
            Slot::Visibility => &self.visibility,
            Slot::Shading => &self.shading,
        }
    }

    pub fn set(&mut self, slot: Slot, name: &str) {
        let v = match slot {
            Slot::Storage => &mut self.storage,
            Slot::Visibility => &mut self.visibility,
            Slot::Shading => &mut self.shading,
        };
        *v = name.to_string();
    }

    /// Short label, e.g. `gpu-compute: prefix + incremental`.
    pub fn label(&self) -> String {
        let shade = if self.shading == "direct" { String::new() } else { format!(" + {}", self.shading) };
        format!("{}: {} + {}{shade}", self.backend, self.storage, self.visibility)
    }

    /// The renderer entry to run and the options that select the components.
    pub fn resolve(&self) -> Result<(&'static str, Vec<(&'static str, String)>), String> {
        self.validate()?;
        let b = backend(&self.backend).unwrap();
        Ok((b.renderer.unwrap(), vec![("storage", self.storage.clone()), ("visibility", self.visibility.clone()), ("shading", self.shading.clone())]))
    }

    /// Why this composition cannot run, if it can't.
    pub fn validate(&self) -> Result<(), String> {
        let b = backend(&self.backend).ok_or_else(|| format!("unknown back end `{}` (have: {})", self.backend, BACKENDS.iter().map(|b| b.name).collect::<Vec<_>>().join(", ")))?;
        if b.renderer.is_none() {
            return Err(format!("back end `{}` is not available yet", self.backend));
        }
        for slot in Slot::ALL {
            if let Some(why) = unavailable(&self.backend, slot, self.get(slot)) {
                return Err(why);
            }
        }
        Ok(())
    }
}

pub fn backend(name: &str) -> Option<&'static BackendInfo> {
    BACKENDS.iter().find(|b| b.name == name)
}

pub fn component(slot: Slot, name: &str) -> Option<&'static ComponentInfo> {
    COMPONENTS.iter().find(|c| c.slot == slot && c.name == name)
}

pub fn components(slot: Slot) -> impl Iterator<Item = &'static ComponentInfo> {
    COMPONENTS.iter().filter(move |c| c.slot == slot)
}

/// Why component `name` cannot fill `slot` on `backend` (`None` = it can).
pub fn unavailable(backend: &str, slot: Slot, name: &str) -> Option<String> {
    let Some(c) = component(slot, name) else {
        return Some(format!("unknown {} component `{name}` (have: {})", slot.name(), components(slot).map(|c| c.name).collect::<Vec<_>>().join(", ")));
    };
    (!c.backends.contains(&backend)).then(|| format!("`{name}` has no {backend} implementation yet (available on: {})", c.backends.join(", ")))
}

/// Advisory note for an allowed pairing that runs through a generic fallback.
pub fn fallback_note(c: &Composition) -> Option<String> {
    let st = component(Slot::Storage, &c.storage)?;
    let vis = component(Slot::Visibility, &c.visibility)?;
    let missing: Vec<&str> = vis.uses.iter().copied().filter(|q| !st.provides.contains(q)).collect();
    (!missing.is_empty()).then(|| format!("`{}` answers {} through a generic fallback: correct, but slower than a storage with that fast path", c.storage, missing.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metadata and implementations agree: every component listed for gpu-compute has GPU
    /// code, every storage's `provides` matches its implementation, and every pairing on a
    /// back end validates.
    #[test]
    fn registry_is_consistent() {
        for c in COMPONENTS.iter().filter(|c| c.backends.contains(&"gpu-compute")) {
            let ok = match c.slot {
                Slot::Storage => storage::gpu(c.name).is_some_and(|g| g.provides == c.provides),
                Slot::Visibility => visibility::gpu_wgsl(c.name).is_some(),
                Slot::Shading => shading::gpu_wgsl(c.name).is_some(),
            };
            assert!(ok, "{} `{}`", c.slot.name(), c.name);
        }
        for s in components(Slot::Storage) {
            for v in components(Slot::Visibility) {
                let comp = Composition { storage: s.name.into(), visibility: v.name.into(), ..Default::default() };
                assert!(comp.validate().is_ok(), "{comp:?}");
            }
        }
        assert!(Composition { backend: "cpu".into(), ..Default::default() }.validate().is_err());
        assert!(Composition { storage: "nope".into(), ..Default::default() }.validate().is_err());
        assert!(fallback_note(&Composition { storage: "bricks".into(), visibility: "sphere".into(), ..Default::default() }).is_some());
        assert!(fallback_note(&Composition { storage: "distance".into(), visibility: "sphere".into(), ..Default::default() }).is_none());
    }
}
