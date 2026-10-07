// missal_title: a title page, as the Jagged Missal's opens: the title,
// a flourish and the credits centred in a ruled frame, inside a border of
// ornamental bands with a piece in each corner.
//
// Flows: "title"; "subtitle" and "credits" (lines broken with "\n") if
// given.
// Roles: "title", "subtitle", "credits", "frame" (the frame's rule), and a
// style for each piece of art that's a path.
// Art: "border" (the bands' motif); "corner" and "flourish" (under the
// title) if given.

fn page_size() {
	#{ width: 1400, height: 1080 }
}

fn corner(top, left) {
	let at = #{ position: "absolute", width: 70, height: 70 };
	if top { at.top = 0; } else { at.bottom = 0; }
	if left { at.left = 0; } else { at.right = 0; }
	figure("corner", at)
}

fn page(n, info) {
	let middle = [flow("title", #{ height: 300, fit: true, align: "center", valign: "center" })];
	if info.art.contains("flourish") {
		middle.push(figure("flourish", #{ height: 50, margin: [10, 300, 30, 300] }));
	} else {
		middle.push(rule(#{ role: "frame", height: 20, margin: [10, 380, 30, 380] }));
	}
	for name in ["subtitle", "credits"] {
		if info.flows.contains(name) {
			middle.push(flow(name, #{ align: "center", margin: [0, 0, 24, 0] }));
		}
	}
	let inside = column(#{ grow: 1, justify: "center", padding: [30, 60], border: "frame" }, middle);
	let framed = [
		band("border", #{ height: 70 }),
		row(#{ grow: 1, padding: [16, 0] }, [
			band("border", #{ width: 70 }),
			column(#{ grow: 1, padding: [0, 16] }, [inside]),
			band("border", #{ width: 70 }),
		]),
		band("border", #{ height: 70 }),
	];
	if info.art.contains("corner") {
		framed += [corner(true, true), corner(true, false), corner(false, true), corner(false, false)];
	}
	column(#{ padding: [70, 150] }, [column(#{ grow: 1 }, framed)])
}
