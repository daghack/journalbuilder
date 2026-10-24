// margin_note: small red notes in the plain glyphset, leant a little, as
// if written in a margin: the showcase's own text style.

fn options() {
	#{ size: 22.0 }
}

fn style(o) {
	#{
		style: new_style(new_brush_style("#9b2318", "Ink Pen", 1.2), new_handwriting_style(#{ glyph_rotation_jitter: 0.04 })),
		type: new_type_style(glyphset("plain"), o.size, #{ italic: true }),
		paragraph: #{ line_height: 1.3 },
	}
}

fn example() {
	"nota bene: lorem ipsum dolor sit amet"
}
