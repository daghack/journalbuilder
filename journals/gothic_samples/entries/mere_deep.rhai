// The deep. On the left, the shallows at the mere's foot, where three
// tablets of black glass lie on the shingle: the three squares of the
// cube, nine letters cut in each, in the order the word on the rail
// gives; and on the whetstone beside them, what they turn (trifid, key
// OBSIDIAN, in fives). On the right, the mere at its middle, where it
// opens without a sound: two whirls, each ringed with every letter, the
// left and the right wheels as they stood the night it began
// (chaocipher), and under them what they gave.

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

seed(7101);

let ALPHA = "-ABCDEFGHIJKLMNOPQRSTUVWXYZ";
let spelled = |syms| { let t = ""; for s in syms { t += ALPHA.sub_string(s, 1); } t };

// ---- the secrets (chains/mere_deep) ----

let puzzle = chain("mere_deep");
let secrets = puzzle.run();
let solved = puzzle.unlock(secrets);
let turned = secrets.get("tablets.cipher_text").value();
let LEFT = solved.text("whirls.left");
let RIGHT = solved.text("whirls.right");
let given = secrets.get("whirls.cipher_text").value();

// The cube's order: the key's letters, then the rest.
let order = [];
let all = solved.get("tablets.key").value();
for s in 0..27 { all.push(s); }
for s in all { if !order.contains(s) { order.push(s); } }

// ---- the left page: the shallows ----

let leaf = mb::leaf("shallows", 71, #{ foxing: 100, stains: 1 });
let DEEP_EDGE = 560.0;
// The tablets, worked out first: everything is drawn round them.
let tablets = [[vec2(270, 830), -0.16], [vec2(612, 1004), 0.08], [vec2(944, 812), 0.2]];
let slabs = new_path();
let cells = new_path();
let cut_letters = new_canvas();
for (t, k) in tablets {
	let c = t[0];
	let a = t[1];
	let corner = |x, y| c + vec2(x, y).rotated(a);
	slabs.append(shapes::closed([corner.call(-118, -112), corner.call(112, -122), corner.call(124, 116), corner.call(-110, 120)]));
	for n in 0..9 {
		let at = corner.call((n % 3 - 1) * 66.0, (n / 3 - 1) * 66.0);
		cells.append(shapes::closed([at + vec2(-26, -26).rotated(a), at + vec2(26, -26).rotated(a), at + vec2(26, 26).rotated(a), at + vec2(-26, 26).rotated(a)]));
		let s = order[k * 9 + n];
		cut_letters.add(leaves::centred(if s == 0 { "-" } else { ALPHA.sub_string(s, 1) }, at, leaves::figures_hand(30.0, INK)).rotate_around(at, a));
	}
	// Its number, I to III, on its upper edge.
	cut_letters.add(leaves::centred(["I", "II", "III"][k], corner.call(0, -150), leaves::figures_hand(24.0, INK)).rotate_around(corner.call(0, -150), a));
}
// The whetstone, long, across the foot of the shingle.
let WHET = [vec2(150, 1236), vec2(1060, 1210)];
let whet = shapes::closed([WHET[0] + vec2(0, -34), WHET[1] + vec2(0, -30), WHET[1] + vec2(14, 30), WHET[0] + vec2(-10, 36)]);
let lying = new_path().append(slabs).append(whet);

// The sky and the far shore, small; the mere black below them, out to
// the deep's edge; then the shallows, pale, rippled, over shingle.
let SHORE = 196.0;
let top = new_canvas();
let sky = new_path();
let y = 40.0;
while y < SHORE { sky.append(new_line(vec2(20, y), vec2(1180, y + 1.5))); y += 4.6; }
let said = leaves::note("Three, lying where the water leaves the shingle at the dry end of summer. They were not there in spring.", vec2(70, 42), 1060, #{ ink: INK, size: 27, turn: 0.004 });
top.add(sky.erase_inside(said.hole), inks::fine(SEPIA, 0.8));
let land = shapes::steps(24, |j| {
	let x = j * 50.0;
	vec2(x, SHORE - 12.0 - 10.0 * (x * 0.012 + 2.0).sin() - 6.0 * (x * 0.031).sin())
});
engraving::mass(top, shapes::shape(land + [vec2(1200, SHORE + 2), vec2(0, SHORE + 2)]).append(gothic::reeds(SHORE, 860, 1180, 90, 22, 71)), new_path(), INK, 1.2);
let deep = [vec2(0, SHORE), vec2(1200, SHORE)];
for j in 0..=24 {
	let x = 1200.0 - j * 50.0;
	deep.push(vec2(x, DEEP_EDGE + 30.0 * (x * 0.006 + 1.0).sin() + 10.0 * chance::either_way(72, j, 1)));
}
let mere = shapes::closed(deep);
let pale = engraving::moon_road(780.0, SHORE + 8.0, DEEP_EDGE - 20.0, 50.0, 72);
for w in [[300.0, 420.0, 90.0, 1.7, 0.3], [600.0, 480.0, 60.0, 1.3, 2.4]] {
	pale.append(shapes::sliver(shapes::whorl_points(vec2(w[0], w[1]), w[2] * 0.1, w[2], w[3], 0.3, w[4]), 3.6));
}
engraving::black_water(top, mere, pale, INK, #{ gap: 2.8, weight: 2.0, seed: 71 });
leaf.figures.add(top).add(said.canvas);
// The shallows: ripples and shingle, round what lies on it.
let shallows = new_canvas();
let ripples = new_path();
let y = DEEP_EDGE + 46.0;
while y < 1340.0 {
	let pts = [];
	for j in 0..=40 { let x = 20.0 + 1160.0 * j / 40.0; pts.push(vec2(x, y + 4.0 * (x * 0.02 + y * 0.05).sin())); }
	ripples.append(new_polyline(pts));
	y += 10.0 + (1340.0 - y) * 0.012;
}
shallows.add(ripples.erase_inside(lying, #{ fill_rule: "nonzero" }), inks::fine(INK, 0.7));
let shingle = shapes::join(shapes::times(160, |k| {
	let at = vec2(30 + 1140 * chance::of(73, k, 1), DEEP_EDGE + 80 + 680 * chance::of(73, k, 2));
	let r = 5.0 + 12.0 * chance::of(73, k, 3) ** 2.0;
	let pts = shapes::times(7, |j| {
		let a = j * 2.0 * PI() / 7.0;
		let q = r * (0.7 + 0.4 * chance::of(74, k, 10 + j));
		at + vec2(a.cos() * q, a.sin() * q * 0.6)
	});
	shapes::shape(pts)
}));
shallows.add(shingle.erase_inside(lying, #{ fill_rule: "nonzero" }), inks::detail(INK, 1.0));
leaf.figures.add(shallows);
// The tablets: black, their cells bare, an edge catching the light.
let glass = new_canvas();
let shine = shapes::join(tablets.map(|t| {
	let c = t[0];
	let a = t[1];
	shapes::sliver([c + vec2(-112, -108).rotated(a), c + vec2(0, -114).rotated(a), c + vec2(106, -118).rotated(a)], 3.0)
}));
engraving::mass(glass, slabs, new_path().append(cells).append(shine), INK, 1.3);
glass.add(cells, inks::detail(INK, 1.0));
leaf.figures.add(glass).add(cut_letters);
// The whetstone, grey, the groups cut along it in red.
let stone = new_canvas();
stone.add(whet, inks::contour(INK, 1.8));
stone.add(whet.hatch(3.2, #{ angle: [0.5], roughness: 0.8 }), inks::detail(INK, 0.8));
let groups = "";
for (s, k) in turned {
	if k > 0 && k % 5 == 0 { groups += "  "; }
	groups += ALPHA.sub_string(s, 1);
}
let g = write_line(groups, vec2(0, 0), leaves::figures_hand(32.0, BLOOD));
let gb = g.bounds();
let mid = (WHET[0] + WHET[1]) / 2.0;
stone.add(new_rectangle(mid, gb.max.x - gb.min.x + 30, 44).rotate_around(mid, -0.028), inks::detail(INK, 0.8));
stone.add(g.translate_by(mid - gb.center).rotate_around(mid, -0.028));
leaf.figures.add(stone);

leaves::jot(leaf, "On the whetstone, in fives: write each as its tablet, row and column, all in one line; cut the line in three and read down.", vec2(70, 1400), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the whirls ----

let leaf = mb::leaf("churn", 73, #{ foxing: 110, stains: 1 });
let SHORE = 230.0;
let whirls = [[vec2(330, 620), LEFT, "LEFT"], [vec2(870, 1010), RIGHT, "RIGHT"]];
let R_IN = 236.0;
let R_OUT = 286.0;
let top = new_canvas();
let sky = new_path();
let y = 40.0;
while y < SHORE { sky.append(new_line(vec2(20, y), vec2(1180, y + 1.5))); y += 4.4; }
let said = leaves::note("Where it opens. Twice I have seen it, and the moon lie on it whole again after.", vec2(70, 46), 820, #{ ink: INK, size: 27, turn: 0.004 });
let MOON = vec2(1090, 118);
top.add(sky.erase_inside(said.hole).erase_inside(new_circle(MOON, 52)), inks::fine(SEPIA, 0.8));
let moon = art::moon(MOON, 34, 0.0);
top.add(new_canvas().add(moon, #{ outline: inks::contour(INK, 1.3), terminator: inks::contour(INK, 1.1) }));
let land = shapes::steps(24, |j| {
	let x = j * 50.0;
	vec2(x, SHORE - 14.0 - 12.0 * (x * 0.009 + 0.5).sin() - (if x > 900.0 && x < 1010.0 { 50.0 } else { 0.0 }))
});
let lit = new_rectangle(vec2(950, SHORE - 50), 8, 12);
engraving::mass(top, shapes::shape(land + [vec2(1200, SHORE + 2), vec2(0, SHORE + 2)]), lit, INK, 1.2);
leaf.figures.add(top).add(said.canvas);
// The water, and in it the two whirls, ringed.
let water = shapes::closed([vec2(0, SHORE), vec2(1200, SHORE), vec2(1200, 1352), vec2(0, 1352)]);
// The moon on it whole, as after; its road broken under it.
let bare = shapes::join([shapes::closed(shapes::ellipse_points(vec2(MOON.x, 400), 36, 12, 40)), engraving::moon_road(MOON.x, 432.0, 660.0, 40.0, 74)]);
let wheel_letters = new_canvas();
for w in whirls {
	let c = w[0];
	let outer = shapes::wound(shapes::ellipse_points(c, R_OUT, R_OUT, 120));
	let inner = shapes::wound(shapes::ellipse_points(c, R_IN, R_IN, 120));
	let back = shapes::reversed(inner);
	bare.append(shapes::shape(outer)).append(shapes::shape(back));
	for k in 0..3 {
		bare.append(shapes::sliver(shapes::whorl_points(c, 34.0, R_IN - 22.0, 1.15, 1.0, k * 2.0 * PI() / 3.0), 6.0));
		bare.append(shapes::sliver(shapes::whorl_points(c, 60.0, R_IN - 40.0, 0.8, 1.0, k * 2.0 * PI() / 3.0 + 1.0), 3.0));
	}
	bare.append(shapes::sliver(shapes::ellipse_points(c, 24, 24, 30) + [c + vec2(24, 0)], 2.4));
	// The wheel: every letter, from the top round clockwise.
	for (ch, k) in w[1].chars() {
		let a = -PI() / 2.0 + 2.0 * PI() * k / 27.0;
		let at = c + shapes::polar(a) * ((R_IN + R_OUT) / 2.0);
		wheel_letters.add(leaves::centred(if ch == ' ' { "-" } else { ch.to_string() }, at, leaves::figures_hand(28.0, INK)).rotate_around(at, a + PI() / 2.0));
	}
}
let wet = new_canvas();
engraving::black_water(wet, water, bare, INK, #{ gap: 2.8, weight: 2.0, seed: 73 });
leaf.figures.add(wet).add(wheel_letters);
// Each wheel's top marked, and its halfway, in red; its name beside it.
let marks = new_canvas();
for w in whirls {
	let c = w[0];
	let tip = c + vec2(0, -R_OUT - 6);
	let notch = shapes::closed([tip + vec2(-11, -20), tip + vec2(11, -20), tip]);
	marks.add(inks::solid(notch, 1.1), inks::fill(BLOOD, 1.1));
	let a = -PI() / 2.0 + 2.0 * PI() * 13.0 / 27.0;
	let half = c + shapes::polar(a) * (R_OUT + 16);
	marks.add(inks::solid(new_circle(half, 6), 1.0), inks::fill(BLOOD, 1.0));
}
leaf.figures.add(marks);

// What they gave, in red, on the page below the water.
let line = spelled.call(given);
let gave = write_block(line, vec2(80, 1384), 1040, leaves::figures_hand(30.0, BLOOD)).canvas;
leaf.figures.add(gave);
leaves::jot(leaf, "Find each on the left whirl and write the right's beside it; then turn both, as the water turns them.", vec2(70, 1446), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, ""));
