//! The knobs: the parameter list, each node's knob struct and their live changes.

use mr_core::*;
use mr_components::shading::state as shade_state;
use mr_gpu::voxels::{Shadows, shadows_param};
use serde::Deserialize;

use super::{GpuSampler, MAX_TILE};

pub(super) fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::int("tile", "Initial tile size in pixels (power of two)", 2, MAX_TILE as i64, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::choice("criterion", "Tile coherence: all corners hit the same face plane, or the very same face", &["plane", "face"], "plane", true).of("visibility", &["quadtree"]),
        ParamSpec::int("sky_tile", "Tiles whose corners all miss are accepted as sky at or below this size (px)", 1, MAX_TILE as i64, 4, true).of("visibility", &["quadtree"]),
        ParamSpec::int("keep_frames", "Frames a face stays in S without being re-discovered", 1, 1000, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::int("max_footprint", "Max cells a coherent tile may add (larger footprints subdivide)", 1, 4096, 128, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("strict", "Plane criterion: every footprint cell must be an exposed face of that plane, else subdivide", true, true).of("visibility", &["quadtree"]),
        ParamSpec::int("occluders", "Plane criterion: a footprint cell's face also needs this many cells toward the eye free of blocks, else subdivide (0 off: a block floating just above a floor, between the corners' rays, is missed)", 0, 64, 4, true).of("visibility", &["quadtree"]),
        ParamSpec::int("inline_fp", "Footprints of up to this many cells are expanded by the classifying thread; larger ones get a cooperative 64-thread workgroup (0 = always cooperative)", 0, 4096, 8, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("morton", "Enumerate level-0 tiles and rays along a Z-curve (waves cover compact screen squares; children keep their parents' order) instead of row by row. Measured: no difference; deep levels are bound by each pass's slowest ray, not by coherence", false, true).of("visibility", &["quadtree"]),
        ParamSpec::int("closure_px", "Non-coherent tiles up to this size (px) whose corner hits fit a small box add all its exposed faces instead of splitting (0 = off)", 0, MAX_TILE as i64, 2, true).of("visibility", &["quadtree"]),
        ParamSpec::int("closure_box", "Local-closure box size in cells per axis", 1, 8, 2, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("block_faces", "With each discovered face also add its block's other camera-facing exposed faces (closes raster-vs-ray edge cracks)", true, true).of("visibility", &["quadtree"]),
        ParamSpec::bool("jitter", "Shift the tile lattice randomly every frame (temporal coverage)", true, true).of("visibility", &["quadtree"]),
        ParamSpec::int("keep_unseen", "Incremental: frames a face inside the view stays in S without covering a pixel (faces outside the view use keep_frames)", 1, 1000, 64, true).of("holes", &["coverage"]),
        ParamSpec::int("refresh", "Incremental: re-discover every Nth tile each frame regardless (0 = never)", 0, 1024, 16, true).of("holes", &["coverage"]),
        ParamSpec::float("parallax_px", "Incremental: an occlusion edge needs discovery if the camera's translation can reveal more than this many pixels behind it", 0.05, 16.0, false, 0.5, true).of("holes", &["coverage"]),
        ParamSpec::bool("events", "Incremental: classify frames by what changed. Unchanged camera and world: skip discovery and redraw (frame_kind 0). Rotation or world changes only: skip seen-marking and the parallax test (1). Translation: the full incremental passes (2). Edits mark only the edited blocks' screen tiles", true, true).of("holes", &["coverage"]),
        ParamSpec::choice("merge", "Faces drawn as: none = one quad per face; rows = runs of same-block faces along each face's u axis merged into one quad (one GPU pass over S; ignored with order = painter)", &["none", "rows"], "none", true).of("visibility", &["quadtree"]),
        ParamSpec::choice("order", "Face drawing order. depth: any order, depth-tested (lit mode adds a depth pre-pass). painter: sorted back to front by Manhattan distance from the camera cell (an exact order for voxels; GPU counting sort), drawn without depth testing, so every fragment is shaded", &["depth", "painter"], "depth", true).of("draw", &["raster"]),
        ParamSpec::choice("still_holes", "Events, rotation-only frames: now = rasterise the old face list first and discover holes (faces that rotation moved onto a pixel centre) in the same frame; late = skip that raster and discover them next frame (cheaper, holes visible for one frame)", &["now", "late"], "now", true).of("holes", &["coverage"]),
        ParamSpec::int("reveal_tiles", "Incremental: largest reveal radius around an edge, in level-0 tiles", 1, 16, 4, true).of("holes", &["coverage"]),
        ParamSpec::bool("audit", "Also cast one ray per pixel and count pixels whose face is missing from S", false, true).of("visibility", &["quadtree"]),
        shadows_param(),
        ParamSpec::int("max_steps", "DDA step limit (compiled into the kernels)", 32, 16384, 2048, false).baked(true),
        ParamSpec::bool("depth_prepass", "Lit mode, draw = raster: depth pre-pass so lighting runs once per pixel", true, true),
        ParamSpec::int("max_faces", "Capacity of the face set S (faces)", 1 << 16, 1 << 24, 1 << 20, false).of("visibility", &["quadtree"]),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB); for region storages the limit of their own data", 64, 6144, 1536, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
    .into_iter()
    .chain(mr_components::post::view_knobs(super::report::QUANTITIES))
    .chain(shade_state::params())
    .collect()
}

/// The region's own knobs (the nodes' are read from the graph).
#[derive(Deserialize)]
#[serde(default)]
pub(super) struct Options {
    pub(super) max_steps: u32,
    pub(super) depth_prepass: bool,
    pub(super) pool_mb: u32,
    pub(super) frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_steps: 2048, depth_prepass: true, pool_mb: 1536, frames_in_flight: 3 }
    }
}

/// The visibility node's knobs (quadtree).
#[derive(Deserialize)]
#[serde(default)]
pub(super) struct VisibilityKnobs {
    pub(super) tile: u32,
    pub(super) criterion: String,
    pub(super) sky_tile: u32,
    pub(super) keep_frames: u32,
    pub(super) max_footprint: u32,
    pub(super) strict: bool,
    pub(super) occluders: u32,
    pub(super) inline_fp: u32,
    pub(super) morton: bool,
    pub(super) closure_px: u32,
    pub(super) closure_box: u32,
    pub(super) block_faces: bool,
    pub(super) jitter: bool,
    pub(super) merge: String,
    pub(super) audit: bool,
    pub(super) max_faces: u32,
}

impl Default for VisibilityKnobs {
    fn default() -> Self {
        Self {
            tile: 8,
            criterion: "plane".into(),
            sky_tile: 4,
            keep_frames: 8,
            max_footprint: 128,
            strict: true,
            occluders: 4,
            inline_fp: 8,
            morton: false,
            closure_px: 2,
            closure_box: 2,
            block_faces: true,
            jitter: true,
            merge: "none".into(),
            audit: false,
            max_faces: 1 << 20,
        }
    }
}

/// The hole-detection node's knobs (coverage).
#[derive(Deserialize)]
#[serde(default)]
pub(super) struct HolesKnobs {
    pub(super) keep_unseen: u32,
    pub(super) refresh: u32,
    pub(super) parallax_px: f32,
    pub(super) reveal_tiles: u32,
    pub(super) events: bool,
    pub(super) still_holes: String,
}

impl Default for HolesKnobs {
    fn default() -> Self {
        Self { keep_unseen: 64, refresh: 16, parallax_px: 0.5, reveal_tiles: 4, events: true, still_holes: "now".into() }
    }
}

/// The draw node's knobs.
#[derive(Deserialize)]
#[serde(default)]
pub(super) struct DrawKnobs {
    pub(super) order: String,
}

impl Default for DrawKnobs {
    fn default() -> Self {
        Self { order: "depth".into() }
    }
}

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
pub(super) struct ShadingKnobs {
    pub(super) shadows: Shadows,
    pub(super) bounces: u32,
    pub(super) spp: u32,
    pub(super) accumulate: bool,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { shadows: Shadows::All, bounces: 1, spp: 1, accumulate: true }
    }
}

impl GpuSampler {
    /// Its own knobs; `None` if `name` is not one.
    pub(super) fn set_knob(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        let int = |d: i64| value.as_i64().unwrap_or(d) as u32;
        match name {
            "tile" => self.vis.tile = int(8),
            "criterion" => self.vis.criterion = value.as_str().unwrap_or("plane").into(),
            "sky_tile" => self.vis.sky_tile = int(4),
            "keep_frames" => self.vis.keep_frames = int(8),
            "max_footprint" => self.vis.max_footprint = int(128),
            "strict" => self.vis.strict = value.as_bool().unwrap_or(true),
            "occluders" => self.vis.occluders = int(4),
            "inline_fp" => self.vis.inline_fp = int(8),
            "morton" => self.vis.morton = value.as_bool().unwrap_or(true),
            "closure_px" => self.vis.closure_px = int(2),
            "closure_box" => self.vis.closure_box = int(2),
            "block_faces" => self.vis.block_faces = value.as_bool().unwrap_or(true),
            "jitter" => self.vis.jitter = value.as_bool().unwrap_or(true),
            "refresh" => self.cover.refresh = int(16),
            "keep_unseen" => self.cover.keep_unseen = int(64),
            "parallax_px" => self.cover.parallax_px = value.as_f64().unwrap_or(0.5) as f32,
            "reveal_tiles" => self.cover.reveal_tiles = int(4),
            "audit" => self.vis.audit = value.as_bool().unwrap_or(false),
            "events" => self.cover.events = value.as_bool().unwrap_or(true),
            "still_holes" => self.cover.still_holes = value.as_str().unwrap_or("now").into(),
            // Drawing and display only: the face set is unaffected.
            "merge" => self.vis.merge = value.as_str().unwrap_or("none").into(),
            "order" => self.order = value.as_str().unwrap_or("depth").into(),
            _ => return None,
        }
        // Any discovery setting can change what is discoverable: start over from a full frame.
        if !matches!(name, "merge" | "order") {
            self.force_full = true;
        }
        Some(Ok(()))
    }
}
