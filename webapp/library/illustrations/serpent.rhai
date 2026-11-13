// serpent: symbols written along a snake's back, from behind its head to
// where its tail thins, each turned to the way its body runs: a text to
// read by following the snake. The snake (art::snake) winds from its
// head, on the left, to the tip of its tail, winding times as long as
// the way between them, as long as its text needs and no longer than
// width holds; seed picks its winding. With per_snake, the text is that
// many symbols to a snake, one snake under another, row_gap apart, each
// read head to tail and the snakes top to bottom.
//
// One canvas, width across at most, its corner at the first snake's.
// snake_style, a TextStyle, a style source's, draws the snakes; the
// symbols are symbol_set's, a SymbolSet, glyph_size across (the set's own
// size if that's 0), or smaller if width hasn't room for a snake that long.
// Unwired, a near-black ink pen, and the fine letters set in red. With editor,
// each symbol is committed to the visual editor as EDITOR_ROW_N.

import "lib/symbol_rings" as rings;

fn options() {
	#{
		base: 27, width: 1000.0, per_snake: 0, winding: 1.3, seed: 0, row_gap: 30.0, glyph_size: 30.0, editor: "",
		snake_ink: "#2b1d12", glyph_ink: "#9b2318",
	}
}

fn inputs(o) {
	#{ message: `Base${o.base}Symbol`, snake_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		snake_style: #{ style: new_style(new_brush_style(o.snake_ink, "Ink Pen", 1.6, #{ min_pressure: 0.4, max_pressure: 0.7, pressure_profile: "constant" }), hand) },
		symbol_set: symbol_set("fine_letters", #{ color: o.glyph_ink }),
	}
}

fn render(o, i) {
	rings::check_base(o.base);
	if o.per_snake < 0 { throw `per_snake should be 0 (one snake) or more, got ${o.per_snake}`; }
	if o.winding < 1.1 || o.winding > 2.0 { throw `winding should be from 1.1 to 2, got ${o.winding}`; }
	let set = i.symbol_set;
	if set.base < o.base { throw `serpent: the symbol set '${set.name}' has ${set.base} symbols, fewer than the base, ${o.base}`; }
	let each = if o.per_snake > 0 { o.per_snake } else { max(1, i.message.len()) };
	let c = new_canvas();
	let y = 0.0;
	let row = 0;
	while row == 0 || row * each < i.message.len() {
		let text = i.message.extract(row * each, each);
		let snake = written(o, i, set, text, o.seed + row, row);
		let b = snake.bounds();
		c.add(snake.translate_by(vec2(-b.min.x, y - b.min.y)));
		y += b.max.y - b.min.y + o.row_gap;
		row += 1;
	}
	c
}

fn example() {
	#{ message: symbols(27, "IT SWALLOWS ITS OWN TAIL").symbols() }
}

// How far apart symbols are along the snake, over their size; how much of
// its back, from behind its head, they're written on, the tail left bare;
// and how wide the snake is, over their size.
fn pitch() { 1.2 }
fn written_part() { 0.78 }
fn girth() { 1.9 }

// A snake with `text` along its back, its head at the left: as long as
// the text needs at the type's size, or as long as o.width holds, the
// symbols then as big as fit.
fn written(o, i, set, text, seed, row) {
	let count = max(text.len(), 4);
	let size = if o.glyph_size > 0.0 { o.glyph_size } else { set.size };
	let length = (count + 1) * size * pitch() / written_part();
	// What's across is the way from head to tail, and the head and the
	// body's width beyond them.
	let room = (o.width - size * girth() * 2.0) * o.winding;
	if length > room {
		size *= room / length;
		length = room;
	}
	let snake = art::snake(vec2(0, 0), vec2(length / o.winding, 0), length, #{ max_width: size * girth(), seed: seed, head_angle: PI() });
	let c = new_canvas().add(snake.outline, i.snake_style.style);
	// A place for each symbol, and for the bare tail's.
	let places = snake.spine.points_along(((count + 1) / written_part()).ceiling().to_int() + 1);
	for (s, k) in text {
		let mark = write_at(set.text([s]), places[k + 1].position, #{ type: #{ glyph_scale: size }, turn: places[k + 1].angle });
		if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${row}_${k}`); }
		c.add(mark);
	}
	c
}
