//! Colour jitter: each of a stroke's subpaths a little off its colour, as a
//! brush style's `color_jitter` says. Colours are moved in OKLab, where
//! equal steps look equal, whatever the colour.

/// A move in OKLab: lightness, then a and b (green-red and blue-yellow).
pub type Shift = [f32; 3];

/// How far a stroke's subpaths stray from its colour: lightness (OKLab's L,
/// from 0 to 1) by up to `amount` either way, and a and b (hue and chroma)
/// by up to half that times the colour's lightness, so dark inks don't turn
/// colourful; each subpath its own way, from `seed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorJitter {
    pub amount: f32,
    pub seed: u64,
}

impl ColorJitter {
    /// How subpath `index` (counting from 0 along its path) strays.
    pub fn shift(&self, index: usize) -> Shift {
        let mut z = self.seed ^ (index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut either_way = |reach: f32| {
            z = splitmix(z);
            // The top 24 bits, from -1 to 1.
            ((z >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * reach
        };
        [either_way(self.amount), either_way(self.amount / 2.0), either_way(self.amount / 2.0)]
    }

    /// `rgba` as subpath `index` is drawn in it.
    pub fn apply(&self, rgba: [f32; 4], index: usize) -> [f32; 4] {
        shifted(rgba, self.shift(index))
    }
}

/// SplitMix64's step: a well-mixed hash of `z`.
fn splitmix(z: u64) -> u64 {
    let mut z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// An sRGB RGBA colour (0-1) moved by `shift` in OKLab, a and b by its
/// share of the colour's lightness, its alpha kept, clamped back into sRGB.
pub fn shifted(rgba: [f32; 4], shift: Shift) -> [f32; 4] {
    if shift == [0.0; 3] {
        return rgba;
    }
    let [l, a, b] = oklab([rgba[0], rgba[1], rgba[2]]);
    let [r, g, b] = srgb([l + shift[0], a + shift[1] * l, b + shift[2] * l]);
    [r, g, b, rgba[3]]
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// sRGB (0-1) in OKLab (Björn Ottosson's).
fn oklab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(to_linear);
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab back in sRGB (0-1), clamped.
fn srgb(lab: [f32; 3]) -> [f32; 3] {
    let [l, a, b] = lab;
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ].map(from_linear)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_goes_there_and_back() {
        for rgb in [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.17, 0.25, 0.52], [0.61, 0.14, 0.09], [0.9, 0.8, 0.1]] {
            let back = srgb(oklab(rgb));
            for (c, b) in rgb.iter().zip(back) {
                assert!((c - b).abs() < 1e-4, "{:?} came back as {:?}", rgb, back);
            }
        }
        let [l, _, _] = oklab([1.0, 1.0, 1.0]);
        assert!((l - 1.0).abs() < 1e-3, "white is lightness 1");
    }

    #[test]
    fn shifts_stay_within_reach_and_differ_by_subpath() {
        let jitter = ColorJitter { amount: 0.1, seed: 7 };
        let shifts: Vec<Shift> = (0..200).map(|i| jitter.shift(i)).collect();
        assert!(shifts.iter().all(|[l, a, b]| l.abs() <= 0.1 && a.abs() <= 0.05 && b.abs() <= 0.05));
        assert!(shifts.iter().any(|s| s[0] > 0.07) && shifts.iter().any(|s| s[0] < -0.07), "both ways, most of the way");
        assert_ne!(shifts[0], shifts[1]);
        assert_eq!(jitter.shift(3), ColorJitter { amount: 0.1, seed: 7 }.shift(3), "the same every time");
        assert_ne!(jitter.shift(3), ColorJitter { amount: 0.1, seed: 8 }.shift(3));
    }

    #[test]
    fn no_jitter_keeps_the_colour_and_alpha_is_kept() {
        let navy = [0.17, 0.25, 0.52, 0.4];
        assert_eq!(ColorJitter { amount: 0.0, seed: 3 }.apply(navy, 5), navy);
        let moved = shifted(navy, [0.05, 0.0, 0.0]);
        assert!(moved[0] > navy[0] && moved[1] > navy[1] && moved[2] > navy[2], "lighter: {:?}", moved);
        assert_eq!(moved[3], 0.4);
        let white = shifted([1.0, 1.0, 1.0, 1.0], [0.2, 0.0, 0.0]);
        assert!(white.iter().all(|c| (c - 1.0).abs() < 1e-5), "clamped to sRGB: {:?}", white);
    }
}
