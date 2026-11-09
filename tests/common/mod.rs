//! What the image tests share: comparing a render with its committed
//! image, with a tolerance, so float and rasteriser differences between
//! machines pass while moved, missing or restyled strokes don't.

#![allow(dead_code)]

use image::{Rgba, RgbaImage};
use std::path::Path;

/// A pixel matches another when no channel (composited on white) is off
/// by more than this...
pub const PIXEL_TOLERANCE: u8 = 32;
/// ...and differs when it matches nothing this many pixels or fewer away
/// in the other image, either way round. Floats come out a last bit
/// different on other machines and builds (their sin and cos differ), and
/// a few scripts' choices turn that into a shift of a few units, a pixel
/// or less here; strokes moved further, missing or restyled still differ.
pub const SHIFT_PX: i64 = 1;
/// An image differs when more than this fraction of its pixels do.
pub const MAX_DIFFERING: f64 = 0.0005;

/// Why `actual` doesn't match `expected`, if it doesn't; then a diff image
/// (`{name}.diff.png`) goes in `diff_dir`.
pub fn compare(expected: &RgbaImage, actual: &RgbaImage, diff_dir: &Path, name: &str) -> Option<String> {
    if expected.dimensions() != actual.dimensions() {
        return Some(format!("size changed from {:?} to {:?}", expected.dimensions(), actual.dimensions()));
    }
    let (width, height) = actual.dimensions();
    let (expected, actual) = (white_backed(expected), white_backed(actual));
    let at = |image: &[[u8; 3]], x: i64, y: i64| image[(y * width as i64 + x) as usize];
    let matches = |p: [u8; 3], q: [u8; 3]| (0..3).all(|c| p[c].abs_diff(q[c]) <= PIXEL_TOLERANCE);
    // Whether pixel (x, y) of `one` matches a pixel near it in `other`.
    let near = |one: &[[u8; 3]], other: &[[u8; 3]], x: i64, y: i64| {
        let p = at(one, x, y);
        (-SHIFT_PX..=SHIFT_PX).any(|dy| (-SHIFT_PX..=SHIFT_PX).any(|dx| {
            let (nx, ny) = (x + dx, y + dy);
            nx >= 0 && ny >= 0 && nx < width as i64 && ny < height as i64 && matches(p, at(other, nx, ny))
        }))
    };
    let mut diff = RgbaImage::new(width, height);
    let mut differing = 0usize;
    for (x, y, d) in diff.enumerate_pixels_mut() {
        let (x, y) = (x as i64, y as i64);
        let a = at(&actual, x, y);
        if !matches(at(&expected, x, y), a) && !(near(&actual, &expected, x, y) && near(&expected, &actual, x, y)) {
            differing += 1;
            *d = Rgba([255, 0, 0, 255]);
        } else {
            // The render, faded, to show where the changes are.
            let grey = ((a[0] as u32 + a[1] as u32 + a[2] as u32) / 3 / 4 + 191) as u8;
            *d = Rgba([grey, grey, grey, 255]);
        }
    }
    let fraction = differing as f64 / (width * height) as f64;
    if fraction <= MAX_DIFFERING {
        return None;
    }
    std::fs::create_dir_all(diff_dir).unwrap();
    diff.save(diff_dir.join(format!("{}.diff.png", name))).unwrap();
    Some(format!("{:.2}% of pixels differ (at most {:.2}% may)", fraction * 100.0, MAX_DIFFERING * 100.0))
}

/// The image's pixels composited on white, row by row.
pub fn white_backed(image: &RgbaImage) -> Vec<[u8; 3]> {
    image.pixels().map(|p| {
        let alpha = p[3] as u32;
        let blend = |c: u8| ((c as u32 * alpha + 255 * (255 - alpha)) / 255) as u8;
        [blend(p[0]), blend(p[1]), blend(p[2])]
    }).collect()
}

