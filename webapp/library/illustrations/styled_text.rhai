// styled_text: text written in a text style, wrapped into a block: a
// chain's content in the hand a style source gives it, drawn whole (its
// pen and hand, its glyphs at its size, its paragraph). One canvas, width
// across, the first line a glyph scale down. Draws text, a String (text
// content, say), in style, a TextStyle.

fn options() {
	#{ width: 900.0 }
}

fn inputs(o) {
	#{ text: "String", style: "TextStyle" }
}

fn render(o, i) {
	let t = i.style.type;
	new_canvas().add(write_block(i.text, vec2(0, (t.glyph_scale ?? 0.0) * t.glyph_scale_factor), o.width, i.style).canvas)
}

// What the editor previews it with.
fn example() {
	#{ text: content("lorem"), style: text_style("plain_hand") }
}
