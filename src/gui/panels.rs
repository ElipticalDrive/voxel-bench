//! The toolbar, the plan editor (form and TOML), the compare and benchmark results, and the
//! pipeline editor.

use egui::{Color32, TextureOptions, Vec2};
use glam::Vec3;

use super::*;

impl App {
    // ---------------------------------------------------------------- panels

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("📂 Open", |ui| {
                let recent = recent_plans();
                if !recent.is_empty() {
                    ui.weak("Recent");
                    for p in recent {
                        if ui.button(p.display().to_string()).on_hover_text("Load this plan again (unsaved changes to the current one are lost)").clicked() {
                            self.load(&p);
                            ui.close();
                        }
                    }
                    ui.separator();
                }
                ui.weak("Plans in ./plans");
                for p in plan_files().into_iter().filter(|p| !is_demo(p)) {
                    if ui.button(p.display().to_string()).on_hover_text("Load this plan (unsaved changes to the current one are lost)").clicked() {
                        self.load(&p);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("New").on_hover_text("Start from the plain built-in plan: terrain, the sun, a camera to fly, one renderer").clicked() {
                    self.plan = Plan::parse(DEFAULT_PLAN, &[]).unwrap();
                    self.option_text = option_texts(&self.plan);
                    self.plan_path = None;
                    self.path_text = "plans/untitled.toml".into();
                    self.dirty = true;
                    ui.close();
                }
            });
            ui.menu_button("🎬 Demos", |ui| {
                ui.weak("Set-ups that show something off (plans/demo-*.toml); load one, then ▶ Start live");
                let demos: Vec<PathBuf> = plan_files().into_iter().filter(|p| is_demo(p)).collect();
                if demos.is_empty() {
                    ui.weak("none found in ./plans");
                }
                for p in demos {
                    let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").trim_start_matches("demo-").to_string();
                    if ui.button(name).on_hover_text(lead_comment(&p)).clicked() {
                        self.load(&p);
                        ui.close();
                    }
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.path_text).desired_width(240.0)).on_hover_text("Plan file path used by Load path and Save");
            if ui.button("Load path").on_hover_text("Load the plan at the path to the left").clicked() {
                self.load(&PathBuf::from(self.path_text.clone()));
            }
            if ui.button(if self.dirty { "💾 Save*" } else { "💾 Save" }).on_hover_text("Save the plan as TOML to the path field (Ctrl+S). * = unsaved changes").clicked() {
                self.save(PathBuf::from(self.path_text.clone()));
            }
            ui.separator();
            let live = self.live.is_some();
            if ui.button(if live { "↻ Restart live (F5)" } else { "▶ Start live (F5)" })
                .on_hover_text("Start every renderer in the plan side by side, each in its own process, driven by the camera here")
                .clicked()
            {
                self.start_live(ui.ctx());
            }
            if live && ui.button("■ Stop").on_hover_text("Stop the live session and its renderer processes").clicked() {
                self.live = None;
                self.textures.clear();
            }
            ui.separator();
            let running = self.bench.as_ref().is_some_and(|b| !b.finished);
            if !running && ui.button("⏱ Run benchmark").on_hover_text("Runs the full plan in a separate process; results appear in the Benchmark tab").clicked() {
                match bench::BenchRun::start(&self.plan, &[]) {
                    Ok(b) => {
                        self.bench = Some(b);
                        self.docks.show(ui.ctx(), dock::Pane::Benchmark);
                    }
                    Err(e) => self.message = format!("benchmark failed to start: {e:#}"),
                }
            }
            if running {
                ui.spinner();
                if ui.button("Cancel benchmark").on_hover_text("Kill the benchmark process; finished passes stay in the Benchmark tab").clicked()
                    && let Some(b) = &mut self.bench {
                    b.cancel();
                }
            }
            ui.separator();
            ui.weak(&self.message);
        });
        ui.horizontal_wrapped(|ui| {
            // Two latching toggles: lighting and meshes are independent.
            let mut lit = self.mode.lighting == mr_core::RenderMode::Lit;
            if ui.toggle_value(&mut lit, "💡 Lit").on_hover_text("Sun, sky and point lights with shadows; off = textures only (L toggles)").changed() {
                self.mode.lighting = if lit { mr_core::RenderMode::Lit } else { mr_core::RenderMode::Unlit };
            }
            ui.toggle_value(&mut self.mode.meshes, "🔺 Meshes").on_hover_text("Draw the scene's triangle meshes with the voxel world (renderers with the `meshes` tag; others show voxels only). N toggles");
            ui.separator();
            if ui.checkbox(&mut self.follow_path, "follow path (F)").on_hover_text("Camera follows the plan's scripted path. Any movement key switches to free fly.").changed() && !self.follow_path {
                self.follow_path = true;
                self.go_free();
            }
            if ui.button(if self.playing { "⏸" } else { "▶" }).on_hover_text("Play/pause the scene clock: path position and scripted events (P)").clicked() {
                self.playing = !self.playing;
            }
            let frames = self.plan.run.frames.max(1) as f64;
            ui.add(egui::Slider::new(&mut self.scene_frame, 0.0..=frames).text("scene frame").step_by(1.0).update_while_editing(false)).on_hover_text("Position on the scripted path. [ and ] step 10 frames (Shift: 100), R rewinds.");
            ui.checkbox(&mut self.events_on, "events").on_hover_text("Apply the plan's scripted events (block edits, lights) as the scene clock passes them");
            ui.separator();
            ui.add(egui::DragValue::new(&mut self.speed).speed(0.5).range(0.5..=2000.0).prefix("speed ").suffix(" b/s").update_while_editing(false)).on_hover_text("Free-fly speed in blocks per second (mouse wheel over the view; hold Ctrl for 5×)");
            ui.add(egui::DragValue::new(&mut self.fps_cap).speed(1.0).range(0.0..=10000.0).prefix("fps cap ").update_while_editing(false)).on_hover_text("Frame-rate limit per renderer; 0 = uncapped. Capping frees GPU time for the others.");
            ui.add(egui::DragValue::new(&mut self.probe_interval).speed(0.1).range(0.0..=60.0).prefix("probe every ").suffix(" s").update_while_editing(false))
                .on_hover_text("Score every renderer against the reference this often (0 = only on C / when a tuner asks)");
            ui.add(egui::DragValue::new(&mut self.display_hz).speed(1.0).range(1.0..=240.0).prefix("display ").suffix(" Hz").update_while_editing(false))
                .on_hover_text("How often frames are read back for display. Other frames render without read-back.");
            ui.separator();
            ui.toggle_value(&mut self.edit_mode, "✏ edit (E)").on_hover_text("Left click: remove block. Ctrl+left click: place block.\nEdits are sent to every renderer like scripted ones.");
            if self.edit_mode {
                egui::ComboBox::from_id_salt("place")
                    .selected_text(&self.block_names[self.place_block])
                    .show_ui(ui, |ui| {
                        for (i, n) in self.block_names.iter().enumerate() {
                            ui.selectable_value(&mut self.place_block, i, n);
                        }
                    })
                    .response
                    .on_hover_text("Block placed by Ctrl+left click");
            }
            ui.menu_button("💡 lights", |ui| {
                ui.weak("Point lights you drop are added to the world for every renderer.");
                ui.horizontal(|ui| {
                    ui.label("colour");
                    ui.color_edit_button_rgb(&mut self.light_color).on_hover_text("Colour of the next dropped light");
                });
                ui.add(egui::Slider::new(&mut self.light_intensity, 1.0..=500.0).logarithmic(true).text("intensity").update_while_editing(false)).on_hover_text("Brightness of the next dropped light");
                ui.add(egui::Slider::new(&mut self.light_radius, 2.0..=100.0).text("radius").update_while_editing(false)).on_hover_text("Distance in blocks at which the light's contribution reaches zero");
                if ui.button("Drop light at camera (G)").on_hover_text("Place a point light where the camera is (visible in Lit mode)").clicked()
                    && let Some(c) = self.live_camera() {
                    self.send(Cmd::AddLight { position: c.position, color: Vec3::from_array(self.light_color), intensity: self.light_intensity, radius: self.light_radius });
                }
                if ui.button("Remove dropped lights").on_hover_text("Remove lights placed with G; the plan's own lights stay").clicked() {
                    self.send(Cmd::ClearUserLights);
                }
            });
            ui.separator();
            if ui.button("⛶ Fullscreen (F11)").on_hover_text("Show only the renderer views, fullscreen (F11 or Esc to leave)").clicked() {
                self.set_fullscreen(ui.ctx(), true);
            }
            ui.toggle_value(&mut self.pixel_exact, "1:1 (Z)").on_hover_text("One frame pixel per physical screen pixel (no scaling; larger frames are cropped)");
            if let Some([w, h]) = self.tile_px
                && ui.button(format!("match {w}×{h} (M)")).on_hover_text("Set the render resolution to this tile's exact pixel size and restart live").clicked() {
                self.match_resolution(ui.ctx());
            }
            ui.toggle_value(&mut self.overlays, "overlays (O)").on_hover_text("Renderer names, fps and the status line over the views");
            if ui.button("⚖ Compare (C)").on_hover_text("Render this exact view on every renderer and the reference, then score them").clicked() {
                self.send(Cmd::Compare);
                self.docks.show(ui.ctx(), dock::Pane::Compare);
            }
        });
    }

    pub(super) fn form(&mut self, ui: &mut egui::Ui) {
        let camera = self.live_camera();
        let mut restart = None;
        let mut open_graph = None;
        let live = self.live.is_some();
        steady_scroll(ui, "form", |ui| {
            let mut ectx = editor::EditorCtx {
                infos: &self.infos,
                block_names: &self.block_names,
                camera,
                option_text: &mut self.option_text,
                restart_lane: &mut restart,
                live,
                trials: &mut self.trials,
                open_graph: &mut open_graph,
            };
            if editor::plan_editor(ui, &mut self.plan, &mut ectx) {
                self.dirty = true;
            }
            ui.add_space(8.0);
            if live {
                ui.weak("World, path and run-setting changes take effect when you restart the live session (F5). Use ↻ on a renderer to restart just that one.");
            }
        });
        if let Some((i, sel)) = open_graph {
            self.open_graph(ui.ctx(), graph::Of::Plan(i), sel);
        }
        if let Some(i) = restart {
            let n = self.live.as_ref().map(|l| l.shared.lock().unwrap().lanes.len()).unwrap_or(0);
            let spec = self.plan.renderers[i].clone();
            self.send(if i < n { Cmd::Restart { lane: i, spec } } else { Cmd::AddLane(spec) });
        }
    }

    pub(super) fn toml(&mut self, ui: &mut egui::Ui) {
        let pass = ui.ctx().cumulative_pass_nr();
        if self.toml_pass.is_none_or(|q| pass > q + 1) {
            self.toml_text = self.plan.to_toml().unwrap_or_default();
            self.toml_error = None;
        }
        self.toml_pass = Some(pass);
        ui.horizontal(|ui| {
            if ui.button("Apply").on_hover_text("Parse this TOML and replace the plan with it").clicked() {
                match Plan::parse(&self.toml_text, &[]) {
                    Ok(p) => {
                        self.plan = p;
                        self.option_text = option_texts(&self.plan);
                        self.toml_error = None;
                        self.dirty = true;
                    }
                    Err(e) => self.toml_error = Some(format!("{e:#}")),
                }
            }
            if ui.button("Revert").on_hover_text("Discard text edits; show the current plan again").clicked() {
                self.toml_text = self.plan.to_toml().unwrap_or_default();
                self.toml_error = None;
            }
        });
        if let Some(e) = &self.toml_error {
            ui.colored_label(Color32::LIGHT_RED, e);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.toml_text).code_editor().desired_width(f32::INFINITY).desired_rows(40)).on_hover_text("Edit, then Apply. Field reference: docs/plans.md.");
        });
    }

    pub(super) fn compare(&mut self, ui: &mut egui::Ui) {
        let Some(live) = &self.live else {
            ui.weak("Start a live session, frame a view, then press C.");
            return;
        };
        let sh = live.shared.lock().unwrap();
        if sh.comparing {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("rendering comparison (the reference is slow)…");
            });
        }
        let Some(r) = &sh.compare else {
            ui.weak("Press C (or ⚖ Compare) to render the current view on every renderer and the reference.");
            return;
        };
        if self.compare_tex.as_ref().is_none_or(|c| c.seq != r.seq) {
            let size = [r.width as usize, r.height as usize];
            let mk = |name: String, px: &[u8]| ui.ctx().load_texture(name, egui::ColorImage::from_rgba_unmultiplied(size, px), TextureOptions::NEAREST);
            self.compare_tex = Some(CompareTex {
                seq: r.seq,
                reference: mk("cmp-ref".into(), &r.reference),
                lanes: r.lanes.iter().enumerate().map(|(i, l)| (mk(format!("cmp{i}"), &l.image), mk(format!("cmpd{i}"), &l.diff))).collect(),
                show: 0,
                diff: true,
            });
        }
        let p = r.camera.position;
        ui.label(format!("{} mode · camera ({:.1}, {:.1}, {:.1}) · reference: {}", r.mode.name(), p.x, p.y, p.z, r.reference_label));
        egui::Grid::new("cmp").striped(true).show(ui, |ui| {
            ui.strong("renderer");
            ui.strong("PSNR dB").on_hover_text("Peak signal-to-noise ratio vs the reference; higher is better, ≥40 dB looks identical, 99 = exact");
            ui.strong("MS-SSIM dB").on_hover_text("Multi-scale structural similarity as dB (20 = 0.99, 30 = 0.999); tolerant of resampling");
            ui.strong("MAE").on_hover_text("Mean absolute error per channel, 0–1");
            ui.strong("bad %").on_hover_text("Pixels whose largest channel differs by more than 16/255");
            ui.end_row();
            for l in &r.lanes {
                ui.label(&l.label);
                ui.monospace(format!("{:.1}", l.quality.psnr.min(99.0)));
                ui.monospace(format!("{:.1}", l.quality.ms_ssim_db));
                ui.monospace(format!("{:.4}", l.quality.mae));
                ui.monospace(format!("{:.3}", l.quality.bad_pct));
                ui.end_row();
            }
        });
        let labels: Vec<String> = r.lanes.iter().map(|l| l.label.clone()).collect();
        drop(sh);
        let Some(ct) = &mut self.compare_tex else { return };
        ui.horizontal_wrapped(|ui| {
            for (i, l) in labels.iter().enumerate() {
                ui.selectable_value(&mut ct.show, i, l).on_hover_text("Show this renderer's frame");
            }
            ui.selectable_value(&mut ct.show, usize::MAX, "reference").on_hover_text("Show the reference renderer's frame");
            ui.checkbox(&mut ct.diff, "show diff").on_hover_text("Grey image with differing pixels in red (brighter = larger difference)");
        });
        let tex = match ct.lanes.get(ct.show) {
            Some((img, diff)) => {
                if ct.diff {
                    diff
                } else {
                    img
                }
            }
            None => &ct.reference,
        };
        let w = ui.available_width();
        let s = tex.size_vec2();
        ui.image((tex.id(), Vec2::new(w, w * s.y / s.x)));
        ui.weak("Diff: grey = match, red = differs (brighter = larger).");
    }

    pub(super) fn benchmark(&mut self, ui: &mut egui::Ui) {
        if self.live.is_some() && self.bench.as_ref().is_some_and(|b| !b.finished) {
            ui.colored_label(Color32::YELLOW, "The live session is sharing the GPU with this benchmark; stop it for clean numbers.");
        }
        if let Some(b) = &mut self.bench {
            if b.poll()
                && let Some(p) = b.report.lock().unwrap().take() {
                match bench::load_report(&p) {
                    Ok(r) => self.report = Some(r),
                    Err(e) => self.message = format!("report: {e:#}"),
                }
            }
            egui::CollapsingHeader::new("benchmark output").default_open(!b.finished).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("bench-out").max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
                    for l in b.lines.lock().unwrap().iter() {
                        ui.monospace(l);
                    }
                });
            });
        }
        let base = PathBuf::from(self.plan.output.dir.clone().unwrap_or_else(|| "runs".into()));
        ui.menu_button("📂 Load a previous report", |ui| {
            ui.weak("Benchmark runs found in the plan's output directory, newest first");
            for p in bench::past_runs(&base).into_iter().take(40) {
                let name = p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                    match bench::load_report(&p) {
                        Ok(r) => self.report = Some(r),
                        Err(e) => self.message = format!("report: {e:#}"),
                    }
                    ui.close();
                }
            }
        });
        if let Some(r) = &self.report {
            bench::report_ui(ui, r);
        } else {
            ui.weak("Run a benchmark (⏱) or load a previous report.");
        }
    }

    /// Opens the pipeline editor on a plan's renderer or a live lane, and shows its pane.
    pub(super) fn open_graph(&mut self, ctx: &egui::Context, of: graph::Of, selected: Option<String>) {
        self.graph_win = Some(graph::Window::new(ctx, of, selected));
        self.docks.show(ctx, dock::Pane::Pipeline);
    }

    /// The pipeline editor pane, on a plan's renderer or a live lane.
    pub(super) fn pipeline(&mut self, ui: &mut egui::Ui) {
        let Some(mut win) = self.graph_win.take() else {
            ui.weak("Open a renderer's pipeline with ✏ graph (in the Form's renderers, or a lane in Metrics).");
            return;
        };
        let target = match win.of {
            graph::Of::Plan(i) => self.plan.renderers.get(i).map(|r| {
                let params: &[mr_core::ParamSpec] = self.infos.iter().find(|x| x.name == r.name).map_or(&[], |x| &x.params);
                (format!("plan renderer {} ({})", i + 1, r.label()), r.clone(), params.iter().map(|p| (p.clone(), knobs::option_value(p, &r.options))).collect::<Vec<_>>())
            }),
            graph::Of::Lane(i) => self.live.as_ref().and_then(|l| {
                let sh = l.shared.lock().unwrap();
                sh.lanes.get(i).map(|l| (format!("live lane {} ({})", i + 1, l.spec.label()), l.spec.clone(), l.params.iter().map(|(s, v)| (s.clone(), self.knob_edits.get(&(i, s.name.clone())).cloned().unwrap_or(v.clone()))).collect()))
            }),
        };
        let Some((title, spec, knobs)) = target else { return };
        let marks = matches!(win.of, graph::Of::Lane(_));
        let (edits, open) = graph::pane(ui, &mut win, &title, &spec, &knobs, marks, &mut self.trials, &self.infos, &self.plan.run);
        match win.of {
            graph::Of::Plan(i) => {
                let (changed, _) = graph::apply_to_plan(&mut self.plan.renderers[i], edits);
                if changed {
                    self.option_text = option_texts(&self.plan);
                    self.dirty = true;
                }
            }
            graph::Of::Lane(i) => {
                for e in edits {
                    match e {
                        graph::Edit::Knob { name, value, commit } => {
                            let live = knobs.iter().find(|(s, _)| s.name == name).is_some_and(|(s, _)| s.live);
                            if (live && name != "host.render_scale") || commit {
                                self.knob_edits.remove(&(i, name.clone()));
                                self.send(Cmd::SetParam { lane: i, name, value });
                            } else {
                                self.knob_edits.insert((i, name), value);
                            }
                        }
                        graph::Edit::Spec(new) => match graph::live_switch(&spec, &new, &knobs) {
                            Some((slot, c)) => self.send(Cmd::SetParam { lane: i, name: slot, value: mr_core::ParamValue::Choice(c) }),
                            None => self.send(Cmd::Swap { lane: i, spec: new }),
                        },
                        graph::Edit::Open(_) => {}
                    }
                }
            }
        }
        if open {
            self.graph_win = Some(win);
        }
    }
}
