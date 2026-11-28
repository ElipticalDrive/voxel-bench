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

use super::{FaceSet, Output, Settings, State};
use crate::storage::cpu::{Entry, Grid as StorageGrid, Repr};
use crate::traversal::refloor::cpu::trace_packet;
use glam::{IVec3, Vec3};
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
    fn audit(&self, s: &FaceSet, cam: &Camera, w: u32, h: u32) -> (u64, u64, u64);
}

impl<G: World> Snapshot for G {
    fn discover(&self, st: State, cam: &Camera, w: u32, h: u32, set: &Settings) -> Output {
        super::discover(st, self, cam, w, h, set)
    }

    fn audit(&self, s: &FaceSet, cam: &Camera, w: u32, h: u32) -> (u64, u64, u64) {
        super::audit(self, s, cam, w, h)
    }
}

impl<R: Repr> World for StorageGrid<R> {
    #[inline]
    fn trace(&self, cam: IVec3, o: Vec3, d: Vec3, bmin: IVec3, bmax: IVec3) -> Option<Hit> {
        let mut out = [None];
        trace_packet::<R, 1>(
            self,
            cam,
            o,
            &[d.x],
            &[d.y],
            &[d.z],
            1e7,
            bmin,
            bmax,
            &mut out,
        );
        out[0].map(|h| Hit {
            t: h.t,
            cell: h.cell,
            axis: h.axis,
        })
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

/// The blocks exposure tests read (which faces are exposed, their blocks).
pub trait Blocks: Send + Sync {
    fn block(&self, abs: IVec3) -> u16;
}

impl<R: Repr> Blocks for StorageGrid<R> {
    #[inline]
    fn block(&self, abs: IVec3) -> u16 {
        self.block_at(abs)
    }
}

/// Discovery over two storages: its rays walk `rays`, its exposure tests read `exposure`
/// (the graph's `rays` and `exposure` ports bound to different storages).
pub struct Paired<A, B: ?Sized> {
    rays: Arc<A>,
    exposure: Arc<B>,
}

impl<A: World, B: Blocks + ?Sized> World for Paired<A, B> {
    #[inline]
    fn trace(&self, cam: IVec3, o: Vec3, d: Vec3, bmin: IVec3, bmax: IVec3) -> Option<Hit> {
        self.rays.trace(cam, o, d, bmin, bmax)
    }

    #[inline]
    fn block(&self, abs: IVec3) -> u16 {
        self.exposure.block(abs)
    }

    fn bounds_rel(&self, cam: IVec3) -> (IVec3, IVec3) {
        self.rays.bounds_rel(cam)
    }
}

/// The chunk map(s) a world keeps in sync with the update stream.
trait Store: Send {
    fn apply(&mut self, updates: &[WorldUpdate]);
    fn snapshot(&mut self) -> Arc<dyn Snapshot>;
    fn memory(&self) -> u64;
}

/// The chunk map of one storage representation, which can also pair with another's blocks.
trait Single: Store {
    /// This storage as exposure tests read it.
    fn blocks(&mut self) -> Arc<dyn Blocks>;
    /// A snapshot whose rays walk this storage and whose exposure tests read `exposure`.
    fn paired(&mut self, exposure: Arc<dyn Blocks>) -> Arc<dyn Snapshot>;
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
                                let Some(entry) = self.chunks.get(&pos) else {
                                    continue;
                                };
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
        self.grid()
    }

    fn memory(&self) -> u64 {
        self.memory_of()
    }
}

impl<R: Repr> Single for Chunks<R> {
    fn blocks(&mut self) -> Arc<dyn Blocks> {
        self.grid()
    }

    fn paired(&mut self, exposure: Arc<dyn Blocks>) -> Arc<dyn Snapshot> {
        Arc::new(Paired {
            rays: self.grid(),
            exposure,
        })
    }
}

impl<R: Repr> Chunks<R> {
    /// The current grid (rebuilt if the world changed since the last call).
    fn grid(&mut self) -> Arc<StorageGrid<R>> {
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
                        cells[((c.z * dims.y + c.y) * dims.x + c.x) as usize] =
                            entries.len() as u32;
                    }
                    StorageGrid {
                        min: lo,
                        dims,
                        cells,
                        entries,
                    }
                }
            });
        }
        self.grid.clone()
    }

    fn memory_of(&self) -> u64 {
        let dense: u64 = self
            .chunks
            .values()
            .map(|e| {
                if let Entry::Dense(r) = e {
                    r.total_bytes() as u64
                } else {
                    0
                }
            })
            .sum();
        dense + self.grid.cells.len() as u64 * 4
    }
}

/// Two storages compiled together: rays walk `R`'s, exposure tests read `E`'s.
struct Pair<R: Repr, E: Repr> {
    rays: Chunks<R>,
    exposure: Chunks<E>,
}

impl<R: Repr, E: Repr> Store for Pair<R, E> {
    fn apply(&mut self, updates: &[WorldUpdate]) {
        self.rays.apply(updates);
        self.exposure.apply(updates);
    }

    fn snapshot(&mut self) -> Arc<dyn Snapshot> {
        Arc::new(Paired {
            rays: self.rays.grid(),
            exposure: self.exposure.grid(),
        })
    }

    fn memory(&self) -> u64 {
        self.rays.memory_of() + self.exposure.memory_of()
    }
}

/// Two storages chosen at run time: the rays' walk monomorphised, the exposure tests' block
/// reads one dynamic call each (a pipeline compiled for the graph pairs them as [`Pair`]).
struct Dyn {
    rays: Box<dyn Single>,
    exposure: Box<dyn Single>,
}

impl Store for Dyn {
    fn apply(&mut self, updates: &[WorldUpdate]) {
        self.rays.apply(updates);
        self.exposure.apply(updates);
    }

    fn snapshot(&mut self) -> Arc<dyn Snapshot> {
        let exposure = self.exposure.blocks();
        self.rays.paired(exposure)
    }

    fn memory(&self) -> u64 {
        self.rays.memory() + self.exposure.memory()
    }
}

/// Chunk map mirrored from the update stream, in the chosen CPU storage (or two: the rays'
/// and the exposure tests').
pub struct CpuWorld {
    store: Box<dyn Store>,
}

fn chunks<R: Repr>() -> Chunks<R> {
    Chunks::<R> {
        chunks: HashMap::new(),
        grid: Arc::new(StorageGrid::default()),
        dirty: false,
    }
}

fn store<R: Repr>() -> Box<dyn Single> {
    Box::new(chunks::<R>())
}

impl CpuWorld {
    /// `storage`: a CPU storage component (validated by the caller).
    pub fn new(storage: &str) -> anyhow::Result<Self> {
        Ok(Self {
            store: single(storage)?,
        })
    }

    /// Rays walking storage `rays`, exposure tests reading storage `exposure` (one storage
    /// when they are the same component).
    pub fn paired(rays: &str, exposure: &str) -> anyhow::Result<Self> {
        if rays == exposure {
            return Self::new(rays);
        }
        Ok(Self {
            store: Box::new(Dyn {
                rays: single(rays)?,
                exposure: single(exposure)?,
            }),
        })
    }

    /// The world in storage `R` (a pipeline compiled for one storage names it).
    pub fn of<R: Repr>() -> Self {
        Self {
            store: Box::new(chunks::<R>()),
        }
    }

    /// Rays in storage `R`, exposure tests in storage `E` (a pipeline compiled for both).
    pub fn of_pair<R: Repr, E: Repr>() -> Self {
        Self {
            store: Box::new(Pair {
                rays: chunks::<R>(),
                exposure: chunks::<E>(),
            }),
        }
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

/// The chunk map of CPU storage `storage` (a component name).
fn single(storage: &str) -> anyhow::Result<Box<dyn Single>> {
    use crate::storage as s;
    Ok(match storage {
        "bricks" => store::<s::bricks::cpu::U16>(),
        "u8" => store::<s::u8::cpu::U8>(),
        "bits" => store::<s::bits::cpu::Bits>(),
        "hier" => store::<s::hier::cpu::Hier>(),
        "prefix" => store::<s::prefix::cpu::Prefix>(),
        "palette" => store::<s::palette::cpu::Palette>(),
        "brickmajor" => store::<s::brickmajor::Cpu>(),
        "morton" => store::<s::morton::Cpu>(),
        "hashed" => store::<s::hashed::cpu::Hashed>(),
        other => anyhow::bail!("storage `{other}` has no CPU implementation"),
    })
}
