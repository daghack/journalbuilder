//! The paths written out as SVG.
//!
//! Path data uses only absolute `M`, `L`, `C` and `Z`, with coordinates in
//! domain units rounded to hundredths: the subset journals' path parser
//! reads too.

use crate::geom::{V2, v2};
use crate::objects::{Kind, Object};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Px,
    Mm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgOptions {
    /// The SVG's width, in `unit`; its height follows the domain's shape.
    pub width: f32,
    pub unit: Unit,
    pub stroke: String,
    /// In `unit`.
    pub stroke_width: f32,
    /// How far (domain units) a simplified path may stray from the traced
    /// one. 0 keeps every point.
    pub simplify: f32,
    /// Curves through the points rather than straight lines between them.
    pub smooth: bool,
    /// Shorter paths (domain units) are left out.
    pub min_length: f32,
    /// Which paths: those recorded, and those the seeds trace now.
    pub recorded: bool,
    pub live: bool,
    /// Outlines of the objects, each family in its own group.
    pub obstacles: bool,
    pub stirrers: bool,
    pub attractors: bool,
    pub seeds: bool,
    /// A fill behind everything; none when empty.
    pub background: String,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            width: 297.0,
            unit: Unit::Mm,
            stroke: "#1a1a1a".into(),
            stroke_width: 0.35,
            simplify: 0.4,
            smooth: true,
            min_length: 10.0,
            recorded: true,
            live: true,
            obstacles: false,
            stirrers: false,
            attractors: false,
            seeds: false,
            background: String::new(),
        }
    }
}

/// A polyline to write, and whether it closes.
pub struct Line<'a> {
    pub points: &'a [V2],
    pub closed: bool,
}

pub fn write_svg(width: f32, height: f32, objects: &[Object], lines: &[Line], o: &SvgOptions) -> String {
    let unit = match o.unit {
        Unit::Px => "px",
        Unit::Mm => "mm",
    };
    let out_w = if o.width > 0.0 { o.width } else { width };
    let out_h = out_w * height / width.max(1e-6);
    // Domain units per output unit.
    let k = width / out_w.max(1e-6);
    let sw = o.stroke_width.max(0.0) * k;
    let stroke = escape(&o.stroke);
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}{unit}" height="{}{unit}" viewBox="0 0 {} {}">"#,
        num(out_w),
        num(out_h),
        num(width),
        num(height)
    );
    if !o.background.trim().is_empty() {
        let _ =
            writeln!(s, r#"  <rect width="{}" height="{}" fill="{}"/>"#, num(width), num(height), escape(&o.background));
    }
    let faint = format!(r#"fill="none" stroke="{stroke}" stroke-width="{}" opacity="0.5""#, num(sw * 0.75));
    if o.obstacles {
        let _ = writeln!(s, r#"  <g id="obstacles" {faint}>"#);
        for obj in objects {
            match &obj.kind {
                Kind::Pebble { pos, radius } => circle(&mut s, *pos, *radius),
                Kind::Rect { pos, size, angle } => {
                    let a = angle.to_radians();
                    let half = *size * 0.5;
                    let corners: Vec<V2> = [v2(-1.0, -1.0), v2(1.0, -1.0), v2(1.0, 1.0), v2(-1.0, 1.0)]
                        .iter()
                        .map(|c| *pos + v2(c.x * half.x, c.y * half.y).rotated(a))
                        .collect();
                    let _ = writeln!(s, r#"    <path d="{}"/>"#, polyline_data(&corners, true));
                }
                Kind::Wall { points, thickness } if !points.is_empty() => {
                    let _ = writeln!(
                        s,
                        r#"    <path d="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                        polyline_data(points, false),
                        num(*thickness)
                    );
                }
                _ => {}
            }
        }
        s.push_str("  </g>\n");
    }
    if o.stirrers {
        let _ = writeln!(s, r#"  <g id="stirrers" {faint}>"#);
        for obj in objects {
            match obj.kind {
                Kind::Jet { pos, angle, radius, .. } => {
                    circle(&mut s, pos, radius);
                    let tip = pos + v2(1.0, 0.0).rotated(angle.to_radians()) * (radius * 1.6);
                    let _ = writeln!(s, r#"    <path d="{}"/>"#, polyline_data(&[pos, tip], false));
                }
                Kind::Vortex { pos, radius, .. } | Kind::Source { pos, radius, .. } => circle(&mut s, pos, radius),
                _ => {}
            }
        }
        s.push_str("  </g>\n");
    }
    if o.attractors {
        let _ = writeln!(s, r#"  <g id="attractors" {faint}>"#);
        for obj in objects {
            if let Kind::Attractor { pos, radius, capture, .. } = obj.kind {
                circle(&mut s, pos, radius);
                if capture > 0.0 {
                    circle(&mut s, pos, capture);
                }
            }
        }
        s.push_str("  </g>\n");
    }
    if o.seeds {
        let _ = writeln!(s, r#"  <g id="seeds" {faint}>"#);
        for obj in objects {
            match obj.kind {
                Kind::SeedLine { a, b, .. } => {
                    let _ = writeln!(s, r#"    <path d="{}"/>"#, polyline_data(&[a, b], false));
                }
                Kind::SeedRing { pos, radius, .. } => circle(&mut s, pos, radius),
                Kind::SeedPoint { pos, .. } => circle(&mut s, pos, sw.max(1.0) * 2.0),
                _ => {}
            }
        }
        s.push_str("  </g>\n");
    }
    let _ = writeln!(
        s,
        r#"  <g id="paths" fill="none" stroke="{stroke}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round">"#,
        num(sw)
    );
    for line in lines {
        if line.points.len() < 2 || length(line.points) < o.min_length {
            continue;
        }
        let d = path_data(line.points, line.closed, o.simplify, o.smooth);
        if !d.is_empty() {
            let _ = writeln!(s, r#"    <path d="{d}"/>"#);
        }
    }
    s.push_str("  </g>\n</svg>\n");
    s
}

fn circle(s: &mut String, c: V2, r: f32) {
    let _ = writeln!(s, r#"    <circle cx="{}" cy="{}" r="{}"/>"#, num(c.x), num(c.y), num(r.abs()));
}

pub fn length(points: &[V2]) -> f32 {
    points.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// A path's data: simplified within `tolerance`, then straight or smoothed.
pub fn path_data(points: &[V2], closed: bool, tolerance: f32, smooth: bool) -> String {
    let mut pts = simplify(points, tolerance);
    if closed && pts.len() > 2 && pts[0].dist(*pts.last().unwrap()) < 1e-3 {
        pts.pop();
    }
    let closed = closed && pts.len() > 2;
    if smooth && pts.len() > 2 { curve_data(&pts, closed) } else { polyline_data(&pts, closed) }
}

pub fn polyline_data(points: &[V2], closed: bool) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        let _ = write!(d, "{}{} {}", if i == 0 { "M " } else { " L " }, num(p.x), num(p.y));
    }
    if closed && points.len() > 2 {
        d.push_str(" Z");
    }
    d
}

/// Catmull-Rom curves through the points, as cubics.
fn curve_data(p: &[V2], closed: bool) -> String {
    let n = p.len();
    let at = |i: isize| -> V2 {
        if closed {
            p[i.rem_euclid(n as isize) as usize]
        } else {
            p[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let mut d = format!("M {} {}", num(p[0].x), num(p[0].y));
    let segments = if closed { n } else { n - 1 };
    for i in 0..segments as isize {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c1 = p1 + (p2 - p0) / 6.0;
        let c2 = p2 - (p3 - p1) / 6.0;
        let _ =
            write!(d, " C {} {} {} {} {} {}", num(c1.x), num(c1.y), num(c2.x), num(c2.y), num(p2.x), num(p2.y));
    }
    if closed {
        d.push_str(" Z");
    }
    d
}

/// Ramer–Douglas–Peucker: the fewest of the points that keep every one
/// within `tolerance` of the result.
pub fn simplify(points: &[V2], tolerance: f32) -> Vec<V2> {
    if points.len() < 3 || tolerance <= 0.0 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let mut worst = 0.0;
        let mut at = a;
        for (i, p) in points.iter().enumerate().take(b).skip(a + 1) {
            let d = crate::geom::segment_distance(*p, points[a], points[b]);
            if d > worst {
                worst = d;
                at = i;
            }
        }
        if worst > tolerance {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// A number rounded to hundredths, without trailing zeros.
pub fn num(x: f32) -> String {
    let r = (x as f64 * 100.0).round() / 100.0;
    let mut s = format!("{r:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" { "0".into() } else { s }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
