//! Image paint: strokes coloured from a picture laid over the canvas, each
//! stroke (each subpath of a path) all in one colour, the picture's where
//! the stroke is halfway along. The picture's parts in some colours can be
//! kept clear of, as text keeps clear of what it avoids.

use image::RgbaImage;
use parry2d::math::Vec2;
use std::sync::{Arc, OnceLock};

use crate::journal::avoid::AvoidanceTree;
use crate::journal::canvas::color_jitter::oklab;

/// The most cells across or down the picture is looked over in for the
/// colours it avoids: a bigger picture's pixels are averaged in blocks.
const AVOID_GRID: u32 = 512;

/// A picture laid over the canvas, its top-left corner at `at`, `size`
/// across and down; beyond it, the colour at its nearest edge.
#[derive(Clone, Debug)]
pub struct ImagePaint {
    /// What the journal calls it: `images/NAME.png`.
    pub name: String,
    pub pixels: Arc<RgbaImage>,
    pub at: Vec2,
    pub size: Vec2,
    /// Its colours averaged, for where one colour has to stand for it.
    pub average: [f32; 4],
    /// Colours (sRGB, 0-1) whose parts of the picture are kept clear of,
    /// and how far off them (in OKLab) a colour still counts.
    pub avoid: Vec<[f32; 3]>,
    pub avoid_tolerance: f32,
    /// Those parts, worked out when first asked for: rectangles of the
    /// picture, as their top-left and bottom-right corners on the canvas.
    avoided: Arc<OnceLock<Vec<(Vec2, Vec2)>>>,
}

impl PartialEq for ImagePaint {
    fn eq(&self, other: &Self) -> bool {
        (Arc::ptr_eq(&self.pixels, &other.pixels) || self.name == other.name) && self.at == other.at && self.size == other.size
            && self.avoid == other.avoid && self.avoid_tolerance == other.avoid_tolerance
    }
}

/// How far an avoided colour reaches by default, in OKLab: about a tenth of
/// the way from black to white.
pub const DEFAULT_AVOID_TOLERANCE: f32 = 0.1;

impl ImagePaint {
    pub fn new(name: String, pixels: Arc<RgbaImage>, at: Vec2, size: Vec2) -> Self {
        let n = (pixels.width() as f64 * pixels.height() as f64).max(1.0);
        let mut sum = [0.0f64; 4];
        for p in pixels.pixels() {
            for (s, c) in sum.iter_mut().zip(p.0) {
                *s += c as f64;
            }
        }
        let average = sum.map(|s| (s / n / 255.0) as f32);
        ImagePaint { name, pixels, at, size, average, avoid: Vec::new(), avoid_tolerance: DEFAULT_AVOID_TOLERANCE, avoided: Arc::default() }
    }

    /// The same paint, keeping clear of its picture's parts in `colours`
    /// (sRGB, 0-1), or within `tolerance` of them in OKLab.
    pub fn avoiding(self, colours: Vec<[f32; 3]>, tolerance: f32) -> Self {
        ImagePaint { avoid: colours, avoid_tolerance: tolerance, avoided: Arc::default(), ..self }
    }

    /// The parts of the picture in the colours it avoids, as rectangles on
    /// the canvas (top-left and bottom-right corners): the picture looked
    /// over in at most `AVOID_GRID` cells across and down, each its
    /// pixels' average, rows of cells that match joined, and rows the same
    /// as the row above joined to it.
    pub fn avoided(&self) -> &[(Vec2, Vec2)] {
        self.avoided.get_or_init(|| {
            let (w, h) = (self.pixels.width(), self.pixels.height());
            if self.avoid.is_empty() || w == 0 || h == 0 {
                return Vec::new();
            }
            let block = w.max(h).div_ceil(AVOID_GRID).max(1);
            let (gw, gh) = (w.div_ceil(block), h.div_ceil(block));
            let targets: Vec<[f32; 3]> = self.avoid.iter().map(|c| oklab(*c)).collect();
            let matches = |gx: u32, gy: u32| {
                let mut sum = [0.0f32; 3];
                let mut n = 0.0;
                for y in gy * block..((gy + 1) * block).min(h) {
                    for x in gx * block..((gx + 1) * block).min(w) {
                        let p = self.pixels.get_pixel(x, y).0;
                        for (s, c) in sum.iter_mut().zip(p) {
                            *s += c as f32 / 255.0;
                        }
                        n += 1.0;
                    }
                }
                let lab = oklab(sum.map(|s| s / n));
                targets.iter().any(|t| {
                    let d = [lab[0] - t[0], lab[1] - t[1], lab[2] - t[2]];
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= self.avoid_tolerance
                })
            };
            // Runs of matching cells, row by row, each (start, end, first
            // row); a run the same as one in the row above carries it on.
            let mut done: Vec<(u32, u32, u32, u32)> = Vec::new();
            let mut open: Vec<(u32, u32, u32)> = Vec::new();
            for gy in 0..=gh {
                let mut runs = Vec::new();
                if gy < gh {
                    let mut gx = 0;
                    while gx < gw {
                        if matches(gx, gy) {
                            let start = gx;
                            while gx < gw && matches(gx, gy) {
                                gx += 1;
                            }
                            runs.push((start, gx));
                        } else {
                            gx += 1;
                        }
                    }
                }
                let mut next = Vec::new();
                for (start, end, top) in open.drain(..) {
                    if runs.contains(&(start, end)) {
                        next.push((start, end, top));
                    } else {
                        done.push((start, end, top, gy));
                    }
                }
                for (start, end) in runs {
                    if !next.iter().any(|&(s, e, _)| (s, e) == (start, end)) {
                        next.push((start, end, gy));
                    }
                }
                open = next;
            }
            let cell = Vec2::new(self.size.x * block as f32 / w as f32, self.size.y * block as f32 / h as f32);
            let corner = |gx: u32, gy: u32| (self.at + Vec2::new(gx as f32 * cell.x, gy as f32 * cell.y)).min(self.at + self.size);
            done.into_iter().map(|(x0, x1, y0, y1)| (corner(x0, y0), corner(x1, y1))).collect()
        })
    }

    /// Keep `tree` out of the picture's parts in the colours it avoids, and
    /// `clearance` away from them.
    pub fn add_avoided(&self, tree: &mut AvoidanceTree, clearance: f32) {
        for &(min, max) in self.avoided() {
            tree.add_polygon(&[min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)], clearance);
        }
    }

    /// The picture's colour at canvas point `p`, sRGB RGBA in 0-1, blended
    /// between its four nearest pixels.
    pub fn at_point(&self, p: Vec2) -> [f32; 4] {
        let (w, h) = (self.pixels.width(), self.pixels.height());
        if w == 0 || h == 0 {
            return self.average;
        }
        let u = (p.x - self.at.x) / self.size.x.max(1e-6) * w as f32 - 0.5;
        let v = (p.y - self.at.y) / self.size.y.max(1e-6) * h as f32 - 0.5;
        let (u, v) = (u.clamp(0.0, (w - 1) as f32), v.clamp(0.0, (h - 1) as f32));
        let (x0, y0) = (u.floor() as u32, v.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (fx, fy) = (u - x0 as f32, v - y0 as f32);
        let px = |x, y| self.pixels.get_pixel(x, y).0.map(|c| c as f32 / 255.0);
        let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }

    /// The colour of a stroke through `points`: the picture's where it's
    /// halfway along.
    pub fn stroke_colour(&self, points: impl IntoIterator<Item = Vec2>) -> [f32; 4] {
        halfway(points).map_or(self.average, |p| self.at_point(p))
    }
}

/// The point halfway along a line through `points`; `None` if there are
/// none.
pub fn halfway(points: impl IntoIterator<Item = Vec2>) -> Option<Vec2> {
    let points: Vec<Vec2> = points.into_iter().collect();
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total / 2.0;
    for w in points.windows(2) {
        let d = w[0].distance(w[1]);
        if d >= left && d > 0.0 {
            return Some(w[0].lerp(w[1], left / d));
        }
        left -= d;
    }
    points.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_tone() -> ImagePaint {
        // Red on the left half, blue on the right.
        let pixels = RgbaImage::from_fn(4, 2, |x, _| if x < 2 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        ImagePaint::new("two_tone".into(), Arc::new(pixels), Vec2::new(100.0, 0.0), Vec2::new(40.0, 20.0))
    }

    #[test]
    fn strokes_take_the_colour_halfway_along() {
        let paint = two_tone();
        assert_eq!(halfway([Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 30.0)]), Some(Vec2::new(10.0, 10.0)));
        assert_eq!(halfway([Vec2::new(3.0, 4.0)]), Some(Vec2::new(3.0, 4.0)));
        assert_eq!(halfway(Vec::new()), None);
        // A stroke mostly over the left half, ending in the right.
        let stroke = [Vec2::new(100.0, 10.0), Vec2::new(125.0, 10.0)];
        assert_eq!(paint.stroke_colour(stroke), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(paint.stroke_colour([Vec2::new(130.0, 5.0), Vec2::new(138.0, 5.0)]), [0.0, 0.0, 1.0, 1.0]);
        // Beyond the picture, its nearest edge.
        assert_eq!(paint.at_point(Vec2::new(-500.0, 900.0)), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(paint.average, [0.5, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn avoided_colours_are_rectangles_of_the_picture() {
        // A 6 by 4 picture, white but for a red block two across and three
        // down from (1, 0), and a red pixel alone at (5, 3).
        let red = |x: u32, y: u32| ((1..3).contains(&x) && y < 3) || (x, y) == (5, 3);
        let pixels = RgbaImage::from_fn(6, 4, |x, y| if red(x, y) { image::Rgba([250, 10, 0, 255]) } else { image::Rgba([255, 255, 255, 255]) });
        let paint = ImagePaint::new("block".into(), Arc::new(pixels), Vec2::new(100.0, 50.0), Vec2::new(60.0, 40.0));
        assert!(paint.avoided().is_empty(), "nothing avoided unless asked");
        let paint = paint.avoiding(vec![[1.0, 0.0, 0.0]], 0.1);
        let mut rects = paint.avoided().to_vec();
        rects.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        assert_eq!(rects, vec![
            (Vec2::new(110.0, 50.0), Vec2::new(130.0, 80.0)),
            (Vec2::new(150.0, 80.0), Vec2::new(160.0, 90.0)),
        ], "the block whole, rows joined, and the pixel alone, each 10 units a pixel");
        let mut tree = AvoidanceTree::new();
        paint.add_avoided(&mut tree, 0.0);
        assert!(tree.blocks_point(Vec2::new(120.0, 60.0)) && !tree.blocks_point(Vec2::new(140.0, 60.0)));
        assert!(paint.clone().avoiding(vec![[0.0, 0.0, 1.0]], 0.1).avoided().is_empty(), "no blue in it");
        assert_eq!(paint.clone().avoiding(vec![[1.0, 0.0, 0.0]], 2.0).avoided(), &[(Vec2::new(100.0, 50.0), Vec2::new(160.0, 90.0))], "every colour, within a wide enough tolerance");
    }
}
