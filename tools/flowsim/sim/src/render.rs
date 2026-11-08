//! The field drawn as an image, one pixel a cell, for the page to scale up.

use crate::colour::mixed;
use crate::grid::Grid;
use crate::solver::Tint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Dye,
    Speed,
    Vorticity,
    Pressure,
    Plain,
}

impl View {
    pub fn parse(name: &str) -> View {
        match name {
            "speed" => View::Speed,
            "vorticity" => View::Vorticity,
            "pressure" => View::Pressure,
            "plain" | "none" => View::Plain,
            _ => View::Dye,
        }
    }
}

type Rgb = [f32; 3];

const WATER: Rgb = [14.0, 26.0, 38.0];
const SOLID: Rgb = [62.0, 68.0, 78.0];
const DYE: [(f32, Rgb); 4] =
    [(0.0, WATER), (0.35, [34.0, 92.0, 116.0]), (0.7, [108.0, 186.0, 196.0]), (1.0, [232.0, 244.0, 240.0])];
const SPEED: [(f32, Rgb); 5] = [
    (0.0, WATER),
    (0.25, [38.0, 66.0, 136.0]),
    (0.5, [36.0, 150.0, 156.0]),
    (0.75, [196.0, 216.0, 96.0]),
    (1.0, [255.0, 240.0, 200.0]),
];
const DIVERGING: [(f32, Rgb); 5] = [
    (-1.0, [176.0, 214.0, 255.0]),
    (-0.5, [46.0, 104.0, 206.0]),
    (0.0, WATER),
    (0.5, [206.0, 76.0, 58.0]),
    (1.0, [255.0, 204.0, 172.0]),
];

fn ramp(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    let t = t.clamp(stops[0].0, stops[stops.len() - 1].0);
    for w in stops.windows(2) {
        let (a, ca) = w[0];
        let (b, cb) = w[1];
        if t <= b {
            let f = if b > a { (t - a) / (b - a) } else { 0.0 };
            return [ca[0] + (cb[0] - ca[0]) * f, ca[1] + (cb[1] - ca[1]) * f, ca[2] + (cb[2] - ca[2]) * f];
        }
    }
    stops[stops.len() - 1].1
}

/// Three times the root mean square of the fluid cells' values: a scale
/// that outliers by obstacles don't wash out.
fn spread(values: &[f32], solid: &[bool]) -> f32 {
    let (mut sum, mut n) = (0.0f64, 0usize);
    for (v, s) in values.iter().zip(solid) {
        if !s {
            sum += (*v as f64) * (*v as f64);
            n += 1;
        }
    }
    if n == 0 { 1.0 } else { (3.0 * (sum / n as f64).sqrt()).max(1e-6) as f32 }
}

/// Dye of mixed colours over the water: its colour, faint where there's
/// little, with a sheen where it's thick.
fn tinted(tint: &Tint, c: usize, amount: f32) -> Rgb {
    if amount < 2e-3 {
        return WATER;
    }
    let ch = &tint.channels;
    let rgb = mixed([ch[0][c], ch[1][c], ch[2][c], ch[3][c]]).map(|x| x * 255.0);
    let x = (amount * 1.5).min(1.0);
    let t = x * (2.0 - x);
    let sheen = ((amount - 0.7) / 0.3).clamp(0.0, 1.0) * 0.3;
    std::array::from_fn(|i| {
        let over = WATER[i] + (rgb[i] - WATER[i]) * t;
        over + (255.0 - over) * sheen
    })
}

/// Fills `out` with RGBA, `nx` × `ny`. `reference_speed` is what counts as
/// fast in the speed view; `tint` is the dye's colour, if it's coloured.
pub fn render(g: &Grid, tint: Option<&Tint>, view: View, reference_speed: f32, scratch: &mut Vec<f32>, out: &mut Vec<u8>) {
    let (nx, ny) = (g.nx, g.ny);
    out.clear();
    out.resize(nx * ny * 4, 255);
    match view {
        View::Vorticity => {
            g.vorticity(scratch);
        }
        View::Pressure => {
            scratch.clear();
            scratch.extend_from_slice(&g.q);
        }
        View::Speed => {
            scratch.clear();
            for j in 0..ny {
                for i in 0..nx {
                    scratch.push(g.cell_velocity(i, j).len());
                }
            }
        }
        _ => {}
    }
    let scale = match view {
        View::Vorticity | View::Pressure => spread(scratch, &g.solid),
        _ => reference_speed.max(1.0) * 1.5,
    };
    for c in 0..nx * ny {
        let rgb = if g.solid[c] {
            SOLID
        } else {
            match view {
                View::Dye => match tint {
                    Some(t) => tinted(t, c, g.dye[c]),
                    None => ramp(&DYE, g.dye[c]),
                },
                View::Speed => ramp(&SPEED, scratch[c] / scale),
                View::Vorticity | View::Pressure => ramp(&DIVERGING, scratch[c] / scale),
                View::Plain => WATER,
            }
        };
        out[4 * c] = rgb[0] as u8;
        out[4 * c + 1] = rgb[1] as u8;
        out[4 * c + 2] = rgb[2] as u8;
    }
}
