// steiner_wheels: points drawn as little wheels, a wheel to every points
// of them, row after row: each wheel a ring with a tick at each of the
// base's places, clockwise from the longer tick at the top, where point 0
// is; the wheel's points each a dot at its place, and joined up, two by a
// line and three by a triangle. A point over again is its dot alone.
// What steiner_triples writes a letter as: of the thirteen points' 26
// triples, a triangle is a letter outright, and a line is two corners of
// one, to be made up from the triples.
//
// labels, a String, writes a character under each wheel, the first under
// the first: the alphabet under the 26 triples is the key to read the
// rest by. The wheels are size across, per_row to a row, gap apart, the
// rows row_gap apart (and the labels' room, where there are any). One
// canvas, its corner at the first wheel's.
//
// Its styles are TextStyles, a style source's, each drawn whole:
// ring_style draws the rings and ticks, line_style the dots and the
// lines joining them, and label_style writes the labels, in its glyphs
// at its size. Unwired, they're near-black and red ink pens, the labels
// in plain at label_size.
//
// The whole symbol set can be passed in too: symbol_set, a SymbolSet
// whose symbol p (the first base of them) is written inside each wheel at
// place p, mark_size across (a tenth of the wheel if that's 0), so the
// points a wheel joins are read off it as symbols. Unwired, the wheels
// are unmarked.

import "lib/digit_rows" as rows;

fn options() {
	#{
		base: 13, points: 3, per_row: 9, size: 100.0, gap: 24.0, row_gap: 24.0, label_size: 28.0, mark_size: 0.0,
		ring_ink: "#2b1d12", line_ink: "#9b2318",
	}
}

fn inputs(o) {
	#{ points: `Base${o.base}Symbol`, labels: "String", ring_style: "TextStyle", line_style: "TextStyle", label_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	let steady = #{ min_pressure: 0.4, max_pressure: 0.7, pressure_profile: "constant" };
	#{
		labels: "", symbol_set: (),
		ring_style: #{ style: new_style(new_brush_style(o.ring_ink, "Ink Pen", 1.0, steady), hand) },
		line_style: #{ style: new_style(new_brush_style(o.line_ink, "Ink Pen", 1.8, steady), hand) },
		label_style: #{ style: new_style(new_brush_style(o.ring_ink, "Ink Pen", 1.6, steady), hand), type: new_type_style(glyphset("plain"), o.label_size) },
	}
}

fn render(o, i) {
	if o.base < 3 || o.base > 27 { throw `the base should be from 3 to 27, got ${o.base}`; }
	if o.points < 1 { throw `points should be 1 or more, got ${o.points}`; }
	if i.points.len() % o.points != 0 {
		throw `${i.points.len()} points aren't a whole number of wheels, ${o.points} points each`;
	}
	let labels = [];
	for ch in i.labels.chars() { labels.push(ch); }
	let label = i.label_style.type;
	let label_room = if labels.is_empty() { 0.0 } else { label.glyph_scale * label.glyph_scale_factor * 1.5 };
	let layout = #{ per_row: o.per_row, group: 0, gap: o.gap, group_gap: 0.0, row_gap: o.row_gap + label_room };
	rows::check(layout);
	let radius = o.size * 0.42;
	let rings = new_path();
	let lines = new_path();
	let c = new_canvas();
	let set = i.symbol_set;
	if set != () && set.base < o.base { throw `steiner_wheels: the symbol set '${set.name}' has ${set.base} symbols, fewer than the base, ${o.base}`; }
	let mark = if o.mark_size > 0.0 { o.mark_size } else { o.size * 0.1 };
	for (at, k) in rows::places(i.points.len() / o.points, o.size, o.size, layout) {
		rings.append(new_circle(at, radius));
		for p in 0..o.base {
			let way = toward(p, o.base);
			rings.append(new_line(at + way * radius * (if p == 0 { 0.8 } else { 1.0 }), at + way * radius * (if p == 0 { 1.19 } else { 1.1 })));
		}
		if set != () {
			for p in 0..o.base {
				c.add(write_at(set.text([p]), at + toward(p, o.base) * radius * 0.7, #{ type: #{ glyph_scale: mark } }));
			}
		}
		// Its points, each once, in the order given.
		let corners = [];
		for p in i.points.extract(k * o.points, o.points) {
			let corner = at + toward(p, o.base) * radius;
			if !(corner in corners) { corners.push(corner); }
		}
		for corner in corners {
			let dot = new_circle(corner, o.size * 0.045);
			lines.append(dot).append(dot.hatch(o.size * 0.02));
		}
		if corners.len() > 1 { lines.append(new_polyline(corners, #{ closed: corners.len() > 2 })); }
		if k < labels.len() {
			let written = write_line(`${labels[k]}`, vec2(0, 0), i.label_style);
			let b = written.bounds();
			c.add(written.translate_by(vec2(at.x - b.center.x, at.y + o.size / 2.0 + label_room * 0.5 - b.center.y)));
		}
	}
	c.add(rings, i.ring_style.style).add(lines, i.line_style.style)
}

fn example() {
	// MASK, a triangle a letter: the triples 12 0 3, 0 1 4, 5 7 12 and
	// 10 11 1.
	#{ points: [12, 0, 3, 0, 1, 4, 5, 7, 12, 10, 11, 1], labels: "MASK" }
}

// Which way place p of count is from a wheel's middle, clockwise from
// the top.
fn toward(p, count) {
	let a = -PI() / 2.0 + 2.0 * PI() * p / count;
	vec2(a.cos(), a.sin())
}
