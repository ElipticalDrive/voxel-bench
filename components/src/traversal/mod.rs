//! Traversal kernels (IDEAS §18): answer ray queries against a storage. A GPU kernel
//! provides `ctrace(o, d, max_t) -> Hit`, written only against the storage interface (the
//! capabilities it relies on are declared in the registry, `caps`). Visibility strategies
//! call it for primary rays (ray casting, discovery), shading for shadow and bounce rays
//! (`occluded`). CPU kernels: `refloor::cpu` (ray packets) and `incremental::cpu`.

pub mod cpu;
#[cfg(test)]
mod cpu_tests;
pub mod incremental {
    pub mod cpu;
}
pub mod refloor {
    pub mod cpu;
}

/// The GPU kernels that walk a storage.
pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "refloor" => include_str!("refloor/gpu.wgsl"),
        "incremental" => include_str!("incremental/gpu.wgsl"),
        "sphere" => include_str!("sphere/gpu.wgsl"),
        "rowrun" => include_str!("rowrun/gpu.wgsl"),
        _ => return None,
    })
}

/// The kernels of the gpu-rt back end (hardware ray queries against its BLAS).
pub fn rt_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "rayquery" => include_str!("rayquery/gpu.wgsl"),
        _ => return None,
    })
}

/// Secondary rays as the primary walk (`ctrace`) answers them: `occluded()` for lighting's
/// shadow rays, `strace()` for bounces (pathtrace). With another storage or kernel for them
/// (the `shadow_storage` / `shadow_traversal` stages) see [`secondary_wgsl`].
pub const OCCLUDED_WGSL: &str = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n\
fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit { return ctrace(o, d, max_t); }\n";

/// Shadow and bounce rays through another storage and kernel than the primary rays: the
/// storage (bound at `group` instead of 2) walked by `kernel`, compiled under names of its
/// own ([`namespaced`], suffix `_s`) next to the primary walk, as `strace()` and
/// `occluded()`.
pub fn secondary_wgsl(storage: &crate::storage::GpuStorage, kernel: &str, lod: bool, group: u32) -> String {
    let walk = gpu_wgsl(kernel).unwrap_or_else(|| panic!("no GPU kernel `{kernel}`"));
    let walk = format!("const LOD: bool = {lod};\n{}{}\n{walk}\n", default_consts(kernel), storage.source());
    let walk = namespaced(&walk, "_s").replace("@group(2)", &format!("@group({group})"));
    format!(
        "{walk}fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit {{ return ctrace_s(o, d, max_t); }}\n\
         fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {{ return strace(p, d, max_t).hit; }}\n"
    )
}

/// `src` with every name it declares at the top level (functions, constants, variables,
/// structs, aliases: lines that start with the declaration) renamed with `suffix`, so a
/// second copy of a storage and kernel can sit in the same module. Names used but not
/// declared in `src` (the world's `vp`, `Hit`, `lod_of`, ...) stay shared; member accesses
/// (`.name`) are left alone.
pub fn namespaced(src: &str, suffix: &str) -> String {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut names = std::collections::HashSet::new();
    for line in src.lines().filter(|l| !l.starts_with(char::is_whitespace)) {
        // Skip attributes (`@group(2) @binding(0)`).
        let mut rest = line;
        while let Some(r) = rest.strip_prefix('@') {
            let end = r.find(|c: char| c.is_whitespace()).unwrap_or(r.len());
            rest = r[end..].trim_start();
        }
        let Some(kw) = ["fn ", "const ", "struct ", "alias ", "override ", "var"].into_iter().find(|k| rest.starts_with(k)) else { continue };
        let mut after = &rest[kw.len()..];
        if kw == "var" {
            if let Some(r) = after.strip_prefix('<') {
                after = &r[r.find('>').map_or(r.len(), |i| i + 1)..];
            }
        }
        let after = after.trim_start();
        let name: String = after.chars().take_while(|&c| ident(c)).collect();
        if !name.is_empty() {
            names.insert(name);
        }
    }
    let mut out = String::with_capacity(src.len() + 256);
    let mut word = String::new();
    let mut prev = ' ';
    let flush = |word: &mut String, out: &mut String, prev: char| {
        out.push_str(word);
        if prev != '.' && names.contains(word.as_str()) {
            out.push_str(suffix);
        }
        word.clear();
    };
    for c in src.chars() {
        if ident(c) && !(word.is_empty() && c.is_ascii_digit()) {
            word.push(c);
        } else {
            if !word.is_empty() {
                flush(&mut word, &mut out, prev);
                prev = 'a';
            }
            out.push(c);
            if !c.is_whitespace() {
                prev = c;
            }
        }
    }
    if !word.is_empty() {
        flush(&mut word, &mut out, prev);
    }
    out
}

/// The constants of a kernel's knobs at their defaults, for back ends without those knobs
/// (gpu-compute sets them from its own).
fn default_consts(kernel: &str) -> &'static str {
    match kernel {
        "incremental" => "const EXACT: bool = true;\nconst LEVEL_MASK: u32 = S_LEVELS | 1u | (1u << S_TOP);\n",
        _ => "",
    }
}

/// Rays through a composed storage, for back ends whose primary visibility is not a ray
/// walk (raster, splat; discovery's rays use it too) and for passes outside the main
/// shader (meshes): the storage (`GpuStorage::source`, bind group 2) walked with `kernel`
/// (refloor walks LOD chunks with `lod`) as `ctrace`, plus `occluded()`.
pub fn occlusion_wgsl(storage: &crate::storage::GpuStorage, kernel: &str, lod: bool) -> String {
    let walk = gpu_wgsl(kernel).unwrap_or_else(|| panic!("no GPU kernel `{kernel}`"));
    format!("const LOD: bool = {lod};\n{}{}\n{walk}\n{OCCLUDED_WGSL}", default_consts(kernel), storage.source())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A primary walk plus a namespaced secondary walk form one valid module for every
    /// storage and kernel pair (the shader as gpu-compute assembles it, minus its own code).
    #[test]
    fn secondary_walks_compile_next_to_the_primary() {
        let storages = ["bricks", "hier", "prefix", "distance", "rows", "tree64", "svo_dag", "clipmap"];
        let kernels = ["refloor", "incremental", "sphere", "rowrun"];
        for (primary, secondary) in [("prefix", "distance"), ("bricks", "tree64"), ("clipmap", "svo_dag")] {
            for k in kernels {
                let st = crate::storage::gpu(primary).unwrap();
                let own = format!("const LOD: bool = false;\nconst EXACT: bool = true;\nconst LEVEL_MASK: u32 = S_LEVELS | 1u;\n{}\n{}", st.source(), gpu_wgsl("refloor").unwrap());
                let sec = secondary_wgsl(&crate::storage::gpu(secondary).unwrap(), k, false, 4);
                let src = mr_gpu::voxels::shader_source(&sec, &own);
                let module = wgpu::naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{primary} + refloor, shadows {secondary} + {k}: {}", e.emit_to_string(&src)));
                use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
                if let Err(e) = Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module) {
                    panic!("{primary} + refloor, shadows {secondary} + {k}: {}", e.emit_to_string(&src));
                }
            }
        }
        assert_eq!(storages.len(), 8);
        // Member accesses keep their names; declared names get the suffix.
        let n = namespaced("// a comment ending in a period.\nconst LOD: bool = true;\nfn f(h: Hit) -> u32 { if (LOD) { return h.lod; } return 0u; }\n", "_s");
        assert_eq!(n, "// a comment ending in a period.\nconst LOD_s: bool = true;\nfn f_s(h: Hit) -> u32 { if (LOD_s) { return h.lod; } return 0u; }\n");
    }
}
