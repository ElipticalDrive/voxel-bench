//! Renderer parameters ("knobs"): declared by each renderer so the host, the GUI and the
//! auto-tuner can discover, show and change them.
//!
//! A parameter's initial value comes from the plan's renderer `options` (same key). `live`
//! parameters can then be changed between frames with [`crate::Renderer::set_param`];
//! the others need a restart (the live session hot-swaps a fresh worker).
//!
//! Independently, a parameter is *baked* (specialisable) or not: a
//! baked knob's value may be compiled in as a constant where a pipeline is compiled for its
//! values (GPU shader constants; CPU pipelines compiled on demand), so a different value is
//! a different compiled pipeline and part of its cache key. The generic builds still read
//! it at run time (and may change it live). By default switches and choices are baked (they
//! select code) and numbers are not (quantities read at run time); [`ParamSpec::baked`]
//! overrides it per knob (e.g. a loop count such as `bounces`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Choice(String),
}

impl ParamValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ParamValue::Int(i) => Some(*i),
            ParamValue::Float(f) => Some(f.round() as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ParamValue::Int(i) => Some(*i as f64),
            ParamValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            ParamValue::Choice(s) => Some(s),
            _ => None,
        }
    }

    /// Value as it would appear in a JSON/TOML options table.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            ParamValue::Bool(b) => (*b).into(),
            ParamValue::Int(i) => (*i).into(),
            ParamValue::Float(f) => (*f).into(),
            ParamValue::Choice(s) => s.clone().into(),
        }
    }

    pub fn display(&self) -> String {
        match self {
            ParamValue::Bool(b) => b.to_string(),
            ParamValue::Int(i) => i.to_string(),
            ParamValue::Float(f) => format!("{f:.3}"),
            ParamValue::Choice(s) => s.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamKind {
    Bool,
    Int { min: i64, max: i64 },
    Float { min: f64, max: f64, log: bool },
    Choice(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    pub description: String,
    pub kind: ParamKind,
    pub default: ParamValue,
    /// Changeable between frames without restarting the renderer.
    pub live: bool,
    /// The renderer component this knob belongs to: (slot, components it applies to), e.g.
    /// `("visibility", ["incremental"])`. `None`: a knob of the renderer / back end itself.
    /// Lets UIs show each knob next to the component that uses it.
    /// (Always serialised: renderer infos cross the worker boundary as postcard, which
    /// cannot skip fields.)
    #[serde(default)]
    pub owner: Option<(String, Vec<String>)>,
    /// Specialisable: compiled pipelines may bake its value in (see the module docs).
    #[serde(default)]
    pub baked: bool,
}

impl ParamSpec {
    pub fn bool(name: &str, description: &str, default: bool, live: bool) -> Self {
        Self { name: name.into(), description: description.into(), kind: ParamKind::Bool, default: ParamValue::Bool(default), live, owner: None, baked: true }
    }

    pub fn int(name: &str, description: &str, min: i64, max: i64, default: i64, live: bool) -> Self {
        Self { name: name.into(), description: description.into(), kind: ParamKind::Int { min, max }, default: ParamValue::Int(default), live, owner: None, baked: false }
    }

    pub fn float(name: &str, description: &str, min: f64, max: f64, log: bool, default: f64, live: bool) -> Self {
        Self { name: name.into(), description: description.into(), kind: ParamKind::Float { min, max, log }, default: ParamValue::Float(default), live, owner: None, baked: false }
    }

    pub fn choice(name: &str, description: &str, options: &[&str], default: &str, live: bool) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            kind: ParamKind::Choice(options.iter().map(|s| s.to_string()).collect()),
            default: ParamValue::Choice(default.into()),
            live,
            owner: None,
            baked: true,
        }
    }

    /// Declares the knob specialisable or not, against its kind's default.
    pub fn baked(mut self, baked: bool) -> Self {
        self.baked = baked;
        self
    }

    /// Marks the knob as belonging to `components` of `slot` (see `owner`).
    pub fn of(mut self, slot: &str, components: &[&str]) -> Self {
        self.owner = Some((slot.into(), components.iter().map(|c| c.to_string()).collect()));
        self
    }

    /// Whether the knob belongs to component `component` of `slot`.
    pub fn owned_by(&self, slot: &str, component: &str) -> bool {
        self.owner.as_ref().is_some_and(|(s, cs)| s == slot && cs.iter().any(|c| c == component))
    }

    /// Coerces and clamps a value into this parameter's domain.
    pub fn clamp(&self, v: &ParamValue) -> Option<ParamValue> {
        Some(match &self.kind {
            ParamKind::Bool => ParamValue::Bool(v.as_bool()?),
            ParamKind::Int { min, max } => ParamValue::Int(v.as_i64()?.clamp(*min, *max)),
            ParamKind::Float { min, max, .. } => ParamValue::Float(v.as_f64()?.clamp(*min, *max)),
            ParamKind::Choice(opts) => {
                let s = v.as_str()?;
                if !opts.iter().any(|o| o == s) {
                    return None;
                }
                ParamValue::Choice(s.into())
            }
        })
    }

    /// Reads this parameter from a JSON value (e.g. from plan options).
    pub fn from_json(&self, v: &serde_json::Value) -> Option<ParamValue> {
        let raw = match &self.kind {
            ParamKind::Bool => ParamValue::Bool(v.as_bool()?),
            ParamKind::Int { .. } => ParamValue::Int(v.as_i64().or_else(|| v.as_f64().map(|f| f.round() as i64))?),
            ParamKind::Float { .. } => ParamValue::Float(v.as_f64()?),
            ParamKind::Choice(_) => ParamValue::Choice(v.as_str()?.into()),
        };
        self.clamp(&raw)
    }
}
