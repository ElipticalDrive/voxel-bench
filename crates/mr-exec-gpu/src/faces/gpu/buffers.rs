//! The GPU-side data: the uniforms' layouts and the per-resolution buffers.

use bytemuck::{Pod, Zeroable};
use mr_gpu::{Gpu, wgpu};

use super::MAX_TILE;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(super) struct Params {
    pub(super) forward: [f32; 3],
    pub(super) tan_half_fov: f32,
    pub(super) right: [f32; 3],
    pub(super) aspect: f32,
    pub(super) up: [f32; 3],
    pub(super) frame: u32,
    pub(super) origin: [f32; 3],
    pub(super) tile: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) off_x: u32,
    pub(super) off_y: u32,
    pub(super) lat_w: u32,
    pub(super) lat_h: u32,
    pub(super) cols: u32,
    pub(super) rows: u32,
    pub(super) keep: u32,
    pub(super) criterion: u32,
    pub(super) sky_tile: u32,
    pub(super) max_fp: u32,
    pub(super) table_mask: u32,
    pub(super) list_cap: u32,
    pub(super) chunk_mask: u32,
    pub(super) chunk_probe: u32,
    pub(super) queue_cap: u32,
    pub(super) parity: u32,
    pub(super) strict: u32,
    pub(super) still: u32,
    pub(super) closure_px: u32,
    pub(super) closure_box: i32,
    pub(super) block_faces: u32,
    pub(super) inline_fp: u32,
    pub(super) occluders: u32,
    pub(super) refresh: u32,
    pub(super) par_k: f32,
    pub(super) changed: u32,
    pub(super) full: u32,
    pub(super) r_max: u32,
    pub(super) need_off: u32,
    pub(super) par_thresh: f32,
    pub(super) prev_forward: [f32; 3],
    pub(super) prev_tan: f32,
    pub(super) prev_right: [f32; 3],
    pub(super) has_prev: u32,
    pub(super) prev_up: [f32; 3],
    pub(super) keep_out: u32,
    pub(super) morton: u32,
    pub(super) morton_side: u32,
    pub(super) _q: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(super) struct LevelUniform {
    pub(super) level: u32,
    pub(super) size: u32,
    pub(super) in_off: u32,
    pub(super) out_off: u32,
    pub(super) parity: u32,
    pub(super) cap: u32,
    pub(super) list_cap: u32,
    pub(super) _p: u32,
    pub(super) _pad: [u32; 56],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(super) struct ViewUniform {
    pub(super) frame: u32,
    pub(super) tile: u32,
    pub(super) cols: u32,
    pub(super) rows: u32,
    pub(super) need_off: u32,
    pub(super) lat_w: u32,
    pub(super) off_x: u32,
    pub(super) off_y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) _p: [u32; 2],
}

/// Per-resolution buffers.
pub(super) struct Sized {
    pub(super) lat_w: u32,
    pub(super) lat_h: u32,
    pub(super) cache_c: wgpu::Buffer,
    pub(super) cache_t: wgpu::Buffer,
    pub(super) claim: wgpu::Buffer,
    /// Per lattice point: the frame its ray was last cast (`rays` view).
    pub(super) cast_frame: wgpu::Buffer,
    pub(super) rayq: wgpu::Buffer,
    pub(super) tileq: wgpu::Buffer,
    /// Incremental mode: last frame's faces rasterised at the current camera (list index + 1).
    pub(super) idtex: wgpu::TextureView,
    /// Per level-0 tile: reveal radius + 1, then (from `need_off`) why it was discovered
    /// (0 = not), then (from `2 * need_off`) frames since the tile was last discovered, then
    /// (from `3 * need_off`) a hole seen after the last raster.
    pub(super) need: wgpu::Buffer,
    pub(super) need_off: u32,
    /// Incremental mode: per level-0 tile, 1 if an edited block or changed chunk covers it.
    pub(super) changed_tiles: wgpu::Buffer,
    /// Per pixel, 1 where the final raster was sky; double-buffered by frame parity.
    pub(super) sky: [wgpu::Buffer; 2],
}

pub(super) fn storage(gpu: &Gpu, label: &str, size: u64, extra: wgpu::BufferUsages) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
        mapped_at_creation: false,
    })
}

fn tiles_max(lat_w: u32, lat_h: u32) -> u32 {
    (lat_w / 2 + 1) * (lat_h / 2 + 1)
}

pub(super) fn make_sized(gpu: &Gpu, w: u32, h: u32) -> Sized {
    // Lattice big enough for any tile size and jitter offset.
    let lat_w = w + 2 * MAX_TILE + 1;
    let lat_h = h + 2 * MAX_TILE + 1;
    let n = lat_w as u64 * lat_h as u64;
    let none = wgpu::BufferUsages::empty();
    Sized {
        lat_w,
        lat_h,
        cache_c: storage(gpu, "ray cache cells", n * 16, none),
        cache_t: storage(gpu, "ray cache t", n * 4, none),
        claim: storage(gpu, "ray claims", n * 4, none),
        cast_frame: storage(gpu, "ray cast frames", n * 4, none),
        rayq: storage(gpu, "ray queue", n * 4, none),
        tileq: storage(gpu, "tile queues", 2 * n * 4, none),
        idtex: gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("face ids"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Uint,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default()),
        // Tiles are at least 2 px, so the lattice has at most (lat_w/2) x (lat_h/2) tiles.
        need: storage(gpu, "tile needs", 4 * tiles_max(lat_w, lat_h) as u64 * 4, none),
        need_off: tiles_max(lat_w, lat_h),
        changed_tiles: storage(gpu, "changed tiles", tiles_max(lat_w, lat_h) as u64 * 4, none),
        sky: [0, 1].map(|_| storage(gpu, "sky mask", w as u64 * h as u64 * 4, none)),
    }
}
