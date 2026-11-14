// rearranged_symbol_ring: a base's symbols in a ring as taking step's
// path rearranges them: clockwise from the top, the symbols in the order
// symbol_ring's walk by step reaches them (0, step, 2 step, ...), so
// place k holds symbol k times step. Base 5 and step 2 is 0 2 4 1 3;
// where base and step share a factor, the walk is back at the top early
// and the ring repeats; step 0 takes no path, and leaves every place
// empty. In the middle, larger, is the symbol for the step. With
// bordered, each symbol round the ring is ringed; with center_bordered,
// the middle's is.
//
// The ring is symbol_ring's with the same options: the canvas is size
// across and down, the ring's centre in its middle, and its symbols are
// glyph_style's (a TextStyle, a style source's), chosen, sized and leant
// as symbol_ring chooses, sizes and leans them. Unwired, they're a
// near-black ink pen, the glyphs loche_upper, as big as the ring holds.
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
	#{ base: 0, step: 5, bordered: true, center_bordered: true, size: 900.0, center_scale: 3.0, glyph_size: 0.0, glyph_ink: "#2b1d12" }
}

fn inputs(o) {
	#{ glyph_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		glyph_style: #{
			style: new_style(new_brush_style(o.glyph_ink, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand),
			type: new_type_style(glyphset("loche_upper"), rings::room(o.size, if o.base > 0 { o.base } else { 27 })),
		},
		symbol_set: (),
	}
}

fn render(o, i) {
	let m = rings::marks(o, i.symbol_set, i.glyph_style);
	let o = o;
	o.base = m.base;
	rings::check_base(o.base);
	rings::check_step(o.base, o.step, "the step");
	let r = rings::ring(o, m, 1);
	let c = new_canvas();
	let style = i.glyph_style.style;
	rings::draw_symbols(c, r, m, o.step, style);
	rings::draw_middle(c, r, m, [o.step], [style], style);
	c
}

fn example() {
	#{}
}
