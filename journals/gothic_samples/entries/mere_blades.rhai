// The blades. On the left, the rail in the passage where the blades hang:
// steel for the rest, black glass for what comes up out of the water. Cut
// along the rail in stabs and slashes, a word (morse_code); and pinned to
// the boards by a knife through it, the leather strip that was wound on
// his grandfather's grip, written while it was wound (scytale, a grip of
// four faces). On the right, the guard of that sword, pierced, and the
// plate it was made to lie on (turning_grille): laid on it notch up, its
// holes show nine letters; turned a quarter, nine more, and twice again.

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

seed(5101);

// ---- the secrets (chains/mere_blades) ----

let secrets = chain("mere_blades").run();
let cut = secrets.get("rail.marks").value();
let strip = secrets.text("strip.cipher_text");
let plate_text = secrets.text("guard.cipher_text");
let holes = secrets.get("guard.grille").value().holes;

// Filled in red: a cut, a stab.
let red = |c, shapes| { c.add(inks::solid(shapes, 1.1), inks::fill(BLOOD, 1.1)); c.add(shapes, inks::contour(BLOOD, 1.0)); };

// ---- the left page: the rail ----

let leaf = mb::leaf("blades", 51, #{ foxing: 100, stains: 1 });
let RAIL = 252.0;
let SX = 300.0;
let SWORD_X = 820.0;

// What hangs, worked out first: the wall is drawn behind them.
let sword = gothic::sword(vec2(SWORD_X, 600), vec2(SWORD_X, 1330), 46.0, INK, #{ guard: 170.0, grip: 150.0 });
let knives = [gothic::glass_knife(vec2(500, 420), vec2(500, 760), 58.0, 120.0, 3, INK), gothic::glass_knife(vec2(610, 470), vec2(624, 840), 64.0, 130.0, 8, INK)];
let pin = gothic::glass_knife(vec2(SX + 96, 330), vec2(SX - 4, 404), 40.0, 90.0, 5, INK);
// The strip: a length of leather, notched at its end, a little twisted.
let STOP = 1250.0;
let left_edge = [];
let right_edge = [];
for j in 0..=20 {
	let y = 380.0 + (STOP - 380.0) * j / 20.0;
	let x = SX + 8.0 * (y * 0.008).sin();
	let half = 24.0 - 3.0 * (y * 0.011 + 1.0).sin();
	left_edge.push(vec2(x - half, y));
	right_edge.push(vec2(x + half, y));
}
let back = shapes::reversed(right_edge);
let leather = shapes::closed(left_edge + [vec2(SX, STOP - 22)] + back);
let hanging = new_path().append(sword.shape).append(leather).append(pin.shape);
for k in knives { hanging.append(k.shape); }

// The wall: boards, their grain, knots, the dark coming down from above.
let wall = new_canvas();
let boards = new_path();
let grain = new_path();
for k in 0..11 {
	let x = 40.0 + k * 112.0;
	boards.append(new_line(vec2(x, RAIL + 40), vec2(x + 3, 1380)));
	for g in 0..5 {
		let gx = x + 18.0 + g * 19.0 + 6.0 * chance::either_way(52, k * 10 + g, 1);
		let pts = [];
		for j in 0..=14 { let y = RAIL + 40 + (1380 - RAIL - 40) * j / 14.0; pts.push(vec2(gx + 3.0 * (y * 0.02 + g + k).sin(), y)); }
		grain.append(new_polyline(pts));
	}
}
// Moonlight from the left: each blade's shadow on the boards.
let thrown = shapes::join([sword.shape, leather, knives[0].shape, knives[1].shape].map(|p| p.copy().translate_by(vec2(30, 16))));
let shade = thrown.hatch(3.2, #{ angle: [2.2], roughness: 0.8, fill_rule: "nonzero" });
let wall_area = shapes::closed([vec2(20, RAIL + 30), vec2(1180, RAIL + 30), vec2(1180, 1390), vec2(20, 1390)]);
let behind = |p| p.erase_inside(hanging, #{ fill_rule: "nonzero" }).erase_outside(wall_area);
wall.add(behind.call(boards), inks::detail(INK, 1.2));
wall.add(behind.call(grain), inks::fine(SEPIA, 0.8));
wall.add(behind.call(shade), inks::detail(INK, 0.8));
// The dark coming up from the floor and in from the far side, in layers.
for layer in [[1000.0, 900.0, 5.6, 0.3], [1130.0, 980.0, 4.4, 2.0], [1250.0, 1060.0, 3.4, 1.2]] {
	let gloom = new_path();
	gloom.append(shapes::closed([vec2(20, layer[0]), vec2(1180, layer[0] - 120), vec2(1180, 1390), vec2(20, 1390)]));
	gloom.append(shapes::closed([vec2(layer[1] + 80, RAIL + 30), vec2(1180, RAIL + 30), vec2(1180, layer[0]), vec2(layer[1] + 160, layer[0])]));
	wall.add(behind.call(gloom.hatch(layer[2], #{ angle: [layer[3]], roughness: 1.0, fill_rule: "nonzero" })), inks::detail(INK, 0.8));
}
leaf.figures.add(wall);

// The rail, with its pegs, and the word cut along it.
let rail = new_canvas();
let beam = new_rectangle(vec2(600, RAIL), 1060, 64);
rail.add(beam, inks::contour(INK, 2.2));
let beam_grain = new_path();
for g in 0..6 {
	let pts = [];
	for j in 0..=30 { let x = 70.0 + 1060.0 * j / 30.0; pts.push(vec2(x, RAIL - 26 + g * 10 + 2.0 * (x * 0.013 + g * 2.0).sin())); }
	beam_grain.append(new_polyline(pts));
}
rail.add(beam_grain, inks::fine(SEPIA, 0.8));
rail.add(new_rectangle(vec2(600, RAIL + 30), 1060, 8).hatch(2.0, #{ angle: [0.0] }), inks::detail(INK, 0.9));
let pegs = shapes::join([SWORD_X, 500.0, 616.0].map(|x| shapes::closed(shapes::ellipse_points(vec2(x, RAIL + 44), 9, 9, 12))));
engraving::mass(rail, pegs, new_path(), INK, 1.2);
// The marks: a stab is a puncture, a slash a long cut, letters apart.
let widths = cut.map(|m| if m == 1 { 14.0 } else if m == 2 { 42.0 } else { 26.0 });
let total = 0.0;
for w in widths { total += w + 9.0; }
let x = 600.0 - total / 2.0;
let stabs = new_path();
for (m, k) in cut {
	let w = widths[k];
	let mid = vec2(x + w / 2.0, RAIL - 2 + 3.0 * chance::either_way(53, k, 1));
	if m == 1 {
		stabs.append(shapes::closed([mid + vec2(0, -9), mid + vec2(4, 0), mid + vec2(0, 9), mid + vec2(-4, 0)]));
	} else if m == 2 {
		stabs.append(shapes::sliver([mid + vec2(-w / 2.0, 5), mid + vec2(0, 0.5), mid + vec2(w / 2.0, -5)], 6.0));
	}
	x += w + 9.0;
}
red.call(rail, stabs);
leaf.figures.add(rail);

// The cords the blades hang by.
let cords = new_path();
cords.append(new_line(vec2(SWORD_X, RAIL + 50), sword.back + vec2(-12, 16))).append(new_line(vec2(SWORD_X, RAIL + 50), sword.back + vec2(12, 16)));
cords.append(new_line(vec2(500, RAIL + 50), vec2(496, 304))).append(new_line(vec2(616, RAIL + 50), vec2(604, 344)));
leaf.figures.add(new_canvas().add(cords, inks::contour(INK, 1.2)));
leaf.figures.add(sword.canvas);
// The grip squared off: four faces, the groove the strip ran in.
let faces = shapes::join([-1.0, 1.0].map(|s| new_line(vec2(SWORD_X + s * 8, 600 - 6), vec2(SWORD_X + s * 8, 600 - 144))));
leaf.figures.add(new_canvas().add(faces, inks::detail(INK, 1.0)));
for k in knives { leaf.figures.add(k.canvas); }

// The strip, its letters, and the knife through it.
let hide = new_canvas();
hide.add(leather, inks::contour(INK, 1.6));
hide.add(leather.hatch(4.0, #{ angle: [1.4], roughness: 1.0 }).erase_inside(pin.shape, #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.9));
let letters = new_canvas();
for (ch, k) in strip.chars() {
	let g = write_line(ch.to_string(), vec2(0, 0), leaves::figures_hand(30.0, BLOOD));
	let b = g.bounds();
	let at = vec2(SX + 8.0 * ((440.0 + k * 62.0) * 0.008).sin(), 440.0 + k * 62.0);
	letters.add(g.translate_by(at - b.center));
}
leaf.figures.add(hide).add(letters).add(pin.canvas);

leaves::jot(leaf, "Black glass for what comes up out of the water. Steel for the rest.", vec2(70, 64), 1060, #{ size: 28, turn: 0.004 });
leaves::jot(leaf, "The strip off his grip, written while it was wound: wind it on four faces and read along. The rail is cut in stabs and slashes.", vec2(70, 1420), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the guard and its plate ----

let leaf = mb::leaf("guard", 53, #{ foxing: 120, stains: 1 });
let G = vec2(600, 540);
let GR = 300.0;
let CELL = 60.0;
let cell_at = |n, centre, cell| centre + vec2((n % 6 - 2.5) * cell, (n / 6 - 2.5) * cell);
let guard = new_canvas();
let openings = shapes::join(holes.map(|n| {
	let at = cell_at.call(n, G, CELL);
	shapes::closed([at + vec2(-21, -21), at + vec2(21, -21), at + vec2(21, 21), at + vec2(-21, 21)])
}));
let disc = shapes::closed(shapes::ellipse_points(G, GR, GR, 120));
// The blade's slot, above the holes, where the tang went through.
let slot = shapes::closed([G + vec2(0, -268), G + vec2(22, -224), G + vec2(0, -200), G + vec2(-22, -224)]);
let through = new_path().append(openings).append(slot);
// The steel shaded across, the light off its upper left.
guard.add(disc.hatch(3.6, #{ angle: [0.0], roughness: 0.4 }).erase_inside(through, #{ fill_rule: "nonzero" }), inks::detail(INK, 0.8));
// The ring inside the rim, darker.
let band = shapes::closed(shapes::ellipse_points(G, GR - 32, GR - 32, 120));
guard.add(disc.hatch(2.4, #{ angle: [0.8], roughness: 0.4 }).erase_inside(band, #{ fill_rule: "nonzero" }), inks::detail(INK, 0.9));
// Ripples engraved on its face, round the holes.
let ripples = shapes::join([70.0, 120.0, 170.0, 220.0, 250.0].map(|r| new_circle(G, r)));
guard.add(ripples.erase_inside(through, #{ fill_rule: "nonzero" }).erase_inside(new_rectangle(G, 6.0 * CELL + 16, 6.0 * CELL + 16)), inks::fine(SEPIA, 0.9));
// The rim: a band of waves breaking.
let rim = shapes::join(shapes::times(48, |k| {
	let a = 2.0 * PI() * k / 48.0;
	let way = shapes::polar(a);
	let along = vec2(-way.y, way.x);
	let at = G + way * (GR - 16);
	new_polyline(shapes::curve([at - along * 16 - way * 8, at - along * 4 + way * 6, at + along * 8 + way * 2, at + along * 4 - way * 4], 4))
}));
guard.add(rim, inks::detail(INK, 1.1));
guard.add(disc, inks::contour(INK, 2.6));
guard.add(new_circle(G, GR - 32), inks::contour(INK, 1.4));
// Each hole's inner walls, dark on two sides.
let walls = new_path();
for n in holes {
	let at = cell_at.call(n, G, CELL);
	walls.append(shapes::closed([at + vec2(-21, -21), at + vec2(21, -21), at + vec2(16, -16), at + vec2(-16, -16)]));
	walls.append(shapes::closed([at + vec2(-21, -21), at + vec2(-16, -16), at + vec2(-16, 21), at + vec2(-21, 21)]));
}
engraving::mass(guard, walls, new_path(), INK, 1.0);
guard.add(openings, inks::contour(INK, 1.3));
engraving::mass(guard, slot, new_path(), INK, 1.2);
// The grid it was cut to, faint, as the smith scored it.
let scored = new_path();
for k in 0..=6 {
	scored.append(new_line(G + vec2(-3.0 * CELL, (k - 3) * CELL), G + vec2(3.0 * CELL, (k - 3) * CELL)));
	scored.append(new_line(G + vec2((k - 3) * CELL, -3.0 * CELL), G + vec2((k - 3) * CELL, 3.0 * CELL)));
}
guard.add(scored.erase_inside(through, #{ fill_rule: "nonzero" }), inks::pencil(SEPIA, 0.9));
// The notch at its top, and the way it turns.
let notch = shapes::closed([G + vec2(-12, -GR - 22), G + vec2(12, -GR - 22), G + vec2(0, -GR + 2)]);
red.call(guard, notch);
let turning = new_polyline(shapes::arc_points(G, GR + 40, -PI() / 2.0 + 0.25, -PI() / 2.0 + 0.95, 20));
guard.add(turning, inks::contour(BLOOD, 1.8));
let head_at = G + vec2((-PI() / 2.0 + 0.95).cos(), (-PI() / 2.0 + 0.95).sin()) * (GR + 40);
let along = vec2(-(-PI() / 2.0 + 0.95).sin(), (-PI() / 2.0 + 0.95).cos());
red.call(guard, shapes::closed([head_at + along * 16, head_at + (head_at - G).normalized() * 9, head_at - (head_at - G).normalized() * 9]));
leaves::place(leaf, guard, 14);

// The plate it lies on: steel, bevelled, the letters cut in rows.
let P = vec2(600, 1132);
let PC = 60.0;
let plate = new_canvas();
let face = new_rectangle(P, 6.0 * PC + 40, 6.0 * PC + 40);
plate.add(face, inks::contour(INK, 2.4));
plate.add(new_rectangle(P, 6.0 * PC + 16, 6.0 * PC + 16), inks::contour(INK, 1.2));
let bevel = shapes::closed([P + vec2(-3.0 * PC - 20, 3.0 * PC + 20), P + vec2(3.0 * PC + 20, 3.0 * PC + 20), P + vec2(3.0 * PC + 8, 3.0 * PC + 8), P + vec2(-3.0 * PC - 8, 3.0 * PC + 8)])
	.append(shapes::closed([P + vec2(3.0 * PC + 20, -3.0 * PC - 20), P + vec2(3.0 * PC + 20, 3.0 * PC + 20), P + vec2(3.0 * PC + 8, 3.0 * PC + 8), P + vec2(3.0 * PC + 8, -3.0 * PC - 8)]));
plate.add(bevel.hatch(2.2, #{ angle: [0.8], fill_rule: "nonzero" }), inks::detail(INK, 0.9));
let rules = new_path();
for k in 1..6 {
	rules.append(new_line(P + vec2(-3.0 * PC, (k - 3) * PC), P + vec2(3.0 * PC, (k - 3) * PC)));
	rules.append(new_line(P + vec2((k - 3) * PC, -3.0 * PC), P + vec2((k - 3) * PC, 3.0 * PC)));
}
plate.add(rules, inks::fine(INK, 0.9));
for (ch, n) in plate_text.chars() {
	if ch == ' ' { continue; }
	plate.add(leaves::centred(ch.to_string(), cell_at.call(n, P, PC), leaves::figures_hand(32.0, BLOOD)));
}
leaves::place(leaf, plate, 12);

leaves::jot(leaf, "His guard. He said the holes were for luck, and the plate was for keeping it on.", vec2(70, 60), 640, #{ size: 27, turn: 0.006 });
leaves::jot(leaf, "Notch to the top, on the plate: read what shows, row by row. Then a quarter round the sun's way, and again, and again.", vec2(70, 1400), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, content("mere"), #{ size: 20.0 }));
