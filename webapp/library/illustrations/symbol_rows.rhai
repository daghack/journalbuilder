// symbol_rows: symbols written as the marks of a symbol set, row after row,
// each in a square its size across: a cipher text set out plainly, or a
// moon, an eye or a star a symbol. The squares are per_row to a row, gap
// apart, the rows row_gap apart; with group, the marks of a row are in
// groups of that many (a letter's, say), group_gap more between them. size
// is the set's own unless given. One canvas, its corner at the first
// square's. Draws symbols (a lock value of symbols, each a symbol of the
// set) in symbol_set, a SymbolSet; unwired, the thirteen priests. The
// marks are in the set's own styles, or with restyle_set in glyph_style's
// pen (unwired, an ink pen in glyph_ink).
//
// With editor, each mark is committed to the visual editor as
// EDITOR_0, EDITOR_1 and on, so any may be changed there; the editor's
// names are the entry's own, so two uses in one entry need two editors.

import "lib/digit_rows" as rows;

fn options() {
	#{ per_row: 12, group: 0, size: 0.0, gap: 14.0, group_gap: 30.0, row_gap: 22.0, editor: "", restyle_set: false, glyph_ink: "#2b1d12" }
}

fn inputs(o) {
	#{ symbols: "Any", symbol_set: "SymbolSet", glyph_style: "TextStyle" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		symbol_set: symbol_set("priests"),
		glyph_style: #{ style: new_style(new_brush_style(o.glyph_ink, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand) },
	}
}

fn render(o, i) {
	rows::check(o);
	let set = i.symbol_set;
	let size = if o.size > 0.0 { o.size } else { set.size };
	let marks = set.text(i.symbols);
	let settings = #{ type: #{ glyph_scale: size } };
	if o.restyle_set { settings.style = i.glyph_style.style; }
	let c = new_canvas();
	for (at, k) in rows::places(i.symbols.len(), size, size, o) {
		let mark = write_at(set.text([i.symbols[k]]), at, settings);
		if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${k}`); }
		c.add(mark);
	}
	c
}

fn example() {
	#{ symbols: [2, 6, 1, 0, 11, 4, 3, 2, 8, 1, 12, 5, 4, 0] }
}
