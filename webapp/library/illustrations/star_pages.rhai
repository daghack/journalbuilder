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
//
// Its styles are TextStyles, a style source's, drawn with their pen and
// hand: line_style for links and stars, leaf_style for headings and
// leaves, and fine_style and fine_leaf_style for the numbers and the
// index. Headings are in heading_glyphs, the index's names in name_glyphs
// and the numbers in digit_glyphs. Unwired, they're near-black and red
// ink pens, loche_upper, plain and balanced_ternary.

fn options() {
	#{
		width: 1200.0, height: 1600.0, sky_x: 100.0, sky_y: 180.0,
		large_star: 33.0, large_shape: "corona", small_shape: "rayed",
		digit: 24.0, index_x: 600.0, index_y: 1110.0,
	}
}

fn inputs(o) {
	#{
		chart: "StarChart",
		line_style: "TextStyle", leaf_style: "TextStyle", fine_style: "TextStyle", fine_leaf_style: "TextStyle",
		heading_glyphs: "Glyphset", name_glyphs: "Glyphset", digit_glyphs: "Glyphset",
	}
}

fn fallbacks(o) {
	let ink = "#2b1d12";
	let red = "#9b2318";
	let hand = new_handwriting_style();
	#{
		line_style: #{ style: new_style(new_brush_style(ink, "Ink Pen", 1.6), hand) },
		leaf_style: #{ style: new_style(new_brush_style(red, "Ink Pen", 1.6), hand) },
		fine_style: #{ style: new_style(new_brush_style(ink, "Ink Pen", 1.2), hand) },
		fine_leaf_style: #{ style: new_style(new_brush_style(red, "Ink Pen", 1.2), hand) },
		heading_glyphs: glyphset("loche_upper"),
		name_glyphs: glyphset("plain"),
		digit_glyphs: glyphset("balanced_ternary"),
	}
}

fn render(o, i) {
	let chart = i.chart;
	let line_style = i.line_style.style;
	let mark_style = i.leaf_style.style;
	let fine = i.fine_style.style;
	let fine_red = i.fine_leaf_style.style;
	let corner = vec2(o.sky_x, o.sky_y);
	// A star is a letter's if it's no link's parent on any page.
	let parents = #{};
	for page in chart.pages {
		for link in page.links { parents[page.stars[link[0]].name] = true; }
	}
	let canvases = [];
	for page in chart.pages {
		let c = new_canvas();
		c.add(write_line(page.name.to_upper(), vec2(100, 100), #{ style: mark_style, type: new_type_style(i.heading_glyphs, 44) }));
		c.add(new_line(vec2(100, 132), vec2(o.width - 100, 132)), mark_style);
		for link in page.links {
			let from = page.stars[link[0]];
			let to = page.stars[link[1]];
			let a = vec2(from.point[0], from.point[1]) + corner;
			let b = vec2(to.point[0], to.point[1]) + corner;
			let along = (b - a).normalized();
			let edge = new_line(a + along * (from.size + 8), b - along * (to.size + 8));
			c.add(edge, line_style);
			// The link's own digit, in trits down it from parent to child.
			c.add(edge.fit_text(trits(link[2], chart.output_base), #{ style: line_style, width: 22 }));
		}
		for (star, k) in page.stars {
			let shape = if star.size >= o.large_star { o.large_shape } else { o.small_shape };
			let style = if star.name in parents { line_style } else { mark_style };
			c.add(star_mark(shape, vec2(star.point[0], star.point[1]) + corner, star.size / reach_of(shape)), style);
			for g in index_glyphs(i.digit_glyphs, k, vec2(star.label[0], star.label[1]) + corner, o.digit) { c.add(g, fine); }
		}
		// The index: each star's number on this page, and its name.
		c.add(write_line("INDEX", vec2(o.index_x, o.index_y), #{ style: mark_style, type: new_type_style(i.heading_glyphs, 28) }));
		let rows = (page.stars.len() + 1) / 2;
		for (star, k) in page.stars {
			let cx = o.index_x + (k / rows) * 260;
			let row_y = o.index_y + 52 + (k % rows) * 40;
			for g in index_glyphs(i.digit_glyphs, k, vec2(cx + 16, row_y), o.digit * 0.9) { c.add(g, fine); }
			let style = if star.name in parents { fine } else { fine_red };
			c.add(write_line(star.name, vec2(cx + 54, row_y), #{ style: style, type: new_type_style(i.name_glyphs, 22) }));
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


// n in balanced ternary, a glyph of `glyphs` to every three digits, centred on `at`.
fn index_glyphs(glyphs, n, at, size) {
	let chunks = balanced_ternary(n).digits(3);
	let step = size * 0.8;
	let out = [];
	for (chunk, k) in chunks {
		let x = (k - (chunks.len() - 1) / 2.0) * step;
		out.push(glyphs.glyph(balanced_ternary(chunk), #{ scale: size, position: at + vec2(x, 0) }));
	}
	out
}

// A digit d of a base's code as balanced ternary: shifted to sit round 0
// (in base 3, 0, 1 and 2 are -1, 0 and +1; in base 27, -13 to 13), in as
// many trits as the base needs, so every link carries as many marks.
fn trits(d, base) {
	let w = 1;
	while 3 ** w < base { w += 1; }
	balanced_ternary(d - (base - 1) / 2, #{ width: w })
}
