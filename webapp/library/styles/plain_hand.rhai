// plain_hand: easy-reading text in the plain glyphset, written with an
// ink pen by a hand a little unsteady: each glyph a touch off true, each
// line wandering.

fn options() {
	#{ size: 26.0, color: "#2b1d12" }
}

fn style(o) {
	#{
		style: new_style(
			new_brush_style(o.color, "Ink Pen", 1.4, #{ min_pressure: 0.4, max_pressure: 0.8 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_position_jitter: o.size * 0.02, baseline_wander: o.size * 0.08 }),
		),
		type: new_type_style(glyphset("plain"), o.size),
		paragraph: #{ line_height: 1.4 },
	}
}

fn example() {
	content("lorem")
}
