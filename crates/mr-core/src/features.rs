//! What a pipeline supports, said one way: a typed set of features, each present or missing
//! with the reason. Executors declare their regions' features; a graph narrows them by what
//! its components lack (`mr_components::graph::Checked::narrow`). Plans, the GUI and sweeps
//! ask by it, and reports name what was asked and left out.

use serde::{Deserialize, Serialize};

use crate::{PassMode, RenderMode};

/// A thing a pipeline may support.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// Lit passes: the sun, point lights and the sky shade the surfaces (else albedo only).
    Lit,
    /// Meshes drawn with the voxels (`+meshes` passes).
    Meshes,
    /// Shading rays (shadows, bounces) meet the meshes, not only the voxels: lit `+meshes`
    /// frames without it show meshes lit by voxel shadows only.
    MeshRays,
    /// LOD chunks beyond level 0 (the client's coarser rings); up to [`Features::max_lod`].
    Lod,
    /// See-through blocks (glass, grates) shown with what is behind them, composited front
    /// to back. Asked by the world (one holding such blocks), not by the pass; pipelines
    /// without it draw them opaque.
    Transparency,
}

impl Feature {
    pub fn name(self) -> &'static str {
        match self {
            Feature::Lit => "lit",
            Feature::Meshes => "meshes",
            Feature::MeshRays => "mesh rays",
            Feature::Lod => "lod",
            Feature::Transparency => "transparency",
        }
    }
}

/// The features a pipeline has, and why it lacks the others it was declared or asked for.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Features {
    have: Vec<Feature>,
    /// Features its region has that this pipeline lacks, each with the reason.
    lacks: Vec<(Feature, String)>,
    /// LOD levels it holds, with [`Feature::Lod`].
    lod_levels: u8,
}

impl Features {
    /// A region's features (without LOD; see [`Features::with_lod`]).
    pub fn of(have: &[Feature]) -> Self {
        let mut have: Vec<Feature> = have.iter().copied().filter(|&f| f != Feature::Lod).collect();
        have.sort();
        have.dedup();
        Features { have, lacks: vec![], lod_levels: 0 }
    }

    /// With LOD levels up to `max` (none when 0).
    pub fn with_lod(mut self, max: u8) -> Self {
        self.have.retain(|&f| f != Feature::Lod);
        self.lod_levels = max;
        if max > 0 {
            self.have.push(Feature::Lod);
            self.have.sort();
        }
        self
    }

    pub fn has(&self, f: Feature) -> bool {
        self.have.contains(&f)
    }

    pub fn iter(&self) -> impl Iterator<Item = Feature> + '_ {
        self.have.iter().copied()
    }

    /// Takes `f` away with the reason (a feature it never had keeps the reason too).
    pub fn lack(&mut self, f: Feature, why: impl Into<String>) {
        self.have.retain(|&x| x != f);
        if f == Feature::Lod {
            self.lod_levels = 0;
        }
        if !self.lacks.iter().any(|(x, _)| *x == f) {
            self.lacks.push((f, why.into()));
        }
    }

    /// Why it lacks `f` (`None`: it has it).
    pub fn why_not(&self, f: Feature) -> Option<String> {
        if self.has(f) {
            return None;
        }
        Some(self.lacks.iter().find(|(x, _)| *x == f).map_or_else(|| "its executor does not build it".into(), |(_, w)| w.clone()))
    }

    /// The coarsest LOD level it holds (0: full-resolution chunks only).
    pub fn max_lod(&self) -> u8 {
        if self.has(Feature::Lod) { self.lod_levels } else { 0 }
    }

    /// Whether it draws passes of mode `m` at all.
    pub fn supports(&self, m: PassMode) -> bool {
        (m.lighting == RenderMode::Unlit || self.has(Feature::Lit)) && (!m.meshes || self.has(Feature::Meshes))
    }

    /// What a pass of mode `m` (`None`: an interactive view) with LOD levels up to `max_lod`,
    /// over a world asking the features `world` (see-through blocks: [`Feature::Transparency`]),
    /// asks of it that it leaves out, each with the reason.
    pub fn left_out(&self, m: Option<PassMode>, max_lod: u8, world: &[Feature]) -> Vec<String> {
        let mut out = vec![];
        let mut missing = |f: Feature| {
            if let Some(why) = self.why_not(f) {
                out.push(format!("{}: {why}", f.name()));
            }
        };
        if let Some(m) = m {
            let lit = m.lighting == RenderMode::Lit;
            if lit {
                missing(Feature::Lit);
            }
            if m.meshes {
                missing(Feature::Meshes);
            }
            if lit && m.meshes && self.has(Feature::Meshes) {
                missing(Feature::MeshRays);
            }
        }
        for &f in world {
            missing(f);
        }
        if max_lod > self.max_lod() {
            let why = self.why_not(Feature::Lod).unwrap_or_else(|| format!("it holds levels up to {}", self.max_lod()));
            out.push(format!("max_lod {max_lod}: {why}"));
        }
        out
    }

    /// One line: what it has, then what it lacks and why.
    pub fn describe(&self) -> String {
        let have: Vec<String> = self.iter().map(|f| if f == Feature::Lod { format!("lod<={}", self.lod_levels) } else { f.name().into() }).collect();
        let lacks: Vec<String> = self.lacks.iter().map(|(f, w)| format!("no {}: {w}", f.name())).collect();
        [have.join(","), lacks.join("; ")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrowing_and_left_out() {
        let mut f = Features::of(&[Feature::Lit, Feature::Meshes, Feature::MeshRays]).with_lod(6);
        assert_eq!(f.max_lod(), 6);
        let lit_meshes = PassMode { lighting: RenderMode::Lit, meshes: true };
        assert!(f.supports(lit_meshes) && f.left_out(Some(lit_meshes), 6, &[]).is_empty());
        f.lack(Feature::MeshRays, "draw `raster` shades forward");
        f.lack(Feature::Lod, "storage `tree64` lacks lod");
        assert!(f.supports(lit_meshes));
        assert_eq!(f.left_out(Some(lit_meshes), 2, &[]), vec!["mesh rays: draw `raster` shades forward".to_string(), "max_lod 2: storage `tree64` lacks lod".into()]);
        assert!(f.left_out(Some(PassMode::UNLIT), 0, &[]).is_empty());
        f.lack(Feature::Transparency, "visibility `raycast` lacks layers");
        assert!(f.supports(PassMode::UNLIT), "a world's ask skips no pass");
        assert_eq!(f.left_out(Some(PassMode::UNLIT), 0, &[Feature::Transparency]), vec!["transparency: visibility `raycast` lacks layers".to_string()]);
        let plain = Features::of(&[Feature::Lit]);
        assert!(!plain.supports(lit_meshes) && plain.supports(PassMode::LIT));
        assert_eq!(plain.left_out(None, 1, &[]), vec!["max_lod 1: its executor does not build it".to_string()]);
    }
}
