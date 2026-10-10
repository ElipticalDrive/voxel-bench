//! Shading components: the colour of a hit. Lit / unlit is the pass mode, not a component;
//! a shading component decides how either is computed.

pub mod direct {
    pub mod cpu;
}

pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "direct" => include_str!("direct/gpu.wgsl"),
        _ => return None,
    })
}
