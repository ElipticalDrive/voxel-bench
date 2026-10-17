//! The voxelspace executor (PLAN step 13d): Voxel Space (Comanche, 1992) style 2.5D
//! rendering, storage `heightmap` (top solid block height + block id per (x, z) column, in a
//! ring around the camera) marched by visibility `columns` (`mr_components::visibility::columns`:
//! per screen column front to back, spans drawn with a y-buffer), one executor for both of its
//! configurations:
//! - `gpu-voxelspace`: the march in a compute shader, shading `direct` or `pathtrace`, inline
//!   or deferred ([`gpu`]);
//! - `cpu-voxelspace`: the march rayon-parallel over columns, shading `direct`, inline or
//!   deferred ([`cpu`]).
//!
//! Which lane runs is the visibility node's device in the graph. The rest is shared
//! ([`Core`]): the heightmap storage, kept on the CPU by both and patched from the change log
//! (the GPU lane uploads its changed rows: the change log's transfer to the GPU), the frame
//! size and the timed stages' nodes.

use glam::{IVec2, IVec3};
use mr_components::graph::{self, Device, Role};
use mr_components::storage::heightmap::Heightmap;
use mr_core::*;

mod cpu;
mod gpu;

pub fn gpu_entry() -> RendererEntry {
    RendererEntry { info: gpu::info, create: |p, d, w| VoxelSpace::create(gpu::BACKEND, p, d, w) }
}

pub fn cpu_entry() -> RendererEntry {
    RendererEntry { info: cpu::info, create: |p, d, w| VoxelSpace::create(cpu::BACKEND, p, d, w) }
}

/// Both lanes' share.
pub(crate) struct Core {
    /// The heightmap storage.
    hm: Heightmap,
    /// Cells marched per column (2 * view radius in blocks).
    max_cells: u32,
    /// The nodes of the timed stages (PLAN step 11): the march (with shading when inline),
    /// the march alone, the deferred pass.
    stage_nodes: [Vec<String>; 3],
    width: u32,
    height: u32,
}

impl Core {
    fn counters(&self) -> [(String, f64); 2] {
        [("heightmap_size".into(), self.hm.size as f64), ("chunk_columns".into(), self.hm.columns() as f64)]
    }
}

/// A lane: the visibility node's implementation on its device, with the shading and the
/// world it reads there.
trait Lane: Sized + 'static {
    fn new(p: &InitParams, devices: &Devices, waker: Waker, core: &Core) -> anyhow::Result<Self>;
    /// Applies world changes to the lane's world; the chunks changed.
    fn apply(&mut self, updates: Vec<WorldUpdate>) -> Vec<IVec3>;
    /// A chunk of the lane's world (what the heightmap is built from).
    fn chunk(&self, pos: IVec3) -> Option<&ChunkData>;
    /// The heightmap changed in `tiles` (chunk columns).
    fn heights(&mut self, hm: &Heightmap, tiles: &[IVec2]);
    fn render(&mut self, core: &Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()>;
    fn poll(&mut self, _: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }
    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    fn resize(&mut self, core: &Core) -> anyhow::Result<()>;
    /// (CPU, GPU) bytes beyond the heightmap's CPU copy.
    fn memory(&self, core: &Core) -> (u64, u64);
}

pub struct VoxelSpace<L> {
    core: Core,
    lane: L,
}

impl VoxelSpace<()> {
    fn create(backend: &str, p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Box<dyn Renderer>> {
        let g = graph::of_init(backend, &p)?;
        let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("{backend}: no visibility node"))?;
        let core = Core {
            hm: Heightmap::new(p.view_radius_chunks),
            max_cells: p.view_radius_chunks * CHUNK_SIZE as u32 * 2,
            stage_nodes: [g.ids(&[Role::Visibility, Role::Shading]), g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            width: p.width,
            height: p.height,
        };
        Ok(match g.devices[vis] {
            Device::Cpu => Box::new(VoxelSpace { lane: cpu::CpuColumns::new(&p, devices, waker, &core)?, core }),
            Device::Gpu(_) => Box::new(VoxelSpace { lane: gpu::GpuColumns::new(&p, devices, waker, &core)?, core }),
        })
    }
}

impl<L: Lane> Renderer for VoxelSpace<L> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.lane.apply(updates);
        let lane = &self.lane;
        let tiles = self.core.hm.apply(changed, |pos| lane.chunk(pos));
        self.lane.heights(&self.core.hm, &tiles);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.lane.render(&self.core, req, sink)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        self.lane.set_param(name, value)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.lane.poll(sink)
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.lane.flush(sink)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.core.width = width;
        self.core.height = height;
        self.lane.resize(&self.core)
    }

    fn memory(&self) -> MemoryReport {
        let hm = self.core.hm.hm.len() as u64 * 4;
        let (cpu, gpu) = self.lane.memory(&self.core);
        MemoryReport { cpu_bytes: hm + cpu, gpu_bytes: gpu }
    }
}
