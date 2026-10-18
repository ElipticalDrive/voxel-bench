//! The worker process: hosts exactly one renderer, driven over the IPC protocol
//! (`mr_serve`). Adding a renderer = adding its crate as a dependency and one line in
//! [`registry`].

use mr_core::{RendererEntry, RendererInfo};

pub fn registry() -> Vec<RendererEntry> {
    vec![cpu_reference::entry(), backend_cpu::cpu_raster_entry(), backend_gpu::cpu_discover_entry(), backend_gpu::gpu_discover_entry(), backend_cpu::cpu_entry(), backend_cpu::cpu_voxelspace_entry(), backend_gpu::gpu_splat_entry(), backend_gpu::gpu_voxelspace_entry(), backend_gpu::gpu_compute_entry(), backend_gpu::gpu_raster_entry(), backend_gpu::gpu_rt_entry()]
}

pub fn infos() -> Vec<RendererInfo> {
    registry().iter().map(|e| (e.info)()).collect()
}

/// Connects to the host at `socket` and serves until shutdown.
pub fn run(socket: &str) -> anyhow::Result<()> {
    mr_serve::run(socket, registry(), mr_gpu::devices)
}
