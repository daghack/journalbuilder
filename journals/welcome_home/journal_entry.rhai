// Welcome home: a bramble grown out from the middle of the page along a
// spanning tree of scattered points, thick at the middle and thinning as it
// branches out, and over it an eye stood on end, its corners top and
// bottom, its iris ringed with balanced-ternary glyphs.

const W = 1400.0;
const H = 1900.0;
const MARGIN = 110.0;
const CENTRE = vec2(W / 2.0, H / 2.0);

seed(2207);

// ---- inks, after becoming_us ----

const NAVY = "#2b4184";
const DARK_NAVY = "#0b1433";

const FLOWER_SIZE = 52.0;
const VINE_PEN = new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.5, pool_depth: 0.4 });
const LEAF_PEN = new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.9, pool_depth: 0.5 });
const VINE_INK = new_style(new_gradient(["#0b0b0b", "#3f7d47"]), VINE_PEN);
const LEAF_INK = new_style("#0b0b0b", LEAF_PEN);
const FLOWER_BLUE = "#2f6dff";
const FLOWER_CENTRE = "#000000";

// The eye's lines in becoming_us's eye pen, gone over.
const EYE_PEN = new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.85, pool_depth: 0.6 });
// The whole eye a deep emerald to a darker green, down each line (each
// glyph, for the iris's key).
const EYE_GRADIENT = new_gradient(["#09603d", "#0b2a1c"], #{ vertical: true });

const TERNARY = ternary_font("balanced_ternary", #{ character_padding: 0.32 });
// The ternary glyphs: the same pen unpooled, so their strokes taper away.
const KEY_PEN = new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15 });

// ---- the lock ----

// A substitution cipher for what's written here later: its ordering is the
// key, and the eye's iris holds it, a ternary glyph per symbol, clockwise
// from the top (the space, then A to Z). A glyph at the n-th place in the
// ring stands for the n-th symbol.
const ORDERING = "MQ ZEHWBRKYDTXOAJGNLUFCVISP";
let lock = new_lock("welcome_home")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.set_input("sub.ordering", symbols(27, ORDERING));

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
		canvas.add(f.path, new_style(ink, global::VINE_PEN));
	}
}

// ---- the eye, on end ----

const EYE_LENGTH = 1640.0;

// The eye's lids: corners top and bottom, bowing out left and right. Gives
// #{ left, right, outline, outer, inner }: the two lids, the closed shape
// between them, and the iris's rings' radii.
fn eye_shape(centre, length) {
	let outer = length * 0.215;
	let inner = outer * 0.56;
	let half = length / 2.0;
	let rise = (outer + 6.0) / 0.75;
	let left = new_path().move_to(centre + vec2(-3, -half))
		.cubic_to(centre + vec2(-rise, -half * 0.45), centre + vec2(-rise, half * 0.45), centre + vec2(-3, half));
	let right = new_path().move_to(centre + vec2(9, -half + 8))
		.cubic_to(centre + vec2(rise, -half * 0.45), centre + vec2(rise, half * 0.45), centre + vec2(9, half - 8));
	// Down the left lid, across its corner and back up the right.
	let outline = new_path().move_to(centre + vec2(-3, -half))
		.cubic_to(centre + vec2(-rise, -half * 0.45), centre + vec2(-rise, half * 0.45), centre + vec2(-3, half))
		.line_to(centre + vec2(9, half - 8))
		.cubic_to(centre + vec2(rise, half * 0.45), centre + vec2(rise, -half * 0.45), centre + vec2(9, -half + 8))
		.close();
	#{ left: left, right: right, outline: outline, outer: outer, inner: inner }
}

// The eye drawn: lids and rings gone over, and the ring of ternary glyphs,
// clockwise from the top, each turned to the ring and a little off true.
fn draw_eye(canvas, centre, shape) {
	let lids = new_style(global::EYE_GRADIENT, global::EYE_PEN, #{ passes: 2, jitter: 2.5 });
	let rings = new_style(global::EYE_GRADIENT, global::EYE_PEN, #{ passes: 3, jitter: 4.0 });
	let key = new_style(global::EYE_GRADIENT, global::KEY_PEN, #{ passes: 3, jitter: 2.0 });
	canvas.add(shape.left, lids);
	canvas.add(shape.right, lids);
	canvas.add(new_circle(centre, shape.outer), rings);
	canvas.add(new_circle(centre, shape.inner), rings);
	let glyphs = global::TERNARY.glyphs(global::ORDERING);
	let count = glyphs.len();
	for (at, i) in new_circle(centre, (shape.outer + shape.inner) / 2.0).points_along(count, #{ start: 0.75 }) {
		let g = glyphs[i];
		g.scale = (shape.outer - shape.inner) * 0.8;
		g.position = at.position + vec2(random_range(-0.24, 0.24), random_range(-0.36, 0.36));
		g.rotation = at.angle + random_range(-0.07, 0.07);
		canvas.add(g.scale_by(random_range(0.93, 1.07)), key);
	}
}

// The pupil filled with ciphered text in black, glyph by glyph, as close
// to the inner ring as the ring's wandering passes allow, top to bottom
// and side to side: at the largest size, from PUPIL_SIZE down, that holds
// all of it.
const PUPIL_SIZE = 40.0;
const PUPIL_SIZE_STEP = 0.5;
const PUPIL_CLEARANCE = 7.0;
// Set closer than the ring's key.
const PUPIL_TERNARY = ternary_font("balanced_ternary", #{ character_padding: 0.08 });
// A finer nib for the small glyphs, pooled at their ends as the text's is.
const PUPIL_PEN = new_brush("Rough Ink", 0.3, 0.6, 1.2, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.85, pool_depth: 0.6 });
const PUPIL_INK = "#000000";
fn fill_pupil(canvas, centre, shape, cipher_text) {
	let size = global::PUPIL_SIZE;
	loop {
		let pupil = new_textbox(#{
			shape: new_circle(centre, shape.inner - global::PUPIL_CLEARANCE),
			size: size,
			layout: "flat",
			align: "justify",
			valign: "center",
			position_jitter: [0.03, 0.04],
			rotation_jitter: 0.05,
		});
		let filled = pupil.fill(global::PUPIL_TERNARY.glyphs(cipher_text));
		if filled.unplaced.is_empty() {
			canvas.add(filled.placed, new_style(global::PUPIL_INK, global::PUPIL_PEN, #{ jitter: 0.8 }));
			return size;
		}
		size -= global::PUPIL_SIZE_STEP;
		assert(size > 0.0, "the pupil's text should fit at some size");
	}
}

// ---- the page ----

// What the pupil holds, through the lock: letters and spaces, the
// punctuation dropped.
const WHISPER = "In my ear she whispers Welcome Home. She is in my dreams, and in my secret voice. Hers is the voice which Heralds the End, the Mouth which sustains me.";
let ciphered = lock.run(#{ "sub.plain_text": symbols(27, WHISPER, #{ lenient: true }) }).text("sub.cipher_text");

// Two hundred points spread evenly, joined from the one nearest the middle,
// each by its shortest way there by squared distance: many short hops.
let points = distribute_points("poisson_disk", 200, [[MARGIN, W - MARGIN], [MARGIN, H - MARGIN]]);
let root = nearest(points, CENTRE);
let tree = points.spanning_tree(#{ algorithm: "shortest_path", metric: "squared_euclidean", root: root });
let branches = new_edges(points, tree);
let plant = art::bramble_along(branches, FLOWER_SIZE, #{ seed: 41, origin: points[root], thickness: 6.0 });

// Nothing of the bramble under the eye.
let eye = eye_shape(CENTRE, EYE_LENGTH);
plant.vines.erase_inside(eye.outline);
plant.leaves.erase_inside(eye.outline);
plant.flowers.erase_inside(eye.outline);

let canvas = new_canvas();
draw_bramble(canvas, plant);
draw_eye(canvas, CENTRE, eye);
fill_pupil(canvas, CENTRE, eye, ciphered);

journal_entry.add(new_page(#{ name: "welcome_home_page_0", size: vec2(W, H) }).add(canvas));
