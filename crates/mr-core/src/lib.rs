//! Shared vocabulary between the host, the worker processes and every renderer.
//!
//! Everything a renderer needs to agree on to produce comparable images lives here:
//! the camera model and primary-ray convention, the face/UV convention for textures,
//! the lighting reference formulas, the chunk format, and the [`Renderer`] trait itself.
//! Renderers depend on this crate only.

pub mod camera;
pub mod chunk;
pub mod client;
pub mod face;
pub mod legend;
pub mod link;
pub mod lighting;
pub mod mesh;
pub mod baked;
pub mod params;
pub mod renderer;
pub mod texture;

pub use camera::Camera;
pub use chunk::{BlockId, CHUNK_SIZE, CHUNK_VOLUME, ChunkData, ChunkKey, DenseChunk};
pub use face::Face;
pub use legend::{Legend, Reading};
pub use lighting::{Environment, Light, PassMode, RenderMode, Sun};
pub use mesh::{MeshData, MeshInstance, MeshUpdate};
pub use params::{ParamKind, ParamSpec, ParamValue};
pub use renderer::*;
pub use texture::{BlockRegistry, BlockType, TEX_SIZE, Texture};

pub use glam;

/// Where compiled pipelines live (generated workers, the GPU drivers' pipeline caches):
/// `$MR_PIPELINE_CACHE`, else `$XDG_CACHE_HOME` or `~/.cache`, `multirenderer/pipelines`.
pub fn cache_dir() -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Some(d) = std::env::var_os("MR_PIPELINE_CACHE") {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    base.join("multirenderer").join("pipelines")
}
