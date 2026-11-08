//! Dye colours: `#rrggbb` as the scene writes them, and OKLab, which dye
//! mixes in. Mixed dye keeps the average hue of what went in and the
//! average chroma, so two colours meeting stay vivid rather than going
//! grey as paint would.

/// sRGB, each 0..1.
pub type Rgb = [f32; 3];
/// OKLab: lightness 0..1, and the a and b axes (about −0.4..0.4).
pub type Lab = [f32; 3];

/// Dye with no colour of its own: a light sea-green, the dye view's own.
pub const DEFAULT_DYE: Rgb = [0.47, 0.84, 0.86];

/// `#rrggbb` (or `rrggbb`, or `#rgb`) as sRGB.
pub fn parse_hex(text: &str) -> Option<Rgb> {
    let hex = text.trim().trim_start_matches('#');
    let digit = |i: usize, n: usize| u8::from_str_radix(hex.get(i..i + n)?, 16).ok();
    let [r, g, b] = match hex.len() {
        6 => [digit(0, 2)?, digit(2, 2)?, digit(4, 2)?],
        3 => [digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17],
        _ => return None,
    };
    Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f32) -> f32 {
    if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

pub fn oklab(rgb: Rgb) -> Lab {
    let [r, g, b] = rgb.map(to_linear);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn linear_rgb(lab: Lab) -> Rgb {
    let [l, a, b] = lab;
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
}

/// OKLab as sRGB, its chroma brought in until it fits.
pub fn srgb(lab: Lab) -> Rgb {
    let fits = |c: Rgb| c.iter().all(|x| (-1e-3..=1.001).contains(x));
    let mut rgb = linear_rgb(lab);
    if !fits(rgb) {
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..6 {
            let mid = 0.5 * (lo + hi);
            if fits(linear_rgb([lab[0], lab[1] * mid, lab[2] * mid])) { lo = mid } else { hi = mid }
        }
        rgb = linear_rgb([lab[0], lab[1] * lo, lab[2] * lo]);
    }
    rgb.map(|c| from_linear(c.clamp(0.0, 1.0)))
}

/// What a dye colour adds to a cell's mix, a unit of dye's worth:
/// lightness, a, b, and chroma (|a, b|), which mixing keeps apart from a
/// and b so that hues meeting don't cancel to grey.
pub fn dye_channels(rgb: Rgb) -> [f32; 4] {
    let [l, a, b] = oklab(rgb);
    [l, a, b, (a * a + b * b).sqrt()]
}

/// sRGB's gamma, 0..1 linear in 1024 steps.
fn gamma_table() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| (0..=1024).map(|i| from_linear(i as f32 / 1024.0)).collect())
}

/// Mixed dye's colour as sRGB, quickly, for drawing: its mean dye channels
/// (as `dye_channels` gives them). The mean hue at nearly the mean chroma,
/// so colours meeting stay bright (hues that all but cancel grey a
/// little).
pub fn mixed(channels: [f32; 4]) -> Rgb {
    let [l, a, b, c] = channels;
    let l = l.clamp(0.0, 1.0);
    let m = (a * a + b * b).sqrt();
    let c = c.max(m);
    let (a, b) = if m > 1e-5 {
        let chroma = c * (m / c).sqrt().sqrt();
        (a / m * chroma, b / m * chroma)
    } else {
        (0.0, 0.0)
    };
    let rgb = linear_rgb([l, a, b]);
    let grey = l * l * l;
    // Below black, toward the grey; past white, clipped, which keeps it
    // bright.
    let mut t = 1.0f32;
    for x in rgb {
        if x < 0.0 {
            t = t.min(grey / (grey - x));
        }
    }
    let table = gamma_table();
    rgb.map(|x| {
        let f = ((grey + (x - grey) * t).clamp(0.0, 1.0)) * 1024.0;
        let i = (f as usize).min(1023);
        table[i] + (table[i + 1] - table[i]) * (f - i as f32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for rgb in [[1.0, 0.0, 0.0], [0.2, 0.6, 0.9], [1.0, 1.0, 1.0], DEFAULT_DYE] {
            let back = srgb(oklab(rgb));
            for (x, y) in rgb.iter().zip(back) {
                assert!((x - y).abs() < 2e-3, "{rgb:?} came back as {back:?}");
            }
        }
        assert_eq!(parse_hex("#ff8000"), Some([1.0, 128.0 / 255.0, 0.0]));
        assert_eq!(parse_hex("#f00"), Some([1.0, 0.0, 0.0]));
        assert_eq!(parse_hex("nope"), None);
    }

    #[test]
    fn mixing_keeps_chroma() {
        let red = dye_channels([1.0, 0.1, 0.1]);
        let yellow = dye_channels([1.0, 0.9, 0.1]);
        let mix: [f32; 4] = std::array::from_fn(|i| (red[i] + yellow[i]) / 2.0);
        let rgb = mixed(mix);
        // An orange, not a brown.
        assert!(rgb[0] > 0.9 && rgb[1] > 0.35 && rgb[1] < 0.8 && rgb[2] < 0.3, "{rgb:?}");
    }
}
