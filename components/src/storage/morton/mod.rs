//! Storage `morton` (CPU; from cpu-simd-morton): `bricks` with ids in Z-order over the
//! chunk, see `storage::layout`.

pub type Cpu = super::layout::cpu::Laid<{ super::layout::cpu::MORTON }>;
