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

fn page_size() {
	#{ width: 1400, height: 1080 }
}

fn corner(top, left) {
	let at = #{ position: "absolute", width: 70, height: 70 };
	if top { at.top = 0; } else { at.bottom = 0; }
	if left { at.left = 0; } else { at.right = 0; }
	figure("corner", #{ flex: at })
}

fn page(n, info) {
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
		band("border", #{ flex: #{ height: 70, shrink: 0 } }),
		row([
			band("border", #{ flex: #{ width: 70, shrink: 0 } }),
			column([inside], #{ flex: #{ grow: 1, padding: [0, 16] } }),
			band("border", #{ flex: #{ width: 70, shrink: 0 } }),
		], #{ flex: #{ grow: 1, padding: [16, 0] } }),
		band("border", #{ flex: #{ height: 70, shrink: 0 } }),
	];
	if info.art.contains("corner") {
		framed += [corner(true, true), corner(true, false), corner(false, true), corner(false, false)];
	}
	column([column(framed, #{ flex: #{ grow: 1 } })], #{ flex: #{ padding: [70, 150] } })
}
