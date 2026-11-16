// The churchyard: outside, at night. On the left, the churchyard from
// the lychgate the night he opened her grave: the yew, the stones, her
// stone carved with verses that aren't verses (book_cipher: places in his
// account, a word and a letter each), and the iron railings she wrote on
// with her finger (rail_fence: the words written down and up three rails,
// then read off rail by rail). On the right, the tower and its spire,
// and the three stars that wander over it, sighted twice from the belfry
// a year apart (star_alignment, star_sightings): the night they stand in
// one line is the key to the bells.

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

seed(1501);

// ---- the secrets (chains/psalter_churchyard) ----

let secrets = chain("psalter_churchyard").run();
let railed = secrets.get("rails.cipher_text").value();
let verses = secrets.text("stone.references").split(" ");

// ---- the left page: the churchyard ----

let leaf = ps::leaf("churchyard", 51, #{ foxing: 120, stains: 2 });
let scene = new_canvas();

// The railings first, as a shape: what stands in front of everything.
const RAILS = [1160.0, 1250.0, 1340.0];
const BAR = 40.0;
const FOOT = 1405.0;
let fence = shapes::join(RAILS.map(|y| shapes::closed([vec2(48, y - BAR / 2.0), vec2(1152, y - BAR / 2.0), vec2(1152, y + BAR / 2.0), vec2(48, y + BAR / 2.0)])));
let pickets = new_path();
const BAYS = 22;
let picket_x = |j| 72.0 + j * 1056.0 / BAYS;
for j in 0..=BAYS {
	let x = picket_x.call(j);
	pickets.append(shapes::closed([vec2(x - 5, 1100), vec2(x + 5, 1100), vec2(x + 5, FOOT), vec2(x - 5, FOOT)]));
	// The spearhead finial.
	pickets.append(shapes::closed([vec2(x - 5, 1100), vec2(x - 12, 1090), vec2(x, 1050), vec2(x + 12, 1090), vec2(x + 5, 1100)]));
}
let front = shapes::join([fence, pickets]);
let behind = |p| p.erase_inside(front, #{ fill_rule: "nonzero" });

// The sky: night, ruled close as an engraver rules it, clear about the
// moon.
let ground = |x| 980.0 - 40.0 * (x / 1200.0) + 14.0 * (x * 0.011).sin();
let sky = new_path().move_to(vec2(40, 40));
sky.line_to(vec2(1160, 40));
for k in 0..=24 { let x = 1160.0 - k * 1120.0 / 24.0; sky.line_to(vec2(x, ground.call(x))); }
sky.close();
let moon_at = vec2(880, 250);
let night = sky.hatch(7.0, #{ angle: [0.02], roughness: 2.0 });
night.erase_inside(new_circle(moon_at, 150));
let night_pen = inks::fine(SEPIA, 0.9);
// The moon, waning, with its rings.
let moon = art::moon(moon_at, 104, 0.84);
scene.add(inks::moon(moon, INK, 2.2, #{ gap: 2.4 }));
scene.add(new_circle(moon_at, 150), inks::fine(INK, 0.8));
scene.add(new_circle(moon_at, 126), inks::fine(SEPIA, 0.7));

// The yew by the north wall: an old dark flame of it, ragged, bristling
// at every edge.
let left_side = [];
let right_side = [];
for k in 0..=40 {
	let t = k / 40.0;
	let y = 985.0 - 690.0 * t;
	let half = 175.0 * (1.0 - t) ** 0.75 * (0.82 + 0.18 * (t * 9.0).sin().abs());
	let lx = 255.0 + 28.0 * t - half * (1.0 + 0.16 * (k * 2.7).sin() + 0.08 * (k * 7.1).cos());
	let rx = 255.0 + 28.0 * t + half * (1.0 + 0.16 * (k * 3.3).cos() + 0.08 * (k * 5.9).sin());
	left_side.push(vec2(lx, y));
	right_side.push(vec2(rx, y));
}
right_side.reverse();
let yew_outline = shapes::wound(left_side + right_side);
let yew = shapes::shape(yew_outline);
let yew_bare = shapes::join(shapes::times(9, |k| {
	let t = 0.1 + k * 0.09;
	let at = vec2(255.0 + 28.0 * t - 60.0 * (1.0 - t) + 50.0 * (k * 1.9).sin(), 985.0 - 690.0 * t);
	shapes::sliver([at, at + vec2(26, -10), at + vec2(54, -4)], 2.6)
}));
let tree = new_canvas();
let bristle = new_style(new_brush_style(INK, "Horrid", 9.0, #{ min_pressure: 0.55, max_pressure: 0.85, pressure_profile: "constant" }), new_handwriting_style());
tree.add(behind.call(yew.copy()), bristle);
let yew_lines = yew.hatch(2.0, #{ angle: [0.6, 0.6 + PI() / 2.0], roughness: 0.8 });
yew_lines.erase_inside(yew_bare);
let solid_pen = new_style(new_brush_style(INK, "Rough Ink", 0.85, #{ min_pressure: 0.85, max_pressure: 0.95, pressure_profile: "constant" }), new_handwriting_style());
tree.add(behind.call(yew_lines), solid_pen);
night.erase_inside(yew);

// A dead elm on the south side, bare against the moon.
let elm = gothic::bare_tree(vec2(1020, 990), -PI() / 2.0 - 0.12, 250.0, 50.0, 6, 17);
let limbs = shapes::join(elm.limbs.map(|l| shapes::shape(l)));
limbs.append(shapes::closed([vec2(970, 995), vec2(1000, 960), vec2(1040, 960), vec2(1075, 995)]));
let wall = new_canvas();
let elm_lines = limbs.hatch(2.0, #{ angle: [0.6, 0.6 + PI() / 2.0], roughness: 0.8, fill_rule: "nonzero" });
wall.add(behind.call(elm_lines), solid_pen);
wall.add(behind.call(limbs.copy()), inks::detail(INK, 0.8));
for t in elm.twigs { wall.add(behind.call(new_polyline(shapes::curve(t[0], 4))), inks::contour(INK, max(t[1] * 0.4, 0.5))); }
night.erase_inside(limbs, #{ fill_rule: "nonzero" });

// The stones, leaning as the fen has left them; hers in the middle,
// newer, and carved.
let stones = new_canvas();
let hers = vec2(660, 1000);
for s in [[vec2(430, 992), 96.0, 150.0, "round", -0.08], [vec2(850, 980), 110.0, 190.0, "shoulder", 0.05], [vec2(1110, 975), 110.0, 230.0, "cross", -0.06],
	[vec2(530, 988), 60.0, 96.0, "round", 0.14], [vec2(1130, 960), 70.0, 110.0, "gothic", 0.1], [hers, 210.0, 330.0, "gothic", 0.0]] {
	let pts = gothic::headstone(s[0], s[1], s[2], s[3], s[4]);
	let stone = shapes::shape(pts);
	night.erase_inside(stone);
	stones.add(behind.call(stone.copy()), inks::contour(INK, 1.8));
	// The shade down its right side, and lichen.
	let shade = stone.hatch(3.2, #{ angle: [1.3], roughness: 1.0 });
	shade.erase_outside(new_rectangle(s[0] + vec2(s[1] * 0.3, -s[2] / 2.0), s[1] * 0.4, s[2] * 1.2));
	stones.add(behind.call(shade), inks::fine(INK, 0.7));
	stones.add(behind.call(new_line(s[0] + vec2(-s[1] * 0.7, 2), s[0] + vec2(s[1] * 0.7, 2))), inks::contour(INK, 1.4));
	if s[0] != hers {
		// Names worn past reading.
		for j in 0..3 {
			let y = s[0].y - s[2] * 0.62 + j * 16;
			stones.add(behind.call(new_line(vec2(s[0].x - s[1] * 0.25 + j * 4, y), vec2(s[0].x + s[1] * 0.22 - j * 6, y))), inks::fine(INK, 0.9));
		}
	}
}
// Her stone's carving: a cross, HERE LIES, and her verses, in blood.
stones.add(new_line(hers + vec2(0, -290), hers + vec2(0, -246)), inks::detail(INK, 1.4));
stones.add(new_line(hers + vec2(-14, -276), hers + vec2(14, -276)), inks::detail(INK, 1.4));
stones.add(write_at("HERE LIES", hers + vec2(0, -222), leaves::carved_hand(22, INK)));
let figures = leaves::figures_hand(26, BLOOD);
for (v, k) in verses {
	stones.add(write_at(v, hers + vec2(if k % 2 == 0 { -42.0 } else { 42.0 }, -176 + (k / 2) * 40), figures));
}
// Her grave before it, opened, the spade left in the earth.
let pit = new_ellipse(hers + vec2(0, 34), 120, 18);
stones.add(behind.call(pit.copy()), inks::contour(INK, 1.6));
engraving::mass(stones, pit.erase_inside(front, #{ fill_rule: "nonzero" }), new_path(), INK, 2.2);

// The ground, the long grass along it, mounds over the others.
let earth = new_path().move_to(vec2(44, ground.call(44)));
for k in 1..=40 { let x = 44.0 + k * 1112.0 / 40.0; earth.line_to(vec2(x, ground.call(x))); }
scene.add(behind.call(earth), inks::contour(INK, 1.5));
let grass = shapes::join(shapes::times(140, |k| {
	let x = 50.0 + k * 8.0 + 4.0 * (k * 7.7).sin();
	let y = ground.call(x) + 2.0;
	let tall = 10.0 + 14.0 * ((k * 3.1).sin() * 0.5 + 0.5);
	new_line(vec2(x, y), vec2(x + 5.0 * (k * 2.3).cos(), y - tall))
}));
scene.add(behind.call(grass), inks::fine(INK, 0.9));
for m in [[430.0, 60.0], [850.0, 70.0]] {
	scene.add(behind.call(new_arc(vec2(m[0], ground.call(m[0]) + 30), m[1], PI() * 1.15, PI() * 1.85)), inks::detail(INK, 1.1));
}
// The ground before the railings, ruled dark.
let near = new_path().move_to(vec2(44, ground.call(44)));
for k in 1..=40 { let x = 44.0 + k * 1112.0 / 40.0; near.line_to(vec2(x, ground.call(x))); }
near.line_to(vec2(1156, 1420));
near.line_to(vec2(44, 1420));
near.close();
let soil = near.hatch(4.0, #{ angle: [0.08], roughness: 2.5 });
soil.erase_inside(new_ellipse(hers + vec2(0, 34), 124, 22));
soil.erase_inside(front, #{ fill_rule: "nonzero" });
scene.add(soil, inks::fine(SEPIA, 0.8));

// The rooks that won't settle on the tower.
for r in [[vec2(560, 160), 64.0, 0.5, 0.1], [vec2(660, 110), 44.0, -0.6, -0.12], [vec2(470, 230), 38.0, 0.8, 0.2], [vec2(1080, 470), 40.0, -0.3, 0.0]] {
	engraving::dark(scene, gothic::rook(r[0], r[1], r[2], r[3]), INK, 1.8);
}

// The railings: pickets of black iron and three flat rails, what she
// wrote along each rail in blood, as it reads now, rail by rail.
let iron = new_canvas();
engraving::mass(iron, pickets, new_path(), INK, 2.0);
iron.add(pickets, inks::contour(INK, 1.0));
for y in RAILS {
	iron.add(new_rectangle(vec2(600, y), 1104, BAR), inks::contour(INK, 1.8));
	iron.add(new_line(vec2(48, y + BAR / 2.0 - 5), vec2(1152, y + BAR / 2.0 - 5)), inks::fine(INK, 0.9));
	let rivets = shapes::join(shapes::steps(BAYS, |j| new_circle(vec2(picket_x.call(j), y), 3.0)));
	iron.add(rivets, inks::detail(INK, 1.0));
}
// How many of the text's letters fell to each rail, and where.
let counts = [0, 0, 0];
let round = 4;
for k in 0..railed.len() { let at = k % round; counts[if at < 3 { at } else { round - at }] += 1; }
let carved = symbol_set("letters", #{ color: BLOOD, size: 30.0 });
let n = 0;
for (y, r) in RAILS {
	for i in 0..counts[r] {
		let bay = ((i + 0.5) * BAYS / counts[r]).floor();
		let x = (picket_x.call(bay) + picket_x.call(bay + 1)) / 2.0;
		iron.add(write_at(carved.text([railed[n]]), vec2(x, y), #{}));
		n += 1;
	}
}

let page = new_canvas();
page.add(night, night_pen);
page.add(scene);
page.add(wall);
page.add(tree);
page.add(stones);
page.add(iron);
leaf.figures.add(page);

leaves::jot(leaf, "From the lychgate, the night I opened her grave. Her stone says verses no psalter has.", vec2(104, 64), 420, #{ size: 30, turn: -0.01 });
leaves::jot(leaf, "Down the rails and up again, as a snake goes; the iron reads it a rail at a time.", vec2(84, 1440), 1070, #{ size: 25, ink: BLOOD, turn: 0.004 });

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the spire and the three that wander ----

let leaf = ps::leaf("spire", 53, #{ foxing: 130 });

// Two sightings of the three, from the belfry, a year apart: the month
// they'll stand in one line over the spire is the secret.
let seen = secrets.get("watch.sightings");
let first = illustration("star_sightings", #{ size: 500.0, seed: 6, splatter: 0, editor: "first" }).run(#{ sightings: seen })[0];
let last = illustration("star_sightings", #{ size: 500.0, seed: 6, splatter: 0, editor: "last" }).run(#{ sightings: seen })[1];
let first_at = vec2(620, 70);
let last_at = vec2(600, 640);
leaves::place(leaf, first.translate_by(first_at), 10);
leaves::place(leaf, last.translate_by(last_at), 10);

// The tower and its spire: a dark mass, its belfry open, a clock of
// thirteen hours on it, gargoyles at its corners.
let tower = new_path();
let tx = 270.0;
tower.append(shapes::closed([vec2(tx - 130, 1600), vec2(tx - 130, 900), vec2(tx + 130, 900), vec2(tx + 130, 1600)]));
// Buttresses, stepped.
for side in [-1.0, 1.0] {
	let x = tx + side * 130;
	tower.append(shapes::closed([vec2(x - 18, 1600), vec2(x - 18, 1000), vec2(x, 975), vec2(x + 18, 1000), vec2(x + 18, 1600)]));
	tower.append(shapes::closed([vec2(x - 26, 1600), vec2(x - 26, 1300), vec2(x, 1270), vec2(x + 26, 1300), vec2(x + 26, 1600)]));
	// A pinnacle at the corner.
	tower.append(shapes::closed([vec2(x - 14, 900), vec2(x - 14, 850), vec2(x, 780), vec2(x + 14, 850), vec2(x + 14, 900)]));
}
// The parapet, crenellated.
let crenels = [vec2(tx - 140, 900), vec2(tx - 140, 870)];
for k in 0..7 {
	let x0 = tx - 140 + k * 40;
	crenels += [vec2(x0 + 20, 870), vec2(x0 + 20, 884), vec2(x0 + 40, 884), vec2(x0 + 40, 870)];
}
crenels += [vec2(tx + 140, 900)];
tower.append(shapes::closed(crenels));
// The spire, and its crockets.
let apex = vec2(tx, 190);
tower.append(shapes::closed([vec2(tx - 100, 872), apex, vec2(tx + 100, 872)]));
let crockets = new_path();
for k in 1..11 {
	let t = k / 11.0;
	for side in [-1.0, 1.0] {
		let on = vec2(tx + side * 100 * (1.0 - t), 872 - (872 - 190) * t);
		crockets.append(shapes::shape(shapes::tube([on, on + vec2(side * 12, -4), on + vec2(side * 16, -16), on + vec2(side * 9, -20)], [9.0, 7.0, 5.0, 2.0])));
	}
}
tower.append(crockets);
// Gargoyles: a beast's head straining out from each corner, jaws open.
for side in [-1.0, 1.0] {
	let at = vec2(tx + side * 150, 930);
	let g = [vec2(0, -14), vec2(30, -22), vec2(52, -16), vec2(66, -26), vec2(74, -14), vec2(62, -6), vec2(80, 0), vec2(58, 6), vec2(40, 14), vec2(0, 16)];
	let pts = [];
	for p in g { pts.push(at + vec2(p.x * side, p.y)); }
	tower.append(shapes::closed(pts));
}
// The cross at the top.
tower.append(shapes::closed([vec2(tx - 4, 196), vec2(tx - 4, 150), vec2(tx + 4, 150), vec2(tx + 4, 196)]));
tower.append(shapes::closed([vec2(tx - 18, 166), vec2(tx + 18, 166), vec2(tx + 18, 174), vec2(tx - 18, 174)]));

// What's left open: the belfry's two lights, a slit window, the clock.
let open = new_path();
let lights = [];
for side in [-1.0, 1.0] {
	let foot = vec2(tx + side * 52, 1150);
	let pts = shapes::lancet_points(foot, 62, 150, 1.0);
	lights.push(pts);
	open.append(shapes::closed(pts));
}
open.append(shapes::closed(shapes::lancet_points(vec2(tx, 1500), 22, 80, 1.0)));
let clock = vec2(tx, 1300);
open.append(new_circle(clock, 64));
// Light on the stone: the spire's ridge, string courses, quoins.
open.append(shapes::sliver([apex + vec2(2, 30), vec2(tx + 16, 530), vec2(tx + 30, 872)], 7.0));
for y in [1190.0, 1420.0] {
	open.append(shapes::sliver([vec2(tx - 124, y), vec2(tx - 60, y), vec2(tx + 60, y), vec2(tx + 124, y)], 6.0));
}
for j in 0..14 {
	let y = 930.0 + j * 46.0;
	let w = if j % 2 == 0 { 26.0 } else { 14.0 };
	open.append(shapes::sliver([vec2(tx + 124 - w, y), vec2(tx + 124 - w / 2, y), vec2(tx + 124, y)], 5.0));
}
let mass = new_canvas();
engraving::mass(mass, tower, open, INK, 2.0);
// The belfry's louvres, and the bell dark behind them.
for pts in lights {
	let light = shapes::shape(pts);
	let slats = new_path();
	for j in 0..9 { let y = 1010.0 + j * 15.0; slats.append(new_line(vec2(tx - 100, y), vec2(tx + 100, y + 8))); }
	mass.add(slats.erase_outside(light), inks::contour(INK, 2.2));
	mass.add(light, inks::detail(INK, 1.2));
}
// The clock: thirteen hours, the hands at the thirteenth.
mass.add(new_circle(clock, 64), inks::contour(INK, 1.6));
mass.add(new_circle(clock, 54), inks::detail(INK, 1.0));
mass.add(plates::ticks(clock, 13, 46, 54, 0, 0.0), inks::detail(INK, 1.4));
let priests = symbol_set("priests", #{ color: INK, size: 14.0 });
for k in 0..13 {
	let a = -PI() / 2.0 + 2.0 * PI() * k / 13.0;
	mass.add(write_at(priests.text([k]), clock + shapes::polar(a) * 36, #{}));
}
mass.add(new_line(clock, clock + vec2(0, -44)), inks::contour(INK, 1.8));
mass.add(new_line(clock, clock + vec2(-3, -30)), inks::contour(INK, 2.4));
mass.add(new_circle(clock, 4), inks::fill(INK, 2.0));
leaves::place(leaf, mass, 10);

// Rooks round the spire.
for r in [[vec2(430, 300), 84.0, 0.6, 0.2], [vec2(150, 430), 60.0, -0.5, -0.1], [vec2(480, 480), 48.0, 0.2, 0.3], [vec2(100, 250), 42.0, 0.9, 0.0]] {
	let bird = new_canvas();
	engraving::dark(bird, gothic::rook(r[0], r[1], r[2], r[3]), INK, 1.8);
	leaves::place(leaf, bird, 6);
}

// Sightlines from the spire's top to each plate, in pencil.
leaf.under.add(new_line(apex, first_at + vec2(250, 250)), inks::pencil(SEPIA, 1.0));
leaf.under.add(new_line(apex, last_at + vec2(250, 250)), inks::pencil(SEPIA, 1.0));

// How each one creeps: its houses in a ring, and the step it takes in a
// year (thirteen months round an orbit of five is three houses on).
for (period, k) in seen.value().periods {
	let at = vec2(700 + k * 170, 1225);
	let step = 13 % period;
	leaf.figures.add(new_circle(at, 50), inks::contour(INK, 1.4));
	leaf.figures.add(plates::ticks(at, period, 50, 60, 0, 0.0), inks::contour(INK, 1.5));
	leaf.figures.add(new_polygon(at, 48, period, #{ step: max(step, 1) }), inks::detail(BLOOD, 1.3));
	leaf.figures.add(write_line(`${step} of ${period}`, at + vec2(-30, 86), leaves::note_hand(24.0, BLOOD)));
	leaves::clear(leaf, new_circle(at + vec2(0, 20), 80), 4);
}

leaves::jot(leaf, "From the louvres, the same month a year apart.", vec2(580, 1350), 540, #{ size: 28, turn: 0.01 });
leaves::jot(leaf, "One line, all three, over the spire: that night is the key to the bells.", vec2(580, 1440), 540, #{ size: 28, ink: BLOOD, turn: -0.01 });

journal_entry.add(leaves::bind(leaf, content("psalter"), #{ size: 20.0 }));
