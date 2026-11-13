// labyrinth: a Labyrinth drawn round: rings of cells about its heart,
// their walls with a gap wherever one opens on the next, the outer wall
// open at the entrance and the heart's at the one cell that leads in, and
// each cell's symbol in its middle. What labyrinth_lock hides a text in:
// read from the entrance along the one way to the heart, the symbols
// passed are the text, and every other cell's are there to mislead.
//
// The canvas is size across and down, the heart in its middle. With
// heart, the heart holds that glyph of the symbols glyphset ("" leaves it
// empty). wall_style and heart_style, TextStyles, a style source's, draw
// the walls and the heart's glyph. The cells' symbols are symbol_set's, a
// SymbolSet, glyph_size across (the set's own size if that's 0) or
// smaller if the cells haven't room, upright or, with turned, each turned
// to its ring, its foot to the heart. Unwired, near-black ink pens and
// the fine letters set in red. With editor, each cell's symbol is committed to
// the visual editor as EDITOR_RING_CELL.

import "lib/labyrinth" as lab;
import "lib/symbol_rings" as rings;

fn options() {
	#{ size: 900.0, heart: "basic_eye", turned: false, glyph_size: 60.0, editor: "", wall_ink: "#2b1d12", glyph_ink: "#9b2318" }
}

fn inputs(o) {
	#{ labyrinth: "Labyrinth", wall_style: "TextStyle", symbol_set: "SymbolSet", heart_style: "TextStyle" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		wall_style: #{ style: new_style(new_brush_style(o.wall_ink, "Ink Pen", 2.0, #{ min_pressure: 0.4, max_pressure: 0.7, pressure_profile: "constant" }), hand) },
		heart_style: #{ style: new_style(new_brush_style(o.wall_ink, "Ink Pen", 1.6, #{ min_pressure: 0.4, max_pressure: 0.7, pressure_profile: "constant" }), hand) },
		symbol_set: symbol_set("fine_letters", #{ color: o.glyph_ink }),
	}
}

fn render(o, i) {
	let l = i.labyrinth;
	lab::check(l);
	let rings = l.sectors.len();
	let centre = vec2(o.size / 2.0, o.size / 2.0);
	// A ring's width, the heart lab::heart_radius() of them.
	let wide = o.size * 0.48 / (rings + lab::heart_radius());
	let inner = |k| (lab::heart_radius() + k) * wide;
	let walls = new_path();
	// The wall inside each ring, and the outermost's outside: an arc for
	// each run of cells with no opening.
	for k in 0..rings + 1 {
		let n = l.sectors[min(k, rings - 1)];
		let shut = [];
		for s in 0..n {
			shut.push(if k == 0 { s != l.heart } else if k == rings { s != l.entrance } else { !l.inward[k][s] });
		}
		arcs(walls, centre, inner.call(k), shut);
	}
	// The walls between a ring's cells, each run on from ring to ring
	// where the rings' walls are in line.
	let finest = l.sectors[rings - 1];
	for a in 0..finest {
		let from = -1;
		for k in 0..rings + 1 {
			let shut = false;
			if k < rings {
				let every = finest / l.sectors[k];
				// The wall clockwise of cell a / every - 1.
				shut = a % every == 0 && !l.clockwise[k][(a / every + l.sectors[k] - 1) % l.sectors[k]];
			}
			if shut && from < 0 { from = k; }
			if !shut && from >= 0 {
				let way = toward(a * 1.0 / finest);
				walls.append(new_line(centre + way * inner.call(from), centre + way * inner.call(k)));
				from = -1;
			}
		}
	}
	let c = new_canvas().add(walls, i.wall_style.style);
	if o.heart != "" {
		let marks = glyphset("symbols");
		if !(o.heart in marks.characters()) { throw `labyrinth: heart should be a glyph of the symbols glyphset, or "": there's no '${o.heart}'`; }
		c.add(marks.glyph(o.heart, #{ scale: inner.call(0) * 1.3, position: centre }), i.heart_style.style);
	}
	// Each cell's symbol, as big as the narrowest cell holds.
	let set = i.symbol_set;
	if set.base < l.base { throw `labyrinth: the symbol set '${set.name}' has ${set.base} symbols, fewer than the labyrinth's base, ${l.base}`; }
	let size = min(if o.glyph_size > 0.0 { o.glyph_size } else { set.size }, wide * 0.64);
	for k in 0..rings {
		size = min(size, 2.0 * PI() * (inner.call(k) + wide * 0.5) / l.sectors[k] * 0.62);
	}
	for k in 0..rings {
		let n = l.sectors[k];
		for s in 0..n {
			let at = centre + toward((s + 0.5) / n) * (inner.call(k) + wide * 0.5);
			let turn = if o.turned { 2.0 * PI() * (s + 0.5) / n } else { 0.0 };
			let mark = write_at(set.text([l.symbols[k][s]]), at, #{ type: #{ glyph_scale: size }, turn: turn });
			if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${k}_${s}`); }
			c.add(mark);
		}
	}
	c
}

fn example() {
	// What labyrinth_lock makes of a text.
	let made = new_lock("example").add("l", "labyrinth_lock", #{ base: 27, seed: 3 })
		.run(#{ "l.text": symbols(27, "WHAT WAITS AT THE HEART") });
	#{ labyrinth: made.get("l.labyrinth").value() }
}

// Which way a `turn` of the way round is from the middle, clockwise from
// the top: 0.25 is to the right.
fn toward(turn) {
	let a = -PI() / 2.0 + 2.0 * PI() * turn;
	vec2(a.cos(), a.sin())
}

// Arcs of the circle of `radius` about centre added to path: one for each
// run of its cells that are shut, a cell the n-th of the way round each,
// clockwise from the top; a whole circle if they all are.
fn arcs(path, centre, radius, shut) {
	let n = shut.len();
	if !shut.some(|s| !s) {
		path.append(new_circle(centre, radius));
		return;
	}
	// From just after an opening, so no run is cut in two.
	let first = 0;
	while shut[first] { first += 1; }
	let from = -1;
	for step in 1..n + 1 {
		let s = (first + step) % n;
		if shut[s] && from < 0 { from = first + step; }
		if !shut[s] && from >= 0 {
			path.append(new_arc(centre, radius, -PI() / 2.0 + 2.0 * PI() * from / n, -PI() / 2.0 + 2.0 * PI() * (first + step) / n));
			from = -1;
		}
	}
}
