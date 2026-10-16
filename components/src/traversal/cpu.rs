//! Shared types of the CPU traversal kernels, and [`Kernel`]: what each declares.

use glam::IVec3;

use crate::Cap;

/// A CPU traversal kernel's declaration (PLAN step 10): the optional capabilities it
/// provides, read by the registry (as `Repr::CAPS` for storages). Implemented by a unit type
/// next to the kernel's functions, which generated pipelines will name as type parameters.
pub trait Kernel {
    const NAME: &'static str;
    const CAPS: &'static [Cap];
}

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

