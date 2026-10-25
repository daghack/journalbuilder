//! Parchment to render pages on (`--parchment`): a procedural texture in
//! the manner of an old scroll, mottled and fibrous, browning toward
//! ragged edges, drawn under a page's strokes.
//!
//! Textures are worked out in canvas units, so they look the same at any
//! `--scale`. Each page is a sheet of its own, seeded from its name: the
//! same every run, and different from page to page. A split page's tiles
//! are sheets of their own too, like a big drawing spread over several
//! pages. Ink soaks into a sheet a little unevenly (`soak`).

use image::{ImageBuffer, RgbaImage};
use crate::journal::compose::Placed;
use noise::{NoiseFn, Perlin};
use rayon::prelude::*;

/// A sheet of paper on a grid of pixels from the canvas units' origin at
/// `scale` pixels per unit: its corner and size.
#[derive(Clone, Copy, Debug)]
pub struct Sheet {
    pub corner: (i64, i64),
    pub size: (u32, u32),
    pub scale: f32,
    pub seed: u64,
}

/// The sheet, opaque.
pub fn paint(sheet: &Sheet) -> RgbaImage {
    let (x0, y0) = sheet.corner;
    let (width, height) = sheet.size;
    let texture = Texture::new(sheet);
    let pixels: Vec<u8> = (0..height).into_par_iter().flat_map_iter(|row| {
        let texture = &texture;
        (0..width).flat_map(move |column| {
            // The pixel's centre, in canvas units.
            let x = (x0 + column as i64) as f64 + 0.5;
            let y = (y0 + row as i64) as f64 + 0.5;
            let p = [x / texture.scale, y / texture.scale];
            let [r, g, b] = texture.parchment(p);
            let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            [byte(r), byte(g), byte(b), 255]
        })
    }).collect();
    ImageBuffer::from_raw(width, height, pixels).expect("one pixel per position")
}

/// The most ink thins where the sheet takes it least, as a fraction.
const INK_SOAK: f64 = 0.12;

/// Ink, `ink`, as the sheet takes it: very slightly thinner in places, in
/// blotches and grain of the sheet's own (its alpha scaled down by up to
/// `INK_SOAK`), as ink soaks unevenly into paper. Read in canvas units, so
/// it looks the same at any scale.
pub fn soak(ink: &mut Placed, sheet: &Sheet) {
    let texture = Texture::new(sheet);
    let (x0, y0) = ink.at;
    let width = ink.image.width() as usize;
    ink.image.par_chunks_mut(width * 4).enumerate().for_each(|(row, pixels)| {
        for (column, px) in pixels.chunks_mut(4).enumerate() {
            if px[3] == 0 {
                continue;
            }
            let x = (x0 + column as i64) as f64 + 0.5;
            let y = (y0 + row as i64) as f64 + 0.5;
            let thin = texture.thinning([x / texture.scale, y / texture.scale]);
            px[3] = (px[3] as f64 * (1.0 - INK_SOAK * thin)).round() as u8;
        }
    });
}

type Rgb = [f64; 3];

fn rgb(hex: u32) -> Rgb {
    [(hex >> 16) as f64 / 255.0, ((hex >> 8) & 0xff) as f64 / 255.0, (hex & 0xff) as f64 / 255.0]
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn shade(c: Rgb, k: f64) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// SplitMix64: a well-mixed hash of a seed and a grid cell.
fn hash(seed: u64, a: i64, b: i64) -> u64 {
    let mut z = seed ^ (a as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (b as u64).wrapping_mul(0xC2B2AE3D27D4EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// A hash as a number in [0, 1).
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// The noise a sheet is made from.
struct Texture {
    /// Pixels per unit.
    scale: f64,
    /// The sheet's corners in units.
    min: [f64; 2],
    max: [f64; 2],
    seed: u64,
    /// Independent noise for each feature, so they don't line up.
    layers: Vec<Perlin>,
}

/// Noise layers, by what they're for.
const MOTTLE: usize = 0;
const CLOUD: usize = 1;
const GRAIN: usize = 2;
const FIBRE: usize = 3;
const EDGE: usize = 4;
const INK: usize = 5;
const LAYERS: usize = 6;

impl Texture {
    fn new(sheet: &Sheet) -> Self {
        let scale = sheet.scale as f64;
        let min = [sheet.corner.0 as f64 / scale, sheet.corner.1 as f64 / scale];
        let size = [sheet.size.0 as f64 / scale, sheet.size.1 as f64 / scale];
        let layers = (0..LAYERS).map(|i| Perlin::new(hash(sheet.seed, i as i64, 0) as u32)).collect();
        Texture { scale, min, max: [min[0] + size[0], min[1] + size[1]], seed: sheet.seed, layers }
    }

    /// Fractal noise of `octaves` octaves at `period` units, about -1..1.
    fn fbm(&self, layer: usize, p: [f64; 2], period: f64, octaves: u32) -> f64 {
        let noise = &self.layers[layer];
        let (mut sum, mut amplitude, mut frequency, mut total) = (0.0, 1.0, 1.0 / period, 0.0);
        for octave in 0..octaves {
            // Shifted each octave, so octaves don't share a zero at the origin.
            let shift = 17.3 * octave as f64;
            sum += amplitude * noise.get([p[0] * frequency + shift, p[1] * frequency - shift]);
            total += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }
        // Perlin noise rarely strays past +-0.7; stretch it to about +-1.
        sum / total * 1.4
    }

    /// A period of at least a few pixels: finer noise would alias into
    /// moiré at small scales.
    fn fine(&self, period: f64) -> f64 {
        period.max(2.5 / self.scale)
    }

    /// Paper grain and fibres: a factor near 1.
    fn tooth(&self, p: [f64; 2]) -> f64 {
        let grain_period = self.fine(2.2);
        let grain = self.layers[GRAIN].get([p[0] / grain_period, p[1] / grain_period]);
        // Fibres: noise stretched along a direction that drifts over the
        // sheet.
        let angle = 0.6 + 0.5 * self.layers[FIBRE].get([p[0] / 900.0, p[1] / 900.0]);
        let (c, s) = (angle.cos(), angle.sin());
        let (along, across) = (p[0] * c + p[1] * s, -p[0] * s + p[1] * c);
        let fibre = self.layers[FIBRE].get([along / self.fine(28.0) + 40.0, across / self.fine(1.4)]);
        // Fades out as it gets too fine to see.
        let fade = (2.0 * self.scale).min(1.0);
        1.0 + fade * (0.03 * grain + 0.022 * fibre.max(-0.15))
    }

    /// How much less ink the sheet takes at `p`, from 0 to 1: broad patches,
    /// blotches and grain.
    fn thinning(&self, p: [f64; 2]) -> f64 {
        let broad = self.fbm(INK, [p[0] + 311.0, p[1] - 97.0], 160.0, 2);
        let blotch = self.fbm(INK, p, 40.0, 3);
        let period = self.fine(3.0);
        // Fades out as it gets too fine to see, as the paper's grain does.
        let grain = (2.0 * self.scale).min(1.0) * self.layers[INK].get([p[0] / period + 53.0, p[1] / period - 29.0]) * 1.4;
        (0.5 + 0.5 * (0.3 * broad + 0.45 * blotch + 0.25 * grain)).clamp(0.0, 1.0)
    }

    /// Specks of grit: how dark one is here, 0 for none.
    fn specks(&self, p: [f64; 2], density: f64) -> f64 {
        const CELL: f64 = 14.0;
        let (cx, cy) = ((p[0] / CELL).floor() as i64, (p[1] / CELL).floor() as i64);
        let mut darkest: f64 = 0.0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let h = hash(self.seed ^ 0x5eed, cx + dx, cy + dy);
                if unit(h) > density {
                    continue;
                }
                let u = |k: u64| unit(hash(h, k as i64, 7));
                let centre = [(cx + dx) as f64 * CELL + u(1) * CELL, (cy + dy) as f64 * CELL + u(2) * CELL];
                let radius = 0.35 + 1.1 * u(3) * u(3);
                let d = ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt();
                darkest = darkest.max((0.25 + 0.5 * u(4)) * smoothstep(radius + 0.5, radius - 0.3, d));
            }
        }
        darkest
    }

    /// How far into the sheet `p` is from its nearest edge, roughened, and
    /// how wide the darkened border is.
    fn edge(&self, p: [f64; 2]) -> (f64, f64) {
        let short = (self.max[0] - self.min[0]).min(self.max[1] - self.min[1]);
        let width = (0.07 * short).clamp(25.0, 180.0);
        let d = (p[0] - self.min[0]).min(p[1] - self.min[1]).min(self.max[0] - p[0]).min(self.max[1] - p[1]);
        let ragged = d + width * 0.5 * self.fbm(EDGE, p, width * 1.4, 5);
        (ragged, width)
    }

    fn parchment(&self, p: [f64; 2]) -> Rgb {
        let cream = rgb(0xEFE4C8);
        let light = rgb(0xF8F2E2);
        let tan = rgb(0xDCC293);
        let mottle = self.fbm(MOTTLE, p, 420.0, 5);
        let cloud = self.fbm(CLOUD, p, 110.0, 5);
        let mut c = mix(cream, tan, 0.5 * smoothstep(-0.2, 0.9, mottle) + 0.3 * smoothstep(0.25, 1.0, cloud));
        c = mix(c, light, 0.55 * smoothstep(0.0, -0.8, mottle));
        c = shade(c, self.tooth(p));
        // Aged edges: browning gradually into the sheet, darkest in an
        // uneven rim.
        let (into, width) = self.edge(p);
        let browned = smoothstep(width * 1.6, 0.0, into);
        c = mix(c, rgb(0xC8A06A), 0.55 * browned * browned);
        c = mix(c, rgb(0x9C6C3C), 0.45 * smoothstep(width * 0.3, -width * 0.1, into));
        mix(c, rgb(0x5A3A1E), self.specks(p, 0.05))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_is_opaque_and_follows_its_seed() {
        let sheet = Sheet { corner: (-10, 5), size: (60, 40), scale: 0.2, seed: 3 };
        let a = paint(&sheet);
        assert_eq!(a.dimensions(), (60, 40));
        assert!(a.pixels().all(|p| p.0[3] == 255));
        assert_eq!(a, paint(&sheet));
        assert_ne!(a, paint(&Sheet { seed: 4, ..sheet }));
    }

    #[test]
    fn ink_soaks_in_unevenly_and_only_thins() {
        let sheet = Sheet { corner: (0, 0), size: (400, 300), scale: 1.0, seed: 9 };
        let mut image = RgbaImage::from_pixel(300, 200, image::Rgba([20, 30, 90, 255]));
        image.put_pixel(5, 5, image::Rgba([20, 30, 90, 0]));
        let mut ink = Placed { image, at: (50, 40) };
        soak(&mut ink, &sheet);
        assert_eq!(ink.image.get_pixel(5, 5).0[3], 0, "nothing drawn stays nothing");
        let alphas: Vec<u8> = ink.image.pixels().map(|p| p.0[3]).filter(|a| *a > 0).collect();
        let (lo, hi) = (*alphas.iter().min().unwrap(), *alphas.iter().max().unwrap());
        assert!(lo >= (255.0 * (1.0 - INK_SOAK)).floor() as u8, "only a little thinner: {}", lo);
        assert!(hi - lo > 10, "uneven: {}..{}", lo, hi);
        assert!(ink.image.pixels().all(|p| p.0[..3] == [20, 30, 90]), "the ink's colour is kept");
        let mut again = Placed { image: RgbaImage::from_pixel(300, 200, image::Rgba([20, 30, 90, 255])), at: (50, 40) };
        again.image.put_pixel(5, 5, image::Rgba([20, 30, 90, 0]));
        soak(&mut again, &sheet);
        assert_eq!(again.image, ink.image, "the same sheet soaks the same");
    }
}
