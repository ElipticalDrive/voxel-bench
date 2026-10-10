//! Structured editor for run plans.

use egui::{ComboBox, DragValue, Ui};
use mr_core::{Camera, PassMode, RendererInfo};
use mr_world::GenSpec;
use mr_world::scenario::{CameraPathSpec, EnvSpec, EventSpec, LightSpec};

use crate::plan::{CompareSpec, Isolation, Pacing, Plan, Readback, RendererSpec, SaveSpec, Streaming};

/// Context the editor needs besides the plan itself.
pub struct EditorCtx<'a> {
    pub infos: &'a [RendererInfo],
    pub block_names: &'a [String],
    /// Current live camera, for "from camera" buttons.
    pub camera: Option<Camera>,
    /// Per-renderer option text buffers (TOML), parallel to `plan.renderers`.
    pub option_text: &'a mut Vec<String>,
    /// Set when the user asks to restart a live lane with its edited spec.
    pub restart_lane: &'a mut Option<usize>,
    pub live: bool,
}

type Changed = bool;

fn v3(ui: &mut Ui, v: &mut [f64; 3], speed: f64) -> Changed {
    let mut c = false;
    ui.horizontal(|ui| {
        for (x, l) in v.iter_mut().zip(["x", "y", "z"]) {
            ui.label(l);
            c |= ui.add(DragValue::new(x).speed(speed).max_decimals(2)).on_hover_text(format!("{l} in blocks (y is up). Drag or double-click to type.")).changed();
        }
    });
    c
}

fn opt_v3(ui: &mut Ui, label: &str, v: &mut Option<[f64; 3]>, default: [f64; 3], cam: Option<Camera>, tip: &str) -> Changed {
    let mut c = false;
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.checkbox(&mut on, label).on_hover_text(tip).changed() {
            *v = on.then_some(default);
            c = true;
        }
        if let Some(p) = cam {
            if ui.small_button("⌖ cam").on_hover_text("Use the live camera position").clicked() {
                *v = Some(p.position.to_array());
                c = true;
            }
        }
    });
    if let Some(x) = v {
        c |= v3(ui, x, 0.5);
    }
    c
}

fn color(ui: &mut Ui, label: &str, c: &mut [f32; 3], tip: &str) -> Changed {
    ui.horizontal(|ui| {
        ui.label(label).on_hover_text(tip);
        ui.color_edit_button_rgb(c).on_hover_text(tip).changed()
    })
    .inner
}

fn opt_color(ui: &mut Ui, label: &str, v: &mut Option<[f32; 3]>, default: [f32; 3], tip: &str) -> Changed {
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        let mut c = false;
        if ui.checkbox(&mut on, label).on_hover_text(tip).changed() {
            *v = on.then_some(default);
            c = true;
        }
        if let Some(x) = v {
            c |= ui.color_edit_button_rgb(x).changed();
        }
        c
    })
    .inner
}

fn num<T: egui::emath::Numeric>(ui: &mut Ui, label: &str, v: &mut T, speed: f64, tip: &str) -> Changed {
    ui.horizontal(|ui| {
        ui.label(label).on_hover_text(tip);
        ui.add(DragValue::new(v).speed(speed)).on_hover_text(tip).changed()
    })
    .inner
}

fn block_combo(ui: &mut Ui, id: &str, b: &mut String, names: &[String], toggle: bool) -> Changed {
    let mut c = false;
    ComboBox::from_id_salt(id)
        .selected_text(b.as_str())
        .show_ui(ui, |ui| {
            let extra: &[&str] = if toggle { &["air", "toggle"] } else { &["air"] };
            for n in extra.iter().map(|s| s.to_string()).chain(names.iter().cloned()) {
                c |= ui.selectable_value(b, n.clone(), n).changed();
            }
        })
        .response
        .on_hover_text(if toggle { "Block to write: air removes, toggle makes air solid and solid air" } else { "Block to write: air removes" });
    c
}

/// `variants` are (name, constructor, tooltip).
fn variant_combo<T>(ui: &mut Ui, id: &str, current: &str, value: &mut T, variants: &[(&str, fn() -> T, &str)]) -> Changed {
    let mut c = false;
    let tip = variants.iter().find(|v| v.0 == current).map(|v| v.2).unwrap_or("");
    ComboBox::from_id_salt(id)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (name, make, tip) in variants {
                if ui.selectable_label(*name == current, *name).on_hover_text(*tip).clicked() && *name != current {
                    *value = make();
                    c = true;
                }
            }
        })
        .response
        .on_hover_text(tip);
    c
}

pub fn plan_editor(ui: &mut Ui, plan: &mut Plan, ctx: &mut EditorCtx) -> Changed {
    let mut c = false;
    ui.horizontal(|ui| {
        ui.label("Name");
        c |= ui.text_edit_singleline(&mut plan.name).on_hover_text("Plan name; used in run directory names and reports").changed();
    });
    egui::CollapsingHeader::new("World").default_open(true).show(ui, |ui| c |= world(ui, plan));
    egui::CollapsingHeader::new("Camera path").default_open(true).show(ui, |ui| c |= camera(ui, plan, ctx.camera));
    egui::CollapsingHeader::new("Environment").show(ui, |ui| c |= environment(ui, &mut plan.scene.environment));
    egui::CollapsingHeader::new(format!("Lights ({})", plan.scene.lights.len())).show(ui, |ui| c |= lights(ui, &mut plan.scene.lights, ctx.camera));
    egui::CollapsingHeader::new(format!("Events ({})", plan.scene.events.len())).show(ui, |ui| c |= events(ui, &mut plan.scene.events, ctx));
    egui::CollapsingHeader::new(format!("Renderers ({})", plan.renderers.len())).default_open(true).show(ui, |ui| c |= renderers(ui, plan, ctx));
    egui::CollapsingHeader::new("Run settings").default_open(true).show(ui, |ui| c |= run(ui, plan));
    egui::CollapsingHeader::new("Benchmark outputs").show(ui, |ui| c |= output(ui, plan, ctx.infos));
    c
}

fn world(ui: &mut Ui, plan: &mut Plan) -> Changed {
    let g = &mut plan.scene.generator;
    let name = match g {
        GenSpec::Empty => "empty",
        GenSpec::Terrain { .. } => "terrain",
        GenSpec::Flat { .. } => "flat",
        GenSpec::Menger { .. } => "menger",
        GenSpec::Room { .. } => "room",
        GenSpec::Random { .. } => "random",
        GenSpec::Showcase => "showcase",
    };
    let mut c = ui
        .horizontal(|ui| {
            ui.label("Generator").on_hover_text("How the world's blocks are generated (deterministic; chunks stream in around the camera)");
            variant_combo(
                ui,
                "gen",
                name,
                g,
                &[
                    ("terrain", || GenSpec::Terrain { seed: 0, caves: true, trees: true }, "Infinite noise terrain with caves and trees: the streaming workload"),
                    ("flat", || GenSpec::Flat { checker: false }, "Infinite flat ground"),
                    ("menger", || GenSpec::Menger { level: 4 }, "Menger sponge fractal: many holes, worst case for traversal"),
                    ("room", || GenSpec::Room { size: 48 }, "A closed room: indoor lighting, everything in view is near"),
                    ("random", || GenSpec::Random { size: 128, density: 0.1, seed: 0 }, "A cube of random blocks: incoherent rays, no large empty spaces"),
                    ("showcase", || GenSpec::Showcase, "One of every block type, for checking textures"),
                    ("empty", || GenSpec::Empty, "No blocks (sky only)"),
                ],
            )
        })
        .inner;
    match g {
        GenSpec::Terrain { seed, caves, trees } => {
            c |= num(ui, "seed", seed, 1.0, "Random seed: a different seed gives a different landscape");
            c |= ui.checkbox(caves, "caves").on_hover_text("Carve caves underground (hidden geometry renderers must skip)").changed();
            c |= ui.checkbox(trees, "trees").on_hover_text("Place trees on the surface (fine detail at a distance)").changed();
        }
        GenSpec::Flat { checker } => c |= ui.checkbox(checker, "checkerboard of all blocks").on_hover_text("Alternate every block type across the ground instead of one").changed(),
        GenSpec::Menger { level } => {
            c |= ui.add(egui::Slider::new(level, 1..=7).text("level (3^level blocks)")).on_hover_text("Recursion depth; each level triples the size (level 5 = 243 blocks across)").changed();
        }
        GenSpec::Room { size } => c |= num(ui, "inner size", size, 1.0, "Interior width of the room in blocks"),
        GenSpec::Random { size, density, seed } => {
            c |= num(ui, "size", size, 1.0, "Edge length of the random cube in blocks");
            c |= ui.add(egui::Slider::new(density, 0.0..=1.0).text("density")).on_hover_text("Fraction of cells that are solid").changed();
            c |= num(ui, "seed", seed, 1.0, "Random seed");
        }
        GenSpec::Empty | GenSpec::Showcase => {}
    }
    c |= ui.add(egui::Slider::new(&mut plan.scene.fov_deg, 20.0..=140.0).text("FOV°")).on_hover_text("Vertical field of view in degrees").changed();
    c
}

fn camera(ui: &mut Ui, plan: &mut Plan, cam: Option<Camera>) -> Changed {
    let p = &mut plan.scene.camera;
    let name = match p {
        CameraPathSpec::Static { .. } => "static",
        CameraPathSpec::Orbit { .. } => "orbit",
        CameraPathSpec::Line { .. } => "line",
        CameraPathSpec::Flythrough { .. } => "flythrough",
        CameraPathSpec::Spin { .. } => "spin",
    };
    let mut c = ui
        .horizontal(|ui| {
            ui.label("Path").on_hover_text("Scripted camera motion, indexed by scene frame (benchmarks follow it; live sessions can too)");
            variant_combo(
                ui,
                "campath",
                name,
                p,
                &[
                    ("static", || CameraPathSpec::Static { position: None, target: None, yaw_deg: 0.0, pitch_deg: 0.0 }, "A fixed viewpoint"),
                    ("orbit", || CameraPathSpec::Orbit { center: [0.0, 40.0, 0.0], radius: 80.0, height: 30.0, period: 600 }, "Circle around a point, looking at it"),
                    ("line", || CameraPathSpec::Line { from: None, to: [500.0, 120.0, 0.0], look_at: None }, "Straight line over the run's frames: tests streaming"),
                    ("flythrough", || CameraPathSpec::Flythrough { points: vec![[0.0, 100.0, 0.0], [200.0, 120.0, 100.0]] }, "Smooth path through a list of points"),
                    ("spin", || CameraPathSpec::Spin { position: None, period: 300, pitch_deg: 0.0 }, "Turn on the spot: every direction, no streaming"),
                ],
            )
        })
        .inner;
    match p {
        CameraPathSpec::Static { position, target, yaw_deg, pitch_deg } => {
            if let Some(k) = cam {
                if ui.button("⌖ Set from live camera").on_hover_text("Copy the live camera's position and direction").clicked() {
                    *position = Some(k.position.to_array());
                    *target = None;
                    *yaw_deg = k.yaw.to_degrees();
                    *pitch_deg = k.pitch.to_degrees();
                    c = true;
                }
            }
            c |= opt_v3(ui, "position (else spawn)", position, [0.0, 80.0, 0.0], cam, "Camera position in blocks; unset = the generator's spawn point");
            c |= opt_v3(ui, "look at target", target, [0.0; 3], None, "Point the camera at this position (overrides yaw and pitch)");
            if target.is_none() {
                c |= num(ui, "yaw°", yaw_deg, 1.0, "Horizontal direction in degrees");
                c |= num(ui, "pitch°", pitch_deg, 1.0, "Vertical angle in degrees; negative looks down");
            }
        }
        CameraPathSpec::Orbit { center, radius, height, period } => {
            ui.label("center").on_hover_text("Point the camera circles and looks at");
            c |= v3(ui, center, 0.5);
            c |= num(ui, "radius", radius, 0.5, "Orbit radius in blocks");
            c |= num(ui, "height", height, 0.5, "Camera height above the centre");
            c |= num(ui, "period (frames)", period, 1.0, "Scene frames per full circle");
        }
        CameraPathSpec::Line { from, to, look_at } => {
            c |= opt_v3(ui, "from (else spawn)", from, [0.0, 100.0, 0.0], cam, "Start point; unset = spawn point");
            ui.label("to").on_hover_text("End point, reached at the last benchmark frame");
            c |= v3(ui, to, 0.5);
            c |= opt_v3(ui, "look at (else ahead)", look_at, [0.0; 3], cam, "Keep looking at this point; unset = look along the line");
        }
        CameraPathSpec::Flythrough { points } => {
            let mut remove = None;
            for (i, pt) in points.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{i}"));
                    c |= v3(ui, pt, 0.5);
                    if ui.small_button("✖").on_hover_text("Remove this point").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                points.remove(i);
                c = true;
            }
            ui.horizontal(|ui| {
                if ui.button("+ point").on_hover_text("Append a point (a copy of the last)").clicked() {
                    points.push(points.last().copied().unwrap_or([0.0, 100.0, 0.0]));
                    c = true;
                }
                if let Some(k) = cam {
                    if ui.button("+ live camera position").on_hover_text("Append the live camera's position: fly somewhere, click, repeat").clicked() {
                        points.push(k.position.to_array());
                        c = true;
                    }
                }
            });
        }
        CameraPathSpec::Spin { position, period, pitch_deg } => {
            c |= opt_v3(ui, "position (else spawn)", position, [0.0, 80.0, 0.0], cam, "Where to stand; unset = spawn point");
            c |= num(ui, "period (frames)", period, 1.0, "Scene frames per full turn");
            c |= num(ui, "pitch°", pitch_deg, 1.0, "Vertical angle in degrees; negative looks down");
        }
    }
    c
}

fn environment(ui: &mut Ui, e: &mut EnvSpec) -> Changed {
    let mut c = false;
    c |= opt_color(ui, "sky", &mut e.sky, [0.45, 0.65, 0.95], "Sky colour seen where rays hit nothing; unset = default");
    c |= opt_color(ui, "ambient", &mut e.ambient, [0.08, 0.08, 0.08], "Light reaching every surface, even in shadow (Lit mode)");
    c |= ui.checkbox(&mut e.no_sun, "no sun").on_hover_text("Disable the sun: only ambient and point lights (night, caves)").changed();
    if !e.no_sun {
        ui.horizontal(|ui| {
            let mut on = e.sun_direction.is_some();
            if ui.checkbox(&mut on, "sun direction").on_hover_text("Direction the sunlight travels (x, y, z; normalised). Unset = default afternoon sun.").changed() {
                e.sun_direction = on.then_some([-0.4, -1.0, -0.25]);
                c = true;
            }
            if let Some(d) = &mut e.sun_direction {
                for x in d.iter_mut() {
                    c |= ui.add(DragValue::new(x).speed(0.01).range(-1.0..=1.0)).on_hover_text("Direction component (x, y, z); y should be negative for daylight").changed();
                }
            }
        });
        c |= opt_color(ui, "sun colour", &mut e.sun_color, [1.0, 1.0, 1.0], "Colour and brightness of sunlight");
    }
    ui.horizontal(|ui| {
        let mut on = e.exposure.is_some();
        if ui.checkbox(&mut on, "exposure").on_hover_text("Multiplier applied before tone mapping (Lit mode)").changed() {
            e.exposure = on.then_some(1.0);
            c = true;
        }
        if let Some(x) = &mut e.exposure {
            c |= ui.add(DragValue::new(x).speed(0.01).range(0.0..=20.0)).on_hover_text("1 = neutral; higher is brighter").changed();
        }
    });
    c
}

fn lights(ui: &mut Ui, ls: &mut Vec<LightSpec>, cam: Option<Camera>) -> Changed {
    let mut c = false;
    let mut remove = None;
    for (i, l) in ls.iter_mut().enumerate() {
        ui.push_id(("light", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let name = if matches!(l, LightSpec::Point { .. }) { "point" } else { "scatter" };
                    c |= variant_combo(
                        ui,
                        "kind",
                        name,
                        l,
                        &[
                            ("point", || LightSpec::Point { position: [0.0, 60.0, 0.0], color: [1.0; 3], intensity: 20.0, radius: 16.0 }, "One point light"),
                            ("scatter", || LightSpec::Scatter {
                                count: 100,
                                seed: 0,
                                center: None,
                                extent: [100.0, 40.0, 100.0],
                                color: None,
                                intensity: 20.0,
                                radius: 16.0,
                                on_surface: true,
                            }, "Many point lights scattered randomly in a box: stresses light culling"),
                        ],
                    );
                    if ui.small_button("✖").on_hover_text("Remove this light").clicked() {
                        remove = Some(i);
                    }
                });
                match l {
                    LightSpec::Point { position, color: col, intensity, radius } => {
                        ui.horizontal(|ui| {
                            ui.label("position");
                            if let Some(k) = cam {
                                if ui.small_button("⌖ cam").on_hover_text("Use the live camera position").clicked() {
                                    *position = k.position.to_array();
                                    c = true;
                                }
                            }
                        });
                        c |= v3(ui, position, 0.5);
                        c |= color(ui, "colour", col, "Light colour");
                        c |= num(ui, "intensity", intensity, 0.5, "Brightness");
                        c |= num(ui, "radius", radius, 0.5, "Distance in blocks at which its contribution falls to zero");
                    }
                    LightSpec::Scatter { count, seed, center, extent, color: col, intensity, radius, on_surface } => {
                        c |= num(ui, "count", count, 10.0, "Number of lights");
                        c |= num(ui, "seed", seed, 1.0, "Random seed for positions and colours");
                        c |= opt_v3(ui, "center (else spawn)", center, [0.0, 60.0, 0.0], cam, "Centre of the box the lights are scattered in");
                        ui.label("extent (half size)").on_hover_text("Half-size of the box in blocks along x, y, z");
                        c |= v3(ui, extent, 1.0);
                        c |= opt_color(ui, "fixed colour (else warm random)", col, [1.0, 0.8, 0.5], "One colour for all lights; unset = random warm colours");
                        c |= num(ui, "intensity", intensity, 0.5, "Brightness of each light");
                        c |= num(ui, "radius", radius, 0.5, "Reach of each light in blocks");
                        c |= ui.checkbox(on_surface, "drop onto surface").on_hover_text("Move each light down to just above the ground below it").changed();
                    }
                }
            });
        });
    }
    if let Some(i) = remove {
        ls.remove(i);
        c = true;
    }
    if ui.button("+ light").on_hover_text("Add a point light (at the live camera if running)").clicked() {
        ls.push(LightSpec::Point { position: cam.map(|k| k.position.to_array()).unwrap_or([0.0, 60.0, 0.0]), color: [1.0; 3], intensity: 20.0, radius: 16.0 });
        c = true;
    }
    c
}

fn events(ui: &mut Ui, es: &mut Vec<EventSpec>, ctx: &EditorCtx) -> Changed {
    let mut c = false;
    let mut remove = None;
    for (i, e) in es.iter_mut().enumerate() {
        ui.push_id(("event", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let name = match e {
                        EventSpec::Sphere { .. } => "sphere",
                        EventSpec::Box { .. } => "box",
                        EventSpec::RandomEdits { .. } => "random_edits",
                        EventSpec::AnimateLights { .. } => "animate_lights",
                        EventSpec::SunCycle { .. } => "sun_cycle",
                    };
                    c |= variant_combo(
                        ui,
                        "kind",
                        name,
                        e,
                        &[
                            ("sphere", || EventSpec::Sphere { frame: 60, center: [0.0, 40.0, 0.0], radius: 10.0, block: "air".into() }, "Fill a sphere with a block at one frame (air = crater)"),
                            ("box", || EventSpec::Box { frame: 60, min: [0, 30, 0], max: [4, 80, 4], block: "bricks".into() }, "Fill a box with a block at one frame"),
                            ("random_edits", || EventSpec::RandomEdits { every: 1, count: 500, seed: 0, center: None, radius: 30.0, block: "toggle".into() }, "Continuous random block changes: tests incremental updates"),
                            ("animate_lights", || EventSpec::AnimateLights { amplitude: 2.0, period: 90 }, "Move every point light back and forth"),
                            ("sun_cycle", || EventSpec::SunCycle { period: 1200 }, "Rotate the sun through a day"),
                        ],
                    );
                    if ui.small_button("✖").on_hover_text("Remove this event").clicked() {
                        remove = Some(i);
                    }
                });
                match e {
                    EventSpec::Sphere { frame, center, radius, block } => {
                        c |= num(ui, "at frame", frame, 1.0, "Scene frame at which the edit happens");
                        c |= v3(ui, center, 0.5);
                        c |= num(ui, "radius", radius, 0.5, "Sphere radius in blocks");
                        c |= block_combo(ui, "b", block, ctx.block_names, false);
                    }
                    EventSpec::Box { frame, min, max, block } => {
                        c |= num(ui, "at frame", frame, 1.0, "Scene frame at which the edit happens");
                        for (l, v) in [("min", min), ("max", max)] {
                            ui.horizontal(|ui| {
                                ui.label(l).on_hover_text("Box corner in block coordinates (inclusive)");
                                for x in v.iter_mut() {
                                    c |= ui.add(DragValue::new(x)).on_hover_text("Block coordinate").changed();
                                }
                            });
                        }
                        c |= block_combo(ui, "b", block, ctx.block_names, false);
                    }
                    EventSpec::RandomEdits { every, count, seed, center, radius, block } => {
                        c |= num(ui, "every n frames", every, 1.0, "Apply a batch of edits every this many frames");
                        c |= num(ui, "count", count, 10.0, "Blocks changed per batch");
                        c |= num(ui, "seed", seed, 1.0, "Random seed");
                        c |= opt_v3(ui, "center (else camera)", center, [0.0, 40.0, 0.0], ctx.camera, "Where edits happen; unset = around the camera, so they're visible");
                        c |= num(ui, "radius", radius, 0.5, "Edits fall within this distance of the centre");
                        c |= block_combo(ui, "b", block, ctx.block_names, true);
                    }
                    EventSpec::AnimateLights { amplitude, period } => {
                        c |= num(ui, "amplitude", amplitude, 0.1, "How far lights move, in blocks");
                        c |= num(ui, "period", period, 1.0, "Frames per back-and-forth");
                    }
                    EventSpec::SunCycle { period } => c |= num(ui, "period", period, 10.0, "Frames per full day"),
                }
            });
        });
    }
    if let Some(i) = remove {
        es.remove(i);
        c = true;
    }
    if ui.button("+ event").on_hover_text("Add a scripted event (change its kind with the dropdown)").clicked() {
        es.push(EventSpec::AnimateLights { amplitude: 2.0, period: 90 });
        c = true;
    }
    c
}

/// Whether `name` is a back end's renderer (shown through the composition chooser).
fn is_backend_renderer(name: &str) -> bool {
    mr_components::BACKENDS.iter().any(|b| b.renderer == Some(name))
}

/// The component chooser (IDEAS §12): back end, then one component per slot. Options that
/// cannot run are shown disabled with the reason; pairings that run through a generic
/// fallback get a note.
fn composition(ui: &mut Ui, r: &mut RendererSpec) -> Changed {
    use mr_components::{BACKENDS, components, unavailable};
    let Some(mut comp) = r.compose.clone() else { return false };
    let before = comp.clone();
    ui.horizontal_wrapped(|ui| {
        ComboBox::from_id_salt("backend")
            .selected_text(&comp.backend)
            .show_ui(ui, |ui| {
                for b in BACKENDS {
                    let ok = b.renderer.is_some();
                    let resp = ui.add_enabled(ok, egui::Button::selectable(comp.backend == b.name, b.name));
                    if ok {
                        if resp.on_hover_text(b.description).clicked() {
                            comp.backend = b.name.into();
                        }
                    } else {
                        resp.on_disabled_hover_text(b.description);
                    }
                }
            })
            .response
            .on_hover_text("Back end the renderer runs on");
        // Only the slots this back end has (e.g. geometry only on raster back ends).
        for slot in comp.slots() {
            ui.label(format!("{}:", slot.name()));
            ComboBox::from_id_salt(slot.name())
                .selected_text(comp.get(slot).to_string())
                .show_ui(ui, |ui| {
                    for c in components(slot) {
                        let why = unavailable(&comp.backend, slot, c.name);
                        let resp = ui.add_enabled(why.is_none(), egui::Button::selectable(comp.get(slot) == c.name, c.name));
                        match why {
                            None => {
                                if resp.on_hover_text(c.description).clicked() {
                                    comp.set(slot, c.name);
                                }
                            }
                            Some(why) => {
                                resp.on_disabled_hover_text(format!("{}\n\nUnavailable: {why}", c.description));
                            }
                        }
                    }
                })
                .response
                .on_hover_text(components(slot).find(|c| c.name == comp.get(slot)).map_or("", |c| c.description));
        }
    });
    if let Some(note) = mr_components::fallback_note(&comp) {
        ui.weak(format!("ⓘ {note}"));
    }
    if comp != before {
        // Switching back end: keep components that exist there, default the rest.
        comp.adapt_to_backend();
        if let Err(e) = r.set_compose(comp) {
            tracing::warn!("composition: {e:#}");
        }
        return true;
    }
    false
}

fn renderers(ui: &mut Ui, plan: &mut Plan, ctx: &mut EditorCtx) -> Changed {
    let mut c = false;
    ctx.option_text.resize_with(plan.renderers.len(), String::new);
    let mut remove = None;
    for (i, r) in plan.renderers.iter_mut().enumerate() {
        ui.push_id(("renderer", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if r.compose.is_some() {
                        ui.strong("🧩 composed").on_hover_text("Assembled from components (IDEAS §12): pick the back end and one component per slot below");
                    } else {
                        ui.weak("legacy:").on_hover_text("A monolithic renderer not yet split into components");
                        ComboBox::from_id_salt("name")
                            .selected_text(&r.name)
                            .show_ui(ui, |ui| {
                                for info in ctx.infos.iter().filter(|x| !is_backend_renderer(&x.name)) {
                                    c |= ui.selectable_value(&mut r.name, info.name.clone(), &info.name).on_hover_text(&info.description).changed();
                                }
                            })
                            .response
                            .on_hover_text("Which legacy renderer to run");
                    }
                    let mut label = r.label.clone().unwrap_or_default();
                    if ui.add(egui::TextEdit::singleline(&mut label).hint_text("label").desired_width(110.0)).on_hover_text("Name shown in views and reports; needed when the same renderer appears twice with different options").changed() {
                        r.label = (!label.is_empty()).then_some(label);
                        c = true;
                    }
                    if ui.small_button("✖").on_hover_text("Remove this renderer from the plan").clicked() {
                        remove = Some(i);
                    }
                    if ctx.live && ui.small_button("↻").on_hover_text("Restart this renderer in the live session with these settings").clicked() {
                        *ctx.restart_lane = Some(i);
                    }
                });
                if r.compose.is_some() {
                    c |= composition(ui, r);
                } else if let Some(info) = ctx.infos.iter().find(|x| x.name == r.name) {
                    let modes: Vec<_> = info.modes.iter().map(|m| m.name()).collect();
                    ui.weak(format!("[{}] {} — modes: {}", info.tags.join(", "), info.description, modes.join("/")));
                }
                ui.horizontal(|ui| {
                    let mut on = r.view_radius.is_some();
                    if ui.checkbox(&mut on, "own view radius").on_hover_text("overrides run.view_radius for this renderer").changed() {
                        r.view_radius = on.then_some(8);
                        c = true;
                    }
                    if let Some(v) = &mut r.view_radius {
                        c |= ui.add(DragValue::new(v).range(1..=64).suffix(" chunks")).on_hover_text("Chunks (32 blocks) streamed around the camera for this renderer").changed();
                    }
                    let mut on = r.max_lod.is_some();
                    if ui.checkbox(&mut on, "own max LOD").on_hover_text("overrides run.max_lod (capped by what the renderer supports)").changed() {
                        r.max_lod = on.then_some(2);
                        c = true;
                    }
                    if let Some(v) = &mut r.max_lod {
                        c |= ui.add(DragValue::new(v).range(0..=8)).on_hover_text("Coarser detail levels beyond the view radius; each doubles reach at half resolution").changed();
                    }
                });
                let text = &mut ctx.option_text[i];
                ui.label("options (TOML, e.g. shadows = \"sun\")").on_hover_text("Further settings (knobs). See the knob list in the Metrics tab (hover each for a description).");
                if ui.add(egui::TextEdit::multiline(text).code_editor().desired_rows(2).desired_width(f32::INFINITY)).on_hover_text("One `key = value` per line, applied as you type when valid").changed() {
                    if let Ok(t) = text.parse::<toml::Table>() {
                        r.options = t;
                        let _ = r.resolve();
                        c = true;
                    }
                }
                if text.parse::<toml::Table>().is_err() {
                    ui.colored_label(egui::Color32::LIGHT_RED, "invalid TOML (not applied)");
                }
            });
        });
    }
    if let Some(i) = remove {
        plan.renderers.remove(i);
        ctx.option_text.remove(i);
        c = true;
    }
    ui.horizontal(|ui| {
        if ui.button("+ composed renderer").on_hover_text("Assemble a renderer from components: back end, storage, visibility, shading").clicked() {
            plan.renderers.push(RendererSpec::composed(Default::default()));
            ctx.option_text.push(String::new());
            c = true;
        }
        ui.menu_button("+ legacy renderer", |ui| {
            for info in ctx.infos.iter().filter(|x| !is_backend_renderer(&x.name)) {
                if ui.button(&info.name).on_hover_text(&info.description).clicked() {
                    plan.renderers.push(RendererSpec::new(info.name.clone()));
                    ctx.option_text.push(String::new());
                    c = true;
                }
            }
        })
        .response
        .on_hover_text("Monolithic renderers not yet split into components");
    });
    c
}

fn run(ui: &mut Ui, plan: &mut Plan) -> Changed {
    let r = &mut plan.run;
    let mut c = false;
    ui.horizontal(|ui| {
        ui.label("resolution").on_hover_text("Render resolution in pixels (live: before host.render_scale)");
        c |= ui.add(DragValue::new(&mut r.resolution[0]).range(16..=7680)).on_hover_text("Width").changed();
        ui.label("×");
        c |= ui.add(DragValue::new(&mut r.resolution[1]).range(16..=4320)).on_hover_text("Height").changed();
        ui.menu_button("presets", |ui| {
            for (w, h) in [(640, 360), (960, 540), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)] {
                if ui.button(format!("{w}×{h}")).on_hover_text(format!("{:.1} megapixels", (w * h) as f64 / 1e6)).clicked() {
                    r.resolution = [w, h];
                    c = true;
                }
            }
        });
    });
    c |= ui.add(egui::Slider::new(&mut r.view_radius, 1..=64).text("view radius (chunks)")).on_hover_text("Chunks (32 blocks each) streamed around the camera: view distance vs memory").changed();
    c |= ui.add(egui::Slider::new(&mut r.in_flight, 1..=8).text("frames in flight")).on_hover_text("Frames a renderer may have queued. More hides latency and raises throughput but adds input lag.").changed();
    c |= num(ui, "max LOD", &mut r.max_lod, 1.0, "Level-of-detail rings beyond the view radius, for renderers that support LOD (each doubles reach at half resolution)");
    ui.horizontal(|ui| {
        ui.label("streaming").on_hover_text("How world chunks reach renderers");
        let mut budget = matches!(r.streaming, Streaming::Budget { .. });
        if ui.checkbox(&mut budget, "budgeted").on_hover_text("Send at most N chunks per frame (like a game). Off = load everything in range before each frame.").changed() {
            r.streaming = if budget { Streaming::Budget { budget: 400 } } else { Streaming::Complete };
            c = true;
        }
        if let Streaming::Budget { budget } = &mut r.streaming {
            c |= ui.add(DragValue::new(budget).range(1..=100000).suffix(" chunks/frame")).on_hover_text("Chunk uploads per frame").changed();
        }
    });
    ui.horizontal(|ui| {
        ui.label("isolation").on_hover_text("Where renderers run");
        c |= ui.selectable_value(&mut r.isolation, Isolation::Process, "process").on_hover_text("Each renderer in its own process: crashes can't take others down; memory is measured per renderer").changed();
        c |= ui.selectable_value(&mut r.isolation, Isolation::Thread, "thread").on_hover_text("In-process threads: faster to start, easier to debug").changed();
    });
    let mut launcher = r.launcher.join(" ");
    ui.horizontal(|ui| {
        ui.label("launcher").on_hover_text("Command prefix for renderer processes, e.g. a profiler or taskset");
        if ui.add(egui::TextEdit::singleline(&mut launcher).hint_text("e.g. nice -n 5")).on_hover_text("Command prefix for renderer processes, e.g. a profiler or taskset").changed() {
            r.launcher = launcher.split_whitespace().map(String::from).collect();
            c = true;
        }
    });
    ui.separator();
    ui.weak("Benchmark-only:");
    c |= num(ui, "frames", &mut r.frames, 10.0, "Frames measured per pass (also the length of the camera path)");
    c |= num(ui, "warm-up frames", &mut r.warmup, 1.0, "Frames rendered before measuring (shader compilation, caches)");
    ui.horizontal(|ui| {
        ui.label("modes").on_hover_text("Render modes benchmarked; each is a separate pass");
        for m in [PassMode::UNLIT, PassMode::LIT, PassMode { lighting: mr_core::RenderMode::Unlit, meshes: true }, PassMode { lighting: mr_core::RenderMode::Lit, meshes: true }] {
            let mut on = r.modes.contains(&m);
            if ui.checkbox(&mut on, m.name()).on_hover_text(match (m.lighting, m.meshes) {
                (mr_core::RenderMode::Lit, false) => "Sun, sky, point lights and shadows",
                (mr_core::RenderMode::Unlit, false) => "Textures only",
                (mr_core::RenderMode::Lit, true) => "Lit, with the scene's meshes (renderers with the `meshes` tag)",
                (mr_core::RenderMode::Unlit, true) => "Unlit, with the scene's meshes (renderers with the `meshes` tag)",
            }).changed() {
                r.modes.retain(|x| *x != m);
                if on {
                    r.modes.push(m);
                    r.modes.sort();
                }
                c = true;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("pacing").on_hover_text("How fast frames are requested");
        let mut paced = matches!(r.pacing, Pacing::Fps { .. });
        if ui.checkbox(&mut paced, "fixed fps").on_hover_text("Request frames at a fixed rate and count missed deadlines. Off = as fast as possible.").changed() {
            r.pacing = if paced { Pacing::Fps { fps: 60.0 } } else { Pacing::Max };
            c = true;
        }
        if let Pacing::Fps { fps } = &mut r.pacing {
            c |= ui.add(DragValue::new(fps).range(1.0..=10000.0)).on_hover_text("Requested frame rate").changed();
        }
    });
    c |= ui.checkbox(&mut r.parallel, "run renderers in parallel").on_hover_text("Run all renderers at once (they share the GPU and CPU, so numbers interfere). Off = one at a time.").changed();
    c |= ui.add(egui::Slider::new(&mut r.repeats, 1..=10).text("repeats (round-robin, median)")).on_hover_text("GPU clocks drift between runs; repeat passes and report medians and spread").changed();
    c
}

fn output(ui: &mut Ui, plan: &mut Plan, infos: &[RendererInfo]) -> Changed {
    let o = &mut plan.output;
    let mut c = false;
    ui.horizontal(|ui| {
        ui.label("read-back").on_hover_text("Whether frames are copied back to the host each frame");
        for (v, n, tip) in [
            (Readback::Auto, "auto", "Only when a frame is needed (saving, comparison, window)"),
            (Readback::Always, "always", "Every frame: measures the cost of getting pixels out, like a real app would pay"),
            (Readback::Never, "never", "Never: pure render speed"),
        ] {
            c |= ui.selectable_value(&mut o.readback, v, n).on_hover_text(tip).changed();
        }
    });
    c |= ui.checkbox(&mut o.window, "show window during benchmark").on_hover_text("Display frames while the benchmark runs (costs read-back)").changed();
    let mut save = o.save.is_some();
    if ui.checkbox(&mut save, "save frames").on_hover_text("Write PNG frames into the run directory").changed() {
        o.save = save.then_some(SaveSpec { every: 100, diffs: true });
        c = true;
    }
    if let Some(s) = &mut o.save {
        c |= num(ui, "  every n frames", &mut s.every, 1.0, "Save one frame out of this many");
        c |= ui.checkbox(&mut s.diffs, "  with diff images").on_hover_text("Also save difference images against the reference").changed();
    }
    let mut cmp = o.compare.is_some();
    if ui.checkbox(&mut cmp, "compare against reference").on_hover_text("Score frames against a reference renderer (PSNR, SSIM, bad pixels)").changed() {
        o.compare = cmp.then(|| CompareSpec { reference: "cpu-reference".into(), reference_options: Default::default(), every: 100 });
        c = true;
    }
    if let Some(cs) = &mut o.compare {
        ui.horizontal(|ui| {
            ui.label("  reference").on_hover_text("Renderer treated as ground truth (cpu-reference is the exact categorical reference)");
            ComboBox::from_id_salt("ref")
                .selected_text(&cs.reference)
                .show_ui(ui, |ui| {
                    for i in infos {
                        c |= ui.selectable_value(&mut cs.reference, i.name.clone(), &i.name).on_hover_text(&i.description).changed();
                    }
                })
                .response
                .on_hover_text("Renderer treated as ground truth");
        });
        c |= num(ui, "  every n frames", &mut cs.every, 1.0, "Compare one frame out of this many (the reference is slow)");
    }
    let mut dir = o.dir.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("run dir").on_hover_text("Directory for benchmark results; each run gets a time-stamped subdirectory");
        if ui.add(egui::TextEdit::singleline(&mut dir).hint_text("runs")).on_hover_text("Directory for benchmark results; each run gets a time-stamped subdirectory").changed() {
            o.dir = (!dir.is_empty()).then_some(dir);
            c = true;
        }
    });
    c
}
