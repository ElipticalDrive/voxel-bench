//! The worker process: hosts exactly one renderer, driven over the IPC protocol
//! (`mr_serve`). Adding a renderer = adding its crate as a dependency and one line in
//! [`registry`].

use std::path::Path;
use std::sync::Arc;

use mr_core::{RendererEntry, RendererInfo};

pub fn registry() -> Vec<RendererEntry> {
    vec![cpu_reference::entry(), backend_cpu_raster::entry(), backend_discover::cpu_entry(), backend_discover::gpu_entry(), backend_cpu::entry(), backend_voxelspace::cpu_entry(), backend_gpu_splat::entry(), backend_voxelspace::gpu_entry(), gpu_compute::entry(), backend_gpu_raster::entry(), backend_gpu_rt::entry()]
}

pub fn infos() -> Vec<RendererInfo> {
    registry().iter().map(|e| (e.info)()).collect()
}

/// Connects to the host at `socket` and serves until shutdown.
pub fn run(socket: &Path) -> anyhow::Result<()> {
    mr_serve::run(socket, registry(), devices)
}

/// The process's devices, handed to the renderer: GPUs open on request (CPU back ends open
/// none), in the slots the host's configuration names.
fn devices(cfg: &mr_core::DeviceConfig) -> mr_core::Devices {
    mr_core::Devices { gpus: Some(Arc::new(mr_gpu::GpuPool::new(&cfg.gpus))) }
}
