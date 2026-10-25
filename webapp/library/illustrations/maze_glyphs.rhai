// maze_glyphs: a Maze drawn on a grid of cell by cell: its corridors in
// one style, and each cell's symbol (0 to 26) as its glyph in glyphs (the
// balanced-ternary glyphset unless one's wired in), shifted to sit round 0
// (symbol 13 is the glyph for 0), in another, as a maze lock's text is read
// along its corridors. The styles are TextStyles, a style source's, drawn
// with their pen and hand; unwired, they're ink pens in the corridors and
// glyphs colours.

fn options() {
	#{ cell: 70.0, corridors: "#9b2318", glyphs: "#2b1d12" }
}

fn inputs(o) {
	#{ maze: "Maze", corridor_style: "TextStyle", glyph_style: "TextStyle", glyphs: "Glyphset" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		corridor_style: #{ style: new_style(new_brush_style(o.corridors, "Ink Pen", 1.6), hand) },
		glyph_style: #{ style: new_style(new_brush_style(o.glyphs, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand) },
		glyphs: glyphset("balanced_ternary"),
	}
}

fn render(o, i) {
	let c = new_canvas();
	c.add(i.maze.path().scale_by(o.cell), i.corridor_style.style);
	for cell in i.maze.cells() {
		c.add(i.glyphs.glyph(cell.symbol - 13, #{ scale: o.cell * 0.8, position: vec2(cell.x * o.cell, cell.y * o.cell) }), i.glyph_style.style);
	}
	c
}

fn example() {
	let maze = new_maze(3, 3, #{ seed: 4 });
	maze.fill(symbols(27, "HELLO WORLD").symbols());
	#{ maze: maze }
}
