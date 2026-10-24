// Welcome home: a bramble grown out from the middle of the page along a
// spanning tree of scattered points, thick at the middle and thinning as it
// branches out, and over it an eye stood on end, its corners top and
// bottom, its iris ringed with balanced-ternary glyphs.

const W = 1400.0;
const H = 1900.0;
const MARGIN = 110.0;
const CENTRE = vec2(W / 2.0, H / 2.0);
// How tall and wide the eye is.
const EYE_LENGTH = 1640.0;
const EYE_WIDTH = 717.2;

seed(2207);

// ---- inks, after becoming_us ----

const NAVY = "#2b4184";
const DARK_NAVY = "#0b1433";

const FLOWER_SIZE = 52.0;
const VINE_PEN = new_brush_style(new_gradient(["#0b0b0b", "#3f7d47"]), "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.5, pool_depth: 0.4 });
const LEAF_PEN = new_brush_style("#0b0b0b", "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.9, pool_depth: 0.5 });
const STEADY = new_handwriting_style();
const VINE_INK = new_style(VINE_PEN, STEADY);
const LEAF_INK = new_style(LEAF_PEN, STEADY);
const FLOWER_BLUE = "#2f6dff";
const FLOWER_CENTRE = "#000000";

// ---- the lock ----

// A substitution cipher for what's written here later: its ordering is the
// key, and the eye's iris holds it, a ternary glyph per symbol, clockwise
// from the top (the space, then A to Z). A glyph at the n-th place in the
// ring stands for the n-th symbol.
const ORDERING = "MQ ZEHWBRKYDTXOAJGNLUFCVISP";
// The eye renderer draws them: the ordering in its iris, and the cipher
// text in its pupil.
let lock = new_lock("welcome_home")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.set_input("sub.ordering", symbols(27, ORDERING))
	.attach("eye", "eye", #{ length: EYE_LENGTH, width: EYE_WIDTH })
	.connect("sub.ordering", "eye.iris")
	.connect("sub.cipher_text", "eye.pupil");

// ---- the bramble ----

// The bramble's parts drawn, each flower given its own radial gradient:
// its pieces gathered by how near they are, and each gathering coloured out
// from its own middle.
fn draw_bramble(canvas, plant) {
	canvas.add(plant.vines, global::VINE_INK);
	canvas.add(plant.leaves, global::LEAF_INK);
	let flowers = [];
	for piece in plant.flowers.subpaths() {
		let at = piece.bounds().center;
		let home = -1;
		for (f, i) in flowers {
			if f.center.distance(at) < global::FLOWER_SIZE * 0.6 { home = i; break; }
		}
		if home < 0 {
			flowers.push(#{ path: piece, center: at, count: 1 });
		} else {
			let f = flowers[home];
			f.path.append(piece);
			f.center = (f.center * f.count + at) / (f.count + 1);
			f.count += 1;
			flowers[home] = f;
		}
	}
	for f in flowers {
		let b = f.path.bounds();
		let ink = new_gradient([global::FLOWER_CENTRE, global::DARK_NAVY, global::FLOWER_BLUE], #{ center: b.center, radius: b.width.max(b.height).max(1.0) * 0.55 });
		let pen = global::VINE_PEN;
		pen.color = ink;
		canvas.add(f.path, new_style(pen, global::STEADY));
	}
}

// ---- the eye, on end ----

// The closed shape between the eye's lids, as the eye renderer draws them,
// for the bramble to keep out of.
fn eye_outline(centre, length, width) {
	let half = length / 2.0;
	let rise = width / 2.0 / 0.75;
	new_path().move_to(centre + vec2(-3, -half))
		.cubic_to(centre + vec2(-rise, -half * 0.45), centre + vec2(-rise, half * 0.45), centre + vec2(-3, half))
		.line_to(centre + vec2(9, half - 8))
		.cubic_to(centre + vec2(rise, half * 0.45), centre + vec2(rise, -half * 0.45), centre + vec2(9, -half + 8))
		.close()
}

// ---- the page ----

// What the pupil holds, through the lock: letters and spaces, the
// punctuation dropped.
const WHISPER = "In my ear she whispers Welcome Home. She is in my dreams, and in my secret voice. Hers is the voice which Heralds the End, the Mouth which sustains me.";
let result = lock.run(#{ "sub.plain_text": symbols(27, WHISPER, #{ lenient: true }) });

// Two hundred points spread evenly, joined from the one nearest the middle,
// each by its shortest way there by squared distance: many short hops.
let points = distribute_points("poisson_disk", 200, [[MARGIN, W - MARGIN], [MARGIN, H - MARGIN]]);
let root = nearest(points, CENTRE);
let tree = points.spanning_tree(#{ algorithm: "shortest_path", metric: "squared_euclidean", root: root });
let branches = new_edges(points, tree);
let plant = art::bramble_along(branches, FLOWER_SIZE, #{ seed: 41, origin: points[root], thickness: 6.0 });

// Nothing of the bramble under the eye.
let outline = eye_outline(CENTRE, EYE_LENGTH, EYE_WIDTH);
plant.vines.erase_inside(outline);
plant.leaves.erase_inside(outline);
plant.flowers.erase_inside(outline);

let canvas = new_canvas();
draw_bramble(canvas, plant);
canvas.add(result.canvases("eye")[0].translate_by(CENTRE - vec2(EYE_WIDTH / 2.0, EYE_LENGTH / 2.0)));

journal_entry.add(new_page(#{ name: "welcome_home_page_0", size: vec2(W, H) }).add(canvas));
