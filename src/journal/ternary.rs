//! Balanced-ternary digits written along a path, as the `balanced_ternary`
//! font writes them down its stems: each digit is a mark leaving the path.
//! A 0 is a flat bar across the path and a -1 or +1 a stroke that runs
//! alongside the path, then bends sharply out to its left or right.
//!
//! Left and right are the traveller's, going from the path's start to its
//! end: for a path running left to right across the page, +1 is down.
//! Each digit's mark reaches out from the path at the middle of its slot,
//! so bars and hook tips are evenly spaced; a hook's run alongside the
//! path comes before that.

use parry2d::math::Vec2;

use super::paths::{CurveSample, StrokePath, Walk};

const SAMPLES_PER_CURVE: usize = 20;

#[derive(Debug, Clone, Default)]
pub struct TernaryOptions {
    /// The distance along the path from one digit to the next; by default
    /// the digits share the whole path.
    pub spacing: Option<f32>,
    /// How far marks reach out from the path; by default 0.6 of the spacing.
    pub width: Option<f32>,
}

/// The marks for `digits` (each -1, 0 or 1, first at the path's start)
/// along a path of one subpath, as a new path; the path itself isn't in it.
pub fn ternary_along(path: &StrokePath, digits: &[i8], options: &TernaryOptions) -> Result<StrokePath, String> {
    let substrokes = path.to_substrokes();
    let [substroke] = substrokes.as_slice() else {
        return Err(format!("the path should be one line (one subpath), not {}", substrokes.len()));
    };
    let walk = Walk::new(substroke, SAMPLES_PER_CURVE);
    let length = walk.length();
    if digits.is_empty() || !(length > 0.0) {
        return Ok(StrokePath::new());
    }
    let n = digits.len() as f32;
    let spacing = options.spacing.unwrap_or(length / n);
    if spacing * n > length * 1.0001 {
        return Err(format!(
            "{} digits {} apart need a path {} long, but it's {} long",
            digits.len(), spacing, spacing * n, length,
        ));
    }
    let width = options.width.unwrap_or(spacing * 0.6);
    let at = |d: f32| walk.at(d.clamp(0.0, length)).expect("clamped onto the path");
    let mut out = StrokePath::new();
    for (k, &digit) in digits.iter().enumerate() {
        out.append(hook(&at, (k as f32 + 0.5) * spacing, spacing, width, digit));
    }
    Ok(out)
}

/// The side +1 marks go to: the traveller's right (the normal turns the
/// direction of travel clockwise on the page, y down).
fn plus_side(s: &CurveSample) -> Vec2 {
    s.normal
}

/// A digit's hook (or bar) leaving the path at `centre`: a bar crosses
/// there, and a hook hugs the path for most of a spacing before it, then
/// turns out there.
fn hook(at: &impl Fn(f32) -> CurveSample, centre: f32, spacing: f32, width: f32, digit: i8) -> StrokePath {
    let end = at(centre);
    if digit == 0 {
        let (a, b) = (end.point - plus_side(&end) * width, end.point + plus_side(&end) * width);
        return StrokePath::new().move_to(a.x, a.y).line_to(b.x, b.y);
    }
    let start = at(centre - 0.9 * spacing);
    let bend = at(centre - 0.18 * spacing);
    let tip = end.point + plus_side(&end) * (digit as f32 * width);
    StrokePath::new()
        .move_to(start.point.x, start.point.y)
        .cubic_curve_to(bend.point.x, bend.point.y, end.point.x, end.point.y, tip.x, tip.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(length: f32) -> StrokePath {
        StrokePath::line(Vec2::new(0.0, 0.0), Vec2::new(0.0, length))
    }

    fn xs(path: &StrokePath) -> Vec<f32> {
        path.points_within().iter().map(|p| p.x).collect()
    }

    #[test]
    fn hooks_turn_to_the_travellers_side_and_bars_cross() {
        let opts = TernaryOptions::default();
        // Down the page, the traveller's right, +1, is the page's left.
        let plus = ternary_along(&down(30.0), &[1], &opts).unwrap();
        assert!(xs(&plus).iter().cloned().fold(f32::MAX, f32::min) < -14.0);
        assert!(xs(&plus).iter().all(|&x| x < 0.01));
        let minus = ternary_along(&down(30.0), &[-1], &opts).unwrap();
        assert!(xs(&minus).iter().all(|&x| x > -0.01));
        let bar = ternary_along(&down(30.0), &[0], &opts).unwrap();
        let bx = xs(&bar);
        assert!(bx.iter().any(|&x| x < -14.0) && bx.iter().any(|&x| x > 14.0));
        assert_eq!(ternary_along(&down(30.0), &[1, 0, -1], &opts).unwrap().to_substrokes().len(), 3);
    }

    #[test]
    fn bars_and_hook_tips_line_up() {
        // Across the page: where each mark reaches furthest out is at the
        // middle of its slot, 5, 15 and 25 along.
        let across = StrokePath::line(Vec2::new(0.0, 0.0), Vec2::new(30.0, 0.0));
        let path = ternary_along(&across, &[1, 0, -1], &TernaryOptions::default()).unwrap();
        let tips: Vec<f32> = path.flattened(0.01).iter()
            .map(|(points, _)| points.iter().max_by(|a, b| a.y.abs().total_cmp(&b.y.abs())).unwrap().x)
            .collect();
        assert_eq!(tips.len(), 3, "tips at {:?}", tips);
        for (tip, want) in tips.iter().zip([5.0, 15.0, 25.0]) {
            assert!((tip - want).abs() < 0.3, "tips at {:?}", tips);
        }
    }

    #[test]
    fn digits_must_fit() {
        let opts = TernaryOptions { spacing: Some(20.0), ..TernaryOptions::default() };
        assert!(ternary_along(&down(30.0), &[1, 1], &opts).unwrap_err().contains("need a path 40 long"));
        let two = down(10.0).merge_path(&down(5.0));
        assert!(ternary_along(&two, &[1], &TernaryOptions::default()).is_err());
    }
}
