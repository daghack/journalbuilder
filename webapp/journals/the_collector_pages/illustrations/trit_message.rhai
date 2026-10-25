// trit_message: symbols written in balanced-ternary marks, row after row
// on a page under a heading: each symbol shifted to sit round 0 and
// written in as many trits as its base needs (a bar for 0, a hook for -1
// or +1), as star_pages writes a link's digit. A cipher text in base 3
// takes a mark a symbol.
//
// The heading is in heading_style, its rule in rule_style, the marks in
// mark_style and the lines they sit on in line_style: TextStyles, a style
// source's, drawn with their pen and hand. The heading's glyphs are
// heading_glyphs. Unwired, they're brush_name pens in base_color and
// emph_color, the heading gone over four times, and header_font.

fn options() {
	#{ base: 3, title: "MESSAGE", per_row: 36, width: 1200.0,
      base_color: "#001d12",
      emph_color: "#9b2318",
      brush_name: "Ink Pen",
      header_font: "loche_upper",
    }
}

fn inputs(o) {
	#{
		message: `Base${o.base}Symbol`,
		heading_style: "TextStyle", rule_style: "TextStyle", mark_style: "TextStyle", line_style: "TextStyle",
		heading_glyphs: "Glyphset",
	}
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	let red_pen = new_style(new_brush_style(o.emph_color, o.brush_name, 1.6), hand);
	let scored = red_pen;
	scored.passes = 4;
	scored.pass_end_jitter = 1;
	scored.stroke_point_jitter = 1;
	#{
		heading_style: #{ style: scored },
		rule_style: #{ style: red_pen },
		mark_style: #{ style: new_style(new_brush_style(o.base_color, o.brush_name, 1.6), hand) },
		line_style: #{ style: new_style(new_brush_style(o.base_color, o.brush_name, 1.2), hand) },
		heading_glyphs: glyphset(o.header_font),
	}
}

fn render(o, i) {
	let fine = i.line_style.style;
	let marks = [];
	for s in i.message { marks += trits(s, o.base).digits(); }
	let c = new_canvas();
	c.add(write_line(o.title, vec2(100, 100), #{ style: i.heading_style.style, type: new_type_style(i.heading_glyphs, 44) }));
	c.add(new_line(vec2(100, 132), vec2(o.width - 100, 132)), i.rule_style.style);
	let step = (o.width - 200) / o.per_row;
	let row = 0;
	while row * o.per_row < marks.len() {
		let these = marks.extract(row * o.per_row, o.per_row);
		let y = 230 + row * 90;
		let line = new_line(vec2(100, y), vec2(100 + these.len() * step, y));
		c.add(line, fine);
		c.add(line.fit_text(balanced_ternary(these), #{ style: i.mark_style.style, width: 30 }));
		row += 1;
	}
	c
}

fn example() {
	#{ message: symbols(3, "CABBACCAB").symbols() }
}

// A symbol of `base` as balanced ternary, shifted to sit round 0, in as
// many trits as the base needs.
fn trits(d, base) {
	let most = base - 1 - (base - 1) / 2;
	let w = 1;
	let reach = 1;
	while reach < most { w += 1; reach = reach * 3 + 1; }
	balanced_ternary(balanced_ternary(d - (base - 1) / 2).digits(w)[0])
}
