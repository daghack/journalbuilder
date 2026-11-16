// The mere: its soundings. On the left, the mere cut through as a
// surveyor draws it: the punt on the dead-still surface, the keeper
// leaning out with the lantern, and the line let down into black water
// between the banks, the banks giving out before any bottom; the line
// tagged at every fathom and cut at its end. What the tags say is the
// night's soundings, put through the reckoning board
// (straddling_checkerboard, key FATHOM, blanks 2 and 6). Below, turning,
// the flank of something larger than the hall. On the right, the chart:
// the mere in plan, its depths, the course out to where the line was
// cut, the board itself, and under the chart's title a line in the old
// way (autokey, primer the motto's hidden word).

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

seed(4101);

// ---- the secrets (chains/mere_soundings) ----

let puzzle = chain("mere_soundings");
let secrets = puzzle.run();
let solved = puzzle.unlock(secrets);
let marked = secrets.get("soundings.cipher_text").value();
let under_title = secrets.text("chart.cipher_text");

// ---- the left page: the line ----

let leaf = mb::leaf("mere", 41, #{ foxing: 110, stains: 1 });
let SURF = 520.0;
let DEPTH = 1372.0;
let LX = 566.0;

// The banks in section: the earth either side, giving out as it goes
// down, so the mere has no bottom drawn.
// Narrowing, then opening out again beneath them: wider below than above.
let bank_l = [vec2(-10, SURF - 30), vec2(150, SURF - 20), vec2(230, SURF), vec2(268, 700), vec2(330, 900), vec2(352, 1050), vec2(300, 1180), vec2(160, 1280), vec2(40, DEPTH - 10), vec2(-10, DEPTH + 40)];
let bank_r = [vec2(1210, SURF - 40), vec2(1040, SURF - 24), vec2(960, SURF), vec2(930, 680), vec2(880, 860), vec2(862, 1030), vec2(910, 1170), vec2(1050, 1280), vec2(1170, DEPTH - 10), vec2(1210, DEPTH + 40)];
let earth = new_path().append(shapes::closed(bank_l)).append(shapes::closed(bank_r));
// The water's cut face, ending ragged below, where the drawing gives up.
let face = [vec2(0, SURF), vec2(1200, SURF)];
for j in 0..=24 {
	let x = 1200.0 - j * 50.0;
	face.push(vec2(x, DEPTH + 18.0 * chance::either_way(42, j, 1) - 10.0 * (x * 0.01).sin()));
}
let water = shapes::closed(face);

// What's left bare in the black: the line, its tags, its cut end, the
// eddies, and the flank of what turns below.
let tags = [];
for (d, k) in marked {
	tags.push([vec2(LX + 24, SURF + 44 + k * 40), d]);
}
let LINE_END = SURF + 44 + marked.len() * 40 + 6;
let bare = new_path();
bare.append(shapes::sliver([vec2(LX, SURF + 2), vec2(LX + 2, (SURF + LINE_END) / 2.0), vec2(LX + 5, LINE_END)], 3.4));
for t in tags {
	bare.append(shapes::closed([t[0] + vec2(-14, -12), t[0] + vec2(16, -12), t[0] + vec2(16, 12), t[0] + vec2(-14, 12)]));
	bare.append(shapes::sliver([vec2(LX + 1, t[0].y - 3), t[0] + vec2(-8, -2), t[0] + vec2(-14, 0)], 2.0));
}
// The end, cut: the strands splayed.
for s in [-1.0, 0.0, 1.0] {
	bare.append(shapes::sliver([vec2(LX + 5, LINE_END - 4), vec2(LX + 5 + s * 5, LINE_END + 10), vec2(LX + 5 + s * 11, LINE_END + 20)], 2.0));
}
for w in [[420.0, 980.0, 70.0, 1.6, 0.3], [740.0, 1150.0, 90.0, 1.9, 2.0], [480.0, 1260.0, 50.0, 1.3, 4.0], [760.0, 760.0, 46.0, 1.2, 1.1]] {
	bare.append(shapes::sliver(shapes::whorl_points(vec2(w[0], w[1]), w[2] * 0.1, w[2], w[3], 0.45, w[4]), 4.2));
}
// The flank: ridged, curving up out of the dark to the right, vast.
let FLANK = vec2(1180, 1530);
for k in 0..6 {
	let r = 430.0 + k * 22.0;
	let pts = shapes::steps(40, |j| {
		let a = PI() * (1.02 + 0.42 * j / 40.0);
		FLANK + vec2(a.cos() * r, a.sin() * r * 0.9) + vec2(0, 6.0 * (j * 0.9 + k).sin())
	});
	bare.append(shapes::sliver(pts, if k == 0 { 6.0 } else { 3.6 }));
}
let wet = new_canvas();
engraving::black_water(wet, water, bare.copy().append(earth), INK, #{ gap: 2.8, weight: 2.0, seed: 41 });
// The earth either side, hatched, stones in it, roots near the top.
let ground = new_canvas();
let face_cut = |p| p.erase_outside(water, #{ fill_rule: "nonzero" });
let banks = earth.copy();
ground.add(face_cut.call(earth.hatch(3.4, #{ angle: [0.9], roughness: 1.2, fill_rule: "nonzero" })), inks::detail(INK, 0.9));
ground.add(face_cut.call(earth.hatch(5.0, #{ angle: [2.4], roughness: 1.2, fill_rule: "nonzero" })), inks::fine(SEPIA, 0.8));
let stones = new_path();
for k in 0..34 {
	let side = if k % 2 == 0 { 0 } else { 1 };
	let y = SURF + 40 + 560 * chance::of(43, k, 1);
	let x = if side == 0 { 30 + 180 * chance::of(43, k, 2) } else { 1000 + 170 * chance::of(43, k, 2) };
	stones.append(shapes::closed(shapes::ellipse_points(vec2(x, y), 6 + 12 * chance::of(43, k, 3), 4 + 7 * chance::of(43, k, 4), 12)));
}
engraving::mass(ground, stones, new_path(), INK, 1.2);
leaf.figures.add(ground).add(wet);
// The banks' edge against the water.
leaf.figures.add(new_canvas().add(new_polyline(shapes::curve(bank_l.extract(1, 8), 6)), inks::contour(INK, 2.0)).add(new_polyline(shapes::curve(bank_r.extract(1, 8), 6)), inks::contour(INK, 2.0)));

// What the tags read, in the red of the tally.
let tagged = new_canvas();
for t in tags {
	tagged.add(leaves::centred(t[1].to_string(), t[0] + vec2(1, 0), leaves::figures_hand(20.0, BLOOD)));
}
leaf.figures.add(tagged);

// ---- above the water: the sky, the punt, the keeper, the lantern ----

let LANTERN = vec2(812, 318);
let punt = shapes::closed([vec2(330, SURF - 30), vec2(800, SURF - 30), vec2(782, SURF - 2), vec2(352, SURF - 2)]);
// The keeper, kneeling, cloaked, leaning out over the line.
let keeper = new_path();
keeper.append(shapes::shape(shapes::tube([vec2(640, SURF - 30), vec2(628, SURF - 84), vec2(598, SURF - 124)], [64.0, 58.0, 40.0])));
keeper.append(shapes::closed(shapes::ellipse_points(vec2(584, SURF - 138), 22, 24, 20)));
keeper.append(shapes::shape(shapes::tube([vec2(592, SURF - 146), vec2(608, SURF - 164), vec2(630, SURF - 172)], [34.0, 16.0, 2.0])));
keeper.append(shapes::shape(shapes::tube([vec2(606, SURF - 112), vec2(584, SURF - 82), vec2(LX + 4, SURF - 52)], [16.0, 12.0, 10.0])));
let pole = shapes::shape(shapes::tube([vec2(780, SURF - 30), vec2(800, SURF - 160), LANTERN + vec2(-4, -50)], [7.0, 6.0, 5.0]));
let lamp = shapes::closed([LANTERN + vec2(-13, -20), LANTERN + vec2(13, -20), LANTERN + vec2(16, 18), LANTERN + vec2(-16, 18)]);
let lit = new_rectangle(LANTERN + vec2(0, 1), 16, 22);
let shapes = new_path().append(punt).append(keeper).append(pole).append(lamp);
let sky = new_path();
let y = 40.0;
while y < SURF - 2.0 {
	sky.append(new_line(vec2(0, y + 0.8 * (y * 0.3).sin()), vec2(1200, y - 0.8 * (y * 0.21).sin())));
	y += if y < 260.0 { 4.6 } else { 6.0 };
}
let glow = new_circle(LANTERN, 120);
let MOON = vec2(1010, 150);
let moon = art::moon(MOON, 48, 0.18);
let said = leaves::note("Out to the middle, the night it was stillest. Sixty fathoms of line and nothing took it; and it came up cut, as by a blade.", vec2(70, 62), 760, #{ ink: INK, size: 28, turn: 0.004 });
let sky_shape = shapes::closed([vec2(0, 30), vec2(1200, 30), vec2(1200, SURF - 2), vec2(0, SURF - 2)]);
let night = new_canvas();
night.add(sky.erase_outside(sky_shape).erase_inside(glow).erase_inside(said.hole).erase_inside(new_circle(MOON, 66)).erase_inside(banks, #{ fill_rule: "nonzero" }).erase_inside(shapes, #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.8));
night.add(new_canvas().add(moon, #{ outline: inks::contour(INK, 1.4), terminator: inks::contour(INK, 1.2) }));
if !moon.dark.is_empty { night.add(inks::solid(moon.dark, 2.4), inks::fill(INK, 2.4)); }
// The lantern's light: rays, the pane bare.
let rays = shapes::join(shapes::times(18, |j| {
	let a = 2.0 * PI() * j / 18.0;
	new_line(LANTERN + shapes::polar(a) * 34, LANTERN + shapes::polar(a) * (if j % 2 == 0 { 96.0 } else { 70.0 }))
}));
night.add(rays.erase_inside(shapes, #{ fill_rule: "nonzero" }), inks::fine(INK, 0.8));
leaf.figures.add(night);
let boat = new_canvas();
engraving::mass(boat, shapes, lit, INK, 1.3);
// The line from the keeper's hand down to the water.
boat.add(new_line(vec2(LX + 4, SURF - 50), vec2(LX, SURF + 2)), inks::contour(INK, 1.4));
leaf.figures.add(boat);
// Reeds on both banks, black against the sky.
let rushes = new_canvas();
engraving::mass(rushes, gothic::reeds(SURF - 18, 10, 220, 170, 26, 41).append(gothic::reeds(SURF - 22, 980, 1190, 190, 26, 42)), new_path(), INK, 1.2);
leaf.figures.add(rushes);
leaf.figures.add(said.canvas);

leaves::jot(leaf, "Marked as it came up, a fathom at a time. Read it by the board.", vec2(80, 1430), 1040, #{ size: 28, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the chart ----

let leaf = mb::leaf("chart", 43, #{ foxing: 130, stains: 2 });
let O = vec2(600, 700);
let shore = shapes::times(72, |j| {
	let a = 2.0 * PI() * j / 72.0;
	let r = 1.0 + 0.08 * (3.0 * a + 1.0).sin() + 0.05 * (5.0 * a).sin() + 0.025 * chance::either_way(44, j, 1);
	O + vec2(a.cos() * 430.0 * r, a.sin() * 400.0 * r)
});
let lake = shapes::closed(shore);
let DEEP = vec2(624, 734);
let deep_pts = shapes::times(30, |j| {
	let a = 2.0 * PI() * j / 30.0;
	DEEP + vec2(a.cos(), a.sin() * 0.8) * (104.0 + 16.0 * (4.0 * a).sin() + 8.0 * chance::either_way(45, j, 1))
});
let deep = shapes::closed(deep_pts);
let JETTY = shore[44];
// The course out: dashes from the jetty to the deep's edge.
let course = new_path();
let from = JETTY + (O - JETTY).normalized() * 40.0;
let to = DEEP + (from - DEEP).normalized() * 124.0;
let steps = ((to - from).length() / 22.0).to_int();
for k in 0..steps {
	let a = from + (to - from) * (k * 1.0 / steps);
	course.append(new_line(a, a + (to - from).normalized() * 11.0));
}
// The soundings: depths rising toward the deep, kept off the course.
let depths = new_canvas();
let kept = new_path().append(deep);
let placed_at = [];
for k in 0..60 {
	let a = 2.0 * PI() * chance::of(46, k, 1);
	let f = 0.15 + 0.8 * chance::of(46, k, 2);
	let at = O + vec2(a.cos() * 400.0 * f, a.sin() * 370.0 * f);
	let near_course = false;
	for j in 0..=10 { if (at - (from + (to - from) * (j / 10.0))).length() < 34.0 { near_course = true; } }
	let crowded = false;
	for q in placed_at { if (at - q).length() < 46.0 { crowded = true; } }
	if (at - DEEP).length() < 150.0 || near_course || crowded { continue; }
	placed_at.push(at);
	let fathoms = (4.0 + 54.0 * (1.0 - ((at - DEEP).length() / 460.0)).max(0.0) ** 1.6 + 3.0 * chance::of(46, k, 3)).to_int();
	let placed = leaves::centred(fathoms.to_string(), at, leaves::figures_hand(18.0, INK));
	let pb = placed.bounds();
	let box = new_rectangle(at, pb.max.x - pb.min.x + 10, pb.max.y - pb.min.y + 8);
	depths.add(placed);
	kept.append(box);
}
// The water ruled as a chart rules it; the deep left black.
let ruling = new_path();
let y = O.y - 440.0;
while y < O.y + 440.0 {
	ruling.append(new_line(vec2(O.x - 480, y), vec2(O.x + 480, y)));
	y += 7.0;
}
let chart = new_canvas();
chart.add(ruling.erase_outside(lake, #{ fill_rule: "nonzero" }).erase_inside(kept, #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.8));
let swirl = shapes::join([[DEEP + vec2(-30, -10), 50.0, 1.6, 0.0], [DEEP + vec2(40, 30), 36.0, 1.3, 2.0]].map(|w| {
	shapes::sliver(shapes::whorl_points(w[0], 4.0, w[1], w[2], 0.8, w[3]), 2.6)
}));
engraving::black_water(chart, deep, swirl, INK, #{ gap: 2.6, weight: 1.9, seed: 43 });
chart.add(deep, inks::contour(INK, 1.6));
chart.add(lake, inks::contour(INK, 2.2));
// Hachures round the shore, short strokes out from the water.
let hachures = shapes::join(shapes::times(216, |k| {
	let t = k / 216.0 * 72.0;
	let j = t.floor().to_int();
	let f = t - j;
	let p = shore[j % 72] + (shore[(j + 1) % 72] - shore[j % 72]) * f;
	let out = (p - O).normalized();
	new_line(p + out * 3, p + out * (10.0 + 10.0 * chance::of(47, k, 1)))
}));
chart.add(hachures, inks::detail(INK, 0.9));
chart.add(depths);
chart.add(course, inks::contour(INK, 1.4));
let cross = new_path().append(new_line(to + vec2(-12, -12), to + vec2(12, 12))).append(new_line(to + vec2(12, -12), to + vec2(-12, 12)));
chart.add(cross, inks::contour(BLOOD, 2.2));
// The hall, in plan, and its stair down to the jetty.
let hall = shapes::closed([JETTY + vec2(-130, -150), JETTY + vec2(-30, -150), JETTY + vec2(-30, -70), JETTY + vec2(-130, -70)]);
chart.add(hall, inks::contour(INK, 1.8));
chart.add(hall.hatch(3.0, #{ angle: [0.8] }), inks::detail(INK, 0.8));
chart.add(new_line(JETTY + vec2(-80, -70), JETTY), inks::detail(INK, 1.2));
chart.add(new_line(JETTY + vec2(-6, -6), JETTY + (O - JETTY).normalized() * 40.0 + vec2(-6, -6)), inks::contour(INK, 1.6));
chart.add(new_line(JETTY + vec2(6, 6), JETTY + (O - JETTY).normalized() * 40.0 + vec2(6, 6)), inks::contour(INK, 1.6));
// Tufts of reed round the margins.
let tufts = new_path();
for k in 0..70 {
	let j = (chance::of(48, k, 1) * 72.0).floor().to_int() % 72;
	let p = shore[j] + (shore[j] - O).normalized() * (20.0 + 30.0 * chance::of(48, k, 2));
	for d in [-1.0, 0.0, 1.0] { tufts.append(new_line(p, p + vec2(d * 4.0, -10.0 - 4.0 * chance::of(48, k, 3)))); }
}
chart.add(tufts.erase_inside(hall), inks::detail(INK, 0.9));
leaves::place(leaf, chart, 10);

// A compass rose, its points half dark.
let rose = new_canvas();
let R = vec2(1060, 1240);
let dark_halves = new_path();
let points_out = new_path();
for k in 0..8 {
	let a = -PI() / 2.0 + k * PI() / 4.0;
	let len = if k % 2 == 0 { 86.0 } else { 50.0 };
	let way = shapes::polar(a);
	let side = vec2(-way.y, way.x);
	let tip = R + way * len;
	points_out.append(shapes::shape([R + side * 12.0, tip, R - side * 12.0]));
	dark_halves.append(shapes::closed([R, tip, R + side * 12.0]));
}
rose.add(points_out, inks::contour(INK, 1.4));
engraving::mass(rose, dark_halves, new_path(), INK, 1.2);
rose.add(new_circle(R, 30), inks::detail(INK, 1.0));
let n = write_line("N", vec2(0, 0), leaves::figures_hand(24.0, INK));
let nb_ = n.bounds();
rose.add(n.translate_by(R + vec2(0, -106) - nb_.center));
leaves::place(leaf, rose, 8);

// The reckoning board: ten columns, its top row's two blanks heading
// the rows below. A dash is a space.
let reckoning = new_canvas();
let order = [];
let all = solved.get("soundings.key").value();
for s in 0..27 { all.push(s); }
for s in all { if !order.contains(s) { order.push(s); } }
let cell = 34.0;
let B0 = vec2(780, 70);
let top_cols = [0, 1, 3, 4, 5, 7, 8, 9];
let put = |text, at, ink| leaves::centred(text, at, leaves::figures_hand(21.0, ink));
for d in 0..10 { reckoning.add(put.call(d.to_string(), B0 + vec2((d + 1.5) * cell, cell * 0.5), INK)); }
for (name, row) in ["2", "6"] {
	reckoning.add(put.call(name, B0 + vec2(cell * 0.5, (row + 2.5) * cell), INK));
}
for (s, n) in order {
	let at = if n < 8 { B0 + vec2((top_cols[n] + 1.5) * cell, 1.5 * cell) } else if n < 18 { B0 + vec2((n - 8 + 1.5) * cell, 2.5 * cell) } else { B0 + vec2((n - 18 + 1.5) * cell, 3.5 * cell) };
	let ch = if s == 0 { "-" } else { "-ABCDEFGHIJKLMNOPQRSTUVWXYZ".sub_string(s, 1) };
	reckoning.add(put.call(ch, at, INK));
}
let grid = shapes::join(shapes::steps(4, |r| new_line(B0 + vec2(0, r * cell), B0 + vec2(11.0 * cell, r * cell))));
for c in 0..=11 { grid.append(new_line(B0 + vec2(c * cell, 0), B0 + vec2(c * cell, 4.0 * cell))); }
reckoning.add(grid, inks::detail(INK, 1.0));
reckoning.add(new_rectangle(B0 + vec2(5.5 * cell, 2.0 * cell), 11.0 * cell + 10, 4.0 * cell + 10), inks::contour(INK, 1.8));
leaves::place(leaf, reckoning, 10);

// The cartouche: the chart's title, and under it the line in the old way.
let cart = new_canvas();
let CC = vec2(560, 1262);
let scroll = new_path();
scroll.append(new_rectangle(CC, 700, 128));
for s in [-1.0, 1.0] {
	let end = CC + vec2(s * 350, 0);
	scroll.append(new_polyline(shapes::curve([end + vec2(0, -64), end + vec2(s * 30, -60), end + vec2(s * 34, -30), end + vec2(s * 14, -24), end + vec2(s * 10, -40)], 6)));
	scroll.append(new_polyline(shapes::curve([end + vec2(0, 64), end + vec2(s * 30, 60), end + vec2(s * 34, 30), end + vec2(s * 14, 24), end + vec2(s * 10, 40)], 6)));
}
cart.add(scroll, inks::contour(INK, 1.8));
cart.add(new_rectangle(CC, 680, 108), inks::detail(INK, 1.0));
let title = write_line("THE MERE AT HOLLIN", vec2(0, 0), leaves::title_hand(38.0, INK));
let tb = title.bounds();
cart.add(title.translate_by(CC + vec2(0, -22) - tb.center));
cart.add(leaves::centred(under_title, CC + vec2(0, 30), leaves::figures_hand(28.0, BLOOD)));
leaves::place(leaf, cart, 10);

leaves::jot(leaf, "From the hall stair. Where the cross is, the line went down and came up cut.", vec2(70, 64), 640, #{ size: 27, turn: 0.006 });
leaves::jot(leaf, "Under the title, in the old way: each letter keyed by the one before, the first by the word the motto hides.", vec2(70, 1366), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, content("mere"), #{ size: 20.0 }));
