//! Gradients: strokes whose colour changes along them, or across the page.

use image::RgbaImage;
use parry2d::math::Vec2;
use rayon::prelude::*;
use std::collections::HashMap;

use crate::journal::canvas::color_jitter::{shifted, Shift};

/// A run of colours, and how it's laid over strokes.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// Where each colour is, from 0 to 1 in order, as sRGB RGBA in 0-1.
    pub stops: Vec<(f32, [f32; 4])>,
    pub kind: GradientKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientKind {
    /// Along each stroke (each subpath of a path), from its start to its
    /// end.
    Along,
    /// Down each path (each glyph, for text), from its top to its bottom.
    Vertical,
    /// Across the canvas from `from` to `to`, the same all along lines at
    /// right angles to it; before `from` and past `to`, the end colours.
    Linear { from: Vec2, to: Vec2 },
    /// Out from `center` to `radius`, and the last colour beyond.
    Radial { center: Vec2, radius: f32 },
}

impl Gradient {
    /// The colour `t` of the way through, from 0 to 1.
    pub fn at(&self, t: f32) -> [f32; 4] {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let stops = &self.stops;
        let next = stops.iter().position(|(at, _)| *at >= t).unwrap_or(stops.len() - 1);
        if next == 0 {
            return stops[0].1;
        }
        let ((a, ca), (b, cb)) = (stops[next - 1], stops[next]);
        let k = if b > a { (t - a) / (b - a) } else { 1.0 };
        std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * k)
    }

    /// Whether it's laid over each stroke on its own (along it, or down
    /// it) rather than across the canvas.
    pub fn per_stroke(&self) -> bool {
        matches!(self.kind, GradientKind::Along | GradientKind::Vertical)
    }

    /// How far through the gradient a point is, for gradients laid across
    /// the canvas; `None` for gradients laid over each stroke.
    pub fn t_at(&self, p: Vec2) -> Option<f32> {
        match self.kind {
            GradientKind::Along | GradientKind::Vertical => None,
            GradientKind::Linear { from, to } => {
                let d = to - from;
                Some((p - from).dot(d) / d.length_squared().max(1e-12))
            }
            GradientKind::Radial { center, radius } => Some(p.distance(center) / radius.max(1e-6)),
        }
    }

    /// The colour at a point, for gradients laid across the canvas.
    pub fn at_point(&self, p: Vec2) -> Option<[f32; 4]> {
        self.t_at(p).map(|t| self.at(t))
    }

    /// The first colour: where an along gradient starts.
    pub fn first(&self) -> [f32; 4] {
        self.stops[0].1
    }
}

/// Points along strokes, each with how far through the gradient it is (0
/// to 1: along its stroke, or down it) and how its stroke's colour is
/// jittered, for finding both for any nearby pixel.
pub struct Along {
    cell: f32,
    cells: HashMap<(i32, i32), Vec<(Vec2, f32, Shift)>>,
}

impl Along {
    /// For points about `spacing` apart, on strokes about `reach` across.
    pub fn new(points: Vec<(Vec2, f32, Shift)>, reach: f32) -> Self {
        let cell = reach.max(4.0);
        let mut cells: HashMap<(i32, i32), Vec<(Vec2, f32, Shift)>> = HashMap::new();
        for (p, t, shift) in points {
            cells.entry(Self::key(p, cell)).or_default().push((p, t, shift));
        }
        Along { cell, cells }
    }

    fn key(p: Vec2, cell: f32) -> (i32, i32) {
        ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32)
    }

    /// How far along its stroke the nearest point to `p` is, and its
    /// stroke's jitter; 0 and none if there are none nearby (within a few
    /// cells).
    pub fn near(&self, p: Vec2) -> (f32, Shift) {
        let (cx, cy) = Self::key(p, self.cell);
        let mut best: Option<(f32, f32, Shift)> = None;
        // Rings of cells outwards, until nothing in the next ring could be
        // nearer than what's been found.
        for ring in 0i32..6 {
            if let Some((distance, _, _)) = best
                && distance < (ring as f32 - 1.0) * self.cell {
                break;
            }
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    for &(q, t, shift) in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                        let d = q.distance(p);
                        if best.is_none_or(|(bd, _, _)| d < bd) {
                            best = Some((d, t, shift));
                        }
                    }
                }
            }
        }
        best.map_or((0.0, [0.0; 3]), |(_, t, shift)| (t, shift))
    }
}

/// Colour a render of strokes drawn in white with `gradient`, in place:
/// each pixel takes the gradient's colour where it is, times its own (so a
/// brush's shading stays), with the gradient's alpha on top of its own.
/// `to_canvas` turns a pixel's centre into canvas units, for gradients
/// across the canvas; `along` gives how far through the gradient a pixel
/// is, for gradients laid over each stroke, and how the nearest stroke's
/// colour is jittered, for any gradient.
pub fn colorize(image: &mut RgbaImage, gradient: &Gradient, to_canvas: impl Fn(f32, f32) -> Vec2 + Sync, along: Option<&Along>) {
    let width = image.width() as usize;
    image.par_chunks_mut(width * 4).enumerate().for_each(|(y, row)| {
        for x in 0..width {
            let px = &mut row[x * 4..x * 4 + 4];
            if px[3] == 0 {
                continue;
            }
            let centre = (x as f32 + 0.5, y as f32 + 0.5);
            let (t, shift) = along.map_or((0.0, [0.0; 3]), |a| a.near(Vec2::new(centre.0, centre.1)));
            let colour = shifted(match gradient.kind {
                GradientKind::Along | GradientKind::Vertical => gradient.at(t),
                _ => gradient.at_point(to_canvas(centre.0, centre.1)).expect("a gradient across the canvas"),
            }, shift);
            for (c, g) in px[..3].iter_mut().zip(colour) {
                *c = (*c as f32 * g).round().clamp(0.0, 255.0) as u8;
            }
            px[3] = (px[3] as f32 * colour[3]).round() as u8;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red_to_blue(kind: GradientKind) -> Gradient {
        Gradient { stops: vec![(0.0, [1.0, 0.0, 0.0, 1.0]), (1.0, [0.0, 0.0, 1.0, 0.5])], kind }
    }

    #[test]
    fn colours_blend_between_stops() {
        let g = Gradient { stops: vec![(0.0, [1.0, 0.0, 0.0, 1.0]), (0.25, [0.0, 1.0, 0.0, 1.0]), (1.0, [0.0, 0.0, 1.0, 1.0])], kind: GradientKind::Along };
        assert_eq!(g.at(0.0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(g.at(-3.0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(g.at(0.25), [0.0, 1.0, 0.0, 1.0]);
        let mid = g.at(0.625);
        assert!((mid[1] - 0.5).abs() < 1e-6 && (mid[2] - 0.5).abs() < 1e-6);
        assert_eq!(g.at(7.0), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(g.at(f32::NAN), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn linear_and_radial_gradients_follow_position() {
        let linear = red_to_blue(GradientKind::Linear { from: Vec2::new(0.0, 0.0), to: Vec2::new(10.0, 0.0) });
        assert_eq!(linear.t_at(Vec2::new(5.0, 30.0)), Some(0.5));
        assert_eq!(linear.at_point(Vec2::new(-4.0, 0.0)), Some([1.0, 0.0, 0.0, 1.0]));
        let radial = red_to_blue(GradientKind::Radial { center: Vec2::new(1.0, 1.0), radius: 4.0 });
        assert_eq!(radial.t_at(Vec2::new(1.0, 3.0)), Some(0.5));
        assert_eq!(red_to_blue(GradientKind::Along).t_at(Vec2::ZERO), None);
        assert_eq!(red_to_blue(GradientKind::Vertical).t_at(Vec2::ZERO), None);
    }

    #[test]
    fn along_finds_the_nearest_point() {
        let points = (0..=100).map(|i| (Vec2::new(i as f32, 0.0), i as f32 / 100.0, [i as f32, 0.0, 0.0])).collect();
        let along = Along::new(points, 6.0);
        assert!((along.near(Vec2::new(30.2, 3.0)).0 - 0.3).abs() < 1e-6);
        assert_eq!(along.near(Vec2::new(30.2, 3.0)).1, [30.0, 0.0, 0.0], "the nearest point's jitter");
        assert!((along.near(Vec2::new(99.0, -2.0)).0 - 0.99).abs() < 1e-6);
        assert_eq!(Along::new(Vec::new(), 6.0).near(Vec2::ZERO), (0.0, [0.0; 3]));
    }

    #[test]
    fn colorizing_keeps_shading_and_multiplies_alpha() {
        let mut image = RgbaImage::from_pixel(10, 1, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(9, 0, image::Rgba([128, 128, 128, 100]));
        image.put_pixel(4, 0, image::Rgba([255, 255, 255, 0]));
        let g = red_to_blue(GradientKind::Linear { from: Vec2::new(0.5, 0.0), to: Vec2::new(9.5, 0.0) });
        colorize(&mut image, &g, Vec2::new, None);
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(9, 0).0, [0, 0, 128, 50]);
        assert_eq!(image.get_pixel(4, 0).0, [255, 255, 255, 0], "untouched where nothing is drawn");
    }
}
