//! Putting a page's canvas renders together into one image, and onto
//! paper.

use image::{Rgba, RgbaImage};
use rayon::prelude::*;

/// An image and where its top-left pixel falls on the page's pixel grid.
#[derive(Clone, Debug)]
pub struct Placed {
    pub image: RgbaImage,
    pub at: (i64, i64),
}

impl Placed {
    fn end(&self) -> (i64, i64) {
        (self.at.0 + self.image.width() as i64, self.at.1 + self.image.height() as i64)
    }
}

/// `layers` drawn in order over transparency, into one image just covering
/// them all. `None` if there are none.
pub fn flatten(layers: Vec<Placed>) -> Option<Placed> {
    let first = layers.first()?;
    let (mut min, mut max) = (first.at, first.end());
    for layer in &layers[1..] {
        let end = layer.end();
        min = (min.0.min(layer.at.0), min.1.min(layer.at.1));
        max = (max.0.max(end.0), max.1.max(end.1));
    }
    if layers.len() == 1 {
        return layers.into_iter().next();
    }
    let mut out = Placed { image: RgbaImage::new((max.0 - min.0) as u32, (max.1 - min.1) as u32), at: min };
    for layer in &layers {
        draw(&mut out, layer, over);
    }
    Some(out)
}

/// Draw `top` onto `bottom` where they overlap, pixel by pixel with
/// `blend(top, bottom)`.
pub fn draw(bottom: &mut Placed, top: &Placed, blend: impl Fn(Rgba<u8>, Rgba<u8>) -> Rgba<u8> + Sync) {
    let (x0, y0) = (top.at.0.max(bottom.at.0), top.at.1.max(bottom.at.1));
    let (x1, y1) = (top.end().0.min(bottom.end().0), top.end().1.min(bottom.end().1));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let width = bottom.image.width() as usize;
    let at = bottom.at;
    bottom.image.par_chunks_mut(width * 4).enumerate()
        .filter(|(row, _)| (y0..y1).contains(&(*row as i64 + at.1)))
        .for_each(|(row, pixels)| {
            let y = row as i64 + at.1;
            for x in x0..x1 {
                let i = (x - at.0) as usize * 4;
                let under = Rgba([pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]);
                let ink = *top.image.get_pixel((x - top.at.0) as u32, (y - top.at.1) as u32);
                pixels[i..i + 4].copy_from_slice(&blend(ink, under).0);
            }
        });
}

/// Porter-Duff "over", in straight (unpremultiplied) alpha.
pub fn over(top: Rgba<u8>, bottom: Rgba<u8>) -> Rgba<u8> {
    let [tr, tg, tb, ta] = top.0.map(|v| v as f32 / 255.0);
    let [br, bg, bb, ba] = bottom.0.map(|v| v as f32 / 255.0);
    let a = ta + ba * (1.0 - ta);
    if a <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let channel = |t: f32, b: f32| (((t * ta + b * ba * (1.0 - ta)) / a) * 255.0).round() as u8;
    Rgba([channel(tr, br), channel(tg, bg), channel(tb, bb), (a * 255.0).round() as u8])
}

/// How much ink takes on the paper's shading: darker on stains and
/// scorched edges, and grained like the paper.
const SOAK: f32 = 0.35;
/// The tone of clean paper, which leaves ink as it is.
const CLEAN_PAPER: f32 = 0.9;

/// Ink laid on opaque paper: over it, but darkened a little where the
/// paper is darker, as ink soaks into it.
pub fn ink_on_paper(ink: Rgba<u8>, paper: Rgba<u8>) -> Rgba<u8> {
    let a = ink.0[3] as f32 / 255.0;
    if a <= 0.0 {
        return paper;
    }
    let [pr, pg, pb, _] = paper.0.map(|v| v as f32 / 255.0);
    let tone = ((0.2126 * pr + 0.7152 * pg + 0.0722 * pb) / CLEAN_PAPER).min(1.0);
    let soak = 1.0 - SOAK + SOAK * tone;
    let channel = |i: f32, p: f32| ((i * soak * a + p * (1.0 - a)) * 255.0).round() as u8;
    let [ir, ig, ib, _] = ink.0.map(|v| v as f32 / 255.0);
    Rgba([channel(ir, pr), channel(ig, pg), channel(ib, pb), 255])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4], at: (i64, i64)) -> Placed {
        Placed { image: RgbaImage::from_pixel(w, h, Rgba(c)), at }
    }

    #[test]
    fn flattening_covers_every_layer_in_order() {
        let flat = flatten(vec![solid(4, 4, [255, 0, 0, 255], (0, 0)), solid(4, 4, [0, 0, 255, 255], (2, 3))]).unwrap();
        assert_eq!(flat.at, (0, 0));
        assert_eq!(flat.image.dimensions(), (6, 7));
        assert_eq!(flat.image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(flat.image.get_pixel(3, 3).0, [0, 0, 255, 255], "later layers on top");
        assert_eq!(flat.image.get_pixel(5, 0).0[3], 0, "transparent where nothing is");
        assert!(flatten(Vec::new()).is_none());
    }

    #[test]
    fn over_blends_by_alpha() {
        assert_eq!(over(Rgba([255, 255, 255, 0]), Rgba([10, 20, 30, 255])).0, [10, 20, 30, 255]);
        assert_eq!(over(Rgba([200, 100, 0, 255]), Rgba([10, 20, 30, 255])).0, [200, 100, 0, 255]);
        let half = over(Rgba([255, 255, 255, 128]), Rgba([0, 0, 0, 255]));
        assert!((half.0[0] as i32 - 128).abs() <= 1 && half.0[3] == 255);
    }

    #[test]
    fn ink_soaks_into_darker_paper() {
        let ink = Rgba([200, 60, 40, 255]);
        let clean = ink_on_paper(ink, Rgba([240, 235, 220, 255]));
        let stained = ink_on_paper(ink, Rgba([180, 120, 70, 255]));
        assert_eq!(clean.0, [200, 60, 40, 255]);
        assert!(stained.0[0] < clean.0[0]);
        assert_eq!(ink_on_paper(Rgba([0, 0, 0, 0]), Rgba([1, 2, 3, 255])).0, [1, 2, 3, 255]);
    }
}
