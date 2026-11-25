//! Small Gaussian-process Bayesian optimiser used by the tuner.
//!
//! Knobs are encoded into `[0, 1]^d`. Two GPs model the outcome: log-fps and PSNR. The
//! acquisition is constrained expected improvement: for "max quality at fps" it is
//! `P(fps >= target) * EI(quality)` (or just the feasibility probability until a feasible
//! point has been seen), and symmetrically for "max fps at quality". Observations are kept
//! in a bounded window so the model follows a drifting scene.

use std::collections::VecDeque;

use mr_core::{ParamKind, ParamSpec, ParamValue};

use crate::tune::Goal;

/// Encodes a knob value into `[0, 1]`. Wide integer ranges use a log scale.
pub fn encode(spec: &ParamSpec, v: &ParamValue) -> f64 {
    match &spec.kind {
        ParamKind::Bool => v.as_bool().unwrap_or(false) as u8 as f64,
        ParamKind::Int { min, max } => {
            let x = v.as_i64().unwrap_or(*min) as f64;
            let (lo, hi) = (*min as f64, *max as f64);
            if hi - lo < 1.0 {
                return 0.0;
            }
            if wide(*min, *max) {
                ((x - lo).ln_1p() / (hi - lo).ln_1p()).clamp(0.0, 1.0)
            } else {
                ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
            }
        }
        ParamKind::Float { min, max, log } => {
            let x = v.as_f64().unwrap_or(*min);
            if *log && *min > 0.0 {
                ((x / min).ln() / (max / min).ln()).clamp(0.0, 1.0)
            } else {
                ((x - min) / (max - min).max(1e-12)).clamp(0.0, 1.0)
            }
        }
        ParamKind::Choice(opts) => {
            let i = opts.iter().position(|o| Some(o.as_str()) == v.as_str()).unwrap_or(0);
            if opts.len() <= 1 { 0.0 } else { i as f64 / (opts.len() - 1) as f64 }
        }
    }
}

pub fn decode(spec: &ParamSpec, x: f64) -> ParamValue {
    let x = x.clamp(0.0, 1.0);
    match &spec.kind {
        ParamKind::Bool => ParamValue::Bool(x >= 0.5),
        ParamKind::Int { min, max } => {
            let (lo, hi) = (*min as f64, *max as f64);
            let v = if wide(*min, *max) { lo + (x * (hi - lo).ln_1p()).exp_m1() } else { lo + x * (hi - lo) };
            ParamValue::Int((v.round() as i64).clamp(*min, *max))
        }
        ParamKind::Float { min, max, log } => {
            let v = if *log && *min > 0.0 { min * (max / min).powf(x) } else { min + x * (max - min) };
            ParamValue::Float((v.clamp(*min, *max) * 1000.0).round() / 1000.0)
        }
        ParamKind::Choice(opts) => {
            let i = if opts.len() <= 1 { 0 } else { (x * (opts.len() - 1) as f64).round() as usize };
            ParamValue::Choice(opts[i.min(opts.len().saturating_sub(1))].clone())
        }
    }
}

fn wide(min: i64, max: i64) -> bool {
    max - min > 200
}

// ------------------------------------------------------------------------------ GP

pub struct Gp {
    xs: Vec<Vec<f64>>,
    alpha: Vec<f64>,
    chol: Vec<f64>,
    ell: f64,
    mean: f64,
    std: f64,
}

fn kernel(a: &[f64], b: &[f64], ell: f64) -> f64 {
    let d2: f64 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum();
    (-d2 / (2.0 * ell * ell)).exp()
}

/// Lower-triangular Cholesky factor of a row-major `n x n` SPD matrix.
fn cholesky(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if s <= 0.0 {
                    return None;
                }
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    Some(l)
}

fn forward(l: &[f64], n: usize, b: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= l[i * n + k] * y[k];
        }
        y[i] = s / l[i * n + i];
    }
    y
}

fn backward(l: &[f64], n: usize, y: &[f64]) -> Vec<f64> {
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = y[i];
        for k in i + 1..n {
            s -= l[k * n + i] * x[k];
        }
        x[i] = s / l[i * n + i];
    }
    x
}

impl Gp {
    /// Fits a GP with an RBF kernel, choosing the length scale by marginal likelihood.
    pub fn fit(xs: &[Vec<f64>], ys: &[f64], noise: f64) -> Option<Gp> {
        let n = xs.len();
        if n == 0 {
            return None;
        }
        let mean = ys.iter().sum::<f64>() / n as f64;
        let std = (ys.iter().map(|y| (y - mean).powi(2)).sum::<f64>() / n as f64).sqrt().max(1e-6);
        let yn: Vec<f64> = ys.iter().map(|y| (y - mean) / std).collect();
        let mut best: Option<(f64, Gp)> = None;
        for ell in [0.08, 0.15, 0.25, 0.4, 0.7, 1.2] {
            let mut k = vec![0.0; n * n];
            for i in 0..n {
                for j in 0..n {
                    k[i * n + j] = kernel(&xs[i], &xs[j], ell) + if i == j { noise } else { 0.0 };
                }
            }
            let Some(l) = cholesky(&k, n) else { continue };
            let alpha = backward(&l, n, &forward(&l, n, &yn));
            let fit: f64 = yn.iter().zip(&alpha).map(|(y, a)| y * a).sum();
            let logdet: f64 = (0..n).map(|i| l[i * n + i].ln()).sum();
            let lml = -0.5 * fit - logdet;
            if best.as_ref().is_none_or(|(b, _)| lml > *b) {
                best = Some((lml, Gp { xs: xs.to_vec(), alpha, chol: l, ell, mean, std }));
            }
        }
        best.map(|b| b.1)
    }

    /// Posterior mean and standard deviation at `x`, in the original units.
    pub fn predict(&self, x: &[f64]) -> (f64, f64) {
        let n = self.xs.len();
        let ks: Vec<f64> = self.xs.iter().map(|xi| kernel(xi, x, self.ell)).collect();
        let mu: f64 = ks.iter().zip(&self.alpha).map(|(k, a)| k * a).sum();
        let v = forward(&self.chol, n, &ks);
        let var = (1.0 - v.iter().map(|x| x * x).sum::<f64>()).max(1e-9);
        (self.mean + mu * self.std, var.sqrt() * self.std)
    }
}

fn erf(x: f64) -> f64 {
    // Abramowitz & Stegun 7.1.26 (|error| < 1.5e-7).
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    if x >= 0.0 { y } else { -y }
}

pub fn norm_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

fn expected_improvement(mu: f64, sigma: f64, best: f64) -> f64 {
    if sigma < 1e-12 {
        return (mu - best).max(0.0);
    }
    let z = (mu - best) / sigma;
    (mu - best) * norm_cdf(z) + sigma * norm_pdf(z)
}

// ------------------------------------------------------------------------- optimiser

#[derive(Clone, Debug)]
pub struct Observation {
    pub x: Vec<f64>,
    pub log_fps: f64,
    pub psnr: Option<f64>,
}

pub struct Optimizer {
    pub goal: Goal,
    pub obs: VecDeque<Observation>,
    pub max_obs: usize,
}

impl Optimizer {
    pub fn new(goal: Goal) -> Self {
        Self { goal, obs: VecDeque::new(), max_obs: 40 }
    }

    pub fn observe(&mut self, o: Observation) {
        self.obs.push_back(o);
        while self.obs.len() > self.max_obs {
            self.obs.pop_front();
        }
    }

    fn feasible(&self, o: &Observation) -> bool {
        match self.goal {
            Goal::MaxQualityAtFps { fps } => o.log_fps >= (fps * 0.98).ln(),
            Goal::MaxFpsAtQuality { psnr } => o.psnr.unwrap_or(0.0) >= psnr,
            Goal::MaxFps => true,
        }
    }

    /// Picks the next point to evaluate among `candidates`. Returns the point and its
    /// predicted (fps, psnr).
    pub fn suggest(&self, candidates: &[Vec<f64>]) -> Option<(Vec<f64>, f64, Option<f64>)> {
        let xs: Vec<Vec<f64>> = self.obs.iter().map(|o| o.x.clone()).collect();
        let lf: Vec<f64> = self.obs.iter().map(|o| o.log_fps).collect();
        let gp_f = Gp::fit(&xs, &lf, 0.02)?;
        let with_q: Vec<&Observation> = self.obs.iter().filter(|o| o.psnr.is_some()).collect();
        let gp_q = if self.goal.needs_quality() && !with_q.is_empty() {
            Gp::fit(&with_q.iter().map(|o| o.x.clone()).collect::<Vec<_>>(), &with_q.iter().map(|o| o.psnr.unwrap()).collect::<Vec<_>>(), 0.02)
        } else {
            None
        };
        let feasible: Vec<&Observation> = self.obs.iter().filter(|o| self.feasible(o)).collect();
        let mut best: Option<(f64, &Vec<f64>, f64, Option<f64>)> = None;
        for c in candidates {
            let (mf, sf) = gp_f.predict(c);
            let q = gp_q.as_ref().map(|g| g.predict(c));
            let acq = match (self.goal, q) {
                (Goal::MaxQualityAtFps { fps }, Some((mq, sq))) => {
                    let pf = norm_cdf((mf - (fps * 0.98).ln()) / sf.max(1e-6));
                    match feasible.iter().filter_map(|o| o.psnr).reduce(f64::max) {
                        Some(bq) => pf * expected_improvement(mq, sq, bq),
                        None => pf,
                    }
                }
                (Goal::MaxFpsAtQuality { psnr }, Some((mq, sq))) => {
                    let pq = norm_cdf((mq - psnr) / sq.max(1e-6));
                    match feasible.iter().map(|o| o.log_fps).reduce(f64::max) {
                        Some(bf) => pq * expected_improvement(mf, sf, bf),
                        None => pq,
                    }
                }
                (Goal::MaxFps, _) | (_, None) => {
                    let bf = self.obs.iter().map(|o| o.log_fps).fold(f64::MIN, f64::max);
                    expected_improvement(mf, sf, bf)
                }
            };
            if best.as_ref().is_none_or(|b| acq > b.0) {
                best = Some((acq, c, mf.exp(), q.map(|q| q.0)));
            }
        }
        best.map(|(_, x, f, q)| (x.clone(), f, q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gp_interpolates_smooth_function() {
        let xs: Vec<Vec<f64>> = (0..12).map(|i| vec![i as f64 / 11.0]).collect();
        let ys: Vec<f64> = xs.iter().map(|x| (x[0] * 6.0).sin() * 3.0 + 10.0).collect();
        let gp = Gp::fit(&xs, &ys, 1e-4).unwrap();
        for i in 0..20 {
            let x = i as f64 / 19.0;
            let (mu, sd) = gp.predict(&[x]);
            let truth = (x * 6.0).sin() * 3.0 + 10.0;
            assert!((mu - truth).abs() < 0.3, "x {x}: {mu} vs {truth} (sd {sd})");
        }
        // Uncertainty grows away from data.
        assert!(gp.predict(&[3.0]).1 > gp.predict(&[0.5]).1);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let specs = [
            ParamSpec::int("steps", "", 32, 16384, 2048, true),
            ParamSpec::int("small", "", 1, 8, 3, true),
            ParamSpec::float("scale", "", 0.2, 1.0, false, 0.5, true),
            ParamSpec::choice("shadows", "", &["none", "sun", "all"], "sun", true),
            ParamSpec::bool("b", "", true, true),
        ];
        for s in &specs {
            let x = encode(s, &s.default);
            let back = decode(s, x);
            match (&back, &s.default) {
                (ParamValue::Int(a), ParamValue::Int(b)) => assert!((a - b).abs() <= 1, "{}: {a} vs {b}", s.name),
                _ => assert_eq!(back, s.default, "{}", s.name),
            }
        }
    }
}
