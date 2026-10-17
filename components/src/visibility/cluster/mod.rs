//! Face-cluster discovery (the user's design, IDEAS §10): seed rays, then flood fills over
//! coplanar faces confirmed by corner rays. CPU only: [`cpu`].

pub mod cpu;
