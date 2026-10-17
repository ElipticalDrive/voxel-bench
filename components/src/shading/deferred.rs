//! The common deferred shading pass on the CPU (PLAN step 12), as `deferred.wgsl` on the
//! GPU: every pixel's stored record is read, decoded into a surface and shaded, rows in
//! parallel. The record's encoding is the strategy's (the common [`Record`], or an id its
//! source decodes: cpu-raster's quad ids), and so is the shading call (the f32 component,
//! or cpu-raster's f64 reference model); both are compiled into the pass per strategy
//! (monomorphised), so the pass costs no indirection.
//!
//! [`Record`]: crate::visibility::record::Record

use rayon::prelude::*;

/// A strategy's stored records and how they are shaded.
pub trait Records: Sync {
    /// One pixel's stored record.
    type Record: Copy;
    /// The record of pixel (`x`, `y`), wherever the strategy keeps it.
    fn record_at(&self, x: u32, y: u32) -> Self::Record;
    /// Whether the record holds a surface (else the pixel shows `miss`).
    fn hit(r: Self::Record) -> bool;
    /// The colour of pixel (`x`, `y`) from its record: decoded into the surface on the
    /// pixel's primary ray, then shaded.
    fn shade(&self, r: Self::Record, x: u32, y: u32) -> [u8; 4];
    /// The colour where nothing was hit (the sky).
    fn miss(&self, x: u32, y: u32) -> [u8; 4];
}

/// Shades every pixel of the `w`-wide RGBA8 image `pixels` from its record.
pub fn shade<R: Records>(records: &R, w: u32, pixels: &mut [u8]) {
    pixels.par_chunks_mut(w as usize * 4).enumerate().for_each(|(y, row)| {
        for (x, px) in row.chunks_exact_mut(4).enumerate() {
            let (x, y) = (x as u32, y as u32);
            let r = records.record_at(x, y);
            px.copy_from_slice(&if R::hit(r) { records.shade(r, x, y) } else { records.miss(x, y) });
        }
    });
}
