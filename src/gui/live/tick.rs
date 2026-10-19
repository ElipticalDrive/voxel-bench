//! One session step: spawns, hot swaps, resizes, frame pumping and failures.

use std::time::Duration;

use mr_ipc::WorkerMsg;

use crate::worker::WorkerEvent;

use super::*;

impl Session {
    pub(super) fn tick(&mut self, ctl: &Control) -> anyhow::Result<bool> {
        let mut busy = false;
        let now = self.now();
        busy |= self.poll_spawns();
        self.sync_presenters(ctl);

        // Scripted scene time.
        if ctl.events && self.applied_frame != Some(ctl.scene_frame) {
            let from = match self.applied_frame {
                Some(a) if a < ctl.scene_frame => (a + 1).max(ctl.scene_frame.saturating_sub(120)),
                _ => ctl.scene_frame,
            };
            let cam = self.camera(ctl);
            let (mut edits, mut lights, mut env) = (vec![], None, None);
            for f in from..=ctl.scene_frame {
                let ev = self.scene.events(f, &cam)?;
                edits.extend(ev.edits);
                if !ev.lights.is_empty() {
                    lights = Some(ev.lights);
                }
                env = ev.environment.or(env);
            }
            self.applied_frame = Some(ctl.scene_frame);
            self.apply_edits(edits);
            if !self.mesh_data.is_empty() {
                let inst = self.scene.mesh_instances(ctl.scene_frame);
                if inst != self.instances {
                    self.instances = inst.clone();
                    for l in self.lanes.iter_mut().chain(self.reference.as_mut()) {
                        l.run.pending_meshes.push(MeshUpdate::Instances(inst.clone()));
                        if let Some(s) = &mut l.standby {
                            s.pending_meshes.push(MeshUpdate::Instances(inst.clone()));
                        }
                    }
                }
            }
            if let Some(ls) = lights {
                for l in &ls {
                    self.lights.insert(l.id, *l);
                }
                self.fan_out(WorldUpdate::LightUpsert(ls));
            }
            if let Some(e) = env {
                self.env = e;
                self.fan_out(WorldUpdate::SetEnvironment(e));
            }
            busy = true;
        }

        let cam = self.camera(ctl);
        {
            let mut sh = self.shared.lock().unwrap();
            sh.camera = Some(cam);
            sh.lights = self.lights.len();
            sh.cached_chunks = self.world.cached_chunks();
        }
        // The server's region: as large as the largest any lane needs streamed at each level
        // (none for lanes generating the world).
        let runners = || self.lanes.iter().flat_map(|l| std::iter::once(&l.run).chain(l.standby.as_ref()));
        let mut stream = vec![];
        if runners().any(|r| !r.region.generates) {
            let radii = crate::stream::union(runners().map(|r| r.region.streamed()), self.plan.run.view_radius);
            self.server.set_radii(&radii, &mut stream);
            self.server.update(&mut self.world, cam.position, &mut stream);
        }
        let (world, server) = (&mut self.world, &self.server);
        for r in self.lanes.iter_mut().flat_map(|l| std::iter::once(&mut l.run).chain(l.standby.as_mut())) {
            if r.worker.is_none() {
                continue;
            }
            if r.region.generates {
                // Joining, the edits so far; then the edits as they come.
                if !r.joined {
                    let edits = world.edits();
                    if !edits.is_empty() {
                        r.take(&[WorldUpdate::SetBlocks(edits)]);
                    }
                    r.joined = true;
                }
            } else if r.joined {
                r.take(&stream);
            } else {
                r.take(&server.snapshot(world));
                r.joined = true;
            }
        }

        let mut failures: Vec<(usize, anyhow::Error)> = vec![];
        busy |= self.progress_swaps(cam, ctl.mode, &mut failures);
        self.apply_resizes(&mut failures);

        // Quality probes.
        if ctl.probe_interval > 0.0 && self.probe.is_none() && self.last_probe.elapsed().as_secs_f64() >= ctl.probe_interval {
            self.request_probe(false);
        }
        if self.probe.as_ref().is_some_and(|p| !p.started) {
            self.start_probe(cam, ctl.mode, &mut failures);
        }

        // Live frames.
        let instant = Instant::now();
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            if lane.run.worker.is_none() || lane.want_res.is_some() {
                continue;
            }
            let in_flight = lane.values.get("host.in_flight").and_then(|v| v.as_i64()).unwrap_or(2).max(1) as usize;
            if lane.run.in_flight.len() >= in_flight {
                continue;
            }
            if ctl.fps_cap > 0.0 && lane.last_sent.is_some_and(|t| instant.duration_since(t).as_secs_f64() < 1.0 / ctl.fps_cap) {
                continue;
            }
            let display = lane.last_display.is_none_or(|t| instant.duration_since(t).as_secs_f64() >= 1.0 / ctl.display_hz.max(1.0));
            match lane.run.submit(cam, ctl.mode, display, Purpose::Live) {
                Ok(true) => {
                    if display {
                        lane.last_display = Some(instant);
                    }
                    lane.last_sent = Some(instant);
                    busy = true;
                }
                Ok(false) => {}
                Err(e) => failures.push((i, e)),
            }
        }

        for i in (0..self.lanes.len()).chain(self.reference.is_some().then_some(REFERENCE)) {
            match self.pump(i) {
                Ok(b) => busy |= b,
                Err(e) => failures.push((i, e)),
            }
        }
        if self.probe.as_ref().is_some_and(|p| p.started && p.waiting == 0) {
            self.finish_probe();
        }
        for (i, e) in failures {
            self.fail(i, e);
        }
        self.run_tuners(now);
        Ok(busy)
    }

    fn poll_spawns(&mut self) -> bool {
        let mut busy = false;
        let n = self.lanes.len();
        for i in (0..n).chain(self.reference.is_some().then_some(REFERENCE)) {
            let result = match self.lane_mut(i).and_then(|l| l.run.spawning.as_ref()).map(|rx| rx.try_recv()) {
                Some(Ok(r)) => r,
                _ => continue,
            };
            busy = true;
            let run_spec = self.plan.run.clone();
            let lane = self.lane_mut(i).unwrap();
            lane.run.spawning = None;
            match result {
                Ok(w) => {
                    let info = w.info.clone();
                    let init_ms = w.init_ms;
                    // Renderer knob values from its options (else defaults).
                    for p in &info.params {
                        let v = lane.run.spec.options.get(&p.name).and_then(|t| serde_json::to_value(t).ok()).and_then(|j| p.from_json(&j)).unwrap_or(p.default.clone());
                        lane.values.insert(p.name.clone(), v);
                    }
                    // Now that its LOD support is known, stream the levels it can use.
                    let lods = lane.run.spec.max_lod(&run_spec, info.max_lod);
                    lane.run.region.max_lod = lods;
                    if let Some(own) = &mut lane.run.own {
                        // The reference's client takes its own server's stream as it is.
                        let radii = lane.run.region.reach();
                        own.set_radii(&radii, &mut vec![]);
                        if let Err(e) = w.send(HostMsg::Follow { radii }) {
                            tracing::error!("reference: {e:#}");
                        }
                    }
                    lane.values.insert("host.max_lod".into(), ParamValue::Int(lods as i64));
                    lane.info = Some(info);
                    lane.run.worker = Some(w);
                    if i != REFERENCE {
                        {
                            let mut sh = self.shared.lock().unwrap();
                            if let Some(v) = sh.lanes.get_mut(i) {
                                v.status = LaneStatus::Running;
                                v.init_ms = init_ms;
                            }
                        }
                        self.publish_params(i);
                        self.event(i, format!("started ({init_ms:.0} ms init)"));
                    } else {
                        self.log("reference renderer started");
                    }
                }
                Err(e) => {
                    if i == REFERENCE {
                        self.log(format!("reference failed to start: {e:#}"));
                        self.reference = None;
                        self.probe = None;
                        self.shared.lock().unwrap().comparing = false;
                    } else {
                        let msg = format!("{e:#}");
                        self.shared.lock().unwrap().lanes[i].status = LaneStatus::Failed(msg.clone());
                        self.log(format!("lane {} failed to start: {msg}", i + 1));
                    }
                }
            }
        }
        let run_spec = self.plan.run.clone();
        for i in 0..n {
            let lane = &mut self.lanes[i];
            let Some(sb) = &mut lane.standby else { continue };
            let Some(result) = sb.spawning.as_ref().and_then(|rx| rx.try_recv().ok()) else { continue };
            sb.spawning = None;
            busy = true;
            match result {
                Ok(w) => {
                    let lods = sb.spec.max_lod(&run_spec, w.info.max_lod);
                    sb.region.max_lod = lods;
                    sb.worker = Some(w);
                    self.shared.lock().unwrap().lanes[i].swap = Some("loading world".into());
                }
                Err(e) => {
                    lane.standby = None;
                    self.shared.lock().unwrap().lanes[i].swap = None;
                    self.event(i, format!("hot swap failed: {e:#}"));
                }
            }
        }
        busy
    }

    /// Streams to standbys, warms them with unseen frames, and swaps them in when ready.
    fn progress_swaps(&mut self, cam: Camera, mode: PassMode, failures: &mut Vec<(usize, anyhow::Error)>) -> bool {
        let mut busy = false;
        for i in 0..self.lanes.len() {
            let lane = &mut self.lanes[i];
            let Some(sb) = &mut lane.standby else { continue };
            if sb.worker.is_none() {
                continue;
            }
            // Warm-up frames once everything in range has streamed.
            let pending = if sb.joined { self.server.pending() } else { 1 };
            if pending == 0 && sb.in_flight.is_empty() && lane.standby_warm < 3 {
                match sb.submit(cam, mode, false, Purpose::Warm) {
                    Ok(_) => busy = true,
                    Err(e) => {
                        failures.push((i, e.context("hot swap")));
                        continue;
                    }
                }
            }
            let mut done = 0;
            if let Some(w) = &sb.worker {
                while let Ok(ev) = w.rx.try_recv() {
                    match ev {
                        WorkerEvent::Msg(WorkerMsg::FrameDone { .. }) => {
                            sb.in_flight.pop_front();
                            done += 1;
                        }
                        WorkerEvent::Msg(WorkerMsg::Error(e)) | WorkerEvent::Msg(WorkerMsg::ParamError { error: e, .. }) => {
                            failures.push((i, anyhow::anyhow!("hot swap: {e}")));
                        }
                        WorkerEvent::Closed => failures.push((i, anyhow::anyhow!("hot swap: replacement exited"))),
                        _ => {}
                    }
                }
            }
            lane.standby_warm += done;
            let progress = if pending > 0 { format!("loading world ({pending} chunks left)") } else { format!("warming up ({}/3)", lane.standby_warm) };
            if lane.standby_warm < 3 {
                self.shared.lock().unwrap().lanes[i].swap = Some(progress);
                continue;
            }
            // Swap: the replacement takes over; the old worker shuts down in the background.
            let sb = lane.standby.take().unwrap();
            if let Some(old) = std::mem::replace(&mut lane.run, sb).worker {
                std::thread::spawn(move || old.shutdown());
            }
            let info = lane.run.worker.as_ref().map(|w| w.info.clone());
            if let Some(info) = info {
                for p in &info.params {
                    let v = lane.run.spec.options.get(&p.name).and_then(|t| serde_json::to_value(t).ok()).and_then(|j| p.from_json(&j)).unwrap_or(p.default.clone());
                    lane.values.insert(p.name.clone(), v);
                }
                lane.info = Some(info);
            }
            lane.last_done = None;
            if let Some(t) = &mut lane.tuner {
                t.hold(self.start.elapsed().as_secs_f64());
            }
            self.shared.lock().unwrap().lanes[i].swap = None;
            self.publish_params(i);
            self.event(i, "hot swap complete".into());
            busy = true;
        }
        busy
    }

    fn apply_resizes(&mut self, failures: &mut Vec<(usize, anyhow::Error)>) {
        for i in 0..self.lanes.len() {
            let lane = &mut self.lanes[i];
            let Some(r) = lane.want_res else { continue };
            if !lane.run.in_flight.is_empty() || lane.run.worker.is_none() {
                continue;
            }
            lane.want_res = None;
            if let Err(e) = lane.run.worker.as_mut().unwrap().resize(r[0], r[1], SLOTS) {
                failures.push((i, e.context("resize")));
                continue;
            }
            lane.run.res = r;
            lane.last_done = None;
            if let Some(t) = &mut lane.tuner {
                t.hold(self.start.elapsed().as_secs_f64());
            }
            self.publish_params(i);
            self.event(i, format!("render resolution {}×{}", r[0], r[1]));
        }
    }

    fn pump(&mut self, i: usize) -> anyhow::Result<bool> {
        let start = self.start;
        let shared = self.shared.clone();
        let server_pending = self.server.pending();
        let presenting = self.presenters.contains_key(&i);
        let Some(lane) = self.lane_mut(i) else { return Ok(false) };
        let Some(w) = &mut lane.run.worker else { return Ok(false) };
        let mut busy = false;
        let mut repaint = false;
        let mut probe_px: Vec<(u64, [u32; 2], Arc<Vec<u8>>)> = vec![];
        let mut shown = None;
        if lane.last_mem.elapsed() > Duration::from_secs(1) && i != REFERENCE {
            lane.last_mem = Instant::now();
            w.send(HostMsg::QueryMemory)?;
            let rss = w.rss().map(|b| b as f64 / 1048576.0);
            let mut sh = shared.lock().unwrap();
            if let Some(v) = sh.lanes.get_mut(i) {
                v.rss_mb = rss;
                v.chunks_sent = lane.run.chunks_sent;
                v.pending_chunks = server_pending;
            }
        }
        while let Ok(ev) = w.rx.try_recv() {
            busy = true;
            let msg = match ev {
                WorkerEvent::Msg(WorkerMsg::Error(e)) => anyhow::bail!("worker error: {e}"),
                WorkerEvent::Closed => anyhow::bail!("worker exited"),
                WorkerEvent::Msg(m) => m,
            };
            match msg {
                WorkerMsg::Memory(m) => {
                    if let Some(v) = shared.lock().unwrap().lanes.get_mut(i) {
                        v.gpu_mb = (m.gpu_bytes > 0).then(|| m.gpu_bytes as f64 / 1048576.0);
                    }
                }
                WorkerMsg::ParamError { name, error } => {
                    let what = format!("refused {name}: {error}");
                    let mut sh = shared.lock().unwrap();
                    if let Some(v) = sh.lanes.get_mut(i) {
                        v.events.push((start.elapsed().as_secs_f64(), what.clone()));
                    }
                    sh.log(format!("lane {}: {what}", i.wrapping_add(1)));
                }
                WorkerMsg::FrameDone { frame_id, stats, slot, update_ms, arrivals } => {
                    let Some(f) = lane.run.in_flight.pop_front() else { anyhow::bail!("unexpected frame") };
                    if f.id != frame_id {
                        anyhow::bail!("frames out of order");
                    }
                    let now = Instant::now();
                    let res = lane.run.res;
                    let raw = slot.map(|s| w.take_frame(s, (res[0] * res[1]) as usize));
                    if let (Some(px), Purpose::Live, true) = (&raw, &f.purpose, presenting) {
                        shown = Some((res, w.format, px.clone()));
                    }
                    let pixels = raw.map(|px| Arc::new(crate::worker::rgba8(w.format, px).0));
                    match f.purpose {
                        Purpose::Live => {
                            let t = now.duration_since(start).as_secs_f64();
                            let interval_ms = lane.last_done.map(|d| now.duration_since(d).as_secs_f32() * 1e3).unwrap_or(0.0);
                            lane.last_done = Some(now);
                            if let Some(tu) = &mut lane.tuner {
                                tu.on_frame(t, interval_ms as f64);
                            }
                            let sample = Sample { t, interval_ms, latency_ms: now.duration_since(f.sent).as_secs_f32() * 1e3, cpu_ms: stats.cpu_ms, gpu_ms: stats.gpu_ms, update_ms };
                            let mut sh = shared.lock().unwrap();
                            if let Some(v) = sh.lanes.get_mut(i) {
                                v.samples.push_back(sample);
                                if v.samples.len() > MAX_SAMPLES {
                                    v.samples.pop_front();
                                }
                                v.counters = stats.counters;
                                v.smooth_stages(stats.stages);
                                v.legend = stats.legend;
                                v.arrivals = arrivals;
                                v.frames += 1;
                                if let Some(px) = pixels {
                                    lane.seq += 1;
                                    v.frame = Some(LiveFrame { width: res[0], height: res[1], pixels: px, seq: lane.seq });
                                    repaint = true;
                                }
                            }
                        }
                        Purpose::Probe(id) => {
                            if let Some(px) = pixels {
                                probe_px.push((id, res, px));
                            }
                        }
                        Purpose::Warm => {}
                    }
                }
                m => anyhow::bail!("unexpected message {m:?}"),
            }
        }
        if let (Some((res, format, px)), Some(p)) = (shown, self.presenters.get_mut(&i)) {
            if let Err(e) = p.offer(res[0], res[1], format, &px) {
                tracing::info!("lane {}: window: {e:#}", i + 1);
                self.presenters.remove(&i);
                self.shared.lock().unwrap().closed_windows.push(i);
            }
        }
        for (id, res, px) in probe_px {
            if let Some(p) = &mut self.probe {
                if p.id == id && p.waiting > 0 {
                    p.waiting -= 1;
                    if i == REFERENCE {
                        p.reference = Some(px);
                    } else {
                        p.lanes.insert(i, (res, px));
                    }
                }
            }
        }
        if repaint {
            (self.repaint)();
        }
        Ok(busy)
    }

    fn fail(&mut self, i: usize, e: anyhow::Error) {
        let msg = format!("{e:#}");
        if i == REFERENCE {
            self.reference = None;
            self.probe = None;
            let mut sh = self.shared.lock().unwrap();
            sh.comparing = false;
            sh.log(format!("reference failed: {msg}"));
            return;
        }
        let Some(lane) = self.lanes.get_mut(i) else { return };
        if msg.starts_with("hot swap") {
            if let Some(w) = lane.standby.take().and_then(|s| s.worker) {
                std::thread::spawn(move || w.shutdown());
            }
            self.shared.lock().unwrap().lanes[i].swap = None;
            self.event(i, msg);
            return;
        }
        lane.run.worker = None;
        lane.run.in_flight.clear();
        let mut sh = self.shared.lock().unwrap();
        sh.log(format!("lane {} failed: {msg}", i + 1));
        let t = self.now();
        if let Some(v) = sh.lanes.get_mut(i) {
            v.events.push((t, format!("failed: {}", msg.lines().next().unwrap_or(""))));
            v.status = LaneStatus::Failed(msg);
        }
        drop(sh);
        if let Some(p) = &mut self.probe {
            if p.started && !p.lanes.contains_key(&i) {
                p.waiting = p.waiting.saturating_sub(1);
            }
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.presenters.clear();
        let mut handles = vec![];
        for l in self.lanes.drain(..).chain(self.reference.take()) {
            for w in [l.run.worker, l.standby.and_then(|s| s.worker)].into_iter().flatten() {
                handles.push(std::thread::spawn(move || w.shutdown()));
            }
        }
        for h in handles {
            let _ = h.join();
        }
    }
}
