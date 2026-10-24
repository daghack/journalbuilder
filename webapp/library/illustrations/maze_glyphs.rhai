// maze_glyphs: a Maze drawn on a grid of cell by cell: its corridors in
// one ink, and each cell's symbol (0 to 26) as its balanced-ternary glyph,
// shifted to sit round 0 (symbol 13 is the glyph for 0), in another, as a
// maze lock's text is read along its corridors.

fn options() {
	#{ cell: 70.0, corridors: "#9b2318", glyphs: "#2b1d12" }
}

fn inputs(o) {
	#{ maze: "Maze" }
}

fn render(o, i) {
	let c = new_canvas();
	let hand = new_handwriting_style();
	c.add(i.maze.path().scale_by(o.cell), new_style(new_brush_style(o.corridors, "Ink Pen", 1.6), hand));
	let ternary = glyphset("balanced_ternary");
	for cell in i.maze.cells() {
		c.add(ternary.glyph(cell.symbol - 13, #{ scale: o.cell * 0.8, position: vec2(cell.x * o.cell, cell.y * o.cell) }), new_style(new_brush_style(o.glyphs, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand));
	}
	c
}

fn example() {
	let maze = new_maze(3, 3, #{ seed: 4 });
	maze.fill(symbols(27, "HELLO WORLD").symbols());
	#{ maze: maze }
}
