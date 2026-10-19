//! The metrics tab: per-lane statistics, knobs and auto-tuners.

use egui::Color32;

use crate::tune::{self, Algorithm};

use super::live::{LaneStatus, Sample};

use super::*;

impl App {
    pub(super) fn metrics(&mut self, ui: &mut egui::Ui) {
        let Some(live) = &self.live else {
            ui.weak("No live session.");
            return;
        };
        let shared = live.shared.clone();
        let mut actions: Vec<LaneAction> = vec![];
        let (series, quality): (Vec<(String, Vec<Sample>)>, Vec<(String, Vec<[f64; 2]>)>) = {
            let sh = shared.lock().unwrap();
            ui.label(format!("{} · {} lights · {} chunks cached on host", sh.status, sh.lights, sh.cached_chunks))
                .on_hover_text("Chunks are generated once on the host and streamed to each renderer around its camera");
            for (i, l) in sh.lanes.iter().enumerate() {
                ui.push_id(("lane", i), |ui| {
                    ui.group(|ui| self.lane_panel(ui, i, l, &mut actions));
                });
            }
            (
                sh.lanes.iter().map(|l| (l.spec.label().to_string(), window(&l.samples, self.plot_window).to_vec())).collect(),
                sh.lanes
                    .iter()
                    .map(|l| (l.spec.label().to_string(), l.quality.iter().map(|q| [q.t, if self.plot_ms_ssim { q.ms_ssim_db } else { q.psnr }]).collect()))
                    .collect(),
            )
        };
        for a in actions {
            match a {
                LaneAction::Focus(i) => self.focus = if self.focus == Some(i) { None } else { Some(i) },
                LaneAction::Restart(i) => {
                    if let Some(spec) = self.plan.renderers.get(i).cloned() {
                        self.send(Cmd::Restart { lane: i, spec });
                    }
                }
                LaneAction::Swap(lane, spec) => self.send(Cmd::Swap { lane, spec }),
                LaneAction::Remove(i) => {
                    self.send(Cmd::RemoveLane(i));
                    let below = (1u64 << i.min(63)) - 1;
                    self.windows = self.windows & below | (self.windows >> 1) & !below;
                    self.textures.clear();
                    self.focus = None;
                }
                LaneAction::Set(lane, name, value) => self.send(Cmd::SetParam { lane, name, value }),
                LaneAction::Tune(lane, config) => self.send(Cmd::Tune { lane, config }),
                LaneAction::SaveToPlan(i, spec) => {
                    if let Some(r) = self.plan.renderers.get_mut(i) {
                        *r = spec;
                        self.option_text = option_texts(&self.plan);
                        self.dirty = true;
                        self.message = format!("knobs of renderer {} copied into the plan", i + 1);
                    }
                }
            }
        }
        ui.add(egui::Slider::new(&mut self.plot_window, 2.0..=120.0).text("plot window (s)").update_while_editing(false)).on_hover_text("Seconds of history shown in the plots below");
        let t_end = series.iter().filter_map(|(_, s)| s.last().map(|x| x.t)).fold(0.0, f64::max);
        for (title, f) in [
            ("frame interval (ms)", (|s: &Sample| Some(s.interval_ms)) as fn(&Sample) -> Option<f32>),
            ("GPU time (ms)", |s: &Sample| s.gpu_ms),
            ("CPU time (ms)", |s: &Sample| Some(s.cpu_ms)),
        ] {
            ui.label(title).on_hover_text(match title {
                "frame interval (ms)" => "Time between consecutive completed frames (1000 / fps). Spikes are hitches.",
                "GPU time (ms)" => "GPU execution time per frame from timestamp queries (GPU renderers only)",
                _ => "Time the renderer process spent on the CPU per frame (submission, or the whole frame for CPU renderers)",
            });
            egui_plot::Plot::new(title)
                .height(130.0)
                .legend(egui_plot::Legend::default())
                .include_y(0.0)
                .allow_scroll(false)
                .allow_drag(false)
                .show(ui, |pu| {
                    for (i, (name, s)) in series.iter().enumerate() {
                        let pts: Vec<[f64; 2]> = s.iter().filter_map(|x| f(x).map(|v| [x.t - t_end, v as f64])).collect();
                        pu.line(egui_plot::Line::new(name.clone(), pts).color(lane_color(i)));
                    }
                });
        }
        if quality.iter().any(|(_, q)| !q.is_empty()) {
            ui.horizontal(|ui| {
                ui.label("quality vs reference (dB, probes):").on_hover_text("Each probe renders the current view on the reference renderer and scores every lane against it. Higher is better.");
                ui.selectable_value(&mut self.plot_ms_ssim, false, "PSNR").on_hover_text("Pixel-exact error (≥40 dB looks identical); harsh on resolution changes");
                ui.selectable_value(&mut self.plot_ms_ssim, true, "MS-SSIM").on_hover_text("Structural similarity over 5 scales; 20 dB = 0.99, 30 dB = 0.999");
            });
            egui_plot::Plot::new("quality").height(130.0).legend(egui_plot::Legend::default()).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                for (i, (name, q)) in quality.iter().enumerate() {
                    let pts: Vec<[f64; 2]> = q.iter().filter(|p| p[0] >= t_end - self.plot_window).map(|p| [p[0] - t_end, p[1]]).collect();
                    pu.line(egui_plot::Line::new(name.clone(), pts).color(lane_color(i)));
                    pu.points(egui_plot::Points::new(name.clone(), q.iter().filter(|p| p[0] >= t_end - self.plot_window).map(|p| [p[0] - t_end, p[1]]).collect::<Vec<_>>()).color(lane_color(i)).radius(2.5));
                }
            });
        } else {
            ui.weak("Quality plot: set a probe interval in the toolbar (or press C) to score lanes against the reference.");
        }
    }

    fn lane_panel(&mut self, ui: &mut egui::Ui, i: usize, l: &live::LaneView, actions: &mut Vec<LaneAction>) {
        ui.horizontal(|ui| {
            ui.colored_label(lane_color(i), format!("■ {} {}", i + 1, l.spec.label()));
            ui.weak(format!("{}×{}", l.resolution[0], l.resolution[1])).on_hover_text("Current render resolution (after host.render_scale)");
            if ui.small_button("focus").on_hover_text("Show only this renderer in the view (number keys, or double-click a view)").clicked() {
                actions.push(LaneAction::Focus(i));
            }
            if ui.small_button("↻").on_hover_text("Hot-swap to this renderer's spec from the plan editor; the old one keeps rendering until the new one is ready").clicked() {
                actions.push(LaneAction::Restart(i));
            }
            if ui.small_button("💾").on_hover_text("Copy this renderer's current knob values into the plan (then Save to keep them)").clicked() {
                actions.push(LaneAction::SaveToPlan(i, l.spec.clone()));
            }
            if ui.small_button("✖").on_hover_text("Remove from the live session (the plan is unchanged)").clicked() {
                actions.push(LaneAction::Remove(i));
            }
            if i < 64 {
                let bit = 1u64 << i;
                let mut open = self.windows & bit != 0;
                if ui
                    .toggle_value(&mut open, "☀ HDR")
                    .on_hover_text(
                        "Show this renderer in a window of its own, presented in HDR where the display offers it (an `hdr` post's values above 1 \
                         stay above SDR white; other posts look as here). In the window: H switches HDR / SDR, [ and ] set the paper white, Esc closes.",
                    )
                    .changed()
                {
                    self.windows ^= bit;
                }
            }
        });
        if let Some(w) = &l.window {
            ui.horizontal(|ui| {
                ui.weak(format!("window: {w}"));
                let mut stops = self.paper_white.log2();
                if ui
                    .add(egui::DragValue::new(&mut stops).speed(0.05).range(-4.0..=4.0).custom_formatter(|v, _| format!("{:.2}×", 2f64.powf(v))).prefix("paper white ").update_while_editing(false))
                    .on_hover_text("The frames' 1.0 (an SDR frame's white) relative to the display's SDR white, in every HDR window. Above 1 brightens the frame, highlights included. Shared by all windows; [ and ] in a window set it too.")
                    .changed()
                {
                    self.paper_white = 2f32.powf(stops);
                }
            });
        }
        if let Some(sw) = &l.swap {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("hot swap: {sw} (still rendering the old one)"));
            });
        }
        match &l.status {
            LaneStatus::Failed(e) => {
                ui.colored_label(Color32::LIGHT_RED, e);
                return;
            }
            LaneStatus::Starting => {
                ui.weak("starting…");
                return;
            }
            LaneStatus::Running => {}
        }
        let w = window(&l.samples, 2.0);
        let (fps, _) = recent(&l.samples, 1.0);
        let stat = |f: &dyn Fn(&Sample) -> Option<f32>| {
            let mut v: Vec<f32> = w.iter().filter_map(f).collect();
            if v.is_empty() {
                return "-".to_string();
            }
            v.sort_by(f32::total_cmp);
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            format!("{mean:.2} / {:.2}", v[((v.len() - 1) as f32 * 0.99) as usize])
        };
        egui::CollapsingHeader::new(format!("stats — {fps:.0} fps{}", l.quality.back().map(|q| format!(" · {:.1} dB PSNR · {:.1} dB MS-SSIM", q.psnr, q.ms_ssim_db)).unwrap_or_default()))
            .id_salt(("stats", i))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                    for (name, f) in [
                        ("frame interval ms (mean/p99)", &(|s: &Sample| Some(s.interval_ms).filter(|x| *x > 0.0)) as &dyn Fn(&Sample) -> Option<f32>),
                        ("latency ms", &|s: &Sample| Some(s.latency_ms)),
                        ("gpu ms", &|s: &Sample| s.gpu_ms),
                        ("cpu ms", &|s: &Sample| Some(s.cpu_ms)),
                        ("update ms", &|s: &Sample| Some(s.update_ms)),
                    ] {
                        ui.label(name).on_hover_text(match name {
                            "frame interval ms (mean/p99)" => "Time between completed frames over the last 2 s: mean and 99th percentile",
                            "latency ms" => "Host-measured time from sending a frame request to the frame arriving (includes queueing)",
                            "gpu ms" => "GPU execution time from timestamp queries",
                            "cpu ms" => "Renderer-process CPU time per frame",
                            _ => "Time spent applying world updates (chunk uploads, edits, lights) per frame",
                        });
                        ui.monospace(stat(f));
                        ui.end_row();
                    }
                    if let Some(q) = l.quality.back() {
                        ui.label("quality (last probe)").on_hover_text("Agreement with the reference renderer at the last probe; bad = pixels off by more than 16/255");
                        ui.monospace(format!("PSNR {:.1} · SSIM {:.1} · MS-SSIM {:.1} dB · {:.2}% bad", q.psnr, q.ssim_db, q.ms_ssim_db, q.bad_pct));
                        ui.end_row();
                    }
                    ui.label("memory").on_hover_text("RSS: the renderer process's resident memory. GPU: buffers and textures it reports allocating.");
                    ui.monospace(format!(
                        "RSS {} MB · GPU {} MB",
                        l.rss_mb.map(|x| format!("{x:.0}")).unwrap_or("-".into()),
                        l.gpu_mb.map(|x| format!("{x:.0}")).unwrap_or("-".into())
                    ));
                    ui.end_row();
                    ui.label("streaming").on_hover_text("Chunks sent to this renderer so far, and chunks queued for it (limited per frame)");
                    ui.monospace(format!("{} chunks sent · {} pending", l.chunks_sent, l.pending_chunks));
                    ui.end_row();
                    let a = l.arrivals;
                    ui.label("arrivals").on_hover_text("What reached this renderer's client: bytes arrived (each message once) and, where more, bytes sent over its link (in flight and resends included); holes = chunks of its region still missing on its side (on the link, or not generated yet), plus the server's backlog");
                    let mb = |b: u64| b as f64 / 1048576.0;
                    let sent = if a.bytes > a.arrived { format!(" of {:.1} MB sent", mb(a.bytes)) } else { String::new() };
                    ui.monospace(format!("{:.1} MB arrived{sent} · {} holes", mb(a.arrived), a.holes as usize + l.pending_chunks));
                    ui.end_row();
                    for s in &l.stages {
                        let ms = |x: Option<f32>, what| x.map(|x| format!("{what} {x:.3} ms"));
                        let nodes = if s.nodes.is_empty() { "no node (the executor's own work)".to_string() } else { s.nodes.join(", ") };
                        ui.label(format!("stage {}", s.name)).on_hover_text(format!("Time of this stage per frame (averaged), and the graph nodes it runs: {nodes}. Storages and kernels run inside their callers and are timed with them."));
                        ui.monospace([ms(s.gpu_ms, "gpu"), ms(s.cpu_ms, "cpu")].into_iter().flatten().collect::<Vec<_>>().join(" · "));
                        ui.end_row();
                    }
                    for (k, v) in &l.counters {
                        ui.label(k).on_hover_text("Renderer-specific counter");
                        ui.monospace(if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.3}") });
                        ui.end_row();
                    }
                });
            });
        egui::CollapsingHeader::new("pipeline & knobs").id_salt(("knobs", i)).default_open(true).show(ui, |ui| {
            ui.weak("↻ = changing it hot-swaps the renderer · 🔒 = the tuner may not touch it");
            let locks = &mut self.locks;
            let tunable: HashSet<&str> = l.params.iter().map(|(s, _)| s.name.as_str()).collect();
            let mut lock_box = |ui: &mut egui::Ui, spec: &mr_core::ParamSpec| {
                if !tunable.contains(spec.name.as_str()) {
                    return;
                }
                let key = (i, spec.name.clone());
                let mut locked = locks.contains(&key);
                if ui.checkbox(&mut locked, "🔒").on_hover_text("Lock: the auto-tuner may not change this knob (applies when the tuner is (re)started)").changed() {
                    if locked {
                        locks.insert(key);
                    } else {
                        locks.remove(&key);
                    }
                }
            };
            // Values being dragged are shown before they are applied.
            let knobs: Vec<(mr_core::ParamSpec, mr_core::ParamValue)> = l.params.iter().map(|(s, v)| (s.clone(), self.knob_edits.get(&(i, s.name.clone())).cloned().unwrap_or(v.clone()))).collect();
            let edits = graph::summary(ui, &format!("{:?}", graph::Of::Lane(i)), &l.spec, &knobs, true, Some(&mut lock_box), &mut self.trials, &self.infos, &self.plan.run);
            for e in edits {
                match e {
                    graph::Edit::Knob { name, value, commit } => {
                        let key = (i, name.clone());
                        let live = l.params.iter().find(|(s, _)| s.name == name).is_some_and(|(s, _)| s.live);
                        if (live && name != "host.render_scale") || commit {
                            self.knob_edits.remove(&key);
                            actions.push(LaneAction::Set(i, name, value));
                        } else {
                            self.knob_edits.insert(key, value);
                        }
                    }
                    // A component the running back end switches live, else a hot swap.
                    graph::Edit::Spec(spec) => match graph::live_switch(&l.spec, &spec, &l.params) {
                        Some((slot, c)) => actions.push(LaneAction::Set(i, slot, mr_core::ParamValue::Choice(c))),
                        None => actions.push(LaneAction::Swap(i, spec)),
                    },
                    graph::Edit::Open(sel) => self.graph_win = Some(graph::Window::new(ui.ctx(), graph::Of::Lane(i), sel)),
                }
            }
        });
        let running = l.tuner.is_some();
        egui::CollapsingHeader::new(if running { "auto-tune (running)" } else { "auto-tune" }).id_salt(("tune", i)).default_open(running).show(ui, |ui| {
            let t = self.tune_ui.entry(i).or_default();
            ui.horizontal(|ui| {
                ui.label("goal").on_hover_text("What the tuner optimises. Locked knobs (🔒 in the knob list above) are never touched.");
                ui.selectable_value(&mut t.goal, 0, "max quality @ fps").on_hover_text("Hold at least the target frame rate; spend whatever headroom remains on image quality");
                ui.selectable_value(&mut t.goal, 1, "max fps @ quality").on_hover_text("Keep quality at or above a floor (dB vs the reference); go as fast as possible");
                ui.selectable_value(&mut t.goal, 2, "max fps").on_hover_text("As fast as possible; quality is ignored (no reference probes needed)");
            });
            match t.goal {
                0 => {
                    ui.add(egui::DragValue::new(&mut t.fps).range(1.0..=10000.0).prefix("target ").suffix(" fps").update_while_editing(false)).on_hover_text("Frame rate to hold. Measured as throughput over each window; 2 % below still counts as met.");
                }
                1 => {
                    ui.add(egui::DragValue::new(&mut t.psnr).range(5.0..=99.0).prefix("floor ").suffix(" dB").update_while_editing(false)).on_hover_text("Minimum quality vs the reference renderer, in dB of the chosen metric.\nMS-SSIM: 20 dB = 0.99, 30 dB = 0.999. PSNR: ≥40 dB is visually near-identical.");
                }
                _ => {}
            }
            if t.goal != 2 {
                ui.horizontal(|ui| {
                    ui.label("quality metric").on_hover_text("How quality is measured: the current view is rendered by the reference renderer and compared");
                    ui.selectable_value(&mut t.metric, tune::QualityMetric::MsSsim, "MS-SSIM").on_hover_text("Multi-scale structural similarity: degrades gracefully with resolution (recommended)");
                    ui.selectable_value(&mut t.metric, tune::QualityMetric::Psnr, "PSNR").on_hover_text("Pixel-exact: any resolution change on pixel-scale detail costs ~30 dB at once");
                });
            }
            ui.horizontal(|ui| {
                ui.label("algorithm").on_hover_text("How the next configuration to try is chosen");
                ui.selectable_value(&mut t.algorithm, Algorithm::LocalSearch, "local search").on_hover_text("Randomised hill climbing: changes one unlocked knob at a time, keeps it if the goal improves");
                ui.selectable_value(&mut t.algorithm, Algorithm::ResolutionController, "resolution controller").on_hover_text("Dynamic resolution: steers host.render_scale continuously to hold the fps target (needs the fps goal)");
                ui.selectable_value(&mut t.algorithm, Algorithm::Bayesian, "bayesian").on_hover_text("Learns a model of fps and quality over all unlocked knobs and picks the most promising point; changes several at once");
            });
            ui.checkbox(&mut t.allow_restart, "may change ↻ knobs (hot swaps)").on_hover_text("Knobs marked ↻ need the renderer to restart. The replacement loads while the old one keeps rendering, but each trial costs a load.");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut t.settle).range(0.05..=10.0).speed(0.05).prefix("settle ").suffix(" s").update_while_editing(false)).on_hover_text("Seconds ignored after each change (pipelines warm up, streaming settles)");
                ui.add(egui::DragValue::new(&mut t.window).range(0.1..=30.0).speed(0.05).prefix("measure ").suffix(" s").update_while_editing(false)).on_hover_text("Seconds of frames measured per trial. Longer = less noise, slower search.");
                ui.add(egui::DragValue::new(&mut t.patience).range(0..=200).prefix("patience ").update_while_editing(false)).on_hover_text("Trials without improvement before the tuner settles on the best configuration and only watches it.\nIt resumes exploring if the goal stops being met. 0 = explore forever.");
            });
            let t = t.clone();
            let locked: std::collections::BTreeSet<String> = self.locks.iter().filter(|(lane, _)| *lane == i).map(|(_, n)| n.clone()).collect();
            let config = tune::TunerConfig {
                goal: match t.goal {
                    0 => tune::Goal::MaxQualityAtFps { fps: t.fps },
                    1 => tune::Goal::MaxFpsAtQuality { psnr: t.psnr },
                    _ => tune::Goal::MaxFps,
                },
                algorithm: t.algorithm,
                metric: t.metric,
                locked,
                allow_restart: t.allow_restart,
                settle_s: t.settle,
                window_s: t.window,
                patience: t.patience,
            };
            ui.horizontal(|ui| {
                if ui.button(if running { "↻ restart tuner" } else { "▶ start tuner" }).on_hover_text("Start with the goal, locks and settings above (restarting first restores the best configuration found)").clicked() {
                    actions.push(LaneAction::Tune(i, Some(config.clone())));
                }
                if running && ui.button("■ stop").on_hover_text("Stop and apply the best configuration found").clicked() {
                    actions.push(LaneAction::Tune(i, None));
                }
            });
            if config.goal.needs_quality() {
                ui.weak("This goal needs quality: the tuner requests reference probes itself (the reference renderer uses CPU time).");
            }
            if let Some(tv) = &l.tuner {
                let metric = tv.config.metric.name();
                ui.label(format!("{:?} via {:?}", tv.config.goal, tv.config.algorithm));
                ui.horizontal(|ui| {
                    if tv.holding {
                        ui.colored_label(Color32::LIGHT_GREEN, "● holding").on_hover_text("Converged: the best configuration is applied and re-measured periodically");
                    } else {
                        ui.colored_label(Color32::GOLD, "● exploring").on_hover_text("Trying configurations");
                    }
                    ui.label(format!("{} · {} trials, {} accepted", tv.status, tv.trials, tv.accepted));
                });
                if let Some(best) = &tv.best {
                    ui.label(format!("best: {best}")).on_hover_text("Best measured result; stopping the tuner applies it");
                    if tv.best_diff.is_empty() {
                        ui.weak("   (the starting configuration)");
                    }
                    for (k, from, to) in &tv.best_diff {
                        ui.monospace(format!("   {k}: {from} → {to}"));
                    }
                }
                let pts: Vec<[f64; 2]> = tv.history.iter().map(|h| [h.t, h.score]).collect();
                let pick = |kind: tune::PointKind| -> Vec<[f64; 2]> { tv.history.iter().filter(|h| h.kind == kind).map(|h| [h.t, h.score]).collect() };
                ui.weak("score per evaluation").on_hover_text("Higher is better. Constrained goals score above 100 only when the constraint is met.\nGreen: accepted · red: rejected · blue: check while holding");
                egui_plot::Plot::new(("tuner", i)).height(110.0).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                    pu.line(egui_plot::Line::new("score", pts).color(lane_color(i)));
                    pu.points(egui_plot::Points::new("accepted", pick(tune::PointKind::Accepted)).radius(3.0).color(Color32::LIGHT_GREEN));
                    pu.points(egui_plot::Points::new("rejected", pick(tune::PointKind::Rejected)).radius(2.0).color(Color32::from_rgb(220, 90, 90)));
                    pu.points(egui_plot::Points::new("hold", pick(tune::PointKind::Hold)).radius(2.5).color(Color32::LIGHT_BLUE));
                });
                let fps: Vec<[f64; 2]> = tv.history.iter().map(|h| [h.t, h.fps]).collect();
                let q: Vec<[f64; 2]> = tv.history.iter().filter_map(|h| h.psnr.map(|p| [h.t, p])).collect();
                ui.weak(format!("measured fps and {metric} per evaluation"));
                egui_plot::Plot::new(("tuner-fq", i)).height(90.0).legend(egui_plot::Legend::default()).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                    pu.line(egui_plot::Line::new("fps", fps).color(Color32::LIGHT_BLUE));
                    if !q.is_empty() {
                        pu.line(egui_plot::Line::new(format!("{metric} dB"), q).color(Color32::GOLD));
                    }
                });
                // Knob trajectories, normalised to each knob's range.
                let names: Vec<String> = tv.history.iter().rev().find(|h| !h.knobs.is_empty()).map(|h| h.knobs.iter().map(|(k, _)| k.clone()).collect()).unwrap_or_default();
                if !names.is_empty() {
                    ui.weak("knob values (0 = minimum, 1 = maximum of each knob's range)").on_hover_text("Each line is one tunable knob over time; hover a line for its name");
                    egui_plot::Plot::new(("tuner-knobs", i)).height(110.0).include_y(0.0).include_y(1.0).legend(egui_plot::Legend::default()).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                        for (n, name) in names.iter().enumerate() {
                            let line: Vec<[f64; 2]> = tv.history.iter().filter_map(|h| h.knobs.iter().find(|(k, _)| k == name).map(|(_, v)| [h.t, *v])).collect();
                            let hue = n as f32 / names.len() as f32;
                            pu.line(egui_plot::Line::new(name.trim_start_matches("host."), line).color(egui::ecolor::Hsva::new(hue, 0.6, 0.95, 1.0)));
                        }
                    });
                }
                egui::ScrollArea::vertical().id_salt(("tlog", i)).max_height(120.0).stick_to_bottom(true).show(ui, |ui| {
                    for line in &tv.log {
                        ui.monospace(line);
                    }
                });
            }
        });
        if !l.events.is_empty() {
            egui::CollapsingHeader::new(format!("events ({})", l.events.len())).id_salt(("ev", i)).show(ui, |ui| {
                for (t, e) in l.events.iter().rev().take(20) {
                    ui.monospace(format!("{t:7.1}s  {e}"));
                }
            });
        }
    }
}

enum LaneAction {
    Focus(usize),
    Restart(usize),
    /// Hot-swap a lane to a new spec (e.g. another back end).
    Swap(usize, crate::plan::RendererSpec),
    Remove(usize),
    SaveToPlan(usize, crate::plan::RendererSpec),
    Set(usize, String, mr_core::ParamValue),
    Tune(usize, Option<tune::TunerConfig>),
}

#[derive(Clone)]
pub(super) struct TuneUi {
    goal: u8,
    fps: f64,
    psnr: f64,
    metric: tune::QualityMetric,
    algorithm: Algorithm,
    allow_restart: bool,
    settle: f64,
    window: f64,
    patience: u32,
}

impl Default for TuneUi {
    fn default() -> Self {
        let d = tune::TunerConfig::default();
        Self { goal: 0, fps: 144.0, psnr: 25.0, metric: tune::QualityMetric::MsSsim, algorithm: Algorithm::LocalSearch, allow_restart: false, settle: d.settle_s, window: d.window_s, patience: d.patience }
    }
}

/// Samples within the last `secs` seconds of the newest one.
fn window(s: &std::collections::VecDeque<Sample>, secs: f64) -> Vec<Sample> {
    let Some(last) = s.back() else { return vec![] };
    s.iter().rev().take_while(|x| x.t >= last.t - secs).copied().collect::<Vec<_>>().into_iter().rev().collect()
}

/// (fps, mean gpu ms) over the last `secs` seconds.
/// How long a lane's event shows on its view.
pub(super) const EVENT_SECS: f64 = 6.0;

pub(super) fn recent(s: &std::collections::VecDeque<Sample>, secs: f64) -> (f64, Option<f64>) {
    let w = window(s, secs);
    if w.len() < 2 {
        return (0.0, None);
    }
    let span = (w.last().unwrap().t - w[0].t).max(1e-6);
    let g: Vec<f64> = w.iter().filter_map(|x| x.gpu_ms.map(f64::from)).collect();
    ((w.len() - 1) as f64 / span, (!g.is_empty()).then(|| g.iter().sum::<f64>() / g.len() as f64))
}

fn lane_color(i: usize) -> Color32 {
    const C: [Color32; 6] = [
        Color32::from_rgb(90, 170, 255),
        Color32::from_rgb(255, 140, 70),
        Color32::from_rgb(110, 210, 120),
        Color32::from_rgb(220, 110, 220),
        Color32::from_rgb(240, 210, 80),
        Color32::from_rgb(120, 220, 220),
    ];
    C[i % C.len()]
}
