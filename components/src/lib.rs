//! Renderer components (IDEAS §12): instead of choosing a monolithic renderer, a renderer is
//! assembled from one component per slot and run on a back end:
//!
//! `backend` × `storage` × `geometry` × `prepass` × `visibility` × `shading`
//!
//! Not every slot applies to every back end: `geometry` (storage turned into drawable faces)
//! exists only on rasterising back ends, `prepass` (a stage before the walk, e.g. beams)
//! only on gpu-compute; slots a back end lacks are left empty.
//!
//! This crate is the single source of truth for what exists and what combines: component
//! metadata, compatibility (used by the GUI to forbid unsupported combinations, by plan
//! loading and by the back ends), and the component implementations themselves, one
//! directory per piece (`storage/prefix/gpu.wgsl`, ...). Back ends (`backends/`) assemble a
//! working renderer from a [`Composition`].
//!
//! Compatibility rule: a component combines with a back end if the back end has its slot and
//! the component has an implementation for it. Optional fast-path queries never forbid a
//! combination (generic fallbacks exist), so within a back end every component pairs with
//! every other.

use serde::{Deserialize, Serialize};

pub mod geometry;
pub mod prepass;
pub mod shading;
pub mod storage;
pub mod visibility;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Storage,
    Geometry,
    Prepass,
    Visibility,
    Shading,
}

impl Slot {
    /// In composition order (labels, the GUI chooser).
    pub const ALL: [Slot; 5] = [Slot::Storage, Slot::Geometry, Slot::Prepass, Slot::Visibility, Slot::Shading];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Storage => "storage",
            Slot::Geometry => "geometry",
            Slot::Prepass => "prepass",
            Slot::Visibility => "visibility",
            Slot::Shading => "shading",
        }
    }

    /// Slots that usually keep their default (`direct` shading, no prepass): left out of
    /// labels while at the default, and last (optional) in the CLI form.
    pub fn optional(self) -> bool {
        matches!(self, Slot::Prepass | Slot::Shading)
    }
}

pub struct BackendInfo {
    pub name: &'static str,
    pub description: &'static str,
    /// Renderer entry the worker runs for this back end (`None`: not available yet).
    pub renderer: Option<&'static str>,
    /// The slots this back end has, each with its default component.
    pub slots: &'static [(Slot, &'static str)],
}

impl BackendInfo {
    pub fn has(&self, slot: Slot) -> bool {
        self.slots.iter().any(|&(s, _)| s == slot)
    }

    pub fn default_for(&self, slot: Slot) -> Option<&'static str> {
        self.slots.iter().find(|&&(s, _)| s == slot).map(|&(_, d)| d)
    }
}

const RAY_SLOTS: &[(Slot, &str)] = &[(Slot::Storage, "bricks"), (Slot::Visibility, "refloor"), (Slot::Shading, "direct")];
const COMPUTE_SLOTS: &[(Slot, &str)] = &[(Slot::Storage, "bricks"), (Slot::Prepass, "none"), (Slot::Visibility, "refloor"), (Slot::Shading, "direct")];

/// Every back end, including planned ones (shown disabled, with the reason).
pub const BACKENDS: &[BackendInfo] = &[
    BackendInfo { name: "gpu-compute", description: "GPU compute shaders (wgpu / WGSL, on Vulkan here): one thread per pixel walks the composed storage and visibility", renderer: Some("gpu-compute"), slots: COMPUTE_SLOTS },
    BackendInfo { name: "cpu", description: "CPU, multi-threaded (rayon), one monomorphised instance per storage; ray packets and AVX2/AVX-512 hot loops", renderer: Some("cpu"), slots: RAY_SLOTS },
    BackendInfo {
        name: "gpu-raster",
        description: "GPU rasterisation (wgpu): the geometry component turns chunks into faces on the CPU, drawn with vertex pulling and multi-draw-indirect after chunk and face-direction culling. Storage here is what lit-mode shadow rays walk (refloor)",
        renderer: Some("gpu-raster"),
        slots: &[(Slot::Storage, "bricks"), (Slot::Geometry, "greedy"), (Slot::Visibility, "raster"), (Slot::Shading, "direct")],
    },
    BackendInfo { name: "gpu-rt", description: "GPU hardware ray tracing (ray queries / pipelines) — planned (phase 4)", renderer: None, slots: RAY_SLOTS },
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
const CPU: &[&str] = &["cpu"];
const BOTH: &[&str] = &["gpu-compute", "cpu"];
/// GPU storages: the ray walks, and the raster back end's shadow rays.
const GPU_ST: &[&str] = &["gpu-compute", "gpu-raster"];
const ALL_ST: &[&str] = &["gpu-compute", "cpu", "gpu-raster"];
const RASTER: &[&str] = &["gpu-raster"];

pub const COMPONENTS: &[ComponentInfo] = &[
    // Storage.
    ComponentInfo { slot: Slot::Storage, name: "bricks", description: "The shared pool: a u16 id per voxel and one bit per 4^3 brick (levels 1, 4). gpu-dda's data.", backends: ALL_ST, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "hier", description: "Bricks plus one bit per 8^3 region (levels 1, 4, 8), as cpu-simd-hier.", backends: ALL_ST, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "prefix", description: "The user's structure (IDEAS §1): Morton-ordered 4^3 masks plus a popcount prefix; any aligned 2..16 cube's emptiness is one compare (levels 1-16).", backends: ALL_ST, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "distance", description: "Per-voxel Chebyshev distance to the nearest solid voxel (from gpu-sdf). Fast `s_dist`.", backends: GPU_ST, provides: &["s_dist"], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "rows", description: "Bricks plus 32x32 occupancy rows per chunk (from gpu-bitmask). Fast `s_row`.", backends: GPU_ST, provides: &["s_row"], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "tree64", description: "Sparse 64-tree per 64^3 region (from gpu-tree64): 4x4x4 child masks, popcount-packed children, compressed leaves, uniform subtrees collapsed (levels 0, 2, 4, 6). Owns its data: no voxel pool, ~15% of its VRAM.", backends: GPU_ST, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "svo_dag", description: "Sparse voxel DAG (from gpu-svo-dag; Kampe et al. 2013): per-chunk binary octrees, identical subtrees shared across all chunks (~3x dedup on terrain). No voxel pool: the smallest GPU footprint.", backends: GPU_ST, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "clipmap", description: "Camera-centred toroidal 3D textures (from gpu-clipmap): block ids, 4^3 brick occupancy and slot tags, read with textureLoad only; no hash table, no voxel pool.", backends: GPU_ST, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "u8", description: "Brick mask plus a u8 id per voxel: half of bricks' footprint (32 KiB per chunk; at most 256 block types).", backends: CPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "bits", description: "One occupancy bit per voxel, brick-major (4 KiB per chunk); block ids read only at the hit.", backends: CPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Storage, name: "palette", description: "Per-chunk palette with indices packed at 1/2/4/8/16 bits (7x smaller than bricks on terrain).", backends: CPU, provides: &[], uses: &[] },
    // Visibility.
    ComponentInfo { slot: Slot::Visibility, name: "refloor", description: "Per-pixel DDA: ask the storage for the largest empty cube, step out of it, re-floor (gpu-dda's walk).", backends: BOTH, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Visibility, name: "incremental", description: "Hierarchical DDA keeping per-level exit times, moving between levels incrementally (IDEAS §1). Knobs: exact, skip2..16.", backends: BOTH, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Visibility, name: "sphere", description: "Chebyshev sphere tracing: leave the whole empty box around the cell in one step (from gpu-sdf). Fast with `distance`.", backends: GPU, provides: &[], uses: &["s_dist"] },
    ComponentInfo { slot: Slot::Visibility, name: "rowrun", description: "Refloor plus bit-scan jumps across empty row runs when the ray runs along x (from gpu-bitmask). Fast with `rows`.", backends: GPU, provides: &[], uses: &["s_row"] },
    ComponentInfo { slot: Slot::Visibility, name: "raster", description: "Depth-tested rasterisation of the geometry; the fragment shader intersects the pixel's ray with the face plane and shades. Knob: depth_prepass (lit: shade once per pixel).", backends: RASTER, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Visibility, name: "visbuffer", description: "Visibility buffer: the raster pass writes only a face id and depth; a compute pass reconstructs the hit and shades once per pixel, independent of overdraw.", backends: RASTER, provides: &[], uses: &[] },
    // Prepass.
    ComponentInfo { slot: Slot::Prepass, name: "none", description: "No prepass: every ray walks from the camera.", backends: GPU, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Prepass, name: "beam", description: "Conservative per-tile beam (from gpu-beam): one cone per tile marches top cells; rays start where their tile's cone is provably empty. Exact. Knob: tile (pixels, 1 = off).", backends: GPU, provides: &[], uses: &[] },
    // Geometry.
    ComponentInfo { slot: Slot::Geometry, name: "greedy", description: "Greedy quads (CPU, per chunk, incremental): coplanar exposed faces merged into rectangles. Knob: merge = block (same block only) or exposure (any block, looked up per fragment).", backends: RASTER, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Geometry, name: "naive", description: "One quad per exposed block face, no merging (\"Minecraft-style\"). Knob: cull = false emits all six faces of every solid block.", backends: RASTER, provides: &[], uses: &[] },
    ComponentInfo { slot: Slot::Geometry, name: "cubes", description: "One 36-vertex cube instance per surface block; the hardware back-face culls the hidden half. Knob: cull = false instances every solid block.", backends: RASTER, provides: &[], uses: &[] },
    // Shading.
    ComponentInfo { slot: Slot::Shading, name: "direct", description: "The reference lighting model: texel (unlit); ambient, sun and point lights with shadow rays through the same walk (lit).", backends: ALL_ST, provides: &[], uses: &[] },
];

/// A renderer as an assembly of components. Slots left out (empty) take the back end's
/// default; slots the back end doesn't have stay empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub storage: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub geometry: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prepass: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub visibility: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shading: String,
}

fn default_backend() -> String {
    "gpu-compute".into()
}

/// `gpu-compute: bricks + refloor + direct`.
impl Default for Composition {
    fn default() -> Self {
        Self::new("gpu-compute")
    }
}

impl Composition {
    /// The back end's default composition (empty slots if the back end is unknown).
    pub fn new(backend: &str) -> Self {
        let mut c = Self { backend: backend.into(), storage: String::new(), geometry: String::new(), prepass: String::new(), visibility: String::new(), shading: String::new() };
        c.fill_defaults();
        c
    }

    pub fn get(&self, slot: Slot) -> &str {
        match slot {
            Slot::Storage => &self.storage,
            Slot::Geometry => &self.geometry,
            Slot::Prepass => &self.prepass,
            Slot::Visibility => &self.visibility,
            Slot::Shading => &self.shading,
        }
    }

    pub fn set(&mut self, slot: Slot, name: &str) {
        let v = match slot {
            Slot::Storage => &mut self.storage,
            Slot::Geometry => &mut self.geometry,
            Slot::Prepass => &mut self.prepass,
            Slot::Visibility => &mut self.visibility,
            Slot::Shading => &mut self.shading,
        };
        *v = name.to_string();
    }

    /// The slots of this composition's back end (none if it is unknown).
    pub fn slots(&self) -> impl Iterator<Item = Slot> + use<> {
        let b = backend(&self.backend);
        Slot::ALL.into_iter().filter(move |&s| b.is_some_and(|b| b.has(s)))
    }

    /// Fills empty slots the back end has with its defaults.
    pub fn fill_defaults(&mut self) {
        let Some(b) = backend(&self.backend) else { return };
        for &(slot, d) in b.slots {
            if self.get(slot).is_empty() {
                self.set(slot, d);
            }
        }
    }

    /// After a back-end change: keeps the components that exist there, defaults the other
    /// slots it has and clears the slots it lacks.
    pub fn adapt_to_backend(&mut self) {
        let Some(b) = backend(&self.backend) else { return };
        for slot in Slot::ALL {
            if !b.has(slot) {
                self.set(slot, "");
            } else if unavailable(&self.backend, slot, self.get(slot)).is_some() {
                self.set(slot, b.default_for(slot).unwrap());
            }
        }
    }

    /// Short label: the back end's slots in order, optional slots at their default left
    /// out, e.g. `gpu-compute: prefix + incremental` or `gpu-raster: bricks + greedy + raster`.
    pub fn label(&self) -> String {
        let b = backend(&self.backend);
        let parts: Vec<&str> = self.slots().filter(|&s| !(s.optional() && b.and_then(|b| b.default_for(s)) == Some(self.get(s)))).map(|s| self.get(s)).collect();
        format!("{}: {}", self.backend, parts.join(" + "))
    }

    /// The renderer entry to run and the options that select the components (fills
    /// defaults first).
    pub fn resolve(&mut self) -> Result<(&'static str, Vec<(&'static str, String)>), String> {
        self.fill_defaults();
        self.validate()?;
        let b = backend(&self.backend).unwrap();
        Ok((b.renderer.unwrap(), self.slots().map(|s| (s.name(), self.get(s).to_string())).collect()))
    }

    /// Why this composition cannot run, if it can't.
    pub fn validate(&self) -> Result<(), String> {
        let b = backend(&self.backend).ok_or_else(|| format!("unknown back end `{}` (have: {})", self.backend, BACKENDS.iter().map(|b| b.name).collect::<Vec<_>>().join(", ")))?;
        if b.renderer.is_none() {
            return Err(format!("back end `{}` is not available yet", self.backend));
        }
        for slot in Slot::ALL {
            if !b.has(slot) {
                if !self.get(slot).is_empty() {
                    return Err(format!("back end `{}` has no {} slot (`{}` does not apply)", self.backend, slot.name(), self.get(slot)));
                }
            } else if let Some(why) = unavailable(&self.backend, slot, self.get(slot)) {
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
pub fn unavailable(backend_name: &str, slot: Slot, name: &str) -> Option<String> {
    if backend(backend_name).is_some_and(|b| !b.has(slot)) {
        return Some(format!("`{backend_name}` has no {} slot", slot.name()));
    }
    let Some(c) = component(slot, name) else {
        return Some(format!("unknown {} component `{name}` (have: {})", slot.name(), components(slot).map(|c| c.name).collect::<Vec<_>>().join(", ")));
    };
    (!c.backends.contains(&backend_name)).then(|| format!("`{name}` has no {backend_name} implementation yet (available on: {})", c.backends.join(", ")))
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
                Slot::Prepass => prepass::gpu_wgsl(c.name).is_some(),
                Slot::Geometry => false,
            };
            assert!(ok, "{} `{}`", c.slot.name(), c.name);
        }
        for c in COMPONENTS.iter().filter(|c| c.backends.contains(&"gpu-raster")) {
            let ok = match c.slot {
                Slot::Storage => storage::gpu(c.name).is_some(),
                Slot::Geometry => geometry::gpu(c.name).is_some(),
                Slot::Visibility => visibility::raster_wgsl(c.name).is_some(),
                Slot::Shading => shading::gpu_wgsl(c.name).is_some(),
                Slot::Prepass => false,
            };
            assert!(ok, "gpu-raster {} `{}`", c.slot.name(), c.name);
        }
        // Every component's back ends have its slot; every back end's defaults run.
        for c in COMPONENTS {
            assert!(c.backends.iter().all(|b| backend(b).unwrap().has(c.slot)), "{}", c.name);
        }
        // Within a back end, every component pairs with every other.
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let mut combos = vec![Composition::new(b.name)];
            assert!(combos[0].validate().is_ok(), "{}", b.name);
            for slot in Slot::ALL.into_iter().filter(|&s| b.has(s)) {
                combos = combos
                    .iter()
                    .flat_map(|c| {
                        components(slot).filter(|x| x.backends.contains(&b.name)).map(move |x| {
                            let mut c = c.clone();
                            c.set(slot, x.name);
                            c
                        })
                    })
                    .collect();
            }
            for comp in combos {
                assert!(comp.validate().is_ok(), "{comp:?}");
            }
        }
        assert!(Composition { backend: "cpu".into(), storage: "distance".into(), ..Default::default() }.validate().is_err());
        assert!(Composition { geometry: "greedy".into(), ..Default::default() }.validate().is_err());
        assert!(Composition { visibility: "raster".into(), ..Default::default() }.validate().is_err());
        assert!(Composition { backend: "gpu-rt".into(), ..Default::default() }.validate().is_err());
        let mut raster = Composition { backend: "gpu-raster".into(), storage: String::new(), visibility: String::new(), shading: String::new(), prepass: String::new(), geometry: "cubes".into() };
        assert_eq!(raster.resolve().unwrap().0, "gpu-raster");
        assert_eq!(raster.label(), "gpu-raster: bricks + cubes + raster");
        let mut moved = Composition { storage: "rows".into(), visibility: "sphere".into(), ..Default::default() };
        moved.backend = "gpu-raster".into();
        moved.adapt_to_backend();
        assert_eq!(moved, Composition { backend: "gpu-raster".into(), storage: "rows".into(), geometry: "greedy".into(), prepass: String::new(), visibility: "raster".into(), shading: "direct".into() });
        assert_eq!(Composition { prepass: "beam".into(), ..Default::default() }.label(), "gpu-compute: bricks + beam + refloor");
        assert!(Composition { storage: "nope".into(), ..Default::default() }.validate().is_err());
        assert!(fallback_note(&Composition { storage: "bricks".into(), visibility: "sphere".into(), ..Default::default() }).is_some());
        assert!(fallback_note(&Composition { storage: "distance".into(), visibility: "sphere".into(), ..Default::default() }).is_none());
    }
}
