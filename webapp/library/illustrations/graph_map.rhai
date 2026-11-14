// graph_map: a Graph drawn as a map of places and the ways between them,
// laid out by forces (graph::layout) on one canvas, width by height: each
// place its glyphs in a row (a dot, if it has none), from the glyphset it
// names or glyph_style's, its name boxed beside them on whichever side is
// clearest, and each way a line between the places' glyphs, with an
// arrowhead where it goes one way and its text set on the line. label_place sets the text "along" the line,
// turned to it, the line breaking for it; "beside" it, above the line; or
// "upright", across the line, which breaks round it.
//
// Its styles are TextStyles, a style source's, each drawn whole:
// name_style writes the places' names, wrapped at name_width, and
// edge_style the ways' texts, wrapped at text_width (both as marked-up
// text); glyph_style draws the glyphs, from its glyphset at its size, and
// the dots, dot across; box_style draws the names' boxes, and line_style
// the lines and arrowheads, whose barbs are arrow long. Unwired, they're
// near-black and red ink pens, the names in plain at name_size and the
// texts in plain at edge_size, each centred and at a steady pressure, and
// the glyphs from symbols at glyph_size.
//
// padding is the room in a name's box round the name, and gap the room
// kept between glyphs, names, texts and lines (half of it between a
// place's glyphs and its name). seed, iterations and generations go to the layout,
// which takes more room than width by height for a graph too crowded for
// it; the map is then scaled down to fit.

import "lib/graph_maps" as maps;

fn options() {
	#{
		width: 1200.0, height: 900.0, label_place: "along", seed: 0, iterations: 300, generations: 200,
		name_width: 260.0, text_width: 320.0, padding: 10.0, gap: 14.0, arrow: 18.0,
		name_size: 30.0, edge_size: 20.0, glyph_size: 36.0, dot: 10.0,
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
	maps::check(o, "graph_map");
	let edges = [];
	let lines = [];
	for e in i.graph.edges {
		edges.push(i.edge_style);
		lines.push(i.line_style);
	}
	maps::draw(i.graph, o, #{
		name_style: i.name_style, glyph_style: i.glyph_style, box_style: i.box_style,
		edge_styles: edges, line_styles: lines,
	}, "graph_map")
}

fn example() {
	#{
		graph: graph::parse("Paper Mazes: basic_eye archangel_3\nSaint's Rest: ploughdiem\n"
			+ "Paper Mazes -> Saint's Rest: Follow the red thread until you find _To Fall Endlessly_, then use the crawlspace beneath the broken shelf\n"
			+ "Saint's Rest -- The Cloister: down the bell stair\nThe Cloister -> Paper Mazes: through the reading room"),
	}
}
