// month_wheel: the year drawn as a wheel of thirteen months, in the manner
// of a witness's frontispiece: a lashed eye at its hub in nested stars, a
// ring of thirteen medallions each holding a moon, the months' glyphs
// round them, a ring of writing, rings of solid black between, a star in
// each corner.
//
// The medallions are the puzzle: the first, at the top, holds a star, the
// month the count starts from; the other twelve, clockwise from it, each
// hold a phase of the moon (a symbol of marks_set, the moons by default),
// the digits of a number to be read in order. In the almanac's wheel page
// they spell a word, two digits to a letter.
//
// The canvas is size across and down. Round it all is the writing,
// ring_text, its words over and over round its ring in the old hand.
//   marks_set, marks   the twelve moons: the symbols of the medallions
//                      after the first (default all 0, a new moon)
//   months_set         the glyphs of the thirteen months round the
//                      medallions (unwired, the priests')
//   stars_set, hub, start, corners
//                      the stars: the hub's, the first medallion's and
//                      the four corners' (symbols of the set, default 0,
//                      2 and 1 3 1 3: a lashed eye, a weeping thing,
//                      spirals and needles; unwired, dread_stars)
// With editor, each mark is committed to the visual editor as EDITOR.NAME
// (marks_3, month_0, corners_1, hub, start).

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/chance" as chance;
import "lib/book" as book;

fn options() {
	#{
		size: 1100.0, seed: 3, editor: "", ink: "#2b1d12", blood: "#9b2318",
		ring_text: "the thirteen houses turn and the one that is watching them does not turn",
	}
}

fn inputs(o) {
	#{
		marks_set: "SymbolSet", marks: "Any",
		months_set: "SymbolSet",
		stars_set: "SymbolSet", hub: "Any", start: "Any", corners: "Any",
	}
}

fn fallbacks(o) {
	#{
		marks_set: symbol_set("black_moons", #{ ink: o.ink }), marks: [],
		months_set: symbol_set("priests"),
		stars_set: symbol_set("dread_stars", #{ ink: o.ink, blood: o.blood, seed: o.seed }), hub: [], start: [], corners: [],
	}
}

fn example() {
	#{ marks: [0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3] }
}

// A mark of `set`, symbol `symbol`, `size` across, at `at`, turned.
fn mark(c, o, name, set, symbol, at, size, turn) {
	let m = write_at(set.text([symbol]), at, #{ type: #{ glyph_scale: size }, turn: turn });
	if o.editor != "" { m = m.commit_to_editor(`${o.editor}.${name}`); }
	c.add(m);
}

fn render(o, i) {
	let s = o.size;
	let centre = vec2(s / 2.0, s / 2.0);
	if i.months_set.base < 13 { throw `month_wheel: months_set '${i.months_set.name}' has ${i.months_set.base} symbols; the thirteen months need 13`; }
	let digits = plates::slot(i.marks, 12, i.marks_set, "marks", |k| 0);
	let hub = plates::slot(i.hub, 1, i.stars_set, "hub", |k| 0)[0];
	let start = plates::slot(i.start, 1, i.stars_set, "start", |k| 2 % i.stars_set.base)[0];
	let corners = plates::slot(i.corners, 4, i.stars_set, "corners", |k| [1, 3, 1, 3][k] % i.stars_set.base);
	let c = new_canvas();
	let black = o.ink;
	let red = o.blood;

	// ---- the rings, from the outside in ----
	// The frame: two rules and the ticks between them, a heavy line to the
	// edge.
	let frame = plates::frame(s, s, s * 0.012, s * 0.014, s * 0.02);
	c.add(frame.rules, inks::contour(black, s * 0.0018));
	c.add(frame.ticks, inks::detail(black, s * 0.0011));
	// The year's days in a band of tiny ticks, round a rim of solid black.
	book::fill_ring(c, centre, s * 0.472, s * 0.484, black);
	c.add(new_circle(centre, s * 0.492), inks::contour(black, s * 0.0016));
	c.add(plates::ticks(centre, 364, s * 0.4875, s * 0.495, 28, s * 0.5), inks::fine(black, s * 0.0009));
	// Writing round, between two rules.
	let ring_mid = s * 0.447;
	c.add(new_circle(centre, s * 0.4665), inks::contour(black, s * 0.0016));
	c.add(new_circle(centre, s * 0.428), inks::contour(black, s * 0.0022));
	let inscription = new_circle(centre, ring_mid).rotate_around(centre, -PI() / 2.0);
	let size = s * 0.03;
	let hand = book::loud_hand(size, black);
	c.add(inscription.fit_text(plates::inscription(inscription, o.ring_text, hand), hand));
	// The months' glyphs in a band of their own, each between two bars.
	let glyph_ring = s * 0.397;
	c.add(new_circle(centre, s * 0.366), inks::band(black, s * 0.0024));
	c.add(plates::ticks(centre, 13, s * 0.366, s * 0.428, 0, 0.0), inks::contour(black, s * 0.0016));
	for k in 0..13 {
		let turn = k / 13.0;
		// A month's glyph sits at the medallion's turn; the bars are half
		// a month away.
		mark(c, o, `month_${k}`, i.months_set, k, centre + plates::toward(turn) * glyph_ring, s * 0.052, 2.0 * PI() * turn);
	}
	// ---- the medallions ----
	let ring = s * 0.283;
	let medal = s * 0.0475;
	// A night of solid black round them, from the heart's wall to the
	// months' band, each medallion's place left in it.
	let night = new_path().append(new_circle(centre, s * 0.36)).append(new_circle(centre, s * 0.2));
	for k in 0..13 { night.append(new_circle(centre + plates::toward(k / 13.0) * ring, medal * 1.28)); }
	c.add(inks::solid(night, 3.0), inks::fill(black, 3.0));
	for k in 0..13 {
		let turn = k / 13.0;
		let at = centre + plates::toward(turn) * ring;
		let med = new_path().append(new_circle(at, medal)).append(new_circle(at, medal * 1.12));
		c.add(med, inks::contour(black, s * 0.0018));
		c.add(new_circle(at, medal * 1.2), inks::detail(black, s * 0.0012));
		c.add(plates::ticks(at, 24, medal * 1.12, medal * 1.24, 6, medal * 1.34), inks::fine(black, s * 0.0009));
		if k == 0 {
			// The month the count starts from: a star in the first, and a
			// pointer in red from it across the months' band.
			mark(c, o, "start", i.stars_set, start, at, medal * 1.9, 0.0);
			let pointer = new_path();
			for side in [-1.0, 1.0] {
				let off = plates::toward(turn + 0.25) * s * 0.0035 * side;
				pointer.append(new_line(centre + plates::toward(turn) * (ring + medal * 1.35) + off, centre + plates::toward(turn) * s * 0.426 + off));
			}
			c.add(pointer, inks::detail(red, s * 0.0013));
		} else {
			mark(c, o, `marks_${k - 1}`, i.marks_set, digits[k - 1], at, medal * 1.78, 0.0);
			// A thread to the hub, thin and red.
			let a = centre + plates::toward(turn) * (s * 0.083);
			let b = centre + plates::toward(turn) * (ring - medal * 1.35);
			c.add(new_line(a, b), inks::fine(red, s * 0.0011));
		}
	}

	// ---- the heart ----
	// Stars nested in stars: the thirteen-pointed star two ways, and the
	// hub's star over the middle.
	c.add(new_polygon(centre, s * 0.185, 13, #{ step: 5 }), inks::contour(black, s * 0.0016));
	c.add(new_polygon(centre, s * 0.155, 13, #{ step: 3 }), inks::detail(red, s * 0.0013));
	c.add(new_circle(centre, s * 0.125), inks::contour(black, s * 0.0016));
	c.add(new_circle(centre, s * 0.11), inks::detail(black, s * 0.0011));
	c.add(plates::ticks(centre, 104, s * 0.112, s * 0.124, 8, s * 0.134), inks::fine(black, s * 0.0009));
	// The cross: heavy, to the first ring.
	let cross = new_path();
	for k in 0..4 {
		cross.append(new_line(centre + plates::toward(k / 4.0) * s * 0.134, centre + plates::toward(k / 4.0) * s * 0.196));
	}
	c.add(cross, inks::band(black, s * 0.0016));
	mark(c, o, "hub", i.stars_set, hub, centre, s * 0.215, 0.0);

	// ---- the corners ----
	for k in 0..4 {
		let at = vec2(if k % 2 == 0 { s * 0.092 } else { s - s * 0.092 }, if k < 2 { s * 0.092 } else { s - s * 0.092 });
		let r = s * 0.062;
		let disc = new_path().append(new_circle(at, r)).append(new_circle(at, r * 0.8));
		c.add(disc, inks::contour(black, s * 0.002));
		c.add(plates::ticks(at, 20, r, r * 1.14, 0, 0.0), inks::fine(black, s * 0.0009));
		mark(c, o, `corners_${k}`, i.stars_set, corners[k], at, r * 1.5, 0.0);
	}
	c
}
