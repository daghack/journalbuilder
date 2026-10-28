//! The cursive glyphset, ayan: every glyph, and every pair that joins, at
//! sizes from a footnote's to a heading's.

use super::*;
use crate::journal::symbol::joined_paths;
use crate::journal::typeset::{set_text, TextStyle};
use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn ayan() -> (FontBook, Font) {
    let book = FontBook::from_file(DEFAULT_GLYPHSETS).unwrap();
    let font = book.font("ayan").unwrap();
    (book, font)
}

/// The characters whose glyphs leave and those whose glyphs enter.
fn joining_characters(font: &Font) -> (Vec<String>, Vec<String>) {
    let joins = |c: &String| font.variants(c).unwrap()[0].joins();
    let chars = font.characters();
    (chars.iter().filter(|c| joins(c).is_some_and(|j| j.leaves)).cloned().collect(), chars.iter().filter(|c| joins(c).is_some_and(|j| j.enters)).cloned().collect())
}

/// The turns sharper than `angle` (radians) along a stroke, sampled every
/// `step`.
fn sharp_turns(path: &StrokePath, step: f32, angle: f32) -> usize {
    let Some(sub) = path.to_substrokes().into_iter().next() else { return 0 };
    let points: Vec<Vec2> = sub.get_point_samples(step).iter().map(|s| s.point).collect();
    points.windows(3).filter(|w| (w[1] - w[0]).angle_to(w[2] - w[1]).abs() > angle).count()
}

/// `a` then `b`, each variant of `a` with each variant of `b` it can be
/// followed by, set as a line of text at `size`.
fn pairs(font: &Font, a: &str, b: &str, size: f32) -> Vec<(usize, usize, Vec<Symbol>)> {
    let joins = font.glyphs.joins;
    let (va, bases_b) = (font.variants(a).unwrap(), font.glyphs.glyphs.get(b).unwrap());
    let mut out = Vec::new();
    for (i, first) in va.iter().enumerate() {
        let left_high = first.joins().filter(|j| j.leaves && joins).map(|j| j.exit_high);
        for second in font.joining(bases_b, left_high) {
            let j = bases_b.iter().position(|x| Rc::ptr_eq(x, second)).unwrap();
            let glyphs = vec![first.clone().with_skew(font.skew), font.symbol(b, second).with_skew(font.skew)];
            let chars: Vec<char> = format!("{}{}", a, b).chars().collect();
            let rect = Aabb::new(Vec2::ZERO, Vec2::new(size * 40.0, size * 40.0));
            let setting = set_text(rect, &TextStyle::new(size), &glyphs, &chars, false, None, &mut StdRng::seed_from_u64(1));
            out.push((i, j, setting.placed));
        }
    }
    out
}

/// Points along a path's strokes, every `step`.
fn samples(path: &StrokePath, step: f32) -> Vec<Vec2> {
    path.to_substrokes().iter().flat_map(|sub| sub.get_point_samples(step)).map(|s| s.point).collect()
}

/// How far the furthest of `a`'s points is from all of `b`'s.
fn furthest(a: &[Vec2], b: &[Vec2]) -> f32 {
    a.iter().map(|p| b.iter().map(|q| p.distance(*q)).fold(f32::MAX, f32::min)).fold(0.0, f32::max)
}

const SIZES: [f32; 4] = [8.0, 26.0, 90.0, 300.0];

#[test]
fn every_ayan_glyph_is_drawn_within_its_room() {
    let (_, font) = ayan();
    let chars = font.characters();
    assert!(chars.len() >= 78, "ayan has {} glyphs", chars.len());
    for c in chars {
        for (k, glyph) in font.variants(&c).unwrap().iter().enumerate() {
            let path = glyph.placed_strokepath();
            let points = path.points_within();
            assert!(!points.is_empty() && !path.subpaths().is_empty(), "'{}' variant {} draws nothing", c, k);
            for p in points {
                // Drawn as large as plain, its capitals' flourishes and
                // its loops reach past the box (a font size, -0.5 to 0.5
                // here, at scale 1), into the lines above and below, but
                // no further than a size each way, and not two sizes on.
                assert!(p.is_finite() && p.y.abs() <= 1.0 && (-1.0..=2.5).contains(&p.x), "'{}' variant {}: {:?} is outside its room", c, k, p);
            }
        }
    }
}

#[test]
fn every_pair_of_ayan_glyphs_that_joins_is_one_stroke_at_any_size() {
    let (_, font) = ayan();
    let (leave, enter) = joining_characters(&font);
    // Every lowercase letter enters, and all but f leave.
    assert_eq!((leave.len(), enter.len()), (25, 26));
    for a in &leave {
        for b in &enter {
            for size in SIZES {
                for (i, j, placed) in pairs(&font, a, b, size) {
                    let spans: Vec<_> = joined_paths(&placed, |_| false).into_iter().map(|(span, _)| span).collect();
                    assert_eq!(spans, [0..2], "'{}{}' (variants {} and {}) at {}", a, b, i, j, size);
                }
            }
        }
    }
}

#[test]
fn ayan_joins_are_the_same_at_any_size_short_and_turn_back_at_most_once() {
    let (_, font) = ayan();
    let (leave, enter) = joining_characters(&font);
    for a in &leave {
        for b in &enter {
            let at = |size: f32| pairs(&font, a, b, size).into_iter().map(|(i, j, placed)| (i, j, joined_paths(&placed, |_| false).remove(0).1, placed)).collect::<Vec<_>>();
            let unit: Vec<_> = at(1.0).into_iter().map(|(_, _, path, _)| samples(&path, 0.005)).collect();
            for size in SIZES {
                for ((i, j, path, placed), unit) in at(size).into_iter().zip(&unit) {
                    let what = format!("'{}{}' (variants {} and {}) at {}", a, b, i, j, size);
                    // Scaled down, it's the word at size 1, to within a
                    // twentieth of a glyph scale: two places to join at
                    // can cost nearly the same, and rounding picks either.
                    let scaled: Vec<Vec2> = samples(&path, size * 0.005).iter().map(|p| *p / size).collect();
                    let off = furthest(&scaled, unit).max(furthest(unit, &scaled));
                    assert!(off < 0.05, "{}: {} glyph scales from the word at size 1", what, off);
                    // No longer than the two glyphs' own strokes and a
                    // join as long as the furthest joined (2.6 of ayan's
                    // x-heights, a glyph scale).
                    let length = |p: &StrokePath| p.to_substrokes().first().map_or(0.0, |s| s.get_point_samples(size * 0.01).last().map_or(0.0, |s| s.distance));
                    let own: f32 = placed.iter().map(|g| length(&g.placed_strokepath().subpaths()[0])).sum();
                    assert!(length(&path) < own + size, "{}: the word's stroke is {} long, its glyphs' {}", what, length(&path) / size, own / size);
                    // The pen may turn back over its stroke where it
                    // enters a glyph that starts up and back (a, c, d, g,
                    // q, ...), but nowhere else.
                    let turns = |p: &StrokePath| sharp_turns(p, size * 0.04, 2.0);
                    let extra = turns(&path) as i64 - placed.iter().map(|g| turns(&g.placed_strokepath().subpaths()[0]) as i64).sum::<i64>();
                    assert!(extra <= 1, "{}: the join turns back {} times", what, extra);
                }
            }
        }
    }
}

#[test]
fn ayan_glyphs_after_b_and_o_are_entered_high() {
    let (book, font) = ayan();
    let (_, enter) = joining_characters(&font);
    let entries = |c: &str| font.variants(c).unwrap().iter().map(|g| g.joins().unwrap().entry_high).collect::<Vec<_>>();
    let both: Vec<&String> = enter.iter().filter(|c| entries(c).contains(&true) && entries(c).contains(&false)).collect();
    assert!(!both.is_empty(), "some glyphs are entered high and low");
    for c in both {
        for (before, high) in [("b", true), ("o", true), ("a", false), ("n", false)] {
            for seed in 0..8 {
                let glyphs = font.build(&format!("{}{}", before, c), &book, &mut StdRng::seed_from_u64(seed));
                assert_eq!(glyphs[1].joins().unwrap().entry_high, high, "'{}' after '{}'", c, before);
            }
        }
    }
}

