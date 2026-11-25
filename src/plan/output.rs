//! Where a run's results go: the window, read-back, saved frames, comparison to a reference.

use mr_core::{Choice, Choices, PassMode};
use serde::{Deserialize, Serialize};

use super::RendererSpec;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct OutputSpec {
    /// Show frames live in a window.
    pub window: bool,
    /// `auto`: read frames back only when an output needs them. `always`: every frame
    /// (frames delivered to host memory; measures read-back cost). `never`.
    pub readback: Readback,
    pub save: Option<SaveSpec>,
    pub compare: Option<CompareSpec>,
    /// Where run directories go (default `runs/`).
    pub dir: Option<String>,
    /// Continue the newest unfinished run of this plan in `dir` (one that died part-way: its
    /// journal, no report) instead of starting a new one; a new one when there is none.
    pub resume: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Readback {
    #[default]
    Auto,
    Always,
    Never,
}

impl Choices for Readback {
    const CHOICES: &'static [Choice<Self>] = &[
        Choice { name: "auto", make: || Readback::Auto, about: "Only when a frame is needed (saving, comparison, window)" },
        Choice { name: "always", make: || Readback::Always, about: "Every frame: measures the cost of getting pixels out, like a real app would pay" },
        Choice { name: "never", make: || Readback::Never, about: "Never: pure render speed" },
    ];
    fn choice(&self) -> usize {
        match self {
            Readback::Auto => 0,
            Readback::Always => 1,
            Readback::Never => 2,
        }
    }
}

#[cfg(test)]
mod choice_tests {
    use super::*;

    /// The run's and the output's pickers list every variant once, by the name plans write.
    #[test]
    fn run_choices_are_complete() {
        fn plain<T: Serialize>(v: &T) -> String {
            serde_json::to_value(v).unwrap().as_str().unwrap().to_string()
        }
        mr_core::choice::check::<Readback>(plain);
        mr_core::choice::check::<super::super::run::Compile>(plain);
        mr_core::choice::check::<super::super::run::Isolation>(plain);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSpec {
    /// Save every n-th measured frame (0 = only the last).
    #[serde(default = "default_every")]
    pub every: u32,
    /// Also save amplified difference images against the reference.
    #[serde(default = "yes")]
    pub diffs: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompareSpec {
    /// Renderer producing the reference images (rendered in its own pass first).
    #[serde(default = "default_reference")]
    pub reference: String,
    #[serde(default)]
    pub reference_options: toml::Table,
    /// Compare every n-th measured frame.
    #[serde(default = "default_every")]
    pub every: u32,
    /// Frames a compared frame is held still (camera and world) before a pipeline with
    /// late edges is scored once more, settled (0: no settled score).
    #[serde(default = "default_settle")]
    pub settle: u32,
    /// Paths per pixel a compared frame accumulates, held still, before a pipeline that
    /// accumulates lit frames (`pathtrace` with `accumulate`) is scored settled (0: none).
    #[serde(default = "default_converge")]
    pub converge: u32,
}

impl CompareSpec {
    /// Renders a compared frame of `spec` is held still in its settled pass in mode `m`
    /// (0: no settled pass): `settle` for late edges, enough for `converge` paths per pixel
    /// where it accumulates, the larger.
    pub fn held(&self, spec: &RendererSpec, m: PassMode) -> u32 {
        let late = if spec.late().is_empty() { 0 } else { self.settle };
        // The compared frame's own render is the first of them.
        let converge = spec.accumulates(m).map_or(0, |spp| self.converge.div_ceil(spp).saturating_sub(1));
        late.max(converge)
    }
}

fn default_settle() -> u32 {
    2
}

fn default_converge() -> u32 {
    64
}

fn default_every() -> u32 {
    60
}
fn yes() -> bool {
    true
}
fn default_reference() -> String {
    "cpu-reference".into()
}
