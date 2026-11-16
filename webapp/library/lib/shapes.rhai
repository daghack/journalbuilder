// Shapes to draw with: arrays built from a function (steps, times) and
// paths joined from them (join); smooth curves through points, arcs,
// ellipses and whorls as points; closed shapes wound one way, so shapes
// added together merge rather than cancel, and rings; curves' edges, tubes
// and slivers along a curve; and the pointed arches, lancets and foils of
// gothic tracery.
// import "lib/shapes" as shapes;
//
//   let ring = shapes::closed(shapes::ellipse_points(vec2(600, 800), 200, 120, 60));
//   let limb = shapes::closed(shapes::tube([vec2(0, 0), vec2(80, 40), vec2(160, 20)], [20, 14, 6]));
//   let wave = new_polyline(shapes::steps(40, |j| vec2(j * 20.0, 10.0 * (j * 0.4).sin())));
//   let rays = shapes::join(shapes::times(12, |k| new_line(vec2(600, 800), vec2(600, 800) + shapes::polar(k * 0.52) * 90)));
//   page.add(shapes::behind(wave, ring), ink);

// ---- arrays from a function ----

// f runs inside lib/shapes, so a variable it captures named like one of
// its functions (side, tube, curve, ring, ...) reads as the function:
// name those otherwise. An array's own map has no such catch.

// [f(0), f(1), ..., f(n)]: n + 1 values, as a span sampled n times wants.
fn steps(n, f) {
	let out = [];
	for j in 0..=n { out.push(f.call(j)); }
	out
}

// [f(0), ..., f(n - 1)]: n values, as n things or a loop of n points want.
fn times(n, f) {
	let out = [];
	for j in 0..n { out.push(f.call(j)); }
	out
}

// One path of the paths in `items`, in order: what a loop appending to a
// new path builds. shapes::join(shapes::times(n, |k| new_line(...))).
fn join(items) {
	let out = new_path();
	for p in items { out.append(p); }
	out
}

// The unit vec at angle a: 0 along +x, and with y down, turning clockwise.
fn polar(a) { vec2(a.cos(), a.sin()) }

// `points` in reverse order, as a new array.
fn reversed(points) {
	let out = points;
	out.reverse();
	out
}

// The middle of cell n of a square grid `cols` across, numbered row by
// row from 0 at the top left, the grid's middle at `centre`, `cell` its
// cells' size.
fn cell_centre(n, cols, centre, cell) {
	centre + vec2((n % cols - (cols - 1) / 2.0) * cell, (n / cols - (cols - 1) / 2.0) * cell)
}

// ---- closed shapes ----

// A closed shape through `points`.
fn shape(points) { new_polyline(points, #{ closed: true }) }

// `points` turned to run clockwise on the page (y down), so shapes added
// together merge rather than cancel where they overlap.
fn wound(points) {
	let area = 0.0;
	let n = points.len();
	for k in 0..n {
		let a = points[k];
		let b = points[(k + 1) % n];
		area += a.x * b.y - b.x * a.y;
	}
	if area < 0.0 { reversed(points) } else { points }
}

// A closed shape through `points`, wound clockwise: shape(wound(points)).
fn closed(points) { shape(wound(points)) }

// A ring between two radii about `centre`, as a path (a disc, if `inner`
// is 0): to fill, as engraving::dark does.
fn ring(centre, inner, outer) {
	let shape = new_circle(centre, outer);
	if inner > 0.0 { shape.append(new_circle(centre, inner)); }
	shape
}

// What of `path` lies outside `solid` (closed shapes, wound alike): what
// shows of it behind something solid. Erases in place; gives the path.
fn behind(path, solid) { path.erase_inside(solid, #{ fill_rule: "nonzero" }) }

// ---- curves ----

// A smooth curve through `points`, `per` samples to a span, as vecs.
fn curve(points, per) {
	let out = [];
	let n = points.len();
	for k in 0..n - 1 {
		let p0 = points[max(k - 1, 0)];
		let p1 = points[k];
		let p2 = points[k + 1];
		let p3 = points[min(k + 2, n - 1)];
		for s in 0..per {
			let t = s * 1.0 / per;
			let t2 = t * t;
			let t3 = t2 * t;
			out.push((p1 * 2.0 + (p2 - p0) * t + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2 + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3) * 0.5);
		}
	}
	out.push(points[n - 1]);
	out
}

// A smooth closed curve through `points`, as vecs, the last back to the
// first.
fn loop_curve(points, per) {
	let n = points.len();
	let out = [];
	for k in 0..n {
		let p0 = points[(k + n - 1) % n];
		let p1 = points[k];
		let p2 = points[(k + 1) % n];
		let p3 = points[(k + 2) % n];
		for s in 0..per {
			let t = s * 1.0 / per;
			let t2 = t * t;
			let t3 = t2 * t;
			out.push((p1 * 2.0 + (p2 - p0) * t + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2 + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3) * 0.5);
		}
	}
	out
}

// Points round an arc: from angle a0 to a1 (clockwise, y down), n + 1 of
// them.
fn arc_points(centre, radius, a0, a1, n) {
	steps(n, |k| centre + polar(a0 + (a1 - a0) * k / n) * radius)
}

// Points round an ellipse, n of them, clockwise on the page from its right.
fn ellipse_points(centre, rx, ry, n) {
	times(n, |k| {
		let a = 2.0 * PI() * k / n;
		centre + vec2(a.cos() * rx, a.sin() * ry)
	})
}

// A whorl: points round `centre` from r0 out to r1, turns times round,
// squashed to `squash` of its height (to lie in water, say), starting at
// angle a0.
fn whorl_points(centre, r0, r1, turns, squash, a0) {
	let n = (turns * 36.0).to_int();
	steps(n, |j| {
		let t = j * 1.0 / n;
		let a = a0 + t * turns * 2.0 * PI();
		let r = r0 + (r1 - r0) * t;
		centre + vec2(a.cos() * r, a.sin() * r * squash)
	})
}

// `points` mirrored across the upright line x = `axis`, in reverse order:
// a right half's outline gives the left half's, to join on.
fn mirrored(points, axis) {
	reversed(points).map(|p| vec2(2.0 * axis - p.x, p.y))
}

// Which way is left of the way a run of points goes at its j-th.
fn side(pts, j) {
	let n = pts.len();
	let tan = (pts[min(j + 1, n - 1)] - pts[max(j - 1, 0)]).normalized();
	vec2(-tan.y, tan.x)
}

// `points` moved `d` to the left of the way they run (to the right, d
// negative): a curve's edge, a band's.
fn offset(points, d) {
	times(points.len(), |j| points[j] + side(points, j) * d)
}

// A limb, a lock, a fold: a smooth tube along the curve through `points`,
// `widths[k]` across at points[k] (eased between), as a closed shape's
// points, wound clockwise.
fn tube(points, widths) {
	let per = 10;
	let pts = curve(points, per);
	let left = [];
	let right = [];
	for j in 0..pts.len() {
		let f = j * 1.0 / per;
		let k = min(f.floor().to_int(), widths.len() - 2);
		let t = f - k;
		let e = t * t * (3.0 - 2.0 * t);
		let w = widths[k] + (widths[k + 1] - widths[k]) * e;
		let s = side(pts, j);
		left.push(pts[j] + s * w / 2.0);
		right.push(pts[j] - s * w / 2.0);
	}
	wound(left + reversed(right))
}

// A thin lens along the curve through `points`, `w` across at its
// widest and pointed at both ends: a highlight, a rib, a fold, to cut out
// of a mass of dark.
fn sliver(points, w) {
	let widths = [0.4];
	for k in 1..points.len() - 1 { widths.push(w); }
	widths.push(0.4);
	shape(tube(points, widths))
}

// ---- gothic tracery ----

// A pointed arch from `left` to `right` (level points), `sharp` the
// arcs' radius over the span (1 is equilateral; 0.5 a round arch), as
// points up and over from left to right.
fn arch_points(left, right, sharp, n) {
	let span = right.x - left.x;
	let r = span * max(sharp, 0.5);
	let lc = vec2(left.x + r, left.y);
	let rc = vec2(right.x - r, right.y);
	let mid = (left.x + right.x) / 2.0;
	// The arcs meet over the middle.
	let rise = (r * r - (r - span / 2.0) * (r - span / 2.0)).sqrt();
	let a_top = (vec2(mid, left.y - rise) - lc).angle();
	let b_top = (vec2(mid, left.y - rise) - rc).angle();
	let out = arc_points(lc, r, PI(), a_top + 2.0 * PI(), n);
	let right_arc = arc_points(rc, r, b_top, 0.0, n);
	right_arc.remove(0);
	out + right_arc
}

// An order of a pointed arch's mouldings: the arch from `left` to `right`
// (`sharp` as arch_points) drawn `d` inside it (or outside, d negative)
// about the same centres, so orders nest as cut stone does. Points from
// its left foot, over, to its right.
fn arch_order(left, right, sharp, d, n) {
	let span = right.x - left.x;
	let r = span * max(sharp, 0.5);
	let lc = vec2(left.x + r, left.y);
	let rc = vec2(right.x - r, right.y);
	let ro = r - d;
	let mid = (left.x + right.x) / 2.0;
	let rise = (ro * ro - (mid - lc.x) * (mid - lc.x)).sqrt();
	let apex = vec2(mid, left.y - rise);
	let out = arc_points(lc, ro, PI(), (apex - lc).angle() + 2.0 * PI(), n);
	let right_arc = arc_points(rc, ro, (apex - rc).angle(), 0.0, n);
	right_arc.remove(0);
	out + right_arc
}

// How high a pointed arch over `span` rises, `sharp` as arch_points.
fn arch_rise(span, sharp) {
	let r = span * max(sharp, 0.5);
	(r * r - (r - span / 2.0) * (r - span / 2.0)).sqrt()
}

// A lancet: a window or a doorway, upright sides from its foot to its
// springing and a pointed arch over, as a closed shape's points. foot is
// the middle of its sill.
fn lancet_points(foot, width, spring, sharp) {
	let left = foot + vec2(-width / 2.0, -spring);
	let right = foot + vec2(width / 2.0, -spring);
	[foot + vec2(-width / 2.0, 0)] + arch_points(left, right, sharp, 18) + [foot + vec2(width / 2.0, 0)]
}

// A foil: `lobes` circles round `centre`, their outline where they're
// outermost, cusped between: a trefoil, a quatrefoil, a cinquefoil. `r`
// is how far its lobes reach; `depth` how deep the cusps cut (0.35 is
// usual); turned `turn`.
fn foil_points(centre, r, lobes, depth, turn) {
	let d = r * depth;
	let rl = r - d;
	let n = lobes * 24;
	times(n, |k| {
		let a = turn + 2.0 * PI() * k / n;
		let best = 0.0;
		for l in 0..lobes {
			let phi = turn - PI() / 2.0 + 2.0 * PI() * l / lobes;
			let s = (a - phi).sin() * d;
			if rl * rl >= s * s {
				let t = (a - phi).cos() * d + (rl * rl - s * s).sqrt();
				if t > best { best = t; }
			}
		}
		centre + vec2(a.cos(), a.sin()) * best
	})
}
