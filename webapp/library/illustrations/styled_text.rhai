// styled_text: text written in a text style, wrapped into a block: a
// chain's content in the hand a style source gives it. One canvas, width
// across. Draws text, a String (text content, say), in style, a TextStyle.

fn options() {
	#{ width: 900.0 }
}

fn inputs(o) {
	#{ text: "String", style: "TextStyle" }
}

fn render(o, i) {
	let size = i.style.type.glyph_scale ?? 30.0;
	new_canvas().add(write_block(i.text, vec2(0, size), o.width, i.style).canvas)
}

// What the editor previews it with.
fn example() {
	#{ text: content("lorem"), style: text_style("plain_hand") }
}
