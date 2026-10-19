//! The session thread: startup, lanes, commands, world edits and presenter windows.

use std::time::Duration;

use mr_core::renderer::BlockEdit;
use mr_world::scenario::resolve_edits;
use mr_world::textures::default_registry;

use crate::plan::{RunSpec, Streaming};
use crate::{present, tune};

use super::*;

/// A component knob changed in the options: carry it into the composition, so restarts and
/// the plan keep it.
fn sync_compose(spec: &mut RendererSpec, knob: &str) {
    let Some(slot) = mr_components::Slot::ALL.into_iter().find(|s| s.name() == knob) else { return };
    let Some(v) = spec.options.get(knob).and_then(|v| v.as_str()).map(String::from) else { return };
    if let Some(mut c) = spec.compose.clone() {
        c.set(slot, &v);
        let _ = spec.set_compose(c);
    }
}

fn spawn_async(run: &RunSpec, spec: &RendererSpec, world: &mr_world::GenSpec, blocks: Arc<BlockRegistry>, res: [u32; 2]) -> Spawn {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let (run, spec, world) = (run.clone(), spec.clone(), world.clone());
    std::thread::spawn(move || {
        let _ = tx.send(worker::spawn_renderer(&run, &spec, &world, blocks, None, res, SLOTS));
    });
    rx
}

fn budget(plan: &Plan) -> Option<usize> {
    Some(match plan.run.streaming {
        Streaming::Complete => 2048,
        Streaming::Budget { budget } => budget,
    })
}

impl Session {
    pub(super) fn run(plan: Plan, carry: Vec<Carry>, control: Arc<Mutex<Control>>, shared: Arc<Mutex<Shared>>, rx: Receiver<Cmd>, repaint: Box<dyn Fn() + Send>) -> anyhow::Result<()> {
        shared.lock().unwrap().status = "generating world".into();
        let blocks = Arc::new(default_registry());
        let mut world = World::new(plan.scene.generator.build());
        let scene = Scene::new(plan.scene.clone(), blocks.clone(), &mut world, plan.run.frames)?;
        let lights = scene.initial_lights().iter().map(|l| (l.id, *l)).collect();
        let env = scene.environment;
        let mesh_data = scene.mesh_data();
        let instances = scene.mesh_instances(0);
        let server = Streamer::new(vec![plan.run.view_radius], budget(&plan));
        let start = Instant::now();
        shared.lock().unwrap().start = Some(start);
        let mut s = Session {
            blocks,
            world,
            server,
            scene,
            env,
            lights,
            mesh_data,
            instances,
            lanes: vec![],
            reference: None,
            shared,
            repaint,
            start,
            applied_frame: None,
            next_light: USER_LIGHT_BASE,
            probe: None,
            probe_seq: 0,
            last_probe: Instant::now(),
            compare_seq: 0,
            presenters: BTreeMap::new(),
            plan,
        };
        s.shared.lock().unwrap().status = "running".into();
        let renderers = s.plan.renderers.clone();
        let same = carry.len() == renderers.len();
        for (i, spec) in renderers.into_iter().enumerate() {
            let label = toml::to_string(&spec).ok();
            let c = if same { carry.get(i) } else { carry.iter().find(|c| toml::to_string(&c.given).ok() == label) };
            s.add_lane(spec, c);
        }
        loop {
            let mut busy = false;
            while let Ok(cmd) = rx.try_recv() {
                busy = true;
                if let Cmd::Stop = cmd {
                    s.shutdown();
                    return Ok(());
                }
                s.command(cmd);
            }
            let ctl = *control.lock().unwrap();
            busy |= s.tick(&ctl)?;
            if !busy {
                std::thread::sleep(Duration::from_micros(250));
            }
        }
    }

    pub(super) fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn new_runner(&self, spec: RendererSpec, res: [u32; 2], radius: u32) -> Runner {
        let (levels, generates) = (spec.levels(), spec.generates());
        Runner {
            spawning: Some(spawn_async(&self.plan.run, &spec, &self.plan.scene.generator, self.blocks.clone(), res)),
            spec,
            worker: None,
            region: Region { radius, max_lod: 0, levels, generates },
            joined: false,
            chunks_sent: 0,
            own: None,
            pending: vec![WorldUpdate::LightUpsert(self.lights.values().copied().collect()), WorldUpdate::SetEnvironment(self.env)],
            pending_meshes: if self.mesh_data.is_empty() {
                vec![]
            } else {
                vec![MeshUpdate::Meshes(self.mesh_data.clone()), MeshUpdate::Instances(self.instances.clone())]
            },
            pending_params: vec![],
            in_flight: VecDeque::new(),
            next_id: 0,
            res,
        }
    }

    pub(super) fn new_lane(&self, spec: RendererSpec) -> Lane {
        let radius = spec.view_radius(&self.plan.run);
        let mut values = BTreeMap::new();
        for p in tune::host_params(radius) {
            values.insert(p.name.clone(), p.default.clone());
        }
        values.insert("host.in_flight".into(), ParamValue::Int(self.plan.run.in_flight.clamp(1, 8) as i64));
        Lane {
            given: spec.clone(),
            run: self.new_runner(spec, self.plan.run.resolution, radius),
            info: None,
            standby: None,
            standby_warm: 0,
            values,
            want_res: None,
            last_done: None,
            last_sent: None,
            last_display: None,
            last_mem: Instant::now(),
            seq: 0,
            tuner: None,
        }
    }

    fn add_lane(&mut self, given: RendererSpec, carry: Option<&Carry>) {
        let spec = carry.map_or(given.clone(), |c| keep_knobs(&c.given, &c.now, given.clone()));
        // A pipeline compiled for this machine: the generic build runs while
        // it compiles and is checked, then it hot-swaps in.
        let native = spec.compiled(&self.plan.run).is_some();
        let mut first = spec.clone();
        if native {
            first.compile = Some(crate::plan::Compile::Off);
        }
        let mut lane = self.new_lane(first.clone());
        lane.given = given;
        self.lanes.push(lane);
        self.shared.lock().unwrap().lanes.push(LaneView::new(first));
        let i = self.lanes.len() - 1;
        self.publish_params(i);
        // Host knobs changed in the previous session whose default the plan left alone.
        for (name, default, value) in carry.map_or(&[][..], |c| &c.host[..]) {
            if value != default && self.specs(i).iter().any(|p| p.name == *name && p.default == *default) {
                self.set_param(i, name, value.clone());
            }
        }
        if native {
            self.hot_swap(i, spec);
        }
        (self.repaint)();
    }

    pub(super) fn lane_mut(&mut self, i: usize) -> Option<&mut Lane> {
        if i == REFERENCE { self.reference.as_mut() } else { self.lanes.get_mut(i) }
    }

    pub(super) fn log(&self, s: impl Into<String>) {
        self.shared.lock().unwrap().log(s);
    }

    /// All knobs of a lane: host ones, then the renderer's.
    fn specs(&self, i: usize) -> Vec<ParamSpec> {
        let radius = self.lanes.get(i).map_or(self.plan.run.view_radius, |l| l.given.view_radius(&self.plan.run));
        let mut v = tune::host_params(radius);
        if let Some(info) = self.lanes.get(i).and_then(|l| l.info.as_ref()) {
            if info.max_lod > 0 {
                v.push(ParamSpec::int("host.max_lod", "Coarsest LOD level its client holds: rings beyond the view radius, each doubling reach at half resolution (restart)", 0, info.max_lod as i64, 0, false));
            }
            v.extend(info.params.iter().cloned());
        }
        v
    }

    pub(super) fn publish_params(&self, i: usize) {
        let Some(lane) = self.lanes.get(i) else { return };
        let params = self.specs(i).into_iter().map(|s| (s.clone(), lane.values.get(&s.name).cloned().unwrap_or(s.default))).collect();
        let mut sh = self.shared.lock().unwrap();
        if let Some(v) = sh.lanes.get_mut(i) {
            v.params = params;
            v.spec = lane.latest().clone();
            v.given = lane.given.clone();
            v.resolution = lane.run.res;
        }
    }

    pub(super) fn event(&self, i: usize, what: String) {
        let t = self.now();
        let mut sh = self.shared.lock().unwrap();
        if let Some(v) = sh.lanes.get_mut(i) {
            v.events.push((t, what.clone()));
        }
        let label = sh.lanes.get(i).map(|v| v.spec.label().to_string()).unwrap_or_default();
        sh.log(format!("{label}: {what}"));
    }

    pub(super) fn fan_out(&mut self, u: WorldUpdate) {
        for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
            l.run.pending.push(u.clone());
            if let Some(s) = &mut l.standby {
                s.pending.push(u.clone());
            }
        }
    }

    pub(super) fn apply_edits(&mut self, edits: Vec<BlockEdit>) {
        let edits = resolve_edits(&mut self.world, edits);
        if edits.is_empty() {
            return;
        }
        let changed = self.world.apply_edits(&edits);
        for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
            if let Some(own) = &mut l.run.own {
                let mut stream = vec![];
                own.edits(&edits, &changed, &mut stream);
                l.run.take(&stream);
            }
        }
        let mut stream = vec![];
        self.server.edits(&edits, &changed, &mut stream);
        // Clients generating the world get every edit.
        let all = [WorldUpdate::SetBlocks(edits)];
        for r in self.lanes.iter_mut().flat_map(|l| std::iter::once(&mut l.run).chain(l.standby.as_mut())) {
            if r.joined {
                r.take(if r.region.generates { &all } else { &stream });
            }
        }
    }

    /// Changes one knob of lane `i`.
    pub(super) fn set_param(&mut self, i: usize, name: &str, value: ParamValue) {
        let Some(spec) = self.specs(i).into_iter().find(|s| s.name == name) else {
            self.log(format!("lane {i}: unknown knob `{name}`"));
            return;
        };
        let Some(value) = spec.clamp(&value) else { return };
        let base = self.plan.run.resolution;
        let Some(lane) = self.lanes.get_mut(i) else { return };
        if lane.values.get(name) == Some(&value) {
            return;
        }
        lane.values.insert(name.to_string(), value.clone());
        match name {
            "host.render_scale" => {
                let s = value.as_f64().unwrap_or(1.0);
                let r = [((base[0] as f64 * s).round() as u32).max(16), ((base[1] as f64 * s).round() as u32).max(16)];
                if r != lane.run.res {
                    lane.want_res = Some(r);
                }
            }
            "host.view_radius" => {
                let r = value.as_i64().unwrap_or(8) as u32;
                for run in std::iter::once(&mut lane.run).chain(lane.standby.as_mut()) {
                    run.region.radius = r;
                    run.pending_params.push(("client.radius".into(), ParamValue::Int(r as i64)));
                }
            }
            "host.in_flight" => {}
            "host.max_lod" => {
                let mut new = lane.run.spec.clone();
                if let Err(e) = new.set_client_knob("max_lod", Some(value.as_i64().unwrap_or(0))) {
                    self.log(format!("lane {i}: max LOD: {e}"));
                    return;
                }
                self.hot_swap(i, new);
            }
            _ if spec.live => {
                // The replacement a hot swap is loading takes it too.
                let back_end = lane.run.spec.name.clone();
                for run in std::iter::once(&mut lane.run).chain(lane.standby.as_mut().filter(|s| s.spec.name == back_end)) {
                    run.spec.options.insert(name.into(), crate::gui::param_to_toml(&value));
                    sync_compose(&mut run.spec, name);
                    run.pending_params.push((name.to_string(), value.clone()));
                }
            }
            _ => {
                let mut new = lane.latest().clone();
                new.options.insert(name.into(), crate::gui::param_to_toml(&value));
                sync_compose(&mut new, name);
                self.hot_swap(i, new);
            }
        }
        self.publish_params(i);
    }

    fn hot_swap(&mut self, i: usize, spec: RendererSpec) {
        let radius = self.lanes[i].run.region.radius;
        let res = self.lanes[i].run.res;
        let standby = self.new_runner(spec.clone(), res, radius);
        let lane = &mut self.lanes[i];
        if let Some(old) = lane.standby.take().and_then(|s| s.worker) {
            std::thread::spawn(move || old.shutdown());
        }
        lane.standby = Some(standby);
        lane.standby_warm = 0;
        let native = spec.compiled(&self.plan.run).is_some();
        self.shared.lock().unwrap().lanes[i].swap = Some(if native { "compiling for this machine (generic build meanwhile)".into() } else { "spawning".into() });
        self.event(i, format!("hot swap ➡ {} {}", spec.name, toml::to_string(&spec.options).unwrap_or_default().replace('\n', " ")));
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Edit { origin, dir, place } => {
                if let Some((hit, normal)) = self.world.raycast(origin, dir.as_dvec3(), 512.0) {
                    let edit = match place {
                        Some(b) => BlockEdit { pos: hit + normal, block: b },
                        None => BlockEdit { pos: hit, block: BlockId::AIR },
                    };
                    self.apply_edits(vec![edit]);
                }
            }
            Cmd::AddLight { position, color, intensity, radius } => {
                let l = Light { id: self.next_light, position, color, intensity, radius };
                self.next_light += 1;
                self.lights.insert(l.id, l);
                self.fan_out(WorldUpdate::LightUpsert(vec![l]));
                self.log(format!("light {} at ({:.1}, {:.1}, {:.1})", l.id - USER_LIGHT_BASE, position.x, position.y, position.z));
            }
            Cmd::ClearUserLights => {
                let ids: Vec<u32> = self.lights.keys().copied().filter(|&id| id >= USER_LIGHT_BASE).collect();
                for id in &ids {
                    self.lights.remove(id);
                }
                self.fan_out(WorldUpdate::LightRemove(ids));
            }
            Cmd::SetParam { lane, name, value } => self.set_param(lane, &name, value),
            Cmd::Restart { lane, spec } => {
                if let Some(l) = self.lanes.get_mut(lane) {
                    let kept = keep_knobs(&l.given, l.latest(), spec.clone());
                    l.given = spec;
                    self.hot_swap(lane, kept);
                }
            }
            Cmd::Swap { lane, spec } => {
                if lane < self.lanes.len() {
                    self.hot_swap(lane, spec);
                }
            }
            Cmd::AddLane(spec) => self.add_lane(spec, None),
            Cmd::RemoveLane(i) => {
                if i < self.lanes.len() {
                    let lane = self.lanes.remove(i);
                    for w in [lane.run.worker, lane.standby.and_then(|s| s.worker)].into_iter().flatten() {
                        std::thread::spawn(move || w.shutdown());
                    }
                    self.presenters.remove(&i);
                    self.presenters = std::mem::take(&mut self.presenters).into_iter().map(|(k, p)| (if k > i { k - 1 } else { k }, p)).collect();
                    let mut sh = self.shared.lock().unwrap();
                    sh.lanes.remove(i);
                    sh.comparing = false;
                    self.probe = None;
                }
            }
            Cmd::Compare => self.request_probe(true),
            Cmd::Tune { lane, config } => {
                let now = self.now();
                let specs = self.specs(lane);
                let Some(l) = self.lanes.get_mut(lane) else { return };
                // Stopping (or restarting) restores the best configuration found.
                let was_tuning = l.tuner.is_some();
                let best: Vec<(String, ParamValue)> = l
                    .tuner
                    .as_ref()
                    .and_then(|t| t.best_values())
                    .map(|b| b.iter().filter(|(k, v)| l.values.get(*k) != Some(v)).map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                l.tuner = None;
                if was_tuning {
                    let list: Vec<String> = best.iter().map(|(k, v)| format!("{k}={}", v.display())).collect();
                    self.event(lane, format!("tuner stopped; restored best: {}", if list.is_empty() { "(already applied)".into() } else { list.join(", ") }));
                }
                for (name, value) in best {
                    self.set_param(lane, &name, value);
                }
                let l = &mut self.lanes[lane];
                l.tuner = config.map(|c| Tuner::new(c, specs, l.values.clone(), now));
                if l.tuner.is_none() {
                    self.shared.lock().unwrap().lanes[lane].tuner = None;
                }
            }
            Cmd::Stop => {}
        }
    }

    pub(super) fn camera(&self, ctl: &Control) -> Camera {
        match ctl.camera {
            CameraControl::Free(c) => c,
            CameraControl::Path => self.scene.camera(ctl.scene_frame),
        }
    }

    /// Opens and closes presenter windows as [`Control::windows`] asks, takes their
    /// reports and drops the ones that went.
    pub(super) fn sync_presenters(&mut self, ctl: &Control) {
        let mut gone = vec![];
        for i in 0..self.lanes.len().min(64) {
            let want = ctl.windows >> i & 1 != 0;
            if want && !self.presenters.contains_key(&i) {
                match Presenter::spawn(&self.window_title(i), ctl.paper_white) {
                    Ok(p) => {
                        self.presenters.insert(i, p);
                    }
                    Err(e) => {
                        self.event(i, format!("no window: {e:#}"));
                        gone.push(i);
                    }
                }
            } else if !want
                && self.presenters.remove(&i).is_some()
                && let Some(v) = self.shared.lock().unwrap().lanes.get_mut(i) {
                v.window = None;
            }
        }
        let titles: Vec<(usize, String)> = self.presenters.keys().map(|&i| (i, self.window_title(i))).collect();
        for (i, title) in titles {
            let p = self.presenters.get_mut(&i).unwrap();
            let r = p.poll().and_then(|r| {
                p.set_gain(ctl.paper_white)?;
                p.set_title(&title)?;
                Ok(r)
            });
            let mut sh = self.shared.lock().unwrap();
            match r {
                Ok(reports) => {
                    for r in reports {
                        match r {
                            present::Report::Gain(g) => sh.paper_white = Some(g),
                            present::Report::Status(s) => {
                                if let Some(v) = sh.lanes.get_mut(i) {
                                    v.window = Some(s);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::info!("lane {}: window: {e:#}", i + 1);
                    gone.push(i);
                }
            }
        }
        for i in gone {
            self.presenters.remove(&i);
            let mut sh = self.shared.lock().unwrap();
            sh.closed_windows.push(i);
            if let Some(v) = sh.lanes.get_mut(i) {
                v.window = None;
            }
        }
    }

    fn window_title(&self, i: usize) -> String {
        format!("multirenderer — {} {}", i + 1, self.lanes[i].run.spec.label())
    }
}
