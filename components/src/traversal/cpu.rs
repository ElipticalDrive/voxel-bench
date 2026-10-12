//! Shared types of the CPU visibility components.

use glam::IVec3;

/// First solid cell along a ray (camera-relative), entry time and entry axis.
#[derive(Copy, Clone, Default)]
pub struct Hit {
    pub t: f32,
    pub cell: IVec3,
    pub axis: u8,
}


/// Work done by `trace_incremental`: DDA steps, and occupancy tests made while
/// climbing (ascent attempts) and after descending.
#[derive(Copy, Clone, Default)]
pub struct Tally {
    pub steps: u32,
    pub up: u32,
    pub down: u32,
}

