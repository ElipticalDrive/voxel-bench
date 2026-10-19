//! Structured editor for run plans.

use egui::{ComboBox, DragValue, Ui};
use mr_core::{Camera, PassMode, RendererInfo};
use mr_world::GenSpec;
use mr_world::scenario::{CameraPathSpec, EnvSpec, EventSpec, LightSpec};

use crate::plan::{CompareSpec, Compile, Isolation, Pacing, Plan, Readback, RendererSpec, SaveSpec, Streaming};

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
    /// One `Trial` per renderer (the pipeline editor's choices).
    pub trials: &'a mut super::graph::Trials,
    /// Set when the user opens the pipeline editor on a renderer (and a node).
    pub open_graph: &'a mut Option<(usize, Option<String>)>,
}

type Changed = bool;

fn v3(ui: &mut Ui, v: &mut [f64; 3], speed: f64) -> Changed {
    let mut c = false;
    ui.horizontal(|ui| {
        for (x, l) in v.iter_mut().zip(["x", "y", "z"]) {
            ui.label(l);
            c |= ui.add(DragValue::new(x).speed(speed).max_decimals(2).update_while_editing(false)).on_hover_text(format!("{l} in blocks (y is up). Drag or double-click to type.")).changed();
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
        if let Some(p) = cam
            && ui.small_button("⌖ cam").on_hover_text("Use the live camera position").clicked() {
            *v = Some(p.position.to_array());
            c = true;
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
        ui.add(DragValue::new(v).speed(speed).update_while_editing(false)).on_hover_text(tip).changed()
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
        GenSpec::City { .. } => "city",
        GenSpec::Islands { .. } => "islands",
        GenSpec::Lattice { .. } => "lattice",
        GenSpec::Gyroid { .. } => "gyroid",
        GenSpec::Checker { .. } => "checker",
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
                    ("city", || GenSpec::City { seed: 0, block: 48, max_height: 240 }, "Street grid of hollow towers with windows: long rays down the streets, deep overdraw behind the facades"),
                    ("islands", || GenSpec::Islands { seed: 0, spacing: 96, height: 512, fill: 0.35 }, "Floating islands in a tall volume: mostly empty space, most rays hit nothing"),
                    ("lattice", || GenSpec::Lattice { period: 24, thickness: 1, height: 160 }, "Jungle gym of thin beams: every ray grazes geometry; thin features at LOD"),
                    ("gyroid", || GenSpec::Gyroid { period: 48, thickness: 0.3, height: 192 }, "Gyroid caves: curved walls nothing can merge, two interleaved tunnel networks"),
                    ("checker", || GenSpec::Checker { size: 96, cell: 1 }, "3D checkerboard cube: every block shows all six faces, yet perfectly regular"),
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
            c |= ui.add(egui::Slider::new(level, 1..=7).text("level (3^level blocks)").update_while_editing(false)).on_hover_text("Recursion depth; each level triples the size (level 5 = 243 blocks across)").changed();
        }
        GenSpec::Room { size } => c |= num(ui, "inner size", size, 1.0, "Interior width of the room in blocks"),
        GenSpec::Random { size, density, seed } => {
            c |= num(ui, "size", size, 1.0, "Edge length of the random cube in blocks");
            c |= ui.add(egui::Slider::new(density, 0.0..=1.0).text("density").update_while_editing(false)).on_hover_text("Fraction of cells that are solid").changed();
            c |= num(ui, "seed", seed, 1.0, "Random seed");
        }
        GenSpec::City { seed, block, max_height } => {
            c |= num(ui, "seed", seed, 1.0, "Random seed: building heights and materials");
            c |= num(ui, "block", block, 1.0, "Street grid period in blocks (a 10-block street, the rest one building plot)");
            c |= num(ui, "max height", max_height, 1.0, "Tallest possible tower; most buildings are much lower");
        }
        GenSpec::Islands { seed, spacing, height, fill } => {
            c |= num(ui, "seed", seed, 1.0, "Random seed");
            c |= num(ui, "spacing", spacing, 1.0, "Cell size in blocks: at most one island per cell, up to about a third of it across");
            c |= num(ui, "height", height, 1.0, "Height of the island volume in blocks (from y = 0)");
            c |= ui.add(egui::Slider::new(fill, 0.0..=1.0).text("fill").update_while_editing(false)).on_hover_text("Fraction of cells holding an island").changed();
        }
        GenSpec::Lattice { period, thickness, height } => {
            c |= num(ui, "period", period, 1.0, "Distance between beams in blocks");
            c |= num(ui, "thickness", thickness, 1.0, "Beam thickness in blocks (less than the period)");
            c |= num(ui, "height", height, 1.0, "Height of the lattice in blocks");
        }
        GenSpec::Gyroid { period, thickness, height } => {
            c |= num(ui, "period", period, 1.0, "Repeat length of the gyroid in blocks");
            c |= ui.add(egui::Slider::new(thickness, 0.05..=1.2).text("thickness").update_while_editing(false)).on_hover_text("Wall half-thickness in units of the gyroid function; above about 1 the caves close up").changed();
            c |= num(ui, "height", height, 1.0, "Height of the cave volume in blocks");
        }
        GenSpec::Checker { size, cell } => {
            c |= num(ui, "size", size, 1.0, "Edge length of the cube in blocks");
            c |= num(ui, "cell", cell, 1.0, "Checker cell size in blocks (1 = every other block)");
        }
        GenSpec::Empty | GenSpec::Showcase => {}
    }
    c |= ui.add(egui::Slider::new(&mut plan.scene.fov_deg, 20.0..=140.0).text("FOV°").update_while_editing(false)).on_hover_text("Vertical field of view in degrees").changed();
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
            if let Some(k) = cam
                && ui.button("⌖ Set from live camera").on_hover_text("Copy the live camera's position and direction").clicked() {
                *position = Some(k.position.to_array());
                *target = None;
                *yaw_deg = k.yaw.to_degrees();
                *pitch_deg = k.pitch.to_degrees();
                c = true;
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
                if let Some(k) = cam
                    && ui.button("+ live camera position").on_hover_text("Append the live camera's position: fly somewhere, click, repeat").clicked() {
                    points.push(k.position.to_array());
                    c = true;
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
                    c |= ui.add(DragValue::new(x).speed(0.01).range(-1.0..=1.0).update_while_editing(false)).on_hover_text("Direction component (x, y, z); y should be negative for daylight").changed();
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
            c |= ui.add(DragValue::new(x).speed(0.01).range(0.0..=20.0).update_while_editing(false)).on_hover_text("1 = neutral; higher is brighter").changed();
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
                            if let Some(k) = cam
                                && ui.small_button("⌖ cam").on_hover_text("Use the live camera position").clicked() {
                                *position = k.position.to_array();
                                c = true;
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
                                    c |= ui.add(DragValue::new(x).update_while_editing(false)).on_hover_text("Block coordinate").changed();
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

/// Whether `name` is a back end's renderer (edited as a pipeline graph).
fn is_backend_renderer(name: &str) -> bool {
    mr_components::BACKENDS.iter().any(|b| b.renderer == Some(name))
}

/// A composed renderer's summary (`graph::summary`): edits go into it. Returns whether
/// anything changed.
fn composition(ui: &mut Ui, i: usize, r: &mut RendererSpec, run: &crate::plan::RunSpec, ctx: &mut EditorCtx) -> Changed {
    use super::graph::{self, Of};
    let params: &[mr_core::ParamSpec] = ctx.infos.iter().find(|x| x.name == r.name).map_or(&[], |x| &x.params);
    let knobs: Vec<_> = params.iter().map(|p| (p.clone(), super::knobs::option_value(p, &r.options))).collect();
    let edits = graph::summary(ui, &format!("{:?}", Of::Plan(i)), r, &knobs, false, None, ctx.trials, ctx.infos, run);
    let (changed, open) = graph::apply_to_plan(r, edits);
    if let Some(sel) = open {
        *ctx.open_graph = Some((i, sel));
    }
    changed
}

fn renderers(ui: &mut Ui, plan: &mut Plan, ctx: &mut EditorCtx) -> Changed {
    let mut c = false;
    ctx.option_text.resize_with(plan.renderers.len(), String::new);
    let mut remove = None;
    let run = plan.run.clone();
    for (i, r) in plan.renderers.iter_mut().enumerate() {
        ui.push_id(("renderer", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if r.compose.is_some() {
                        ui.strong(r.label()).on_hover_text("A pipeline graph: a row per node with its component and knobs; ✏ graph opens the editor");
                    } else {
                        ui.strong(&r.name).on_hover_text("A standalone renderer (cpu-reference defines the reference images)");
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
                    if composition(ui, i, r, &run, ctx) {
                        ctx.option_text[i] = toml::to_string(&r.extra_options()).unwrap_or_default();
                        c = true;
                    }
                } else if let Some(info) = ctx.infos.iter().find(|x| x.name == r.name) {
                    let modes: Vec<_> = info.modes.iter().map(|m| m.name()).collect();
                    ui.weak(format!("[{}] {} — modes: {}", info.tags.join(", "), info.description, modes.join("/")));
                }
                let text = &mut ctx.option_text[i];
                egui::CollapsingHeader::new("options as TOML").id_salt("toml").show(ui, |ui| {
                    if ui.add(egui::TextEdit::multiline(text).code_editor().desired_rows(2).desired_width(f32::INFINITY)).on_hover_text("The knobs above as `key = value` lines (only the ones set); edits apply as you type when valid").changed()
                        && let Ok(t) = text.parse::<toml::Table>() {
                        r.options = t;
                        let _ = r.resolve();
                        c = true;
                    }
                    if text.parse::<toml::Table>().is_err() {
                        ui.colored_label(egui::Color32::LIGHT_RED, "invalid TOML (not applied)");
                    }
                });
            });
        });
    }
    if let Some(i) = remove {
        plan.renderers.remove(i);
        ctx.option_text.remove(i);
        c = true;
    }
    ui.horizontal(|ui| {
        if ui.button("+ renderer").on_hover_text("Add a renderer (gpu-compute's default pipeline; then change its graph: ✏ graph)").clicked() {
            plan.renderers.push(RendererSpec::composed(Default::default()));
            ctx.option_text.push(String::new());
            c = true;
        }
        for info in ctx.infos.iter().filter(|x| !is_backend_renderer(&x.name)) {
            if ui.button(format!("+ {}", info.name)).on_hover_text(format!("Add the standalone renderer: {}", info.description)).clicked() {
                plan.renderers.push(RendererSpec::new(info.name.clone()));
                ctx.option_text.push(String::new());
                c = true;
            }
        }
    });
    c
}

fn run(ui: &mut Ui, plan: &mut Plan) -> Changed {
    use super::knobs::{fit_slider, form_row};
    let r = &mut plan.run;
    let mut c = false;
    ui.weak("A renderer's reach (load radius, LOD) is its client node's knobs.");
    egui::Grid::new("run-settings").num_columns(2).spacing([6.0, 2.0]).show(ui, |ui| {
        c |= form_row(ui, "resolution", "Render resolution in pixels (live: before host.render_scale)", |ui| {
            let mut c = ui.add(DragValue::new(&mut r.resolution[0]).range(16..=7680).update_while_editing(false)).on_hover_text("Width").changed();
            ui.label("×");
            c |= ui.add(DragValue::new(&mut r.resolution[1]).range(16..=4320).update_while_editing(false)).on_hover_text("Height").changed();
            ui.menu_button("presets", |ui| {
                for (w, h) in [(640, 360), (960, 540), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)] {
                    if ui.button(format!("{w}×{h}")).on_hover_text(format!("{:.1} megapixels", (w * h) as f64 / 1e6)).clicked() {
                        r.resolution = [w, h];
                        c = true;
                    }
                }
            });
            c
        });
        let tip = "Frames a renderer may have queued. More hides latency and raises throughput but adds input lag.";
        c |= form_row(ui, "frames in flight", tip, |ui| {
            fit_slider(ui);
            ui.add(egui::Slider::new(&mut r.in_flight, 1..=8).update_while_editing(false)).on_hover_text(tip).changed()
        });
        c |= form_row(ui, "streaming", "How world chunks reach renderers", |ui| {
            let mut c = false;
            let mut budget = matches!(r.streaming, Streaming::Budget { .. });
            if ui.checkbox(&mut budget, "").on_hover_text("Budgeted: send at most N chunks per frame (like a game). Off = load everything in range before each frame.").changed() {
                r.streaming = if budget { Streaming::Budget { budget: 400 } } else { Streaming::Complete };
                c = true;
            }
            match &mut r.streaming {
                Streaming::Budget { budget } => c |= ui.add(DragValue::new(budget).range(1..=100000).suffix(" chunks/frame").update_while_editing(false)).on_hover_text("Chunk uploads per frame").changed(),
                Streaming::Complete => {
                    ui.weak("complete");
                }
            }
            c
        });
        c |= form_row(ui, "isolation", "Where renderers run", |ui| {
            ui.selectable_value(&mut r.isolation, Isolation::Process, "process").on_hover_text("Each renderer in its own process: crashes can't take others down; memory is measured per renderer").changed()
                | ui.selectable_value(&mut r.isolation, Isolation::Thread, "thread").on_hover_text("In-process threads: faster to start, easier to debug").changed()
        });
        c |= form_row(ui, "compile", "CPU executor pipelines (cpu, cpu-raster, cpu-voxelspace): the generic build, or a pipeline compiled for this machine", |ui| {
            let mut c = ui.selectable_value(&mut r.compile, Compile::Off, "generic").on_hover_text("The precompiled build: every knob live").changed();
            let tip = "CPU executor pipelines run as pipelines compiled for this CPU with their choices baked in (seconds to build once, cached; checked against the generic build before use; process isolation). The generic build runs meanwhile, then hot-swaps.";
            c |= ui.add_enabled_ui(r.isolation == Isolation::Process, |ui| ui.selectable_value(&mut r.compile, Compile::Native, "native").on_hover_text(tip).on_disabled_hover_text("Compiled pipelines run in their own process: choose process isolation")).inner.changed();
            let tip = "As native, built for speed whatever it takes: native, whole-program LTO, profile-guided (trained on a short flight) and both, each timed on this machine, the fastest kept (minutes, once per pipeline and machine). The generic build runs meanwhile, then hot-swaps.";
            c |= ui.add_enabled_ui(r.isolation == Isolation::Process, |ui| ui.selectable_value(&mut r.compile, Compile::BreakIn, "break-in").on_hover_text(tip).on_disabled_hover_text("Compiled pipelines run in their own process: choose process isolation")).inner.changed();
            c
        });
        let tip = "Command prefix for renderer processes, e.g. a profiler or taskset";
        c |= form_row(ui, "launcher", tip, |ui| {
            let mut launcher = r.launcher.join(" ");
            let changed = ui.add(egui::TextEdit::singleline(&mut launcher).hint_text("e.g. nice -n 5").desired_width(super::knobs::VALUE_W - 8.0)).on_hover_text(tip).changed();
            if changed {
                r.launcher = launcher.split_whitespace().map(String::from).collect();
            }
            changed
        });
    });
    ui.separator();
    ui.weak("Benchmark-only:");
    egui::Grid::new("run-bench").num_columns(2).spacing([6.0, 2.0]).show(ui, |ui| {
        let tip = "Frames measured per pass (also the length of the camera path)";
        c |= form_row(ui, "frames", tip, |ui| ui.add(DragValue::new(&mut r.frames).speed(10.0).update_while_editing(false)).on_hover_text(tip).changed());
        let tip = "Frames rendered before measuring (shader compilation, caches)";
        c |= form_row(ui, "warm-up frames", tip, |ui| ui.add(DragValue::new(&mut r.warmup).speed(1.0).update_while_editing(false)).on_hover_text(tip).changed());
        for m in [PassMode::UNLIT, PassMode::LIT, PassMode { lighting: mr_core::RenderMode::Unlit, meshes: true }, PassMode { lighting: mr_core::RenderMode::Lit, meshes: true }] {
            let tip = match (m.lighting, m.meshes) {
                (mr_core::RenderMode::Lit, false) => "A pass with sun, sky, point lights and shadows",
                (mr_core::RenderMode::Unlit, false) => "A pass with textures only",
                (mr_core::RenderMode::Lit, true) => "A lit pass with the scene's meshes (renderers with the `meshes` tag)",
                (mr_core::RenderMode::Unlit, true) => "An unlit pass with the scene's meshes (renderers with the `meshes` tag)",
            };
            let mut on = r.modes.contains(&m);
            if form_row(ui, &format!("mode {}", m.name()), tip, |ui| ui.checkbox(&mut on, "").on_hover_text(tip).changed()) {
                r.modes.retain(|x| *x != m);
                if on {
                    r.modes.push(m);
                    r.modes.sort();
                }
                c = true;
            }
        }
        c |= form_row(ui, "pacing", "How fast frames are requested", |ui| {
            let mut c = false;
            let mut paced = matches!(r.pacing, Pacing::Fps { .. });
            if ui.checkbox(&mut paced, "").on_hover_text("Fixed fps: request frames at a fixed rate and count missed deadlines. Off = as fast as possible.").changed() {
                r.pacing = if paced { Pacing::Fps { fps: 60.0 } } else { Pacing::Max };
                c = true;
            }
            match &mut r.pacing {
                Pacing::Fps { fps } => c |= ui.add(DragValue::new(fps).range(1.0..=10000.0).suffix(" fps").update_while_editing(false)).on_hover_text("Requested frame rate").changed(),
                Pacing::Max => {
                    ui.weak("as fast as possible");
                }
            }
            c
        });
        let tip = "Run all renderers at once (they share the GPU and CPU, so numbers interfere). Off = one at a time.";
        c |= form_row(ui, "renderers in parallel", tip, |ui| ui.checkbox(&mut r.parallel, "").on_hover_text(tip).changed());
        let tip = "GPU clocks drift between runs; repeat passes and report medians and spread";
        c |= form_row(ui, "repeats (round-robin, median)", tip, |ui| {
            fit_slider(ui);
            ui.add(egui::Slider::new(&mut r.repeats, 1..=10).update_while_editing(false)).on_hover_text(tip).changed()
        });
    });
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
