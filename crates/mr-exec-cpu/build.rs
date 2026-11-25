//! The precompiled (generic) build's coverage, the user's build setting `MR_CPU_COVERAGE`
//! (docs/architecture.md, "Coverage is the user's choice"). Unset or `all`: everything. Else
//! space-separated narrowings, each `dimension=kept,values`:
//! - `packets=4,8`: the ray packet sizes compiled into the generic tile loop (of 4, 8, 16; 1, the
//!   `packet` knob's default, always stays, so the default graphs run);
//! - `isa=avx2`: the instruction sets dispatched to at run time (of avx2, avx512; scalar always
//!   stays, every CPU runs it).
//!
//! Storages, walks and shadings are always covered. A graph asking for what a narrowed build left
//! out fails to build, naming the setting; a compiled pipeline (`run.compile`) is unaffected.

const PACKETS: [&str; 4] = ["1", "4", "8", "16"];
const ISAS: [&str; 2] = ["avx2", "avx512"];
/// The `packet` knob's default (`raycast.rs`; its test checks they agree).
const DEFAULT_PACKET: &str = "1";

fn main() {
    println!("cargo::rerun-if-env-changed=MR_CPU_COVERAGE");
    for p in PACKETS {
        println!("cargo::rustc-check-cfg=cfg(cover_packet_{p})");
    }
    for i in ISAS {
        println!("cargo::rustc-check-cfg=cfg(cover_{i})");
    }
    let setting = std::env::var("MR_CPU_COVERAGE").unwrap_or_default();
    let (mut packets, mut isas) = (PACKETS.to_vec(), ISAS.to_vec());
    for part in setting.split_whitespace().filter(|p| *p != "all") {
        let Some((dim, values)) = part.split_once('=') else {
            panic!("MR_CPU_COVERAGE: `{part}` is not `dimension=values` (dimensions: packets, isa)");
        };
        let values: Vec<&str> = values.split(',').filter(|v| !v.is_empty()).collect();
        let (all, kept): (&[&str], &mut Vec<&str>) = match dim {
            "packets" => (&PACKETS, &mut packets),
            "isa" => (&ISAS, &mut isas),
            _ => panic!("MR_CPU_COVERAGE: no dimension `{dim}` (packets, isa)"),
        };
        let always = if dim == "isa" { "scalar" } else { DEFAULT_PACKET };
        if let Some(v) = values.iter().find(|v| !all.contains(v) && **v != always) {
            panic!("MR_CPU_COVERAGE: {dim} has no value `{v}` (of {all:?}; {always} always stays)");
        }
        kept.retain(|v| values.contains(v) || dim == "packets" && *v == DEFAULT_PACKET);
    }
    for p in &packets {
        println!("cargo::rustc-cfg=cover_packet_{p}");
    }
    for i in &isas {
        println!("cargo::rustc-cfg=cover_{i}");
    }
    println!("cargo::rustc-env=MR_CPU_COVERED=packets={} isa=scalar{}", packets.join(","), isas.iter().map(|i| format!(",{i}")).collect::<String>());
}
