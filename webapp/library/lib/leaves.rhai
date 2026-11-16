// A leaf of an old book to draw a page on, and the writing on it: the
// leaf aged (stained, foxed, folded, tide-marked, burnt or rotted at its
// edges), figures placed on it, notes jotted, ribbons of writing, and the
// page bound with the account walled round everything in an old blocky
// hand; and the hands a book is
// written in (the account's, a cursive for notes, plain figures for
// numbers, a blackletter blade for headings, letters cut in stone).
// import "lib/leaves" as leaves;
//
//   let leaf = leaves::leaf("door", 71, #{ ink: "#16131c" });
//   leaves::place(leaf, figure, 8);
//   leaves::jot(leaf, "It has no knob.", vec2(80, 60), 500);
//   journal_entry.add(leaves::bind(leaf, content("nights")));

import "lib/inks" as inks;
import "lib/chance" as chance;
import "lib/shapes" as shapes;
import "lib/plates" as plates;

// ---- the leaf ----

// A leaf to draw a page on: #{ name, size, seed, ink, under, figures,
// over, avoid }. under is for what lies beneath the ink (stains, pencil),
// figures for the ink, over for what's happened to the page since (its
// burnt edges), and avoid holds what the wall of writing bind puts round
// them keeps off; ink is what its wall and notes are written in. It comes
// aged by `seed`. o: size (1200 by 1600); ink (inks::black()); stains (3)
// of tea, the colour `tea` ([96, 58, 20], r, g and b), stain_size ([110,
// 90]: so far round and up to so much more) and stain_strength ([0.1,
// 0.08], how dark, likewise); foxing (140 specks) in `fox` ([110, 66, 26])
// kept foxing_margin (30) off the edges; fold ("none", "down" or
// "across"), where it was folded; tide (0), tide-lines where it lay in
// water, in tide_ink; burn (1, how far its edges are eaten in, 0 for not
// at all), the eaten edge in char, browned beyond it in brown (brown_width
// wide) and rimmed in rim: burnt by default; for edges rotted by damp,
// greyer colours.
fn leaf(name, seed) { leaf(name, seed, #{}) }
fn leaf(name, seed, o) {
	let size = o.size ?? vec2(1200, 1600);
	let leaf = #{ name: name, size: size, seed: seed, ink: o.ink ?? inks::black(), under: new_canvas(), figures: new_canvas(), over: new_canvas(), avoid: new_avoidance_tree() };
	let radii = o.stain_size ?? [110, 90];
	let strength = o.stain_strength ?? [0.1, 0.08];
	let spots = shapes::times(o.stains ?? 3, |k| [100 + (size.x - 200) * chance::of(seed, k, 60), 100 + (size.y - 200) * chance::of(seed, k, 61),
		radii[0] + radii[1] * chance::of(seed, k, 62), strength[0] + strength[1] * chance::of(seed, k, 63)]);
	stains(leaf.under, spots, o.tea ?? [96, 58, 20]);
	let m = o.foxing_margin ?? 30;
	foxing(leaf.under, [m, m, size.x - m, size.y - m], o.foxing ?? 140, seed, o.fox ?? [110, 66, 26]);
	let fold = o.fold ?? "none";
	let off = |k| 8.0 * chance::either_way(seed, k, 64);
	if fold == "down" {
		crease(leaf.under, vec2(size.x / 2.0 + off.call(0), 0), vec2(size.x / 2.0 + off.call(1), size.y), seed);
	} else if fold == "across" {
		crease(leaf.under, vec2(0, size.y / 2.0 + off.call(0)), vec2(size.x, size.y / 2.0 + off.call(1)), seed);
	} else if fold != "none" {
		throw `leaves::leaf: fold should be "none", "down" or "across", got ${fold}`;
	}
	tidemarks(leaf, o.tide ?? 0, o.tide_ink ?? "rgba(72, 68, 58, 0.14)");
	scorch(leaf, o.burn ?? 1.0, o);
	leaf
}

// Puts `canvas` on the leaf, in its ink, and keeps the wall `margin` off
// its hull. Gives the canvas.
fn place(leaf, canvas, margin) {
	leaf.figures.add(canvas);
	leaf.avoid.add_hull(canvas, #{ margin: margin });
	canvas
}

// Puts `canvas` on the leaf and keeps the wall `margin` off its ink
// itself, not its hull: the wall runs on between its strokes.
fn place_open(leaf, canvas, margin) {
	leaf.figures.add(canvas);
	leaf.avoid.add(canvas, #{ margin: margin });
	canvas
}

// Keeps the wall `margin` off `region`, a closed path, on the leaf.
fn clear(leaf, region, margin) {
	leaf.avoid.add_region(region, #{ margin: margin });
}

// A note on the leaf (see note), in the leaf's ink unless o.ink says, the
// wall kept `o.margin` (8) off it. Gives the note, #{ canvas, hole }.
fn jot(leaf, text, at, width) { jot(leaf, text, at, width, #{}) }
fn jot(leaf, text, at, width, o) {
	let n = note(text, at, width, #{ ink: leaf.ink } + o);
	leaf.figures.add(n.canvas);
	clear(leaf, n.hole, o.margin ?? 8);
	n
}

// The leaf as a page named its name: what's under, then the account walled
// round everything placed, jotted and cleared, then the ink, then the
// burns. account is the text of the wall ("" for none). o: size (24), the
// wall's glyphs; hand (wall_hand at that size, in the leaf's ink); area
// (the leaf, 50 in from its edges), where it's walled; density, as wall
// takes it.
fn bind(leaf, account) { bind(leaf, account, #{}) }
fn bind(leaf, account, o) {
	let size = o.size ?? 24.0;
	let area = o.area ?? new_rectangle(leaf.size / 2.0, leaf.size.x - 100, leaf.size.y - 100);
	let page = new_canvas();
	page.add(leaf.under);
	if account != "" { page.add(wall(account, area, leaf.avoid, o.hand ?? wall_hand(size, leaf.ink), o)); }
	page.add(leaf.figures);
	page.add(leaf.over);
	new_page(#{ name: leaf.name, size: leaf.size }).add(page)
}

// ---- writing ----

// A note in the cursive, wrapped to `width` from `at` (its first line's
// top left) and turned a little about it: #{ canvas, hole }. o: size (30),
// ink (inks::black()), turn (0, in radians), hand (note_hand).
fn note(text, at, width) { note(text, at, width, #{}) }
fn note(text, at, width, o) {
	let size = o.size ?? 30.0;
	let turn = o.turn ?? 0.0;
	let hand = o.hand ?? note_hand(size, o.ink ?? inks::black());
	let written = write_block(text, at + vec2(0, size), width, hand).canvas;
	let b = written.bounds();
	let middle = (b.min + b.max) / 2.0;
	let box = new_rectangle(middle, b.max.x - b.min.x + size * 0.8, b.max.y - b.min.y + size * 0.6);
	#{ canvas: written.rotate_around(at, turn), hole: box.rotate_around(at, turn) }
}

// `text` written on one line in `hand`, centred on `at`.
fn centred(text, at, hand) {
	let written = write_line(text, vec2(0, 0), hand);
	let b = written.bounds();
	written.translate_by(at - b.center)
}

// A ribbon of writing: a band `width` wide along the curve through
// `points`, its edges ruled twice in `ink`, the text on its middle in
// `hand`, at about the hand's size: as many of its words as fill it, over
// and over. Gives #{ canvas, hole }, the hole the band's outline, to keep
// walls of writing out of it (clear).
fn ribbon(points, width, text, hand, ink) {
	let pts = shapes::curve(points, 12);
	let left = shapes::offset(pts, width / 2.0);
	let right = shapes::offset(pts, -width / 2.0);
	let c = new_canvas();
	c.add(new_polyline(left), inks::contour(ink, 2.0));
	c.add(new_polyline(right), inks::contour(ink, 2.0));
	c.add(new_polyline(shapes::offset(pts, width / 2.0 - 5.0)), inks::detail(ink, 0.8));
	c.add(new_polyline(shapes::offset(pts, 5.0 - width / 2.0)), inks::detail(ink, 0.8));
	let centre = new_polyline(pts);
	c.add(centre.fit_text(plates::inscription(centre, text, hand), hand));
	#{ canvas: c, hole: shapes::shape(left + shapes::reversed(right)) }
}

// The writing a page is walled with: `text` in `hand`, as many times over
// as fills `area` (a closed path), kept off everything in `avoid`. Gives
// the canvas. o.density (0.5): how much text it's given, a glyph to so
// much of a square of the glyph size; what doesn't fit is dropped, but
// how much there is moves where the lines fall.
fn wall(text, area, avoid, hand) { wall(text, area, avoid, hand, #{}) }
fn wall(text, area, avoid, hand, o) {
	let hand = hand;
	hand.avoid = avoid;
	let size = hand.type.glyph_scale;
	let b = area.bounds();
	let want = ((b.max.x - b.min.x) * (b.max.y - b.min.y) / (size * size * (o.density ?? 0.5))).to_int();
	write_inside_path(repeated(old_text(text), want), area, hand).canvas
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

// `text` over and over, till it's `want` characters.
fn repeated(text, want) {
	let said = text;
	while said.len() < want { said += " " + text; }
	said.sub_string(0, want)
}

// ---- hands ----

// The account's hand: the eldritch letters, blocky and close, cut with a
// quill that runs dry and is dipped again, so the wall is darker and
// lighter by turns, as the old books are.
fn wall_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Iron Gall Quill", size * 0.075, #{
			min_pressure: 0.45, max_pressure: 0.85, pressure_profile: "taper", pressure_wander: 0.2,
			color_jitter: 0.06, ink_load: 0.5, redip_every: 80,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.05, glyph_position_jitter: [size * 0.04, size * 0.06], glyph_scale_jitter: 0.08, baseline_wander: size * 0.1 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.08 }),
		paragraph: new_paragraph_style(#{ line_height: 1.25, clearance: size * 0.3, line_start_jitter: size * 0.25, line_gap_jitter: size * 0.08 }),
	}
}

// Notes: a quick cursive, in `ink`, pressed and lifted as a writer's pen
// is, the lines not quite level.
fn note_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.034, #{
			min_pressure: 0.3, max_pressure: 0.75, pressure_profile: "taper", pressure_wander: 0.25, color_jitter: 0.05,
			ink_load: 0.6, redip_every: 120,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_position_jitter: [size * 0.02, size * 0.03], glyph_scale_jitter: 0.04, baseline_wander: size * 0.1 })),
		type: new_type_style(glyphset("ayan"), size, #{ glyph_tracking: 0.0 }),
		paragraph: new_paragraph_style(#{ line_height: 1.3, line_start_jitter: size * 0.25, line_gap_jitter: size * 0.08 }),
	}
}

// Plain figures, for what mustn't be misread: the plain glyphs (which have
// every sign the cursive lacks), in a steady pen whose stops don't taper.
fn figures_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.038, #{ min_pressure: 0.55, max_pressure: 0.75, pressure_profile: "constant", color_jitter: 0.05 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.04, glyph_position_jitter: [size * 0.028, size * 0.042], baseline_wander: size * 0.06 })),
		type: new_type_style(glyphset("plain"), size, #{ glyph_tracking: 0.08 }),
		paragraph: new_paragraph_style(#{ line_height: 1.3 }),
	}
}

// Headings: loche capitals cut with a blackletter blade.
fn title_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Fraktur", size * 0.07, #{ min_pressure: 0.6, max_pressure: 0.95, pressure_profile: "taper", color_jitter: 0.04 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.015, glyph_scale_jitter: 0.02, passes: 2, pass_end_jitter: size * 0.01 })),
		type: new_type_style(glyphset("loche_upper"), size, #{ glyph_tracking: 0.12 }),
		paragraph: new_paragraph_style(#{ line_height: 1.1, align: "center" }),
	}
}

// Letters cut in stone or iron, to be read: loche capitals, heavy, in `ink`.
fn carved_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.06, #{ min_pressure: 0.6, max_pressure: 0.85, pressure_profile: "taper", pressure_wander: 0.15, color_jitter: 0.04 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_scale_jitter: 0.04 })),
		type: new_type_style(glyphset("loche_upper"), size, #{ glyph_tracking: 0.1 }),
		paragraph: new_paragraph_style(#{ line_height: 1.1, align: "center" }),
	}
}

// ---- what age has done ----

// Stains: wet rings and patches the colour of old tea, on `c`. spots: [x,
// y, radius, strength]; tea, their colour as [r, g, b].
fn stains(c, spots, tea) {
	let rgb = `${tea[0]}, ${tea[1]}, ${tea[2]}`;
	for spot in spots {
		let tea = `rgba(${rgb}, ${spot[3]})`;
		c.add(new_arc(vec2(spot[0], spot[1]), spot[2], 0.3, 5.4), inks::stain(tea, spot[2] * 0.11));
		c.add(new_arc(vec2(spot[0], spot[1]), spot[2] * 0.45, 2.0, 7.4), inks::stain(tea, spot[2] * 0.2));
	}
}

// Foxing: brown specks in the box [x0, y0, x1, y1], most small, in clouds,
// in `fox`, their colour as [r, g, b].
fn foxing(c, box, count, seed, fox) {
	let rgb = `${fox[0]}, ${fox[1]}, ${fox[2]}`;
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
		let tea = `rgba(${rgb}, ${0.1 + 0.22 * chance::of(seed, k, 46)})`;
		c.add(new_circle(at, r), inks::stain(tea, r));
	}
}

// Where the page was folded, from `a` to `b`, on `c`: a soft line of grime
// that wanders a little, darker where the fold wore through.
fn crease(c, a, b, seed) {
	let len = (b - a).length();
	let across = vec2(-(b - a).y, (b - a).x) / len;
	let n = max((len / 40.0).to_int(), 1);
	let pts = shapes::steps(n, |k| {
		let t = k * 1.0 / n;
		a + (b - a) * t + across * (2.5 * (t * 9.0 + seed).sin() + 1.2 * chance::either_way(seed, k, 50))
	});
	c.add(new_polyline(pts), inks::pencil("rgba(92, 60, 28, 0.22)", 3.5));
	c.add(new_polyline(pts), inks::pencil("rgba(70, 44, 18, 0.45)", 0.8));
}

// Where the book lay in water: `count` tide-lines across the leaf, low
// down, each a wavering rim in `ink`.
fn tidemarks(leaf, count, ink) {
	let w = leaf.size.x;
	let h = leaf.size.y;
	for k in 0..count {
		let base = h * (0.86 + 0.1 * chance::of(leaf.seed, k, 80)) - k * 60.0;
		let rim = [];
		for j in 0..=40 {
			let x = -10.0 + (w + 20.0) * j / 40.0;
			rim.push(vec2(x, base + 26.0 * (x * 0.004 + leaf.seed * 0.7 + k).sin() + 9.0 * (x * 0.019 + k * 3.0).sin() + 4.0 * chance::either_way(leaf.seed, k * 100 + j, 81)));
		}
		leaf.under.add(new_polyline(rim), inks::stain(ink, 3.0));
	}
}

// The page's edges eaten in: by fire, a scorch creeping in from each,
// wandering, deeper at the corners and wherever the seed says it caught;
// how far in, by `depth` (1 is a finger's breadth). o: char, brown,
// brown_width and rim, as leaf takes them.
fn scorch(leaf, depth, o) {
	if depth <= 0.0 { return; }
	let seed = leaf.seed;
	let w = leaf.size.x;
	let h = leaf.size.y;
	let edges = [[vec2(0, 0), vec2(w, 0)], [vec2(w, 0), vec2(w, h)], [vec2(w, h), vec2(0, h)], [vec2(0, h), vec2(0, 0)]];
	for (e, k) in edges {
		let a = e[0];
		let b = e[1];
		let along = (b - a) / (b - a).length();
		let inward = vec2(-along.y, along.x);
		let n = ((b - a).length() / 12.0).to_int();
		// The reach along this edge: hardly in, mostly, with a slow swell,
		// a few deep bites, and deeper at the corners.
		let bites = [];
		for q in 0..3 { bites.push([chance::of(seed, k * 7 + q, 70), 16.0 + 46.0 * chance::of(seed, k * 7 + q, 71), 10.0 + 18.0 * chance::of(seed, k * 7 + q, 73)]); }
		let reach = [];
		for j in 0..=n {
			let t = j * 1.0 / n;
			let r = 6.0 + 7.0 * ((t * 9.0 + seed * 0.37 + k).sin() * 0.5 + 0.5) + 4.0 * (t * 31.0 + k * 2.0).sin()
				+ 40.0 * ((-(t * 18.0)).exp() + (-((1.0 - t) * 18.0)).exp());
			for bite in bites { r += bite[1] * (-(((t - bite[0]) * bite[2]) ** 2.0)).exp(); }
			r += 5.0 * chance::either_way(seed, k * 1000 + j, 72);
			reach.push(max(r, 2.0) * depth);
		}
		let at = |j, f| a + (b - a) * (j * 1.0 / n) + inward * reach[j] * f;
		// What's gone: black from the edge to the reach.
		let gone = new_path().move_to(a - inward * 6.0);
		for j in 0..=n { gone.line_to(at.call(j, 1.0)); }
		gone.line_to(b - inward * 6.0);
		gone.close();
		leaf.over.add(inks::solid(gone, 3.0), inks::fill(o.char ?? "#120c08", 3.0));
		// Browned beyond it, soft, fading in.
		let browned = [];
		let rim = [];
		for j in 0..=n { browned.push(at.call(j, 1.0) + inward * 9.0); rim.push(at.call(j, 1.0)); }
		leaf.over.add(new_polyline(browned), new_style(new_brush_style(o.brown ?? "rgba(92, 52, 18, 0.55)", "Scorch", o.brown_width ?? 9.0, #{
			min_pressure: 0.3, max_pressure: 0.7, pressure_profile: "constant", pressure_wander: 0.5,
		}), new_handwriting_style()));
		leaf.over.add(new_polyline(rim), new_style(new_brush_style(o.rim ?? "#1e1209", "Cinder", 5.0, #{
			min_pressure: 0.4, max_pressure: 0.8, pressure_profile: "constant", pressure_wander: 0.5,
		}), new_handwriting_style()));
	}
}
