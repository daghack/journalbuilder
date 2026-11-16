// The nave: the church inside, where she walks. On the left, the arcade
// of the nave as he drew it from the font (columnar_transposition): seven
// pillars, each with what she cut down it, stood in the order the cipher
// reads them; set them under the word on the vestry wall, the least
// letter first, and read across. On the right, the rose window, which
// shows a maze and not the lamb (circular_maze_lock, circular_maze): a
// sentence along the one way in to its heart.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/psalter" as ps;

const INK = ps::ink();
const BLOOD = ps::blood();
const SEPIA = ps::sepia();

seed(1401);

// ---- the secret the pillars hold (chains/psalter_nave) ----

let puzzle = chain("psalter_nave");
let secrets = puzzle.run();
let solved = puzzle.unlock(secrets);
let word = solved.get("nave.key");
let text = solved.get("nave.plain_text");
let cut = secrets.get("nave.cipher_text").value();
// The cipher text is the columns one after another, in their key letters'
// order: a pillar to each.
let key = word.value();
let order = shapes::times(key.len(), |c| c);
for a in 1..order.len() {
	let c = order[a];
	let b = a;
	while b > 0 && key[order[b - 1]] > key[c] { order[b] = order[b - 1]; b -= 1; }
	order[b] = c;
}
let groups = [];
let from = 0;
for c in order {
	let n = (text.value().len() - c + key.len() - 1) / key.len();
	groups.push(cut.extract(from, n));
	from += n;
}

// ---- the left page: the arcade ----

let leaf = ps::leaf("nave", 41, #{ foxing: 120 });
const PIERS = 7;
let px = |k| 120.0 + k * 160.0;
const SPRING = 700.0;
const FLOOR = 1250.0;

let stone = new_canvas();
let dark = new_path();

// The clerestory: a lancet over each bay, its glass leaded in diamonds.
for k in 0..PIERS - 1 {
	let x = px.call(k) + 80.0;
	let foot = vec2(x, 360);
	let outer = shapes::shape(shapes::lancet_points(foot, 70, 150, 1.0));
	let inner = shapes::shape(shapes::lancet_points(foot - vec2(0, 6), 56, 144, 1.0));
	stone.add(outer, inks::contour(INK, 1.6));
	stone.add(inner, inks::detail(INK, 1.0));
	let lattice = new_path();
	for j in -8..9 {
		lattice.append(new_line(vec2(x + j * 18 - 120, 360), vec2(x + j * 18 + 120, 120)));
		lattice.append(new_line(vec2(x + j * 18 + 120, 360), vec2(x + j * 18 - 120, 120)));
	}
	stone.add(lattice.erase_outside(inner), inks::fine(INK, 0.8));
	stone.add(new_rectangle(vec2(x, 368), 84, 12), inks::detail(INK, 1.0));
}
// String courses, the triforium's blind arches between them.
for y in [384.0, 394.0, 520.0, 530.0] {
	stone.add(new_line(vec2(40, y), vec2(1160, y)), if y == 384.0 || y == 530.0 { inks::contour(INK, 1.6) } else { inks::detail(INK, 0.9) });
}
for k in 0..PIERS - 1 {
	for j in 0..3 {
		let left = vec2(px.call(k) + 14 + j * 44, 470);
		let right = left + vec2(44, 0);
		let arch = shapes::shape([vec2(left.x, 514)] + shapes::arch_points(left, right, 1.0, 10) + [vec2(right.x, 514)]);
		stone.add(arch, inks::detail(INK, 1.1));
		stone.add(arch.hatch(4.0, #{ angle: [0.9], roughness: 1.0 }), inks::fine(SEPIA, 0.7));
		stone.add(shapes::shape(shapes::foil_points((left + right) / 2.0 - vec2(0, 18), 9, 3, 0.4, 0.0)), inks::fine(INK, 0.9));
	}
}

// The arcade's arches, three orders of moulding each, the bay under them
// dark.
for k in 0..PIERS - 1 {
	let left = vec2(px.call(k), SPRING);
	let right = vec2(px.call(k + 1), SPRING);
	stone.add(new_polyline(shapes::arch_order(left, right, 1.0, -12.0, 20)), inks::contour(INK, 1.8));
	stone.add(new_polyline(shapes::arch_order(left, right, 1.0, 0.0, 20)), inks::detail(INK, 1.1));
	stone.add(new_polyline(shapes::arch_order(left, right, 1.0, 10.0, 20)), inks::detail(INK, 1.1));
	let hollow = shapes::arch_order(left, right, 1.0, 24.0, 20);
	dark.append(shapes::closed([vec2(hollow[0].x, FLOOR)] + hollow + [vec2(hollow[hollow.len() - 1].x, FLOOR)]));
}
// Roundels in the spandrels.
for k in 1..PIERS - 1 {
	let at = vec2(px.call(k), 590);
	stone.add(new_circle(at, 30), inks::contour(INK, 1.4));
	stone.add(shapes::shape(shapes::foil_points(at, 24, 4, 0.38, PI() / 4.0)), inks::detail(INK, 1.0));
	stone.add(new_circle(at, 4), inks::detail(INK, 1.0));
}

// The pillars: clustered shafts, a capital of leaves, a moulded base, the
// shade down their right sides, and what she cut down each.
let letters = symbol_set("letters", #{ color: BLOOD, size: 50.0 });
for k in 0..PIERS {
	let x = px.call(k);
	// Capital: abacus, a bell of leaves curling at the corners, necking.
	stone.add(new_rectangle(vec2(x, SPRING + 8), 84, 16), inks::contour(INK, 1.6));
	let bell = shapes::shape([vec2(x - 36, SPRING + 16), vec2(x + 36, SPRING + 16), vec2(x + 26, SPRING + 52), vec2(x - 26, SPRING + 52)]);
	stone.add(bell, inks::detail(INK, 1.2));
	for side in [-1.0, 1.0] {
		let curl = vec2(x + side * 30, SPRING + 26);
		stone.add(new_arc(curl, 7, 0.0, 5.5), inks::detail(INK, 1.1));
		stone.add(new_polyline(shapes::curve([vec2(x + side * 6, SPRING + 50), vec2(x + side * 18, SPRING + 38), curl + vec2(-side * 4, 6)], 6)), inks::detail(INK, 1.0));
	}
	stone.add(new_polyline(shapes::curve([vec2(x, SPRING + 50), vec2(x - 3, SPRING + 34), vec2(x, SPRING + 22)], 6)), inks::fine(INK, 1.0));
	stone.add(new_rectangle(vec2(x, SPRING + 56), 56, 8), inks::detail(INK, 1.1));
	// Shafts: the main one and a colonnette either side.
	let top = SPRING + 60;
	let bottom = FLOOR - 44;
	for dx in [-22.0, 22.0] { stone.add(new_line(vec2(x + dx, top), vec2(x + dx, bottom)), inks::contour(INK, 1.5)); }
	for dx in [-30.0, -26.0, 26.0, 30.0] { stone.add(new_line(vec2(x + dx, top), vec2(x + dx, bottom)), inks::detail(INK, 0.9)); }
	stone.add(new_rectangle(vec2(x + 14, (top + bottom) / 2.0), 14, bottom - top).hatch(3.4, #{ angle: [PI() / 2.0 + 0.05], roughness: 1.2 }), inks::fine(INK, 0.7));
	stone.add(new_rectangle(vec2(x + 28, (top + bottom) / 2.0), 4, bottom - top).hatch(2.0, #{ angle: [PI() / 2.0] }), inks::fine(INK, 0.6));
	// Base: a roll, a plinth.
	stone.add(new_ellipse(vec2(x, bottom + 8), 38, 9), inks::detail(INK, 1.2));
	stone.add(new_rectangle(vec2(x, FLOOR - 16), 90, 30), inks::contour(INK, 1.5));
	stone.add(new_rectangle(vec2(x + 26, FLOOR - 16), 38, 30).hatch(3.0, #{ angle: [0.8] }), inks::fine(INK, 0.6));
	// What she cut: the pillar's letters, one under another.
	let group = groups[k];
	for (s, j) in group {
		stone.add(write_at(letters.text([s]), vec2(x - 2, top + 70 + j * 92), #{}));
	}
}

// The floor: a pavement of stones in two colours running away to the east
// end, under the dark of the aisle.
let vp = vec2(600, 640);
let rows = [FLOOR, 1272.0, 1300.0, 1336.0, 1382.0, 1440.0];
let at_y = |x, y| vp + (vec2(x, FLOOR) - vp) * ((y - vp.y) / (FLOOR - vp.y));
let floor = shapes::join(rows.map(|y| new_line(vec2(40, y), vec2(1160, y))));
let tiles = new_path();
for j in -14..16 {
	let x = 600.0 + j * 56.0;
	floor.append(new_line(at_y.call(x, FLOOR), at_y.call(x, 1440.0)));
	for r in 0..rows.len() - 1 {
		if (j + r) % 2 == 0 { continue; }
		tiles.append(shapes::closed([at_y.call(x, rows[r]), at_y.call(x + 56.0, rows[r]), at_y.call(x + 56.0, rows[r + 1]), at_y.call(x, rows[r + 1])]));
	}
}
let ground = new_rectangle(vec2(600, 1345), 1120, 190);
stone.add(floor.erase_outside(ground), inks::detail(INK, 1.0));
engraving::mass(stone, tiles.erase_outside(ground), new_path(), INK, 2.4);

// The dark of the aisles behind the pillars, and her in it, pale, walking
// away from him toward the east end: a hole in the dark.
let her = new_path();
let at = vec2(600, 0);
let walk = vec2(683, 1010);
her.append(shapes::closed(shapes::loop_curve([walk + vec2(0, -150), walk + vec2(13, -138), walk + vec2(12, -120), walk + vec2(26, -110), walk + vec2(30, -60), walk + vec2(40, 60), walk + vec2(52, 196), walk + vec2(20, 210), walk + vec2(-14, 204), walk + vec2(-46, 196), walk + vec2(-34, 60), walk + vec2(-28, -60), walk + vec2(-24, -110), walk + vec2(-12, -120), walk + vec2(-13, -138)], 6)));
for j in 0..9 {
	let t = (j - 4.0) / 4.0;
	her.append(shapes::sliver([walk + vec2(t * 8, -140), walk + vec2(t * 22 + 6.0 * (j * 1.3).sin(), -190), walk + vec2(t * 46, -232 - 14.0 * (j * 2.1).cos())], 3.2));
}
// The pillars stand in front of the dark.
let fronts = shapes::join(shapes::times(PIERS, |k| new_rectangle(vec2(px.call(k), (SPRING + FLOOR) / 2.0 + 30), 66, FLOOR - SPRING - 20)));
let lines = dark.hatch(2.2, #{ angle: [0.6, 0.6 + PI() / 2.0], roughness: 0.9, fill_rule: "nonzero" });
lines.erase_inside(fronts, #{ fill_rule: "nonzero" });
lines.erase_inside(her, #{ fill_rule: "nonzero" });
let night = new_canvas();
night.add(lines, new_style(new_brush_style(INK, "Rough Ink", 0.9, #{ min_pressure: 0.8, max_pressure: 0.95, pressure_profile: "constant", color_jitter: 0.04 }), new_handwriting_style()));
// Her folds, faint.
for j in 0..4 {
	let t = (j - 1.5) / 1.5;
	night.add(new_polyline(shapes::curve([walk + vec2(t * 10, -50), walk + vec2(t * 18, 60), walk + vec2(t * 30, 190)], 6)), inks::fine(SEPIA, 0.8));
}
leaf.figures.add(night);
leaf.figures.add(stone);

leaves::jot(leaf, "The nave at vespers, from the font, as I saw her pass. Seven pillars, and on each what she cut into it with her nail.", vec2(90, 58), 990, #{ size: 31, turn: -0.006 });
leaves::jot(leaf, "Stand them under her word, the least letter first, and read across.", vec2(80, 1448), 1060, #{ size: 29, ink: BLOOD, turn: 0.006 });

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the rose window ----

let leaf = ps::leaf("rose", 43, #{ foxing: 150 });
let O = vec2(600, 700);

// The sentence along the way in, and the glass it's set in.
let maze = secrets.get("rose.maze").value();
let glass = illustration("circular_maze", #{ size: 800.0, glyph_size: 42.0, editor: "rose" }).run(#{
	maze: maze,
	wall_style: #{ style: inks::contour(INK, 2.8) },
	heart_style: #{ style: inks::contour(INK, 1.8) },
	symbol_set: symbol_set("letters", #{ color: BLOOD, size: 40.0 }),
})[0];

let rose = new_canvas();
// What he set it out by.
leaf.under.add(new_line(O - vec2(560, 0), O + vec2(560, 0)), inks::pencil(SEPIA, 0.9));
leaf.under.add(new_line(O - vec2(0, 560), O + vec2(0, 560)), inks::pencil(SEPIA, 0.9));
for k in 0..8 {
	let a = PI() * k / 8.0;
	leaf.under.add(new_line(O + shapes::polar(a) * 396, O - shapes::polar(a) * 396), inks::pencil(SEPIA, 0.6));
}
// The glass, a thin wash of the light coming through it.
rose.add(new_circle(O, 150), inks::wash("rgba(130, 30, 20, 0.07)", 50.0));
rose.add(new_circle(O, 300), inks::wash("rgba(70, 40, 30, 0.06)", 44.0));
rose.add(glass.translate_by(O - vec2(400, 400)));

// The tracery round it: a ring of sixteen quatrefoils, each in a roundel
// punched out of the dark stone.
let ring = new_path();
ring.append(shapes::closed(shapes::arc_points(O, 464, 0.0, 2.0 * PI(), 120)));
let holes = new_path();
holes.append(shapes::closed(shapes::arc_points(O, 398, 0.0, 2.0 * PI(), 120)));
for k in 0..16 {
	let a = -PI() / 2.0 + 2.0 * PI() * k / 16.0;
	let at = O + shapes::polar(a) * 431;
	holes.append(new_circle(at, 30));
	rose.add(new_circle(at, 30), inks::contour(INK, 1.4));
	rose.add(shapes::shape(shapes::foil_points(at, 25, 4, 0.36, a)), inks::detail(INK, 1.1));
	rose.add(new_circle(at, 4.5), inks::fill(INK, 2.0));
	rose.add(new_line(O + shapes::polar(a) * 398, at - shapes::polar(a) * 30), inks::detail(INK, 1.4));
}
engraving::mass(rose, ring, holes, INK, 2.2);
rose.add(new_circle(O, 398), inks::contour(INK, 2.0));
rose.add(new_circle(O, 464), inks::contour(INK, 1.8));
// The outer mouldings, a band of dog-tooth between.
rose.add(new_circle(O, 490), inks::detail(INK, 1.2));
rose.add(new_circle(O, 502), inks::contour(INK, 2.2));
let teeth = shapes::steps(120, |k| {
	let a = 2.0 * PI() * k / 120.0;
	O + shapes::polar(a) * (if k % 2 == 0 { 468.0 } else { 488.0 })
});
rose.add(new_polyline(teeth), inks::detail(INK, 1.2));
for k in 0..60 {
	let a = 2.0 * PI() * (k + 0.5) / 60.0;
	let tri = shapes::shape([O + shapes::polar(a) * 470, O + vec2((a - 0.04).cos(), (a - 0.04).sin()) * 486, O + vec2((a + 0.04).cos(), (a + 0.04).sin()) * 486]);
	rose.add(inks::solid(tri, 1.6), inks::fill(INK, 1.6));
}
leaves::place(leaf, rose, 8);

// Under it, the five lancets that light the east end, dark.
let lancets = new_canvas();
let panes = new_path();
for k in 0..5 {
	let foot = vec2(360 + k * 120, 1420);
	let outer = shapes::shape(shapes::lancet_points(foot, 84, 120, 1.0));
	lancets.add(outer, inks::contour(INK, 1.6));
	panes.append(shapes::closed(shapes::lancet_points(foot - vec2(0, 8), 64, 112, 1.0)));
	lancets.add(new_rectangle(foot + vec2(0, 8), 100, 14), inks::detail(INK, 1.1));
}
engraving::velvet(lancets, panes, INK, 2.4);
leaves::place(leaf, lancets, 8);

leaves::jot(leaf, "The rose of the east end. When I was a boy it showed the lamb. Now this.", vec2(96, 66), 980, #{ size: 31, turn: 0.004 });
leaves::jot(leaf, "From the way in to the eye. The rest is there to lose you.", vec2(110, 1452), 1000, #{ size: 30, ink: BLOOD, turn: -0.006 });

journal_entry.add(leaves::bind(leaf, content("psalter"), #{ size: 22.0 }));
