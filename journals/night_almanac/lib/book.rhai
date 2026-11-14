// What the almanac's pages share, so they look like one man's notebook:
// the leaf a page is drawn on (aged, and walled with his account when it's
// bound), the hands he writes in (an old script for the account, a
// cursive for his notes, plain figures for numbers), what age has done to
// the paper (tea rings, foxing, the fold), the construction lines under
// his figures, and the few figures every page draws with: ribbons of
// writing and rings of solid ink.
// import "lib/book" as book;
//
//   let leaf = book::leaf("wheel", 14);
//   book::place(leaf, figure.translate_by(at), 6);
//   book::jot(leaf, "Old to old.", vec2(660, 1150), 440, #{ ink: inks::blood() });
//   journal_entry.add(book::bind(leaf, content("watch")));

import "lib/inks" as inks;
import "lib/chance" as chance;
import "lib/plates" as plates;

// The paler ink the walls of writing are in, so the figures stand out of
// them: iron gall gone brown.
fn sepia() { "#5b3e22" }

// ---- the leaf ----

// A leaf of the notebook to draw a page on: #{ name, size, seed, under,
// figures, avoid }. under is for what lies beneath the ink (stains,
// pencil), figures for the ink, and avoid holds what the wall of writing
// bind puts round them keeps off. It comes aged by `seed`: three tea
// stains, foxing and a fold. o: size (1200 by 1600), fold ("down", "across"
// or "none"), foxing (160 specks).
fn leaf(name, seed) { leaf(name, seed, #{}) }
fn leaf(name, seed, o) {
	let size = o.size ?? vec2(1200, 1600);
	let leaf = #{ name: name, size: size, seed: seed, under: new_canvas(), figures: new_canvas(), avoid: new_avoidance_tree() };
	let spots = [];
	for k in 0..3 {
		spots.push([100 + (size.x - 200) * chance::of(seed, k, 60), 100 + (size.y - 200) * chance::of(seed, k, 61),
			120 + 80 * chance::of(seed, k, 62), 0.12 + 0.07 * chance::of(seed, k, 63)]);
	}
	stains(leaf.under, spots);
	foxing(leaf.under, [40, 40, size.x - 40, size.y - 40], o.foxing ?? 160, seed);
	let fold = o.fold ?? "down";
	let off = |k| 8.0 * chance::either_way(seed, k, 64);
	if fold == "down" {
		crease(leaf.under, vec2(size.x / 2.0 + off.call(0), 0), vec2(size.x / 2.0 + off.call(1), size.y), seed);
	} else if fold == "across" {
		crease(leaf.under, vec2(0, size.y / 2.0 + off.call(0)), vec2(size.x, size.y / 2.0 + off.call(1)), seed);
	} else if fold != "none" {
		throw `book::leaf: fold should be "down", "across" or "none", got ${fold}`;
	}
	leaf
}

// Puts `canvas` on the leaf, in its ink, and keeps the wall `margin` off
// its ink. Gives the canvas.
fn place(leaf, canvas, margin) {
	leaf.figures.add(canvas);
	leaf.avoid.add_hull(canvas, #{ margin: margin });
	canvas
}

// Keeps the wall `margin` off `region`, a closed path, on the leaf.
fn clear(leaf, region, margin) {
	leaf.avoid.add_region(region, #{ margin: margin });
}

// A note of his on the leaf (see note), the wall kept `o.margin` (8) off
// it. Gives the note, #{ canvas, hole }.
fn jot(leaf, text, at, width) { jot(leaf, text, at, width, #{}) }
fn jot(leaf, text, at, width, o) {
	let n = note(text, at, width, o);
	leaf.figures.add(n.canvas);
	clear(leaf, n.hole, o.margin ?? 8);
	n
}

// The leaf as a page named its name: what's under, then his account walled
// round everything placed, jotted and cleared, then the ink. account is
// the text of the wall. o: size (15), the wall's glyphs; hand (wall_hand
// at that size); area (the leaf, 40 in from its edges), where it's walled.
fn bind(leaf, account) { bind(leaf, account, #{}) }
fn bind(leaf, account, o) {
	let size = o.size ?? 15.0;
	let area = o.area ?? new_rectangle(leaf.size / 2.0, leaf.size.x - 80, leaf.size.y - 80);
	let page = new_canvas();
	page.add(leaf.under);
	page.add(wall(account, area, leaf.avoid, o.hand ?? wall_hand(size)));
	page.add(leaf.figures);
	new_page(#{ name: leaf.name, size: leaf.size }).add(page)
}

// ---- hands ----

// The old hand the account is in: a glyph for a letter, in a pen that
// runs dry and is dipped again, so the wall of it is lighter and darker
// by turns.
fn old_hand(size) {
	#{
		style: new_style(new_brush_style(inks::black(), "Rough Ink", size * 0.035, #{
			min_pressure: 0.35, max_pressure: 0.66, pressure_profile: "taper", pressure_wander: 0.15,
			pool: 0.25, pool_depth: 0.4, color_jitter: 0.05, ink_load: 0.36, redip_every: 70,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.04, glyph_position_jitter: [size * 0.03, size * 0.05], glyph_scale_jitter: 0.05, baseline_wander: size * 0.08 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.02 }),
		paragraph: new_paragraph_style(#{ line_height: 1.18, clearance: 5.0, line_start_jitter: size * 0.3, line_gap_jitter: size * 0.08 }),
	}
}

// The same script small and close, in sepia: the writing a page is
// walled with, round and between its figures.
fn wall_hand(size) {
	#{
		style: new_style(new_brush_style(sepia(), "Rough Ink", size * 0.04, #{
			min_pressure: 0.3, max_pressure: 0.62, pressure_profile: "taper", pressure_wander: 0.2,
			color_jitter: 0.08, ink_load: 0.45, redip_every: 90,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.05, glyph_position_jitter: [size * 0.04, size * 0.06], glyph_scale_jitter: 0.08, baseline_wander: size * 0.1 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.03 }),
		paragraph: new_paragraph_style(#{ line_height: 1.1, clearance: size * 0.35, line_start_jitter: size * 0.2, line_gap_jitter: size * 0.06 }),
	}
}

// The same hand in another ink, a heavier pen: for what's written to be
// seen, on ribbons and rings.
fn loud_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.05, #{
			min_pressure: 0.5, max_pressure: 0.8, pressure_profile: "taper", pressure_wander: 0.2, color_jitter: 0.04,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_scale_jitter: 0.04 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.04 }),
		paragraph: new_paragraph_style(#{ line_height: 1.0 }),
	}
}

// The hand of his notes: a quick cursive, in `ink`, the pen pressed and
// lifted as a writer's is, the lines not quite level.
fn note_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.032, #{
			min_pressure: 0.3, max_pressure: 0.75, pressure_profile: "taper", pressure_wander: 0.25, color_jitter: 0.05,
			ink_load: 0.6, redip_every: 120,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_position_jitter: [size * 0.02, size * 0.03], glyph_scale_jitter: 0.04, baseline_wander: size * 0.1 })),
		type: new_type_style(glyphset("ayan"), size, #{ glyph_tracking: 0.0 }),
		paragraph: new_paragraph_style(#{ line_height: 1.3, line_start_jitter: size * 0.25, line_gap_jitter: size * 0.08 }),
	}
}

// A text as the old hand writes it: its letters and spaces alone.
fn old_text(text) {
	let out = "";
	let gap = false;
	for ch in text.to_lower().chars() {
		if ch >= 'a' && ch <= 'z' {
			if gap && out != "" { out += " "; }
			out += ch;
			gap = false;
		} else {
			gap = true;
		}
	}
	out
}

// A note in his cursive, wrapped to `width` from `at` (its first line's
// top left) and turned a little about it: #{ canvas, hole }, the hole the
// box it fills, to keep the walls of writing off it. o: size (30), ink
// (black), turn (0, in radians).
fn note(text, at, width) { note(text, at, width, #{}) }
fn note(text, at, width, o) {
	let size = o.size ?? 30.0;
	let turn = o.turn ?? 0.0;
	let written = write_block(text, at + vec2(0, size), width, note_hand(size, o.ink ?? inks::black())).canvas;
	let b = written.bounds();
	let middle = (b.min + b.max) / 2.0;
	let box = new_rectangle(middle, b.max.x - b.min.x + size * 0.8, b.max.y - b.min.y + size * 0.6);
	#{ canvas: written.rotate_around(at, turn), hole: box.rotate_around(at, turn) }
}

// `text` over and over, till it's `want` characters (its last repeat cut
// short).
fn repeated(text, want) {
	let said = text;
	while said.len() < want { said += " " + text; }
	said.sub_string(0, want)
}

// Plain figures, for numbers that mustn't be misread: the plain glyphs
// (which have every sign the cursive lacks), in a steady pen whose stops
// don't taper away.
fn figures_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.036, #{ min_pressure: 0.55, max_pressure: 0.75, pressure_profile: "constant", color_jitter: 0.05 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.04, glyph_position_jitter: [size * 0.028, size * 0.042], baseline_wander: size * 0.083 })),
		type: new_type_style(glyphset("plain"), size, #{ glyph_tracking: 0.05 }),
		paragraph: new_paragraph_style(#{ line_height: 1.3, line_start_jitter: size * 0.25, line_gap_jitter: size * 0.08 }),
	}
}

// ---- walls of writing ----

// The writing a page is walled with: `text` in `hand` (wall_hand and the
// like), as many times over as fills `area` (a closed path), kept off
// everything in `avoid` (an avoidance tree, a canvas or paths). Gives the
// canvas.
fn wall(text, area, avoid, hand) {
	let hand = hand;
	hand.avoid = avoid;
	let size = hand.type.glyph_scale;
	let b = area.bounds();
	// Plenty: a glyph to every 0.55 of a square of the glyph scale, and
	// what's left over is dropped.
	let want = ((b.max.x - b.min.x) * (b.max.y - b.min.y) / (size * size * 0.55)).to_int();
	write_inside_path(repeated(old_text(text), want), area, hand).canvas
}

// ---- what age has done ----

// Stains under everything: wet rings and patches, the colour of old tea.
// spots: [x, y, radius, strength].
fn stains(c, spots) {
	for spot in spots {
		let tea = `rgba(104, 66, 24, ${spot[3]})`;
		c.add(new_arc(vec2(spot[0], spot[1]), spot[2], 0.3, 5.4), inks::stain(tea, spot[2] * 0.11));
		c.add(new_arc(vec2(spot[0], spot[1]), spot[2] * 0.45, 2.0, 7.4), inks::stain(tea, spot[2] * 0.2));
	}
}

// Foxing: the brown specks damp leaves in old paper, `count` of them in
// the box [x0, y0, x1, y1], most of them small, gathered in a few clouds.
fn foxing(c, box, count, seed) {
	let clouds = [];
	for k in 0..5 {
		clouds.push(vec2(box[0] + (box[2] - box[0]) * chance::of(seed, k, 40), box[1] + (box[3] - box[1]) * chance::of(seed, k, 41)));
	}
	for k in 0..count {
		let near = clouds[k % 5];
		let reach = 40.0 + 160.0 * chance::of(seed, k, 42);
		let a = 6.283 * chance::of(seed, k, 43);
		let at = near + vec2(a.cos(), a.sin()) * reach * chance::of(seed, k, 44);
		let r = 0.5 + 2.2 * (chance::of(seed, k, 45) ** 3.0);
		let tea = `rgba(120, 74, 30, ${0.1 + 0.22 * chance::of(seed, k, 46)})`;
		c.add(new_circle(at, r), inks::stain(tea, r));
	}
}

// Where the page was folded, from `a` to `b`: a soft line of grime that
// wanders a little, darker where the fold wore through.
fn crease(c, a, b, seed) {
	let len = (b - a).length();
	let across = vec2(-(b - a).y, (b - a).x) / len;
	let n = max((len / 40.0).to_int(), 1);
	let pts = [];
	for k in 0..=n {
		let t = k * 1.0 / n;
		pts.push(a + (b - a) * t + across * (2.5 * (t * 9.0 + seed).sin() + 1.2 * chance::either_way(seed, k, 50)));
	}
	c.add(polyline(pts), inks::pencil("rgba(92, 60, 28, 0.22)", 3.5));
	c.add(polyline(pts), inks::pencil("rgba(70, 44, 18, 0.45)", 0.8));
}

// ---- construction ----

// The lines a figure is built on, as a draughtsman leaves them: a cross
// through its centre reaching `reach` each way, and a compass circle
// `radius` round it broken where the pen was lifted. In pencil.
fn construction(c, centre, reach, radius, ink) {
	let lines = new_path();
	lines.append(new_line(centre - vec2(reach, 0), centre + vec2(reach, 0)));
	lines.append(new_line(centre - vec2(0, reach), centre + vec2(0, reach)));
	c.add(lines, inks::pencil(ink, 1.0));
	if radius > 0.0 {
		c.add(new_arc(centre, radius, -1.3, 1.9), inks::pencil(ink, 0.9));
		c.add(new_arc(centre, radius, 2.3, 4.5), inks::pencil(ink, 0.9));
	}
}

// ---- a flowing line ----

// A smooth curve through `points` (an array of vecs), `per` samples to a
// span, as an array of vecs.
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

// A polyline through an array of vecs, as a path.
fn polyline(pts) {
	let p = new_path().move_to(pts[0]);
	for k in 1..pts.len() { p.line_to(pts[k]); }
	p
}

// Which way is left of the way a curve runs at its j-th point.
fn side(pts, j) {
	let n = pts.len();
	let tan = (pts[min(j + 1, n - 1)] - pts[max(j - 1, 0)]).normalized();
	vec2(-tan.y, tan.x)
}

// ---- ribbons ----

// A ribbon of writing: a band `width` wide along the curve through
// `points`, its edges ruled twice in `ink`, the text on its middle in
// `hand` (old_hand and the like), at about the hand's size: as many of its
// words as fill it, over and over. Gives #{ canvas, hole }, the hole the
// band's outline, to keep walls of writing out of it.
fn ribbon(points, width, text, hand, ink) {
	let pts = curve(points, 12);
	let left = [];
	let right = [];
	let left_in = [];
	let right_in = [];
	for j in 0..pts.len() {
		let s = side(pts, j);
		left.push(pts[j] + s * width / 2.0);
		right.push(pts[j] - s * width / 2.0);
		left_in.push(pts[j] + s * (width / 2.0 - 5.0));
		right_in.push(pts[j] - s * (width / 2.0 - 5.0));
	}
	let c = new_canvas();
	c.add(polyline(left), inks::contour(ink, 2.0));
	c.add(polyline(right), inks::contour(ink, 2.0));
	c.add(polyline(left_in), inks::detail(ink, 0.8));
	c.add(polyline(right_in), inks::detail(ink, 0.8));
	let centre = polyline(pts);
	c.add(centre.fit_text(plates::inscription(centre, text, hand), hand));
	let outline = polyline(left);
	right.reverse();
	for p in right { outline.line_to(p); }
	outline.close();
	#{ canvas: c, hole: outline }
}

// ---- solid ink ----

// A ring of solid ink between two radii, as a closed mass, to draw with
// inks::fill: gives #{ shape, gap, pen }.
fn solid_ring(centre, inner, outer, ink) {
	let shape = new_circle(centre, outer);
	if inner > 0.0 { shape.append(new_circle(centre, inner)); }
	let gap = 3.0;
	#{ shape: shape, gap: gap, pen: inks::fill(ink, gap) }
}

// Adds a ring (or disc, if `inner` is 0) of solid ink to canvas c.
fn fill_ring(c, centre, inner, outer, ink) {
	let r = solid_ring(centre, inner, outer, ink);
	c.add(inks::solid(r.shape, r.gap), r.pen);
}
