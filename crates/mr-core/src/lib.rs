//! Shared vocabulary between the host, the worker processes and every renderer.
//!
//! Everything a renderer needs to agree on to produce comparable images lives here:
//! the camera model and primary-ray convention, the face/UV convention for textures,
//! the lighting reference formulas, the chunk format, and the [`Renderer`] trait itself.
//! Renderers depend on this crate only.

pub mod camera;
pub mod chunk;
pub mod face;
pub mod lighting;
pub mod params;
pub mod renderer;
pub mod texture;

pub use camera::Camera;
pub use chunk::{BlockId, CHUNK_SIZE, CHUNK_VOLUME, ChunkData, ChunkKey, DenseChunk};
pub use face::Face;
pub use lighting::{Environment, Light, RenderMode, Sun};
pub use params::{ParamKind, ParamSpec, ParamValue};
pub use renderer::*;
pub use texture::{BlockRegistry, BlockType, TEX_SIZE, Texture};

pub use glam;
