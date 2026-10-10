//! Storage `clipmap` (from gpu-clipmap): a camera-centred toroidal volume of 3D textures
//! (R8 or R16 block ids, R8 brick occupancy, R32 slot tags), traversed with `textureLoad`
//! only. Chunk `c` lives in slot `c mod slots`; vertical slots are capped (worlds are much
//! wider than tall). A *texture* storage: its own bindings 4-7 in group 2, no voxel pool.

use std::collections::HashMap;

use glam::IVec3;
use mr_core::ChunkData;
use mr_gpu::{Gpu, wgpu};

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// Vertical chunk slots are capped to bound memory.
const MAX_Y_SLOTS: i32 = 16;

fn tag(c: IVec3) -> u32 {
    let u = c.to_array().map(|v| v as u32 & 1023);
    u[0] | (u[1] << 10) | (u[2] << 20) | 0x8000_0000
}

/// Group 2 bindings 4 (uniform) and 5-7 (3D uint textures).
pub fn layout_entries(visibility: wgpu::ShaderStages) -> [wgpu::BindGroupLayoutEntry; 4] {
    let tex = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false },
        count: None,
    };
    [
        wgpu::BindGroupLayoutEntry { binding: 4, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
        tex(5),
        tex(6),
        tex(7),
    ]
}

pub struct Clipmap {
    slots: IVec3,
    wide: bool,
    uniform: wgpu::Buffer,
    vox: wgpu::TextureView,
    occ: wgpu::TextureView,
    tags: wgpu::TextureView,
    textures: [wgpu::Texture; 3],
    owner: HashMap<IVec3, IVec3>,
    /// Chunks that evicted another from their slot (view radius larger than the volume).
    pub dropped: u64,
    scratch: Vec<u8>,
}

impl Clipmap {
    /// `wide`: more than 256 block types (R16 ids).
    pub fn new(gpu: &Gpu, view_radius: u32, wide: bool) -> Self {
        let max = (gpu.device.limits().max_texture_dimension_3d / 32) as i32;
        let side = (2 * view_radius as i32 + 1).min(max).max(1);
        let slots = IVec3::new(side, side.min(MAX_Y_SLOTS), side);
        let tex = |label, n: i32, format| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: (slots.x * n) as u32, height: (slots.y * n) as u32, depth_or_array_layers: (slots.z * n) as u32 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        // Fresh textures are zero-initialised: every slot reads as empty.
        let textures = [
            tex("clipmap voxels", 32, if wide { wgpu::TextureFormat::R16Uint } else { wgpu::TextureFormat::R8Uint }),
            tex("clipmap bricks", 8, wgpu::TextureFormat::R8Uint),
            tex("clipmap tags", 1, wgpu::TextureFormat::R32Uint),
        ];
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("clipmap params"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        gpu.queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&[slots.x, slots.y, slots.z, 0]));
        let [vox, occ, tags] = [0, 1, 2].map(|i| textures[i].create_view(&Default::default()));
        Self { slots, wide, uniform, vox, occ, tags, textures, owner: HashMap::new(), dropped: 0, scratch: vec![] }
    }

    pub fn bind_entries(&self) -> [wgpu::BindGroupEntry<'_>; 4] {
        [
            wgpu::BindGroupEntry { binding: 4, resource: self.uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.vox) },
            wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.occ) },
            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.tags) },
        ]
    }

    pub fn memory_bytes(&self) -> u64 {
        let cells = (self.slots.x * self.slots.y * self.slots.z) as u64;
        cells * (32 * 32 * 32 * if self.wide { 2 } else { 1 } + 8 * 8 * 8 + 4) + 16
    }

    fn write(gpu: &Gpu, tex: &wgpu::Texture, origin: IVec3, n: u32, bpp: u32, data: &[u8]) {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: origin.x as u32, y: origin.y as u32, z: origin.z as u32 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * bpp), rows_per_image: Some(n) },
            wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
        );
    }

    /// Stores (or, for `None`, clears) LOD-0 chunk `pos` in its toroidal slot.
    pub fn store(&mut self, gpu: &Gpu, pos: IVec3, data: Option<&ChunkData>) {
        let [vox, occ_tex, tags] = &self.textures;
        let slot = pos.rem_euclid(self.slots);
        let Some(data) = data else {
            if self.owner.get(&slot) == Some(&pos) {
                self.owner.remove(&slot);
                Self::write(gpu, tags, slot, 1, 4, &0u32.to_le_bytes());
            }
            return;
        };
        if let Some(prev) = self.owner.insert(slot, pos)
            && prev != pos
        {
            self.dropped += 1;
        }
        let bpp = if self.wide { 2 } else { 1 };
        let buf = &mut self.scratch;
        buf.clear();
        buf.resize(32 * 32 * 32 * bpp, 0);
        let mut occ = [0u8; 512];
        // Texture order: x fastest, then y (rows), then z (layers).
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let b = match data {
                        ChunkData::Uniform(b) => b.0,
                        ChunkData::Dense(d) => d.get(IVec3::new(x, y, z)).0,
                    };
                    if b != 0 {
                        let i = (x | (y << 5) | (z << 10)) as usize;
                        if self.wide {
                            buf[i * 2..i * 2 + 2].copy_from_slice(&b.to_le_bytes());
                        } else {
                            buf[i] = b as u8;
                        }
                        occ[((x >> 2) | ((y >> 2) << 3) | ((z >> 2) << 6)) as usize] = 1;
                    }
                }
            }
        }
        Self::write(gpu, vox, slot * 32, 32, bpp as u32, buf);
        Self::write(gpu, occ_tex, slot * 8, 8, 1, &occ);
        Self::write(gpu, tags, slot, 1, 4, &tag(pos).to_le_bytes());
    }
}
