//! Shading components: the colour of a surface point, `shade_surface(s, d, lit, px)` on the
//! GPU (`s`: the `Surface` a visibility strategy found, ray walks via `surface_of`; `d`: the
//! view ray; `px`: the pixel, for per-pixel state such as random sequences and
//! accumulation) and `shade` on the CPU (`direct::cpu`). Lit / unlit is
//! the pass mode, not a component; a shading component decides how either is computed.

pub mod direct {
    pub mod cpu;
}
pub mod state;

pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "direct" => include_str!("direct/gpu.wgsl"),
        "pathtrace" => include_str!("pathtrace/gpu.wgsl"),
        _ => return None,
    })
}
