//! Shader programs assembled from modules (PLAN step 9b) through an existing module system,
//! `naga_oil` (the composer Bevy uses; built on the same naga as wgpu). Each piece of a
//! pipeline (the shared world code, every storage and kernel instance, the ray services,
//! lighting, shading) is its own module with its own path, so two instances of one storage
//! or kernel sit side by side without renaming: the module system keeps their names apart.
//!
//! Sources stay plain WGSL. A module lists the modules it depends on; its imports are
//! generated from what it uses: every name it uses but does not declare, taken from the
//! nearest dependency that declares it (its own dependencies first, in order, then theirs,
//! breadth first). So the primary rays' `ctrace` and the shadow rays' `ctrace` never meet:
//! each caller reaches only the kernel it is bound to. Per-instance constants (bind group,
//! LOD, kernel knobs) go in the instance's own source. Choices compiled into one body (a
//! variant of the root without a call boundary, e.g. gpu-voxelspace's shading placement)
//! are the module system's shader defs (`#ifdef NAME`, [`Program::define`]).

use std::collections::{HashMap, HashSet, VecDeque};

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue, ShaderLanguage, ShaderType};

use wgpu::naga;

/// One module: a path (`mr::voxels`, `n3`, ...), its WGSL, the paths it depends on.
#[derive(Clone, Debug)]
struct Module {
    path: String,
    source: String,
    deps: Vec<String>,
}

/// A set of modules a root shader is compiled against.
#[derive(Clone, Debug, Default)]
pub struct Program {
    modules: Vec<Module>,
    /// Shader defs set for every module and the root (`#ifdef NAME`).
    defs: Vec<String>,
}

/// The shared modules: `mr::common` (colour, camera, face helpers), `mr::voxels` (the
/// voxel world at group 1, `Hit`, `Surface`, ...).
pub const COMMON: &str = "mr::common";
pub const VOXELS: &str = "mr::voxels";
/// The reference lighting model (`shade_lit`), its shadow rays through `RAYS`.
pub const LIGHTING: &str = "mr::lighting";

impl Program {
    pub fn new() -> Self {
        Self::default()
    }

    /// The shared world modules ([`COMMON`], [`VOXELS`]) and, when shadow rays are bound
    /// (`occluded` from the module at `rays`), [`LIGHTING`].
    pub fn world(rays: Option<&str>) -> Self {
        let mut p = Self::new();
        p.add(COMMON, crate::COMMON_WGSL, &[]);
        p.add(VOXELS, crate::voxels::VOXELS_WGSL, &[COMMON]);
        if let Some(r) = rays {
            p.add(LIGHTING, crate::voxels::LIGHTING_WGSL, &[r, VOXELS]);
        }
        p
    }

    /// Adds (or replaces) the module at `path`.
    pub fn add(&mut self, path: &str, source: impl Into<String>, deps: &[&str]) -> &mut Self {
        let m = Module { path: path.into(), source: source.into(), deps: deps.iter().map(|d| d.to_string()).collect() };
        match self.modules.iter_mut().find(|x| x.path == path) {
            Some(x) => *x = m,
            None => self.modules.push(m),
        }
        self
    }

    /// Sets shader def `name` (true) for the modules and the root.
    pub fn define(&mut self, name: &str) -> &mut Self {
        if !self.defs.iter().any(|d| d == name) {
            self.defs.push(name.into());
        }
        self
    }

    fn shader_defs(&self) -> HashMap<String, ShaderDefValue> {
        self.defs.iter().map(|d| (d.clone(), ShaderDefValue::Bool(true))).collect()
    }

    pub fn has(&self, path: &str) -> bool {
        self.modules.iter().any(|m| m.path == path)
    }

    fn get(&self, path: &str) -> Result<&Module, String> {
        self.modules.iter().find(|m| m.path == path).ok_or_else(|| format!("no shader module `{path}` (have: {})", self.modules.iter().map(|m| m.path.as_str()).collect::<Vec<_>>().join(", ")))
    }

    /// The `#import` lines for a module with `source` and dependencies `deps`.
    fn imports(&self, source: &str, deps: &[String]) -> Result<String, String> {
        let own: HashSet<String> = declared(source).into_iter().collect();
        let mut wanted: HashSet<String> = used(source).into_iter().filter(|n| !own.contains(n)).collect();
        let mut by_module: Vec<(String, Vec<String>)> = Vec::new();
        let mut seen = HashSet::new();
        let mut queue: VecDeque<String> = deps.iter().cloned().collect();
        while let Some(path) = queue.pop_front() {
            if !seen.insert(path.clone()) {
                continue;
            }
            let m = self.get(&path)?;
            let mut names: Vec<String> = declared(&m.source).into_iter().filter(|n| wanted.remove(n)).collect();
            names.sort();
            if !names.is_empty() {
                by_module.push((path, names));
            }
            queue.extend(m.deps.iter().cloned());
        }
        Ok(by_module.iter().map(|(p, names)| format!("#import {p}::{{{}}}\n", names.join(", "))).collect())
    }

    /// The modules `root` reaches through `deps`, dependencies first.
    fn order(&self, deps: &[String]) -> Result<Vec<&Module>, String> {
        fn visit<'a>(p: &'a Program, path: &str, done: &mut HashMap<String, bool>, out: &mut Vec<&'a Module>) -> Result<(), String> {
            match done.get(path) {
                Some(true) => return Ok(()),
                Some(false) => return Err(format!("shader modules depend on each other in a cycle through `{path}`")),
                None => {}
            }
            done.insert(path.to_string(), false);
            let m = p.get(path)?;
            for d in &m.deps {
                visit(p, d, done, out)?;
            }
            done.insert(path.to_string(), true);
            out.push(m);
            Ok(())
        }
        let (mut done, mut out) = (HashMap::new(), Vec::new());
        for d in deps {
            visit(self, d, &mut done, &mut out)?;
        }
        Ok(out)
    }

    /// The text of every module `root` reaches (with its generated imports), then the root:
    /// what is compiled, for dumps and error messages.
    pub fn text(&self, root: &str, deps: &[&str]) -> Result<String, String> {
        let deps: Vec<String> = deps.iter().map(|d| d.to_string()).collect();
        let mut s = if self.defs.is_empty() { String::new() } else { format!("// ---- shader defs: {}\n", self.defs.join(", ")) };
        for m in self.order(&deps)? {
            s += &format!("// ---- module {}\n#define_import_path {}\n{}{}\n", m.path, m.path, self.imports(&m.source, &m.deps)?, m.source);
        }
        s += &format!("// ---- root\n{}", with_imports(root, &self.imports(root, &deps)?));
        Ok(s)
    }

    /// Composes `root` (which may use every name its `deps` reach) into one naga module.
    pub fn naga(&self, root: &str, deps: &[&str]) -> Result<naga::Module, String> {
        let deps: Vec<String> = deps.iter().map(|d| d.to_string()).collect();
        let mut c = Composer::default().with_capabilities(naga::valid::Capabilities::all());
        for m in self.order(&deps)? {
            let src = format!("#define_import_path {}\n{}{}", m.path, self.imports(&m.source, &m.deps)?, m.source);
            if let Err(e) = c.add_composable_module(ComposableModuleDescriptor { source: &src, file_path: &m.path, language: ShaderLanguage::Wgsl, shader_defs: self.shader_defs(), ..Default::default() }) {
                return Err(format!("shader module `{}`: {}", m.path, e.emit_to_string(&c)));
            }
        }
        let src = with_imports(root, &self.imports(root, &deps)?);
        c.make_naga_module(NagaModuleDescriptor { source: &src, file_path: "root", shader_type: ShaderType::Wgsl, shader_defs: self.shader_defs(), ..Default::default() }).map_err(|e| e.emit_to_string(&c))
    }
}

/// Composes and creates a shader module. With `MR_DUMP_WGSL=dir`, the program's text
/// ([`Program::text`]) is written to `dir/<label>-<hash>.wgsl` (zero-cost checks: a refactor
/// that must not change what runs leaves the set of files identical).
pub fn compile(dev: &wgpu::Device, label: &str, p: &Program, root: &str, deps: &[&str]) -> wgpu::ShaderModule {
    if let Some(dir) = std::env::var_os("MR_DUMP_WGSL") {
        let text = p.text(root, deps).unwrap_or_else(|e| e);
        crate::dump_wgsl(&dir, label, &text);
    }
    let module = p.naga(root, deps).unwrap_or_else(|e| panic!("shader `{label}`: {e}"));
    dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)) })
}

/// `src` with `imports` after its directives (`enable ...;` must come first).
fn with_imports(src: &str, imports: &str) -> String {
    let (directives, rest): (Vec<&str>, Vec<&str>) = src.lines().partition(|l| l.starts_with("enable ") || l.starts_with("requires "));
    format!("{}{imports}{}\n", directives.iter().map(|l| format!("{l}\n")).collect::<String>(), rest.join("\n"))
}

fn ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The names `src` declares at the top level (functions, virtual functions, constants,
/// variables, structs, aliases, pipeline overrides: declarations start a line).
pub fn declared(src: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in src.lines().filter(|l| !l.starts_with(char::is_whitespace)) {
        // Skip attributes (`@group(2) @binding(0)`, `@compute @workgroup_size(8, 8)`).
        let mut rest = line;
        while let Some(r) = rest.strip_prefix('@') {
            let mut depth = 0;
            let end = r.char_indices().find(|&(_, c)| {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0 && c.is_whitespace()
            });
            rest = end.map_or("", |(i, _)| r[i..].trim_start());
        }
        // `override fn a::f` replaces another module's virtual function: no name of its own.
        if rest.starts_with("override fn ") {
            continue;
        }
        let Some(kw) = ["fn ", "virtual fn ", "const ", "struct ", "alias ", "override ", "var"].into_iter().find(|k| rest.starts_with(k)) else { continue };
        let mut after = &rest[kw.len()..];
        if kw == "var" {
            if let Some(r) = after.strip_prefix('<') {
                after = &r[r.find('>').map_or(r.len(), |i| i + 1)..];
            }
        }
        let name: String = after.trim_start().chars().take_while(|&c| ident(c)).collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

/// The identifiers `src` uses outside comments (member accesses `.name` excluded).
fn used(src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for line in src.lines() {
        let code = line.split("//").next().unwrap();
        let mut word = String::new();
        let mut prev = ' ';
        for c in code.chars().chain([' ']) {
            if ident(c) && !(word.is_empty() && c.is_ascii_digit()) {
                word.push(c);
            } else {
                if !word.is_empty() {
                    if prev != '.' {
                        out.insert(std::mem::take(&mut word));
                    }
                    word.clear();
                    prev = 'a';
                }
                if !c.is_whitespace() {
                    prev = c;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two instances of one storage and kernel (different bind groups and constants) sit in
    /// one shader without renaming; each caller reaches the kernel it is bound to.
    #[test]
    fn instances_side_by_side() {
        let storage = "@group(S_GROUP) @binding(0) var<storage, read> sdata: array<u32>;\nfn s_probe(i: u32) -> u32 { return sdata[i]; }\n";
        let kernel = "fn ctrace(i: u32) -> Hit { var h: Hit; ct_iters += 1u; h.hit = s_probe(i) != 0u && LOD; return h; }\n";
        let mut p = Program::world(Some("shadow"));
        p.add("s1", format!("const S_GROUP: u32 = 2u;\n{storage}"), &[VOXELS]);
        p.add("s2", format!("const S_GROUP: u32 = 4u;\n{storage}"), &[VOXELS]);
        p.add("k1", format!("const LOD: bool = true;\n{kernel}"), &["s1", VOXELS]);
        p.add("k2", format!("const LOD: bool = false;\n{kernel}"), &["s2", VOXELS]);
        p.add("shadow", "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(0u).hit; }\n", &["k2"]);
        let root = "@group(0) @binding(0) var<storage, read_write> out: array<u32>;\n\
                    @compute @workgroup_size(64) fn main(@builtin(local_invocation_index) i: u32) {\n\
                    out[i] = select(0u, 1u, ctrace(i).hit) + u32(shade_lit(vec4<f32>(1.0), vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0)).x) + ct_iters;\n}\n";
        let m = p.naga(root, &["k1", LIGHTING]).unwrap_or_else(|e| panic!("{e}"));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&m).unwrap();
        let text = p.text(root, &["k1", LIGHTING]).unwrap();
        assert!(text.contains("#import k1::{ctrace}") && text.contains("#import k2::{ctrace}"), "{text}");
        assert!(text.contains("#import mr::voxels::{Hit, ct_iters}"), "{text}");
        // Both storages' bindings are in the module, at their own groups.
        let groups: HashSet<u32> = m.global_variables.iter().filter_map(|(_, v)| v.binding.as_ref().map(|b| b.group)).collect();
        assert!(groups.contains(&2) && groups.contains(&4), "{groups:?}");
    }

    #[test]
    fn names() {
        let src = "@group(2) @binding(0) var<storage, read> aux: array<u32>;\nconst A: u32 = 1u;\nstruct S { y: u32 }\nfn f(s: S) -> u32 { return s.x + A; } // B\n@compute @workgroup_size(8, 8) fn main() {}\nvirtual fn g() {}\noverride fn m::g() {}\n";
        assert_eq!(declared(src), ["aux", "A", "S", "f", "main", "g"]);
        let u = used(src);
        // Member accesses (`s.x`) and comments (`B`) are not uses.
        assert!(u.contains("A") && u.contains("S") && !u.contains("x") && !u.contains("B"), "{u:?}");
    }
}
