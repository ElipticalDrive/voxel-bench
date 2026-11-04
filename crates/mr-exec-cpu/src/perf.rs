//! Minimal hardware-counter access through the raw `perf_event_open` syscall
//! (user-space only, so `perf_event_paranoid = 2` suffices). One counter set per rayon
//! worker thread, opened on that thread; enabled/read from the render thread via ioctl.
//! Linux only: elsewhere the knob is refused with the reason (Windows's
//! counters need a kernel driver or an administrator's ETW session).

const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_TYPE_HW_CACHE: u32 = 3;

/// (name, type, config)
pub const EVENTS: &[(&str, u32, u64)] = &[
    ("cycles", PERF_TYPE_HARDWARE, 0),
    ("instructions", PERF_TYPE_HARDWARE, 1),
    ("cache_refs", PERF_TYPE_HARDWARE, 2),
    ("cache_misses", PERF_TYPE_HARDWARE, 3),
    ("stall_backend", PERF_TYPE_HARDWARE, 8),
    ("l1d_miss", PERF_TYPE_HW_CACHE, read_miss(0)),
    ("dtlb_miss", PERF_TYPE_HW_CACHE, read_miss(3)),
];

/// A `PERF_TYPE_HW_CACHE` config: the cache (0 L1D, 3 dTLB), operation read (0), result miss (1).
const fn read_miss(cache: u64) -> u64 {
    cache | (1 << 16)
}

/// The `perf` knob's value, refused where this system has no counters.
pub fn allowed(on: bool) -> anyhow::Result<bool> {
    if on && !cfg!(target_os = "linux") {
        anyhow::bail!("perf: hardware counters need Linux's perf_event_open, not available on this system");
    }
    Ok(on)
}

#[cfg(target_os = "linux")]
pub use linux::Counters;

#[cfg(target_os = "linux")]
mod linux {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    use super::EVENTS;

    const IOC_ENABLE: libc::c_ulong = 0x2400;
    const IOC_DISABLE: libc::c_ulong = 0x2401;
    const IOC_RESET: libc::c_ulong = 0x2403;

    fn open(kind: u32, config: u64) -> Option<OwnedFd> {
        // perf_event_attr, PERF_ATTR_SIZE_VER8 = 136 bytes.
        let mut attr = [0u64; 17];
        attr[0] = kind as u64 | (136u64 << 32);
        attr[1] = config;
        attr[4] = 1 | 2; // read_format: TOTAL_TIME_ENABLED | TOTAL_TIME_RUNNING
        attr[5] = 1 | (1 << 5) | (1 << 6); // disabled, exclude_kernel, exclude_hv
        // SAFETY: valid attr buffer; pid 0 = calling thread, cpu -1 = any.
        let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, attr.as_ptr(), 0, -1, -1, 0u64) };
        if fd < 0 {
            return None;
        }
        // SAFETY: fresh fd owned by us.
        Some(unsafe { OwnedFd::from_raw_fd(fd as i32) })
    }

    /// Counters for every worker thread of a pool: `fds[thread][event]`.
    pub struct Counters {
        fds: Vec<Vec<Option<OwnedFd>>>,
    }

    impl Counters {
        pub fn open_on(pool: &rayon::ThreadPool) -> Option<Self> {
            let fds = pool.broadcast(|_| EVENTS.iter().map(|&(_, k, c)| open(k, c)).collect::<Vec<_>>());
            let ok = fds.first().map(|v| v.iter().filter(|f| f.is_some()).count()).unwrap_or(0);
            tracing::info!("perf counters: {ok}/{} events open on {} threads", EVENTS.len(), fds.len());
            Some(Self { fds })
        }

        fn ioctl_all(&self, req: libc::c_ulong) {
            for f in self.fds.iter().flatten().flatten() {
                // SAFETY: perf fd ioctl without argument.
                unsafe { libc::ioctl(f.as_raw_fd(), req, 0) };
            }
        }

        pub fn start(&self) {
            self.ioctl_all(IOC_RESET);
            self.ioctl_all(IOC_ENABLE);
        }

        /// Stops counting and returns per-event totals over all threads (multiplex-scaled);
        /// NaN for events that could not be opened.
        pub fn stop(&self) -> Vec<f64> {
            self.ioctl_all(IOC_DISABLE);
            let mut total = vec![0f64; EVENTS.len()];
            let mut seen = vec![false; EVENTS.len()];
            for t in &self.fds {
                for (e, f) in t.iter().enumerate() {
                    let Some(f) = f else { continue };
                    let mut buf = [0u64; 3];
                    // SAFETY: reading 24 bytes into a 24-byte buffer.
                    let n = unsafe { libc::read(f.as_raw_fd(), buf.as_mut_ptr().cast(), 24) };
                    if n == 24 && buf[2] > 0 {
                        total[e] += buf[0] as f64 * buf[1] as f64 / buf[2] as f64;
                        seen[e] = true;
                    }
                }
            }
            total.iter().zip(&seen).map(|(v, s)| if *s { *v } else { f64::NAN }).collect()
        }
    }
}

/// No counters here: `allowed` refuses the knob, so none is ever opened.
#[cfg(not(target_os = "linux"))]
pub enum Counters {}

#[cfg(not(target_os = "linux"))]
impl Counters {
    pub fn open_on(_: &rayon::ThreadPool) -> Option<Self> {
        None
    }

    pub fn start(&self) {
        match *self {}
    }

    pub fn stop(&self) -> Vec<f64> {
        match *self {}
    }
}
