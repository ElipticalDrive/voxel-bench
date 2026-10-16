//! Where components run (PLAN step 10, IDEAS §21): every component declares its
//! implementations per device kind and API ([`Impl`]), with the capabilities each one
//! provides (they differ: `bricks` has LOD on the GPU, not on the CPU). An API is how code
//! runs on a device: on the GPU a compute pass, a raster pass or a compute pass with ray
//! queries; on the CPU a hot loop compiled for the base x86-64 target (`scalar`) or for
//! AVX2 / AVX-512.
//!
//! An executor (a back end until step 13) declares where it runs each slot ([`Place`]: one
//! device kind and the APIs it may run that slot with); a component fills the slot if it
//! has an implementation of that kind covering every one of those APIs. Graph nodes carry an
//! optional `api` beside `device` (unset: the executor's choice), checked against both.

use serde::{Deserialize, Serialize};

use crate::Cap;

/// The kind of device a node runs on (a `Device` slot of the run's configuration is one).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Cpu,
    Gpu,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Cpu => "cpu",
            Kind::Gpu => "gpu",
        }
    }
}

/// How code runs on its device.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Api {
    /// wgpu compute pass (Vulkan here).
    Compute,
    /// wgpu raster pipeline (vertex and fragment shaders).
    Raster,
    /// wgpu compute pass with ray queries against an acceleration structure (RT cores).
    RayQuery,
    /// CPU, the base x86-64 target.
    Scalar,
    /// CPU, an AVX2 + FMA hot loop.
    Avx2,
    /// CPU, an AVX-512 (F, VL, BW) hot loop.
    Avx512,
}

impl Api {
    pub const ALL: [Api; 6] = [Api::Compute, Api::Raster, Api::RayQuery, Api::Scalar, Api::Avx2, Api::Avx512];

    pub fn name(self) -> &'static str {
        match self {
            Api::Compute => "compute",
            Api::Raster => "raster",
            Api::RayQuery => "rayquery",
            Api::Scalar => "scalar",
            Api::Avx2 => "avx2",
            Api::Avx512 => "avx512",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Api::Compute | Api::Raster | Api::RayQuery => Kind::Gpu,
            Api::Scalar | Api::Avx2 | Api::Avx512 => Kind::Cpu,
        }
    }
}

impl TryFrom<String> for Api {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        Api::ALL.into_iter().find(|a| a.name() == s).ok_or_else(|| format!("unknown api `{s}` (have: {})", Api::ALL.map(|a| a.name()).join(", ")))
    }
}

impl From<Api> for String {
    fn from(a: Api) -> String {
        a.name().into()
    }
}

/// One implementation of a component: the device kind, the APIs it runs under, and the
/// capabilities it provides there.
pub struct Impl {
    pub kind: Kind,
    pub apis: &'static [Api],
    pub provides: &'static [Cap],
}

impl Impl {
    /// `gpu: compute/raster` (for listings and reasons).
    pub fn label(&self) -> String {
        format!("{}: {}", self.kind.name(), self.apis.iter().map(|a| a.name()).collect::<Vec<_>>().join("/"))
    }
}

/// Where an executor runs a slot: a device kind and the APIs it may use there.
#[derive(Copy, Clone, Debug)]
pub struct Place {
    pub kind: Kind,
    pub apis: &'static [Api],
}

impl Place {
    pub fn label(&self) -> String {
        format!("{} {}", self.kind.name(), self.apis.iter().map(|a| a.name()).collect::<Vec<_>>().join("/"))
    }

    /// Whether `i` runs here: the same kind and every API of the place.
    pub fn takes(&self, i: &Impl) -> bool {
        i.kind == self.kind && self.apis.iter().all(|a| i.apis.contains(a))
    }
}

/// WGSL modules: plain functions any shader may call (compute, fragment, ray-query compute).
pub const SHADER: &[Api] = &[Api::Compute, Api::Raster, Api::RayQuery];
/// CPU code compiled into every hot-loop variant (generic Rust, monomorphised per ISA entry).
pub const ISAS: &[Api] = &[Api::Scalar, Api::Avx2, Api::Avx512];

pub const GPU_COMPUTE: Place = Place { kind: Kind::Gpu, apis: &[Api::Compute] };
pub const GPU_RASTER: Place = Place { kind: Kind::Gpu, apis: &[Api::Raster] };
pub const GPU_RAYQUERY: Place = Place { kind: Kind::Gpu, apis: &[Api::RayQuery] };
/// Shading of a face drawer: in the fragment shader (`raster`) or a deferred compute pass
/// (`visbuffer`, `gbuffer`); with it the storages and kernels its shadow rays walk.
pub const GPU_DRAWN: Place = Place { kind: Kind::Gpu, apis: &[Api::Raster, Api::Compute] };
/// gpu-compute's walk and its inline shading: a compute thread or a fragment shader per
/// pixel (knob `dispatch`, or the nodes' `api`).
pub const GPU_DISPATCH: Place = Place { kind: Kind::Gpu, apis: &[Api::Compute, Api::Raster] };
/// The `cpu` back end's hot loop: one ISA per run (knob `isa`, or the nodes' `api`).
pub const CPU_ISAS: Place = Place { kind: Kind::Cpu, apis: ISAS };
pub const CPU_SCALAR: Place = Place { kind: Kind::Cpu, apis: &[Api::Scalar] };

/// A back end's choice of API: the flat option that sets it, the stages it sets (all of
/// them run with one API), and the option's value per API. Unset, the option's default.
#[derive(Copy, Clone, Debug)]
pub struct ApiChoice {
    pub knob: &'static str,
    pub slots: &'static [crate::Slot],
    pub values: &'static [(Api, &'static str)],
    /// The API the option's default runs (`None`: chosen at run time, e.g. the best ISA).
    pub default: Option<Api>,
}

impl ApiChoice {
    pub fn value(&self, a: Api) -> Option<&'static str> {
        self.values.iter().find(|(x, _)| *x == a).map(|&(_, v)| v)
    }
}
