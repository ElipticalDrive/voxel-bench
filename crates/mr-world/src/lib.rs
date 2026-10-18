//! World-side machinery used by the host: generators, textures, the authoritative chunk
//! store, and scripted scenes. Renderers never depend on this crate.

pub mod generators;
pub mod meshes;
pub mod noise;
pub mod scenario;
pub mod stream;
pub mod textures;
pub mod world;

pub use generators::{GenSpec, WorldGen};
pub use scenario::{Scene, SceneSpec};
pub use world::World;
