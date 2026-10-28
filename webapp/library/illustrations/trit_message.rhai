// trit_message: symbols written in balanced-ternary marks, row after row
// on a page under a heading: each symbol shifted to sit round 0 and
// written in as many trits as its base needs (a bar for 0, a hook for -1
// or +1), as star_pages writes a link's digit. A cipher text in base 3
// takes a mark a symbol.
//
// Its styles are TextStyles, a style source's, each drawn whole:
// heading_style writes the title, in its glyphs at its size, and draws
// the rule under it; mark_style draws the marks, reaching its glyph scale
// out from their lines, the rows three of them apart; line_style draws
// the lines. Unwired, they're red and near-black ink pens, the heading in
// loche_upper at heading_size and the marks mark_size.

fn options() {
	#{ base: 3, title: "MESSAGE", per_row: 36, width: 1200.0, heading_size: 44.0, mark_size: 30.0 }
}

fn inputs(o) {
	#{ message: `Base${o.base}Symbol`, heading_style: "TextStyle", mark_style: "TextStyle", line_style: "TextStyle" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		heading_style: #{ style: new_style(new_brush_style("#9b2318", "Ink Pen", 1.6), hand), type: new_type_style(glyphset("loche_upper"), o.heading_size) },
		mark_style: #{ style: new_style(new_brush_style("#2b1d12", "Ink Pen", 1.6), hand), type: #{ glyph_scale: o.mark_size } },
		line_style: #{ style: new_style(new_brush_style("#2b1d12", "Ink Pen", 1.2), hand) },
	}
}

fn render(o, i) {
	let marks = [];
	for s in i.message { marks += trits(s, o.base).digits(); }
	let c = new_canvas();
	let heading = size_of(i.heading_style.type);
	c.add(write_line(o.title, vec2(100, 100), i.heading_style));
	let rule = 100 + heading * 0.75;
	c.add(new_line(vec2(100, rule), vec2(o.width - 100, rule)), i.heading_style.style);
	let reach = size_of(i.mark_style.type);
	let step = (o.width - 200) / o.per_row;
	for row in 0..(marks.len() + o.per_row - 1) / o.per_row {
		let these = marks.extract(row * o.per_row, o.per_row);
		let y = rule + reach * (10.0 / 3.0) + row * reach * 3.0;
		let line = new_line(vec2(100, y), vec2(100 + these.len() * step, y));
		c.add(line, i.line_style.style);
		c.add(line.fit_text(balanced_ternary(these), #{ style: i.mark_style.style, width: reach }));
	}
	c
}

fn example() {
	#{ message: symbols(3, "CABBACCAB").symbols() }
}

// The size a type sets glyphs at.
fn size_of(t) {
	t.glyph_scale * t.glyph_scale_factor
}

// A symbol of `base` as balanced ternary, shifted to sit round 0, in as
// many trits as the base needs: w trits write 3^w numbers.
fn trits(d, base) {
	let w = 1;
	while 3 ** w < base { w += 1; }
	balanced_ternary(d - (base - 1) / 2, #{ width: w })
}
