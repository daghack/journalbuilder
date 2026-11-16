// The heath: the dream, every night since the equinox. On the left, the
// black door with no knob, alone in the heather in its frame of grey
// stone, light under it and nothing behind it, and the stars over it,
// which are not stars: they open, and watch. On the right, the three of
// them that wander, which the sleeper could only bear to watch in a
// hand-glass, the same month a year apart (star_alignment,
// star_sightings): the night they stand in one line over the door is the
// key to the knocking.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/chance" as chance;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/gothic" as gothic;
import "lib/nocturne" as nb;

const INK = nb::ink();
const COLD = nb::cold();
const SEPIA = nb::sepia();

seed(2101);

// ---- the left page: the door ----

let leaf = nb::leaf("door", 71, #{ stains: 1, foxing: 110 });
let C = 600.0;
let HORIZON = 1120.0;
let FOOT = 1190.0;
let DOOR = [vec2(452, 640), vec2(748, FOOT)];

// The notes first: what's drawn keeps out of them.
let said = leaves::note("The heath, every night since the equinox. Nothing behind it but more heath.", vec2(70, 1430), 440, #{ ink: INK, size: 28, turn: -0.01 });
let warned = leaves::note("Three of them wander. The night they stand in one line over it, it opens.", vec2(690, 1430), 440, #{ size: 28, ink: COLD, turn: 0.01 });
let notes = new_path().append(said.hole).append(warned.hole);

// The stars that watch: eyes in lashes, and among them needles, a spiral
// or two, one that weeps. [x, y, radius, kind]
let stars = [
	[150, 170, 30, "lashed"], [330, 110, 18, "lashed"], [870, 150, 34, "lashed"], [1050, 260, 22, "lashed"],
	[230, 420, 22, "lashed"], [990, 470, 26, "lashed"], [120, 700, 16, "lashed"], [1080, 760, 18, "lashed"],
	[400, 300, 12, "needle"], [760, 380, 14, "spiral"], [300, 640, 13, "needle"], [880, 640, 12, "weeping"],
	[470, 520, 10, "needle"], [1100, 980, 12, "needle"], [90, 960, 11, "needle"], [780, 90, 10, "needle"],
];
// The three that wander, nearly in a line over the door.
let wanderers = [[574, 470, 24, "lashed"], [632, 300, 20, "lashed"], [588, 150, 22, "lashed"]];
let glare = new_path();
let sky_ink = new_canvas();
for (s, k) in stars + wanderers {
	let at = vec2(s[0], s[1]);
	sky_ink.add(gothic::watcher(at, s[2] * 1.0, 40 + k * 3, INK, #{ kind: s[3], reach: if s[3] == "lashed" { 1.9 } else { 2.6 } }));
	glare.append(new_circle(at, s[2] * 2.7));
}
// Glints too small to look back: the rest of the sky.
for k in 0..90 {
	let at = vec2(60 + 1080 * chance::of(71, k, 1), 70 + (HORIZON - 120) * (chance::of(71, k, 2) ** 1.3));
	let r = 1.5 + 4.0 * (chance::of(71, k, 3) ** 3.0);
	let near = false;
	for s in stars + wanderers { if (at - vec2(s[0], s[1])).length() < s[2] * 3.0 { near = true; } }
	if near || (at.x > 380 && at.x < 820 && at.y > 560) { continue; }
	let rays = shapes::join(shapes::times(4, |j| {
		let a = PI() * j / 2.0 + 0.3 * chance::of(71, k, 4);
		new_line(at, at + shapes::polar(a) * r * 2.6)
	}));
	sky_ink.add(rays, inks::fine(INK, 0.6));
	sky_ink.add(new_circle(at, r * 0.4), inks::fill(INK, 1.0));
	glare.append(new_circle(at, r * 2.2));
}
// Where they'll stand: a line pricked through from the lintel up, in
// pencil.
let pricked = new_path();
let y = 560.0;
while y > 80.0 { pricked.append(new_line(vec2(C + (y - 560) * 0.012, y), vec2(C + (y - 554) * 0.012, y - 8))); y -= 18.0; }
leaf.figures.add(pricked.erase_inside(glare), inks::detail(COLD, 1.2));

// The sky itself, engraved: close lines, a little wavering, darker low.
let sky = new_path();
let y = 64.0;
while y < HORIZON + 30.0 {
	let pts = shapes::steps(40, |j| {
		let x = 30.0 + 1140.0 * j / 40.0;
		vec2(x, y + 1.6 * (x * 0.011 + y * 0.07).sin())
	});
	sky.append(new_polyline(pts));
	y += if y < 700.0 { 7.0 } else { 5.5 };
}

// The heath: rolling, and the path worn through the heather to the door.
let hills = shapes::steps(24, |j| {
	let x = 20.0 + 1160.0 * j / 24.0;
	vec2(x, HORIZON - 18.0 * (x * 0.006 + 0.8).sin() - 10.0 * (x * 0.017).sin() + if (x - C).abs() < 220.0 { 14.0 } else { 0.0 })
});
let land = shapes::shape(hills + [vec2(1180, 1600), vec2(20, 1600)]);
sky = sky.erase_inside(land).erase_inside(glare);
let path_shape = shapes::shape(shapes::curve([vec2(470, 1600), vec2(520, 1420), vec2(560, 1280), vec2(470, 1200), vec2(440, FOOT + 4)], 8)
	+ shapes::curve([vec2(760, FOOT + 4), vec2(700, 1220), vec2(660, 1300), vec2(700, 1430), vec2(780, 1600)], 8));
// The light under the door, fallen out over the heath.
let spill = shapes::shape([vec2(456, FOOT), vec2(744, FOOT), vec2(900, 1440), vec2(300, 1440)]);
let ground = land.hatch(4.2, #{ angle: [0.08], roughness: 1.6 }).erase_inside(path_shape).erase_inside(spill).erase_inside(notes);
// The path's dust, lightly, but not where the light lies.
let dust = path_shape.hatch(6.0, #{ angle: [0.08], roughness: 1.2 }).erase_inside(spill).erase_inside(notes);
let darker = land.hatch(4.6, #{ angle: [2.5], roughness: 1.4 }).erase_inside(path_shape).erase_inside(spill).erase_inside(notes);
let door_box = new_rectangle((DOOR[0] + DOOR[1]) / 2.0 + vec2(0, -20), 420, 650);
let scene = new_canvas();
scene.add(sky.erase_inside(door_box).erase_inside(notes), inks::fine(SEPIA, 0.8));
scene.add(ground.erase_inside(door_box), inks::detail(INK, 0.7));
scene.add(darker.erase_inside(door_box), inks::fine(INK, 0.7));
scene.add(dust, inks::fine(SEPIA, 0.7));
scene.add(new_polyline(hills).erase_inside(door_box), inks::contour(INK, 1.6));
// Heather: little tufts struck up, thick near, thin far.
let tufts = new_path();
for k in 0..240 {
	let f = chance::of(72, k, 1);
	let at = vec2(30 + 1140 * chance::of(72, k, 2), HORIZON + 20 + 440 * f * f);
	let h = 6.0 + 22.0 * f;
	if path_shape.bounds().min.y < at.y && (at.x - C).abs() < 160.0 + 120.0 * f { continue; }
	for j in [-1.0, 0.0, 1.0] {
		tufts.append(new_line(at, at + vec2(j * h * 0.35 + 2.0 * chance::either_way(72, k, 3), -h)));
	}
}
scene.add(tufts.erase_inside(notes).erase_inside(door_box), inks::detail(INK, 1.0));
// Stones along the path.
let stones = new_path();
for k in 0..16 {
	let f = k / 16.0;
	let side = if k % 2 == 0 { -1.0 } else { 1.0 };
	let at = vec2(C + side * (70 + 150 * f) + 20 * chance::either_way(73, k, 1) - 20.0 * f, FOOT + 30 + 380 * f * f);
	stones.append(new_ellipse(at, 6 + 14 * f, 3 + 7 * f));
}
scene.add(stones, inks::contour(INK, 1.3));
// The light's rays on the ground.
let rays = shapes::join(shapes::times(9, |j| {
	let f = j / 8.0;
	new_line(vec2(470 + 260 * f, FOOT + 8), vec2(330 + 540 * f, 1420))
}));
scene.add(rays.erase_inside(notes), inks::fine(SEPIA, 0.8));
leaf.figures.add(scene);
leaf.figures.add(sky_ink);

// The frame: grey stone, laid in blocks, a lintel across.
let frame = new_canvas();
let blocks = new_path();
let shade = new_path();
for side in [-1.0, 1.0] {
	let x0 = if side < 0.0 { DOOR[0].x - 44 } else { DOOR[1].x };
	let y = FOOT;
	let k = 0;
	while y > 640.0 {
		let h = if k % 2 == 0 { 92.0 } else { 70.0 };
		let w = if k % 2 == 0 { 44.0 } else { 58.0 };
		let x = if side < 0.0 { x0 + 44 - w } else { x0 };
		let block = new_rectangle(vec2(x + w / 2.0, y - h / 2.0), w, h);
		blocks.append(block);
		if side > 0.0 { shade.append(block.hatch(3.5, #{ angle: [0.9], roughness: 0.8 })); }
		y -= h;
		k += 1;
	}
}
let lintel = new_rectangle(vec2(C, 610), 420, 60);
blocks.append(lintel);
shade.append(shapes::shape([vec2(390, 640), vec2(810, 640), vec2(810, 626), vec2(390, 632)]).hatch(2.5, #{ angle: [0.0] }));
frame.add(blocks, inks::contour(INK, 1.6)).add(shade, inks::detail(INK, 0.8));
// Cracks and wear on the stone.
frame.add(new_polyline([vec2(560, 582), vec2(572, 600), vec2(566, 618), vec2(578, 638)]), inks::detail(INK, 1.0));
frame.add(new_polyline([vec2(420, 900), vec2(430, 912), vec2(426, 930)]), inks::detail(INK, 1.0));

// The door: black, with no knob, no keyhole, no hinge: its panels just
// there in the dark, and the light under it.
let door = shapes::closed([DOOR[0], vec2(DOOR[1].x, DOOR[0].y), DOOR[1], vec2(DOOR[0].x, DOOR[1].y)]);
let bare = new_path();
for col in [0, 1] {
	for row in [0, 1, 2] {
		let x0 = 478.0 + col * 140.0;
		let y0 = 668.0 + row * 168.0;
		let w = 104.0;
		let h = 136.0;
		for edge in [[x0, y0, w, 2.4], [x0, y0 + h, w, 2.4]] { bare.append(new_rectangle(vec2(edge[0] + edge[2] / 2.0, edge[1]), edge[2], edge[3])); }
		bare.append(new_rectangle(vec2(x0, y0 + h / 2.0), 2.4, h));
		bare.append(new_rectangle(vec2(x0 + w, y0 + h / 2.0), 2.4, h));
	}
}
bare.append(new_rectangle(vec2(C, FOOT - 6), DOOR[1].x - DOOR[0].x - 4, 12));
engraving::mass(frame, door, bare, INK, 2.0);
frame.add(new_line(DOOR[0] + vec2(0, 0), vec2(DOOR[0].x, FOOT)), inks::contour(INK, 2.0));
leaf.figures.add(frame);

// The sleeper, from behind, on the path, in her nightgown, her hair down.
let her = new_canvas();
let F = vec2(604, 1540);
let H = 330.0;
let at = |x, y| F + vec2(x, y) * H;
let gown = shapes::closed(shapes::curve([at.call(-0.09, -0.78), at.call(-0.12, -0.6), at.call(-0.13, -0.3), at.call(-0.18, -0.02), at.call(-0.1, 0.0), at.call(-0.04, -0.01), at.call(0.03, 0.01), at.call(0.11, -0.01), at.call(0.17, -0.02), at.call(0.12, -0.32), at.call(0.11, -0.6), at.call(0.09, -0.78)], 6));
her.add(gown, inks::contour(INK, 1.8));
her.add(engraving::shade(gown, 3.0, #{ angle: [1.3], roughness: 0.6, lit: vec2(-22, 0) }), inks::detail(INK, 0.8));
let folds = shapes::join([-0.07, -0.01, 0.06, 0.12].map(|x| new_polyline(shapes::curve([at.call(x * 0.6, -0.5), at.call(x * 0.85, -0.25), at.call(x, -0.02)], 6))));
her.add(folds, inks::detail(INK, 0.9));
// Her arms, hanging, and her hair down her back.
for side in [-1.0, 1.0] {
	her.add(shapes::shape(shapes::tube([at.call(side * 0.1, -0.76), at.call(side * 0.15, -0.6), at.call(side * 0.155, -0.44), at.call(side * 0.15, -0.4)], [12.0, 10.0, 8.0, 6.0])), inks::contour(INK, 1.5));
}
let hair = shapes::closed(shapes::curve([at.call(-0.06, -0.9), at.call(-0.07, -0.84), at.call(-0.1, -0.7), at.call(-0.08, -0.56), at.call(-0.03, -0.5), at.call(0.02, -0.52), at.call(0.07, -0.54), at.call(0.09, -0.7), at.call(0.07, -0.84), at.call(0.05, -0.92), at.call(0.0, -0.95)], 6));
engraving::dark(her, hair, INK, 1.6);
leaves::place(leaf, her, 6);

leaf.figures.add(said.canvas).add(warned.canvas);
journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the watchers ----

let leaf = nb::leaf("watchers", 73, #{ foxing: 130 });

// Two sightings of the three, the same month a year apart: the month
// they'll stand in one line over the door is the secret (chains/door_heath).
let seen = chain("door_heath").run().get("watch.sightings");
let plates_of = |k, editor| illustration("star_sightings", #{ size: 390.0, seed: 4, splatter: 0, editor: editor, ink: INK, blood: COLD }).run(#{ sightings: seen })[k];

// A hand-glass, the plate in its glass, its handle turned.
let hand_glass = |centre, plate, way| {
	let g = gothic::looking_glass(centre, 292.0, 292.0, INK, #{ crest: false, drop: false, width: 44.0 });
	let c = new_canvas();
	let dir = shapes::polar(way);
	let across = vec2(-dir.y, dir.x);
	let start = centre + dir * 332;
	let pts = [];
	let widths = [];
	for p in [[-6, 60], [14, 30], [30, 44], [52, 26], [100, 22], [150, 28], [172, 40], [190, 24], [212, 46], [236, 20]] {
		pts.push(start + dir * p[0]);
		widths.push(p[1] * 1.0);
	}
	let handle = shapes::shape(shapes::tube(pts, widths));
	c.add(handle, inks::contour(INK, 2.0));
	c.add(handle.hatch(3.0, #{ angle: [way + 1.2] }).erase_outside(shapes::shape([start - across * 2, start + dir * 260 - across * 2, start + dir * 260 + across * 30, start + across * 30])), inks::detail(INK, 0.8));
	for p in [14, 30, 52, 172, 190, 212] {
		let at = start + dir * p;
		c.add(new_line(at - across * 16, at + across * 16), inks::detail(INK, 1.1));
	}
	c.add(g.canvas);
	c.add(plate.translate_by(centre - vec2(195, 195)));
	gothic::glints(c, g.glass, centre, 292.0, SEPIA);
	c
};
let A = vec2(420, 470);
let B = vec2(780, 1090);
leaves::place(leaf, hand_glass.call(A, plates_of.call(0, "first"), 1.96), 10);
leaves::place(leaf, hand_glass.call(B, plates_of.call(1, "last"), -1.08), 10);

// How each one creeps: its houses in a ring, and the step it takes in a
// year (thirteen months round an orbit of five is three houses on).
for (period, k) in seen.value().periods {
	let at = vec2(100 + k * 125, 1190);
	let step = 13 % period;
	leaf.figures.add(new_circle(at, 42), inks::contour(INK, 1.4));
	leaf.figures.add(plates::ticks(at, period, 42, 51, 0, 0.0), inks::contour(INK, 1.5));
	leaf.figures.add(new_polygon(at, 40, period, #{ step: max(step, 1) }), inks::detail(COLD, 1.3));
	leaf.figures.add(write_line(`${step} of ${period}`, at + vec2(-28, 76), leaves::note_hand(23.0, COLD)));
	leaves::clear(leaf, new_circle(at + vec2(0, 18), 70), 4);
}

leaves::jot(leaf, "I could not look up at them. I watched them in the hand-glass, the same month, a year apart.", vec2(820, 70), 330, #{ size: 27, turn: 0.01 });
leaves::jot(leaf, "One line, all three, over the door: that night, its year and its month, is the key to the knocking.", vec2(64, 1450), 700, #{ size: 27, ink: COLD, turn: -0.005 });

journal_entry.add(leaves::bind(leaf, content("nights"), #{ size: 20.0 }));
