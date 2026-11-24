//! What a pipeline is given and computes when it differs from the full-resolution reference's.

use mr_core::PassMode;

use super::RunSpec;

/// What a pipeline is given of the world and the lighting model it computes, when they are
/// not the full-resolution reference's (another radius, LOD levels; path tracing; features
/// it declares it leaves out): a reference given the same tells a wrong pipeline from a
/// lossy, a noisy or a declared-partial one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Like {
    pub radius: u32,
    pub max_lod: u8,
    pub levels: mr_core::client::Levels,
    pub lighting: Lighting,
    pub leaves: Leaves,
}

/// What a pass asks that a pipeline declares it leaves out and its frames still show
/// otherwise: the reference draws them the same way, so the score judges what it claims.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Leaves {
    /// Lit `+meshes` frames whose shading rays see voxels only (meshes cast no shadows).
    pub mesh_rays: bool,
    /// See-through blocks drawn opaque in a world asking transparency.
    pub transparency: bool,
    /// The world as a heightmap: each column solid below its top block (2.5D).
    pub volume: bool,
}

impl Leaves {
    /// What a pipeline with `features` leaves out of a pass of mode `m` over a world asking
    /// `world`.
    pub fn of(features: &mr_core::Features, m: Option<PassMode>, world: &[mr_core::Feature]) -> Self {
        use mr_core::Feature;
        let lit_meshes = m.is_some_and(|m| m.lighting == mr_core::RenderMode::Lit && m.meshes);
        Leaves {
            mesh_rays: lit_meshes && features.has(Feature::Meshes) && !features.has(Feature::MeshRays),
            transparency: world.contains(&Feature::Transparency) && !features.has(Feature::Transparency),
            volume: !features.has(Feature::Volume),
        }
    }
}

/// The lighting model of a lit frame: the reference's direct lighting, or diffuse path
/// tracing (the `pathtrace` shading) with its bounces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lighting {
    #[default]
    Direct,
    Paths {
        bounces: u8,
    },
}

impl Like {
    /// The class of a client's region, if it differs from the full-resolution reference's
    /// (the run's radius at LOD 0).
    pub fn of(region: &crate::stream::Region, lighting: Lighting, leaves: Leaves, run: &RunSpec) -> Option<Self> {
        // Without coarse levels it does not matter how they would be made.
        let levels = if region.max_lod > 0 { region.levels } else { Default::default() };
        (region.max_lod > 0 || region.radius != run.view_radius || lighting != Lighting::Direct || leaves != Leaves::default()).then_some(Self { radius: region.radius, max_lod: region.max_lod, levels, lighting, leaves })
    }
}

impl std::fmt::Display for Like {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let levels = match self.levels {
            mr_core::client::Levels::Pick => "pick",
            mr_core::client::Levels::Derive => "derive",
        };
        match self.max_lod {
            0 => write!(f, "{levels}(radius={})", self.radius),
            l => write!(f, "{levels}(radius={}, max_lod={l})", self.radius),
        }?;
        match self.lighting {
            Lighting::Direct => Ok(()),
            Lighting::Paths { bounces } => write!(f, ", pathtrace(bounces={bounces})"),
        }?;
        if self.leaves.mesh_rays {
            write!(f, ", no mesh rays")?;
        }
        if self.leaves.transparency {
            write!(f, ", no transparency")?;
        }
        if self.leaves.volume {
            write!(f, ", heightmap")?;
        }
        Ok(())
    }
}
