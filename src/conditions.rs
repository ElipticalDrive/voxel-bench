//! The machine's conditions when a pass ends: what could explain a timing that jumped between
//! two runs of the same build (GPU clocks and power state, its temperature and throttling, the
//! power source and power profile, the CPU's clock). Sampled once per pass, right after its
//! last frame while the GPU is still clocked for it, outside the measured time. Each part is
//! optional: what this machine cannot tell stays empty. The numbers are metrics of their own
//! (`crate::metrics`, none better either way); the rest is kept as it was read.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Conditions {
    /// The GPU these are of (an NVIDIA GPU, by `nvidia-smi`: the first one it lists).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_sm_mhz: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_mem_mhz: Option<f64>,
    /// Its performance state (P0 the highest: 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_pstate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_power_w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_temp_c: Option<f64>,
    /// Why the GPU holds its clocks down (the driver's reasons, as it names them; none: not held).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gpu_throttle: Vec<String>,
    /// 1 on mains, 0 on battery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_ac: Option<f64>,
    /// The firmware's power profile (`performance`, `balanced`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_profile: Option<String>,
    /// The mean clock over the CPU's cores.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_mhz: Option<f64>,
}

impl Conditions {
    /// The conditions now (about 30-60 ms where `nvidia-smi` answers; instant otherwise).
    pub fn sample() -> Self {
        let mut c = Self::default();
        nvidia(&mut c);
        c.on_ac = on_ac();
        c.platform_profile = std::fs::read_to_string("/sys/firmware/acpi/platform_profile").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        c.cpu_mhz = cpu_mhz(&std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default());
        c
    }
}

const QUERY: &str = "name,clocks.sm,clocks.mem,pstate,power.draw,temperature.gpu,clocks_throttle_reasons.active";

fn nvidia(c: &mut Conditions) {
    let Ok(out) = std::process::Command::new("nvidia-smi").args([&format!("--query-gpu={QUERY}"), "--format=csv,noheader,nounits"]).output() else { return };
    if out.status.success() {
        parse_nvidia(&String::from_utf8_lossy(&out.stdout), c);
    }
}

/// The throttle reasons' bits as `nvidia-smi` documents them.
const REASONS: &[(u64, &str)] = &[
    (0x1, "gpu idle"),
    (0x2, "applications clocks setting"),
    (0x4, "software power cap"),
    (0x8, "hardware slowdown"),
    (0x10, "sync boost"),
    (0x20, "software thermal slowdown"),
    (0x40, "hardware thermal slowdown"),
    (0x80, "hardware power brake slowdown"),
    (0x100, "display clock setting"),
];

fn parse_nvidia(text: &str, c: &mut Conditions) {
    let Some(line) = text.lines().next() else { return };
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    if f.len() < 7 {
        return;
    }
    let num = |s: &str| s.parse::<f64>().ok();
    c.gpu = Some(f[0].to_string()).filter(|s| !s.is_empty());
    c.gpu_sm_mhz = num(f[1]);
    c.gpu_mem_mhz = num(f[2]);
    c.gpu_pstate = f[3].strip_prefix('P').and_then(num);
    c.gpu_power_w = num(f[4]);
    c.gpu_temp_c = num(f[5]);
    if let Ok(bits) = u64::from_str_radix(f[6].trim_start_matches("0x"), 16) {
        c.gpu_throttle = REASONS.iter().filter(|(b, _)| bits & b != 0).map(|(_, n)| n.to_string()).collect();
        let known: u64 = REASONS.iter().map(|(b, _)| b).sum();
        if bits & !known != 0 {
            c.gpu_throttle.push(format!("other {:#x}", bits & !known));
        }
    }
}

fn on_ac() -> Option<f64> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    let mains: Vec<bool> = dir
        .flatten()
        .filter(|e| std::fs::read_to_string(e.path().join("type")).is_ok_and(|t| t.trim() == "Mains"))
        .filter_map(|e| std::fs::read_to_string(e.path().join("online")).ok())
        .map(|s| s.trim() == "1")
        .collect();
    (!mains.is_empty()).then(|| if mains.iter().any(|&m| m) { 1.0 } else { 0.0 })
}

fn cpu_mhz(cpuinfo: &str) -> Option<f64> {
    let v: Vec<f64> = cpuinfo.lines().filter_map(|l| l.strip_prefix("cpu MHz")).filter_map(|r| r.split_once(':')).filter_map(|(_, v)| v.trim().parse().ok()).collect();
    (!v.is_empty()).then(|| (v.iter().sum::<f64>() / v.len() as f64).round())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nvidia_smi_and_cpuinfo() {
        let mut c = Conditions::default();
        parse_nvidia("NVIDIA GeForce RTX 5060 Laptop GPU, 877, 9001, P4, 24.24, 56, 0x0000000000000504\n", &mut c);
        assert_eq!((c.gpu_sm_mhz, c.gpu_mem_mhz, c.gpu_pstate, c.gpu_power_w, c.gpu_temp_c), (Some(877.0), Some(9001.0), Some(4.0), Some(24.24), Some(56.0)));
        assert_eq!(c.gpu_throttle, ["software power cap", "display clock setting", "other 0x400"], "an unnamed bit stays, by value");
        let mut d = Conditions::default();
        parse_nvidia("x, [N/A], 1, P0, [N/A], 40, 0x0\n", &mut d);
        assert_eq!((d.gpu_sm_mhz, d.gpu_pstate, d.gpu_power_w), (None, Some(0.0), None));
        assert!(d.gpu_throttle.is_empty());
        assert_eq!(cpu_mhz("cpu MHz\t\t: 1000.5\ncpu MHz\t\t: 3000.5\n"), Some(2001.0));
        assert_eq!(cpu_mhz(""), None);
    }
}
