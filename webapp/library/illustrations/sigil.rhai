// sigil: a word drawn as one line, the way sigils are made: the base's
// symbols stand in a ring, clockwise from the top, and the line is traced
// from the word's first symbol to its next and on to its last, straight
// across the ring each time. It starts in a small circle and ends in a
// bar across it. Each time the line comes to a symbol it stops a little
// farther in from it than the time before, so its corners at a symbol
// used twice stay apart and its order can be followed; a symbol twice
// running is a short stroke straight in toward the middle.
//
// ordering is the ring: the symbol at each place, clockwise from the top
// (a substitution cipher's ordering, say); unwired, the symbols in order,
// as symbol_ring has them. With show_ring false only the line is drawn:
// the sigil alone, to lay over a ring drawn elsewhere (symbol_ring's and
// rearranged_symbol_ring's places, at the same size and base, are these).
// With bordered, each symbol of the ring is ringed.
//
// The canvas is size across and down, the ring's centre in its middle.
// Draws word, the symbols the line goes through, and ordering, the ring,
// in symbol_set, a SymbolSet: its glyphs at its size, or smaller if the
// ring hasn't room, upright; a blank symbol a gap in the ring (the line
// still goes there). line_style, a TextStyle, a style source's, draws the
// sigil. Unwired, a red ink pen, and the symbols the letters set, in ink.
// With editor, each symbol of the ring is committed to the visual editor
// as EDITOR_0 to EDITOR_N, so any can be changed there.

import "lib/symbol_rings" as rings;
import "lib/permutations" as perm;
import "lib/inks" as inks;

fn options() {
	#{ base: 27, size: 700.0, show_ring: true, bordered: false, editor: "", line_ink: "#9b2318", glyph_ink: "#2b1d12" }
}

fn inputs(o) {
	#{ word: `Base${o.base}Symbol`, ordering: `Base${o.base}Symbol`, line_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	let ordering = [];
	for s in 0..o.base { ordering.push(s); }
	#{
		ordering: ordering,
		line_style: #{ style: new_style(new_brush_style(o.line_ink, "Ink Pen", 2.0, #{ min_pressure: 0.5, max_pressure: 0.8, pressure_profile: "constant" }), hand) },
		symbol_set: symbol_set("letters", #{ color: o.glyph_ink }),
	}
}

fn render(o, i) {
	rings::check_base(o.base);
	perm::check(i.ordering, o.base, "the ordering");
	let set = i.symbol_set;
	if set.base < o.base { throw `sigil: the symbol set '${set.name}' has ${set.base} symbols, fewer than the base, ${o.base}`; }
	let centre = vec2(o.size / 2.0, o.size / 2.0);
	let room = rings::room(o.size, o.base);
	let radius = o.size / 2.0 - room;
	let glyph = min(set.size, room);
	let c = new_canvas();
	// Where each symbol stands.
	let place = perm::inverse(i.ordering);
	if o.show_ring {
		for k in 0..o.base {
			let at = centre + toward(k, o.base) * radius;
			if o.bordered { c.add(new_circle(at, glyph * 0.75), inks::contour(o.glyph_ink, 1.4)); }
			let mark = write_at(set.text([i.ordering[k]]), at, #{ type: #{ glyph_scale: glyph } });
			if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${k}`); }
			c.add(mark);
		}
	}
	if i.word.is_empty() { return c; }
	// The line's corners: clear of each symbol, and a step farther in each
	// time it's come to, as far as half way to the middle.
	let most = 1;
	let times = #{};
	for s in i.word {
		let key = `${s}`;
		times[key] = if key in times { times[key] + 1 } else { 1 };
		most = max(most, times[key]);
	}
	let reach = radius - rings::clear_of(glyph, glyph, o.bordered) - glyph * 0.25;
	let step = min(glyph * 0.6, reach * 0.5 / most);
	let came = #{};
	let corners = [];
	for s in i.word {
		let key = `${s}`;
		let n = if key in came { came[key] } else { 0 };
		came[key] = n + 1;
		corners.push(centre + toward(place[s], o.base) * (reach - n * step));
	}
	let style = i.line_style.style;
	let mark = glyph * 0.22;
	// From the edge of its circle, toward the next corner (or the middle,
	// for a word of one symbol).
	let onward = if corners.len() > 1 { corners[1] - corners[0] } else { centre - corners[0] };
	let line = new_path().move_to(corners[0] + onward.normalized() * mark);
	for k in 1..corners.len() { line.line_to(corners[k]); }
	c.add(new_circle(corners[0], mark), style);
	let last = corners[corners.len() - 1];
	if corners.len() > 1 {
		c.add(line, style);
		onward = last - corners[corners.len() - 2];
	}
	let across = onward.normalized().rotated(PI() / 2.0) * mark * 2.4;
	c.add(new_line(last - across, last + across), style);
	c
}

fn example() {
	#{ word: symbols(27, "ARCANA").symbols() }
}

// Which way place k of count is from the middle, clockwise from the top.
fn toward(k, count) {
	let a = -PI() / 2.0 + 2.0 * PI() * k / count;
	vec2(a.cos(), a.sin())
}
