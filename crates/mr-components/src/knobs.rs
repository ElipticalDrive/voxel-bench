//! The defaults of the components' knobs (those a node sets, `ParamSpec`s with an owner):
//! what the code runs with where a node leaves a knob unset. A node setting a knob to its
//! default is the same pipeline as one leaving it unset, so the canonical form drops it
//! ([`crate::graph::Graph::canonical`]) and both share a key.
//!
//! The executors declare the knobs (`ParamSpec`, with descriptions and ranges); this table
//! holds the defaults they agree on everywhere, and a test of the root crate keeps it equal
//! to theirs: a knob is here exactly when every region declaring it gives it one default,
//! with that default. A knob whose default differs between regions or devices (`placement`,
//! the quadtree's `tile`) is absent: written, it always keys apart.

use crate::Role;

/// A default as the knob's TOML value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

impl Value {
    pub fn value(self) -> toml::Value {
        match self {
            Value::Bool(b) => b.into(),
            Value::Int(i) => i.into(),
            Value::Float(f) => f.into(),
            Value::Str(s) => s.into(),
        }
    }
}

use Value::{Bool as B, Float as F, Int as I, Str as S};

/// Per component, its knobs' defaults.
pub const DEFAULTS: &[(Role, &str, &[(&str, Value)])] = &[
    (
        Role::Draw,
        "raster",
        &[
            ("band_height", I(16)),
            ("depth_prepass", B(true)),
            ("order", S("depth")),
            ("tile_width", I(64)),
        ],
    ),
    (Role::Geometry, "cubes", &[("cull", B(true))]),
    (
        Role::Geometry,
        "greedy",
        &[("cull_faces", S("back")), ("merge", S("block"))],
    ),
    (
        Role::Geometry,
        "naive",
        &[("cull", B(true)), ("cull_faces", S("back"))],
    ),
    (
        Role::Holes,
        "coverage",
        &[
            ("events", B(true)),
            ("keep_unseen", I(64)),
            ("parallax_px", F(0.5)),
            ("refresh", I(16)),
            ("reveal_tiles", I(4)),
            ("still_holes", S("now")),
        ],
    ),
    (
        Role::Post,
        "aces",
        &[("view", S("image")), ("view_max", I(256))],
    ),
    (
        Role::Post,
        "clamp",
        &[("view", S("image")), ("view_max", I(256))],
    ),
    (
        Role::Post,
        "hdr",
        &[("view", S("image")), ("view_max", I(256))],
    ),
    (
        Role::Post,
        "reinhard",
        &[("view", S("image")), ("view_max", I(256))],
    ),
    (Role::Prepass, "beam", &[("tile", I(8))]),
    (
        Role::Shading,
        "ao",
        &[
            ("accumulate", B(true)),
            ("placement", S("inline")),
            ("reach", I(4)),
            ("shadows", S("all")),
            ("spp", I(1)),
        ],
    ),
    (Role::Shading, "direct", &[("shadows", S("all"))]),
    (
        Role::Shading,
        "pathtrace",
        &[
            ("accumulate", B(true)),
            ("bounces", I(1)),
            ("placement", S("inline")),
            ("shadows", S("all")),
            ("spp", I(1)),
        ],
    ),
    (
        Role::Traversal,
        "incremental",
        &[
            ("exact", B(true)),
            ("skip16", B(true)),
            ("skip2", B(true)),
            ("skip4", B(true)),
            ("skip8", B(true)),
        ],
    ),
    (Role::Traversal, "refloor", &[("packet", S("1"))]),
    (
        Role::Visibility,
        "cluster",
        &[
            ("dilate", I(2)),
            ("dilate_below", I(2)),
            ("seed_step", I(16)),
            ("sky", S("corners")),
        ],
    ),
    (
        Role::Visibility,
        "quadtree",
        &[
            ("audit", B(false)),
            ("block_faces", B(true)),
            ("closure_box", I(2)),
            ("closure_px", I(2)),
            ("criterion", S("plane")),
            ("dilate", I(2)),
            ("dilate_below", I(2)),
            ("inline_fp", I(8)),
            ("jitter", B(true)),
            ("keep_frames", I(8)),
            ("max_faces", I(1048576)),
            ("merge", S("none")),
            ("morton", B(false)),
            ("occluders", I(4)),
            ("sky_tile", I(4)),
            ("strict", B(true)),
        ],
    ),
    (Role::Visibility, "splat", &[("max_splat", I(16))]),
];

/// The default of knob `knob` of `role` component `component`, where it has one everywhere.
pub fn default(role: Role, component: &str, knob: &str) -> Option<toml::Value> {
    DEFAULTS
        .iter()
        .find(|(r, c, _)| *r == role && *c == component)
        .and_then(|(_, _, ks)| ks.iter().find(|(k, _)| *k == knob))
        .map(|(_, d)| d.value())
}
