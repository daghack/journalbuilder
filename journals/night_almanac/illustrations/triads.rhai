// triads: the cards of steiner_sieve drawn as seals, the way the watcher
// copied them into his book: three ringed medallions on the corners of a
// triangle, a point of the thirteen in each, the triangle's sides ruled
// twice between them, the card's letter in its middle and its number in a
// lozenge under it. The three points are points_set's symbols; the letter
// is letters_set's (a space leaves the middle empty) and the number
// place_set's, read as numbers from -13 up (the ternary set's glyphs).
//
// Seals go cols to a row, each in a cell cell_width by cell_height, every
// other row half a cell along, out of true by seed: moved by up to a twentieth of the cell and turned by up to
// tilt (in radians), each triangle a little uneven. A canvas for each
// seal, so a page can keep its writing off each one; the first cell's
// corner is the canvases' origin. Unwired, the sets are fine_priests,
// fine_letters in red and fine_ternary (this journal's symbolsets: the
// library's priests and ternary, in a fine pen). With editor, each mark is
// committed to the visual editor as EDITOR_cN_pointK / letter / number.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/chance" as chance;

fn options() {
	#{ cols: 6, cell_width: 186.0, cell_height: 140.0, tilt: 0.2, seed: 0, editor: "", ink: "#2b1d12", blood: "#9b2318" }
}

fn inputs(o) {
	#{ cards: "Base27Symbol", points_set: "SymbolSet", letters_set: "SymbolSet", place_set: "SymbolSet" }
}

fn fallbacks(o) {
	#{
		points_set: symbol_set("fine_priests"),
		letters_set: symbol_set("fine_letters", #{ color: o.blood }),
		place_set: symbol_set("fine_ternary"),
	}
}

fn example() {
	#{ cards: [0, 1, 4, 3, 14, 5, 7, 9, 21, 15, 2, 6, 12, 5, 1] }
}

fn render(o, i) {
	let cards = i.cards;
	if cards.len() % 5 != 0 { throw `triads: the cards should be five symbols each (three points, a letter, a place), got ${cards.len()} symbols`; }
	if i.points_set.base < 13 { throw `triads: points_set '${i.points_set.name}' has ${i.points_set.base} symbols; the points need 13`; }
	let seals = [];
	for n in 0..cards.len() / 5 {
		let card = cards.extract(n * 5, 5);
		// Every other row half a cell along, as he'd have fitted them in.
		let at = vec2((n % o.cols + 0.5 + if (n / o.cols) % 2 == 1 { 0.5 } else { 0.0 }) * o.cell_width, (n / o.cols + 0.5) * o.cell_height);
		at += vec2(chance::either_way(o.seed, n, 1) * o.cell_width, chance::either_way(o.seed, n, 2) * o.cell_height) * 0.05;
		seals.push(one(o, i, n, card).rotate_around(vec2(0, 0), chance::either_way(o.seed, n, 3) * o.tilt).translate_by(at));
	}
	seals
}

// A mark of `set`, symbol `symbol`, `size` across, at `at`, in seal n.
fn mark(c, o, n, name, set, symbol, at, size) {
	let m = write_at(set.text([symbol]), at, #{ type: #{ glyph_scale: size } });
	if o.editor != "" { m = m.commit_to_editor(`${o.editor}_c${n}_${name}`); }
	c.add(m);
}

// One seal, centred on (0, 0).
fn one(o, i, n, card) {
	let h = o.cell_height;
	let w = o.cell_width;
	let r = h * 0.155;
	let uneven = h * 0.025;
	let verts = [vec2(0, -h * 0.31), vec2(-w * 0.25, h * 0.13), vec2(w * 0.25, h * 0.13)];
	for k in 0..3 { verts[k] += vec2(chance::either_way(o.seed, n * 3 + k, 4), chance::either_way(o.seed, n * 3 + k, 5)) * uneven; }
	let middle = (verts[0] + verts[1] + verts[2]) / 3.0;
	let c = new_canvas();
	// The sides, ruled twice from ring to ring.
	let sides = new_path();
	for k in 0..3 {
		let a = verts[k];
		let b = verts[(k + 1) % 3];
		let way = (b - a).normalized();
		let off = vec2(-way.y, way.x) * 2.4;
		for s in [-1.0, 1.0] {
			sides.append(new_line(a + way * r * 1.12 + off * s, b - way * r * 1.12 + off * s));
		}
	}
	c.add(sides, inks::detail(o.ink, 0.9));
	// The medallions: a heavy ring, a fine one inside it, ticks round it.
	for (v, k) in verts {
		c.add(new_circle(v, r), inks::band(o.ink, 2.2));
		c.add(new_circle(v, r * 0.82), inks::fine(o.ink, 0.7));
		c.add(plates::ticks(v, 16, r * 1.06, r * 1.2, 0, 0.0), inks::fine(o.ink, 0.6));
		mark(c, o, n, `point${k}`, i.points_set, card[k], v, r * 1.25);
	}
	// The letter in the middle, and the number in its lozenge under the
	// base.
	mark(c, o, n, "letter", i.letters_set, card[3], middle + vec2(0, h * 0.03), h * 0.21);
	let low = (verts[1] + verts[2]) / 2.0 + vec2(0, h * 0.22);
	c.add(new_polygon(low, h * 0.13, 4), inks::contour(o.ink, 1.4));
	c.add(new_line((verts[1] + verts[2]) / 2.0 + vec2(0, 2.4), low - vec2(0, h * 0.13)), inks::fine(o.ink, 0.7));
	mark(c, o, n, "number", i.place_set, card[4], low, h * 0.2);
	c
}
