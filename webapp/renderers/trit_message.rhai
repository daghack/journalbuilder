// trit_message: symbols written in balanced-ternary marks, row after row
// on a page under a heading: each symbol shifted to sit round 0 and
// written in as many trits as its base needs (a bar for 0, a hook for -1
// or +1), as star_pages writes a link's digit. A cipher text in base 3
// takes a mark a symbol.

fn options() {
	#{ base: 3, title: "MESSAGE", per_row: 36, width: 1200.0 }
}

fn inputs(o) {
	#{ message: `Base${o.base}Symbol` }
}

fn render(o, i) {
	let ink = "#2b1d12";
	let red = "#9b2318";
	let pen = new_brush("Ink Pen", 0.3, 0.7, 1.6);
	let fine = new_style(ink, new_brush("Ink Pen", 0.3, 0.7, 1.2));
	let marks = [];
	for s in i.message { marks += trits(s, o.base).digits(); }
	let c = new_canvas();
	c.add(write_line(o.title, vec2(100, 100), #{ font: font("loche_upper"), style: new_style(red, pen), size: 44 }));
	c.add(new_line(vec2(100, 132), vec2(o.width - 100, 132)), new_style(red, pen));
	let step = (o.width - 200) / o.per_row;
	let row = 0;
	while row * o.per_row < marks.len() {
		let these = marks.extract(row * o.per_row, o.per_row);
		let y = 230 + row * 90;
		let line = new_line(vec2(100, y), vec2(100 + these.len() * step, y));
		c.add(line, fine);
		c.add(line.fit_text(balanced_ternary(these), #{ style: new_style(ink, pen), width: 30 }));
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
