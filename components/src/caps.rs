//! Capabilities (PLAN step 6, IDEAS §17): every interface between stages has a required
//! core, which each component implements, plus optional capabilities. A component declares
//! the capabilities it provides and the ones it needs from other stages (`(slot, cap)`); a
//! feature (a knob such as LOD) can need capabilities too. A need is met by the component in
//! that slot, or by an *adapter*: compile-time code that gives the component the capability
//! from its core, slower. A need met by neither makes the combination invalid, with the
//! reason; nothing substitutes silently. Adapters are compiled in only where the chosen
//! component lacks the capability, so capabilities nobody needs cost nothing.
//!
//! So far the storage interface on the GPU (`storage/adapters.wgsl`). Capabilities are
//! declared per component, not per back end: `Lod` is gpu-compute's (CPU walks have no LOD).

use crate::{Composition, Slot, component, components};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cap {
    /// Storage: `s_dist(h, l)`, the Chebyshev distance to the nearest solid voxel.
    Dist,
    /// Storage: `s_row(h, x0, y, z)`, the occupancy of the 32 cells from x0 along x.
    Row,
    /// Storage: LOD chunks (`s_top_lod`, cells 2^lod blocks wide). Visibility: a walk that
    /// reads them with distance. Prepass: correct with them.
    Lod,
}

impl Cap {
    pub fn name(self) -> &'static str {
        match self {
            Cap::Dist => "s_dist",
            Cap::Row => "s_row",
            Cap::Lod => "lod",
        }
    }
}

/// Compile-time code that gives a `slot` component lacking `cap` the capability.
pub struct Adapter {
    pub slot: Slot,
    pub cap: Cap,
    pub description: &'static str,
}

/// Every adapter. Storage adapters are WGSL sections of `storage/adapters.wgsl`, named by
/// the capability (`//@s_dist`), appended in this order.
pub const ADAPTERS: &[Adapter] = &[
    Adapter { slot: Slot::Storage, cap: Cap::Dist, description: "`s_dist` from `s_probe`: the distance to the boundary of the empty aligned cube the storage reports" },
    Adapter { slot: Slot::Storage, cap: Cap::Row, description: "`s_row` from 32 voxel-level `s_occupied` queries" },
];

pub fn adapter(slot: Slot, cap: Cap) -> Option<&'static Adapter> {
    ADAPTERS.iter().find(|a| a.slot == slot && a.cap == cap)
}

/// Whether `slot` component `name` provides `cap` natively.
pub fn provides(slot: Slot, name: &str, cap: Cap) -> bool {
    component(slot, name).is_some_and(|c| c.provides.contains(&cap))
}

/// How a composition meets one need.
pub enum Met {
    /// The component in the slot provides it.
    Native,
    /// Through an adapter (correct, slower than a native implementation).
    Adapter(&'static Adapter),
    /// The back end has no such stage, so nothing there lacks it.
    NoStage,
}

/// How `c` meets need `(slot, cap)`, or why it can't.
pub fn meet(c: &Composition, slot: Slot, cap: Cap) -> Result<Met, String> {
    let name = c.get(slot);
    if name.is_empty() {
        return Ok(Met::NoStage);
    }
    if provides(slot, name, cap) {
        return Ok(Met::Native);
    }
    if let Some(a) = adapter(slot, cap) {
        return Ok(Met::Adapter(a));
    }
    let have: Vec<&str> = components(slot).filter(|x| x.provides.contains(&cap)).map(|x| x.name).collect();
    Err(format!("{} `{name}` lacks {} (have it: {})", slot.name(), cap.name(), if have.is_empty() { "none".into() } else { have.join(", ") }))
}

/// Why `c` cannot meet `needs`, if it can't (every unmet need).
pub fn lacks(c: &Composition, needs: &[(Slot, Cap)]) -> Option<String> {
    let why: Vec<String> = needs.iter().filter_map(|&(s, cap)| meet(c, s, cap).err()).collect();
    (!why.is_empty()).then(|| why.join("; "))
}

/// Why `c`'s components' needs cannot be met, if they can't.
pub fn unmet(c: &Composition) -> Option<String> {
    let why: Vec<String> = c
        .slots()
        .filter_map(|s| component(s, c.get(s)))
        .filter_map(|x| lacks(c, x.needs).map(|w| format!("`{}` needs it: {w}", x.name)))
        .collect();
    (!why.is_empty()).then(|| why.join("; "))
}

/// Advisory note for the needs of `c`'s components met through adapters.
pub fn adapter_note(c: &Composition) -> Option<String> {
    let notes: Vec<String> = c
        .slots()
        .filter_map(|s| component(s, c.get(s)))
        .flat_map(|x| x.needs.iter().map(move |&n| (x, n)))
        .filter_map(|(x, (s, cap))| match meet(c, s, cap) {
            Ok(Met::Adapter(a)) => Some(format!("`{}` answers {} for `{}` through an adapter ({}): correct, but slower than a {} with it built in", c.get(s), cap.name(), x.name, a.description, s.name())),
            _ => None,
        })
        .collect();
    (!notes.is_empty()).then(|| notes.join("\n"))
}
