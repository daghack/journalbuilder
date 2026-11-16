// The glass: the mirrors of the house. On the left, the long gallery's
// six pier glasses, the sleeper before them with a candle and in none of
// them, each glass holding a column of what the glasses said, written as
// a glass writes, backwards (columnar_transposition): the columns one after
// another in their key letters' order, a glass to each. One of them shows
// the door. On the right, the round convex glass over the stair, a
// girandole crested with a watcher, and in it, backwards and bowed, the
// way in to the eye (circular_maze_lock): what it says is the key word
// the six answer to.

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

seed(2201);

// ---- the secrets (chains/door_glass) ----

let puzzle = chain("door_glass");
let secrets = puzzle.run();
let solved = puzzle.unlock(secrets);
let word = solved.get("gallery.key");
let text = solved.get("gallery.plain_text");
let cut = secrets.get("gallery.cipher_text").value();
// The cipher text is the columns one after another, in their key letters'
// order: a glass to each.
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

let maze = secrets.get("round.maze").value();

// ---- the left page: the gallery ----

let leaf = nb::leaf("gallery", 81, #{ foxing: 120, stains: 2 });
let C = 600.0;
let GX = |k| C + (k - 2.5) * 180.0;
let TOP = 340.0;
let BOTTOM = 940.0;
let GW = 104.0;
let DADO = 1000.0;
let FLOOR = 1150.0;
let letters = symbol_set("letters", #{ color: COLD, size: 48.0 });

// A pier glass's shape: upright sides and a round head, `grow` out from
// the glass itself.
let pier = |x, grow| {
	let half = GW / 2.0 + grow;
	shapes::shape([vec2(x - half, BOTTOM + grow)] + shapes::arc_points(vec2(x, TOP), half, PI(), 2.0 * PI(), 24) + [vec2(x + half, BOTTOM + grow)])
};
let frames = shapes::join(shapes::times(6, |k| pier.call(GX.call(k), 24.0)));

// The sleeper, from behind, her candle held up beside her.
let her = new_canvas();
let F = vec2(C, 1572);
let H = 560.0;
let at = |x, y| F + vec2(x, y) * H;
let gown = shapes::closed(shapes::curve([at.call(-0.09, -0.78), at.call(-0.12, -0.6), at.call(-0.13, -0.3), at.call(-0.18, -0.02), at.call(-0.1, 0.0), at.call(-0.04, -0.01), at.call(0.03, 0.01), at.call(0.11, -0.01), at.call(0.17, -0.02), at.call(0.12, -0.32), at.call(0.11, -0.6), at.call(0.09, -0.78)], 6));
let arm = shapes::shape(shapes::tube([at.call(0.09, -0.76), at.call(0.16, -0.68), at.call(0.2, -0.74), at.call(0.215, -0.82)], [15.0, 12.0, 11.0, 10.0]));
let hang = shapes::shape(shapes::tube([at.call(-0.1, -0.76), at.call(-0.15, -0.6), at.call(-0.155, -0.44), at.call(-0.15, -0.4)], [15.0, 12.0, 10.0, 8.0]));
her.add(gown, inks::contour(INK, 2.2));
her.add(engraving::shade(gown, 2.6, #{ angle: [1.3], roughness: 0.6, lit: vec2(-40, 0) }), inks::detail(INK, 1.0));
her.add(engraving::shade(gown, 3.4, #{ angle: [2.6], roughness: 0.6, lit: vec2(-70, 30) }), inks::detail(INK, 0.8));
her.add(new_polyline(shapes::curve([at.call(-0.07, -0.74), at.call(0.0, -0.7), at.call(0.075, -0.74)], 6)), inks::detail(INK, 1.1));
let folds = shapes::join([-0.08, -0.02, 0.04, 0.1].map(|x| new_polyline(shapes::curve([at.call(x * 0.6, -0.55), at.call(x * 0.85, -0.28), at.call(x, -0.02)], 6))));
her.add(folds, inks::detail(INK, 1.0));
her.add(arm, inks::contour(INK, 1.8)).add(shapes::behind(hang, gown), inks::contour(INK, 1.8));
let hair_pts = [];
for p in [[0.0, -0.95], [0.04, -0.93], [0.055, -0.87], [0.05, -0.81], [0.07, -0.76], [0.08, -0.68], [0.072, -0.6], [0.05, -0.565], [0.032, -0.6], [0.012, -0.555], [-0.01, -0.6], [-0.03, -0.56], [-0.05, -0.6], [-0.07, -0.62], [-0.08, -0.7], [-0.07, -0.77], [-0.05, -0.82], [-0.055, -0.88], [-0.04, -0.93]] { hair_pts.push(at.call(p[0], p[1])); }
let hair = shapes::closed(shapes::loop_curve(hair_pts, 4));
engraving::dark(her, hair, INK, 1.8);
let locks = shapes::join(shapes::times(9, |j| {
	let x = -0.05 + j * 0.0125;
	new_polyline(shapes::curve([at.call(x * 1.1, -0.66), at.call(x * 1.2 + 0.003 * (j % 2), -0.6), at.call(x * 1.1 + 0.006 * (j % 3), -0.53 - 0.02 * (j % 4))], 6))
}));
her.add(locks, new_style(new_brush_style(INK, "Strands", 3.0, #{ min_pressure: 0.4, max_pressure: 0.9, pressure_profile: "taper" }), new_handwriting_style()));
// The candle: a saucer, the stick, the flame, and its light.
let hand = at.call(0.215, -0.83);
her.add(new_ellipse(hand + vec2(0, -4), 26, 6), inks::contour(INK, 1.6));
let stick = shapes::closed([hand + vec2(-7, -6), hand + vec2(-7, -62), hand + vec2(7, -62), hand + vec2(7, -6)]);
her.add(stick, inks::contour(INK, 1.4));
her.add(shapes::shape(shapes::tube([hand + vec2(5, -60), hand + vec2(8, -46), hand + vec2(7, -36)], [4.0, 4.0, 3.0])), inks::detail(INK, 1.0));
let lit_flame = shapes::shape(shapes::tube([hand + vec2(0, -64), hand + vec2(1, -76), hand + vec2(-1, -90), hand + vec2(2, -104)], [7.0, 11.0, 7.0, 1.0]));
her.add(lit_flame, inks::contour(INK, 1.0));
her.add(new_line(hand + vec2(0, -62), hand + vec2(0, -70)), inks::contour(INK, 2.4));
let glow = shapes::join(shapes::times(20, |j| {
	let a = 2.0 * PI() * j / 20.0;
	let way = shapes::polar(a);
	new_line(hand + vec2(0, -82) + way * 46, hand + vec2(0, -82) + way * (70 + 30 * (j % 3)))
}));
her.add(glow.erase_inside(gown, #{ fill_rule: "nonzero" }).erase_inside(hair, #{ fill_rule: "nonzero" }), inks::pencil(SEPIA, 1.0));

let silhouette = new_path().append(gown).append(hair).append(arm);

let room = new_canvas();
// The cornice, its dentils, and the shadow under it.
for y in [140.0, 150.0, 172.0, 198.0] { room.add(new_line(vec2(30, y), vec2(1170, y)), inks::contour(INK, if y == 198.0 { 2.0 } else { 1.3 })); }
let dentils = new_path();
let x = 40.0;
while x < 1160.0 { dentils.append(new_rectangle(vec2(x + 5, 185), 10, 22)); x += 18.0; }
room.add(dentils, inks::detail(INK, 1.0));
room.add(new_rectangle(vec2(C, 206), 1140, 14).hatch(2.6, #{ angle: [0.0] }), inks::fine(INK, 0.8));
// The paper: stripes, and a sprig between.
let stripes = new_path();
let x = 50.0;
while x < 1160.0 {
	stripes.append(new_line(vec2(x, 214), vec2(x, DADO - 4))).append(new_line(vec2(x + 5, 214), vec2(x + 5, DADO - 4)));
	x += 44.0;
}
let sprigs = new_path();
for row in 0..12 {
	for col in 0..26 {
		let at = vec2(72 + col * 44, 250 + row * 64 + (col % 2) * 32);
		sprigs.append(shapes::shape(shapes::foil_points(at, 5.0, 4, 0.5, 0.0)));
	}
}
room.add(shapes::behind(stripes.erase_inside(frames), silhouette), inks::pencil(SEPIA, 0.9));
room.add(shapes::behind(sprigs.erase_inside(frames), silhouette), inks::fine(SEPIA, 0.8));
// The dado and the wainscot under it.
for y in [DADO, DADO + 10.0, DADO + 18.0] { room.add(shapes::behind(new_line(vec2(30, y), vec2(1170, y)), silhouette), inks::contour(INK, 1.3)); }
for k in 0..6 {
	let x = GX.call(k);
	room.add(shapes::behind(new_rectangle(vec2(x, 1075), 146, 82), silhouette), inks::detail(INK, 1.1));
	room.add(shapes::behind(new_rectangle(vec2(x, 1075), 132, 68), silhouette), inks::detail(INK, 0.9));
}
let skirting = new_canvas();
engraving::mass(skirting, shapes::closed([vec2(30, 1122), vec2(1170, 1122), vec2(1170, FLOOR), vec2(30, FLOOR)]), silhouette, INK, 2.0);
room.add(skirting);
// The floor, its boards running away from the candle into the dark.
let VP = vec2(C, 640);
let boards = new_path();
for j in -14..30 {
	let foot = vec2(-700 + j * 70, 1700);
	let t = (FLOOR - VP.y) / (foot.y - VP.y);
	boards.append(new_line(VP + (foot - VP) * t, foot));
}
let joints = shapes::join(shapes::times(40, |k| {
	let j = (chance::of(81, k, 1) * 42).to_int() - 13;
	let depth = 0.1 + 0.9 * chance::of(81, k, 2);
	let y = FLOOR + (1600 - FLOOR) * depth;
	let x0 = VP.x + (-700 + j * 70 - VP.x) * (y - VP.y) / (1700 - VP.y);
	let x1 = VP.x + (-700 + (j + 1) * 70 - VP.x) * (y - VP.y) / (1700 - VP.y);
	new_line(vec2(x0, y), vec2(x1, y))
}));
let floor = new_rectangle(vec2(C, (FLOOR + 1600) / 2.0), 1200, 1600 - FLOOR);
let cleared_note = leaves::note("Set them under the round glass's word, the least letter first, and read across.", vec2(64, 1250), 400, #{ size: 28, ink: COLD, turn: 0.01 });
let note_hole = cleared_note.hole;
let lit = new_ellipse(vec2(C, 1500), 380, 150);
room.add(shapes::behind(boards.erase_outside(floor).erase_inside(note_hole), silhouette), inks::detail(INK, 1.0)).add(shapes::behind(joints.erase_inside(note_hole), silhouette), inks::detail(INK, 0.9));
room.add(shapes::behind(floor.hatch(4.0, #{ angle: [0.05], roughness: 1.0 }).erase_inside(lit).erase_inside(note_hole), silhouette), inks::fine(INK, 0.8));
room.add(shapes::behind(floor.hatch(5.0, #{ angle: [2.6], roughness: 1.0 }).erase_inside(new_ellipse(vec2(C, 1480), 520, 220)).erase_inside(note_hole), silhouette), inks::fine(INK, 0.8));
leaf.figures.add(room);

// The six glasses: their frames, what they show of the gallery (only
// the gallery), and what's written in them.
for (letters_here, k) in groups {
	let x = GX.call(k);
	let g = new_canvas();
	let glass = pier.call(x, 0.0);
	// What's in the glass: the gallery going on, glass in glass, toward a
	// point low down; no one in it.
	let far = vec2(x, 860);
	let echoes = new_path();
	for j in 1..5 {
		let s = 1.0 - j * 0.19;
		echoes.append(glass.copy().translate_by(vec2(-far.x, -far.y)).scale_by(vec2(s, s)).translate_by(far));
	}
	for j in [-2.0, -1.0, 1.0, 2.0] { echoes.append(new_line(far, far + vec2(j * 30, 80))); }
	g.add(echoes.erase_outside(glass), inks::fine(SEPIA, 0.9));
	gothic::glints(g, glass, vec2(x, 640), 300.0, SEPIA);
	// The frame: carved, three mouldings, shaded on its right.
	g.add(pier.call(x, 24.0), inks::contour(INK, 2.2));
	g.add(pier.call(x, 17.0), inks::detail(INK, 1.0));
	g.add(pier.call(x, 10.0), inks::detail(INK, 1.2));
	g.add(glass, inks::contour(INK, 1.5));
	let band = pier.call(x, 24.0).hatch(3.0, #{ angle: [0.8], roughness: 0.6 }).erase_inside(pier.call(x, 0.0)).erase_inside(new_rectangle(vec2(x - 40, 600), 120, 1000));
	g.add(band, inks::detail(INK, 0.8));
	// Its cresting: two scrolls and a flame.
	let crest = vec2(x, TOP - GW / 2.0 - 24.0);
	for side in [-1.0, 1.0] {
		let pts = [];
		for j in 0..=20 {
			let t = j / 20.0;
			let a = -PI() / 2.0 + side * t * 5.2;
			let r = 18.0 * (1.0 - 0.75 * t);
			pts.push(crest + vec2(side * 30, -8) + vec2(a.cos() * r * side, a.sin() * r));
		}
		g.add(new_polyline(pts), inks::contour(INK, 1.4));
	}
	let flame = shapes::shape(shapes::tube([crest + vec2(0, 2), crest + vec2(0, -16), crest + vec2(4, -34), crest + vec2(-2, -50)], [10.0, 16.0, 10.0, 1.0]));
	g.add(flame, inks::contour(INK, 1.4));
	g.add(flame.hatch(2.6, #{ angle: [0.7] }), inks::detail(INK, 0.7));
	// What it says, as a glass would write it.
	for (s, j) in letters_here {
		g.add(gothic::reflected(new_canvas().add(write_at(letters.text([s]), vec2(x, 410 + j * 100), #{}))));
	}
	// The fourth shows the door, far down the gallery that isn't there.
	if k == 3 {
		let door = new_canvas();
		engraving::mass(door, shapes::closed([vec2(x - 22, 786), vec2(x + 22, 786), vec2(x + 22, 868), vec2(x - 22, 868)]), new_rectangle(vec2(x, 864), 40, 5), INK, 1.4);
		door.add(new_rectangle(vec2(x, 824), 58, 92), inks::detail(INK, 1.1));
		g.add(door);
	}
	leaves::place(leaf, g, 6);
}

leaves::place(leaf, her, 8);

leaves::jot(leaf, "The long gallery. Six glasses, and in not one of them was I.", vec2(130, 60), 960, #{ size: 28, turn: -0.004 });
leaf.figures.add(cleared_note.canvas);

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the round glass ----

let leaf = nb::leaf("round", 83, #{ foxing: 140 });
let O = vec2(600, 760);
let R = 330.0;
let glass = new_circle(O, R);

let girandole = new_canvas();
// The glass is convex: dark round its rim, darker to its lower right,
// and what it shows bowed: a window, and the stair going down.
girandole.add(glass.hatch(4.2, #{ angle: [2.4], roughness: 0.8 }).erase_inside(new_circle(O, R - 30)), inks::fine(INK, 0.8));
girandole.add(glass.hatch(3.6, #{ angle: [0.8], roughness: 0.8 }).erase_inside(new_circle(O + vec2(-44, -44), R)), inks::detail(INK, 0.8));
let window = shapes::join(shapes::times(3, |k| {
	let x = -230.0 + k * 54.0;
	new_polyline(shapes::curve([O + vec2(x, -210 + k * 6), O + vec2(x - 8, -150), O + vec2(x - 14, -90 + k * 4)], 6))
}));
for y in [-214.0, -150.0, -88.0] { window.append(new_polyline(shapes::curve([O + vec2(-236, y), O + vec2(-200, y - 10), O + vec2(-124, y - 6)], 6))); }
for k in 0..7 {
	let y = 150.0 + k * 22.0;
	window.append(new_polyline(shapes::curve([O + vec2(-280 + k * 12, y), O + vec2(0, y + 18 - k * 2), O + vec2(280 - k * 12, y)], 6)));
}
girandole.add(window.erase_outside(glass), inks::pencil(SEPIA, 1.0));
// The way in, as the glass shows it.
let drawn = illustration("circular_maze", #{ size: 600.0, glyph_size: 34.0, editor: "round" }).run(#{
	maze: maze,
	wall_style: #{ style: inks::contour(INK, 2.4) },
	heart_style: #{ style: inks::contour(INK, 1.6) },
	symbol_set: symbol_set("letters", #{ color: COLD, size: 32.0 }),
})[0];
girandole.add(gothic::reflected(drawn.translate_by(O - vec2(300, 300))));
gothic::glints(girandole, glass, O, R, SEPIA);

// The frame: a reed, a dark hollow set with balls, the outer mouldings.
let hollow = new_path().append(shapes::closed(shapes::arc_points(O, R + 62, 0.0, 2.0 * PI(), 140)));
let balls = new_path().append(shapes::closed(shapes::arc_points(O, R + 16, 0.0, 2.0 * PI(), 140)));
let ball_ink = new_path();
for k in 0..32 {
	let a = -PI() / 2.0 + 2.0 * PI() * k / 32.0;
	let at = O + shapes::polar(a) * (R + 39);
	balls.append(new_circle(at, 15));
	ball_ink.append(new_circle(at, 15));
	ball_ink.append(new_circle(at, 15).hatch(2.4, #{ angle: [0.8] }).erase_inside(new_circle(at + vec2(-5, -5), 14)));
}
engraving::mass(girandole, hollow, balls, INK, 2.0);
girandole.add(ball_ink, inks::detail(INK, 1.0));
for r in [R + 4.0, R + 10.0] { girandole.add(new_circle(O, r), inks::detail(INK, 1.1)); }
girandole.add(glass, inks::contour(INK, 1.8));
girandole.add(new_circle(O, R + 70), inks::detail(INK, 1.2));
girandole.add(new_circle(O, R + 80), inks::contour(INK, 2.6));
girandole.add(new_circle(O, R + 80).hatch(3.2, #{ angle: [0.8] }).erase_inside(new_circle(O, R + 70)).erase_inside(new_circle(O + vec2(-14, -14), R + 80)), inks::detail(INK, 0.8));

// The crest: a watcher in a burst of rays, on a plinth, scrolls of
// acanthus either side.
let crown = vec2(C, O.y - R - 156);
let plinth = shapes::closed([crown + vec2(-38, 74), crown + vec2(38, 74), crown + vec2(30, 76 + 22), crown + vec2(-30, 76 + 22)]);
engraving::mass(girandole, plinth, new_path(), INK, 1.8);
let burst = shapes::join(shapes::times(32, |j| {
	let a = 2.0 * PI() * j / 32.0;
	let len = if j % 2 == 0 { 112.0 } else { 80.0 };
	new_line(crown + shapes::polar(a) * 56, crown + shapes::polar(a) * len)
}));
girandole.add(burst.erase_inside(new_rectangle(crown + vec2(0, 110), 300, 70)), inks::detail(INK, 1.1));
girandole.add(gothic::watcher(crown, 42.0, 9, INK, #{ reach: 1.3 }));
for side in [-1.0, 1.0] {
	let leaf_pts = shapes::curve([crown + vec2(side * 34, 94), crown + vec2(side * 90, 96), crown + vec2(side * 150, 60), crown + vec2(side * 170, 10), crown + vec2(side * 150, -24), crown + vec2(side * 118, -14), crown + vec2(side * 120, 14), crown + vec2(side * 140, 12)], 6);
	girandole.add(new_polyline(leaf_pts), inks::contour(INK, 2.2));
	let under_pts = shapes::curve([crown + vec2(side * 40, 104), crown + vec2(side * 110, 108), crown + vec2(side * 170, 80), crown + vec2(side * 196, 30)], 6);
	girandole.add(new_polyline(under_pts), inks::contour(INK, 1.6));
	for t in [0.3, 0.5, 0.7] {
		let p = crown + vec2(side * (60 + 120 * t), 100 - 70 * t * t);
		girandole.add(new_line(p, p + vec2(side * 8, -18)), inks::detail(INK, 1.0));
	}
}
// The drop under it.
let bottom = O + vec2(0, R + 80);
let drop = shapes::closed(shapes::curve([bottom + vec2(-34, -4), bottom + vec2(-26, 30), bottom + vec2(-8, 76), bottom + vec2(0, 110), bottom + vec2(8, 76), bottom + vec2(26, 30), bottom + vec2(34, -4)], 6));
girandole.add(drop, inks::contour(INK, 1.8));
girandole.add(drop.hatch(3.2, #{ angle: [0.9] }).erase_inside(new_circle(bottom + vec2(-10, 26), 20)), inks::detail(INK, 0.8));
for y in [16.0, 40.0] { girandole.add(new_line(bottom + vec2(-28, y), bottom + vec2(28, y)), inks::detail(INK, 1.0)); }
leaves::place(leaf, girandole, 12);

// The candle arms, curling out from under it, their candles lit.
for side in [-1.0, 1.0] {
	let arm = new_canvas();
	let a = PI() / 2.0 - side * 0.95;
	let start = O + shapes::polar(a) * (R + 76);
	let cup = vec2(C + side * 470, 1000);
	let pts = [start, start + vec2(side * 40, 40), cup + vec2(-side * 60, 40), cup + vec2(-side * 10, 24), cup + vec2(0, 10)];
	let limb = shapes::shape(shapes::tube(pts, [16.0, 13.0, 11.0, 10.0, 14.0]));
	arm.add(limb, inks::contour(INK, 1.8));
	arm.add(engraving::shade(limb, 2.8, #{ angle: [0.6], lit: vec2(0, -6) }), inks::detail(INK, 0.8));
	let dish = shapes::closed([cup + vec2(-36, 0), cup + vec2(36, 0), cup + vec2(26, 14), cup + vec2(-26, 14)]);
	engraving::mass(arm, dish, new_path(), INK, 1.6);
	let candle = shapes::closed([cup + vec2(-11, 0), cup + vec2(-11, -120), cup + vec2(11, -120), cup + vec2(11, 0)]);
	arm.add(candle, inks::contour(INK, 1.5));
	arm.add(candle.hatch(3.0, #{ angle: [1.4] }).erase_inside(new_rectangle(cup + vec2(-6, -60), 12, 130)), inks::detail(INK, 0.7));
	arm.add(shapes::shape(shapes::tube([cup + vec2(9, -118), cup + vec2(13, -100), cup + vec2(12, -84)], [5.0, 5.0, 3.0])), inks::detail(INK, 1.0));
	let flame = shapes::shape(shapes::tube([cup + vec2(0, -124), cup + vec2(1, -140), cup + vec2(-2, -158), cup + vec2(2, -176)], [8.0, 13.0, 8.0, 1.0]));
	arm.add(flame, inks::contour(INK, 1.1));
	arm.add(new_line(cup + vec2(0, -120), cup + vec2(0, -130)), inks::contour(INK, 2.4));
	let glow = shapes::join(shapes::times(18, |j| {
		let g = 2.0 * PI() * j / 18.0;
		new_line(cup + vec2(0, -150) + shapes::polar(g) * 44, cup + vec2(0, -150) + shapes::polar(g) * (64 + 24 * (j % 3)))
	}));
	arm.add(glow.erase_inside(candle), inks::pencil(SEPIA, 1.0));
	leaves::place(leaf, arm, 8);
}

leaves::jot(leaf, "The round glass over the stair. All in it is backwards, and bowed, and further off than it is.", vec2(110, 70), 980, #{ size: 28, turn: 0.005 });
leaves::jot(leaf, "From the rim in to the eye. What it says is the word the six glasses keep.", vec2(80, 1440), 1040, #{ size: 28, ink: COLD, turn: -0.005 });

journal_entry.add(leaves::bind(leaf, content("nights"), #{ size: 20.0 }));
