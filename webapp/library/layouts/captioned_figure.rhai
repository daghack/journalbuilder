// captioned_figure: a figure filling the page, with its caption centred
// beneath it.
//
// Flows: "caption".
// Roles: "caption", and a style for the figure if it's a path.
// Art: "figure".
// Options: the page's width and height.

fn options() {
	#{ width: 1400.0, height: 1080.0 }
}

fn page_size(o) {
	#{ width: o.width, height: o.height }
}

fn page(n, info) {
	column([
		figure("figure", #{ flex: #{ grow: 1 } }),
		flow("caption", #{ flex: #{ margin: [30, 0, 0, 0] }, paragraph: #{ align: "center" } }),
	], #{ flex: #{ padding: [90, 200, 60, 200] } })
}

// A circle, captioned in the library's plain hand.
fn example() {
	#{
		flows: #{ caption: "sed do eiusmod tempor incididunt" },
		roles: #{ caption: text_style("plain_hand", #{ size: 28.0, color: "#9b2318" }), figure: #{ style: text_style("plain_hand").style } },
		art: #{ figure: new_circle(vec2(0, 0), 100) },
	}
}
