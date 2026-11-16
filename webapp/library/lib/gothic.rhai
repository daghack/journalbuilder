// Figures for gothic books: headstones, bare trees, rooks in flight and
// perched, reeds; a star that watches, looking-glasses in carved frames
// and the sheen on them, writing as a glass shows it; swords, and knives
// knapped from black glass. Outlines come as closed shapes (to mass with
// engraving::mass, or to keep things behind with shapes::behind); what's
// drawn comes in the ink it's given.
// import "lib/gothic" as gothic;
//
//   engraving::mass(page, gothic::rook(vec2(300, 200), 90, 0.4, 0.1), new_path(), ink, 1.2);
//   let blade = gothic::sword(vec2(400, 900), vec2(900, 500), 26, ink, #{ black: true });
//   page.add(blade.canvas);

import "lib/inks" as inks;
import "lib/chance" as chance;
import "lib/shapes" as shapes;
import "lib/engraving" as engraving;

// ---- the churchyard ----

// A headstone's outline, standing on `foot` (the middle of its base),
// `w` across and `h` high, as a closed shape's points: kind "round" (a
// round head), "gothic" (pointed), "shoulder" (a round head between two
// shoulders) or "cross" (a wheel-head cross); leaning `lean` radians.
fn headstone(foot, w, h, kind, lean) {
	let x0 = foot.x - w / 2.0;
	let x1 = foot.x + w / 2.0;
	let pts = [];
	if kind == "round" {
		pts = [vec2(x0, foot.y)] + shapes::arc_points(vec2(foot.x, foot.y - h + w / 2.0), w / 2.0, PI(), 2.0 * PI(), 20) + [vec2(x1, foot.y)];
	} else if kind == "gothic" {
		let spring = h - shapes::arch_rise(w, 0.9);
		pts = [vec2(x0, foot.y)] + shapes::arch_points(vec2(x0, foot.y - spring), vec2(x1, foot.y - spring), 0.9, 14) + [vec2(x1, foot.y)];
	} else if kind == "shoulder" {
		let s = w * 0.22;
		pts = [vec2(x0, foot.y), vec2(x0, foot.y - h + w * 0.45)]
			+ shapes::arc_points(vec2(x0 + s, foot.y - h + w * 0.45), s, PI(), 1.5 * PI(), 6)
			+ shapes::arc_points(vec2(foot.x, foot.y - h + w * 0.32), w * 0.3, PI() * 1.05, PI() * 1.95, 12)
			+ shapes::arc_points(vec2(x1 - s, foot.y - h + w * 0.45), s, 1.5 * PI(), 2.0 * PI(), 6)
			+ [vec2(x1, foot.y)];
	} else if kind == "cross" {
		let a = w * 0.17;
		let arm = w * 0.5;
		let mid = foot.y - h + arm;
		pts = [vec2(foot.x - a, foot.y), vec2(foot.x - a, mid + a), vec2(foot.x - arm, mid + a), vec2(foot.x - arm, mid - a), vec2(foot.x - a, mid - a),
			vec2(foot.x - a, foot.y - h), vec2(foot.x + a, foot.y - h), vec2(foot.x + a, mid - a), vec2(foot.x + arm, mid - a),
			vec2(foot.x + arm, mid + a), vec2(foot.x + a, mid + a), vec2(foot.x + a, foot.y)];
	} else {
		throw `gothic::headstone: kind should be round, gothic, shoulder or cross, got ${kind}`;
	}
	shapes::wound(pts.map(|p| foot + (p - foot).rotated(lean)))
}

// A dead tree, bare: a limb from `base` at `angle` (0 is +x, -PI()/2 up)
// `length` long and `width` thick, forking `depth` times more, bent and
// spread by `seed`. Gives #{ limbs, twigs }: limbs, closed shapes to fill
// as one mass; twigs, too thin to fill, as [points, width] to stroke.
fn bare_tree(base, angle, length, width, depth, seed) {
	let out = #{ limbs: [], twigs: [] };
	let bend = 0.5 * chance::either_way(seed, depth, 80);
	let way = vec2(angle.cos(), angle.sin());
	let mid = base + way * length * 0.5 + vec2(-way.y, way.x) * length * 0.07 * bend;
	let a2 = angle + bend * 0.5;
	let end = mid + vec2(a2.cos(), a2.sin()) * length * 0.5;
	if width >= 5.0 {
		out.limbs.push(shapes::tube([base, mid, end], [width, width * 0.84, width * 0.66]));
	} else {
		out.twigs.push([[base, mid, end], width]);
	}
	if depth > 0 {
		let n = if chance::of(seed, depth, 81) < 0.35 { 3 } else { 2 };
		for k in 0..n {
			let spread = 0.3 + 0.4 * chance::of(seed + k * 11, depth, 82);
			let da = if n == 2 { if k == 0 { -spread } else { spread * 0.8 } } else { (k - 1) * spread };
			let s = (seed * 3 + k * 7 + 1) % 10007;
			let more = bare_tree(end, a2 + da + 0.2 * chance::either_way(s, depth, 83), length * (0.6 + 0.2 * chance::of(s, depth, 84)), width * 0.66, depth - 1, s);
			out.limbs += more.limbs;
			out.twigs += more.twigs;
		}
	}
	out
}

// A rook in flight, seen from below and in front, as a closed shape: its
// wings raised by `beat` (-1, down, to 1, up), their fingered tips
// spread, `size` from wingtip to wingtip, tilted `turn`.
fn rook(at, size, beat, turn) {
	let s = size / 2.0;
	let b = beat;
	let right = [
		vec2(0.0, -0.13), vec2(0.04, -0.08),
		vec2(0.08, -0.05), vec2(0.25, -0.1 - 0.25 * b), vec2(0.5, -0.14 - 0.45 * b), vec2(0.75, -0.08 - 0.55 * b), vec2(1.0, 0.0 - 0.6 * b),
		vec2(0.92, 0.04 - 0.52 * b), vec2(0.95, 0.09 - 0.48 * b), vec2(0.85, 0.08 - 0.46 * b), vec2(0.88, 0.14 - 0.42 * b),
		vec2(0.77, 0.12 - 0.39 * b), vec2(0.78, 0.17 - 0.35 * b), vec2(0.64, 0.12 - 0.3 * b),
		vec2(0.45, 0.1 - 0.22 * b), vec2(0.25, 0.1 - 0.1 * b), vec2(0.08, 0.12),
		vec2(0.09, 0.28), vec2(0.04, 0.33),
	];
	shapes::shape((right + shapes::mirrored(right, 0.0)).map(|p| at + vec2(p.x * s, p.y * s).rotated(turn)))
}

// A rook perched, side on, facing +x (or -x with `facing` -1), `size`
// from its feet to the top of its head, as a closed shape.
fn perched_rook(feet, size, facing) {
	let pts = [
		vec2(0.0, 0.0), vec2(0.05, -0.18), vec2(0.22, -0.32), vec2(0.3, -0.52), vec2(0.36, -0.7), vec2(0.34, -0.84),
		vec2(0.42, -0.86), vec2(0.62, -0.83), vec2(0.42, -0.8), vec2(0.44, -0.74), vec2(0.36, -0.68),
		vec2(0.4, -0.8), vec2(0.36, -0.94), vec2(0.24, -1.0), vec2(0.12, -0.94), vec2(0.06, -0.8),
		vec2(-0.08, -0.62), vec2(-0.22, -0.42), vec2(-0.38, -0.2), vec2(-0.6, 0.06), vec2(-0.66, 0.1), vec2(-0.6, 0.0),
		vec2(-0.44, -0.08), vec2(-0.2, -0.08), vec2(-0.06, -0.04), vec2(-0.04, 0.02),
	];
	shapes::shape(pts.map(|p| feet + vec2(p.x * facing, p.y) * size))
}

// Reeds and bulrushes standing out of the water at y, from x0 to x1, at
// most h tall, count of them placed by seed: solid shapes, to mass.
fn reeds(y, x0, x1, h, count, seed) {
	let out = new_path();
	for k in 0..count {
		let x = x0 + (x1 - x0) * chance::of(seed, k, 1);
		let tall = h * (0.4 + 0.6 * chance::of(seed, k, 2));
		let lean = 0.22 * chance::either_way(seed, k, 3);
		let top = vec2(x + lean * tall, y - tall);
		let mid = vec2(x + lean * tall * 0.35, y - tall * 0.5);
		out.append(shapes::shape(shapes::tube([vec2(x, y + 4), mid, top], [3.4, 2.4, 0.5])));
		if chance::of(seed, k, 4) < 0.3 {
			let way = (top - mid).normalized();
			let head = top - way * tall * 0.16;
			out.append(shapes::shape(shapes::tube([head - way * tall * 0.11, head, head + way * tall * 0.07], [6.0, 8.0, 3.0])));
		}
		if chance::of(seed, k, 5) < 0.55 {
			let start = vec2(x + lean * tall * 0.15, y - tall * 0.25);
			let side = if chance::of(seed, k, 6) < 0.5 { -1.0 } else { 1.0 };
			out.append(shapes::shape(shapes::tube([start, start + vec2(side * tall * 0.14, -tall * 0.36), start + vec2(side * tall * 0.36, -tall * 0.28)], [3.0, 2.0, 0.4])));
		}
	}
	out
}

// ---- the night's own ----

// A star that watches: art::star's eye in lashes (or o.kind), inked in
// `ink`, its iris washed grey (o.wash). r is its body's radius; o: kind,
// reach (2.2), angle, wash.
fn watcher(at, r, seed, ink) { watcher(at, r, seed, ink, #{}) }
fn watcher(at, r, seed, ink, o) {
	let s = art::star(at, r, #{ kind: o.kind ?? "lashed", reach: o.reach ?? 2.2, seed: seed, angle: o.angle ?? 0.0 });
	inks::star(s, ink, o.wash ?? "#5f5a68", r)
}

// `c` flipped left to right about its own middle, as a glass shows it.
fn reflected(c) {
	let b = c.bounds();
	c.scale_by(vec2(-1, 1)).translate_by(vec2(b.center.x * 2.0, 0))
}

// `text` written as a glass shows it, centred on `at`, in `hand`.
fn backwards(text, at, hand) {
	let w = write_line(text, vec2(0, 0), hand);
	let b = w.bounds();
	reflected(w.translate_by(at - b.center))
}

// The sheen on a glass: a few faint diagonal strokes in pairs, across
// `glass` (a closed path) about `centre`, `size` its radius, in pencil of
// `ink`, added to c.
fn glints(c, glass, centre, size, ink) {
	let lines = shapes::join([-0.55, -0.48, -0.1, 0.36, 0.41].map(|d| {
		let mid = centre + vec2(d, d * 0.2) * size;
		new_line(mid + vec2(-0.7, 0.7) * size, mid + vec2(0.7, -0.7) * size)
	}));
	c.add(lines.erase_outside(glass), inks::pencil(ink, 0.9));
}

// A looking-glass in a carved frame, in `ink`: an oval glass rx by ry
// about centre, a bead and a dark hollow and an egg-and-dart round it,
// crested with a star that watches (o.crest, true) and hung with a drop
// (o.drop, true); o.width (52), the frame's; o.seed (3), the star's.
// Gives #{ canvas, glass, outer }: the frame drawn, the glass's shape and
// the frame's outer edge, to keep things out of or behind.
fn looking_glass(centre, rx, ry, ink) { looking_glass(centre, rx, ry, ink, #{}) }
fn looking_glass(centre, rx, ry, ink, o) {
	let c = new_canvas();
	let w = o.width ?? 52.0;
	let n = 160;
	let glass = shapes::shape(shapes::ellipse_points(centre, rx, ry, n));
	let ring = |d| shapes::shape(shapes::ellipse_points(centre, rx + d, ry + d, n));
	// The hollow, dark, its beads left bare.
	let holes = new_path().append(shapes::closed(shapes::ellipse_points(centre, rx + w * 0.42, ry + w * 0.42, n)));
	let beads = new_path();
	let bead_line = ring.call(w * 0.21);
	for at in bead_line.points_along(((rx + ry) * 3.1 / 15.0).to_int()) {
		beads.append(new_circle(at.position, w * 0.07));
	}
	engraving::mass(c, shapes::closed(shapes::ellipse_points(centre, rx + w * 0.62, ry + w * 0.62, n)), holes, ink, 1.8);
	c.add(beads, inks::contour(ink, 1.2));
	// Egg and dart round the outside.
	let eggs = new_path();
	let darts = new_path();
	let egg_line = ring.call(w * 0.8);
	let count = ((rx + ry) * 3.1 / (w * 0.42)).to_int();
	for (at, k) in egg_line.points_along(count) {
		let across = vec2(at.angle.cos(), at.angle.sin());
		let out = vec2(across.y, -across.x);
		eggs.append(shapes::shape(shapes::times(12, |j| {
			let a = 2.0 * PI() * j / 12.0;
			at.position + across * (a.cos() * w * 0.14) + out * (a.sin() * w * 0.17)
		})));
		let next = at.position + across * w * 0.25;
		darts.append(new_line(next - out * w * 0.17, next + out * w * 0.12));
	}
	c.add(eggs, inks::detail(ink, 1.1)).add(darts, inks::detail(ink, 1.0));
	let outer = ring.call(w);
	c.add(outer, inks::contour(ink, 2.4));
	c.add(ring.call(w * 0.62), inks::contour(ink, 1.6));
	c.add(ring.call(6.0), inks::contour(ink, 1.6));
	c.add(glass, inks::detail(ink, 1.0));
	let rim = new_path().append(outer);
	// The crest: scrolls either side of a watching star in a burst of rays.
	if o.crest ?? true {
		let top = centre - vec2(0, ry + w);
		let scrolls = new_path();
		// (flip, not side: a closure run where a function named side is in
		// reach, as shapes::side is, reads side as the function.)
		for flip in [-1.0, 1.0] {
			scrolls.append(new_polyline(shapes::steps(28, |j| {
				let t = j / 28.0;
				let a = PI() * 0.5 + flip * (t * 3.6);
				let r = 34.0 * (1.0 - t * 0.7);
				top + vec2(flip * 58, -26) + vec2(a.cos() * -r * flip, -a.sin() * r) + vec2(flip * t * 10.0, 0)
			})));
			scrolls.append(new_polyline(shapes::curve([top + vec2(flip * 18, 6), top + vec2(flip * 60, 2), top + vec2(flip * 98, -20), top + vec2(flip * 110, -52)], 6)));
		}
		c.add(scrolls, inks::contour(ink, 2.0));
		let star = top + vec2(0, -46);
		let burst = shapes::join(shapes::times(24, |j| {
			let way = shapes::polar(-PI() + PI() * j / 23.0);
			new_line(star + way * 40, star + way * (if j % 2 == 0 { 86.0 } else { 62.0 }))
		}));
		c.add(burst, inks::detail(ink, 1.1));
		c.add(watcher(star, 26.0, o.seed ?? 3, ink, #{ reach: 1.4 }));
		rim.append(new_circle(star, 90));
	}
	// The drop: a carved pendant under it.
	if o.drop ?? true {
		let bottom = centre + vec2(0, ry + w);
		let drop = shapes::closed(shapes::curve([bottom + vec2(-30, -4), bottom + vec2(-22, 30), bottom + vec2(-6, 70), bottom + vec2(0, 96), bottom + vec2(6, 70), bottom + vec2(22, 30), bottom + vec2(30, -4)], 6));
		c.add(drop, inks::contour(ink, 1.8));
		c.add(drop.hatch(3.5, #{ angle: [0.9] }).erase_inside(new_circle(bottom + vec2(-10, 24), 18)), inks::detail(ink, 0.8));
		rim.append(drop);
	}
	#{ canvas: c, glass: glass, outer: rim }
}

// ---- blades ----

// A sword, in `ink`: its blade from `hilt`, where the guard crosses, to
// `tip`, w wide at the guard; the guard, the bound grip and the pommel
// behind the hilt. Steel, one bevel shaded and the fuller cut; o.black
// for a blade of black glass, solid, the scars of its flaking left bare.
// o.guard (w * 3.4) and o.grip (w * 3.6) are the guard's span and the
// grip's length. Gives #{ canvas, shape, back }: drawn, its silhouette,
// and the grip's far end.
fn sword(hilt, tip, w, ink) { sword(hilt, tip, w, ink, #{}) }
fn sword(hilt, tip, w, ink, o) {
	let way = (tip - hilt).normalized();
	let side = vec2(-way.y, way.x);
	let len = (tip - hilt).length();
	let c = new_canvas();
	let shoulder = hilt + way * len * 0.84;
	let blade = shapes::closed([hilt + side * w * 0.5, shoulder + side * w * 0.42, tip, shoulder - side * w * 0.42, hilt - side * w * 0.5]);
	let grip_len = o.grip ?? w * 3.6;
	let back = hilt - way * grip_len;
	let gw = o.guard ?? w * 3.4;
	let guard = shapes::shape(shapes::tube([hilt + side * gw * 0.5 + way * w * 0.6, hilt + side * gw * 0.22, hilt - side * gw * 0.22, hilt - side * gw * 0.5 + way * w * 0.6], [w * 0.2, w * 0.36, w * 0.36, w * 0.2]));
	let grip = shapes::shape(shapes::tube([hilt - way * 2.0, hilt - way * grip_len * 0.5, back], [w * 0.5, w * 0.58, w * 0.46]));
	let pommel = shapes::closed(shapes::ellipse_points(back - way * w * 0.36, w * 0.44, w * 0.44, 24));
	if o.black ?? false {
		let scars = new_path();
		let n = (len / (w * 0.55)).to_int();
		for k in 1..n {
			let t = k * 0.84 / n;
			let at = hilt + way * len * t;
			let half = w * 0.46 * (1.0 - t * 0.12);
			let s = if k % 2 == 0 { 1.0 } else { -1.0 };
			scars.append(shapes::sliver([at + side * s * half * 0.92, at + side * s * half * 0.5 + way * w * 0.12, at + side * s * half * 0.08 + way * w * 0.3], 1.6));
		}
		scars.append(shapes::sliver([hilt + way * w, hilt + way * len * 0.5 + side * w * 0.04, hilt + way * len * 0.8], 1.2));
		engraving::mass(c, blade, scars, ink, 1.3);
	} else {
		c.add(blade, inks::contour(ink, 1.8));
		let bevel = shapes::closed([hilt, shoulder, tip, shoulder - side * w * 0.42, hilt - side * w * 0.5]);
		c.add(bevel.hatch(2.4, #{ angle: [way.angle() + 0.35], roughness: 0.6 }), inks::detail(ink, 0.8));
		c.add(new_line(hilt + way * w * 0.7, hilt + way * len * 0.64), inks::detail(ink, 1.4));
	}
	engraving::mass(c, guard, new_path(), ink, 1.3);
	engraving::mass(c, pommel, new_path(), ink, 1.3);
	c.add(grip, inks::contour(ink, 1.6));
	let wraps = new_path();
	let n = (grip_len / (w * 0.32)).to_int();
	for k in 0..n {
		let p = hilt - way * grip_len * ((k + 0.5) / n);
		wraps.append(new_line(p + side * w * 0.27 - way * w * 0.13, p - side * w * 0.27 + way * w * 0.13));
	}
	c.add(wraps, inks::detail(ink, 1.2));
	#{ canvas: c, shape: new_path().append(blade).append(guard).append(grip).append(pommel), back: back }
}

// A knife knapped from black glass, in `ink`: a leaf of a blade from
// `hilt` to `tip`, w at its widest, its edges toothed where flakes came
// off (by seed) and the scars of them left bare; a haft bound with cord
// behind, haft long. Gives #{ canvas, shape }.
fn glass_knife(hilt, tip, w, haft, seed, ink) {
	let way = (tip - hilt).normalized();
	let side = vec2(-way.y, way.x);
	let len = (tip - hilt).length();
	let n = 16;
	// Its half-width a fraction t of the way to the tip.
	let half_at = |t| w * 0.5 * (PI() * (t ** 0.75)).sin().abs();
	let left = [];
	let right = [];
	for j in 0..=n {
		let t = j * 1.0 / n;
		let half = half_at.call(t);
		let bite = 0.86 + 0.1 * chance::of(seed, j, 1);
		left.push(hilt + way * len * t + side * half * (if j % 2 == 1 { bite } else { 1.0 }));
		right.push(hilt + way * len * t - side * half * (if j % 2 == 0 { bite } else { 1.0 }));
	}
	let back = shapes::reversed(right);
	let blade = shapes::closed([hilt + side * w * 0.22] + left.extract(1, n - 1) + [tip] + back.extract(1, n - 1) + [hilt - side * w * 0.22]);
	let scars = new_path();
	for j in 1..n - 1 {
		let t = j * 1.0 / n;
		let at = hilt + way * len * t;
		let half = half_at.call(t);
		let s = if j % 2 == 0 { 1.0 } else { -1.0 };
		scars.append(shapes::sliver([at + side * s * half * 0.85, at + side * s * half * 0.45 + way * len * 0.02, at + side * s * half * 0.1 + way * len * 0.035], 1.5 + chance::of(seed, j, 2)));
	}
	let c = new_canvas();
	engraving::mass(c, blade, scars, ink, 1.2);
	let rear = hilt - way * haft;
	let shaft = shapes::shape(shapes::tube([hilt + way * w * 0.3, hilt - way * haft * 0.5, rear], [w * 0.42, w * 0.4, w * 0.34]));
	c.add(shaft, inks::contour(ink, 1.5));
	c.add(shaft.hatch(2.8, #{ angle: [way.angle() + 1.4], roughness: 0.6 }).erase_inside(new_rectangle(hilt - way * haft * 0.62, w * 2.0, w * 2.0).rotate_around(hilt - way * haft * 0.62, way.angle())), inks::detail(ink, 0.8));
	let cord = new_path();
	for k in 0..9 {
		let p = hilt - way * (w * 0.4 + k * w * 0.16);
		cord.append(new_line(p + side * w * 0.24, p - side * w * 0.24 + way * w * 0.06));
	}
	c.add(cord, inks::contour(ink, 1.3));
	#{ canvas: c, shape: new_path().append(blade).append(shaft) }
}
