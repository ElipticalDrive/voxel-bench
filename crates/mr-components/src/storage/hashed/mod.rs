//! Storage `hashed`: Voxel hashing (Niessner et al. 2013) inside a chunk: only non-empty 4^3 bricks are kept (64 u16 ids each), found through an open-addressed hash table keyed by brick coordinate; empty space costs no memory, every probe pays a hash lookup. CPU only.

pub mod cpu;
