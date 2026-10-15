//! The per-pixel state a GPU shading component keeps across frames (`pathtrace`'s
//! progressive accumulation), owned by the back end but declared, bound and advanced the
//! same way on every back end: the back end reserves two bindings in one of its groups,
//! puts [`ShadeState::wgsl`] into its shader and [`ShadeState::layout_entries`] /
//! [`ShadeState::bind_entries`] into its layouts, and calls [`ShadeState::begin`] once per
//! frame. Shading components without state (`direct`) leave the buffers as small
//! placeholders and cost nothing per frame.

use bytemuck::{Pod, Zeroable};
use mr_core::{Camera, ParamSpec, ParamValue, RenderMode};
use mr_gpu::Gpu;

/// What `pathtrace` reads (`sp` in WGSL).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Params {
    width: u32,
    frame: u32,
    /// Samples accumulated so far (0: start over).
    prev_count: u32,
    bounces: u32,
    spp: u32,
    _pad: [u32; 3],
}

/// The shading knobs that configure the state (owned by the `pathtrace` component).
pub fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::int("bounces", "pathtrace shading: diffuse indirect bounces (lit)", 0, 8, 1, true).of("shading", &["pathtrace"]),
        ParamSpec::int("spp", "pathtrace shading: paths per pixel per frame (lit)", 1, 64, 1, true).of("shading", &["pathtrace"]),
        ParamSpec::bool("accumulate", "pathtrace shading: average frames while camera and world are still", true, true).of("shading", &["pathtrace"]),
    ]
}

/// Whether shading component `name` keeps per-pixel state.
pub fn stateful(name: &str) -> bool {
    name == "pathtrace"
}

/// The device feature stateful shading needs when it runs in a fragment shader.
pub const FRAGMENT_FEATURE: wgpu::Features = wgpu::Features::SHADER_EARLY_DEPTH_TEST;

/// Fragment shading with `shading`: stateful shading writes per-pixel state, so only the
/// fragment that survives the depth test may run. Its fragment entry point `fs` gets
/// `@early_depth_test(force)` ([`FRAGMENT_FEATURE`]), and lit passes need a depth prepass
/// with an `Equal` test so that exactly one fragment per pixel shades. Stateless shading
/// keeps the source as it is.
pub fn fragment_wgsl(shading: &str, src: &str) -> String {
    if !stateful(shading) {
        return src.to_string();
    }
    assert!(src.contains("@fragment\nfn fs("), "fragment entry point `fs` not found");
    src.replace("@fragment\nfn fs(", "@fragment @early_depth_test(force)\nfn fs(")
}

/// Why `shading` cannot shade fragments on `device` (the feature is missing), if it can't.
pub fn fragment_blocker(shading: &str, device: &wgpu::Device) -> Option<String> {
    (stateful(shading) && !device.features().contains(FRAGMENT_FEATURE))
        .then(|| format!("`{shading}` shading in a fragment shader needs forced early depth tests (wgpu SHADER_EARLY_DEPTH_TEST), which this device lacks"))
}

pub struct ShadeState {
    uniform: wgpu::Buffer,
    accum: wgpu::Buffer,
    shading: String,
    width: u32,
    height: u32,
    count: u32,
    frame: u32,
    last: Option<(Camera, RenderMode)>,
    pub accumulate: bool,
    pub bounces: u32,
    pub spp: u32,
}

impl ShadeState {
    pub fn new(gpu: &Gpu, width: u32, height: u32, shading: &str, bounces: u32, spp: u32, accumulate: bool) -> Self {
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shade state"),
            size: size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let accum = Self::accum_buffer(gpu, width, height, shading);
        Self { uniform, accum, shading: shading.into(), width, height, count: 0, frame: 0, last: None, accumulate, bounces: bounces.min(8), spp: spp.clamp(1, 64) }
    }

    /// Full size only for shading that accumulates (a placeholder otherwise).
    fn accum_buffer(gpu: &Gpu, width: u32, height: u32, shading: &str) -> wgpu::Buffer {
        let size = if stateful(shading) { width as u64 * height as u64 * 16 } else { 16 };
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("accum"), size: size.max(16), usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
    }

    /// WGSL declarations at `group`, bindings `binding` and `binding + 1`.
    pub fn wgsl(group: u32, binding: u32) -> String {
        format!(
            "struct ShadeParams {{ width: u32, frame: u32, prev_count: u32, bounces: u32, spp: u32, _pa: u32, _pb: u32, _pc: u32 }}\n\
             @group({group}) @binding({binding}) var<uniform> sp: ShadeParams;\n\
             @group({group}) @binding({}) var<storage, read_write> accum: array<vec4<f32>>;\n",
            binding + 1
        )
    }

    pub fn layout_entries(binding: u32, visibility: wgpu::ShaderStages) -> [wgpu::BindGroupLayoutEntry; 2] {
        let buf = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }, count: None };
        [buf(binding, wgpu::BufferBindingType::Uniform), buf(binding + 1, wgpu::BufferBindingType::Storage { read_only: false })]
    }

    pub fn bind_entries(&self, binding: u32) -> [wgpu::BindGroupEntry<'_>; 2] {
        [
            wgpu::BindGroupEntry { binding, resource: self.uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: binding + 1, resource: self.accum.as_entire_binding() },
        ]
    }

    /// After a shading change; `true` if the buffers changed (rebuild the bind group).
    pub fn set_shading(&mut self, gpu: &Gpu, shading: &str) -> bool {
        if shading == self.shading {
            return false;
        }
        let realloc = stateful(shading) != stateful(&self.shading);
        self.shading = shading.into();
        self.count = 0;
        if realloc {
            self.accum = Self::accum_buffer(gpu, self.width, self.height, shading);
        }
        realloc
    }

    /// After a resize: the buffers change (rebuild the bind group).
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        (self.width, self.height, self.count) = (width, height, 0);
        self.accum = Self::accum_buffer(gpu, width, height, &self.shading);
    }

    /// The world changed: start accumulating again.
    pub fn invalidate(&mut self) {
        self.count = 0;
    }

    /// Advances one frame (restarting the average when the view or the mode changed) and
    /// uploads the parameters. Nothing for stateless shading.
    pub fn begin(&mut self, gpu: &Gpu, camera: &Camera, mode: RenderMode) {
        if !stateful(&self.shading) {
            return;
        }
        if !self.accumulate || self.last != Some((*camera, mode)) {
            self.count = 0;
        }
        self.last = Some((*camera, mode));
        self.frame = self.frame.wrapping_add(1);
        let p = Params { width: self.width, frame: self.frame, prev_count: self.count, bounces: self.bounces, spp: self.spp, _pad: [0; 3] };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&p));
        self.count = self.count.saturating_add(self.spp);
    }

    /// `accumulated_spp` after `begin`, for stateful shading in lit mode.
    pub fn counter(&self, mode: RenderMode) -> Option<(String, f64)> {
        (stateful(&self.shading) && mode == RenderMode::Lit).then(|| ("accumulated_spp".to_string(), self.count as f64))
    }

    /// Handles the knobs of [`params`]; `None` if `name` is not one of them.
    pub fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<()> {
        match name {
            "bounces" => (self.bounces, self.count) = (value.as_i64().unwrap_or(1).clamp(0, 8) as u32, 0),
            "spp" => (self.spp, self.count) = (value.as_i64().unwrap_or(1).clamp(1, 64) as u32, 0),
            "accumulate" => self.accumulate = value.as_bool().unwrap_or(true),
            _ => return None,
        }
        Some(())
    }

    pub fn memory_bytes(&self) -> u64 {
        self.accum.size() + self.uniform.size()
    }
}
