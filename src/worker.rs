//! Spawning and talking to renderer workers.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;

use mr_core::lighting::PixelFormat;
use mr_core::{BlockRegistry, Environment, InitParams, RendererInfo};
use mr_ipc::{FrameSlots, HostMsg, PROTOCOL_VERSION, WorkerMsg};

use crate::plan::{Isolation, RendererSpec, RunSpec, Watchdog};

pub enum WorkerEvent {
    Msg(WorkerMsg),
    Closed,
}

pub struct Worker {
    pub label: String,
    pub info: RendererInfo,
    pub pid: u32,
    pub init_ms: f32,
    /// The part of `init_ms` spent making shader modules and pipelines.
    pub shaders: mr_core::shader_clock::ShaderTime,
    pub shm: FrameSlots,
    /// The frames' pixel format (the pipeline's post: `hdr` is float).
    pub format: PixelFormat,
    free_slots: Vec<u32>,
    tx: Sender<HostMsg>,
    pub rx: Receiver<WorkerEvent>,
    child: Option<Child>,
    thread: Option<std::thread::JoinHandle<()>>,
    watchdog: Watchdog,
    /// The longest the host has waited on it so far (its build included); None until its
    /// first frame came back (it is still building: the build allowance applies).
    slowest: Option<Duration>,
    /// The longest wait while it was building (its init, initial load, first frame).
    building: Duration,
    /// It missed a deadline: killed (a process) or abandoned (a thread).
    hung: bool,
}

pub struct SpawnConfig<'a> {
    /// The worker's entry: the executor (`cpu`, `gpu`) a graph runs on, or a standalone
    /// renderer.
    pub renderer: &'a str,
    pub label: &'a str,
    pub params: InitParams,
    pub isolation: Isolation,
    pub launcher: &'a [String],
    pub log_path: Option<PathBuf>,
    pub slots: u32,
    /// A compiled pipeline's worker (`pipelines`) instead of this executable.
    pub exe: Option<PathBuf>,
    pub watchdog: Watchdog,
}

/// Spawns a worker for `spec` using the plan's run settings; `world` is the world's
/// generator, which its client runs if the graph places the generator there.
pub fn spawn_renderer(
    run: &RunSpec,
    spec: &RendererSpec,
    world: &mr_world::GenSpec,
    blocks: Arc<BlockRegistry>,
    log_path: Option<PathBuf>,
    resolution: [u32; 2],
    slots: u32,
) -> anyhow::Result<Worker> {
    let [width, height] = resolution;
    // A pipeline compiled for this machine, if asked for and the renderer has one.
    let exe = match (
        spec.compile.unwrap_or(run.compile),
        crate::pipelines::bake(spec)?,
    ) {
        (level @ (crate::plan::Compile::Native | crate::plan::Compile::BreakIn), Some(b)) => {
            if run.isolation != Isolation::Process {
                anyhow::bail!(
                    "{}: compiled pipelines run in their own process (isolation = \"process\")",
                    spec.label()
                );
            }
            let c = crate::pipelines::trusted(&b, level)?;
            if let Some(s) = c.build_s {
                tracing::info!("{}: compiled pipeline {} in {s:.1} s", spec.label(), c.key);
            }
            Some(c.exe)
        }
        _ => None,
    };
    let graph = spec.worker_graph()?;
    let params = InitParams {
        width,
        height,
        blocks,
        // The world's comes over the stream (`SetEnvironment` first).
        environment: Environment::default(),
        view_radius_chunks: spec.view_radius(run),
        max_lod: spec.max_lod(run, u8::MAX),
        options_json: serde_json::to_string(&spec.options)?,
        graph_json: graph
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_default(),
        devices: spec.devices(run),
        stage_times: run.stage_times,
        client: spec.levels(),
        link: spec.link(),
    };
    let w = Worker::spawn(SpawnConfig {
        renderer: spec.entry(),
        label: spec.label(),
        params,
        isolation: run.isolation,
        launcher: &run.launcher,
        log_path,
        slots,
        exe,
        watchdog: run.watchdog.clone(),
    })?;
    if spec.generates() {
        w.send(HostMsg::Generate {
            world: serde_json::to_string(world)?,
            budget: run.budget().map(|b| b as u32),
        })?;
    }
    Ok(w)
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// This executable, to start as a child (a worker, presenter, check run): `MR_WORKER_EXE`
/// where set (tests, whose own executable is the test harness), else on Linux
/// `/proc/self/exe`, the running file even after a rebuild replaced it on disk (its path then
/// names nothing: a sweep's workers failed so), so parent and child stay one build.
pub fn self_exe() -> anyhow::Result<PathBuf> {
    if let Some(e) = std::env::var_os("MR_WORKER_EXE") {
        return Ok(PathBuf::from(e));
    }
    if cfg!(target_os = "linux") {
        return Ok(PathBuf::from("/proc/self/exe"));
    }
    Ok(std::env::current_exe()?)
}

impl Worker {
    pub fn spawn(cfg: SpawnConfig) -> anyhow::Result<Self> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tag = format!("multirenderer-{}-{n}", std::process::id());
        let sock = mr_ipc::endpoint(&tag);
        let listener = mr_ipc::listen(&sock)?;

        let mut child = None;
        let mut thread = None;
        match cfg.isolation {
            Isolation::Process => {
                // A compiled pipeline's worker, else this executable.
                let exe = match &cfg.exe {
                    Some(e) => e.clone(),
                    None if cfg.launcher.is_empty() => self_exe()?,
                    // A launcher runs it by its path (`/proc/self/exe` would be the launcher).
                    None => std::env::var_os("MR_WORKER_EXE")
                        .map(PathBuf::from)
                        .map_or_else(std::env::current_exe, Ok)?,
                };
                let mut cmd = match cfg.launcher.split_first() {
                    Some((first, rest)) => {
                        let mut c = Command::new(first);
                        c.args(rest).arg(&exe);
                        c
                    }
                    None => Command::new(&exe),
                };
                cmd.arg("worker")
                    .arg("--socket")
                    .arg(&sock)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());
                let mut c = cmd
                    .spawn()
                    .with_context(|| format!("spawning worker for {}", cfg.renderer))?;
                forward_stderr(
                    c.stderr.take().unwrap(),
                    cfg.label.to_string(),
                    cfg.log_path.clone(),
                );
                child = Some(c);
            }
            Isolation::Thread => {
                let s = sock.clone();
                let label = cfg.label.to_string();
                thread = Some(
                    std::thread::Builder::new()
                        .name(format!("worker-{label}"))
                        .spawn(move || {
                            if let Err(e) = mr_worker::run(&s) {
                                tracing::error!("[{label}] worker failed: {e:#}");
                            }
                        })?,
                );
            }
        }

        // Connecting, the handshake and the build share the build allowance.
        let build = Duration::from_secs_f64(cfg.watchdog.build_s.max(0.0));
        let until = Instant::now() + build;
        let left = || until.saturating_duration_since(Instant::now());
        // Silence past it is a hang: said so, and a process killed (dropping a `Child` would
        // leave it running) or a thread abandoned.
        let fate = if child.is_some() {
            "its process killed"
        } else {
            "its thread abandoned"
        };
        let hung = |e: anyhow::Error| {
            if e.to_string().starts_with("worker timed out") {
                anyhow::anyhow!(
                    "hung while building: no answer within {build:.1?} (run.watchdog.build_s); {fate}"
                )
            } else {
                e
            }
        };
        let mut child = KillOnDrop(child);
        let stream = accept(&listener, &mut child.0, build)?;
        drop(listener);
        let (mut input, mut output) = mr_ipc::split::<WorkerMsg, HostMsg>(stream);

        let (ev_tx, rx) = crossbeam_channel::unbounded();
        std::thread::Builder::new()
            .name(format!("rx-{}", cfg.label))
            .spawn(move || {
                loop {
                    match input.recv() {
                        Ok(Some(m)) => {
                            if ev_tx.send(WorkerEvent::Msg(m)).is_err() {
                                return;
                            }
                        }
                        _ => {
                            let _ = ev_tx.send(WorkerEvent::Closed);
                            return;
                        }
                    }
                }
            })?;
        let (tx, msg_rx) = crossbeam_channel::unbounded::<HostMsg>();
        std::thread::Builder::new()
            .name(format!("tx-{}", cfg.label))
            .spawn(move || {
                for m in msg_rx {
                    if output.send(&m).is_err() {
                        return;
                    }
                }
            })?;

        let pid = match recv_timeout(&rx, left()).map_err(hung)? {
            WorkerMsg::Hello { pid, protocol, .. } if protocol == PROTOCOL_VERSION => pid,
            WorkerMsg::Hello { protocol, .. } => {
                bail!("worker speaks protocol {protocol}, host {PROTOCOL_VERSION}")
            }
            m => bail!("unexpected first message {m:?}"),
        };
        // A standalone renderer (no graph) writes RGBA8.
        let format = mr_components::graph::of_init(cfg.renderer, &cfg.params)
            .map_or(PixelFormat::Rgba8, |g| g.tone().format());
        let slot_bytes = (cfg.params.width * cfg.params.height * format.bytes()) as u64;
        let mut shm = FrameSlots::create(&tag, cfg.slots, slot_bytes)?;
        tx.send(HostMsg::Init {
            renderer: cfg.renderer.to_string(),
            params: cfg.params,
            shm: shm.spec().clone(),
        })?;
        let ready = recv_timeout(&rx, left()).map_err(|e| {
            // A process gone while building says how (a signal names a crash it could not report).
            let closed = e.to_string().starts_with("worker exited");
            match child.0.as_mut().and_then(|c| {
                if closed {
                    c.wait().ok()
                } else {
                    c.try_wait().ok().flatten()
                }
            }) {
                Some(status) => e.context(format!("worker process ended: {status}")),
                None => hung(e),
            }
        });
        let (info, init_ms, shaders) = match ready? {
            WorkerMsg::Ready {
                info,
                init_ms,
                shaders,
            } => (info, init_ms, shaders),
            m => bail!("[{}] expected Ready, got {m:?}", cfg.label),
        };
        // Both sides have it mapped now; unlinking means nothing leaks if we're killed.
        shm.unlink();
        Ok(Self {
            label: cfg.label.to_string(),
            info,
            pid,
            init_ms,
            shaders,
            free_slots: (0..cfg.slots).rev().collect(),
            shm,
            format,
            tx,
            rx,
            child: child.0.take(),
            thread,
            watchdog: cfg.watchdog,
            slowest: None,
            building: Duration::from_secs_f64(init_ms.max(0.0) as f64 / 1e3),
            hung: false,
        })
    }

    /// How long the next wait on it may last before it counts as hung ([`Watchdog`]).
    pub fn deadline(&self) -> Duration {
        match self.slowest {
            None => Duration::from_secs_f64(self.watchdog.build_s.max(0.0)),
            Some(s) => s
                .mul_f64(self.watchdog.factor.max(1.0))
                .max(Duration::from_secs_f64(self.watchdog.min_s.max(0.0))),
        }
    }

    /// A wait on it that ended after `took`: the deadlines follow the longest.
    pub fn observe(&mut self, took: Duration) {
        match &mut self.slowest {
            None => self.building = self.building.max(took),
            Some(s) => *s = (*s).max(took),
        }
    }

    /// Its first frame came back: it has built, and later deadlines derive from its timing.
    pub fn built(&mut self) {
        if self.slowest.is_none() {
            self.slowest = Some(self.building);
        }
    }

    /// Whether it missed a deadline (and was killed or abandoned).
    pub fn is_hung(&self) -> bool {
        self.hung
    }

    /// The next message, within [`Self::deadline`]; past it the worker is hung: a process is
    /// killed (freeing what it held), a thread abandoned (it cannot be stopped: that
    /// isolation's cost), and the error says so.
    pub fn recv(&mut self) -> anyhow::Result<WorkerMsg> {
        let d = self.deadline();
        let t = Instant::now();
        let r = recv_timeout(&self.rx, d);
        if r.as_ref()
            .is_err_and(|e| e.to_string().starts_with("worker timed out"))
        {
            self.check_hung(d.max(t.elapsed()))?;
        }
        if r.is_ok() {
            self.observe(t.elapsed());
        }
        r
    }

    /// For a host that polls (the GUI): an error, once it has waited `waited` for an answer
    /// past [`Self::deadline`], with the worker killed (a process) or abandoned (a thread).
    pub fn check_hung(&mut self, waited: Duration) -> anyhow::Result<()> {
        let d = self.deadline();
        if waited < d {
            return Ok(());
        }
        self.hung = true;
        let fate = match &mut self.child {
            Some(c) => {
                let _ = c.kill();
                let _ = c.wait();
                "its process killed"
            }
            None => "its thread abandoned",
        };
        let why = if self.slowest.is_some() {
            format!("{}x its slowest wait so far", self.watchdog.factor)
        } else {
            "the build allowance".into()
        };
        bail!("hung: no answer within {d:.1?} ({why}, run.watchdog); {fate}")
    }

    pub fn send(&self, m: HostMsg) -> anyhow::Result<()> {
        self.tx
            .send(m)
            .map_err(|_| anyhow::anyhow!("[{}] worker connection closed", self.label))
    }

    /// Changes the render resolution. All frames in flight must have completed (their
    /// slots returned); a fresh shared-memory region is created for the new size.
    pub fn resize(&mut self, width: u32, height: u32, slots: u32) -> anyhow::Result<()> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut shm = FrameSlots::create(
            &format!("multirenderer-{}-{n}", std::process::id()),
            slots,
            (width * height * self.format.bytes()) as u64,
        )?;
        self.send(HostMsg::Resize {
            width,
            height,
            shm: shm.spec().clone(),
        })?;
        // The worker maps it while handling Resize; Flushed below proves it has.
        self.send(HostMsg::Flush)?;
        loop {
            match self.recv()? {
                WorkerMsg::Flushed => break,
                WorkerMsg::FrameDone { .. } => {}
                _ => {}
            }
        }
        shm.unlink();
        self.shm = shm;
        self.free_slots = (0..slots).rev().collect();
        Ok(())
    }

    pub fn take_slot(&mut self) -> Option<u32> {
        self.free_slots.pop()
    }

    pub fn free_slot(&mut self, s: u32) {
        self.free_slots.push(s);
    }

    /// The frame of `pixels` pixels in slot `s` (freed), in [`Self::format`].
    pub fn take_frame(&mut self, s: u32, pixels: usize) -> Vec<u8> {
        let px = self.shm.slot(s)[..pixels * self.format.bytes() as usize].to_vec();
        self.free_slot(s);
        px
    }

    /// Blocks until the worker answers `Flushed`, handing other messages to `on_msg`.
    pub fn flush(
        &mut self,
        mut on_msg: impl FnMut(WorkerMsg) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.send(HostMsg::Flush)?;
        loop {
            match self.recv() {
                Ok(WorkerMsg::Flushed) => return Ok(()),
                Ok(m) => on_msg(m)?,
                Err(e) => return Err(e.context(format!("[{}] waiting for flush", self.label))),
            }
        }
    }

    /// Peak resident set size of the worker process, in bytes (process isolation only).
    pub fn peak_rss(&self) -> Option<u64> {
        self.proc_status("VmHWM:")
    }

    /// Current resident set size of the worker process, in bytes (process isolation only).
    pub fn rss(&self) -> Option<u64> {
        self.proc_status("VmRSS:")
    }

    fn proc_status(&self, key: &str) -> Option<u64> {
        self.child.as_ref()?;
        let s = std::fs::read_to_string(format!("/proc/{}/status", self.pid)).ok()?;
        let line = s.lines().find(|l| l.starts_with(key))?;
        Some(line.split_whitespace().nth(1)?.parse::<u64>().ok()? * 1024)
    }

    /// How the worker process ended (`exit status: 1`, `signal: 11 (SIGSEGV) (core dumped)`):
    /// None for a thread or a process still running. With `closed` (its connection is gone,
    /// so all its descriptors are: it is exiting) this waits for the exit, no deadline needed;
    /// otherwise it only looks.
    pub fn exit_status(&mut self, closed: bool) -> Option<String> {
        let c = self.child.as_mut()?;
        let status = if closed {
            c.wait().ok()
        } else {
            c.try_wait().ok().flatten()
        };
        status.map(|s| s.to_string())
    }

    /// Asks it to stop and waits for its end, within [`Self::deadline`]: None when it ended
    /// cleanly, else what went wrong (an error it reported, its exit status, or that it did
    /// not stop: a process is then killed, a thread abandoned).
    pub fn shutdown(mut self) -> Option<String> {
        if self.hung {
            return Some(
                if self.child.is_some() {
                    "hung earlier; its process was killed"
                } else {
                    "hung earlier; its thread was abandoned"
                }
                .into(),
            );
        }
        let _ = self.tx.send(HostMsg::Shutdown);
        let wait = self.deadline();
        let until = Instant::now() + wait;
        let mut problem = None;
        // What it says while stopping (an error or panic in its teardown), until it hangs up
        // or, a thread, ends (its connection may outlive it).
        loop {
            if self.thread.as_ref().is_some_and(|t| t.is_finished()) {
                while let Ok(WorkerEvent::Msg(WorkerMsg::Error(e))) = self.rx.try_recv() {
                    problem = Some(format!("failed while stopping: {e}"));
                }
                break;
            }
            let step = if self.thread.is_some() {
                until.min(Instant::now() + Duration::from_millis(5))
            } else {
                until
            };
            match self.rx.recv_deadline(step) {
                Ok(WorkerEvent::Msg(WorkerMsg::Error(e))) => {
                    problem = Some(format!("failed while stopping: {e}"))
                }
                Ok(WorkerEvent::Msg(_)) => {}
                Ok(WorkerEvent::Closed) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) if Instant::now() < until => {}
                Err(RecvTimeoutError::Timeout) => {
                    problem = Some(format!("did not stop within {wait:.1?}"));
                    break;
                }
            }
        }
        if let Some(t) = self.thread.take() {
            // Thread isolation: let the renderer tear down before the process might exit.
            while !t.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            } else {
                problem = Some(format!(
                    "{}; its thread abandoned",
                    problem.unwrap_or_else(|| format!("did not stop within {wait:.1?}"))
                ));
            }
        }
        if let Some(mut c) = self.child.take() {
            loop {
                if let Ok(Some(status)) = c.try_wait() {
                    if !status.success() {
                        problem = Some(match problem {
                            Some(p) => format!("{p}; exited: {status}"),
                            None => format!("exited uncleanly: {status}"),
                        });
                    }
                    break;
                }
                if Instant::now() >= until {
                    let _ = c.kill();
                    let _ = c.wait();
                    problem = Some(format!(
                        "{}; its process killed",
                        problem.unwrap_or_else(|| format!("did not stop within {wait:.1?}"))
                    ));
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        if let Some(p) = &problem {
            tracing::warn!("[{}] worker shutdown: {p}", self.label);
        }
        problem
    }
}

/// A worker process not yet handed to its [`Worker`]: killed if spawning fails.
struct KillOnDrop(Option<Child>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(c) = &mut self.0 {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(c) = &mut self.child {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Receives the next protocol message, turning worker errors and disconnects into errors.
pub fn recv_timeout(rx: &Receiver<WorkerEvent>, t: Duration) -> anyhow::Result<WorkerMsg> {
    match rx.recv_timeout(t) {
        Ok(WorkerEvent::Msg(WorkerMsg::Error(e))) => bail!("worker error: {e}"),
        Ok(WorkerEvent::Msg(m)) => Ok(m),
        Ok(WorkerEvent::Closed) | Err(RecvTimeoutError::Disconnected) => {
            bail!("worker exited (see its log)")
        }
        Err(RecvTimeoutError::Timeout) => bail!("worker timed out after {t:?}"),
    }
}

fn accept(
    l: &mr_ipc::Listener,
    child: &mut Option<Child>,
    timeout: Duration,
) -> anyhow::Result<mr_ipc::Stream> {
    let start = Instant::now();
    loop {
        if let Some(s) = mr_ipc::try_accept(l)? {
            return Ok(s);
        }
        if let Some(c) = child
            && let Some(status) = c.try_wait()?
        {
            bail!("worker exited before connecting: {status}");
        }
        if start.elapsed() > timeout {
            bail!("worker did not connect within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Copies worker stderr into its log file, echoing warnings and errors to the host log.
fn forward_stderr(stderr: std::process::ChildStderr, label: String, log: Option<PathBuf>) {
    std::thread::spawn(move || {
        let mut file = log
            .as_deref()
            .and_then(|p: &Path| std::fs::File::create(p).ok());
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(f) = &mut file {
                let _ = writeln!(f, "{line}");
            }
            if line.contains(" WARN ")
                || line.contains("ERROR")
                || line.contains("panicked")
                || file.is_none()
            {
                eprintln!("[{label}] {line}");
            }
        }
    });
}

/// A frame of `format` as its RGBA8 form and, for `Rgba16F`, the float frame.
pub fn rgba8(format: PixelFormat, px: Vec<u8>) -> (Vec<u8>, Option<Vec<u8>>) {
    match format {
        PixelFormat::Rgba8 => (px, None),
        PixelFormat::Rgba16F => (mr_core::lighting::hdr_to_rgba8(&px), Some(px)),
    }
}
