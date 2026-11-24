//! Diagnostic views: the post node's `view` shows one per-pixel quantity a node of the graph
//! writes ([`Quantity`]) instead of the image. While a view is chosen the core keeps a buffer
//! of one `u32` per pixel in group 0 at [`QUANTITY_BINDING`]; the writer's program, compiled
//! with it, fills it as it casts (and still shades); after the frame's passes the view pass
//! maps it through the shared colour ramp into the output. Unviewed, nothing is compiled or
//! bound.

use mr_core::{Legend, ParamSpec, Quantity};
use mr_gpu::Gpu;

/// The quantity buffer's binding in group 0 (past every other's).
pub(crate) const QUANTITY_BINDING: u32 = 31;

/// The quantity buffer's declaration in a writer's program.
pub(crate) fn wgsl() -> String {
    format!("@group(0) @binding({QUANTITY_BINDING}) var<storage, read_write> quantity: array<u32>;\n")
}

/// The post node's knobs over the quantities `qs` the graph's nodes write: `view` (the image
/// or one of them) and `view_max` (the scale's top).
pub(crate) fn knobs(qs: &[Quantity]) -> [ParamSpec; 2] {
    let mut names = vec!["image"];
    names.extend(qs.iter().map(|q| q.name));
    let help: Vec<String> = std::iter::once("image: the rendered scene".to_string()).chain(qs.iter().map(|q| format!("{}: {}", q.name, q.help))).collect();
    let tops: Vec<&str> = qs.iter().map(|q| q.name).collect();
    let posts = mr_components::graph::POSTS;
    [
        ParamSpec::choice("view", &format!("{}. Drawn on the shared colour scale (white above view_max); the frame is still shaded", help.join(". ")), &names, "image", true).of("post", posts),
        ParamSpec::int("view_max", &format!("{} view: the top of the colour scale", tops.join(" / ")), 1, 16384, 256, true).of("post", posts),
    ]
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct PostKnobs {
    view: Option<String>,
    view_max: Option<u32>,
}

/// The view and its scale's top at creation: the post node's knobs (where a plan's or the
/// editor's land) over the options' `view` / `max`.
pub(crate) fn chosen(g: &mr_components::graph::Checked, view: &str, max: u32) -> (String, u32) {
    let post = (g.node(&g.output).role == mr_components::graph::Role::Post).then(|| g.knobs::<PostKnobs>(&g.output).ok()).flatten().unwrap_or_default();
    (post.view.unwrap_or_else(|| view.into()), post.view_max.unwrap_or(max))
}

/// The view chosen: its quantity, the buffer its writer fills, the mapping pass.
pub(crate) struct Viewing {
    pub quantity: Quantity,
    buffer: wgpu::Buffer,
    params: wgpu::Buffer,
    pipeline: Option<(wgpu::BindGroupLayout, wgpu::ComputePipeline)>,
}

impl Viewing {
    pub fn new(gpu: &Gpu, quantity: Quantity, width: u32, height: u32) -> Self {
        let params = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("view params"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        Self { quantity, buffer: Self::buffer(gpu, width, height), params, pipeline: None }
    }

    fn buffer(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        let size = (width as u64 * height as u64 * 4).max(16);
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("quantity"), size, usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        self.buffer = Self::buffer(gpu, width, height);
    }

    pub fn layout_entry(&self, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
        crate::storage_entry(QUANTITY_BINDING, false, visibility)
    }

    pub fn bind_entry(&self) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry { binding: QUANTITY_BINDING, resource: self.buffer.as_entire_binding() }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.buffer.size()
    }

    pub fn legend(&self, max: u32) -> Legend {
        self.quantity.legend(max as f64)
    }

    /// The mapping pass into `output` (format `format`), after the frame's passes.
    pub fn encode(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, output: &wgpu::Texture, format: wgpu::TextureFormat, max: u32) {
        let (w, h) = (output.width(), output.height());
        let flags = self.quantity.float as u32 | (self.quantity.log as u32) << 1;
        gpu.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&[w, h, (max as f32).to_bits(), flags]));
        let (layout, pipeline) = self.pipeline.get_or_insert_with(|| pipeline(gpu, format));
        let view = output.create_view(&Default::default());
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("view"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: self.params.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("view"), timestamp_writes: None });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
    }
}

fn pipeline(gpu: &Gpu, format: wgpu::TextureFormat) -> (wgpu::BindGroupLayout, wgpu::ComputePipeline) {
    let dev = &gpu.device;
    let c = wgpu::ShaderStages::COMPUTE;
    let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("view"),
        entries: &[
            crate::storage_entry(0, true, c),
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: c, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format, view_dimension: wgpu::TextureViewDimension::D2 }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 2, visibility: c, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
        ],
    });
    let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("view"), source: wgpu::ShaderSource::Wgsl(shader(format).into()) });
    let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("view"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let pipeline = dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("view"), layout: Some(&pl), module: &module, entry_point: Some("main"), compilation_options: Default::default(), cache: gpu.cache() });
    (layout, pipeline)
}

/// The mapping pass's shader for an output of `format`.
fn shader(format: wgpu::TextureFormat) -> String {
    let fmt = if format == wgpu::TextureFormat::Rgba16Float { "rgba16float" } else { "rgba8unorm" };
    format!(
        "@group(0) @binding(0) var<storage, read> quantity: array<u32>;\n\
         @group(0) @binding(1) var out_tex: texture_storage_2d<{fmt}, write>;\n\
         struct Params {{ width: u32, height: u32, max_v: f32, flags: u32 }}\n\
         @group(0) @binding(2) var<uniform> p: Params;\n\
         {}\n\
         @compute @workgroup_size(8, 8)\n\
         fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n\
         \x20   if (gid.x >= p.width || gid.y >= p.height) {{ return; }}\n\
         \x20   let q = quantity[gid.y * p.width + gid.x];\n\
         \x20   let v = select(f32(q), bitcast<f32>(q), (p.flags & 1u) != 0u);\n\
         \x20   textureStore(out_tex, vec2<i32>(gid.xy), ramp_value(v, p.max_v, (p.flags & 2u) != 0u));\n\
         }}\n",
        mr_core::legend::ramp_wgsl()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn shader_parses() {
        for f in [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Rgba16Float] {
            use wgpu::naga::{front::wgsl, valid};
            let m = wgsl::parse_str(&super::shader(f)).unwrap_or_else(|e| panic!("{e}"));
            valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::all()).validate(&m).unwrap();
        }
    }
}
