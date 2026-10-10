//! Image comparison metrics on RGBA8 sRGB frames.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Quality {
    /// Peak signal-to-noise ratio over RGB in dB (`inf` for identical images).
    pub psnr: f64,
    /// Mean absolute error over RGB, in `[0, 1]`.
    pub mae: f64,
    /// Percentage of pixels whose largest channel difference exceeds 16/255.
    pub bad_pct: f64,
    /// Mean structural similarity of luminance (8x8 windows, stride 4), in `[-1, 1]`.
    /// Tolerant to small shifts and resampling, unlike PSNR.
    pub ssim: f64,
    /// SSIM as decibels, `-10 log10(1 - ssim)` (0.99 → 20 dB, 0.999 → 30 dB), capped at 99.
    pub ssim_db: f64,
    /// Multi-scale SSIM (5 scales, standard weights): degrades gracefully with resolution
    /// changes and resampling instead of falling off a cliff like PSNR. In `[0, 1]`.
    pub ms_ssim: f64,
    /// MS-SSIM as decibels (same mapping as `ssim_db`).
    pub ms_ssim_db: f64,
}

/// Compares two RGBA8 images of `width` pixels per row.
pub fn compare(a: &[u8], b: &[u8], width: u32) -> Quality {
    assert_eq!(a.len(), b.len());
    let w = width as usize;
    let la = luma(a);
    let lb = luma(b);
    let h = la.len() / w.max(1);
    let ssim = ssim_luma(&la, &lb, w, h);
    let ms_ssim = ms_ssim(la, lb, w, h);
    let (mut se, mut ae, mut bad) = (0u64, 0u64, 0u64);
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut worst = 0;
        for c in 0..3 {
            let d = (pa[c] as i32 - pb[c] as i32).unsigned_abs();
            se += (d * d) as u64;
            ae += d as u64;
            worst = worst.max(d);
        }
        bad += (worst > 16) as u64;
    }
    let pixels = (a.len() / 4) as f64;
    let n = pixels * 3.0;
    let mse = se as f64 / n;
    Quality {
        psnr: if mse == 0.0 { f64::INFINITY } else { 10.0 * (255.0 * 255.0 / mse).log10() },
        mae: ae as f64 / n / 255.0,
        bad_pct: bad as f64 / pixels * 100.0,
        ssim,
        ssim_db: to_db(ssim),
        ms_ssim,
        ms_ssim_db: to_db(ms_ssim),
    }
}

fn to_db(s: f64) -> f64 {
    (-10.0 * (1.0 - s).max(1e-10).log10()).min(99.0)
}

fn luma(p: &[u8]) -> Vec<f32> {
    p.chunks_exact(4).map(|c| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32).collect()
}

fn half(l: &[f32], w: usize, h: usize) -> (Vec<f32>, usize, usize) {
    let (hw, hh) = (w / 2, h / 2);
    let mut out = vec![0.0; hw * hh];
    for y in 0..hh {
        for x in 0..hw {
            out[y * hw + x] = 0.25 * (l[2 * y * w + 2 * x] + l[2 * y * w + 2 * x + 1] + l[(2 * y + 1) * w + 2 * x] + l[(2 * y + 1) * w + 2 * x + 1]);
        }
    }
    (out, hw, hh)
}

/// Weighted geometric mean of SSIM over a 2x box pyramid (Wang et al. 2003 weights).
fn ms_ssim(mut a: Vec<f32>, mut b: Vec<f32>, mut w: usize, mut h: usize) -> f64 {
    const WEIGHTS: [f64; 5] = [0.0448, 0.2856, 0.3001, 0.2363, 0.1333];
    let (mut log_sum, mut weight_sum) = (0.0, 0.0);
    for (i, wt) in WEIGHTS.iter().enumerate() {
        if w < 8 || h < 8 {
            break;
        }
        log_sum += wt * ssim_luma(&a, &b, w, h).max(1e-6).ln();
        weight_sum += wt;
        if i + 1 < WEIGHTS.len() {
            (a, _, _) = half(&a, w, h);
            let (nb, nw, nh) = half(&b, w, h);
            b = nb;
            w = nw;
            h = nh;
        }
    }
    if weight_sum == 0.0 { 1.0 } else { (log_sum / weight_sum).exp() }
}

fn ssim_luma(la: &[f32], lb: &[f32], w: usize, h: usize) -> f64 {
    if w < 8 || h < 8 {
        return 1.0;
    }
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (mut sum, mut n) = (0.0f64, 0usize);
    for y0 in (0..=h - 8).step_by(4) {
        for x0 in (0..=w - 8).step_by(4) {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
            for y in y0..y0 + 8 {
                for x in x0..x0 + 8 {
                    let (va, vb) = (la[y * w + x] as f64, lb[y * w + x] as f64);
                    sa += va;
                    sb += vb;
                    saa += va * va;
                    sbb += vb * vb;
                    sab += va * vb;
                }
            }
            let (ma, mb) = (sa / 64.0, sb / 64.0);
            let (va, vb, cov) = (saa / 64.0 - ma * ma, sbb / 64.0 - mb * mb, sab / 64.0 - ma * mb);
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
        }
    }
    sum / n as f64
}

/// Visualises differences: grey copy of `a`, with differing pixels in red (brighter is
/// a larger difference).
pub fn diff_image(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; a.len()];
    for ((pa, pb), o) in a.chunks_exact(4).zip(b.chunks_exact(4)).zip(out.chunks_exact_mut(4)) {
        let d = (0..3).map(|c| (pa[c] as i32 - pb[c] as i32).unsigned_abs()).max().unwrap();
        if d > 2 {
            let v = (64 + d * 4).min(255) as u8;
            o.copy_from_slice(&[v, 0, 0, 255]);
        } else {
            let g = ((pa[0] as u32 + pa[1] as u32 + pa[2] as u32) / 9) as u8;
            o.copy_from_slice(&[g, g, g, 255]);
        }
    }
    out
}

/// Resamples an RGBA8 image to `to`, mapping pixel centres onto pixel centres.
/// `bilinear = false` is nearest-neighbour.
pub fn resample(px: &[u8], [w, h]: [u32; 2], [tw, th]: [u32; 2], bilinear: bool) -> Vec<u8> {
    let mut out = vec![0u8; (tw * th * 4) as usize];
    let sx = w as f32 / tw as f32;
    let sy = h as f32 / th as f32;
    let at = |x: u32, y: u32, c: usize| px[((y * w + x) * 4) as usize + c] as f32;
    for y in 0..th {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (h - 1) as f32);
        for x in 0..tw {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (w - 1) as f32);
            let d = ((y * tw + x) * 4) as usize;
            if !bilinear {
                let (ix, iy) = (fx.round() as u32, fy.round() as u32);
                for c in 0..4 {
                    out[d + c] = at(ix, iy, c) as u8;
                }
                continue;
            }
            let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            for c in 0..4 {
                let top = at(x0, y0, c) * (1.0 - tx) + at(x1, y0, c) * tx;
                let bot = at(x0, y1, c) * (1.0 - tx) + at(x1, y1, c) * tx;
                out[d + c] = (top * (1.0 - ty) + bot * ty + 0.5) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_identity_and_constant() {
        let px: Vec<u8> = (0..16 * 9 * 4).map(|i| (i * 7 % 251) as u8).collect();
        assert_eq!(resample(&px, [16, 9], [16, 9], true), px);
        assert_eq!(resample(&px, [16, 9], [16, 9], false), px);
        let flat = vec![100u8; 8 * 8 * 4];
        assert!(resample(&flat, [8, 8], [13, 11], true).iter().all(|&v| v == 100));
    }

    #[test]
    fn ssim_identical_and_different() {
        let px: Vec<u8> = (0..32 * 32 * 4).map(|i| (i * 13 % 251) as u8).collect();
        let q = compare(&px, &px, 32);
        assert!((q.ssim - 1.0).abs() < 1e-9 && q.ssim_db >= 90.0);
        let inv: Vec<u8> = px.iter().map(|v| 255 - v).collect();
        assert!(compare(&px, &inv, 32).ssim < 0.2);
    }
}
