//! The GUI's own GPU device (eframe's, which draws the window and its torn-off panes; renderers
//! have devices of their own). wgpu's default treats any error on it as fatal (a panic, which
//! took the whole program and every lane down with it). Here:
//!
//! - a lost surface is egui-wgpu's to handle (it recreates the surface and skips the frame);
//! - an error on the device that is not its loss (a validation error in drawing) is shown in the
//!   toolbar's message and the GUI goes on (wgpu drops the failed call);
//! - the device lost (driver reset, GPU removed or hung, `destroy`) cannot be recovered in place:
//!   eframe builds its painter once and offers no way to swap the device under it, and
//!   egui-wgpu panics when drawing on a lost device (a staging buffer it cannot get). So the GUI
//!   hands over: the live session stops (workers shut down as at any stop), the plan is written
//!   to `recovered-plan.toml` in the configuration directory when it has unsaved changes, a
//!   fresh GUI starts on it (on a new device) and says in its message what happened, and this
//!   one exits. Whether the GUI notices the loss first (its next frame) or egui-wgpu's panic
//!   does, the hand-over is the same ([`super::run`] catches that panic). If the fresh GUI cannot
//!   be started, the reason is printed and the program ends with an error.
//!
//! Fault injection (tests): with `MR_FAULT_INJECTION=1`, `MR_GUI_FAULT` is a comma list of
//! `<what>:<seconds>`, applied in order, each the given seconds after the one before (the first
//! after the GUI starts): `live` (starts the live session), `dirty` (marks the plan changed),
//! `device-lost` (destroys the device),
//! `device-lost-unseen` (destroys it and lets drawing find out: egui-wgpu's panic), `quit`
//! (closes the GUI). What follows a `device-lost` is the fresh GUI's list.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui_wgpu::{self, wgpu};

use crate::plan::Plan;

const RESTARTED_ENV: &str = "MR_GUI_RESTARTED";
/// Set when the fresh GUI opens a recovery copy: the plan file it stands in for (empty: an
/// untitled plan).
const ORIGINAL_ENV: &str = "MR_GUI_ORIGINAL_PLAN";
const FAULT_ENV: &str = "MR_GUI_FAULT";

/// Why the GUI's device was lost, once it was (one GUI device per process).
static LOST: Mutex<Option<String>> = Mutex::new(None);
/// What a hand-over needs, kept current every frame: the plan file and, when it has unsaved
/// changes, the plan; and the faults the fresh GUI gets.
static KEEP: Mutex<Keep> = Mutex::new(Keep {
    plan_path: None,
    unsaved: None,
    faults: String::new(),
});

struct Keep {
    plan_path: Option<PathBuf>,
    unsaved: Option<Plan>,
    faults: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Fault {
    Live,
    Dirty,
    DeviceLost,
    DeviceLostUnseen,
    Quit,
}

/// What the GUI is to do, from [`GuiGpu::poll`].
pub enum Event {
    /// The device is lost, for this reason.
    Lost(String),
    /// Fault injection: start the live session.
    StartLive,
    /// Fault injection: mark the plan changed.
    MarkDirty,
}

pub struct GuiGpu {
    device: Option<wgpu::Device>,
    errors: Arc<Mutex<Vec<String>>>,
    faults: VecDeque<(Fault, f64, String)>,
    since: Instant,
    unseen: bool,
}

impl GuiGpu {
    pub fn new(state: Option<&egui_wgpu::RenderState>, ctx: &egui::Context) -> Self {
        let errors = Arc::new(Mutex::new(vec![]));
        if let Some(s) = state {
            let c = ctx.clone();
            s.device.set_device_lost_callback(move |reason, msg| {
                let why = match reason {
                    wgpu::DeviceLostReason::Destroyed => "destroyed".to_string(),
                    _ if msg.is_empty() => "lost by the driver".to_string(),
                    _ => format!("lost by the driver ({msg})"),
                };
                *LOST.lock().unwrap() = Some(why);
                c.request_repaint();
            });
            let (e, c) = (errors.clone(), ctx.clone());
            s.device.on_uncaptured_error(Arc::new(move |err| {
                // Errors after the loss are its echo, not news.
                if LOST.lock().unwrap().is_none() {
                    e.lock().unwrap().push(
                        err.to_string()
                            .lines()
                            .take(6)
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                    c.request_repaint();
                }
            }));
        }
        GuiGpu {
            device: state.map(|s| s.device.clone()),
            errors,
            faults: faults(),
            since: Instant::now(),
            unseen: false,
        }
    }

    /// Once per frame, with what a hand-over would need: injected faults, then what the device
    /// reported.
    pub fn poll(
        &mut self,
        ctx: &egui::Context,
        plan_path: &Option<PathBuf>,
        unsaved: Option<&Plan>,
        message: &mut String,
    ) -> Option<Event> {
        let mut event = None;
        if let Some(&(f, at, _)) = self.faults.front() {
            let t = self.since.elapsed().as_secs_f64();
            if t >= at {
                self.faults.pop_front();
                self.since = Instant::now();
                match f {
                    Fault::Live => {
                        eprintln!("fault injection: starting the live session");
                        event = Some(Event::StartLive);
                    }
                    Fault::Dirty => {
                        eprintln!("fault injection: marking the plan changed");
                        event = Some(Event::MarkDirty);
                    }
                    Fault::DeviceLost | Fault::DeviceLostUnseen => {
                        eprintln!(
                            "fault injection: destroying the GUI's GPU device{}",
                            if f == Fault::DeviceLostUnseen {
                                " (drawing finds out)"
                            } else {
                                ""
                            }
                        );
                        self.unseen = f == Fault::DeviceLostUnseen;
                        if let Some(d) = &self.device {
                            d.destroy();
                        }
                    }
                    Fault::Quit => {
                        eprintln!("fault injection: closing the GUI");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        // The next fault's moment wakes the GUI (an idle GUI draws no frames).
        if let Some(&(_, at, _)) = self.faults.front() {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                (at - self.since.elapsed().as_secs_f64()).max(0.0),
            ));
        }
        {
            let mut k = KEEP.lock().unwrap();
            k.plan_path.clone_from(plan_path);
            k.unsaved = unsaved.cloned();
            k.faults = self
                .faults
                .iter()
                .map(|(_, _, item)| item.as_str())
                .collect::<Vec<_>>()
                .join(",");
        }
        if let Some(d) = &self.device {
            // Delivers the loss callback (wgpu calls it from a poll or a submit).
            let _ = d.poll(wgpu::PollType::Poll);
        }
        if let Some(e) = self.errors.lock().unwrap().drain(..).next_back() {
            eprintln!("GUI GPU error: {e}");
            *message = format!("GUI GPU error (drawing goes on): {e}");
        }
        if event.is_some() || self.unseen {
            return event;
        }
        lost().map(Event::Lost)
    }
}

/// Why the GUI's device was lost, if it was.
pub fn lost() -> Option<String> {
    LOST.lock().unwrap().clone()
}

fn faults() -> VecDeque<(Fault, f64, String)> {
    if std::env::var(mr_serve::fault::ENV).as_deref() != Ok("1") {
        return VecDeque::new();
    }
    let list = std::env::var(FAULT_ENV).unwrap_or_default();
    list.split(',')
        .filter(|i| !i.is_empty())
        .filter_map(|item| {
            let parsed = item.split_once(':').and_then(|(w, s)| {
                let f = match w {
                    "live" => Fault::Live,
                    "dirty" => Fault::Dirty,
                    "device-lost" => Fault::DeviceLost,
                    "device-lost-unseen" => Fault::DeviceLostUnseen,
                    "quit" => Fault::Quit,
                    _ => return None,
                };
                Some((f, s.parse().ok()?, item.to_string()))
            });
            if parsed.is_none() {
                eprintln!("{FAULT_ENV}: `{item}` is not `live`, `dirty`, `device-lost`, `device-lost-unseen` or `quit` with `:<seconds>`; ignored");
            }
            parsed
        })
        .collect()
}

/// What a GUI started by a hand-over reads: the message to show, and, when it opened a
/// recovery copy, the plan file that copy stands in for (`Some(None)`: an untitled plan).
pub fn restarted() -> Option<(String, Option<Option<PathBuf>>)> {
    let why = std::env::var(RESTARTED_ENV).ok()?;
    let original =
        std::env::var_os(ORIGINAL_ENV).map(|o| if o.is_empty() { None } else { Some(o.into()) });
    Some((why, original))
}

/// The hand-over after the device was lost (`why`): the unsaved plan written, a fresh GUI
/// started on it. The live session is the caller's to stop (before, or by unwinding).
pub fn hand_over(why: &str) -> anyhow::Result<()> {
    let (plan_path, unsaved, faults) = {
        let k = KEEP.lock().unwrap();
        (k.plan_path.clone(), k.unsaved.clone(), k.faults.clone())
    };
    let (plan, original) = match unsaved {
        Some(p) => match write_recovery(&p) {
            Ok(f) => {
                eprintln!("unsaved plan written to {}", f.display());
                (Some(f), Some(plan_path))
            }
            Err(e) => {
                eprintln!(
                    "the unsaved plan could not be written ({e:#}); the fresh GUI opens the plan as last saved"
                );
                (plan_path, None)
            }
        },
        None => (plan_path, None),
    };
    let mut cmd = std::process::Command::new(crate::worker::self_exe()?);
    cmd.arg("gui");
    if let Some(p) = &plan {
        cmd.arg(p);
    }
    cmd.env(RESTARTED_ENV, why)
        .env(FAULT_ENV, faults)
        .env_remove(ORIGINAL_ENV);
    if let Some(o) = &original {
        cmd.env(
            ORIGINAL_ENV,
            o.as_deref().map(Path::as_os_str).unwrap_or_default(),
        );
    }
    cmd.spawn().map_err(|e| {
        anyhow::anyhow!("the GUI's GPU device was {why}, and a fresh GUI could not be started: {e}")
    })?;
    Ok(())
}

fn write_recovery(plan: &Plan) -> anyhow::Result<PathBuf> {
    let f = super::config_dir()
        .ok_or_else(|| anyhow::anyhow!("no configuration directory"))?
        .join("recovered-plan.toml");
    std::fs::create_dir_all(f.parent().unwrap())?;
    std::fs::write(&f, plan.to_toml()?)?;
    Ok(f)
}
