//! Hole detection (PLAN step 13b): discovery's piece that decides where the sampler looks.
//! `none` (no node: every tile, every frame); `coverage` ([`coverage::GPU_WGSL`], the root
//! of its passes over `crate::visibility::quadtree::SET_WGSL`): last frame's faces drawn at
//! this camera mark the faces still seen, and the tiles with uncovered pixels (not known
//! sky), occlusion edges whose parallax can reveal something, edited blocks or changed
//! chunks, or their refresh turn need discovery; after the final draw the sky mask is kept
//! for the next frame. In the graph it sits in a loop with the drawer (`draw.faces`
//! iterates twice) and reads the last frame's image (`sky`, timing `prev`).

pub mod coverage {
    pub const GPU_WGSL: &str = include_str!("coverage/gpu.wgsl");
}
