//! The post node: what turns the shading's linear colour
//! into the output's display colour, written once for every pipeline. Today: tone mapping,
//! compiled into the shading's final encode (fused: no pass, no image of its own), on the
//! GPU as the `mr::post` module (`display`), on the CPU as [`Tone::encode`].
//!
//! It maps linear colours: lit radiance (after the environment's exposure) and the sky.
//! Unlit pixels are texels, already display colours, and pass through unmapped.
//!
//! `hdr` maps nothing and limits nothing: the output becomes a float image holding extended
//! sRGB (the sRGB curve continued past 1). Every other writer of the output (the sky, unlit
//! texels, diagnostic views) writes the same display colours as before, which extended sRGB
//! holds unchanged; only the lit colours exceed 1.

pub use mr_core::lighting::Tone;

/// The `mr::post` module of tone mapper `t` (`display(c) -> vec4<f32>`, as [`Tone::encode`]).
pub fn wgsl(t: Tone) -> &'static str {
    match t {
        Tone::Clamp => mr_gpu::program::POST_WGSL,
        Tone::Reinhard => include_str!("reinhard.wgsl"),
        Tone::Aces => include_str!("aces.wgsl"),
        Tone::Hdr => include_str!("hdr.wgsl"),
    }
}

/// Replaces the program's `mr::post` module (`Program::world` adds `clamp`'s) with `t`'s;
/// `hdr` also defines `HDR` (the output is `rgba16float`).
pub fn add_to(t: Tone, p: &mut mr_gpu::program::Program) {
    p.add(mr_gpu::program::POST, wgsl(t), &[mr_gpu::program::COMMON]);
    if t == Tone::Hdr {
        p.define("HDR");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use mr_core::lighting::encode_srgb;

    #[test]
    fn tones() {
        let c = Vec3::new(0.2, 0.5, 4.0);
        assert_eq!(Tone::Clamp.encode(c), encode_srgb(c));
        for t in Tone::ALL {
            assert_eq!(Tone::parse(t.name()), Some(t));
            assert_eq!(t.map(Vec3::ZERO), Vec3::ZERO, "{t:?} keeps black");
        }
        // The curves compress: bright input stays near or below 1 (ACES's fit reaches 1.003
        // before the encode clamps) and keeps its order.
        assert_eq!(
            Tone::Hdr.encode(c),
            encode_srgb(c),
            "hdr's 8-bit form is clamp's"
        );
        for t in [Tone::Reinhard, Tone::Aces] {
            let (a, b) = (t.map(Vec3::splat(2.0)), t.map(Vec3::splat(8.0)));
            assert!(a.x < b.x && b.x <= 1.01, "{t:?}: {a} {b}");
        }
    }
}

/// The post node's knobs over the per-pixel quantities `qs` the graph's nodes write: `view`
/// (the image or one of them) and `view_max` (the ramps' top). Both executors' views; the
/// view is chosen at run time, never compiled into a pipeline.
pub fn view_knobs(qs: &[mr_core::Quantity]) -> [mr_core::ParamSpec; 2] {
    use mr_core::ParamSpec;
    let mut names = vec!["image"];
    names.extend(qs.iter().map(|q| q.name));
    let help: Vec<String> = std::iter::once("image: the rendered scene".to_string())
        .chain(qs.iter().map(|q| format!("{}: {}", q.name, q.help)))
        .collect();
    let tops: Vec<&str> = qs
        .iter()
        .filter(|q| !q.is_categorical())
        .map(|q| q.name)
        .collect();
    let how = match (
        tops.is_empty(),
        qs.iter().any(mr_core::Quantity::is_categorical),
    ) {
        (false, true) => {
            "Values are drawn on the shared colour scale (white above view_max), categories as tints of the greyed image"
        }
        (false, false) => "Drawn on the shared colour scale (white above view_max)",
        _ => "Drawn as tints of the greyed image",
    };
    let posts = crate::graph::POSTS;
    [
        ParamSpec::choice(
            "view",
            &format!("{}. {how}; the frame is still shaded", help.join(". ")),
            &names,
            "image",
            true,
        )
        .of("post", posts)
        .baked(false),
        ParamSpec::int(
            "view_max",
            &format!("{} view: the top of the colour scale", tops.join(" / ")),
            1,
            16384,
            256,
            true,
        )
        .of("post", posts),
    ]
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct ViewKnobs {
    view: Option<String>,
    view_max: Option<u32>,
}

/// The view and its scale's top at creation: the post node's knobs (where a plan's or the
/// editor's land) over the options' `view` / `max`.
pub fn view_chosen(g: &crate::graph::Checked, view: &str, max: u32) -> (String, u32) {
    let post = (g.node(&g.output).role == crate::graph::Role::Post)
        .then(|| g.knobs::<ViewKnobs>(&g.output).ok())
        .flatten()
        .unwrap_or_default();
    (
        post.view.unwrap_or_else(|| view.into()),
        post.view_max.unwrap_or(max).clamp(1, 16384),
    )
}

/// The quantity `name` among `qs` (`image`: none), or why not.
pub fn view_quantity(
    name: &str,
    qs: &[mr_core::Quantity],
) -> anyhow::Result<Option<mr_core::Quantity>> {
    if name == "image" {
        return Ok(None);
    }
    qs.iter()
        .find(|q| q.name == name)
        .copied()
        .map(Some)
        .ok_or_else(|| {
            let names: Vec<&str> = qs.iter().map(|q| q.name).collect();
            anyhow::anyhow!(
                "view `{name}`: this pipeline writes {}",
                if names.is_empty() {
                    "no quantities".into()
                } else {
                    format!("only `{}`", names.join("`, `"))
                }
            )
        })
}
