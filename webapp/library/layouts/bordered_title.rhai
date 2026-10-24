// bordered_title: a title page: the title, a flourish and the credits
// centred in a ruled frame, inside a border of ornamental bands with a
// piece in each corner.
//
// Flows: "title"; "subtitle" and "credits" (lines broken with "\n") if
// given.
// Roles: "title", "subtitle", "credits", "frame" (the frame's rule and
// border), and a style for each piece of art that's a path.
// Art: "border" (the bands' motif); "corner" and "flourish" (under the
// title) if given.
// Options: the page's width and height, and the border's breadth.

fn options() {
	#{ width: 1400.0, height: 1080.0, border: 70.0 }
}

fn page_size(o) {
	#{ width: o.width, height: o.height }
}

fn corner(top, left, breadth) {
	let at = #{ position: "absolute", width: breadth, height: breadth };
	if top { at.top = 0; } else { at.bottom = 0; }
	if left { at.left = 0; } else { at.right = 0; }
	figure("corner", #{ flex: at })
}

fn page(n, info) {
	let breadth = info.options.border;
	let middle = [flow("title", #{ flex: #{ height: 300 }, paragraph: #{ fit: true, align: "center", valign: "center" } })];
	if info.art.contains("flourish") {
		middle.push(figure("flourish", #{ flex: #{ height: 50, margin: [10, "25%", 30, "25%"] } }));
	} else {
		middle.push(rule(#{ role: "frame", flex: #{ height: 20, margin: [10, "30%", 30, "30%"] } }));
	}
	for name in ["subtitle", "credits"] {
		if info.flows.contains(name) {
			middle.push(flow(name, #{ flex: #{ margin: [0, 0, 24, 0] }, paragraph: #{ align: "center" } }));
		}
	}
	let inside = column(middle, #{ flex: #{ grow: 1, justify: "center", padding: [30, 60], border: "frame" } });
	let framed = [
		band("border", #{ flex: #{ height: breadth, shrink: 0 } }),
		row([
			band("border", #{ flex: #{ width: breadth, shrink: 0 } }),
			column([inside], #{ flex: #{ grow: 1, padding: [0, 16] } }),
			band("border", #{ flex: #{ width: breadth, shrink: 0 } }),
		], #{ flex: #{ grow: 1, padding: [16, 0] } }),
		band("border", #{ flex: #{ height: breadth, shrink: 0 } }),
	];
	if info.art.contains("corner") {
		framed += [corner(true, true, breadth), corner(true, false, breadth), corner(false, true, breadth), corner(false, false, breadth)];
	}
	column([column(framed, #{ flex: #{ grow: 1 } })], #{ flex: #{ padding: [70, 150] } })
}

// A title page in the library's text styles, bordered with leaves.
fn example() {
	let red = text_style("plain_hand", #{ color: "#9b2318" });
	#{
		flows: #{ title: "Lorem Ipsum", subtitle: "dolor sit amet, consectetur adipiscing elit", credits: "sed do eiusmod tempor\nincididunt ut labore" },
		roles: #{
			title: text_style("copper_heading", #{ size: 150.0 }),
			subtitle: text_style("plain_hand", #{ size: 34.0 }),
			credits: text_style("plain_hand"),
			frame: #{ style: red.style },
			border: #{ style: text_style("plain_hand").style },
		},
		art: #{ border: new_path().move_to(vec2(0, -22)).quad_to(vec2(14, -4), vec2(0, 22)).quad_to(vec2(-14, -4), vec2(0, -22)) },
	}
}
