// The title: the mere-keeper's frontispiece. An engraved plate: the black
// mere at night under a full moon, the far shore and the hall on it, and
// a sword of black glass standing point down in the water, its hilt a
// cross against the moon. The water lies dead still, the moon's road on
// it unbroken but where the blade's reflection falls; under the surface,
// faint, the slow turning of things far larger. Under the plate a motto
// in two founts, plain and blackletter, which is Bacon's way of hiding a
// word (baconian, kinds 2: a letter to five of the motto's).

import "lib/inks" as inks;
import "lib/chance" as chance;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/gothic" as gothic;
import "lib/mere" as mb;

const INK = mb::ink();
const BLOOD = mb::blood();
const SEPIA = mb::sepia();

seed(3101);

// ---- the secret (chains/mere_title) ----

let hidden = chain("mere_title").run().get("motto.marks").value();

let leaf = mb::leaf("title", 31, #{ stains: 2, foxing: 120 });
let C = 600.0;
let PLATE = [vec2(130, 262), vec2(1070, 1180)];
let plate = shapes::closed([PLATE[0], vec2(PLATE[1].x, PLATE[0].y), PLATE[1], vec2(PLATE[0].x, PLATE[1].y)]);
let SHORE = 780.0;
let MOON = vec2(C, 470);
let MR = 118.0;

// ---- the sword, first: the rest is drawn behind it ----

let blade = gothic::sword(vec2(C, 548), vec2(C, SHORE + 150), 30.0, INK, #{ black: true, guard: 150.0, grip: 112.0 });
let sword_shape = blade.shape;

// ---- the sky ----

let scene = new_canvas();
let sky_box = shapes::closed([PLATE[0], vec2(PLATE[1].x, PLATE[0].y), vec2(PLATE[1].x, SHORE), vec2(PLATE[0].x, SHORE)]);
let halo = new_circle(MOON, MR + 34);
let sky = new_path();
let y = PLATE[0].y + 4.0;
while y < SHORE {
	sky.append(new_line(vec2(PLATE[0].x, y + 0.8 * (y * 0.3).sin()), vec2(PLATE[1].x, y - 0.8 * (y * 0.21).sin())));
	y += if y < 420.0 { 4.2 } else { 5.6 };
}
scene.add(sky.erase_outside(sky_box).erase_inside(halo).erase_inside(sword_shape, #{ fill_rule: "nonzero" }), inks::detail(INK, 0.9));
// Its few stars, bare.
for k in 0..26 {
	let at = vec2(PLATE[0].x + 30 + (PLATE[1].x - PLATE[0].x - 60) * chance::of(31, k, 1), PLATE[0].y + 20 + 400 * chance::of(31, k, 2));
	if (at - MOON).length() < MR + 60 || (at.x - C).abs() < 90.0 { continue; }
	scene.add(new_circle(at, 1.0 + 2.2 * chance::of(31, k, 3) ** 2.0), inks::fill(INK, 1.0));
}
// The moon: a ring and its seas, faint.
let moon = new_canvas();
moon.add(new_circle(MOON, MR).erase_inside(sword_shape, #{ fill_rule: "nonzero" }), inks::contour(INK, 1.6));
for sea in [[-34, -28, 30, 22], [26, 10, 38, 26], [-12, 44, 22, 16], [40, -46, 16, 12]] {
	let s = shapes::shape(shapes::ellipse_points(MOON + vec2(sea[0], sea[1]), sea[2], sea[3], 20));
	moon.add(s.hatch(3.0, #{ angle: [0.5], roughness: 0.8 }).erase_inside(sword_shape, #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.8));
}
leaf.figures.add(scene).add(moon);

// ---- the far shore: low land, dead trees, the hall ----

let land = shapes::steps(30, |j| {
	let x = PLATE[0].x + (PLATE[1].x - PLATE[0].x) * j / 30.0;
	vec2(x, SHORE - 18.0 - 14.0 * (x * 0.011 + 1.0).sin() - 8.0 * (x * 0.037).sin() - if x < 380.0 { 30.0 } else { 0.0 })
});
let ground = shapes::shape(land + [vec2(PLATE[1].x, SHORE + 2), vec2(PLATE[0].x, SHORE + 2)]);
let hall = new_path();
// The hall: a gable, a tower, one window lit.
hall.append(shapes::closed([vec2(196, SHORE - 50), vec2(196, SHORE - 112), vec2(246, SHORE - 150), vec2(296, SHORE - 112), vec2(296, SHORE - 50)]));
hall.append(shapes::closed([vec2(300, SHORE - 50), vec2(300, SHORE - 176), vec2(318, SHORE - 206), vec2(336, SHORE - 176), vec2(336, SHORE - 50)]));
let lit = new_rectangle(vec2(246, SHORE - 96), 10, 16);
let far = new_canvas();
let limbs = new_path().append(ground).append(hall);
let twigs = new_path();
for t in [[880.0, 0.9, 120.0, 4], [970.0, -0.7, 90.0, 9], [430.0, 0.5, 70.0, 2]] {
	let tree = gothic::bare_tree(vec2(t[0], SHORE - 26), -PI() / 2.0 + t[1] * 0.2, t[2], 9.0, 4, t[3]);
	for l in tree.limbs { limbs.append(shapes::shape(l)); }
	for tw in tree.twigs { twigs.append(new_polyline(shapes::curve(tw[0], 4))); }
}
engraving::mass(far, limbs, lit, INK, 1.4);
far.add(twigs.erase_inside(sword_shape, #{ fill_rule: "nonzero" }), inks::contour(INK, 1.1));
leaf.figures.add(far);

// ---- the water ----

let water = shapes::closed([vec2(PLATE[0].x, SHORE), vec2(PLATE[1].x, SHORE), PLATE[1], vec2(PLATE[0].x, PLATE[1].y)]);
// The blade's reflection: a dark column the moon's road breaks round.
let reflection = new_rectangle(vec2(C, SHORE + 200), 40, 400);
let road = engraving::moon_road(C, SHORE + 8.0, PLATE[1].y - 20.0, 70.0, 31, #{ split: 18.0 });
// Under it, turning: whorls cut so fine they're more felt than seen.
let deep = shapes::join([[330.0, 1050.0, 120.0, 1.8, 0.0], [850.0, 1000.0, 150.0, 2.2, 1.4], [600.0, 1140.0, 90.0, 1.5, 2.6], [980.0, 1150.0, 60.0, 1.3, 0.4], [220.0, 900.0, 50.0, 1.2, 3.0]].map(|w| {
	shapes::sliver(shapes::whorl_points(vec2(w[0], w[1]), w[2] * 0.12, w[2], w[3], 0.32, w[4]), 3.4)
}));
// Rings where something rose, round the blade.
let rings = shapes::join([46.0, 74.0, 110.0].map(|r| shapes::sliver(shapes::ellipse_points(vec2(C, SHORE + 8), r, r * 0.12, 60) + [vec2(C + r, SHORE + 8)], 2.6)));
// The far shore's light lying along the water under it.
for k in 0..22 {
	let x = PLATE[0].x + 20 + (PLATE[1].x - PLATE[0].x - 40) * chance::of(32, k, 1);
	let y = SHORE + 5 + 34.0 * chance::of(32, k, 2) ** 1.5;
	let half = 24.0 + 70.0 * chance::of(32, k, 3);
	if (x - C).abs() < 40.0 { continue; }
	rings.append(shapes::sliver([vec2(x - half, y), vec2(x, y + 0.3), vec2(x + half, y)], 2.4));
}
let pale = new_path().append(road).append(deep).append(rings);
let wet = new_canvas();
engraving::black_water(wet, water, pale, INK, #{ gap: 3.0, weight: 2.3, seed: 3 });
leaf.figures.add(wet);
leaf.figures.add(blade.canvas);

// The plate's rules.
let rules = new_canvas();
rules.add(plate, inks::contour(INK, 2.4));
rules.add(new_rectangle((PLATE[0] + PLATE[1]) / 2.0, PLATE[1].x - PLATE[0].x + 22, PLATE[1].y - PLATE[0].y + 22), inks::detail(INK, 1.1));
leaves::place(leaf, rules, 12);

// ---- the title ----

let words = new_canvas();
for line in [["THE", 104.0, 44.0], ["BLACK MERE", 190.0, 96.0]] {
	let written = write_line(line[0], vec2(0, 0), leaves::title_hand(line[2], INK));
	let b = written.bounds();
	words.add(written.translate_by(vec2(C, line[1]) - b.center));
	leaves::clear(leaf, new_rectangle(vec2(C, line[1]), b.max.x - b.min.x + 30, b.max.y - b.min.y + 20), 6);
}
leaf.figures.add(words);

// ---- the motto, in two founts ----

// Each letter plain where its mark is 0, blackletter where it's 1; what
// isn't a letter, and letters past the marks, plain.
let motto = new_canvas();
let k = 0;
for (line, row) in ["THE WATER IS STILL, AND THE STILL", "WATER IS DEEP."] {
	let set = new_canvas();
	let x = 0.0;
	for ch in line.chars() {
		if ch == ' ' { x += 18.0; continue; }
		let letter = ch >= 'A' && ch <= 'Z';
		let ornate = letter && k < hidden.len() && hidden[k] == 1;
		if letter { k += 1; }
		let hand = if ornate { leaves::title_hand(40.0, INK) } else { leaves::figures_hand(34.0, INK) };
		let g = write_line(ch.to_string(), vec2(0, 0), hand);
		let b = g.bounds();
		set.add(g.translate_by(vec2(x - b.min.x, -b.max.y)));
		x += b.max.x - b.min.x + 5.0;
	}
	let sb = set.bounds();
	motto.add(set.translate_by(vec2(C - (sb.min.x + sb.max.x) / 2.0, 1262 + row * 58)));
}
leaves::place(leaf, motto, 10);

leaves::jot(leaf, "The mere-keeper's book, of Hollin Fen.", vec2(340, 1384), 760, #{ size: 29 });
leaves::jot(leaf, "Do not go down to the water at night.", vec2(300, 1446), 800, #{ size: 32, ink: BLOOD, turn: -0.012 });

journal_entry.add(leaves::bind(leaf, content("mere"), #{ size: 20.0 }));
