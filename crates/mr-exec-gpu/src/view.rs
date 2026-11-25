//! Diagnostic views: the post node's `view` shows one per-pixel quantity a node of the graph
//! writes ([`Quantity`]) instead of the image. While a view is chosen the core keeps a buffer
//! of one `u32` per pixel in group 0 at [`QUANTITY_BINDING`], cleared before each frame; the
//! writer's program, compiled with it, fills it as it casts (and still shades), or the
//! region's own view passes do (`Region::view`); after them the view pass maps it into the
//! output: a ramp's value through the shared colour ramp, a category as a tint of the greyed
//! image. Unviewed, nothing is compiled or bound.

use mr_core::{Legend, Quantity};
use mr_gpu::Gpu;
use mr_gpu::Pipelines as _;

/// The quantity buffer's binding in group 0 (past every other's).
pub(crate) const QUANTITY_BINDING: u32 = 31;

/// The quantity buffer's declaration in a writer's program.
pub(crate) fn wgsl() -> String {
    format!(
        "@group(0) @binding({QUANTITY_BINDING}) var<storage, read_write> quantity: array<u32>;\n"
    )
}

/// The declaration for a writer that counts (`atomicAdd`).
pub(crate) fn atomic_wgsl() -> String {
    format!(
        "@group(0) @binding({QUANTITY_BINDING}) var<storage, read_write> quantity: array<atomic<u32>>;\n"
    )
}

/// The view chosen: its quantity, the buffer its writer fills, the mapping pass (a category
/// view's also its colours and a copy of the image it tints).
pub(crate) struct Viewing {
    pub quantity: Quantity,
    buffer: wgpu::Buffer,
    params: wgpu::Buffer,
    palette: Option<wgpu::Buffer>,
    under: Option<wgpu::Texture>,
    pipeline: Option<(wgpu::BindGroupLayout, wgpu::ComputePipeline)>,
}

impl Viewing {
    pub fn new(gpu: &Gpu, quantity: Quantity, width: u32, height: u32) -> Self {
        let params = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("view params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Per category: its colour, alpha 0 for one colour per id.
        let palette = quantity.is_categorical().then(|| {
            let colors: Vec<[f32; 4]> = quantity
                .categories
                .iter()
                .map(|(_, c)| {
                    c.map_or([0.0; 4], |c| {
                        [
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            1.0,
                        ]
                    })
                })
                .collect();
            let b = gpu.storage(
                "view palette",
                colors.len() as u64 * 16,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                || "the view's palette".into(),
            );
            gpu.queue.write_buffer(&b, 0, bytemuck::cast_slice(&colors));
            b
        });
        Self {
            quantity,
            buffer: Self::buffer(gpu, width, height),
            params,
            palette,
            under: None,
            pipeline: None,
        }
    }

    fn buffer(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        let size = (width as u64 * height as u64 * 4).max(16);
        gpu.storage(
            "quantity",
            size,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            || format!("the resolution ({width}x{height})"),
        )
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        self.buffer = Self::buffer(gpu, width, height);
        self.under = None;
    }

    pub fn layout_entry(&self, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
        crate::storage_entry(QUANTITY_BINDING, false, visibility)
    }

    pub fn bind_entry(&self) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding: QUANTITY_BINDING,
            resource: self.buffer.as_entire_binding(),
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.buffer.size()
            + self.under.as_ref().map_or(0, |t| {
                t.width() as u64
                    * t.height() as u64
                    * t.format().block_copy_size(None).unwrap_or(4) as u64
            })
    }

    pub fn legend(&self, max: u32) -> Legend {
        self.quantity.legend(max as f64)
    }

    /// Before the frame's passes: every pixel 0 (counts start there; categories: the image).
    pub fn clear(&self, enc: &mut wgpu::CommandEncoder) {
        enc.clear_buffer(&self.buffer, 0, None);
    }

    /// The mapping pass into `output` (format `format`), after the frame's passes.
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        output: &wgpu::Texture,
        format: wgpu::TextureFormat,
        max: u32,
    ) {
        let (w, h) = (output.width(), output.height());
        let flags = self.quantity.float as u32 | (self.quantity.log as u32) << 1;
        gpu.queue.write_buffer(
            &self.params,
            0,
            bytemuck::cast_slice(&[w, h, (max as f32).to_bits(), flags]),
        );
        let categorical = self.quantity.is_categorical();
        let (layout, pipeline) = self
            .pipeline
            .get_or_insert_with(|| pipeline(gpu, format, categorical));
        let view = output.create_view(&Default::default());
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: self.params.as_entire_binding(),
            },
        ];
        // Categories tint the image: a copy of it to read.
        let under_view;
        if let Some(palette) = &self.palette {
            let under = self.under.get_or_insert_with(|| {
                gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("view image"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            });
            enc.copy_texture_to_texture(
                output.as_image_copy(),
                under.as_image_copy(),
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            under_view = under.create_view(&Default::default());
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&under_view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 4,
                resource: palette.as_entire_binding(),
            });
        }
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("view"),
            layout,
            entries: &entries,
        });
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("view"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
    }
}

fn pipeline(
    gpu: &Gpu,
    format: wgpu::TextureFormat,
    categorical: bool,
) -> (wgpu::BindGroupLayout, wgpu::ComputePipeline) {
    let dev = &gpu.device;
    let c = wgpu::ShaderStages::COMPUTE;
    let mut entries = vec![
        crate::storage_entry(0, true, c),
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: c,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format,
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: c,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ];
    if categorical {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: c,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(crate::storage_entry(4, true, c));
    }
    let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("view"),
        entries: &entries,
    });
    let module = mr_gpu::shader(dev, "view", shader(format, categorical));
    let pl = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("view"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = dev.compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("view"),
        layout: Some(&pl),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: gpu.cache(),
    });
    (layout, pipeline)
}

/// The mapping pass's shader for an output of `format`, of a category view's quantity when
/// `categorical`.
fn shader(format: wgpu::TextureFormat, categorical: bool) -> String {
    let fmt = if format == wgpu::TextureFormat::Rgba16Float {
        "rgba16float"
    } else {
        "rgba8unorm"
    };
    let head = format!(
        "@group(0) @binding(0) var<storage, read> quantity: array<u32>;\n\
         @group(0) @binding(1) var out_tex: texture_storage_2d<{fmt}, write>;\n\
         struct Params {{ width: u32, height: u32, max_v: f32, flags: u32 }}\n\
         @group(0) @binding(2) var<uniform> p: Params;\n"
    );
    if categorical {
        // Category k + 1 in the low byte (0: the image), an id above it; a category without
        // a colour is coloured by a hash of the id.
        return format!(
            "{head}\
             @group(0) @binding(3) var under: texture_2d<f32>;\n\
             @group(0) @binding(4) var<storage, read> palette: array<vec4<f32>>;\n\
             @compute @workgroup_size(8, 8)\n\
             fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n\
             \x20   if (gid.x >= p.width || gid.y >= p.height) {{ return; }}\n\
             \x20   let q = quantity[gid.y * p.width + gid.x];\n\
             \x20   let image = textureLoad(under, vec2<i32>(gid.xy), 0);\n\
             \x20   if ((q & 255u) == 0u) {{ textureStore(out_tex, vec2<i32>(gid.xy), image); return; }}\n\
             \x20   let k = palette[min((q & 255u) - 1u, arrayLength(&palette) - 1u)];\n\
             \x20   var tint = k.rgb;\n\
             \x20   if (k.a == 0.0) {{\n\
             \x20       var h = (q >> 8u) * 0x9E3779B1u;\n\
             \x20       h ^= h >> 15u; h *= 0x85EBCA77u; h ^= h >> 13u;\n\
             \x20       tint = vec3<f32>(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) / 255.0;\n\
             \x20   }}\n\
             \x20   let g = clamp(dot(image.rgb, vec3<f32>(0.3, 0.5, 0.2)), 0.0, 1.0);\n\
             \x20   textureStore(out_tex, vec2<i32>(gid.xy), vec4<f32>(mix(vec3<f32>(g), tint, 0.6), 1.0));\n\
             }}\n"
        );
    }
    format!(
        "{head}\
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
        for f in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba16Float,
        ] {
            for categorical in [false, true] {
                use wgpu::naga::{front::wgsl, valid};
                let m = wgsl::parse_str(&super::shader(f, categorical))
                    .unwrap_or_else(|e| panic!("{e}"));
                valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::all())
                    .validate(&m)
                    .unwrap();
            }
        }
    }
}
