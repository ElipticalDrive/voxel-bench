//! Quality probes against the reference renderer, and the auto-tuners they feed.

use crate::tune::Action;

use super::*;

impl Session {
    pub(super) fn request_probe(&mut self, keep_images: bool) {
        if keep_images {
            self.shared.lock().unwrap().comparing = true;
        }
        if let Some(p) = &mut self.probe {
            p.keep_images |= keep_images;
            return;
        }
        self.probe_seq += 1;
        self.probe = Some(Probe {
            id: self.probe_seq,
            camera: Camera::default(),
            mode: PassMode::UNLIT,
            keep_images,
            started: false,
            reference: None,
            lanes: HashMap::new(),
            waiting: 0,
        });
    }

    pub(super) fn start_probe(&mut self, cam: Camera, mode: PassMode, failures: &mut Vec<(usize, anyhow::Error)>) {
        if self.reference.is_none() {
            let name = self.plan.output.compare.as_ref().map(|c| c.reference.clone()).unwrap_or_else(|| "cpu-reference".into());
            let options = self.plan.output.compare.as_ref().map(|c| c.reference_options.clone()).unwrap_or_default();
            let mut spec = RendererSpec::new(name.clone());
            (spec.label, spec.options) = (Some(format!("{name} (ref)")), options);
            self.shared.lock().unwrap().reference = spec.label().to_string();
            self.log(format!("starting reference {}", spec.label()));
            let mut r = self.new_lane(spec);
            r.run.own = Some(Streamer::new(r.run.region.reach(), None));
            self.reference = Some(r);
            return;
        }
        let Some(reference) = &mut self.reference else { return };
        if reference.run.worker.is_none() {
            return; // still spawning
        }
        // The reference must see the whole streamed region.
        loop {
            let own = reference.run.own.as_mut().unwrap();
            let mut stream = vec![];
            own.update(&mut self.world, cam.position, &mut stream);
            let done = own.pending() == 0;
            reference.run.take(&stream);
            if done {
                break;
            }
        }
        let probe = self.probe.as_mut().unwrap();
        probe.started = true;
        probe.camera = cam;
        probe.mode = mode;
        let id = probe.id;
        match reference.run.submit(cam, mode, true, Purpose::Probe(id)) {
            Ok(true) => probe.waiting += 1,
            Ok(false) => {}
            Err(e) => failures.push((REFERENCE, e)),
        }
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            if lane.run.worker.is_none() || lane.want_res.is_some() {
                continue;
            }
            match lane.run.submit(cam, mode, true, Purpose::Probe(id)) {
                Ok(true) => probe.waiting += 1,
                Ok(false) => {}
                Err(e) => failures.push((i, e)),
            }
        }
        self.last_probe = Instant::now();
    }

    pub(super) fn finish_probe(&mut self) {
        let Some(p) = self.probe.take() else { return };
        let t = self.now();
        let mut sh = self.shared.lock().unwrap();
        sh.comparing = false;
        let Some(reference) = p.reference else {
            sh.log("probe failed: no reference image");
            return;
        };
        let ref_res = self.reference.as_ref().map(|r| r.run.res).unwrap_or(self.plan.run.resolution);
        let mut kept = vec![];
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            let Some((res, px)) = p.lanes.get(&i) else { continue };
            // Bilinear, centre-aligned: what a display path does with a lower-resolution frame.
            let px = if *res != ref_res { Arc::new(compare::resample(px, *res, ref_res, true)) } else { px.clone() };
            let q = compare::compare(&reference, &px, ref_res[0]);
            if let Some(v) = sh.lanes.get_mut(i) {
                v.quality.push_back(QualitySample { t, psnr: q.psnr.min(99.0), ssim_db: q.ssim_db, ms_ssim_db: q.ms_ssim_db, bad_pct: q.bad_pct });
                if v.quality.len() > 2000 {
                    v.quality.pop_front();
                }
            }
            if let Some(tu) = &mut lane.tuner {
                tu.on_probe(t, q.psnr.min(99.0), q.ms_ssim_db);
            }
            if p.keep_images {
                kept.push(CompareLane { label: lane.run.spec.label().to_string(), quality: q, diff: Arc::new(compare::diff_image(&reference, &px)), image: px });
            }
        }
        if p.keep_images {
            self.compare_seq += 1;
            let summary: Vec<String> = kept.iter().map(|l| format!("{} {:.1} dB / {:.2}% bad", l.label, l.quality.psnr.min(99.0), l.quality.bad_pct)).collect();
            sh.log(format!("compare [{}]: {}", p.mode.name(), summary.join(", ")));
            let reference_label = sh.reference.clone();
            sh.compare = Some(CompareResult {
                seq: self.compare_seq,
                mode: p.mode,
                camera: p.camera,
                width: ref_res[0],
                height: ref_res[1],
                reference_label,
                reference,
                lanes: kept,
            });
        }
        drop(sh);
        (self.repaint)();
    }

    pub(super) fn run_tuners(&mut self, now: f64) {
        let mut probe = false;
        for i in 0..self.lanes.len() {
            let busy = self.lanes[i].standby.is_some() || self.lanes[i].want_res.is_some();
            let Some(t) = &mut self.lanes[i].tuner else { continue };
            if busy {
                t.hold(now);
                continue;
            }
            match t.tick(now) {
                Action::Idle => {}
                Action::NeedProbe => probe = true,
                Action::Set(changes) => {
                    for (name, value) in changes {
                        self.set_param(i, &name, value);
                    }
                }
            }
            let t = self.lanes[i].tuner.as_ref().unwrap();
            let view = TunerView {
                config: t.cfg.clone(),
                status: t.status(),
                log: t.log.iter().cloned().collect(),
                history: t.history.iter().rev().take(400).rev().cloned().collect(),
                trials: t.trials,
                accepted: t.accepted,
                best: t.best_summary(),
                best_diff: t
                    .best_values()
                    .map(|b| {
                        b.iter()
                            .filter_map(|(k, v)| {
                                let start = t.start_values.get(k)?;
                                (start != v).then(|| (k.clone(), start.display(), v.display()))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                holding: t.holding(),
            };
            if let Some(v) = self.shared.lock().unwrap().lanes.get_mut(i) {
                v.tuner = Some(view);
            }
        }
        if probe {
            self.request_probe(false);
        }
    }
}
