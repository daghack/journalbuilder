// star_pages: a StarChart drawn page by page, each page its own canvas:
// its constellation's name over it, each star's mark (a corona from
// large_star across, rayed below), each link carrying its digit in
// balanced-ternary marks (a bar for 0, a hook for -1 or +1), each star
// numbered on its page in balanced ternary at its label, and an index of
// the numbers to the stars' names in the bottom right corner. Leaves (a
// letter's star) are in red.
//
// The canvases are page-sized, width by height; the chart's own sky (the
// box star_chart fitted each constellation to) starts at sky_x, sky_y.
// The star_chart's label_size should be digit_style's size, so the
// numbers fit the room it left them.
//
// Its styles are TextStyles, a style source's, each drawn whole:
// heading_style writes the constellation's name and the index's heading
// (at 0.64 of its size) and draws the rule under the name; line_style
// draws the links and stars, the links' marks reaching its glyph scale
// out from them, and leaf_style the leaves; digit_style writes the
// numbers (in the index at 0.9 of its size; in balanced-ternary digits,
// a glyph to three trits, or else in decimal), name_style the index's
// names, and leaf_name_style the leaves'. The index's rows and columns are
// spaced by the numbers' and names' sizes. Unwired, they're near-black and
// red ink pens, the heading in loche_upper at heading_size, the numbers in
// balanced_ternary at digit_size and the names in plain at name_size.

fn options() {
	#{
		width: 1200.0, height: 1600.0, sky_x: 100.0, sky_y: 180.0,
		large_star: 33.0, large_shape: "corona", small_shape: "rayed",
		heading_size: 44.0, digit_size: 24.0, name_size: 22.0, index_x: 600.0, index_y: 1110.0,
	}
}

fn inputs(o) {
	#{
		chart: "StarChart",
		heading_style: "TextStyle", line_style: "TextStyle", leaf_style: "TextStyle",
		digit_style: "TextStyle", name_style: "TextStyle", leaf_name_style: "TextStyle",
	}
}

fn fallbacks(o) {
	let ink = "#2b1d12";
	let red = "#9b2318";
	let hand = new_handwriting_style();
	let pen = |color, width| new_style(new_brush_style(color, "Ink Pen", width), hand);
	let names = glyphset("plain");
	#{
		heading_style: #{ style: pen.call(red, 1.6), type: new_type_style(glyphset("loche_upper"), o.heading_size) },
		line_style: #{ style: pen.call(ink, 1.6), type: #{ glyph_scale: 22.0 } },
		leaf_style: #{ style: pen.call(red, 1.6) },
		digit_style: #{ style: pen.call(ink, 1.2), type: new_type_style(glyphset("balanced_ternary"), o.digit_size) },
		name_style: #{ style: pen.call(ink, 1.2), type: new_type_style(names, o.name_size) },
		leaf_name_style: #{ style: pen.call(red, 1.2), type: new_type_style(names, o.name_size) },
	}
}

fn render(o, i) {
	let chart = i.chart;
	let line_style = i.line_style.style;
	let leaf_style = i.leaf_style.style;
	let corner = vec2(o.sky_x, o.sky_y);
	let heading = size_of(i.heading_style.type);
	let index_heading = i.heading_style;
	index_heading.type.glyph_scale_factor *= 0.64;
	let reach = size_of(i.line_style.type);
	let digit = size_of(i.digit_style.type);
	let name = size_of(i.name_style.type);
	// The index: a row a star, its number and then its name, the rows
	// as far apart as the larger of them needs.
	let pitch = max(digit, name) * (5.0 / 3.0);
	let name_x = digit * 2.25;
	let column = name_x + name * 9.4;
	// A star is a letter's if it's no link's parent on any page.
	let parents = #{};
	for page in chart.pages {
		for link in page.links { parents[page.stars[link[0]].name] = true; }
	}
	let canvases = [];
	for page in chart.pages {
		let c = new_canvas();
		c.add(write_line(page.name.to_upper(), vec2(100, 100), i.heading_style));
		let rule = 100 + heading * 0.73;
		c.add(new_line(vec2(100, rule), vec2(o.width - 100, rule)), i.heading_style.style);
		for link in page.links {
			let from = page.stars[link[0]];
			let to = page.stars[link[1]];
			let a = vec2(from.point[0], from.point[1]) + corner;
			let b = vec2(to.point[0], to.point[1]) + corner;
			let along = (b - a).normalized();
			let edge = new_line(a + along * (from.size + 8), b - along * (to.size + 8));
			c.add(edge, line_style);
			// The link's own digit, in trits down it from parent to child.
			c.add(edge.fit_text(trits(link[2], chart.output_base), #{ style: line_style, width: reach }));
		}
		for (star, k) in page.stars {
			let shape = if star.size >= o.large_star { o.large_shape } else { o.small_shape };
			let style = if star.name in parents { line_style } else { leaf_style };
			c.add(star_mark(shape, vec2(star.point[0], star.point[1]) + corner, star.size / reach_of(shape)), style);
			c.add(index_glyphs(i.digit_style, k, vec2(star.label[0], star.label[1]) + corner, 1.0));
		}
		// The index: each star's number on this page, and its name.
		c.add(write_line("INDEX", vec2(o.index_x, o.index_y), index_heading));
		let rows = (page.stars.len() + 1) / 2;
		for (star, k) in page.stars {
			let cx = o.index_x + (k / rows) * column;
			let row_y = o.index_y + pitch * 1.3 + (k % rows) * pitch;
			c.add(index_glyphs(i.digit_style, k, vec2(cx + digit * (2.0 / 3.0), row_y), 0.9));
			let names = if star.name in parents { i.name_style } else { i.leaf_name_style };
			c.add(write_line(star.name, vec2(cx + name_x, row_y), names));
		}
		canvases.push(c);
	}
	canvases
}

fn example() {
	let lock = new_lock("example").add("sky", "star_chart", #{ input_base: 27, output_base: 3, most_stars: 9 });
	let code = huffman::code(symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), 3);
	#{ chart: lock.run(#{ "sky.code": code }).get("sky.chart").value() }
}

// A star's mark, filled by hatching, about r across its body.
//   celestial: a compass of four long points and four short, ringed,
//     with rays between the points.
//   corona: a disc in a ring, four long points out through it and short
//     rays between.
//   rayed: a disc and eight rays round it, long and short.
//   orrery: a disc, a ring, and eight rays long and short, a thin
//     compass laid over them.
fn star_mark(shape, at, r) {
	let up = -PI() / 2.0;
	let filled = |outline| { let m = outline.copy(); m.append(outline.hatch(2.0, #{ angle: 0.7 })); m };
	let rays = |from, long, short, offset| {
		let m = new_path();
		for k in 0..8 {
			let dir = vec2(1, 0).rotated(offset + k * PI() / 4.0);
			m.append(new_line(at + dir * r * from, at + dir * r * (if k % 2 == 0 { long } else { short })));
		}
		m
	};
	switch shape {
		"celestial" => {
			// Points outside the ring only, a disc within it.
			let ring = new_circle(at, r * 0.95);
			let m = filled.call(new_star(at, r * 2.1, r * 0.32, 4, #{ angle: up })).erase_inside(ring);
			m.append(new_star(at, r * 1.4, r * 0.3, 4, #{ angle: up + PI() / 4.0 }).erase_inside(ring));
			m.append(ring);
			m.append(filled.call(new_circle(at, r * 0.45)));
			let between = new_path();
			for k in 0..8 {
				let dir = vec2(1, 0).rotated(up + PI() / 8.0 + k * PI() / 4.0);
				between.append(new_line(at + dir * r * 1.15, at + dir * r * 1.4));
			}
			m.append(between);
			m
		},
		"corona" => {
			let m = filled.call(new_circle(at, r * 0.5));
			m.append(new_circle(at, r * 0.9));
			m.append(filled.call(new_star(at, r * 2.2, r * 0.22, 4, #{ angle: up })));
			let between = new_path();
			for k in 0..4 {
				let dir = vec2(1, 0).rotated(up + PI() / 4.0 + k * PI() / 2.0);
				between.append(new_line(at + dir * r * 1.1, at + dir * r * 1.55));
			}
			m.append(between);
			m
		},
		"orrery" => {
			let m = filled.call(new_circle(at, r * 0.45));
			m.append(new_circle(at, r * 0.8));
			m.append(rays.call(1.0, 2.0, 1.4, up));
			m.append(new_star(at, r * 2.0, r * 0.18, 4, #{ angle: up }));
			m
		},
		"rayed" => {
			let m = filled.call(new_circle(at, r * 0.7));
			m.append(rays.call(1.0, 1.9, 1.3, up));
			m
		},
		"sparkle" => filled.call(new_star(at, r * 2.0, r * 0.35, 4, #{ angle: up })),
		"star" => filled.call(new_star(at, r * 1.6, r * 0.65, 5, #{ angle: up })),
		_ => filled.call(new_circle(at, r)),
	}
}

// How far a star's mark reaches, over its r.
fn reach_of(shape) {
	switch shape { "rayed" => 1.9, "celestial" => 2.1, "corona" => 2.2, "orrery" => 2.0, "sparkle" => 2.0, "star" => 1.6, _ => 1.0 }
}


// The size a type sets glyphs at.
fn size_of(t) {
	t.glyph_scale * t.glyph_scale_factor
}

// n, centred on `at`, in a TextStyle's glyphs at `share` of its size,
// drawn in its pen: in balanced-ternary digits (balanced_ternary's), n in
// balanced ternary, a glyph to every three digits; in any other glyphset,
// n's decimal digits, written as the style writes.
fn index_glyphs(text, n, at, share) {
	let t = text.type;
	let size = size_of(t) * share;
	if !("-13" in t.glyphset.characters()) {
		let style = text;
		style.type.glyph_scale_factor *= share;
		let line = write_line(`${n}`, at, style);
		return line.translate_by(vec2(at.x - line.bounds().center.x, 0));
	}
	let chunks = balanced_ternary(n).digits(3);
	let step = size * 0.8;
	let c = new_canvas();
	for (chunk, k) in chunks {
		let x = (k - (chunks.len() - 1) / 2.0) * step;
		c.add(t.glyphset.glyph(balanced_ternary(chunk), #{ scale: size, position: at + vec2(x, 0), skew: t.skew ?? t.glyphset.skew }), text.style);
	}
	c
}

// A digit d of a base's code as balanced ternary: shifted to sit round 0
// (in base 3, 0, 1 and 2 are -1, 0 and +1; in base 27, -13 to 13), in as
// many trits as the base needs, so every link carries as many marks.
fn trits(d, base) {
	let w = 1;
	while 3 ** w < base { w += 1; }
	balanced_ternary(d - (base - 1) / 2, #{ width: w })
}
