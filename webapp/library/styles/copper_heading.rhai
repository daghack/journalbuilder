// copper_heading: headings in the copper glyphset, centred, with a broad
// calligraphy nib at a constant pressure; strong text gone over twice.

fn options() {
	#{ size: 72.0, color: "#9b2318" }
}

fn style(o) {
	#{
		style: new_style(
			new_brush_style(o.color, "Calligraphy", 1.5, #{ min_pressure: 0.48, max_pressure: 0.96, pressure_profile: "constant" }),
			new_handwriting_style(),
		),
		type: new_type_style(glyphset("copper"), o.size),
		paragraph: #{ align: "center", line_height: 1.1 },
		spans: #{ strong: #{ style: #{ passes: 2, pass_end_jitter: 1.0 } } },
	}
}

fn example() {
	"Dolor Sit Amet"
}
