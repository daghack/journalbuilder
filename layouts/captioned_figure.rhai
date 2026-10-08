// captioned_figure: a figure filling the page, with its caption centred
// beneath it.
//
// Flows: "caption".
// Roles: "caption", and a style for the figure if it's a path.
// Art: "figure".

fn page_size() {
	#{ width: 1400, height: 1080 }
}

fn page(n, info) {
	column(#{ padding: [90, 200, 60, 200] }, [
		figure("figure", #{ grow: 1 }),
		flow("caption", #{ align: "center", margin: [30, 0, 0, 0] }),
	])
}
