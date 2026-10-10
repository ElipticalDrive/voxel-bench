//! Storage `brickmajor` (CPU; from cpu-simd-brickmajor): `bricks` with each 4^3 brick's 64
//! ids contiguous (128 bytes), see `storage::layout`.

pub type Cpu = super::layout::cpu::Laid<{ super::layout::cpu::BRICK }>;
