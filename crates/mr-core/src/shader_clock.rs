//! Time this thread spends turning shaders into GPU programs: composing and creating shader
//! modules (naga_oil's composition, naga's validation and translation) and creating pipelines
//! (the driver's compile). A renderer builds on its worker's thread, so the worker reads the
//! clock around construction and reports the split beside its whole build time (`init_ms`).
//! Per thread, so renderers in thread isolation do not mix their figures.

use std::cell::Cell;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Milliseconds spent, by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShaderTime {
    /// Composing and creating shader modules (naga_oil, naga validation and translation).
    pub module_ms: f32,
    /// Creating pipelines (the driver compiles here).
    pub pipeline_ms: f32,
    /// Modules and pipelines created.
    pub modules: u32,
    pub pipelines: u32,
}

impl ShaderTime {
    pub fn is_zero(&self) -> bool {
        self.modules == 0 && self.pipelines == 0
    }
}

thread_local! {
    static CLOCK: Cell<ShaderTime> = const { Cell::new(ShaderTime { module_ms: 0.0, pipeline_ms: 0.0, modules: 0, pipelines: 0 }) };
}

/// Runs `f`, counting its time as shader module work.
pub fn module<T>(f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f32() * 1e3;
    CLOCK.with(|c| {
        let mut v = c.get();
        v.module_ms += ms;
        v.modules += 1;
        c.set(v);
    });
    r
}

/// Runs `f`, counting its time as pipeline creation.
pub fn pipeline<T>(f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f32() * 1e3;
    CLOCK.with(|c| {
        let mut v = c.get();
        v.pipeline_ms += ms;
        v.pipelines += 1;
        c.set(v);
    });
    r
}

/// This thread's time so far, reset to zero.
pub fn take() -> ShaderTime {
    CLOCK.with(|c| c.replace(ShaderTime::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_per_kind_and_resets() {
        take();
        module(|| std::thread::sleep(std::time::Duration::from_millis(2)));
        pipeline(|| ());
        pipeline(|| ());
        let t = take();
        assert_eq!((t.modules, t.pipelines), (1, 2));
        assert!(t.module_ms >= 2.0);
        assert!(take().is_zero());
        // Another thread's work is its own.
        std::thread::spawn(|| module(|| ())).join().unwrap();
        assert!(take().is_zero());
    }
}
