//! Fault injection, for testing that the host outlives any renderer: a renderer that fails
//! on demand at a chosen moment in a chosen way. Off unless the worker's environment has
//! `MR_FAULT_INJECTION=1` (tests set it; a plan alone cannot turn it on); then a renderer's
//! option `fault = "<at>:<how>"` wraps it in [`Faulty`].
//!
//! `at`: `build` (creating it), `first` (the first frame), `mid` (the tenth frame), `knob` (a
//! live parameter change), `world` (a world update), `shutdown` (the host's shutdown).
//! `how`: `error` (an `Err`), `panic`, `abort`, `segv` (a write through a null pointer),
//! `hang` (never returns), `oom` (allocates and touches memory until refused).

use anyhow::{Context, bail};
use mr_core::{FrameRequest, FrameSink, MeshUpdate, ParamValue, Renderer, WorldUpdate};

/// The environment variable that allows fault injection in this process.
pub const ENV: &str = "MR_FAULT_INJECTION";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum At {
    Build,
    First,
    Mid,
    Knob,
    World,
    Shutdown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum How {
    Error,
    Panic,
    Abort,
    Segv,
    Hang,
    Oom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fault {
    pub at: At,
    pub how: How,
}

impl Fault {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let (at, how) = s.split_once(':').context("fault takes `<at>:<how>`")?;
        let at = match at {
            "build" => At::Build,
            "first" => At::First,
            "mid" => At::Mid,
            "knob" => At::Knob,
            "world" => At::World,
            "shutdown" => At::Shutdown,
            _ => bail!("fault: unknown moment `{at}` (build, first, mid, knob, world, shutdown)"),
        };
        let how = match how {
            "error" => How::Error,
            "panic" => How::Panic,
            "abort" => How::Abort,
            "segv" => How::Segv,
            "hang" => How::Hang,
            "oom" => How::Oom,
            _ => bail!("fault: unknown way `{how}` (error, panic, abort, segv, hang, oom)"),
        };
        Ok(Self { at, how })
    }

    /// The fault asked for in `options_json` (its `fault` key), if injection is allowed here.
    pub fn of_options(options_json: &str) -> anyhow::Result<Option<Self>> {
        if std::env::var(ENV).as_deref() != Ok("1") {
            return Ok(None);
        }
        let v: serde_json::Value = serde_json::from_str(options_json).unwrap_or_default();
        v.get("fault").and_then(|f| f.as_str()).map(Self::parse).transpose()
    }

    /// Fails now if this is the moment.
    pub fn trip(&self, now: At) -> anyhow::Result<()> {
        if now != self.at {
            return Ok(());
        }
        let what = format!("injected fault at {:?}", self.at).to_lowercase();
        match self.how {
            How::Error => bail!("{what}"),
            How::Panic => panic!("{what}"),
            How::Abort => std::process::abort(),
            // SAFETY: none; it is the fault asked for.
            How::Segv => unsafe {
                std::ptr::write_volatile(std::ptr::null_mut::<u64>(), 1);
                unreachable!()
            },
            How::Hang => loop {
                tracing::warn!("{what}: hanging");
                std::thread::sleep(std::time::Duration::from_secs(3600));
            },
            How::Oom => {
                let mut held: Vec<Vec<u8>> = vec![];
                loop {
                    let mut v = Vec::<u8>::new();
                    if v.try_reserve_exact(1 << 28).is_err() {
                        bail!("{what}: allocation refused after {} MiB", held.len() * 256);
                    }
                    v.resize(1 << 28, 1);
                    held.push(v);
                }
            }
        }
    }
}

/// A renderer that fails as its [`Fault`] says, else does what `inner` does.
pub struct Faulty {
    pub inner: Box<dyn Renderer>,
    pub fault: Fault,
    frames: u64,
}

impl Faulty {
    pub fn new(inner: Box<dyn Renderer>, fault: Fault) -> Self {
        Self { inner, fault, frames: 0 }
    }
}

impl Renderer for Faulty {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        self.fault.trip(At::World)?;
        self.inner.update(updates)
    }
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.inner.update_meshes(update)
    }
    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.frames += 1;
        match self.frames {
            1 => self.fault.trip(At::First)?,
            10 => self.fault.trip(At::Mid)?,
            _ => {}
        }
        self.inner.render(req, sink)
    }
    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        self.fault.trip(At::Knob)?;
        self.inner.set_param(name, value)
    }
    fn edit(&mut self, edits: &str) -> anyhow::Result<()> {
        self.fault.trip(At::Knob)?;
        self.inner.edit(edits)
    }
    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.inner.poll(sink)
    }
    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.inner.flush(sink)
    }
    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.inner.resize(width, height)
    }
    fn memory(&self) -> mr_core::MemoryReport {
        self.inner.memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_trips_only_at_its_moment() {
        let f = Fault::parse("mid:error").unwrap();
        assert_eq!(f, Fault { at: At::Mid, how: How::Error });
        assert!(f.trip(At::First).is_ok());
        assert!(f.trip(At::Mid).unwrap_err().to_string().contains("injected fault at mid"));
        assert!(Fault::parse("mid").is_err() && Fault::parse("later:error").is_err() && Fault::parse("mid:explode").is_err());
        let p = Fault::parse("knob:panic").unwrap();
        assert!(std::panic::catch_unwind(|| p.trip(At::Knob)).is_err());
    }
}
