// joined_symbol_ring: two paths joined into one. A base's symbols in a
// ring, as symbol_ring draws them, with two walks round it from the top:
// first's, each line to the symbol first on, and second's, each its own
// colour, and their symbols next to each other in the middle in their
// colours, first on the left. Taking one path and then the other is
// taking the path of their product, first times second round the base: a
// second canvas draws its ring, as symbol_ring does, its walk and middle
// symbol in a third colour, the two joined. Base 13, first 3 and second 5
// join to 15 round 13, 2. A step of 0, first, second or what they join
// to, takes no path: no lines.
//
// Where both walks draw the same line (first and second the same, or
// adding to the base), each is moved a little to its own side, so both
// show. steps, if not 0, draws only the first that many lines of each
// walk, and half the next with an arrow, as symbol_ring does. With
// bordered, each symbol round the ring is ringed; with center_bordered,
// one circle rings the middle's, both of them on the first canvas.
//
// Two canvases, each size across and down, the ring's centre in its
// middle. Its styles are TextStyles, a style source's: glyph_style for
// the ring's symbols and every border, and first_style, second_style and
// joined_style for each walk's dashed lines and middle symbol; every
// symbol's glyphs are glyph_style's, chosen, sized and leant as
// symbol_ring chooses, sizes and leans them, the middle's center_scale
// times the ring's, up to as big as fit in symbol_ring's middle.
// Unwired, they're ink pens, near-black, red, blue and the purple of the
// two, the glyphs loche_upper, as big as the ring holds.

import "lib/symbol_rings" as rings;

fn options() {
	#{
		base: 27, first: 2, second: 5, steps: 0, bordered: true, center_bordered: true, size: 900.0, center_scale: 3.0,
		dash: 14.0, gap: 10.0, glyph_ink: "#2b1d12", first_ink: "#9b2318", second_ink: "#23409b",
		joined_ink: "#5f2a6e",
	}
}

fn inputs(o) {
	#{ glyph_style: "TextStyle", first_style: "TextStyle", second_style: "TextStyle", joined_style: "TextStyle" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		glyph_style: #{
			style: new_style(new_brush_style(o.glyph_ink, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand),
			type: new_type_style(glyphset("loche_upper"), rings::room(o.size, o.base)),
		},
		first_style: #{ style: path_pen(o.first_ink, hand) },
		second_style: #{ style: path_pen(o.second_ink, hand) },
		joined_style: #{ style: path_pen(o.joined_ink, hand) },
	}
}

fn render(o, i) {
	rings::check_base(o.base);
	rings::check_step(o.base, o.first, "first");
	rings::check_step(o.base, o.second, "second");
	rings::check_steps(o.steps);
	let t = i.glyph_style.type;
	let names = rings::symbol_names(t.glyphset, o.base);

	// The two walks, and their symbols next to each other in the middle.
	let r = rings::ring(o, t, 2);
	let first = rings::walk(o.base, o.first, o.steps);
	let second = rings::walk(o.base, o.second, o.steps);
	let common = rings::common(first, second);
	let nudge = r.glyph * 0.12;
	let paths = new_canvas();
	rings::draw_walk(paths, r, first, i.first_style.style, o, nudge, common);
	rings::draw_walk(paths, r, second, i.second_style.style, o, -nudge, common);
	rings::draw_symbols(paths, r, t, names, 1, i.glyph_style.style);
	rings::draw_middle(paths, r, t, [names[o.first], names[o.second]], [i.first_style.style, i.second_style.style], i.glyph_style.style);

	// What they join to: the ring of their product.
	let product = (o.first * o.second) % o.base;
	let r = rings::ring(o, t, 1);
	let joined = new_canvas();
	rings::draw_walk(joined, r, rings::walk(o.base, product, o.steps), i.joined_style.style, o, 0.0, #{});
	rings::draw_symbols(joined, r, t, names, 1, i.glyph_style.style);
	rings::draw_middle(joined, r, t, [names[product]], [i.joined_style.style], i.glyph_style.style);
	[paths, joined]
}

fn example() {
	#{}
}

// The pen a walk's lines and middle symbol are drawn in, unwired.
fn path_pen(ink, hand) {
	new_style(new_brush_style(ink, "Ink Pen", 1.8, #{ min_pressure: 0.5, max_pressure: 0.8 }), hand)
}
