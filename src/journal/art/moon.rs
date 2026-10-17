//! The moon in a phase: its disc, the terminator between its lit and dark
//! sides, and those sides as closed shapes, to hatch or fill.
//!
//! The phase runs from 0 (new) through 0.25 (first quarter, the right half
//! lit, as seen from the northern hemisphere), 0.5 (full) and 0.75 (last
//! quarter, the left half lit) back to new at 1; it wraps, so 1.25 is a
//! first quarter again. The terminator is half an ellipse from the top of
//! the disc to the bottom.

use std::f32::consts::TAU;

use parry2d::math::Vec2;

use crate::journal::paths::StrokePath;

pub struct Moon {
    /// The whole disc's outline.
    pub outline: StrokePath,
    /// The line between the lit and dark sides (none at new or full).
    pub terminator: StrokePath,
    /// The lit side, closed (empty at new moon).
    pub lit: StrokePath,
    /// The dark side, closed (empty at full moon).
    pub dark: StrokePath,
}

/// How close to new or full a phase draws as all dark or all lit, as a
/// fraction of the disc lit.
const WHOLE: f32 = 1e-3;

pub fn moon(center: Vec2, radius: f32, phase: f32) -> Result<Moon, String> {
    if !(radius > 0.0 && radius.is_finite()) {
        return Err(format!("radius must be a positive number, got {}", radius));
    }
    if !phase.is_finite() || !center.x.is_finite() || !center.y.is_finite() {
        return Err("the centre and phase must be finite".into());
    }
    let phase = phase.rem_euclid(1.0);
    let lit_fraction = (1.0 - (TAU * phase).cos()) / 2.0;
    // The lit limb's side: right while waxing, left while waning.
    let side = if phase < 0.5 { 1.0 } else { -1.0 };
    // The terminator's middle, from -1 (the dark limb) to 1 (the lit limb),
    // times the lit limb's side.
    let bulge = side * (TAU * phase).cos();
    let limb = |x: f32| half(center, radius, x, true);
    let terminator = half(center, radius, bulge, false);
    let closed = |limb_side: f32| {
        let mut path = limb(limb_side);
        path.append(terminator.clone());
        // The terminator starts where the limb ends, so it carries on.
        StrokePath::from_commands(skip_second_move(path.commands())).close_path()
    };
    let (lit, dark) = if lit_fraction < WHOLE {
        (StrokePath::new(), StrokePath::circle(center, radius))
    } else if lit_fraction > 1.0 - WHOLE {
        (StrokePath::circle(center, radius), StrokePath::new())
    } else {
        (closed(side), closed(-side))
    };
    let terminator = if lit.commands().is_empty() || dark.commands().is_empty() { StrokePath::new() } else { half(center, radius, bulge, true) };
    Ok(Moon { outline: StrokePath::circle(center, radius), terminator, lit, dark })
}

/// Half an ellipse from the top of the disc to the bottom (or the bottom
/// to the top), reaching `x` radii across at its middle: 1 the right limb,
/// -1 the left, 0 straight down the middle.
fn half(center: Vec2, radius: f32, x: f32, down: bool) -> StrokePath {
    // A quarter circle's control points, as a cubic.
    const K: f32 = 0.552_284_8;
    let p = |px: f32, py: f32| center + Vec2::new(px * x * radius, py * radius);
    let (top, mid, bottom) = (p(0.0, -1.0), p(1.0, 0.0), p(0.0, 1.0));
    let (a1, a2, b1, b2) = (p(K, -1.0), p(1.0, -K), p(1.0, K), p(K, 1.0));
    let path = StrokePath::new();
    if down {
        path.move_to(top.x, top.y)
            .cubic_curve_to(a1.x, a1.y, a2.x, a2.y, mid.x, mid.y)
            .cubic_curve_to(b1.x, b1.y, b2.x, b2.y, bottom.x, bottom.y)
    } else {
        path.move_to(bottom.x, bottom.y)
            .cubic_curve_to(b2.x, b2.y, b1.x, b1.y, mid.x, mid.y)
            .cubic_curve_to(a2.x, a2.y, a1.x, a1.y, top.x, top.y)
    }
}

/// Commands with every move after the first left out, joining subpaths
/// that start where the last ended into one.
fn skip_second_move(commands: &[crate::journal::paths::PathCmd]) -> Vec<crate::journal::paths::PathCmd> {
    use crate::journal::paths::PathCmd;
    commands.iter().enumerate()
        .filter(|(i, c)| *i == 0 || !matches!(c, PathCmd::MoveTo(..)))
        .map(|(_, c)| *c)
        .collect()
}
