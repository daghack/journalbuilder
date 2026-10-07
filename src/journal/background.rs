//! Paper to render pages on: procedural textures in the manner of old
//! letters and scrolls, drawn under a page's strokes (`--background`).
//!
//! Textures are worked out in canvas units, so they look the same at any
//! `--scale`, and from the whole page's pixel grid, so a split page's tiles
//! have their own parts of one sheet. Each page's sheet is seeded from its
//! name: the same every run, and different from page to page.

use image::{ImageBuffer, RgbaImage};
use noise::{NoiseFn, Perlin};
use rayon::prelude::*;

/// A kind of paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paper {
    /// Cream parchment, mottled and fibrous, darkening to scorched,
    /// ragged edges: a scroll.
    Parchment,
    /// Pale paper splashed with tea: blotches with dark tide lines at
    /// their edges, cup rings, and clear patches between.
    Stained,
}

impl Paper {
    pub const NAMES: [&'static str; 2] = ["parchment", "stained"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "parchment" => Some(Self::Parchment),
            "stained" => Some(Self::Stained),
            _ => None,
        }
    }
}

/// A sheet of paper on a grid of pixels from the canvas units' origin at
/// `scale` pixels per unit: its corner and size, and the pixels to paint,
/// `[x0, y0, x1, y1)` (all of it, but for a tile). The texture is fixed to
/// canvas units, so the same spot looks the same on any sheet with the
/// same seed; only the edges follow the sheet.
#[derive(Clone, Copy, Debug)]
pub struct Sheet {
    pub corner: (i64, i64),
    pub size: (u32, u32),
    pub crop: [i64; 4],
    pub scale: f32,
    pub seed: u64,
}

impl Sheet {
    /// A whole sheet, painted all over.
    pub fn whole(corner: (i64, i64), size: (u32, u32), scale: f32, seed: u64) -> Self {
        let crop = [corner.0, corner.1, corner.0 + size.0 as i64, corner.1 + size.1 as i64];
        Sheet { corner, size, crop, scale, seed }
    }
}

/// The paper under `sheet`'s pixels, opaque.
pub fn paint(paper: Paper, sheet: &Sheet) -> RgbaImage {
    let [x0, y0, x1, y1] = sheet.crop;
    let (width, height) = ((x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32);
    let texture = Texture::new(sheet);
    let pixels: Vec<u8> = (0..height).into_par_iter().flat_map_iter(|row| {
        let texture = &texture;
        (0..width).flat_map(move |column| {
            // The pixel's centre, in canvas units.
            let x = (x0 + column as i64) as f64 + 0.5;
            let y = (y0 + row as i64) as f64 + 0.5;
            let p = [x / texture.scale, y / texture.scale];
            let [r, g, b] = match paper {
                Paper::Parchment => texture.parchment(p),
                Paper::Stained => texture.stained(p),
            };
            let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            [byte(r), byte(g), byte(b), 255]
        })
    }).collect();
    ImageBuffer::from_raw(width, height, pixels).expect("one pixel per position")
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
    /// Cup rings: centre, radius and how far they wander.
    rings: Vec<([f64; 2], f64, f64)>,
}

/// Noise layers, by what they're for.
const MOTTLE: usize = 0;
const CLOUD: usize = 1;
const GRAIN: usize = 2;
const FIBRE: usize = 3;
const EDGE: usize = 4;
const WARP_X: usize = 5;
const WARP_Y: usize = 6;
const STAIN: usize = 7;
const DEPTH: usize = 8;
const LAYERS: usize = 9;

impl Texture {
    fn new(sheet: &Sheet) -> Self {
        let scale = sheet.scale as f64;
        let min = [sheet.corner.0 as f64 / scale, sheet.corner.1 as f64 / scale];
        let size = [sheet.size.0 as f64 / scale, sheet.size.1 as f64 / scale];
        let layers = (0..LAYERS).map(|i| Perlin::new(hash(sheet.seed, i as i64, 0) as u32)).collect();
        // A few cup rings, more on bigger sheets, kept mostly on the sheet.
        let area = size[0] * size[1];
        let count = 1 + (unit(hash(sheet.seed, -1, 0)) * (area / 700_000.0).clamp(1.0, 4.0)) as i64;
        let rings = (0..count).map(|i| {
            let u = |k: i64| unit(hash(sheet.seed, -2 - i, k));
            let radius = size[0].min(size[1]) * (0.08 + 0.1 * u(0));
            let centre = [min[0] + size[0] * (0.1 + 0.8 * u(1)), min[1] + size[1] * (0.1 + 0.8 * u(2))];
            (centre, radius, radius * (0.05 + 0.06 * u(3)))
        }).collect();
        Texture { scale, min, max: [min[0] + size[0], min[1] + size[1]], seed: sheet.seed, layers, rings }
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

    /// The tea's level here, and how far `p` is from where it dried at
    /// each of `SHORES`, in units: positive inside.
    fn tea(&self, p: [f64; 2]) -> (f64, [f64; 2]) {
        let level = |p: [f64; 2]| {
            let warp = [p[0] + 25.0 * self.fbm(WARP_X, p, 200.0, 3), p[1] + 25.0 * self.fbm(WARP_Y, p, 200.0, 3)];
            self.fbm(STAIN, warp, 380.0, 7)
        };
        let here = level(p);
        // The slope gives the distance to a shore, so tide lines are as
        // wide where the level is flat as where it's steep.
        const H: f64 = 0.75;
        let slope = (((level([p[0] + H, p[1]]) - here) / H).powi(2) + ((level([p[0], p[1] + H]) - here) / H).powi(2)).sqrt().max(1e-4);
        (here, SHORES.map(|shore| (here - shore) / slope))
    }

    fn stained(&self, p: [f64; 2]) -> Rgb {
        let paper = rgb(0xF6F0E2);
        let wash = rgb(0xEDD9B4);
        let pale_tea = rgb(0xE8C38C);
        let strong_tea = rgb(0xCC7F3C);
        let tide = rgb(0x9A4A1E);
        let tooth = self.tooth(p);
        // A faint wash over everything, so no paper is quite clean.
        let mut c = mix(paper, wash, 0.35 * smoothstep(-0.4, 0.8, self.fbm(CLOUD, p, 300.0, 4)));
        c = shade(c, tooth);
        // Blotches where the tea's level is high: darkest in a band inside
        // their edges, where it pooled as it dried, with a dark tide line at
        // the edge itself, and a fainter one where a second, smaller pool
        // dried inside it.
        let (level, [shore, inner]) = self.tea(p);
        let depth = 0.5 + 0.5 * self.fbm(DEPTH, p, 55.0, 4);
        let feather = 0.6 + 2.5 * smoothstep(0.3, 1.0, self.fbm(MOTTLE, p, 160.0, 3));
        let wet = smoothstep(-feather, feather, shore);
        let pooled = (-shore.max(0.0) / 28.0).exp();
        let darker = 0.15 + 0.4 * depth * depth + 0.35 * pooled + 0.25 * smoothstep(-2.0, 6.0, inner) + 0.2 * smoothstep(SHORES[0] + 0.3, SHORES[0] + 0.8, level);
        let tea = shade(mix(pale_tea, strong_tea, darker), tooth);
        c = mix(c, tea, 0.92 * wet);
        let line = |from_shore: f64, width: f64| (-((from_shore - width) / width).powi(2)).exp();
        let width = 1.4 + 1.6 * depth;
        c = mix(c, tide, 0.7 * line(shore, width) / feather.sqrt());
        c = mix(c, tide, 0.35 * line(inner, width * 0.8));
        // Cup rings, wobbling and broken.
        for (i, &(centre, radius, wander)) in self.rings.iter().enumerate() {
            let (dx, dy) = (p[0] - centre[0], p[1] - centre[1]);
            let angle = dy.atan2(dx);
            let around = [angle.cos() * 1.7 + i as f64 * 9.0, angle.sin() * 1.7];
            let wobble = wander * self.layers[EDGE].get(around);
            let off = (dx * dx + dy * dy).sqrt() - radius - wobble;
            let thickness = radius * 0.02 + 1.5;
            let strength = smoothstep(-0.45, 0.3, self.layers[CLOUD].get([around[0] * 1.4, around[1] * 1.4 + 3.0]));
            let ring = (-(off / thickness).powi(2)).exp() * strength;
            c = mix(c, pale_tea, 0.3 * smoothstep(0.0, -thickness * 2.0, off));
            c = mix(c, tide, 0.6 * ring);
        }
        let (into, width) = self.edge(p);
        c = mix(c, shade(strong_tea, 0.9), 0.4 * smoothstep(width * 0.8, 0.0, into).powf(1.5));
        mix(c, rgb(0x6B4426), self.specks(p, 0.03))
    }
}

/// Where tea dried: the levels of the stain noise at a blotch's edge, and
/// at a smaller pool's inside it.
const SHORES: [f64; 2] = [0.1, 0.32];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn papers_are_opaque_and_follow_their_seed() {
        for paper in [Paper::Parchment, Paper::Stained] {
            let sheet = Sheet::whole((-10, 5), (60, 40), 0.2, 3);
            let a = paint(paper, &sheet);
            assert_eq!(a.dimensions(), (60, 40));
            assert!(a.pixels().all(|p| p.0[3] == 255));
            assert_eq!(a, paint(paper, &sheet));
            assert_ne!(a, paint(paper, &Sheet { seed: 4, ..sheet }));
        }
    }

    #[test]
    fn a_tile_is_its_part_of_the_whole_sheet() {
        let whole = Sheet::whole((0, 0), (50, 30), 0.1, 9);
        let tile = Sheet { crop: [20, 10, 50, 30], ..whole };
        let (whole, tile) = (paint(Paper::Stained, &whole), paint(Paper::Stained, &tile));
        for (x, y, p) in tile.enumerate_pixels() {
            assert_eq!(p, whole.get_pixel(x + 20, y + 10));
        }
    }

    #[test]
    fn papers_have_names() {
        for name in Paper::NAMES {
            assert!(Paper::from_name(name).is_some());
        }
        assert_eq!(Paper::from_name("vellum"), None);
    }
}
