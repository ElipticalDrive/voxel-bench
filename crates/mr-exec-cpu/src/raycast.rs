//! CPU ray casting (`cpu`): a renderer from components (`mr-components`), run as nodes of
//! the generic executor ([`crate::exec`]): a
//! **storage** (`Repr`, one generic instantiation per storage: monomorphised, so the split
//! costs nothing), a **traversal** kernel cast per pixel (`refloor` with SIMD-friendly ray
//! packets, or `incremental`; the visibility strategy is `raycast`) and the **shading** component (`direct`), multi-threaded with rayon.
//! Optional hardware counters (`perf` knob) via `perf_event_open` report cycles,
//! instructions and cache misses per pixel.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use mr_reference::{Pool, Shadows, World};
use glam::{IVec3, Vec3};
use mr_components::storage::bits::cpu::Bits;
use mr_components::storage::bricks::cpu::U16;
use mr_components::storage::cpu::{Entry, Grid, Repr};
use mr_components::storage::hier::cpu::Hier;
use mr_components::storage::palette::cpu::Palette;
use mr_components::storage::prefix::cpu::Prefix;
use mr_components::storage::u8::cpu::U8;
use mr_components::graph::{Build, Builds, Checked, Role};
use mr_components::traversal::cpu::Hit;
use mr_components::traversal::incremental::cpu::trace_incremental;
use mr_components::traversal::refloor::cpu::{NOWHERE, trace_packet};
use mr_components::visibility::record::Record;
use mr_components::{Kind, names_on, parse_placement, placement_param, require, Slot};
use mr_core::chunk::local_of_block;
use mr_core::*;
use rayon::prelude::*;
use serde::Deserialize;

use crate::{Generic, Pixel, Spec, fixed, perf};
pub(crate) use nodes::executor;

/// The Rust type of each CPU storage, for generated code.
pub fn storage_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "bricks" => "mr_components::storage::bricks::cpu::U16",
        "u8" => "mr_components::storage::u8::cpu::U8",
        "bits" => "mr_components::storage::bits::cpu::Bits",
        "hier" => "mr_components::storage::hier::cpu::Hier",
        "prefix" => "mr_components::storage::prefix::cpu::Prefix",
        "palette" => "mr_components::storage::palette::cpu::Palette",
        "brickmajor" => "mr_components::storage::brickmajor::Cpu",
        "morton" => "mr_components::storage::morton::Cpu",
        _ => return None,
    })
}

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu".into(),
        description: "CPU back end: renders a composition of components (storage, traversal, shading), multi-threaded".into(),
        tags: vec!["cpu".into(), "raycast".into(), "simd".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

/// The executor for graph `g`, its nodes monomorphised for its storage.
pub(crate) fn create<P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let opts: Options = p.options()?;
    let storage = require(BACKEND, Slot::Storage, &opts.storage)?;
    match storage.as_str() {
        "bricks" => of::<U16, P>(p, g),
        "u8" => of::<U8, P>(p, g),
        "bits" => of::<Bits, P>(p, g),
        "hier" => of::<Hier, P>(p, g),
        "prefix" => of::<Prefix, P>(p, g),
        "palette" => of::<Palette, P>(p, g),
        "brickmajor" => of::<mr_components::storage::brickmajor::Cpu, P>(p, g),
        "morton" => of::<mr_components::storage::morton::Cpu, P>(p, g),
        s => anyhow::bail!("storage `{s}` has no CPU implementation"),
    }
}

fn of<R: Repr, P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    executor::<R, Generic, P>(p, g)
}

pub(crate) const BACKEND: &str = "cpu";

/// The nodes it builds: one storage (one monomorphised instance; the shadow rays walk it
/// too), its primary and shadow kernels, the ray caster and the shading.
pub(crate) const BUILDS: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["bricks", "u8", "bits", "hier", "prefix", "palette", "brickmajor", "morton"], Kind::Cpu),
        Build::one(Role::Traversal, &["refloor", "incremental"], Kind::Cpu).count(1, 2),
        Build::one(Role::Visibility, &["raycast"], Kind::Cpu),
        Build::one(Role::Shading, &["direct"], Kind::Cpu),
    ],
    same: &[],
};

fn params() -> Vec<ParamSpec> {
    let mut p = mr_reference::cpu_params();
    p.push(ParamSpec::choice("storage", "Storage component (CPU implementations)", &names_on(BACKEND, Slot::Storage), "bricks", false));
    p.push(ParamSpec::choice("visibility", "Visibility strategy", &names_on(BACKEND, Slot::Visibility), "raycast", false));
    p.push(ParamSpec::choice("traversal", "Traversal kernel. refloor: probe for the largest empty cube, step out, re-floor (ray packets). incremental: hierarchical DDA keeping per-level exit times (packets ignored)", &names_on(BACKEND, Slot::Traversal), "refloor", true));
    p.push(ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", true));
    p.push(placement_param(BACKEND, "inline"));
    p.push(ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell (no float drift) instead of accumulating them", true, true).of("traversal", &["incremental"]));
    p.push(ParamSpec::choice("packet", "Rays traced together in lockstep (refloor; measured: no gain on Zen 5)", &["1", "4", "8", "16"], "1", true).of("traversal", &["refloor"]));
    p.push(ParamSpec::choice("isa", "Instruction set for the hot loop", &["auto", "scalar", "avx2", "avx512"], "auto", true));
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them. Fewer levels mean fewer failed ascent tests but smaller skips"), true, true).of("traversal", &["incremental"]));
    }
    p.push(ParamSpec::choice("schedule", "Work split over the threads: image rows (packets along the row), or screen tiles along a space-filling curve (packets as 2x2 / 4x2 / 4x4 pixel blocks; from cpu-simd-tiles)", &["rows", "tiles"], "rows", true));
    p.push(ParamSpec::choice("tile_order", "schedule = tiles: order tiles are handed out in", &["morton", "hilbert", "linear"], "morton", true));
    p.push(ParamSpec::int("tile", "schedule = tiles: tile edge in pixels", 4, 256, 16, true));
    p.push(ParamSpec::bool("perf", "Hardware counters per frame (Linux: perf_event_open; cycles, instructions, cache misses per pixel)", false, true));
    p.push(ParamSpec::choice("view", "image: the rendered scene. steps: the walk's work per pixel on a colour scale (log; white above view_max): refloor = lockstep iterations of the pixel's packet, incremental = occupancy tests of the pixel's ray. distance: the primary hit's distance in blocks (linear; white above view_max and for the sky)", &["image", "steps", "distance"], "image", true));
    p.push(ParamSpec::int("view_max", "steps / distance view: the top of the colour scale", 1, 16384, 256, true));
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    threads: usize,
    shadows: Shadows,
    storage: String,
    visibility: String,
    traversal: String,
    shading: String,
    placement: String,
    exact: bool,
    packet: String,
    isa: String,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    schedule: String,
    tile_order: String,
    tile: u32,
    perf: bool,
    view: String,
    view_max: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { threads: 0, shadows: Shadows::All, storage: "bricks".into(), visibility: "raycast".into(), traversal: "refloor".into(), shading: "direct".into(), placement: "inline".into(), exact: true, packet: "1".into(), isa: "auto".into(), skip2: true, skip4: true, skip8: true, skip16: true, schedule: "rows".into(), tile_order: "morton".into(), tile: 16, perf: false, view: "image".into(), view_max: 256 }
    }
}

/// The traversal kernel, with the `exact` knob folded in.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Traversal {
    /// `trace_packet`: the probe returns a skip size, and coarse steps re-floor.
    Refloor,
    /// `trace_incremental`, exit times accumulated by adds.
    Incremental,
    /// `trace_incremental`, exit times recomputed from the integer cell (one multiply).
    IncrementalExact,
}

/// What the pixels show (`view`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Image,
    Steps,
    Distance,
}

fn parse_view(s: &str) -> anyhow::Result<View> {
    match s {
        "image" => Ok(View::Image),
        "steps" => Ok(View::Steps),
        "distance" => Ok(View::Distance),
        _ => anyhow::bail!("view must be `image`, `steps` or `distance`, got `{s}`"),
    }
}

fn parse_schedule(s: &str) -> anyhow::Result<bool> {
    match s {
        "rows" => Ok(false),
        "tiles" => Ok(true),
        _ => anyhow::bail!("schedule must be rows or tiles, got `{s}`"),
    }
}

/// The traversal kernel for the `traversal` and `exact` knobs (also for generated code).
pub fn traversal(kernel: &str, exact: bool) -> anyhow::Result<Traversal> {
    Ok(match require(BACKEND, Slot::Traversal, kernel)?.as_str() {
        "refloor" => Traversal::Refloor,
        "incremental" if exact => Traversal::IncrementalExact,
        "incremental" => Traversal::Incremental,
        v => anyhow::bail!("traversal `{v}` has no CPU implementation"),
    })
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Isa {
    Scalar,
    Avx2,
    Avx512,
}

fn detect(choice: &str) -> Isa {
    let avx512 = std::arch::is_x86_feature_detected!("avx512f") && std::arch::is_x86_feature_detected!("avx512vl") && std::arch::is_x86_feature_detected!("avx512bw");
    let avx2 = std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma");
    match choice {
        "scalar" => Isa::Scalar,
        "avx2" if avx2 => Isa::Avx2,
        "avx512" if avx512 => Isa::Avx512,
        "auto" if avx512 => Isa::Avx512,
        "auto" if avx2 => Isa::Avx2,
        _ => Isa::Scalar,
    }
}

// ------------------------------------------------------------------------- tracing

/// Per-row totals: lockstep lane steps (refloor) or occupancy tests (incremental), plus the
/// incremental breakdown.
#[derive(Copy, Clone, Default)]
struct RowWork {
    lane_steps: u64,
    steps: u64,
    up: u64,
    down: u64,
}

impl std::ops::Add for RowWork {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self { lane_steps: self.lane_steps + o.lane_steps, steps: self.steps + o.steps, up: self.up + o.up, down: self.down + o.down }
    }
}

struct Scene<R> {
    chunks: HashMap<IVec3, Entry<R>>,
    grid: Grid<R>,
    grid_dirty: bool,
    traversal: Traversal,
    /// Levels the incremental traversals may use (bit k = 2^k cubes), from the `skip*` knobs.
    levels: u32,
    /// The shadow rays' own kernel and levels (a kernel node of their own over
    /// the same storage), `None`: the primary rays' kernel.
    shadow: Option<(Traversal, u32)>,
}

/// The shadow rays' own kernel and levels in graph `g` (a kernel node of their
/// own, its knobs or their defaults), `None` if they share the primary rays' (also for
/// generated code).
pub fn shadow_kernel(g: &Checked) -> anyhow::Result<Option<(Traversal, u32)>> {
    let primary = g.site(Role::Visibility, "primary").map(|s| s.kernel);
    match g.site(Role::Shading, "shadow") {
        Some(s) if Some(s.kernel) != primary => {
            let n = g.node(s.kernel);
            let on = |k: &str| n.knobs.get(k).and_then(|v| v.as_bool()).unwrap_or(true);
            Ok(Some((traversal(&n.component, on("exact"))?, level_mask(["skip2", "skip4", "skip8", "skip16"].map(on)))))
        }
        _ => Ok(None),
    }
}

/// The skip knobs as a level mask (levels 0 and 5 always allowed; also for generated code).
pub fn level_mask(skips: [bool; 4]) -> u32 {
    skips.iter().enumerate().fold(0b100001, |m, (i, &on)| m | (on as u32) << (i + 1))
}

impl<R: Repr> Scene<R> {
    /// Applies the chunk updates (the world applied its own).
    fn apply(&mut self, updates: &[WorldUpdate]) {
        let s = self;
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod == 0 => {
                    match data {
                        &ChunkData::Uniform(b) if b.is_air() => {
                            s.chunks.remove(&key.pos);
                        }
                        &ChunkData::Uniform(b) => {
                            s.chunks.insert(key.pos, Entry::Uniform(b.0));
                        }
                        ChunkData::Dense(d) => {
                            s.chunks.insert(key.pos, Entry::Dense(R::build(d)));
                        }
                    }
                    s.grid_dirty = true;
                }
                WorldUpdate::ChunkUnload { key } if key.lod == 0 => {
                    s.chunks.remove(&key.pos);
                    s.grid_dirty = true;
                }
                WorldUpdate::ChunkLoad { .. } | WorldUpdate::ChunkUnload { .. } => {}
                WorldUpdate::SetBlocks(edits) => {
                    // Apply all edits first, then rebuild each touched chunk's bricks once.
                    let mut touched: HashMap<IVec3, ChunkData> = HashMap::new();
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        let data = match touched.get_mut(&pos) {
                            Some(d) => d,
                            None => {
                                let Some(entry) = s.chunks.get(&pos) else { continue };
                                let d = match entry {
                                    Entry::Uniform(b) => ChunkData::Uniform(BlockId(*b)),
                                    Entry::Dense(r) => ChunkData::Dense(Arc::new(dense_of(r))),
                                };
                                touched.entry(pos).or_insert(d)
                            }
                        };
                        data.set(local_of_block(e.pos), e.block);
                    }
                    for (pos, data) in touched {
                        let entry = match data.compact() {
                            ChunkData::Uniform(b) if b.is_air() => {
                                s.chunks.remove(&pos);
                                continue;
                            }
                            ChunkData::Uniform(b) => Entry::Uniform(b.0),
                            ChunkData::Dense(d) => Entry::Dense(R::build(&d)),
                        };
                        s.chunks.insert(pos, entry);
                    }
                    s.grid_dirty = true;
                }
                WorldUpdate::LightUpsert(_) | WorldUpdate::LightRemove(_) | WorldUpdate::SetEnvironment(_) => {}
            }
        }
    }

    fn rebuild_grid(&mut self) {
        let mut it = self.chunks.keys();
        let Some(&first) = it.next() else {
            self.grid = Grid::default();
            self.grid_dirty = false;
            return;
        };
        let (lo, hi) = it.fold((first, first), |(lo, hi), &k| (lo.min(k), hi.max(k)));
        let dims = hi - lo + 1;
        let mut cells = vec![0u32; (dims.x * dims.y * dims.z) as usize];
        let mut entries = Vec::with_capacity(self.chunks.len());
        for (pos, e) in &self.chunks {
            entries.push(e.clone());
            let c = *pos - lo;
            cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] = entries.len() as u32;
        }
        self.grid = Grid { min: lo, dims, cells, entries };
        self.grid_dirty = false;
    }

    fn bounds_rel(&self, cam: IVec3) -> (IVec3, IVec3) {
        if self.grid.cells.is_empty() {
            return NOWHERE;
        }
        (self.grid.min * 32 - cam, (self.grid.min + self.grid.dims) * 32 - cam)
    }

    /// Shadow rays: their own kernel, else the primary rays' (constants where `S` baked them).
    fn occluded<S: Spec>(&self, cam: IVec3, p: Vec3, d: Vec3, dist: f32) -> bool {
        let (bmin, bmax) = self.bounds_rel(cam);
        let primary = (S::TRAVERSAL.unwrap_or(self.traversal), S::LEVELS.unwrap_or(self.levels));
        let (kernel, levels) = S::SHADOW.unwrap_or(self.shadow).unwrap_or(primary);
        match kernel {
            Traversal::Refloor => {}
            Traversal::Incremental => return trace_incremental::<R, false>(&self.grid, levels, cam, p, d, dist, bmin, bmax).0.is_some(),
            Traversal::IncrementalExact => return trace_incremental::<R, true>(&self.grid, levels, cam, p, d, dist, bmin, bmax).0.is_some(),
        }
        let mut out = [None];
        trace_packet::<R, 1>(&self.grid, cam, p, &[d.x], &[d.y], &[d.z], dist, bmin, bmax, &mut out);
        out[0].is_some()
    }

    /// The block and face of walk hit `h` on ray direction `d`.
    #[inline]
    fn hit_face(&self, cam: IVec3, d: Vec3, h: &Hit) -> (u16, Face) {
        (self.grid.block_at(h.cell + cam), Face::entered(h.axis as usize, if d[h.axis as usize] > 0.0 { 1 } else { -1 }))
    }

    fn shade<S: Spec, P: Pixel>(&self, world: &World, cam: IVec3, o: Vec3, d: Vec3, h: &Hit, mode: RenderMode) -> P {
        let (block, face) = self.hit_face(cam, d, h);
        self.shade_surface::<S, P>(world, cam, &mr_components::shading::direct::cpu::Surface::at_cell(block, face, h.cell, o + d * h.t), mode)
    }

    /// Shades surface `s` of `world` (its blocks, environment, lights), shadow rays walking
    /// these chunks.
    fn shade_surface<S: Spec, P: Pixel>(&self, world: &World, cam: IVec3, s: &mr_components::shading::direct::cpu::Surface, mode: RenderMode) -> P {
        let shadows = S::SHADOWS.unwrap_or(world.shadows);
        let ctx = mr_components::shading::direct::cpu::Ctx {
            blocks: &world.blocks,
            env: &world.env,
            cam,
            sun_shadows: shadows != Shadows::None,
            light_shadows: shadows == Shadows::All,
            lights_near: |w: IVec3| world.lights_near(w),
            occluded: |p: Vec3, d: Vec3, dist: f32| self.occluded::<S>(cam, p, d, dist),
            tone: S::TONE.unwrap_or(world.tone),
        };
        mr_components::shading::direct::cpu::shade(&ctx, s, mode)
    }
}

// ------------------------------------------------------------------------- the parts

/// The kernel's knobs (`traversal`, `exact`, `skip*`), set on the scene its rays walk.
struct Kernel {
    kernel: String,
    exact: bool,
}

impl Kernel {
    /// Sets knob `name` on `scene`; `false` if it is not one of the kernel's.
    fn set<R>(&mut self, scene: &mut Scene<R>, name: &str, value: &ParamValue) -> anyhow::Result<bool> {
        match name {
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << (name[4..].parse::<u32>()?.trailing_zeros());
                if value.as_bool().unwrap_or(true) {
                    scene.levels |= bit;
                } else {
                    scene.levels &= !bit;
                }
            }
            "traversal" | "exact" => {
                let (kernel, exact) = match name {
                    "traversal" => (value.as_str().unwrap_or("refloor").to_string(), self.exact),
                    _ => (self.kernel.clone(), value.as_bool().unwrap_or(true)),
                };
                let t = traversal(&kernel, exact)?;
                (self.kernel, self.exact, scene.traversal) = (kernel, exact, t);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// The primary rays' cast: how the frame's rays are scheduled and walked, and the views.
struct Cast<S, P> {
    /// The baked choices and the output's pixel.
    spec: std::marker::PhantomData<(S, P)>,
    perf: bool,
    counters: Option<perf::Counters>,
    packet: usize,
    isa: Isa,
    isa_choice: String,
    /// `schedule = tiles` and its tile order and size; `rects` is the work list (rebuilt
    /// lazily when any of them or the size changes).
    tiles: bool,
    tile_order: TileOrder,
    tile: u32,
    rects: Vec<Rect>,
    /// The view and the top of its scale.
    view: View,
    view_max: u32,
}

/// What a cast did.
struct Casted {
    work: RowWork,
    legend: Option<Legend>,
    /// Hardware counters, with the `perf` knob.
    hw: Option<Vec<f64>>,
    /// Wrote hit records (else pixels).
    deferred: bool,
    ms: f32,
}

/// The nodes' parts, from their knobs.
struct Parts<R, S, P> {
    scene: Scene<R>,
    kernel: Kernel,
    cast: Cast<S, P>,
    deferred: bool,
    threads: usize,
    shadows: Shadows,
}

impl<R: Repr, S: Spec, P: Pixel> Parts<R, S, P> {
    fn new(p: &InitParams, g: &Checked) -> anyhow::Result<Self> {
        let o: Options = p.options()?;
        if o.storage == "u8" && p.blocks.blocks.len() > 256 {
            anyhow::bail!("storage u8 needs <= 256 block types, registry has {}", p.blocks.blocks.len());
        }
        let isa = detect(&o.isa);
        tracing::info!("cpu: storage {}, traversal {}, {:?} hot loop, packets of {}", o.storage, o.traversal, isa, o.packet);
        require(BACKEND, Slot::Visibility, &o.visibility)?;
        require(BACKEND, Slot::Shading, &o.shading)?;
        let shadow = shadow_kernel(g)?;
        if let Some((t, _)) = shadow {
            tracing::info!("cpu: shadow rays through their own kernel ({t:?})");
        }
        fixed("shadow rays", S::SHADOW, shadow)?;
        fixed("tone", S::TONE, g.tone())?;
        let b = Self {
            kernel: Kernel { kernel: o.traversal.clone(), exact: o.exact },
            cast: Cast {
                spec: std::marker::PhantomData,
                tiles: parse_schedule(&o.schedule)?,
                tile_order: parse_order(&o.tile_order)?,
                tile: o.tile.clamp(4, 256),
                rects: vec![],
                perf: perf::allowed(o.perf)?,
                counters: None,
                packet: o.packet.parse().unwrap_or(1),
                isa,
                isa_choice: o.isa,
                view: parse_view(&o.view)?,
                view_max: o.view_max.clamp(1, 16384),
            },
            scene: Scene {
                chunks: HashMap::new(),
                grid: Grid::default(),
                grid_dirty: false,
                traversal: fixed("traversal", S::TRAVERSAL, traversal(&o.traversal, o.exact)?)?,
                levels: level_mask([o.skip2, o.skip4, o.skip8, o.skip16]),
                shadow,
            },
            deferred: parse_placement(&o.placement)?,
            threads: o.threads,
            shadows: o.shadows,
        };
        fixed("packets of", S::PACKET, b.cast.packet)?;
        fixed("levels", S::LEVELS, b.scene.levels)?;
        Ok(b)
    }
}

/// A screen rectangle `[x0, x1) x [y0, y1)`: an image row (`schedule = rows`) or a tile.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Rect {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

/// Output pixels, written by disjoint rectangles from several threads.
struct OutPtr<P>(*mut P);
// SAFETY: every pixel belongs to exactly one rectangle of a frame.
unsafe impl<P> Send for OutPtr<P> {}
unsafe impl<P> Sync for OutPtr<P> {}

/// `placement = deferred`: the frame's hit records (one per pixel).
struct RecPtr(*mut Record);
// SAFETY: as `OutPtr`.
unsafe impl Send for RecPtr {}
unsafe impl Sync for RecPtr {}

/// The unit primary ray through pixel (`x`, `y`) of a `w` x `h` image, for the camera basis
/// `f`, `rt`, `u`, `tan` (of half the vertical field of view) and `aspect`.
#[inline(always)]
fn pixel_dir(f: Vec3, rt: Vec3, u: Vec3, tan: f32, aspect: f32, w: u32, h: u32, x: u32, y: u32) -> Vec3 {
    let sy = (1.0 - 2.0 * (y as f32 + 0.5) / h as f32) * tan;
    let sx = (2.0 * (x as f32 + 0.5) / w as f32 - 1.0) * tan * aspect;
    (f + sy * u + sx * rt).normalize()
}

/// What every rectangle of a frame shares.
struct Job<'a, R, P> {
    scene: &'a Scene<R>,
    world: &'a World,
    cam: Camera,
    w: u32,
    h: u32,
    mode: RenderMode,
    packet: usize,
    /// Packets as 2D pixel blocks (tiles) instead of runs along a row.
    two_d: bool,
    /// A diagnostic view: pixels show the walk's work (`View::Steps`) or the hit's
    /// distance on this scale instead of the image.
    steps: Option<(Legend, View)>,
    out: OutPtr<P>,
    /// Where deferred rectangles write their records instead of pixels.
    records: RecPtr,
}

/// The pixels of `r`, `N` rays per packet: `N` along a row, or with `two_d` a 1x1, 2x2,
/// 4x2 or 4x4 block; shaded, or with `D` (`placement = deferred`) their hit records.
/// `#[inline(always)]` so each ISA wrapper below compiles its own copy with its target
/// features.
#[inline(always)]
fn render_rect<R: Repr, S: Spec, P: Pixel, const N: usize, const D: bool>(job: &Job<R, P>, r: Rect) -> RowWork {
    let (scene, w, h, mode) = (job.scene, job.w, job.h, job.mode);
    let mut work = RowWork::default();
    let (cam_block, o) = job.cam.split_position();
    // Camera basis once per rectangle (Camera::ray_dir recomputes sin/cos per call).
    let (f, rt, u) = (job.cam.forward(), job.cam.right(), job.cam.up());
    let tan = (job.cam.fov_y * 0.5).tan();
    let aspect = w as f32 / h as f32;
    let (bmin, bmax) = scene.bounds_rel(cam_block);
    let sky: P = job.world.sky();
    let (bw, bh) = if job.two_d {
        match N {
            1 => (1, 1),
            4 => (2, 2),
            8 => (4, 2),
            _ => (4, 4),
        }
    } else {
        (N as u32, 1)
    };
    let mut by = r.y0;
    while by < r.y1 {
        let mut bx = r.x0;
        while bx < r.x1 {
            let (mut dx, mut dy, mut dz) = ([f32::NAN; N], [0f32; N], [0f32; N]);
            let mut dirs = [Vec3::ZERO; N];
            let mut pix = [u32::MAX; N];
            for l in 0..N {
                let (x, y) = (bx + l as u32 % bw, by + l as u32 / bw);
                if x < r.x1 && y < r.y1 {
                    let d = pixel_dir(f, rt, u, tan, aspect, w, h, x, y);
                    (dx[l], dy[l], dz[l]) = (d.x, d.y, d.z);
                    dirs[l] = d;
                    pix[l] = y * w + x;
                }
            }
            let mut hits = [None; N];
            // Work per lane, for the steps view.
            let mut lane_work = [0u32; N];
            // A baked kernel is a constant: the other branches fold away.
            let kernel = S::TRAVERSAL.unwrap_or(scene.traversal);
            let levels = S::LEVELS.unwrap_or(scene.levels);
            if kernel == Traversal::Refloor {
                let it = trace_packet::<R, N>(&scene.grid, cam_block, o, &dx, &dy, &dz, 1e7, bmin, bmax, &mut hits);
                work.lane_steps += it as u64 * N as u64;
                if job.steps.is_some() {
                    lane_work = [it; N];
                }
            } else {
                for l in 0..N {
                    if !dx[l].is_nan() {
                        let (hit, t) = match kernel {
                            Traversal::IncrementalExact => trace_incremental::<R, true>(&scene.grid, levels, cam_block, o, dirs[l], 1e7, bmin, bmax),
                            _ => trace_incremental::<R, false>(&scene.grid, levels, cam_block, o, dirs[l], 1e7, bmin, bmax),
                        };
                        hits[l] = hit;
                        // Occupancy tests: one per step and per descent, plus the first
                        // cell and the ascent attempts.
                        work.lane_steps += (t.steps + t.down + t.up + 1) as u64;
                        if job.steps.is_some() {
                            lane_work[l] = t.steps + t.down + t.up + 1;
                        }
                        work.steps += t.steps as u64;
                        work.up += t.up as u64;
                        work.down += t.down as u64;
                    }
                }
            }
            for l in 0..N {
                if pix[l] == u32::MAX {
                    continue;
                }
                if D {
                    let rec = match &hits[l] {
                        Some(hit) => {
                            let (block, face) = scene.hit_face(cam_block, dirs[l], hit);
                            Record::hit(hit.t, block, face, hit.cell)
                        }
                        None => Record::SKY,
                    };
                    // SAFETY: as for the pixels below.
                    unsafe { *job.records.0.add(pix[l] as usize) = rec };
                    continue;
                }
                let px = match &hits[l] {
                    Some(hit) => scene.shade::<S, P>(job.world, cam_block, o, dirs[l], hit, mode),
                    None => sky,
                };
                // SAFETY: pix < w * h, and each pixel belongs to exactly one rectangle.
                unsafe { *job.out.0.add(pix[l] as usize) = px };
            }
            if let Some((scale, view)) = &job.steps {
                for l in 0..N {
                    if pix[l] != u32::MAX {
                        let v = if *view == View::Steps { lane_work[l] as f64 } else { hits[l].as_ref().map_or(f64::INFINITY, |h| h.t as f64) };
                        let [r, g, b] = scale.color(v);
                        // SAFETY: as above.
                        unsafe { *job.out.0.add(pix[l] as usize) = P::from_rgba8([r, g, b, 255]) };
                    }
                }
            }
            bx += bw;
        }
        by += bh;
    }
    work
}

#[inline(always)]
fn render_rect_dyn<R: Repr, S: Spec, P: Pixel, const D: bool>(job: &Job<R, P>, r: Rect) -> RowWork {
    match S::PACKET.unwrap_or(job.packet) {
        1 => render_rect::<R, S, P, 1, D>(job, r),
        4 => render_rect::<R, S, P, 4, D>(job, r),
        16 => render_rect::<R, S, P, 16, D>(job, r),
        _ => render_rect::<R, S, P, 8, D>(job, r),
    }
}

#[target_feature(enable = "avx2,fma")]
fn render_rect_avx2<R: Repr, S: Spec, P: Pixel, const D: bool>(job: &Job<R, P>, r: Rect) -> RowWork {
    render_rect_dyn::<R, S, P, D>(job, r)
}

#[target_feature(enable = "avx512f,avx512vl,avx512bw,avx2,fma")]
fn render_rect_avx512<R: Repr, S: Spec, P: Pixel, const D: bool>(job: &Job<R, P>, r: Rect) -> RowWork {
    render_rect_dyn::<R, S, P, D>(job, r)
}

fn render_rect_isa<R: Repr, S: Spec, P: Pixel, const D: bool>(isa: Isa, job: &Job<R, P>, r: Rect) -> RowWork {
    if S::NATIVE {
        // The whole crate is compiled for this CPU.
        return render_rect_dyn::<R, S, P, D>(job, r);
    }
    match isa {
        // SAFETY: only selected when the CPU reports these features.
        Isa::Avx512 => unsafe { render_rect_avx512::<R, S, P, D>(job, r) },
        Isa::Avx2 => unsafe { render_rect_avx2::<R, S, P, D>(job, r) },
        Isa::Scalar => render_rect_dyn::<R, S, P, D>(job, r),
    }
}

/// `placement = deferred`: the frame's hit records, shaded by the common deferred pass
/// (`shading::deferred`).
struct Deferred<'a, R, S, P> {
    spec: std::marker::PhantomData<S>,
    scene: &'a Scene<R>,
    world: &'a World,
    records: &'a [Record],
    cam: IVec3,
    o: Vec3,
    /// Forward, right, up; tan(fov / 2), aspect.
    basis: (Vec3, Vec3, Vec3, f32, f32),
    w: u32,
    h: u32,
    mode: RenderMode,
    sky: P,
}

impl<'a, R: Repr, S: Spec, P: Pixel> Deferred<'a, R, S, P> {
    fn new(scene: &'a Scene<R>, world: &'a World, records: &'a [Record], cam: &Camera, w: u32, h: u32, mode: RenderMode) -> Self {
        let (cam_block, o) = cam.split_position();
        let basis = (cam.forward(), cam.right(), cam.up(), (cam.fov_y * 0.5).tan(), w as f32 / h as f32);
        Self { spec: std::marker::PhantomData, scene, world, records, cam: cam_block, o, basis, w, h, mode, sky: world.sky() }
    }
}

impl<R: Repr, S: Spec, P: Pixel> mr_components::shading::deferred::Records for Deferred<'_, R, S, P> {
    type Record = Record;
    type Px = P;
    #[inline]
    fn record_at(&self, x: u32, y: u32) -> Record {
        self.records[(y * self.w + x) as usize]
    }
    #[inline]
    fn hit(r: Record) -> bool {
        r.is_hit()
    }
    #[inline]
    fn shade(&self, r: Record, x: u32, y: u32) -> P {
        let (f, rt, u, tan, aspect) = self.basis;
        let d = pixel_dir(f, rt, u, tan, aspect, self.w, self.h, x, y);
        self.scene.shade_surface::<S, P>(self.world, self.cam, &r.surface(self.o, d), self.mode)
    }
    #[inline]
    fn miss(&self, _: u32, _: u32) -> P {
        self.sky
    }
}

// ------------------------------------------------------------------------- schedule

/// Order screen tiles are handed to the threads in (`schedule = tiles`).
#[derive(Copy, Clone, PartialEq, Debug)]
enum TileOrder {
    Linear,
    Morton,
    Hilbert,
}

fn parse_order(s: &str) -> anyhow::Result<TileOrder> {
    Ok(match s {
        "linear" => TileOrder::Linear,
        "morton" => TileOrder::Morton,
        "hilbert" => TileOrder::Hilbert,
        _ => anyhow::bail!("tile_order must be linear, morton or hilbert, got `{s}`"),
    })
}

fn morton2(x: u32, y: u32) -> u64 {
    let mut r = 0u64;
    for i in 0..16 {
        r |= (((x >> i) & 1) as u64) << (2 * i) | (((y >> i) & 1) as u64) << (2 * i + 1);
    }
    r
}

/// Distance along a Hilbert curve on an `n` x `n` grid (`n` a power of two).
fn hilbert(n: u32, mut x: u32, mut y: u32) -> u64 {
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = ((x & s) > 0) as u32;
        let ry = ((y & s) > 0) as u32;
        d += s as u64 * s as u64 * ((3 * rx) ^ ry) as u64;
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - (x & (s - 1)) + (x & !(s - 1));
                y = s - 1 - (y & (s - 1)) + (y & !(s - 1));
                x &= s - 1;
                y &= s - 1;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x &= s - 1;
        y &= s - 1;
        s /= 2;
    }
    d
}

/// The frame's work items: image rows, or `tile`-pixel tiles along a space-filling curve.
fn rects(w: u32, h: u32, tiles: bool, tile: u32, order: TileOrder) -> Vec<Rect> {
    if !tiles {
        return (0..h).map(|y| Rect { x0: 0, y0: y, x1: w, y1: y + 1 }).collect();
    }
    let (tw, th) = (w.div_ceil(tile), h.div_ceil(tile));
    let n = tw.max(th).next_power_of_two();
    let mut v: Vec<(u64, Rect)> = Vec::with_capacity((tw * th) as usize);
    for ty in 0..th {
        for tx in 0..tw {
            let key = match order {
                TileOrder::Linear => (ty * tw + tx) as u64,
                TileOrder::Morton => morton2(tx, ty),
                TileOrder::Hilbert => hilbert(n, tx, ty),
            };
            v.push((key, Rect { x0: tx * tile, y0: ty * tile, x1: ((tx + 1) * tile).min(w), y1: ((ty + 1) * tile).min(h) }));
        }
    }
    v.sort_by_key(|e| e.0);
    v.into_iter().map(|e| e.1).collect()
}

impl<R: Repr> Scene<R> {
    /// (bytes read by traversal, bytes kept alive) over all dense chunks.
    fn world_bytes(&self) -> (u64, u64) {
        self.chunks.values().fold((0, 0), |(t, a), e| match e {
            Entry::Dense(r) => (t + r.traversal_bytes() as u64, a + r.total_bytes() as u64),
            Entry::Uniform(_) => (t, a),
        })
    }

    /// Rebuilds the grid if chunks changed, timed as the storage's stage.
    fn prepare(&mut self, nodes: &[String], stages: &mut Vec<StageTime>) {
        if self.grid_dirty {
            let start = Instant::now();
            self.rebuild_grid();
            stages.push(StageTime::cpu("storage", nodes, start.elapsed().as_secs_f32() * 1e3));
        }
    }

    /// Its counters for the frame.
    fn counters(&self) -> [(String, f64); 3] {
        let (trav, total) = self.world_bytes();
        [("chunks".into(), self.chunks.len() as f64), ("world_traversal_mb".into(), trav as f64 / 1048576.0), ("world_total_mb".into(), total as f64 / 1048576.0)]
    }

    fn memory(&self) -> u64 {
        self.world_bytes().1 + self.grid.cells.len() as u64 * 4
    }
}

/// Reconstructs a u16 chunk from any representation (for edits).
fn dense_of<R: Repr>(r: &R) -> DenseChunk {
    r.to_dense()
}

impl<S: Spec, P: Pixel> Cast<S, P> {
    /// Casts the frame's primary rays through `scene`: shaded pixels into `pixels`, or with
    /// `records` (deferred, unless a diagnostic view shows the walk) one hit record per pixel.
    fn run<R: Repr>(&mut self, scene: &Scene<R>, pool: &Pool, world: &World, req: &FrameRequest, w: u32, h: u32, pixels: &mut [P], records: Option<&mut Vec<Record>>) -> Casted {
        let (isa, packet) = (self.isa, self.packet);
        let (cam, mode) = (req.camera, req.mode);
        if self.perf && self.counters.is_none() {
            self.counters = perf::Counters::open_on(pool.rayon());
        }
        let counters = if self.perf { self.counters.as_ref() } else { None };
        if let Some(c) = counters {
            c.start();
        }
        if self.rects.is_empty() {
            self.rects = rects(w, h, self.tiles, self.tile, self.tile_order);
        }
        let legend = match self.view {
            View::Image => None,
            View::Steps => {
                let quantity = if scene.traversal == Traversal::Refloor { "lockstep loop iterations (packet)" } else { "occupancy tests (ray)" };
                Some(Legend::ramp(quantity, self.view_max as f64, true))
            }
            View::Distance => Some(Legend::ramp("distance to the primary hit (blocks)", self.view_max as f64, false)),
        };
        // A diagnostic view shows the walk itself, so it runs inline whatever the placement.
        let records = records.filter(|_| legend.is_none()).map(|r| {
            r.resize((w * h) as usize, Record::SKY);
            RecPtr(r.as_mut_ptr())
        });
        let deferred = records.is_some();
        let job = Job { scene, world, cam, w, h, mode, packet, two_d: self.tiles, steps: legend.clone().map(|l| (l, self.view)), out: OutPtr(pixels.as_mut_ptr()), records: records.unwrap_or(RecPtr(std::ptr::null_mut())) };
        let cast = Instant::now();
        let rect: fn(Isa, &Job<R, P>, Rect) -> RowWork = if deferred { render_rect_isa::<R, S, P, true> } else { render_rect_isa::<R, S, P, false> };
        let work: RowWork = self.rects.par_iter().map(|&r| rect(isa, &job, r)).reduce(RowWork::default, |a, b| a + b);
        let ms = cast.elapsed().as_secs_f32() * 1e3;
        Casted { work, legend, hw: counters.map(|c| c.stop()), deferred, ms }
    }

    /// Sets knob `name`; `false` if it is not one of the cast's.
    fn set(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<bool> {
        match name {
            "perf" => self.perf = perf::allowed(value.as_bool().unwrap_or(false))?,
            "view" => self.view = parse_view(value.as_str().unwrap_or(""))?,
            "view_max" => self.view_max = value.as_i64().unwrap_or(256).clamp(1, 16384) as u32,
            "visibility" => drop(require(BACKEND, Slot::Visibility, value.as_str().unwrap_or(""))?),
            "shading" => {
                require(BACKEND, Slot::Shading, value.as_str().unwrap_or(""))?;
            }
            "packet" => {
                self.packet = value.as_str().and_then(|s| s.parse().ok()).unwrap_or(8);
            }
            "schedule" => (self.tiles, self.rects) = (parse_schedule(value.as_str().unwrap_or(""))?, vec![]),
            "tile_order" => (self.tile_order, self.rects) = (parse_order(value.as_str().unwrap_or(""))?, vec![]),
            "tile" => (self.tile, self.rects) = (value.as_i64().unwrap_or(16).clamp(4, 256) as u32, vec![]),
            "isa" => {
                self.isa_choice = value.as_str().unwrap_or("auto").to_string();
                self.isa = detect(&self.isa_choice);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

impl Casted {
    /// The walk's counters for a `px`-pixel frame through `traversal`.
    fn counters(&self, traversal: Traversal, px: f64) -> Vec<(String, f64)> {
        let work = &self.work;
        // Lockstep iterations x packet width (work including idle lanes); with
        // incremental traversals, occupancy tests per ray.
        let mut counters = vec![("lane_steps_per_pixel".into(), work.lane_steps as f64 / px)];
        if traversal != Traversal::Refloor {
            counters.push(("dda_steps_per_pixel".into(), work.steps as f64 / px));
            counters.push(("ascent_tests_per_pixel".into(), work.up as f64 / px));
            counters.push(("descents_per_pixel".into(), work.down as f64 / px));
        }
        if let Some(hw) = &self.hw {
            for (&(name, _, _), v) in perf::EVENTS.iter().zip(hw) {
                if v.is_finite() {
                    counters.push((format!("{name}_per_px"), v / px));
                }
            }
            if hw[0].is_finite() && hw[1].is_finite() {
                counters.push(("ipc".into(), hw[1] / hw[0]));
            }
        }
        counters
    }
}

/// Shades `records` (the common deferred pass) into `pixels`.
fn shade_records<R: Repr, S: Spec, P: Pixel>(scene: &Scene<R>, world: &World, records: &[Record], req: &FrameRequest, w: u32, h: u32, pixels: &mut [P]) {
    let deferred = Deferred::<R, S, P>::new(scene, world, records, &req.camera, w, h, req.mode);
    mr_components::shading::deferred::shade(&deferred, w, pixels);
}

// ------------------------------------------------------------------------- the nodes

/// The nodes of the generic executor: the storage (its output: the scene the rays
/// walk, the storage-and-rays port), the visibility (the primary rays' cast; its output: the
/// hit records) and the shading (the common deferred pass; its output: the image). With
/// `placement = inline` visibility and shading are one fused pass writing the image.
mod nodes {
    use std::collections::HashMap;

    use super::*;
    use crate::exec::{self, Cx, Node, Pass, Report, Res};

    struct Storage<R> {
        /// The scene before the first frame (taken by [`Node::output`]).
        init: Option<Scene<R>>,
        kernel: Kernel,
        nodes: Vec<String>,
    }

    fn scene<R: 'static>(r: &Res) -> &Scene<R> {
        r.downcast_ref().expect("a scene")
    }

    impl<R: Repr> Node for Storage<R> {
        fn output(&mut self, _: u32, _: u32) -> Res {
            Box::new(self.init.take().expect("one output per storage"))
        }
        fn update(&mut self, out: &mut Res, updates: &[WorldUpdate]) -> anyhow::Result<()> {
            out.downcast_mut::<Scene<R>>().expect("a scene").apply(updates);
            Ok(())
        }
        fn run(&mut self, _: &Cx, _: &[&Res], out: &mut Res, report: &mut Report) {
            let s = out.downcast_mut::<Scene<R>>().expect("a scene");
            s.prepare(&self.nodes, &mut report.stages);
            report.counters.extend(s.counters());
        }
        fn set_param(&mut self, out: &mut Res, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
            self.kernel.set(out.downcast_mut::<Scene<R>>().expect("a scene"), name, value).map(|k| k.then_some(())).transpose()
        }
        fn memory(&self, out: &Res) -> u64 {
            scene::<R>(out).memory()
        }
    }

    /// The hit records, or with a diagnostic view (`shown`) the pixels showing the walk
    /// (the buffer kept across frames).
    struct Hits<P> {
        records: Vec<Record>,
        pixels: Vec<P>,
        shown: bool,
    }

    /// The primary rays' cast; `fused`: shading inline, its output the image.
    struct Visibility<R, S, P> {
        cast: Cast<S, P>,
        fused: bool,
        nodes: Vec<String>,
        storage: std::marker::PhantomData<R>,
    }

    impl<R: Repr, S: Spec, P: Pixel> Node for Visibility<R, S, P> {
        fn output(&mut self, w: u32, h: u32) -> Res {
            if self.fused { exec::image::<P>(w, h) } else { Box::new(Hits::<P> { records: vec![], pixels: vec![], shown: false }) }
        }
        fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
            let scene = scene::<R>(ins[0]);
            let (w, h) = (cx.width, cx.height);
            let cast = if self.fused {
                let pixels = out.downcast_mut::<Vec<P>>().expect("an image");
                self.cast.run(scene, cx.pool, cx.world, cx.req, w, h, pixels, None)
            } else {
                let hits = out.downcast_mut::<Hits<P>>().expect("hit records");
                hits.pixels.resize((w * h) as usize, P::zeroed());
                let cast = self.cast.run(scene, cx.pool, cx.world, cx.req, w, h, &mut hits.pixels, Some(&mut hits.records));
                hits.shown = !cast.deferred;
                cast
            };
            report.stages.push(StageTime::cpu(if self.fused { "cast+shade" } else { "cast" }, &self.nodes, cast.ms));
            report.counters.extend(cast.counters(scene.traversal, (w * h) as f64));
            report.legend = cast.legend;
        }
        fn set_param(&mut self, _: &mut Res, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
            self.cast.set(name, value).map(|k| k.then_some(())).transpose()
        }
        fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
            self.cast.rects.clear();
            if self.fused {
                *out = exec::image::<P>(w, h);
            }
        }
        fn memory(&self, out: &Res) -> u64 {
            out.downcast_ref::<Hits<P>>().map_or(0, |h| (std::mem::size_of_val(&h.records[..]) + std::mem::size_of_val(&h.pixels[..])) as u64)
        }
    }

    /// The common deferred pass over the hit records; shadow rays walk the scene.
    struct Shading<R, S, P> {
        nodes: Vec<String>,
        spec: std::marker::PhantomData<(R, S, P)>,
    }

    impl<R: Repr, S: Spec, P: Pixel> Node for Shading<R, S, P> {
        fn output(&mut self, w: u32, h: u32) -> Res {
            exec::image::<P>(w, h)
        }
        fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
            let (hits, scene) = (ins[0].downcast_ref::<Hits<P>>().expect("hit records"), scene::<R>(ins[1]));
            let pixels = out.downcast_mut::<Vec<P>>().expect("an image");
            if hits.shown {
                pixels.copy_from_slice(&hits.pixels);
                return;
            }
            let start = Instant::now();
            shade_records::<R, S, P>(scene, cx.world, &hits.records, cx.req, cx.width, cx.height, pixels);
            report.stages.push(StageTime::cpu("shade", &self.nodes, start.elapsed().as_secs_f32() * 1e3));
        }
        fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
            *out = exec::image::<P>(w, h);
        }
    }

    /// The executor for graph `g`: one pass per scheduled storage, visibility and shading
    /// node, each reading the passes its inputs reach (through fused nodes); `S` what a
    /// compiled pipeline baked in.
    pub(crate) fn executor<R: Repr, S: Spec, P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
        g.built_by(super::BACKEND, super::BUILDS).map_err(anyhow::Error::msg)?;
        let parts = Parts::<R, S, P>::new(p, g)?;
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        let mut cast = Some(parts.cast);
        let mut storage = Some((parts.scene, parts.kernel));
        let mut passes: Vec<Pass> = vec![];
        let mut index: HashMap<String, usize> = HashMap::new();
        for pass in schedule.passes() {
            let id = pass.node.as_str();
            let node: Box<dyn Node> = match g.node(id).role {
                Role::Storage => {
                    let (scene, kernel) = storage.take().expect("one storage node");
                    Box::new(Storage::<R> { init: Some(scene), kernel, nodes: vec![id.to_string()] })
                }
                Role::Visibility => {
                    let fused = !parts.deferred;
                    let nodes = if fused { g.ids(&[Role::Visibility, Role::Shading]) } else { vec![id.to_string()] };
                    Box::new(Visibility::<R, S, P> { cast: cast.take().expect("one visibility node"), fused, nodes, storage: std::marker::PhantomData })
                }
                // Fused into the visibility pass.
                Role::Shading if !parts.deferred => continue,
                Role::Shading => Box::new(Shading::<R, S, P> { nodes: vec![id.to_string()], spec: std::marker::PhantomData }),
                _ => continue,
            };
            let mut ins = vec![];
            exec::reads(g, id, &index, &mut ins);
            index.insert(id.to_string(), passes.len());
            passes.push(Pass { node, ins });
        }
        let world = crate::world::<P>(p, g, parts.shadows)?;
        Ok(Box::new(exec::Executor::<P>::new(p, parts.threads, world, passes)?.baked(S::KNOBS, S::NATIVE)))
    }
}
