const NAVY = "#2b4184";
const DARK_NAVY = "#0b1433";

const SIZE = 60.0;
const TEXT_INK = new_gradient([NAVY, DARK_NAVY], #{ vertical: true });
// Each stroke a shade off the next, as ink goes on unevenly.
const TEXT_PEN = new_brush_style(TEXT_INK, "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.5, pool_depth: 0.4, color_jitter: 0.04 });
const STEADY = new_handwriting_style();

// `pen` in `colour`, by a steady hand.
fn inked(colour, pen) {
	pen.color = colour;
	new_style(pen, global::STEADY)
}

// Words set a little closer than copper's own spacing.
let copper = glyphset("copper");
copper.space_width = 0.38;
let hand = #{
	// Each glyph a little off true, each line wandering.
	style: new_style(TEXT_PEN, new_handwriting_style(#{
		glyph_rotation_jitter: 0.06, glyph_position_jitter: [SIZE * 0.04, SIZE * 0.06], baseline_wander: SIZE * 0.12,
	})),
	// Letters a touch further apart than copper sets them.
	type: new_type_style(copper, SIZE, #{ glyph_tracking: 0.05 }),
	paragraph: new_paragraph_style(#{ line_height: 1.5, line_start_jitter: SIZE * 0.5, line_gap_jitter: SIZE * 0.15 }),
};

// The layout for this should be fairly straightforward, as if they are being written in small pages. I want 13 straight-edged eyes with the navy blue/light teal colors as gradiants scattered throughout, with the text tightly wrapping around them (but not through them). Each should be gazing at a single point per page. I want the pen type of the color for eyes to utilize a mostly transparent color, but then have multiple jittered copies of the color part (each with slightly different coloring) interleaving to give some texture and depth to the color.

let paragraphs = [
	"The future is like a branching vine, twisting and growing in many paths all at once, and it is beyond us to conceive of exactly how and when the thorny paths will bend, snag, or blossom. Machinate as you may, it is inevitable that in one's devisings, there will be a failure to contend with the true nature of the gnarled way set before you, for no mortal mind can contend with the vastness of what is possible. Predict as you may the thunderous light of a storm, the spirits of the sky prove out again and again that they are beyond such reckoning.",
	"This is not to say that there is no value in imaging or constructing a path forward. On the contrary, to lack vision or direction is to be set adrift amongst the endless, sweeping desert sands with neither sun nor stars to guide—at the perfect mercy of the spirits and elements above and beyond your deliberate control. But it is vital to understand that, invariably, execution must veer wildly from its initial perceived course. Most often this is to avoid an unforeseen or emergent catastrophe with which you have failed, in advanced, to reckon.",
	"Rarely, however, Fate is fickle in ones favor, and deviation must be made not to avert disaster but to seize some singular opportunity which has presented itself. In such cases, it behooves to cast away whatever designs one has carefully crafted and instead embrace what by Fate has been so graciously granted.",
	`When Crom refused use of his blade, the sense that my design for Us had hit an obstacle insurmountable barely had time to wash over me before his next words drove away that irritating disappointment. "You could borrow from somebody else. All the Lazarines have one." In the course of that one sentence, everything changed. It was as though I was merely an empty cup, and some greater mind than mine all at once filled me with the knowledge of what I had to do. More than that, it filled me with the certainty that I would succeed. Before I had even left Crom, the plan was clear inside me, like a radiant ray of noonday sun cutting through the dark of twisting caves it was a promise and a path alike.`,
	"I knew that to ask you directly would fail—we're the type of creature who desires control, craves to believe that we move our own hand. And so I approached Atesh. From first approach, for all the reasons Crom had refused to deliver his blade, I knew that Atesh would likewise refuse my request. The intent was not ever to get Atesh's blade, however. It was to get yours, always to get yours. To ask you directly? That would have raised defenses that at even my most persuasive I could have never overcome, but by placing my need before you by way of Atesh you would be unable to refuse the temptation of giving me exactly what I desired, what I needed to become Us—not just the bait to lure you away, but the instrument of Our uplifting.",
	"By black sand and blood, you evaded Us, true. It was masterfully escape, but I was not left empty-handed. Before I tracked you back to the tavern, I made one small stop before the inevitable confrontation which would lead to my demise.",
	"The intent was, at the time, to use it as collateral—something for barter that I might avoid reprisal—but Ankara and his eager hand has given Us a gift, for the longer I held Our knife and considered how to use it, the more certain I became that I would never let it part from Us. It is Ours now truly and certainly, baptized in blood and whispered words in Our voice and delivered unto safely keeping hands."];

// ---- layout ----
//
// Small pages, written with write_block: each page's eyes go down first,
// then the paragraphs, wrapping close round the eyes' outlines. A
// paragraph that runs off a page goes on at the top of the next. Every eye
// on a page gazes at the same point.

const W = 1400.0;
const H = 1900.0;
const MARGIN = 110.0;
const PARAGRAPH_GAP = SIZE * 0.6;
// How many eyes each page has: 13 in all.
const EYES_PER_PAGE = [3, 2, 2, 2, 2, 2];
// The eyes' colour, washed on thin and gone over in jittered copies, each
// stroke of them its own shade (the text pen's colour jitter, a little
// more).
const EYE_TEAL = "46, 232, 214";
const EYE_NAVY = "43, 65, 132";
const EYE_JITTER = 0.06;
const WASH_ALPHA = 0.3;
const COPIES = 5;
const COPY_ALPHA = 0.4;
// The eyes' lines in the text's pen, pooled heavier, from navy to
// black, gone over several times:
// each pass its own copy of the lines, shifted, turned and scaled a little
// about the eye's centre.
const EYE_PEN = new_brush_style(new_gradient([NAVY, "#000000"], #{ vertical: true }), "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.85, pool_depth: 0.6, color_jitter: 0.04 });
const EYE_INK = new_style(EYE_PEN, new_handwriting_style(#{ stroke_point_jitter: 10.0 }));
// A second go over the colour, smearing it along the same lines, the
// brush's width wandering as it goes.
const SMEAR = new_brush_style("black", "Liquify", 1.0, #{ min_pressure: 0.1, max_pressure: 1.0, pressure_profile: "taper", pressure_wander: 0.7 });
const EYE_PASSES = 5;
const EYE_SHIFT = 8.0;
const EYE_TURN = 0.04;
const EYE_SCALE = 0.08;

// A number from 0 up to 1 for a key, the same every run.
fn hashed(k) {
	let x = (k * 7919 + 104729) % 1000003;
	x = (x * x + 12345) % 1000003;
	x / 1000003.0
}

// A path cut into strokes about `length` long, one after another along
// each of its subpaths.
fn short_strokes(path, length) {
	let out = new_path();
	for sub in path.subpaths() {
		let b = sub.bounds();
		// Enough points to follow its curves; the strokes take a run of them.
		let count = ((b.width + b.height) * 3.0 / length).max(4.0).to_int();
		let points = sub.points_along(count * 4).map(|p| p.position);
		let i = 0;
		while i + 1 < points.len() {
			let end = (i + 5).min(points.len() - 1);
			out.append(new_polyline(points.extract(i, end - i + 1)));
			i = end;
		}
	}
	out
}

// An eye looking at a point: a thin wash of colour, jittered copies of it
// each its own shade, then the lines over them.
fn draw_eye(center, length, look_at, seed) {
	let to = look_at - center;
	let eye = art::eye(center, length, #{
		flat: true, seed: seed, messiness: 0.0, openness: 0.4,
		gaze_angle: to.angle(), gaze: (to.length() / 500.0).min(1.0),
	});
	let r = eye.iris_radius;
	let iris = eye.wash.bounds().center;
	let colour = |alpha| new_gradient([`rgba(${global::EYE_TEAL}, ${alpha})`, `rgba(${global::EYE_NAVY}, ${alpha})`], #{ center: iris, radius: r });
	// The colour in the text's own pen, its strokes' shades further apart.
	let wash_brush = global::TEXT_PEN;
	wash_brush.color_jitter = global::EYE_JITTER;
	let rim_brush = wash_brush;
	// In short strokes, so each pools at its end as the text's glyphs do.
	let wash = short_strokes(eye.wash, 14.0);
	let rim = short_strokes(eye.rim, 14.0);
	// The smear goes over the colour in its gradient: strokes sharing a
	// gradient share a layer, and Liquify can only push what's on its own.
	let wash_smear = global::SMEAR;
	wash_smear.brush_size = r * 0.25;
	let rim_smear = global::SMEAR;
	rim_smear.brush_size = r * 0.12;
	let canvas = new_canvas();
	let base = colour.call(global::WASH_ALPHA);
	canvas.add(wash, inked(base, wash_brush));
	canvas.add(eye.wash, inked(base, wash_smear));
	let shade = colour.call(global::COPY_ALPHA);
	for c in 0..global::COPIES {
		let off = vec2(random_range(-1.0, 1.0), random_range(-1.0, 1.0)) * (r * 0.08);
		canvas.add(wash.copy().translate_by(off), inked(shade, wash_brush));
		canvas.add(rim.copy().translate_by(off), inked(shade, rim_brush));
		canvas.add(eye.wash.copy().translate_by(off), inked(shade, wash_smear));
		canvas.add(eye.rim.copy().translate_by(off), inked(shade, rim_smear));
	}
	canvas.add(eye.accent, new_style(new_brush_style(colour.call(0.6), "Ink Pen", 1.2, #{ min_pressure: 0.2, max_pressure: 0.6, color_jitter: global::EYE_JITTER }), global::STEADY));
	for p in 0..global::EYE_PASSES {
		let lines = eye.lines.copy();
		lines.translate_by(-center).scale_by(1.0 + random_range(-global::EYE_SCALE, global::EYE_SCALE)).translate_by(center);
		lines.rotate_around(center, random_range(-global::EYE_TURN, global::EYE_TURN));
		lines.translate_by(vec2(random_range(-1.0, 1.0), random_range(-1.0, 1.0)) * global::EYE_SHIFT);
		canvas.add(lines, global::EYE_INK);
	}
	canvas
}

// Page n's eyes, each in its own band down the page, all looking at one
// point: an array of canvases, an eye each.
fn page_eyes(n) {
	let count = if n < global::EYES_PER_PAGE.len() { global::EYES_PER_PAGE[n] } else { 0 };
	let eyes = [];
	let inner_w = global::W - 2.0 * global::MARGIN;
	let inner_h = global::H - 2.0 * global::MARGIN;
	let look_at = vec2(global::MARGIN + hashed(n * 31 + 1) * inner_w, global::MARGIN + hashed(n * 31 + 2) * inner_h);
	for i in 0..count {
		let k = n * 17 + i * 5;
		let length = 260.0 + hashed(k + 3) * 120.0;
		let band = inner_h / count;
		let center = vec2(
			global::MARGIN + length / 2.0 + hashed(k + 4) * (inner_w - length),
			global::MARGIN + band * i + length * 0.3 + hashed(k + 5) * (band - length * 0.6),
		);
		eyes.push(draw_eye(center, length, look_at, n * 10 + i));
	}
	eyes
}

// A new page's canvas, its eyes drawn, and text kept clear of each eye's
// outline.
fn start_page(n) {
	let eyes = page_eyes(n);
	let tree = new_avoidance_tree();
	for eye in eyes { tree.add_hull(eye); }
	this.avoid = tree;
	new_canvas().add(eyes)
}

// ---- brambles ----
//
// Curling brambles in the margins: the text's brush, vines from black to
// dark green, black leaves pooled heavier, and flowers bright blue round
// their edges, through dark navy to black at their centres.

const FLOWER_SIZE = 52.0;
const VINE_PEN = new_brush_style(new_gradient(["#0b0b0b", "#3f7d47"]), "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.5, pool_depth: 0.4, color_jitter: 0.05 });
const LEAF_PEN = new_brush_style("#0b0b0b", "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.9, pool_depth: 0.5, color_jitter: 0.05 });
const VINE_INK = new_style(VINE_PEN, STEADY);
const LEAF_INK = new_style(LEAF_PEN, STEADY);
const FLOWER_BLUE = "#2f6dff";
const FLOWER_CENTRE = "#000000";

// A vine from a to b, waving either side of the line between them and
// ending in a curl of `turns` turns, `radius` across, toward `curl_side`
// (off the page, so it curls in the margin): a polyline.
fn curling_vine(a, b, wave, radius, turns, curl_side, k) {
	let points = [];
	let along = b - a;
	let side = vec2(-along.y, along.x).normalized();
	let steps = 40;
	let phase = hashed(k) * 6.28;
	let waves = 1.5 + hashed(k + 1) * 2.0;
	for i in 0..=steps {
		let t = i / steps.to_float();
		points.push(a + along * t + side * (wave * (t * 6.28 * waves + phase).sin()));
	}
	// The curl: round from the end, closing in, turning whichever way
	// carries on as the vine was going.
	let dir = along.normalized();
	let centre = b + curl_side * (radius * 0.4);
	let turn = if vec2(curl_side.y, -curl_side.x).dot(dir) > 0.0 { 1.0 } else { -1.0 };
	let start = (b - centre).angle();
	let curl_steps = (turns * 24.0).to_int();
	for i in 1..=curl_steps {
		let t = i / curl_steps.to_float();
		let angle = start + turn * t * turns * 6.28;
		let r = radius * (1.0 - 0.8 * t);
		points.push(centre + vec2(angle.cos(), angle.sin()) * r);
	}
	new_polyline(points)
}

// The bramble's parts drawn, each flower given its own radial gradient:
// its pieces (petals, stamens, buds, each a subpath) gathered by how near
// they are, and each gathering coloured out from its own middle.
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
		canvas.add(f.path, inked(ink, global::VINE_PEN));
	}
}

// Page n's brambles: a few large vines, each coming in from off the page
// across an edge, running along the margin and curling there; and, below
// free_from, one across the space left under the text.
fn page_brambles(n, free_from) {
	let canvas = new_canvas();
	let w = global::W;
	let h = global::H;
	let inset = global::MARGIN * 0.6;
	// Each edge: where it runs from and to, and which way is off the page.
	let edges = [
		[vec2(0.0, 0.0), vec2(w, 0.0), vec2(0.0, -1.0)],
		[vec2(w, 0.0), vec2(w, h), vec2(1.0, 0.0)],
		[vec2(w, h), vec2(0.0, h), vec2(0.0, 1.0)],
		[vec2(0.0, h), vec2(0.0, 0.0), vec2(-1.0, 0.0)],
	];
	let count = 3 + (hashed(n * 7 + 1) * 2.0).floor().to_int();
	// The edges in an order of the page's own, a vine on each of the first
	// few, so no two share an edge.
	let order = [0, 1, 2, 3];
	order.sort(|a, b| if hashed(n * 7 + 20 + a) < hashed(n * 7 + 20 + b) { -1 } else { 1 });
	for v in 0..count {
		let k = n * 101 + v * 13;
		let edge = edges[order[v]];
		// Running either way along its edge.
		if hashed(k + 9) < 0.5 { edge = [edge[1], edge[0], edge[2]]; }
		let along = edge[1] - edge[0];
		let out = edge[2];
		let span = (400.0 + hashed(k + 3) * 400.0) / along.length();
		let t = hashed(k + 2) * (1.0 - span);
		let start = edge[0] + along * t + out * 120.0;
		let end = edge[0] + along * (t + span) - out * inset;
		let path = curling_vine(start, end, 15.0 + hashed(k + 5) * 15.0, 55.0 + hashed(k + 6) * 25.0, 1.3 + hashed(k + 7) * 0.5, out, k);
		draw_bramble(canvas, art::bramble_along(path, global::FLOWER_SIZE, #{ seed: k, origin: start, thickness: 2.5, reach: 500.0 }));
	}
	// Room left under the text: one vine in from the side, curling there.
	let top = free_from + global::SIZE;
	let bottom = h - global::MARGIN * 0.5;
	if bottom - top > 200.0 {
		let k = n * 101 + 900;
		let yy = (top + bottom) / 2.0;
		let start = vec2(-120.0, yy + 80.0);
		let end = vec2(w * 0.6, yy);
		let path = curling_vine(start, end, 50.0, 80.0, 1.6, vec2(0.0, 1.0), k);
		draw_bramble(canvas, art::bramble_along(path, global::FLOWER_SIZE, #{ seed: k, origin: start, thickness: 2.5, reach: 600.0 }));
	}
	canvas
}

fn words_from(words, from, to) {
	let text = "";
	for i in from..to { text += if i == from { words[i] } else { " " + words[i] }; }
	text
}

// The text written at y, its glyphs' randomness drawn from `key`'s seed,
// so a block tried for size is written the same again.
fn written(text, y, key, settings) {
	seed(key);
	write_block(text, vec2(global::MARGIN, y), global::W - 2.0 * global::MARGIN, settings)
}

// Words break round the eyes (pieces of two letters at least), so the text
// hugs them.
let body = hand;
body.paragraph.clearance = 10.0;
body.paragraph.split_words = true;
let last_line = H - MARGIN;
let canvases = [];
let n = 0;
let canvas = body.start_page(n);
let y = MARGIN + SIZE;
for (paragraph, p) in paragraphs {
	let words = paragraph.split(" ");
	let from = 0;
	while from < words.len() {
		let key = 1000 * n + 10 * p + from;
		// The most words that fit above the bottom margin.
		let fits = |to| written(words_from(words, from, to), y, key, body).bottom - SIZE * hand.paragraph.line_height <= last_line;
		let lo = from;
		let hi = words.len();
		if fits.call(hi) { lo = hi; } else {
			while hi - lo > 1 {
				let mid = (lo + hi) / 2;
				if fits.call(mid) { lo = mid; } else { hi = mid; }
			}
		}
		if lo > from {
			let block = written(words_from(words, from, lo), y, key, body);
			canvas.add(block.canvas);
			y = block.bottom;
			from = lo;
		}
		if from < words.len() {
			canvases.push(canvas);
			n += 1;
			canvas = body.start_page(n);
			y = MARGIN + SIZE;
		}
	}
	y += PARAGRAPH_GAP;
}
canvases.push(canvas);

// Brambles in every page's margins, and across the space left at the
// bottom of the last.
for (canvas, n) in canvases {
	let free_from = if n == canvases.len() - 1 { y } else { H };
	canvas.add(page_brambles(n, free_from));
	journal_entry.add(new_page(#{ name: `becoming_us_page_${n}`, size: vec2(W, H) }).add(canvas));
}
