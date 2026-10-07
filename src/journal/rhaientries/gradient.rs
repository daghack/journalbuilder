//! Gradients for rhai scripts: strokes whose colour changes along them or
//! across the page.
//!
//! ```rhai
//! // Along each stroke, from its start to its end: colours evenly spaced...
//! let fade = new_gradient(["#ff8a7a", "#e8c170", "#9ad0ff"]);
//! // ...or placed, as [position, colour] from 0 to 1 (mixing the two
//! // spaces the unplaced ones evenly between their neighbours).
//! let late = new_gradient(["gold", [0.8, "gold"], "red"]);
//! // Across the page, from one point to another...
//! let across = new_gradient(["red", "blue"], #{ from: new_vec(0.0, 0.0), to: new_vec(500.0, 0.0) });
//! // ...or out from a centre.
//! let glow = new_gradient(["white", "rgba(255, 200, 0, 0)"], #{ center: new_vec(0.0, 0.0), radius: 120.0 });
//!
//! let style = new_strokestyle(1.0, fade, brush);   // in place of a colour
//! style.color = across;                            // or set it later
//! ```
//!
//! In Darkly renders a gradient stroke is drawn in white and coloured
//! after, keeping its brush's texture; brushes that move ink (Smudge, Blur)
//! don't reach across it. In SVG output a gradient along a stroke is drawn
//! in short pieces, each its own colour.

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use super::layout::{check_keys, number};
use super::Vec2;
use crate::journal::canvas::parse_color;
use crate::journal::gradient::{Gradient, GradientKind};

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

const CALLER: &str = "new_gradient";

/// The colour stops: each a colour, or `[position, colour]`.
fn stops(colours: Array) -> RhaiResult<Vec<(f32, [f32; 4])>> {
    if colours.len() < 2 {
        return Err(format!("{}: needs at least two colours, got {}", CALLER, colours.len()).into());
    }
    let mut placed: Vec<(Option<f32>, [f32; 4])> = Vec::new();
    for (i, item) in colours.into_iter().enumerate() {
        let bad = |what: &str| -> Box<EvalAltResult> { format!("{}: colour {} {}", CALLER, i, what).into() };
        let (at, colour) = if item.is_array() {
            let pair = item.into_array().expect("checked");
            if pair.len() != 2 {
                return Err(bad("should be a colour or [position, colour]"));
            }
            let at = pair[0].as_float().map(|f| f as f32).or_else(|_| pair[0].as_int().map(|n| n as f32))
                .map_err(|_| bad("should have a number for its position"))?;
            if !(0.0..=1.0).contains(&at) {
                return Err(bad(&format!("is at {}; positions run from 0 to 1", at)));
            }
            (Some(at), pair[1].clone())
        } else {
            (None, item)
        };
        let colour = colour.into_immutable_string().map_err(|t| bad(&format!("is a {}, not a colour string", t)))?;
        let colour = parse_color(&colour).map_err(|e| -> Box<EvalAltResult> { format!("{}: colour {}: {}", CALLER, i, e).into() })?;
        placed.push((at, colour));
    }
    // Unplaced ends go at 0 and 1, and other unplaced stops evenly between
    // their placed neighbours.
    let last = placed.len() - 1;
    placed[0].0.get_or_insert(0.0);
    placed[last].0.get_or_insert(1.0);
    let mut i = 0;
    while i < last {
        let next = i + 1 + placed[i + 1..].iter().position(|(at, _)| at.is_some()).expect("the last is placed");
        let (a, b) = (placed[i].0.expect("placed"), placed[next].0.expect("placed"));
        if b < a {
            return Err(format!("{}: colour {} is at {}, before colour {} at {}; positions must not go back", CALLER, next, b, i, a).into());
        }
        for (k, stop) in placed[i + 1..next].iter_mut().enumerate() {
            stop.0 = Some(a + (b - a) * (k + 1) as f32 / (next - i) as f32);
        }
        i = next;
    }
    Ok(placed.into_iter().map(|(at, c)| (at.expect("placed"), c)).collect())
}

fn vec_option(options: &Map, key: &str) -> RhaiResult<Option<parry2d::math::Vec2>> {
    options.get(key).map(|v| {
        v.clone().try_cast::<Vec2>()
            .map(Into::into)
            .ok_or_else(|| -> Box<EvalAltResult> { format!("{}: '{}' should be a vec", CALLER, key).into() })
    }).transpose()
}

/// `new_gradient(colours[, #{ from, to } or #{ center, radius }])`.
pub fn new_gradient(colours: Array, options: Map) -> RhaiResult<Gradient> {
    check_keys(CALLER, &options, &["from", "to", "center", "radius"])?;
    let stops = stops(colours)?;
    let (from, to, center) = (vec_option(&options, "from")?, vec_option(&options, "to")?, vec_option(&options, "center")?);
    let radius = number(CALLER, &options, "radius")?;
    let kind = match (from, to, center, radius) {
        (None, None, None, None) => GradientKind::Along,
        (Some(from), Some(to), None, None) => {
            if from == to {
                return Err(format!("{}: 'from' and 'to' must be different points", CALLER).into());
            }
            GradientKind::Linear { from, to }
        }
        (None, None, Some(center), Some(radius)) => {
            if !(radius > 0.0 && radius.is_finite()) {
                return Err(format!("{}: 'radius' must be a positive number, got {}", CALLER, radius).into());
            }
            GradientKind::Radial { center, radius }
        }
        _ => return Err(format!("{}: give 'from' and 'to' (across the page), 'center' and 'radius' (out from a point), or neither (along each stroke)", CALLER).into()),
    };
    Ok(Gradient { stops, kind })
}

fn describe(g: &mut Gradient) -> String {
    let kind = match g.kind {
        GradientKind::Along => "along strokes".to_string(),
        GradientKind::Linear { from, to } => format!("from ({}, {}) to ({}, {})", from.x, from.y, to.x, to.y),
        GradientKind::Radial { center, radius } => format!("out from ({}, {}) to {}", center.x, center.y, radius),
    };
    format!("gradient of {} colours {}", g.stops.len(), kind)
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<Gradient>("Gradient")
        .register_fn("new_gradient", |colours: Array| new_gradient(colours, Map::new()))
        .register_fn("new_gradient", new_gradient)
        .register_fn("to_string", describe)
        .register_fn("to_debug", describe);
}

/// A colour or a gradient, as `new_strokestyle` and `style.color` take.
pub fn colour_or_gradient(caller: &str, value: Dynamic) -> RhaiResult<crate::journal::canvas::StrokeColor> {
    use crate::journal::canvas::StrokeColor;
    if value.is::<Gradient>() {
        return Ok(StrokeColor::Gradient(value.cast::<Gradient>()));
    }
    let type_name = value.type_name();
    let colour = value.into_immutable_string()
        .map_err(|_| -> Box<EvalAltResult> { format!("{}: expected a colour string or a gradient, got {}", caller, type_name).into() })?;
    parse_color(&colour).map_err(|e| -> Box<EvalAltResult> { format!("{}: {}", caller, e).into() })?;
    Ok(StrokeColor::Solid(colour.to_string()))
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::gradient::{Gradient, GradientKind};
    use crate::journal::symboldict::SymbolPathDict;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).unwrap()))
    }

    fn gradient(script: &str) -> Gradient {
        engine().engine.eval::<Gradient>(script).expect("script should run")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn stops_are_spaced_or_placed() {
        let g = gradient(r#"new_gradient(["red", "lime", "blue"])"#);
        assert_eq!(g.stops.iter().map(|s| s.0).collect::<Vec<_>>(), vec![0.0, 0.5, 1.0]);
        assert_eq!(g.stops[1].1, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(g.kind, GradientKind::Along);
        let g = gradient(r#"new_gradient(["red", [0.8, "red"], "white", "blue"])"#);
        let at: Vec<f32> = g.stops.iter().map(|s| s.0).collect();
        assert_eq!(at, vec![0.0, 0.8, 0.9, 1.0]);
    }

    #[test]
    fn options_lay_gradients_across_the_page() {
        let g = gradient(r#"new_gradient(["red", "blue"], #{ from: new_vec(0.0, 0.0), to: new_vec(10.0, 0.0) })"#);
        assert!(matches!(g.kind, GradientKind::Linear { .. }));
        let g = gradient(r#"new_gradient(["red", "blue"], #{ center: new_vec(1.0, 1.0), radius: 5 })"#);
        assert!(matches!(g.kind, GradientKind::Radial { radius: 5.0, .. }));
    }

    #[test]
    fn styles_take_gradients_as_colours() {
        let e = engine();
        let ok: bool = e.engine.eval(r#"
            let g = new_gradient(["red", "blue"]);
            let style = new_strokestyle(1.0, g, new_brush("Ink Pen", 0.4, 0.8, 1.6));
            let back = style.color;
            style.color = "gold";
            let solid = style.color == "gold";
            style.color = g;
            type_of(back) == "Gradient" && solid && type_of(style.color) == "Gradient"
        "#).unwrap();
        assert!(ok);
    }

    #[test]
    fn bad_gradients_fail_at_their_line() {
        for (script, want) in [
            (r#"new_gradient(["red"])"#, "needs at least two colours"),
            (r#"new_gradient(["red", "nope"])"#, "colour 1: Invalid color 'nope'"),
            (r#"new_gradient(["red", 3])"#, "colour 1 is a i64, not a colour string"),
            (r#"new_gradient([[0.6, "red"], [0.2, "blue"]])"#, "positions must not go back"),
            (r#"new_gradient([[1.5, "red"], "blue"])"#, "positions run from 0 to 1"),
            (r#"new_gradient(["red", "blue"], #{ from: new_vec(0.0, 0.0) })"#, "give 'from' and 'to'"),
            (r#"new_gradient(["red", "blue"], #{ center: new_vec(0.0, 0.0), radius: 0 })"#, "'radius' must be a positive"),
            (r#"new_gradient(["red", "blue"], #{ angle: 1 })"#, "unknown option 'angle'"),
            (r#"new_strokestyle(1.0, 3, new_brush("Ink Pen", 0.4, 0.8, 1.6))"#, "expected a colour string or a gradient"),
        ] {
            let err = fails(&format!("let ok = 1;\n{};", script));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
}
