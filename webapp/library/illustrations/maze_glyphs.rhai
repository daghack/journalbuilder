// maze_glyphs: a Maze drawn on a grid of cell by cell: its corridors in
// corridor_style, and each cell's symbol (0 to 26) in glyph_style, as a
// maze lock's text is read along its corridors. The styles are
// TextStyles, a style source's: glyph_style's glyphs are its type's, at
// its size, or smaller if a cell hasn't room (0.8 of one), leant as it
// leans them. In balanced-ternary digits (balanced_ternary's), a symbol
// is its digit, shifted to sit round 0 (symbol 13 is the glyph for 0); in
// any other glyphset, its letter (0 a space, 1 to 26 A to Z). Unwired,
// they're ink pens in the corridors and glyphs colours, the glyphs
// balanced ternary, as big as the cells hold.
//
// symbol_set, a SymbolSet, draws the cells' symbols instead of glyph_style
// when it's wired: each symbol is the set's own (the cell's symbol 0 to 26
// of the set's), at its size or smaller if a cell hasn't room, upright.
// With editor, each cell's symbol is committed to the visual editor as
// EDITOR_N, N the cell's place in walking order, so any can be changed.

fn options() {
	#{ cell: 70.0, corridors: "#9b2318", glyphs: "#2b1d12", editor: "" }
}

fn inputs(o) {
	#{ maze: "Maze", corridor_style: "TextStyle", glyph_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		corridor_style: #{ style: new_style(new_brush_style(o.corridors, "Ink Pen", 1.6), hand) },
		glyph_style: #{
			style: new_style(new_brush_style(o.glyphs, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand),
			type: new_type_style(glyphset("balanced_ternary"), o.cell * 0.8),
		},
		// Unwired, the glyph style draws.
		symbol_set: (),
	}
}

fn render(o, i) {
	let c = new_canvas();
	c.add(i.maze.path().scale_by(o.cell), i.corridor_style.style);
	if i.symbol_set != () {
		let set = i.symbol_set;
		let size = min(set.size, o.cell * 0.8);
		for (cell, k) in i.maze.cells() {
			if cell.symbol >= set.base { throw `maze_glyphs: symbol ${cell.symbol} is outside the symbol set '${set.name}' (0 to ${set.base - 1})`; }
			let mark = write_at(set.text([cell.symbol]), vec2(cell.x * o.cell, cell.y * o.cell), #{ type: #{ glyph_scale: size } });
			if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${k}`); }
			c.add(mark);
		}
		return c;
	}
	let t = i.glyph_style.type;
	let scale = min(t.glyph_scale * t.glyph_scale_factor, o.cell * 0.8);
	let digits = "-13" in t.glyphset.characters();
	for cell in i.maze.cells() {
		let g = if digits { t.glyphset.glyph(cell.symbol - 13) } else { t.glyphset.glyphs(`${" ABCDEFGHIJKLMNOPQRSTUVWXYZ"[cell.symbol]}`)[0] };
		g.scale = scale;
		g.position = vec2(cell.x * o.cell, cell.y * o.cell);
		g.skew = t.skew ?? t.glyphset.skew;
		c.add(g, i.glyph_style.style);
	}
	c
}

fn example() {
	let maze = new_maze(3, 3, #{ seed: 4 });
	maze.fill(symbols(27, "HELLO WORLD").symbols());
	#{ maze: maze }
}
