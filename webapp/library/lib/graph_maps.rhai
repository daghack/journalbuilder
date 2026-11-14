// A Graph drawn as a map of places and the ways between them, laid out by
// forces (graph::layout): what graph_map and modular_maze draw with.
// import "lib/graph_maps" as maps;

import "lib/symbol_rings" as rings;

// Throws unless o.label_place is one graph::layout takes; `who` names the
// illustration.
fn check(o, who) {
	if !(o.label_place in ["along", "beside", "upright"]) {
		throw `${who}: label_place should be "along", "beside" or "upright", got "${o.label_place}"`;
	}
}

// Graph g as a map on one canvas, o.width by o.height (scaled down to fit
// if the layout needed more room), as graph_map describes it, with graph_map's
// options in o. s holds the styles, TextStyles each whole: name_style,
// glyph_style and box_style, and for each edge, by index, edge_styles[e]
// (its text) and line_styles[e] (its line and arrowhead). The places
// listed in s.twice (by index) have their names boxed twice, the second
// box o.padding / 2 inside the first. `who` names the illustration.
fn draw(g, o, s, who) {
	let twice = s.twice ?? [];
	// Each place's glyphs in a row about (0, 0), or a dot, and their size;
	// its name about (0, 0) in its box, and the box's size.
	let glyph_size = size_of(s.glyph_style.type);
	let sets = glyphset_names();
	let symbols = [];
	let sizes = [];
	let names = [];
	let name_sizes = [];
	for k in 0..g.nodes.len() {
		let node = g.nodes[k];
		let glyphs = s.glyph_style.type.glyphset;
		if node.glyphset != () {
			if !(node.glyphset in sets) {
				throw `${who}: ${node.name}'s glyphset '${node.glyphset}' isn't one there is`;
			}
			glyphs = glyphset(node.glyphset);
		}
		let known = glyphs.characters();
		// Side by side, a fifth of a glyph apart.
		let row = new_canvas();
		let x = 0.0;
		for name in node.glyphs {
			if !(name in known) {
				throw `${who}: ${node.name}'s glyph '${name}' isn't one of its glyphset's`;
			}
			let glyph = new_canvas().add(glyphs.glyph(name, #{ scale: glyph_size }), s.glyph_style.style);
			let b = glyph.bounds();
			row.add(glyph.translate_by(vec2(x - b.min.x, 0)));
			x += b.max.x - b.min.x + glyph_size * 0.2;
		}
		if node.glyphs.len() == 0 {
			let dot = new_circle(vec2(0, 0), o.dot / 2.0);
			row.add(dot, s.glyph_style.style).add(dot.hatch(o.dot / 5.0), s.glyph_style.style);
		}
		row = centred(row);
		let b = row.bounds();
		symbols.push(row);
		sizes.push([b.max.x - b.min.x, b.max.y - b.min.y]);
		let name = centred(write_block(markdown(node.name), vec2(0, 0), o.name_width, s.name_style).canvas);
		let nb = name.bounds();
		let inset = if k in twice { o.padding / 2.0 } else { 0.0 };
		let size = [nb.max.x - nb.min.x + 2.0 * (o.padding + inset), nb.max.y - nb.min.y + 2.0 * (o.padding + inset)];
		name.add(new_rectangle(vec2(0, 0), size[0], size[1]), s.box_style.style);
		if inset > 0.0 {
			name.add(new_rectangle(vec2(0, 0), size[0] - 2.0 * inset, size[1] - 2.0 * inset), s.box_style.style);
		}
		names.push(name);
		name_sizes.push(size);
	}
	// Each way's text about (0, 0), and its size.
	let labels = [];
	let label_sizes = [];
	for e in 0..g.edges.len() {
		let edge = g.edges[e];
		if edge.text == "" {
			labels.push(());
			label_sizes.push(());
			continue;
		}
		let c = centred(write_block(markdown(edge.text), vec2(0, 0), o.text_width, s.edge_styles[e]).canvas);
		let b = c.bounds();
		labels.push(c);
		label_sizes.push([b.max.x - b.min.x, b.max.y - b.min.y]);
	}
	let laid = graph::layout(g, sizes, #{
		labels: label_sizes, names: name_sizes, label_place: o.label_place, gap: o.gap,
		width: o.width, height: o.height, seed: o.seed, iterations: o.iterations,
		generations: o.generations ?? 0,
	});
	let map = new_canvas();
	for k in 0..g.nodes.len() {
		map.add(symbols[k].translate_by(point(laid.points[k])));
		map.add(names[k].translate_by(point(laid.names[k])));
	}
	for (edge, e) in laid.edges {
		let lines = new_path();
		for piece in edge.pieces {
			lines.append(new_line(point(piece[0]), point(piece[1])));
		}
		if g.edges[e].directed {
			let end = point(edge.end);
			lines.append(rings::arrowhead(end, (end - point(edge.start)).normalized(), o.arrow));
		}
		map.add(lines, s.line_styles[e].style);
		if g.edges[e].text != "" {
			map.add(labels[e].rotate_around(vec2(0, 0), edge.angle).translate_by(point(edge.label)));
		}
	}
	// A crowded graph took more room: back into width by height.
	let grown = laid.size[0] / o.width;
	if grown > 1.0 {
		map.scale_by(1.0 / grown);
	}
	map
}

// The size a type sets glyphs at.
fn size_of(t) {
	t.glyph_scale * t.glyph_scale_factor
}

// [x, y] as a vector.
fn point(xy) {
	vec2(xy[0], xy[1])
}

// A canvas moved so what's on it is centred on (0, 0).
fn centred(c) {
	let b = c.bounds();
	c.translate_by(vec2(-b.center.x, -b.center.y))
}
