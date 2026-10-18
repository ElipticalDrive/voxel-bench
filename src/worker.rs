//! Spawning and talking to renderer workers.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;

use mr_core::{BlockRegistry, Environment, InitParams, RendererInfo};
use mr_ipc::{FrameSlots, HostMsg, PROTOCOL_VERSION, WorkerMsg};

use crate::plan::{Isolation, RendererSpec, RunSpec};

pub enum WorkerEvent {
    Msg(WorkerMsg),
    Closed,
}

pub struct Worker {
    pub label: String,
    pub info: RendererInfo,
    pub pid: u32,
    pub init_ms: f32,
    pub shm: FrameSlots,
    free_slots: Vec<u32>,
    tx: Sender<HostMsg>,
    pub rx: Receiver<WorkerEvent>,
    child: Option<Child>,
    thread: Option<std::thread::JoinHandle<()>>,
}

pub struct SpawnConfig<'a> {
    pub renderer: &'a str,
    pub label: &'a str,
    pub params: InitParams,
    pub isolation: Isolation,
    pub launcher: &'a [String],
    pub log_path: Option<PathBuf>,
    pub slots: u32,
    /// A compiled pipeline's worker (`pipelines`) instead of this executable.
    pub exe: Option<PathBuf>,
}

/// Spawns a worker for `spec` using the plan's run settings; `world` is the world's
/// generator, which its client runs if the graph places the generator there (PLAN step 14d).
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
    let exe = match (spec.compile.unwrap_or(run.compile), crate::pipelines::bake(spec)?) {
        (level @ (crate::plan::Compile::Native | crate::plan::Compile::BreakIn), Some(b)) => {
            if run.isolation != Isolation::Process {
                anyhow::bail!("{}: compiled pipelines run in their own process (isolation = \"process\")", spec.label());
            }
            let c = crate::pipelines::trusted(&b, level)?;
            if let Some(s) = c.build_s {
                tracing::info!("{}: compiled pipeline {} in {s:.1} s", spec.label(), c.key);
            }
            Some(c.exe)
        }
        _ => None,
    };
    let params = InitParams {
        width,
        height,
        blocks,
        // The world's comes over the stream (`SetEnvironment` first: PLAN step 14c).
        environment: Environment::default(),
        view_radius_chunks: spec.view_radius(run),
        max_lod: spec.max_lod(run, u8::MAX),
        options_json: serde_json::to_string(&spec.options)?,
        graph_json: spec.pipeline().map(|g| serde_json::to_string(&g)).transpose()?.unwrap_or_default(),
        devices: spec.devices(run),
        stage_times: run.stage_times,
        client: spec.levels(),
        link: spec.link(),
    };
    let w = Worker::spawn(SpawnConfig {
        renderer: &spec.name,
        label: spec.label(),
        params,
        isolation: run.isolation,
        launcher: &run.launcher,
        log_path,
        slots,
        exe,
    })?;
    if spec.generates() {
        w.send(HostMsg::Generate { world: serde_json::to_string(world)?, budget: run.budget().map(|b| b as u32) })?;
    }
    Ok(w)
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

impl Worker {
    pub fn spawn(cfg: SpawnConfig) -> anyhow::Result<Self> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tag = format!("multirenderer-{}-{n}", std::process::id());
        let sock = runtime_dir().join(format!("{tag}.sock"));
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).with_context(|| format!("binding {}", sock.display()))?;
        listener.set_nonblocking(true)?;

        let mut child = None;
        let mut thread = None;
        match cfg.isolation {
            Isolation::Process => {
                // A compiled pipeline's worker, else this executable (`MR_WORKER_EXE`
                // overrides it, e.g. for tests, whose own executable is the test harness).
                let exe = match &cfg.exe {
                    Some(e) => e.clone(),
                    None => std::env::var_os("MR_WORKER_EXE").map(PathBuf::from).map_or_else(std::env::current_exe, Ok)?,
                };
                let mut cmd = match cfg.launcher.split_first() {
                    Some((first, rest)) => {
                        let mut c = Command::new(first);
                        c.args(rest).arg(&exe);
                        c
                    }
                    None => Command::new(&exe),
                };
                cmd.arg("worker").arg("--socket").arg(&sock).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
                let mut c = cmd.spawn().with_context(|| format!("spawning worker for {}", cfg.renderer))?;
                forward_stderr(c.stderr.take().unwrap(), cfg.label.to_string(), cfg.log_path.clone());
                child = Some(c);
            }
            Isolation::Thread => {
                let s = sock.clone();
                let label = cfg.label.to_string();
                thread = Some(std::thread::Builder::new().name(format!("worker-{label}")).spawn(move || {
                    if let Err(e) = mr_worker::run(&s) {
                        tracing::error!("[{label}] worker failed: {e:#}");
                    }
                })?);
            }
        }

        let stream = accept(&listener, &mut child, Duration::from_secs(20))?;
        let _ = std::fs::remove_file(&sock);
        stream.set_nonblocking(false)?;
        let (mut input, mut output) = mr_ipc::split::<WorkerMsg, HostMsg>(stream)?;

        let (ev_tx, rx) = crossbeam_channel::unbounded();
        std::thread::Builder::new().name(format!("rx-{}", cfg.label)).spawn(move || {
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
        std::thread::Builder::new().name(format!("tx-{}", cfg.label)).spawn(move || {
            for m in msg_rx {
                if output.send(&m).is_err() {
                    return;
                }
            }
        })?;

        let pid = match recv_timeout(&rx, Duration::from_secs(20))? {
            WorkerMsg::Hello { pid, protocol, .. } if protocol == PROTOCOL_VERSION => pid,
            WorkerMsg::Hello { protocol, .. } => bail!("worker speaks protocol {protocol}, host {PROTOCOL_VERSION}"),
            m => bail!("unexpected first message {m:?}"),
        };
        let slot_bytes = (cfg.params.width * cfg.params.height * 4) as u64;
        let shm = FrameSlots::create(&tag, cfg.slots, slot_bytes)?;
        tx.send(HostMsg::Init { renderer: cfg.renderer.to_string(), params: cfg.params, shm: shm.spec().clone() })?;
        let (info, init_ms) = match recv_timeout(&rx, Duration::from_secs(300))? {
            WorkerMsg::Ready { info, init_ms } => (info, init_ms),
            m => bail!("[{}] expected Ready, got {m:?}", cfg.label),
        };
        // Both sides have it mapped now; unlinking means nothing leaks if we're killed.
        let _ = std::fs::remove_file(shm.path());
        Ok(Self {
            label: cfg.label.to_string(),
            info,
            pid,
            init_ms,
            free_slots: (0..cfg.slots).rev().collect(),
            shm,
            tx,
            rx,
            child,
            thread,
        })
    }

    pub fn send(&self, m: HostMsg) -> anyhow::Result<()> {
        self.tx.send(m).map_err(|_| anyhow::anyhow!("[{}] worker connection closed", self.label))
    }

    /// Changes the render resolution. All frames in flight must have completed (their
    /// slots returned); a fresh shared-memory region is created for the new size.
    pub fn resize(&mut self, width: u32, height: u32, slots: u32) -> anyhow::Result<()> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let shm = FrameSlots::create(&format!("multirenderer-{}-{n}", std::process::id()), slots, (width * height * 4) as u64)?;
        self.send(HostMsg::Resize { width, height, shm: shm.spec().clone() })?;
        // The worker maps it while handling Resize; Flushed below proves it has.
        self.send(HostMsg::Flush)?;
        loop {
            match recv_timeout(&self.rx, Duration::from_secs(60))? {
                WorkerMsg::Flushed => break,
                WorkerMsg::FrameDone { .. } => {}
                _ => {}
            }
        }
        let _ = std::fs::remove_file(shm.path());
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

    /// Blocks until the worker answers `Flushed`, handing other messages to `on_msg`.
    pub fn flush(&self, mut on_msg: impl FnMut(WorkerMsg) -> anyhow::Result<()>) -> anyhow::Result<()> {
        self.send(HostMsg::Flush)?;
        loop {
            match recv_timeout(&self.rx, Duration::from_secs(600)) {
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

    pub fn shutdown(mut self) {
        let _ = self.tx.send(HostMsg::Shutdown);
        if let Some(t) = self.thread.take() {
            // Thread isolation: let the renderer tear down before the process might exit.
            let deadline = Instant::now() + Duration::from_secs(10);
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            } else {
                tracing::warn!("[{}] worker thread did not stop", self.label);
            }
        }
        if let Some(mut c) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if let Ok(Some(status)) = c.try_wait() {
                    if !status.success() {
                        tracing::warn!("[{}] worker exited uncleanly: {status}", self.label);
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
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
        Ok(WorkerEvent::Closed) | Err(RecvTimeoutError::Disconnected) => bail!("worker exited (see its log)"),
        Err(RecvTimeoutError::Timeout) => bail!("worker timed out after {t:?}"),
    }
}

fn accept(l: &UnixListener, child: &mut Option<Child>, timeout: Duration) -> anyhow::Result<std::os::unix::net::UnixStream> {
    let start = Instant::now();
    loop {
        match l.accept() {
            Ok((s, _)) => return Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.into()),
        }
        if let Some(c) = child {
            if let Some(status) = c.try_wait()? {
                bail!("worker exited before connecting: {status}");
            }
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
        let mut file = log.as_deref().and_then(|p: &Path| std::fs::File::create(p).ok());
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(f) = &mut file {
                let _ = writeln!(f, "{line}");
            }
            if line.contains("ERROR") || line.contains("panicked") || file.is_none() {
                eprintln!("[{label}] {line}");
            }
        }
    });
}
