// two_column_article: an article in two columns. The first page opens with a heading over the left column and a
// drop cap, with an illustration and its caption atop the right column; the
// body runs down the left column, on down the right, and on over as many
// pages as it takes, two columns each, with an ornamental band between them
// and a page number below.
//
// Flows: "title", "body", and "caption" (only with an "illustration").
// Roles: "body", "heading", "caption", "initial", "folio" (page numbers),
// and a style for each piece of art.
// Art: "divider" (a band's motif) and, optionally, "illustration".
// Options: the page's width and height, the gap either side of the
// divider, and whether pages are numbered (folios).

fn options() {
	#{ width: 1400.0, height: 1080.0, gap: 40.0, folios: true }
}

fn page_size(o) {
	#{ width: o.width, height: o.height }
}

// The band between the columns, and the page number under them.
fn divider() {
	band("divider", #{ flex: #{ width: 22, margin: [10, 0] } })
}

// Page numbers in roman numerals, which glyphsets without digits can set.
fn folio(n) {
	label(roman(n + 1), #{ role: "folio", flex: #{ height: 40 }, paragraph: #{ align: "center" } })
}

fn page(n, info) {
	let gap = info.options.gap;
	let columns = if n == 0 {
		let left = column([
			flow("title", #{ role: "heading", flex: #{ height: 110 }, paragraph: #{ fit: true, valign: "center", align: "center" } }),
			rule(#{ flex: #{ height: 20, margin: [0, 120, 20, 120] } }),
			column([
				initial("body", #{ flex: #{ position: "absolute", top: 0, left: 0, width: 130, height: 130 } }),
				flow("body", #{ flex: #{ grow: 1 } }),
			], #{ flex: #{ grow: 1 } }),
		], #{ flex: #{ grow: 1 } });
		let right = if info.art.contains("illustration") {
			column([
				figure("illustration", #{ flex: #{ height: "48%", margin: [0, 30] } }),
				flow("caption", #{ flex: #{ margin: [10, 0, 24, 0] } }),
				flow("body", #{ flex: #{ grow: 1 } }),
			], #{ flex: #{ grow: 1 } })
		} else {
			flow("body", #{ flex: #{ grow: 1 } })
		};
		row([left, divider(), right], #{ flex: #{ grow: 1, gap: gap } })
	} else {
		row([flow("body", #{ flex: #{ grow: 1 } }), divider(), flow("body", #{ flex: #{ grow: 1 } })], #{ flex: #{ grow: 1, gap: gap } })
	};
	let page = [columns];
	if info.options.folios {
		page.push(folio(n));
	}
	column(page, #{ flex: #{ padding: [80, 110, 40, 110] } })
}

// An article of lorem ipsum, in the library's text styles.
fn example() {
	let body = text_style("plain_hand");
	let heading = text_style("copper_heading", #{ size: 90.0 });
	let red = text_style("plain_hand", #{ size: 22.0, color: "#9b2318" });
	#{
		flows: #{ title: "Dolor Sit Amet", body: content("lorem") + "\n\n" + content("lorem") + "\n\n" + content("lorem"), caption: "consectetur adipiscing" },
		roles: #{
			body: body,
			heading: heading,
			caption: red + #{ paragraph: #{ align: "center" } },
			initial: #{ style: heading.style, type: #{ glyphset: glyphset("copper") } },
			folio: red,
			rule: #{ style: red.style },
			divider: #{ style: red.style },
		},
		art: #{
			divider: new_path().move_to(vec2(0, -10)).line_to(vec2(6, 0)).line_to(vec2(0, 10)).line_to(vec2(-6, 0)).close(),
			illustration: new_circle(vec2(0, 0), 40),
		},
	}
}
