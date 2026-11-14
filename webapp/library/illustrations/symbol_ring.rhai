// symbol_ring: a base's symbols in a ring, clockwise from the top, each
// joined by a dashed line to the symbol step on from it, round and round:
// base 5 and step 2 is a five-pointed star; where base and step share a
// factor, a figure for each cycle. In the middle, larger, is the symbol
// for the step. With bordered, each symbol round the ring is ringed,
// and the dashes run from border to border; with center_bordered, the
// middle's is. Step 0 takes no path: no lines.
//
// steps, if not 0, draws only the first that many lines of the walk from
// the top (0, step, 2 step, ...), and half the next, ending in an arrow
// to say it goes on; a walk that comes back to the top before then is
// drawn closed, without the arrow.
//
// The canvas is size across and down, the ring's centre in its middle.
// Its styles are TextStyles, a style source's: glyph_style for the
// symbols and their borders, line_style for the dashed lines.
// glyph_style's glyphs are its type's, at its size, or smaller if the
// ring hasn't room, leant as it leans them; the middle's are center_scale
// times that, up to half the ring's radius. A symbol is one of the
// glyphset's own glyphs, never its fallback's. In balanced-ternary digits
// (balanced_ternary's), it's its digit, shifted to sit round 0 (1 to 13 as
// they are, 14 to 26 as -13 to -1); in a glyphset with a whole alphabet,
// its letter, in capitals or else lowercase (0 a space, a gap in the ring,
// 1 to 26 A to Z); in any other (thirteen_black_priests, the_four_divine),
// the glyph that far along its characters, of which it needs at least
// base. Unwired, they're near-black and red ink pens, the glyphs
// loche_upper, as big as the ring holds.
//
// The whole symbol set can be passed in instead: symbol_set, a SymbolSet,
// whose marks are the ring's symbols (symbol k of the set at place k),
// glyph_size across (the set's own size if that's 0) or smaller if the
// ring hasn't room, and whose base is the ring's (the base option is 0,
// the set's whole, unless it gives fewer). The set then draws every
// symbol, borders still in glyph_style's pen; unwired, the glyphs
// are glyph_style's, as above.

import "lib/symbol_rings" as rings;

fn options() {
	#{
		base: 0, step: 5, steps: 0, bordered: true, center_bordered: true, size: 900.0, center_scale: 3.0, glyph_size: 0.0,
		dash: 14.0, gap: 10.0, glyph_ink: "#2b1d12", line_ink: "#9b2318",
	}
}

fn inputs(o) {
	#{ glyph_style: "TextStyle", line_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		glyph_style: #{
			style: new_style(new_brush_style(o.glyph_ink, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand),
			type: new_type_style(glyphset("loche_upper"), rings::room(o.size, if o.base > 0 { o.base } else { 27 })),
		},
		line_style: #{ style: new_style(new_brush_style(o.line_ink, "Ink Pen", 1.4), hand) },
		symbol_set: (),
	}
}

fn render(o, i) {
	let m = rings::marks(o, i.symbol_set, i.glyph_style);
	let o = o;
	o.base = m.base;
	rings::check_base(o.base);
	rings::check_step(o.base, o.step, "the step");
	rings::check_steps(o.steps);
	let r = rings::ring(o, m, 1);
	let c = new_canvas();
	rings::draw_walk(c, r, rings::walk(o.base, o.step, o.steps), i.line_style.style, o, 0.0, #{});
	let style = i.glyph_style.style;
	rings::draw_symbols(c, r, m, 1, style);
	rings::draw_middle(c, r, m, [o.step], [style], style);
	c
}

fn example() {
	#{}
}
