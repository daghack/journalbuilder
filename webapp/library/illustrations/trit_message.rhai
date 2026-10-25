// trit_message: symbols written in balanced-ternary marks, row after row
// on a page under a heading: each symbol shifted to sit round 0 and
// written in as many trits as its base needs (a bar for 0, a hook for -1
// or +1), as star_pages writes a link's digit. A cipher text in base 3
// takes a mark a symbol.
//
// The heading and its rule are in heading_style, the marks in mark_style
// and the lines they sit on in line_style: TextStyles, a style source's,
// drawn with their pen and hand. The heading's glyphs are heading_glyphs.
// Unwired, they're red and near-black ink pens and loche_upper.

fn options() {
	#{ base: 3, title: "MESSAGE", per_row: 36, width: 1200.0 }
}

fn inputs(o) {
	#{
		message: `Base${o.base}Symbol`,
		heading_style: "TextStyle", mark_style: "TextStyle", line_style: "TextStyle",
		heading_glyphs: "Glyphset",
	}
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		heading_style: #{ style: new_style(new_brush_style("#9b2318", "Ink Pen", 1.6), hand) },
		mark_style: #{ style: new_style(new_brush_style("#2b1d12", "Ink Pen", 1.6), hand) },
		line_style: #{ style: new_style(new_brush_style("#2b1d12", "Ink Pen", 1.2), hand) },
		heading_glyphs: glyphset("loche_upper"),
	}
}

fn render(o, i) {
	let heading = i.heading_style.style;
	let fine = i.line_style.style;
	let marks = [];
	for s in i.message { marks += trits(s, o.base).digits(); }
	let c = new_canvas();
	c.add(write_line(o.title, vec2(100, 100), #{ style: heading, type: new_type_style(i.heading_glyphs, 44) }));
	c.add(new_line(vec2(100, 132), vec2(o.width - 100, 132)), heading);
	let step = (o.width - 200) / o.per_row;
	for row in 0..(marks.len() + o.per_row - 1) / o.per_row {
		let these = marks.extract(row * o.per_row, o.per_row);
		let y = 230 + row * 90;
		let line = new_line(vec2(100, y), vec2(100 + these.len() * step, y));
		c.add(line, fine);
		c.add(line.fit_text(balanced_ternary(these), #{ style: i.mark_style.style, width: 30 }));
	}
	c
}

fn example() {
	#{ message: symbols(3, "CABBACCAB").symbols() }
}

// A symbol of `base` as balanced ternary, shifted to sit round 0, in as
// many trits as the base needs: w trits write 3^w numbers.
fn trits(d, base) {
	let w = 1;
	while 3 ** w < base { w += 1; }
	balanced_ternary(d - (base - 1) / 2, #{ width: w })
}
