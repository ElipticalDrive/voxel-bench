//! The rule every pipeline compiled for one graph keeps: the knob
//! values it baked in (name, value as JSON) cannot change, at creation or live; another value
//! needs another build. The executors' `Spec`s list them; the generic builds bake none.

use crate::{InitParams, RendererInfo};

/// The knob values a compiled pipeline baked in (name, value as JSON).
pub type Knobs = &'static [(&'static str, &'static str)];

/// Fails if `value` (JSON) of knob `name` is not the one `baked` compiled in.
pub fn refuse(baked: &[(&str, &str)], name: &str, value: &serde_json::Value) -> anyhow::Result<()> {
    match baked.iter().find(|(k, _)| *k == name) {
        Some((_, v)) if serde_json::from_str::<serde_json::Value>(v).ok().as_ref() != Some(value) => {
            anyhow::bail!("this pipeline was compiled for {name} = {v} (not {value}); a change needs another build")
        }
        _ => Ok(()),
    }
}

/// Checks the creation options against what `baked` compiled in: each baked knob's value,
/// given or `info`'s default.
pub fn check(baked: &[(&str, &str)], p: &InitParams, info: fn() -> RendererInfo) -> anyhow::Result<()> {
    if baked.is_empty() {
        return Ok(());
    }
    let options: serde_json::Map<String, serde_json::Value> = if p.options_json.trim().is_empty() { Default::default() } else { serde_json::from_str(&p.options_json)? };
    let params = info().params;
    for (name, _) in baked {
        let value = options.get(*name).cloned().or_else(|| params.iter().find(|q| q.name == *name).map(|q| q.default.to_json()));
        refuse(baked, name, &value.ok_or_else(|| anyhow::anyhow!("baked knob `{name}` is not one of the executor's"))?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compiled pipeline refuses another value of a knob it baked in, and only that.
    #[test]
    fn baked_knobs_are_refused() {
        let knobs = &[("packet", "\"4\""), ("exact", "true")];
        assert!(refuse(knobs, "packet", &"4".into()).is_ok());
        let e = refuse(knobs, "packet", &"8".into()).unwrap_err().to_string();
        assert!(e.contains("compiled for packet = \"4\""), "{e}");
        assert!(refuse(knobs, "exact", &false.into()).is_err());
        assert!(refuse(knobs, "tile", &16.into()).is_ok(), "not baked: live");
    }
}
