//! Presenters: a lane's frames in a window of their own, HDR where the display offers it.
//! The GUI's surface (eframe's) is SDR and its event loop is eframe's, so each presenter is
//! a process (`multirenderer present`) running the view window ([`crate::window`]), fed
//! like a worker: messages over a local socket, pixels through shared memory, one frame in
//! flight (a frame offered while the presenter is still copying the last is skipped).

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError, TryRecvError};
use mr_core::lighting::PixelFormat;
use mr_ipc::{FrameSlots, Listener, MsgWriter, PresentMsg, PresentReply};

use crate::window::{UiEvent, UiHandle};

/// What a presenter tells the host ([`Presenter::poll`]).
pub enum Report {
    /// The window's `[` / `]` changed the gain.
    Gain(f32),
    /// How it presents now.
    Status(String),
}

enum Link {
    Connecting(Listener, Instant),
    Connected { tx: MsgWriter<PresentMsg>, rx: Receiver<PresentReply> },
}

/// The host's end of one presenter.
pub struct Presenter {
    /// `None` only once dropped.
    child: Option<Child>,
    link: Link,
    shm: Option<FrameSlots>,
    /// The region has not been unlinked yet (the presenter may not have opened it).
    fresh: bool,
    busy: bool,
    gain: f32,
    title: String,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl Presenter {
    /// Starts a presenter window titled `title`, its frames' 1.0 at `gain` × SDR white.
    pub fn spawn(title: &str, gain: f32) -> anyhow::Result<Self> {
        let tag = format!("multirenderer-{}-present{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
        let sock = mr_ipc::endpoint(&tag);
        let listener = mr_ipc::listen(&sock)?;
        let exe = std::env::var_os("MR_WORKER_EXE").map(std::path::PathBuf::from).map_or_else(std::env::current_exe, Ok)?;
        let child = Command::new(exe)
            .args(["present", "--socket", &sock, "--title", title])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .context("spawning a presenter")?;
        Ok(Self { child: Some(child), link: Link::Connecting(listener, Instant::now()), shm: None, fresh: false, busy: false, gain, title: title.into() })
    }

    fn send(&mut self, m: PresentMsg) -> anyhow::Result<()> {
        match &mut self.link {
            Link::Connected { tx, .. } => tx.send(&m),
            Link::Connecting(..) => Ok(()),
        }
    }

    /// Connects once the presenter has, and takes its replies. An error: it is gone (its
    /// window closed) or never connected.
    pub fn poll(&mut self) -> anyhow::Result<Vec<Report>> {
        if let Some(status) = self.child.as_mut().unwrap().try_wait()? {
            bail!("presenter exited ({status})");
        }
        if let Link::Connecting(l, since) = &self.link {
            let Some(stream) = mr_ipc::try_accept(l)? else {
                if since.elapsed() > Duration::from_secs(20) {
                    bail!("the presenter did not connect");
                }
                return Ok(vec![]);
            };
            let (mut input, tx) = mr_ipc::split::<PresentReply, PresentMsg>(stream);
            let (reply_tx, rx) = crossbeam_channel::unbounded();
            std::thread::Builder::new().name("rx-present".into()).spawn(move || {
                while let Ok(Some(m)) = input.recv() {
                    if reply_tx.send(m).is_err() {
                        return;
                    }
                }
            })?;
            self.link = Link::Connected { tx, rx };
            let (gain, title) = (self.gain, self.title.clone());
            self.send(PresentMsg::Gain(gain))?;
            self.send(PresentMsg::Title(title))?;
        }
        let mut out = vec![];
        loop {
            let Link::Connected { rx, .. } = &self.link else { break };
            match rx.try_recv() {
                Ok(PresentReply::Shown) => {
                    self.busy = false;
                    if std::mem::take(&mut self.fresh) {
                        // Both sides have it mapped: nothing leaks if either is killed.
                        if let Some(s) = &mut self.shm {
                            s.unlink();
                        }
                    }
                }
                Ok(PresentReply::Gain(g)) => {
                    self.gain = g;
                    out.push(Report::Gain(g));
                }
                Ok(PresentReply::Status(s)) => out.push(Report::Status(s)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => bail!("presenter closed"),
            }
        }
        Ok(out)
    }

    /// Hands it a frame in the worker's format, unless it is still copying the last one.
    pub fn offer(&mut self, width: u32, height: u32, format: PixelFormat, pixels: &[u8]) -> anyhow::Result<()> {
        if self.busy || matches!(self.link, Link::Connecting(..)) {
            return Ok(());
        }
        if self.shm.as_ref().is_none_or(|s| (s.spec().slot_bytes as usize) < pixels.len()) {
            let tag = format!("multirenderer-{}-present{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
            let shm = FrameSlots::create(&tag, 1, pixels.len() as u64)?;
            self.send(PresentMsg::Shm(shm.spec().clone()))?;
            self.shm = Some(shm);
            self.fresh = true;
        }
        self.shm.as_mut().unwrap().slot_mut(0)[..pixels.len()].copy_from_slice(pixels);
        self.send(PresentMsg::Frame { width, height, format })?;
        self.busy = true;
        Ok(())
    }

    pub fn set_gain(&mut self, gain: f32) -> anyhow::Result<()> {
        if gain != self.gain {
            self.gain = gain;
            self.send(PresentMsg::Gain(gain))?;
        }
        Ok(())
    }

    pub fn set_title(&mut self, title: &str) -> anyhow::Result<()> {
        if title != self.title {
            self.title = title.into();
            self.send(PresentMsg::Title(title.into()))?;
        }
        Ok(())
    }
}

impl Drop for Presenter {
    /// Asks it to close, and kills it if it has not within two seconds.
    fn drop(&mut self) {
        let _ = self.send(PresentMsg::Shutdown);
        let Some(mut child) = self.child.take() else { return };
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(2);
            while Instant::now() < until {
                if let Ok(Some(_)) = child.try_wait() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = child.kill();
            let _ = child.wait();
        });
    }
}

/// The presenter process: connects to the host at `socket` and shows what it sends.
pub fn run(socket: &str, title: &str) -> anyhow::Result<()> {
    let stream = mr_ipc::connect(socket)?;
    crate::window::run_with_window(title, move |ui| serve(stream, ui))
}

fn serve(stream: mr_ipc::Stream, ui: UiHandle) -> anyhow::Result<()> {
    let (mut input, mut tx) = mr_ipc::split::<PresentMsg, PresentReply>(stream);
    let (msg_tx, rx) = crossbeam_channel::bounded(4);
    std::thread::Builder::new().name("rx-host".into()).spawn(move || {
        while let Ok(Some(m)) = input.recv() {
            if msg_tx.send(m).is_err() {
                return;
            }
        }
    })?;
    let mut shm: Option<FrameSlots> = None;
    while !ui.closed() {
        let (gain, status) = {
            let mut i = ui.input.lock().unwrap();
            (i.gain.take(), i.status.take())
        };
        if let Some(g) = gain {
            tx.send(&PresentReply::Gain(g))?;
        }
        if let Some(s) = status {
            tx.send(&PresentReply::Status(s))?;
        }
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(PresentMsg::Shm(spec)) => shm = Some(FrameSlots::open(&spec)?),
            Ok(PresentMsg::Frame { width, height, format }) => {
                let n = (width * height * format.bytes()) as usize;
                let Some(s) = &shm else { bail!("a frame before the frame region") };
                let pixels = std::sync::Arc::new(s.slot(0)[..n].to_vec());
                tx.send(&PresentReply::Shown)?;
                ui.send(UiEvent::Frame { lane: 0, width, height, format, pixels });
            }
            Ok(PresentMsg::Gain(g)) => ui.send(UiEvent::Gain(g)),
            Ok(PresentMsg::Title(t)) => ui.send(UiEvent::Title(t)),
            Ok(PresentMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
    Ok(())
}
