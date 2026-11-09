// graph_map: a Graph drawn as a map of places and the ways between them,
// laid out by forces (graph::layout) on one canvas, width by height: each
// place a box round its name, its glyphs in a row under the name, and
// each way a line between the boxes, with an arrowhead where it goes one
// way and its text set on the line. label_place sets the text "along" the
// line, turned to it, the line breaking for it; "beside" it, above the
// line; or "upright", across the line, which breaks round it.
//
// Its styles are TextStyles, a style source's, each drawn whole:
// name_style writes the places' names, wrapped at name_width, and
// edge_style the ways' texts, wrapped at text_width (both as marked-up
// text); glyph_style draws the glyphs, from its glyphset at its size;
// box_style draws the boxes, and line_style the lines and arrowheads,
// whose barbs are arrow long. Unwired, they're near-black and red ink
// pens, the names in plain at name_size and the texts in plain at
// edge_size, each centred and at a steady pressure, and the glyphs from
// symbols at glyph_size.
//
// padding is the room in a box round what's in it, and gap the room kept
// between boxes, texts and lines. seed and iterations go to the layout,
// which takes more room than width by height for a graph too crowded for
// it; the map is then scaled down to fit.

import "lib/symbol_rings" as rings;

fn options() {
	#{
		width: 1200.0, height: 900.0, label_place: "along", seed: 0, iterations: 300,
		name_width: 260.0, text_width: 320.0, padding: 16.0, gap: 14.0, arrow: 18.0,
		name_size: 30.0, edge_size: 20.0, glyph_size: 36.0,
	}
}

fn inputs(o) {
	#{
		graph: "Graph",
		name_style: "TextStyle", edge_style: "TextStyle", glyph_style: "TextStyle",
		box_style: "TextStyle", line_style: "TextStyle",
	}
}

fn fallbacks(o) {
	let ink = "#2b1d12";
	let red = "#9b2318";
	let hand = new_handwriting_style();
	let pen = |color, width| new_style(new_brush_style(color, "Ink Pen", width), hand);
	// Text at a steady pressure, so its dots and commas show.
	let steady = |color, width| new_style(new_brush_style(color, "Ink Pen", width, #{ min_pressure: 0.5, max_pressure: 0.9, pressure_profile: "constant" }), hand);
	let plain = glyphset("plain");
	let centred = new_paragraph_style(#{ align: "center" });
	#{
		name_style: #{ style: steady.call(ink, 1.6), type: new_type_style(plain, o.name_size), paragraph: centred },
		edge_style: #{ style: steady.call(red, 1.2), type: new_type_style(plain, o.edge_size), paragraph: centred },
		glyph_style: #{ style: pen.call(ink, 1.6), type: new_type_style(glyphset("symbols"), o.glyph_size) },
		box_style: #{ style: pen.call(ink, 1.8) },
		line_style: #{ style: pen.call(ink, 1.4) },
	}
}

fn render(o, i) {
	if !(o.label_place in ["along", "beside", "upright"]) {
		throw `graph_map: label_place should be "along", "beside" or "upright", got "${o.label_place}"`;
	}
	let g = i.graph;
	// Each place's box's contents, its name over its glyphs, about (0, 0),
	// and the box's size.
	let glyphs = i.glyph_style.type.glyphset;
	let known = glyphs.characters();
	let glyph_size = size_of(i.glyph_style.type);
	let contents = [];
	let sizes = [];
	for node in g.nodes {
		let c = centred(write_block(markdown(node.name), vec2(0, 0), o.name_width, i.name_style).canvas);
		if node.glyphs.len() > 0 {
			let row = new_canvas();
			for (name, k) in node.glyphs {
				if !(name in known) {
					throw `graph_map: ${node.name}'s glyph '${name}' isn't one of its glyphset's`;
				}
				row.add(glyphs.glyph(name, #{ scale: glyph_size, position: vec2(k * glyph_size * 1.2, 0) }), i.glyph_style.style);
			}
			row = centred(row);
			let below = c.bounds().max.y + o.padding * 0.5 - row.bounds().min.y;
			c.add(row.translate_by(vec2(0, below)));
			c = centred(c);
		}
		let b = c.bounds();
		contents.push(c);
		sizes.push([b.max.x - b.min.x + 2.0 * o.padding, b.max.y - b.min.y + 2.0 * o.padding]);
	}
	// Each way's text about (0, 0), and its size.
	let labels = [];
	let label_sizes = [];
	for edge in g.edges {
		if edge.text == "" {
			labels.push(());
			label_sizes.push(());
			continue;
		}
		let c = centred(write_block(markdown(edge.text), vec2(0, 0), o.text_width, i.edge_style).canvas);
		let b = c.bounds();
		labels.push(c);
		label_sizes.push([b.max.x - b.min.x, b.max.y - b.min.y]);
	}
	let laid = graph::layout(g, sizes, #{
		labels: label_sizes, label_place: o.label_place, gap: o.gap,
		width: o.width, height: o.height, seed: o.seed, iterations: o.iterations,
	});
	let map = new_canvas();
	for (node, k) in g.nodes {
		let p = point(laid.points[k]);
		map.add(new_rectangle(p, sizes[k][0], sizes[k][1]), i.box_style.style);
		map.add(contents[k].translate_by(p));
	}
	for (edge, e) in laid.edges {
		let lines = new_path();
		for piece in edge.pieces {
			lines.append(new_line(point(piece[0]), point(piece[1])));
		}
		if g.edges[e].directed {
			let end = point(edge.end);
			lines.append(rings::arrowhead(end, (end - point(edge.start)).normalized(), o.arrow));
		}
		map.add(lines, i.line_style.style);
		if g.edges[e].text != "" {
			map.add(labels[e].rotate_around(vec2(0, 0), edge.angle).translate_by(point(edge.label)));
		}
	}
	// A crowded graph took more room: back into width by height.
	let grown = laid.size[0] / o.width;
	if grown > 1.0 {
		map.scale_by(1.0 / grown);
	}
	map
}

fn example() {
	#{
		graph: graph::parse("Paper Mazes: basic_eye archangel_3\nSaint's Rest: ploughdiem\n"
			+ "Paper Mazes -> Saint's Rest: Follow the red thread until you find _To Fall Endlessly_, then use the crawlspace beneath the broken shelf\n"
			+ "Saint's Rest -- The Cloister: down the bell stair\nThe Cloister -> Paper Mazes: through the reading room"),
	}
}

// The size a type sets glyphs at.
fn size_of(t) {
	t.glyph_scale * t.glyph_scale_factor
}

// [x, y] as a vector.
fn point(xy) {
	vec2(xy[0], xy[1])
}

// A canvas moved so what's on it is centred on (0, 0).
fn centred(c) {
	let b = c.bounds();
	c.translate_by(vec2(-b.center.x, -b.center.y))
}
