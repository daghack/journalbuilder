// star_chart: a Huffman code drawn as a sky of named stars over pages, an
// art node: its chart is the code as a reader sees it. Each node of the
// code's tree is a star and each link a line carrying its digit; the sky is
// cut into constellations of at most most_stars stars, a page each, every
// one fitted to its page. Neighbouring pages share a star, under the same
// name, and that's all that joins them.
//
// The root is Shiathri, and a letter's star is named for its letter (the
// space's, in base 27, is Vacii); every other name, the constellations'
// too, comes from name_seed. The sky's shape comes from seed, its stars'
// sizes (how far their marks reach) from size_seed. width and height are
// each page's sky. label_size is the size of the number each star is
// labelled with (star_pages' digit_style's): its label gets room 1.1 of
// that across and 1.4 down, label_gap from the star's mark.
//
// Unlocking reads only what the pages show: it joins them on their shared
// names, takes the root as the star no link points to, a leaf's symbol
// from its name's first letter and each digit from its link, and so has
// the code back.

import "lib/names" as names;

fn options() {
	#{
		input_base: integer(), output_base: integer(), most_stars: integer(9),
		seed: integer(7), name_seed: integer(11), size_seed: integer(29), star_size: number(22.0),
		width: number(1000.0), height: number(860.0),
		label_size: number(24.0), label_gap: number(14.0),
	}
}

fn check(o) {
	if o.input_base == 10 || o.input_base < 2 || o.input_base > 27 {
		throw `star names start with letters, so input_base must be 27 or a base of letters from 2 to 26 (not 10), got ${o.input_base}`;
	}
	if o.most_stars < 2 {
		throw `most_stars must be at least 2, got ${o.most_stars}`;
	}
}

fn inputs(o) {
	#{ code: "HuffmanCode" }
}

fn outputs(o) {
	#{ chart: "StarChart" }
}

fn lock(o, i) {
	let code = i.code;
	check_bases(code, o);
	let laid = stars::layout(code, #{ seed: o.seed, star_size: o.star_size, width: 2600, height: 2000 });
	// How far each star's mark reaches: by depth, as it's laid out, then
	// some bigger and some smaller.
	let rng = new_random(o.size_seed);
	let sizes = [];
	for r in laid.sizes {
		sizes.push(r * 2.2 * rng.range(0.6, 1.5));
	}
	let edges = [];
	for (node, k) in code.nodes {
		if node.parent != () {
			edges.push([node.parent, k]);
		}
	}
	let pieces = stars::split(laid.points, edges, sizes, #{
		most_stars: o.most_stars, width: o.width, height: o.height,
		label_width: o.label_size * 1.1, label_height: o.label_size * 1.4, label_gap: o.label_gap,
	});
	let named = names::sky(code, new_random(o.name_seed), pieces.len());
	let pages = [];
	for (piece, p) in pieces {
		let stars = [];
		for (s, k) in piece.stars {
			stars.push(#{ name: named.stars[s], point: piece.points[k], size: sizes[s], label: piece.labels[k] });
		}
		let links = [];
		for pair in piece.pairs {
			links.push([pair[0], pair[1], code.nodes[piece.stars[pair[1]]].digit]);
		}
		pages.push(#{ name: named.constellations[p], stars: stars, links: links });
	}
	#{ chart: #{ kind: "StarChart", input_base: code.input_base, output_base: code.output_base, pages: pages } }
}

fn unlock(o, out) {
	let chart = out.chart;
	check_bases(chart, o);
	// The pages joined on their stars' names.
	let ids = #{};
	let called = [];
	let parent = [];
	let digit = [];
	let children = [];
	for page in chart.pages {
		let here = [];
		for star in page.stars {
			if !(star.name in ids) {
				ids[star.name] = called.len();
				called.push(star.name);
				parent.push(());
				digit.push(());
				children.push([]);
			}
			here.push(ids[star.name]);
		}
		for link in page.links {
			let a = here[link[0]];
			let b = here[link[1]];
			if parent[b] != () {
				throw `${called[b]} has two links to it: the pages give it two parents`;
			}
			parent[b] = a;
			digit[b] = link[2];
			children[a].push(b);
		}
	}
	let roots = [];
	for id in 0..called.len() {
		if parent[id] == () {
			roots.push(called[id]);
		}
	}
	if roots.len() != 1 {
		throw `the pages don't join into one sky: ${roots.len()} stars have no link to them (${roots}); is a page missing?`;
	}
	// The code rebuilt breadth first from the root, children in digit order.
	let order = [ids[roots[0]]];
	let place = #{};
	let nodes = [];
	let at = 0;
	while at < order.len() {
		let id = order[at];
		place[`${id}`] = at;
		let kids = children[id];
		let by_digit = #{};
		for k in kids {
			by_digit[`${digit[k]}`] = k;
		}
		for d in 0..chart.output_base {
			let k = by_digit[`${d}`];
			if k != () {
				order.push(k);
			}
		}
		let symbol = if kids.len() == 0 { names::symbol_of(chart.input_base, called[id]) } else { () };
		let up = if parent[id] == () { () } else { place[`${parent[id]}`] };
		nodes.push(#{ parent: up, digit: digit[id], symbol: symbol });
		at += 1;
	}
	#{ code: #{ kind: "HuffmanCode", input_base: chart.input_base, output_base: chart.output_base, nodes: nodes } }
}

// Throws unless a code's or chart's bases are the lock's.
fn check_bases(value, o) {
	if value.input_base != o.input_base || value.output_base != o.output_base {
		throw `the ${value.kind} is base ${value.input_base} to base ${value.output_base}, but the lock is base ${o.input_base} to base ${o.output_base}`;
	}
}

fn examples() {
	[
		#{
			// A pangram's code in base 3: every letter a star.
			options: #{ input_base: 27, output_base: 3 },
			inputs: #{ code: huffman::code(symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), 27, 3) },
		},
		#{
			// Binary, only the letters used, small constellations.
			options: #{ input_base: 27, output_base: 2, most_stars: 4, seed: 2 },
			inputs: #{ code: huffman::code(symbols(27, "SEE THE SEA"), 27, 2, #{ smoothing: 0 }) },
		},
		#{
			// Letters A to E in base 5, rewritten in base 2.
			options: #{ input_base: 5, output_base: 2 },
			inputs: #{ code: huffman::code([0, 1, 2, 3, 4, 4, 4, 2], 5, 2) },
		},
		#{
			options: #{ input_base: 10, output_base: 2 },
			error: "input_base must be 27 or a base of letters",
		},
		#{
			options: #{ input_base: 27, output_base: 2 },
			inputs: #{ code: huffman::code(symbols(27, "HI"), 27, 3) },
			error: "but the lock is base 27 to base 2",
		},
	]
}
