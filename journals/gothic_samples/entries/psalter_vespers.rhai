// Vespers: the spread the psalter opens on. On the left, his account in
// the old script, a wall of it, and over it in blood the sigil she drew
// on the vestry wall (sigil): a word as one line through a ring of the
// letters, the key the pillars of the nave answer to. On the right, the
// one they drowned, as he saw her before the altar: arms out, her hair
// standing up about her as though the water had not let her go, in a ring
// of thirteen candles.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/gothic" as gothic;
import "lib/psalter" as ps;

const INK = ps::ink();
const BLOOD = ps::blood();
const SEPIA = ps::sepia();

seed(1301);

// ---- the left page: the sigil on the wall ----

let leaf = ps::leaf("vespers", 31);
const SIZE = 980.0;
let heart = vec2(600, 790);
let corner = heart - vec2(SIZE / 2.0, SIZE / 2.0);

// The ring of letters, each ringed, in ink: the wall runs between them.
let letters = symbol_set("letters", #{ color: INK, size: 40.0 });
let ring = illustration("sigil", #{ size: SIZE, bordered: true, glyph_ink: INK, editor: "ring" })
	.run(#{ word: symbols(27, ""), symbol_set: letters })[0];
leaves::place_open(leaf, ring.translate_by(corner), 10);
// A circle ruled under the ring, as she drew it first.
leaf.under.add(new_circle(heart, SIZE / 2.0 - 30.0), inks::pencil(SEPIA, 1.2));

// The sigil itself, laid over the wall in blood.
let line = #{ style: new_style(new_brush_style(BLOOD, "Fraktur", 7.0, #{ min_pressure: 0.5, max_pressure: 1.0, pressure_profile: "taper", pressure_wander: 0.15 }),
	inks::hand(1, 0.0, 6.0)) };
let sigil = illustration("sigil", #{ size: SIZE, show_ring: false })
	.run(#{ word: symbols(27, "LAZARUS"), symbol_set: letters, line_style: line })[0];
// Gone over again with a finer finger, not quite on the first.
let again = #{ style: new_style(new_brush_style(BLOOD, "Sanguine", 1.6, #{ min_pressure: 0.4, max_pressure: 0.9, pressure_profile: "taper" }), inks::hand(1, 0.0, 5.0)) };
let echo = illustration("sigil", #{ size: SIZE, show_ring: false })
	.run(#{ word: symbols(27, "LAZARUS"), symbol_set: letters, line_style: again })[0];

leaves::jot(leaf, "On the vestry wall, the third night, in what the fen left there. The pillars answer to it.", vec2(90, 70), 900, #{ size: 34, turn: -0.01 });

// The account's first letter, rubricated: its glyph large in blood, in a
// box ruled twice, and a briar trailing from it down the margin.
let cap = vec2(165, 268);
let framed = new_canvas();
framed.add(new_rectangle(cap, 150, 150), inks::contour(INK, 1.8));
framed.add(new_rectangle(cap, 136, 136), inks::detail(INK, 0.9));
framed.add(new_rectangle(cap, 136, 136).hatch(6.0, #{ angle: [0.8], roughness: 1.5 }), inks::fine(SEPIA, 0.7));
framed.add(write_at("t", cap, #{
	style: new_style(new_brush_style(BLOOD, "Fraktur", 5.0, #{ min_pressure: 0.6, max_pressure: 0.95, pressure_profile: "taper" }), inks::hand(2, 1.0, 1.0)),
	type: new_type_style(glyphset("eldritch"), 120),
}));
leaves::place(leaf, framed, 6);
let briar = new_polyline(shapes::curve([cap + vec2(-56, 78), vec2(102, 520), vec2(118, 760), vec2(94, 1000), vec2(114, 1240), vec2(100, 1470)], 16));
let plant = art::bramble_along(briar, 30.0, #{ seed: 7, origin: cap + vec2(-56, 78), thickness: 2.2 });
leaves::place_open(leaf, new_canvas().add(plant, #{ vines: inks::contour(INK, 1.3), leaves: inks::detail(INK, 0.9), flowers: inks::detail(INK, 1.0) }), 6);

let left = leaves::bind(leaf, content("psalter"), #{ size: 26.0 });
let over = new_canvas();
over.add(sigil.translate_by(corner));
over.add(echo.translate_by(corner + vec2(9, -7)));
left.add(over);
journal_entry.add(left);

// ---- the right page: the one they drowned ----

let leaf = ps::leaf("saint", 37, #{ stains: 1, foxing: 110 });
const C = 600.0;
let O = vec2(C, 820);
const R = 400.0;
let H = vec2(C, 440);

// What he set it out by: the upright, the circle, the cross at its heart.
leaf.under.add(new_line(vec2(C, 120), vec2(C, 1500)), inks::pencil(SEPIA, 1.0));
leaf.under.add(new_line(vec2(120, 820), vec2(1080, 820)), inks::pencil(SEPIA, 0.8));
leaf.under.add(new_arc(O, R + 150, -2.7, 0.5), inks::pencil(SEPIA, 0.9));
leaf.under.add(new_arc(O, R - 130, 0.9, 2.7), inks::pencil(SEPIA, 0.9));

// ---- her shape ----
// Drawn first, so what stands behind her can be cut away where she covers
// it: her head and the hair close about it, the neck, the torso thin as
// the drowned are, arms lifted as the saints' are in the old glass, the
// hands, and the shroud from the hips, torn at the hem.

let body = new_path();
body.append(shapes::closed(shapes::loop_curve([H + vec2(0, -66), H + vec2(52, -34), H + vec2(52, 30), H + vec2(26, 70), H + vec2(-26, 70), H + vec2(-52, 30), H + vec2(-52, -34)], 6)));
body.append(shapes::shape(shapes::tube([vec2(C, 480), vec2(C, 525), vec2(C, 610), vec2(C, 720), vec2(C, 815)], [32.0, 124.0, 84.0, 56.0, 96.0])));
let arm = [vec2(645, 518), vec2(720, 508), vec2(790, 480), vec2(838, 426), vec2(868, 374)];
let arm_w = [44.0, 26.0, 21.0, 18.0, 13.0];
body.append(shapes::shape(shapes::tube(arm, arm_w)));
body.append(shapes::shape(shapes::tube(shapes::mirrored(arm, C), [13.0, 18.0, 21.0, 26.0, 44.0])));
let skirt = shapes::curve([vec2(644, 800), vec2(664, 930), vec2(690, 1070), vec2(722, 1200), vec2(752, 1300)], 8);
let hem = [];
for k in 1..14 {
	let x = 752.0 - 304.0 * k / 14.0;
	hem.push(vec2(x, 1300 + (if k % 2 == 0 { 26.0 } else { -6.0 }) + 14.0 * (k * 2.9).sin()));
}
body.append(shapes::closed(skirt + hem + shapes::mirrored(skirt, C)));
// The tatters below the hem, drifting as they did in the water.
let tatters = [];
for (p, k) in hem {
	if k % 2 == 1 { continue; }
	let t = (p.x - C) / 150.0;
	let drift = vec2(t * 50 + 20.0 * (k * 0.9).sin(), 60 + 30.0 * (k * 1.9).cos());
	let way = [p + vec2(0, -10), p + drift * 0.5 + vec2(14.0 * (k * 1.1).cos(), 0), p + drift];
	tatters.push(way);
	body.append(shapes::shape(shapes::tube(way, [24.0, 14.0, 3.0])));
}
for side in [-1.0, 1.0] {
	let wrist = vec2(C + side * 268, 374);
	let palm = wrist + vec2(side * 9, -18);
	body.append(shapes::closed(shapes::loop_curve([palm + vec2(0, -16), palm + vec2(side * 13, 0), palm + vec2(0, 18), palm + vec2(-side * 12, 0)], 4)));
	for f in [[-0.25, 46.0], [-1.0, 66.0], [-1.3, 76.0], [-1.6, 70.0], [-1.9, 58.0]] {
		let a = if side > 0.0 { f[0] } else { PI() - f[0] };
		let way = shapes::polar(a);
		let base = palm + way * 8.0;
		body.append(shapes::shape(shapes::tube([base, base + way * f[1] * 0.5 + vec2(side * 3, 0), base + way * f[1]], [7.0, 5.0, 1.5])));
	}
}
// What's cut away behind her.
let behind = |p| p.erase_inside(body, #{ fill_rule: "nonzero" });

// ---- the ring of thirteen candles ----

// The thirteen round the ring, from just right of the top: each a point of
// the bells' system (see the belfry), named in the priests' figures, a
// candle standing out from it.
let turn_of = |k| -PI() / 2.0 + PI() / 13.0 + 2.0 * PI() * k / 13.0;
let at = |k, r| { let a = turn_of.call(k); O + shapes::polar(a) * r };
let ring = new_canvas();
ring.add(behind.call(new_circle(O, R)), inks::contour(INK, 2.2));
ring.add(behind.call(new_circle(O, R - 9)), inks::detail(INK, 1.1));
ring.add(behind.call(new_circle(O, R - 50)), inks::contour(INK, 1.6));
ring.add(behind.call(new_circle(O, R - 57)), inks::detail(INK, 0.9));
// Written round between the rings: what she did that night.
let round = new_circle(O, R - 30);
let words = leaves::wall_hand(20, INK);
words.avoid = body;
ring.add(round.fit_text(plates::inscription(round, "she lifted her arms and the thirteen candles lit of themselves", words), words));
// The thirteen-pointed star, every fifth joined, and every third, fainter.
let star = new_path().move_to(at.call(0, R - 57));
for k in 1..=13 { star.line_to(at.call((k * 5) % 13, R - 57)); }
ring.add(behind.call(star), inks::detail(INK, 1.1));
let star3 = new_path().move_to(at.call(0, R - 57));
for k in 1..=13 { star3.line_to(at.call((k * 3) % 13, R - 57)); }
ring.add(behind.call(star3), inks::fine(SEPIA, 0.8));
// The first triple, in blood: the three that ring together for A.
ring.add(behind.call(new_polyline([at.call(0, R - 57), at.call(1, R - 57), at.call(4, R - 57)], #{ closed: true })), inks::detail(BLOOD, 2.4));
let priests = symbol_set("priests", #{ color: INK, size: 38.0 });
for k in 0..13 {
	// The candle drawn standing up at the origin, then stood out from the
	// ring.
	let c = new_canvas();
	let wax = new_rectangle(vec2(0, -24), 17, 46);
	c.add(wax, inks::contour(INK, 1.4));
	c.add(wax.hatch(3.2, #{ angle: [0.0] }), inks::fine(INK, 0.6));
	c.add(new_polyline([vec2(-8.5, -46), vec2(-10, -36), vec2(-8, -30), vec2(-9, -24)]), inks::detail(INK, 1.4));
	c.add(new_polyline([vec2(8.5, -46), vec2(9.5, -40)]), inks::detail(INK, 1.4));
	c.add(new_line(vec2(0, -46), vec2(0, -53)), inks::detail(INK, 1.0));
	c.add(new_path().move_to(vec2(0, -50)).quad_to(vec2(12, -60), vec2(0, -92)).quad_to(vec2(-12, -60), vec2(0, -50)), inks::detail(INK, 1.2));
	c.add(inks::solid(new_ellipse(vec2(0, -61), 3.2, 6.5), 1.6), inks::fill(INK, 1.6));
	ring.add(c.rotate_around(vec2(0, 0), turn_of.call(k) + PI() / 2.0).translate_by(at.call(k, R + 4)));
	ring.add(write_at(priests.text([k]), at.call(k, R + 122), #{}));
}
leaves::place_open(leaf, ring, 6);

// ---- the halo, and the spears ----

let halo = new_canvas();
halo.add(behind.call(new_arc(H, 172, PI(), 2.0 * PI())), inks::contour(INK, 2.0));
halo.add(behind.call(new_arc(H, 154, PI(), 2.0 * PI())), inks::detail(INK, 1.2));
halo.add(behind.call(new_arc(H, 126, PI(), 2.0 * PI())), inks::detail(INK, 1.2));
let teeth = shapes::steps(30, |k| {
	let a = PI() + PI() * k / 30.0;
	let r = if k % 2 == 0 { 128.0 } else { 152.0 };
	H + shapes::polar(a) * r
});
halo.add(behind.call(new_polyline(teeth)), inks::detail(INK, 1.4));
for k in 0..15 {
	let a = PI() + PI() * (k + 0.5) / 15.0;
	halo.add(new_circle(H + shapes::polar(a) * 163, 2.6), inks::fill(INK, 2.0));
}
for k in 0..41 {
	let a = PI() + PI() * (k + 0.5) / 41.0;
	let long = if k % 4 == 1 { 250.0 } else { 205.0 };
	halo.add(new_line(H + shapes::polar(a) * 180, H + shapes::polar(a) * long), inks::fine(INK, 0.9));
}
leaf.figures.add(halo);

// Two spears crossed behind her, the blades up.
let spear = |butt, tip| {
	let c = new_canvas();
	let way = (tip - butt).normalized();
	let side = vec2(-way.y, way.x);
	let neck = tip - way * 74;
	c.add(behind.call(new_line(butt, neck)), inks::band(INK, 2.6));
	c.add(behind.call(new_line(butt + side * 4.0, neck + side * 4.0)), inks::fine(INK, 0.7));
	let blade = new_polyline([neck, neck + way * 20 + side * 13, tip, neck + way * 20 - side * 13], #{ closed: true });
	c.add(blade, inks::contour(INK, 1.6));
	c.add(new_line(neck, tip), inks::fine(INK, 0.8));
	c.add(new_line(neck - side * 13, neck + side * 13), inks::contour(INK, 1.8));
	for k in 0..3 { c.add(new_line(neck - way * (10 + k * 7) - side * 4, neck - way * (10 + k * 7) + side * 4), inks::detail(INK, 1.0)); }
	c
};
leaf.figures.add(spear.call(vec2(270, 1360), vec2(1070, 900)));
leaf.figures.add(spear.call(vec2(930, 1360), vec2(130, 900)));

// ---- her ----

let her = new_canvas();
// Her hair, standing up out of her as though the water still held it:
// locks from the crown, each drawn from its tip down to the root, so the
// pen presses hardest at her head.
let heavy_lock = new_style(new_brush_style(INK, "Strands", 14.0, #{ min_pressure: 0.06, max_pressure: 1.0, pressure_profile: "ramp" }), new_handwriting_style());
let fine_lock = new_style(new_brush_style(INK, "Strands", 9.0, #{ min_pressure: 0.04, max_pressure: 0.9, pressure_profile: "ramp" }), new_handwriting_style());
for k in 0..25 {
	let t = k / 24.0;
	let a = PI() * (1.06 + 0.88 * t);
	let root = H + vec2(a.cos() * 40, -6 + a.sin() * 50);
	let lean = (t - 0.5) * 2.0;
	let len = 230.0 + 110.0 * (1.0 - (lean.abs() ** 1.5)) + 40.0 * (k * 2.3).sin();
	let pts = shapes::steps(7, |j| {
		let s = j / 7.0;
		let wave = (s * 6.4 + k * 1.7).sin() * (6.0 + 46.0 * s);
		let up = vec2(lean * (0.3 + 1.5 * s), -1.0).normalized();
		root + up * len * s + vec2(-up.y, up.x) * wave
	});
	pts.reverse();
	her.add(new_polyline(shapes::curve(pts, 10)), if k % 3 == 0 { heavy_lock } else { fine_lock });
}

// Her face, bare in the dark of her hair: shut hollows for eyes, the
// water still running from them.
let face = shapes::times(48, |k| {
	let a = 2.0 * PI() * k / 48.0;
	let chin = 1.0 - 0.32 * max(a.sin(), 0.0) ** 2.0;
	H + vec2(a.cos() * 31 * chin, 14 + a.sin() * 42)
});
let face = shapes::wound(face);

// What's left bare: her face, the ribs, the breastbone, the light down
// her arms, the folds of the shroud and its tatters.
let bare = new_path();
bare.append(shapes::shape(face));
for k in 0..5 {
	let y = 566.0 + k * 25.0;
	for side in [-1.0, 1.0] {
		bare.append(shapes::sliver([vec2(C + side * 7, y), vec2(C + side * 26, y + 7 + k * 1.5), vec2(C + side * (40 - k * 3), y + 22)], 5.0));
	}
}
bare.append(shapes::sliver([vec2(C, 540), vec2(C, 600), vec2(C, 690)], 4.0));
for side in [-1.0, 1.0] {
	let lit = [];
	for (p, k) in arm { lit.push(vec2(C + side * (p.x - C), p.y - arm_w[k] * 0.18)); }
	bare.append(shapes::sliver(lit, 4.0));
}
for k in 0..7 {
	let t = (k - 3.0) / 3.0;
	bare.append(shapes::sliver([vec2(C + t * 22, 860), vec2(C + t * 60 + 6.0 * (k * 1.7).sin(), 1060), vec2(C + t * 120, 1270)], 4.0 + 1.5 * (k % 2)));
}
for way in tatters { bare.append(shapes::sliver([way[0] + vec2(0, 14), way[1], way[2] - (way[2] - way[1]) * 0.3], 2.4)); }
engraving::mass(her, body, bare, INK, 2.0);
her.add(shapes::shape(face), inks::contour(INK, 1.3));
for side in [-1.0, 1.0] {
	let eye = H + vec2(side * 13, 8);
	her.add(inks::solid(new_ellipse(eye, 9, 4.2, #{ angle: side * 0.12 }), 1.4), inks::fill(INK, 1.4));
	her.add(new_arc(eye + vec2(0, 2), 13, 3.5, 5.9), inks::fine(INK, 1.0));
	her.add(new_polyline([eye + vec2(side * 2, 4), eye + vec2(side * 3, 16), eye + vec2(side * 2, 30)]), inks::fine(INK, 0.8));
}
her.add(new_polyline([H + vec2(1, 10), H + vec2(-2, 30), H + vec2(3, 33)]), inks::fine(INK, 1.1));
her.add(inks::solid(new_ellipse(H + vec2(0, 44), 5, 3.4), 1.4), inks::fill(INK, 1.4));
leaves::place(leaf, her, 6);

// ---- the sky over her ----

// A moon gone dark, high on the right, and the rooks that won't settle.
let moon = art::moon(vec2(1035, 175), 58, 0.03);
leaf.figures.add(inks::moon(moon, INK, 2.0, #{ gap: 2.6 }));
leaf.figures.add(new_circle(vec2(1035, 175), 72), inks::fine(INK, 0.9));
for r in [[vec2(150, 190), 74.0, 0.6, -0.1], [vec2(270, 130), 54.0, -0.4, 0.15], [vec2(240, 270), 44.0, 0.9, 0.05], [vec2(360, 210), 34.0, -0.8, -0.2], [vec2(120, 320), 30.0, 0.2, 0.3]] {
	engraving::dark(leaf.figures, gothic::rook(r[0], r[1], r[2], r[3]), INK, 1.8);
}

leaves::jot(leaf, "Before the altar, the eve of Candlemas. Thirteen candles, and every one lit of itself.", vec2(70, 1400), 470, #{ size: 29, turn: -0.02 });
leaves::jot(leaf, "Nought, one and four ring together: the first letter.", vec2(790, 1400), 340, #{ size: 29, ink: BLOOD, turn: 0.02 });

journal_entry.add(leaves::bind(leaf, ""));
