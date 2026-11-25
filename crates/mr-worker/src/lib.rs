//! The worker process: hosts exactly one renderer, driven over the IPC protocol
//! (`mr_serve`): an executor (`cpu`, `gpu`), running the region its graph picks, or a
//! standalone renderer. Adding a renderer = adding its crate as a dependency and one line in
//! [`registry`].

use mr_core::{RendererEntry, RendererInfo};

pub fn registry() -> Vec<RendererEntry> {
    vec![
        mr_reference::entry(),
        mr_exec_cpu::entry(),
        mr_exec_gpu::entry(),
    ]
}

/// The standalone renderers (a pipeline's info is its graph's: the host asks the executor
/// that runs it).
pub fn infos() -> Vec<RendererInfo> {
    vec![mr_reference::info()]
}

/// Connects to the host at `socket` and serves until shutdown.
pub fn run(socket: &str) -> anyhow::Result<()> {
    mr_serve::run(socket, registry(), mr_gpu::devices)
}
