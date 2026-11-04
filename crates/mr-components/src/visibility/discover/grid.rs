//! The CPU voxel world for discovery rays: the composed CPU **storage** (any
//! `mr_components` representation) in the components' dense chunk grid, walked with the
//! `refloor` visibility walk, plus the chunk bookkeeping that keeps it in sync with the
//! `WorldUpdate` stream. Discovery jobs hold immutable snapshots (`Arc<dyn Snapshot>`) while
//! the render thread applies new updates. (Was a copy of cpu-simd's grid and DDA.)
//!
//! Monomorphised per storage: discovery is generic over the [`World`] it
//! walks, and a snapshot is the storage's own grid behind [`Snapshot`], so the one dynamic
//! call is per frame (a discovery pass, an audit), never per ray.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{IVec3, Vec3};
use super::{FaceMap, Output, Settings, State};
use crate::storage::cpu::{Entry, Grid as StorageGrid, Repr};
use crate::traversal::refloor::cpu::trace_packet;
use mr_core::chunk::local_of_block;
use mr_core::*;

pub use crate::storage::cpu::ffloor;

#[derive(Copy, Clone, Default)]
pub struct Hit {
    pub t: f32,
    pub cell: IVec3,
    pub axis: u8,
}

/// What discovery asks of the world.
pub trait World: Send + Sync {
    /// Single ray from camera-relative `o` (`cam` = camera block), ignoring the start cell.
    fn trace(&self, cam: IVec3, o: Vec3, d: Vec3, bmin: IVec3, bmax: IVec3) -> Option<Hit>;
    /// Block id at absolute cell `abs` (0 = air / not loaded).
    fn block(&self, abs: IVec3) -> u16;
    /// Loaded region relative to camera block `cam`.
    fn bounds_rel(&self, cam: IVec3) -> (IVec3, IVec3);
}

/// A world snapshot, whatever the storage: discovery and the audit compiled for it.
pub trait Snapshot: Send + Sync {
    /// One discovery pass (the sampler `set.cluster` chooses) from `cam`.
    fn discover(&self, st: State, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output;
    /// One ray per pixel for `cam`: (pixels whose face is missing from `s`, distinct missing
    /// faces, pixels hitting geometry). Parallel over rows (rayon's current pool).
    fn audit(&self, s: &FaceMap, cam: &Camera, w: u32, h: u32) -> (u64, u64, u64);
}

impl<G: World> Snapshot for G {
    fn discover(&self, st: State, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output {
        super::discover(st, self, cam, w, h, set)
    }

    fn audit(&self, s: &FaceMap, cam: &Camera, w: u32, h: u32) -> (u64, u64, u64) {
        super::audit(self, s, cam, w, h)
    }
}

impl<R: Repr> World for StorageGrid<R> {
    #[inline]
    fn trace(&self, cam: IVec3, o: Vec3, d: Vec3, bmin: IVec3, bmax: IVec3) -> Option<Hit> {
        let mut out = [None];
        trace_packet::<R, 1>(self, cam, o, &[d.x], &[d.y], &[d.z], 1e7, bmin, bmax, &mut out);
        out[0].map(|h| Hit { t: h.t, cell: h.cell, axis: h.axis })
    }

    #[inline]
    fn block(&self, abs: IVec3) -> u16 {
        self.block_at(abs)
    }

    fn bounds_rel(&self, cam: IVec3) -> (IVec3, IVec3) {
        if self.cells.is_empty() {
            return crate::traversal::refloor::cpu::NOWHERE;
        }
        (self.min * 32 - cam, (self.min + self.dims) * 32 - cam)
    }
}

/// The chunk map of one storage representation.
trait Store: Send {
    fn apply(&mut self, updates: &[WorldUpdate]);
    fn snapshot(&mut self) -> Arc<dyn Snapshot>;
    fn memory(&self) -> u64;
}

struct Chunks<R: Repr> {
    chunks: HashMap<IVec3, Entry<R>>,
    grid: Arc<StorageGrid<R>>,
    dirty: bool,
}

impl<R: Repr> Store for Chunks<R> {
    fn apply(&mut self, updates: &[WorldUpdate]) {
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod == 0 => {
                    match data {
                        ChunkData::Uniform(b) if b.is_air() => {
                            self.chunks.remove(&key.pos);
                        }
                        ChunkData::Uniform(b) => {
                            self.chunks.insert(key.pos, Entry::Uniform(b.0));
                        }
                        ChunkData::Dense(d) => {
                            self.chunks.insert(key.pos, Entry::Dense(R::build(d)));
                        }
                    }
                    self.dirty = true;
                }
                WorldUpdate::ChunkUnload { key } if key.lod == 0 => {
                    self.chunks.remove(&key.pos);
                    self.dirty = true;
                }
                WorldUpdate::SetBlocks(edits) => {
                    // Apply all edits first, then rebuild each touched chunk once.
                    let mut touched: HashMap<IVec3, ChunkData> = HashMap::new();
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        let data = match touched.get_mut(&pos) {
                            Some(d) => d,
                            None => {
                                let Some(entry) = self.chunks.get(&pos) else { continue };
                                let d = match entry {
                                    Entry::Uniform(b) => ChunkData::Uniform(BlockId(*b)),
                                    Entry::Dense(r) => ChunkData::Dense(Arc::new(r.to_dense())),
                                };
                                touched.entry(pos).or_insert(d)
                            }
                        };
                        data.set(local_of_block(e.pos), e.block);
                    }
                    for (pos, data) in touched {
                        let entry = match data.compact() {
                            ChunkData::Uniform(b) if b.is_air() => {
                                self.chunks.remove(&pos);
                                continue;
                            }
                            ChunkData::Uniform(b) => Entry::Uniform(b.0),
                            ChunkData::Dense(d) => Entry::Dense(R::build(&d)),
                        };
                        self.chunks.insert(pos, entry);
                    }
                    self.dirty = true;
                }
                _ => {}
            }
        }
    }

    fn snapshot(&mut self) -> Arc<dyn Snapshot> {
        if self.dirty {
            self.dirty = false;
            let mut it = self.chunks.keys();
            self.grid = Arc::new(match it.next() {
                None => StorageGrid::default(),
                Some(&first) => {
                    let (lo, hi) = it.fold((first, first), |(lo, hi), &k| (lo.min(k), hi.max(k)));
                    let dims = hi - lo + 1;
                    let mut cells = vec![0u32; (dims.x * dims.y * dims.z) as usize];
                    let mut entries = Vec::with_capacity(self.chunks.len());
                    for (pos, e) in &self.chunks {
                        entries.push(e.clone());
                        let c = *pos - lo;
                        cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] = entries.len() as u32;
                    }
                    StorageGrid { min: lo, dims, cells, entries }
                }
            });
        }
        self.grid.clone()
    }

    fn memory(&self) -> u64 {
        let dense: u64 = self.chunks.values().map(|e| if let Entry::Dense(r) = e { r.total_bytes() as u64 } else { 0 }).sum();
        dense + self.grid.cells.len() as u64 * 4
    }
}

/// Chunk map mirrored from the update stream, in the chosen CPU storage.
pub struct CpuWorld {
    store: Box<dyn Store>,
}

fn store<R: Repr>() -> Box<dyn Store> {
    Box::new(Chunks::<R> { chunks: HashMap::new(), grid: Arc::new(StorageGrid::default()), dirty: false })
}

impl CpuWorld {
    /// `storage`: a CPU storage component (validated by the caller).
    pub fn new(storage: &str) -> anyhow::Result<Self> {
        use crate::storage as s;
        let store = match storage {
            "bricks" => store::<s::bricks::cpu::U16>(),
            "u8" => store::<s::u8::cpu::U8>(),
            "bits" => store::<s::bits::cpu::Bits>(),
            "hier" => store::<s::hier::cpu::Hier>(),
            "prefix" => store::<s::prefix::cpu::Prefix>(),
            "palette" => store::<s::palette::cpu::Palette>(),
            "brickmajor" => store::<s::brickmajor::Cpu>(),
            "morton" => store::<s::morton::Cpu>(),
            other => anyhow::bail!("storage `{other}` has no CPU implementation"),
        };
        Ok(Self { store })
    }

    /// The world in storage `R` (a pipeline compiled for one storage names it).
    pub fn of<R: Repr>() -> Self {
        Self { store: store::<R>() }
    }

    pub fn apply(&mut self, updates: &[WorldUpdate]) {
        self.store.apply(updates);
    }

    /// Current grid snapshot (rebuilt if the world changed since the last call).
    pub fn snapshot(&mut self) -> Arc<dyn Snapshot> {
        self.store.snapshot()
    }

    pub fn memory(&self) -> u64 {
        self.store.memory()
    }
}
