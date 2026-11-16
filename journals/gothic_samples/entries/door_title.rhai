// The title: the night-book's frontispiece. A looking-glass in a carved
// frame, crested with a star that watches; in the glass the heath at
// night, the black door on it with the light under it, the three that
// wander stood in their line over it at last, and behind whoever looks,
// standing close, something with no face. The title over it in
// blackletter, whose book it is under it, and her account walls the leaf.

import "lib/inks" as inks;
import "lib/chance" as chance;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/gothic" as gothic;
import "lib/nocturne" as nb;

const INK = nb::ink();
const COLD = nb::cold();
const SEPIA = nb::sepia();

seed(2001);

let leaf = nb::leaf("title", 21, #{ stains: 2, foxing: 130 });
let C = 600.0;
let G = vec2(C, 838);
let RX = 300.0;
let RY = 390.0;
let frame = gothic::looking_glass(G, RX, RY, INK, #{ seed: 11, width: 56.0 });
let glass = frame.glass;

// ---- what the glass shows ----

let HORIZON = 972.0;
let FOOT = 1062.0;
let DOOR = [vec2(C - 34, FOOT - 150), vec2(C + 34, FOOT)];

// Behind the one who looks: tall, close, faceless, its head bowed to the
// shoulder that's looked over.
let head = shapes::shape(shapes::ellipse_points(vec2(770, 930), 36, 60, 48)).rotate_around(vec2(774, 934), -0.34);
let body = shapes::closed(shapes::curve([vec2(640, 1260), vec2(662, 1134), vec2(700, 1074), vec2(752, 1050), vec2(770, 1002), vec2(792, 998), vec2(802, 1048), vec2(868, 1074), vec2(930, 1124), vec2(958, 1260)], 8));
let it = new_path().append(head).append(body);

let scene = new_canvas();
// The sky engraved, darker low, bare round every star.
let stars = [
	[392, 560, 20, "lashed"], [818, 548, 22, "lashed"], [726, 470, 13, "lashed"], [360, 790, 13, "lashed"], [868, 760, 12, "lashed"],
	[480, 476, 10, "needle"], [520, 730, 8, "needle"], [700, 640, 11, "spiral"], [430, 900, 8, "needle"], [880, 890, 8, "weeping"],
	[620, 448, 7, "needle"], [330, 680, 7, "needle"],
];
// The three that wander, in one line over the door: the night it opens.
let wanderers = [[C, 860, 15, "lashed"], [C, 760, 13, "lashed"], [C, 654, 14, "lashed"]];
let glare = new_path();
let lights = new_canvas();
for (s, k) in stars + wanderers {
	let at = vec2(s[0], s[1]);
	lights.add(gothic::watcher(at, s[2] * 1.0, 80 + k * 7, INK, #{ kind: s[3], reach: if s[3] == "lashed" { 1.9 } else { 2.6 } }));
	glare.append(new_circle(at, s[2] * 2.6));
}
for k in 0..40 {
	let at = vec2(310 + 580 * chance::of(21, k, 1), 450 + (HORIZON - 480) * chance::of(21, k, 2));
	let r = 1.4 + 2.6 * (chance::of(21, k, 3) ** 2.0);
	let near = false;
	for s in stars + wanderers { if (at - vec2(s[0], s[1])).length() < s[2] * 3.0 { near = true; } }
	if near { continue; }
	lights.add(new_circle(at, r * 0.5), inks::fill(INK, 1.0));
	glare.append(new_circle(at, r * 2.0));
}
let sky = new_path();
let y = 440.0;
while y < HORIZON + 30.0 {
	let pts = shapes::steps(30, |j| {
		let x = 290.0 + 620.0 * j / 30.0;
		vec2(x, y + 1.4 * (x * 0.013 + y * 0.07).sin())
	});
	sky.append(new_polyline(pts));
	y += if y < 820.0 { 6.0 } else { 4.6 };
}
let hills = shapes::steps(20, |j| {
	let x = 280.0 + 640.0 * j / 20.0;
	vec2(x, HORIZON - 12.0 * (x * 0.009 + 0.4).sin() - 6.0 * (x * 0.023).sin() + if (x - C).abs() < 120.0 { 8.0 } else { 0.0 })
});
let land = shapes::shape(hills + [vec2(920, 1260), vec2(280, 1260)]);
let spill = shapes::shape([vec2(DOOR[0].x, FOOT), vec2(DOOR[1].x, FOOT), vec2(C + 150, 1260), vec2(C - 150, 1260)]);
let door_box = new_rectangle(vec2(C, FOOT - 84), 110, 190);
let keep_out = |p| p.erase_inside(door_box).erase_inside(it, #{ fill_rule: "nonzero" }).erase_outside(glass);
scene.add(keep_out.call(sky.erase_inside(land).erase_inside(glare)), inks::fine(SEPIA, 0.8));
scene.add(keep_out.call(land.hatch(3.6, #{ angle: [0.06], roughness: 1.2 }).erase_inside(spill)), inks::detail(INK, 0.7));
scene.add(keep_out.call(land.hatch(4.2, #{ angle: [2.5], roughness: 1.2 }).erase_inside(spill)), inks::fine(INK, 0.7));
scene.add(keep_out.call(new_polyline(hills)), inks::contour(INK, 1.4));
let rays = shapes::join(shapes::times(7, |j| {
	let f = j / 6.0;
	new_line(vec2(DOOR[0].x + 68.0 * f, FOOT + 6), vec2(C - 140 + 280 * f, 1250))
}));
scene.add(keep_out.call(rays), inks::fine(SEPIA, 0.8));
let tufts = new_path();
for k in 0..90 {
	let f = chance::of(22, k, 1);
	let at = vec2(290 + 620 * chance::of(22, k, 2), HORIZON + 14 + 260 * f * f);
	let h = 4.0 + 14.0 * f;
	if (at.x - C).abs() < 60.0 + 90.0 * f { continue; }
	for j in [-1.0, 0.0, 1.0] { tufts.append(new_line(at, at + vec2(j * h * 0.35, -h))); }
}
scene.add(keep_out.call(tufts), inks::detail(INK, 0.9));
// The door itself, black, no knob, in its stones; light under it.
let stones = new_path();
stones.append(new_rectangle(vec2(C, DOOR[0].y - 9), 104, 18));
for side in [-1.0, 1.0] { stones.append(new_rectangle(vec2(C + side * 43, (DOOR[0].y + FOOT) / 2.0), 18, 150)); }
scene.add(stones, inks::contour(INK, 1.4));
scene.add(stones.hatch(2.6, #{ angle: [1.0] }), inks::detail(INK, 0.7));
let gap = shapes::sliver([vec2(DOOR[0].x + 2, FOOT - 5), vec2(C, FOOT - 6), vec2(DOOR[1].x - 2, FOOT - 5)], 4.0);
engraving::mass(scene, shapes::closed([DOOR[0], vec2(DOOR[1].x, DOOR[0].y), DOOR[1], vec2(DOOR[0].x, DOOR[1].y)]), gap, INK, 1.5);
// And what stands behind, in solid ink, the glass's sheen over all.
engraving::mass(scene, it.erase_outside(glass), new_path(), INK, 1.6);
leaf.figures.add(scene);
leaf.figures.add(lights);
gothic::glints(leaf.figures, glass, G, RX * 0.9, SEPIA);
leaves::place(leaf, frame.canvas, 12);
leaf.avoid.add_region(frame.outer, #{ margin: 12 });

// ---- the title ----

let words = new_canvas();
for line in [["THE", 98.0, 44.0], ["BLACK DOOR", 184.0, 96.0]] {
	let written = write_line(line[0], vec2(0, 0), leaves::title_hand(line[2], INK));
	let b = written.bounds();
	words.add(written.translate_by(vec2(C, line[1]) - b.center));
	leaves::clear(leaf, new_rectangle(vec2(C, line[1]), b.max.x - b.min.x + 30, b.max.y - b.min.y + 20), 6);
}
leaf.figures.add(words);

// ---- whose book ----

leaves::jot(leaf, "Her night-book: the dream, from the equinox, and what came of it.", vec2(96, 1404), 1010, #{ size: 29 });
leaves::jot(leaf, "Do not wake me if I walk.", vec2(410, 1462), 520, #{ size: 32, ink: COLD, turn: -0.015 });

journal_entry.add(leaves::bind(leaf, content("nights"), #{ size: 20.0 }));
