// The hunt. On the left, the old flagged court below the hall, half sunk
// in the mere: six flags by six, a letter scored on most of them, and
// across them the hounds' tracks, every bound a knight's move, from where
// they came out of the reeds to where they went into the water
// (knights_tour). Over it, against the night, the pack that runs there
// still. On the right, the game-book: seven nights, what each took, in
// numbers (homophonic), and the roll that gives each name its many
// numbers.

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

seed(6101);

// ---- the secrets (chains/mere_hunt) ----

let puzzle = chain("mere_hunt");
let secrets = puzzle.run();
let TRAIL = puzzle.unlock(secrets).text("court.plain_text");
let tour = secrets.get("court.tour").value().squares;
let numbers = secrets.get("game.cipher_text").value();
let roll = secrets.get("game.table").value().homophones;

// A hound running, as solid shapes: at its middle, s its size, facing
// 1 (right) or -1, stride 0 (gathered) to 1 (stretched out).
let hound = |at, s, facing, stride| {
	let p = |x, y| at + vec2(x * facing, y) * s;
	let st = stride;
	let shapes = new_path();
	shapes.append(shapes::shape(shapes::tube([p.call(-58, 2), p.call(-20, -8), p.call(22, -10), p.call(42, -4)], [26.0 * s, 18.0 * s, 22.0 * s, 30.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(40, -12), p.call(60, -30), p.call(84, -32), p.call(112, -24)], [22.0 * s, 15.0 * s, 13.0 * s, 4.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(76, -36), p.call(70, -46), p.call(62, -50)], [9.0 * s, 6.0 * s, 1.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(36, 8), p.call(54 + 10 * st, 28), p.call(76 + 24 * st, 34 - 8 * st)], [10.0 * s, 6.0 * s, 3.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(30, 10), p.call(40 + 14 * st, 34), p.call(58 + 30 * st, 46 - 4 * st)], [9.0 * s, 6.0 * s, 3.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(-50, 6), p.call(-74 - 6 * st, 24), p.call(-100 - 18 * st, 26 + 6 * st)], [14.0 * s, 7.0 * s, 3.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(-44, 8), p.call(-58 - 4 * st, 32), p.call(-84 - 14 * st, 44)], [12.0 * s, 6.0 * s, 3.0 * s])));
	shapes.append(shapes::shape(shapes::tube([p.call(-60, -4), p.call(-86, -18), p.call(-110, -12)], [7.0 * s, 4.0 * s, 1.0 * s])));
	shapes
};

// A hound's print, pointing along `way`.
let pawprint = |at, way, size| {
	let side = vec2(-way.y, way.x);
	let marks = new_path();
	marks.append(shapes::closed(shapes::ellipse_points(at - way * size * 0.25, size * 0.42, size * 0.36, 12)));
	for t in [[-0.5, 0.1], [-0.18, 0.42], [0.18, 0.42], [0.5, 0.1]] {
		marks.append(shapes::closed(shapes::ellipse_points(at + way * size * (0.25 + t[1] * 0.5) + side * size * t[0] * 0.8, size * 0.15, size * 0.15, 8)));
	}
	marks
};

// ---- the left page: the court ----

let leaf = mb::leaf("hunt", 61, #{ foxing: 100, stains: 1 });
let B = vec2(600, 770);
let CELL = 108.0;
let centre_of = |n| B + vec2((n % 6 - 2.5) * CELL, (n / 6 - 2.5) * CELL);
let WATER_Y = 1150.0;

// The sky, and the pack across it.
let pack = shapes::join([[220.0, 230.0, 0.9, 0.2], [430.0, 170.0, 1.05, 0.9], [640.0, 250.0, 0.95, 0.5], [860.0, 190.0, 1.1, 1.0], [1060.0, 260.0, 0.8, 0.1], [330.0, 300.0, 0.7, 0.7]].map(|h| {
	hound.call(vec2(h[0], h[1]), h[2], 1.0, h[3])
}));
let sky = new_path();
let y = 70.0;
while y < 370.0 {
	sky.append(new_line(vec2(20, y + 0.8 * (y * 0.3).sin()), vec2(1180, y - 0.8 * (y * 0.21).sin())));
	y += 4.4 + (y - 70.0) * 0.012;
}
let said = leaves::note("They run on the nights it is stillest, over the court and out across the water, and the hounds in the yard answer them.", vec2(70, 40), 1060, #{ ink: INK, size: 27, turn: 0.004 });
let night = new_canvas();
night.add(sky.erase_inside(pack, #{ fill_rule: "nonzero" }).erase_inside(said.hole).erase_inside(new_circle(vec2(120, 150), 52)), inks::fine(SEPIA, 0.8));
engraving::mass(night, pack, new_path(), INK, 1.2);
let moon = art::moon(vec2(120, 150), 38, 0.62);
night.add(new_canvas().add(moon, #{ outline: inks::contour(INK, 1.3), terminator: inks::contour(INK, 1.1) }));
if !moon.dark.is_empty { night.add(inks::solid(moon.dark, 2.2), inks::fill(INK, 2.2)); }
leaf.figures.add(night).add(said.canvas);

// The court: its flags a little out of true, the dark ones shaded,
// cracked here and there; round it, flags broken and gone to grass.
let court = new_canvas();
let flags = new_path();
let dark = new_path();
for n in 0..36 {
	let c = centre_of.call(n);
	let h = CELL / 2.0 - 3.0;
	let j = |q| 3.0 * chance::either_way(62, n * 4 + q, 1);
	let stone = shapes::closed([c + vec2(-h + j.call(0), -h + j.call(1)), c + vec2(h + j.call(2), -h + j.call(3)), c + vec2(h + j.call(1), h + j.call(0)), c + vec2(-h + j.call(3), h + j.call(2))]);
	flags.append(stone);
	if (n % 6 + n / 6) % 2 == 1 { dark.append(stone); }
}
court.add(flags, inks::contour(INK, 1.3));
court.add(dark.hatch(4.6, #{ angle: [0.9], roughness: 0.8, fill_rule: "nonzero" }), inks::fine(SEPIA, 0.9));
let cracks = shapes::join(shapes::times(9, |k| {
	let c = centre_of.call((chance::of(63, k, 1) * 36.0).floor().to_int());
	let a = c + vec2(-40, -10 + 30 * chance::either_way(63, k, 2));
	new_polyline([a, a + vec2(22, 8), a + vec2(36, -2), a + vec2(54, 12)])
}));
court.add(cracks, inks::detail(INK, 0.9));
// The broken flags beyond the court, and the grass between.
let rubble = new_path();
for k in 0..40 {
	let a = 2.0 * PI() * chance::of(64, k, 1);
	let d = 3.6 + 1.6 * chance::of(64, k, 2);
	let c = B + vec2(a.cos() * d * CELL, a.sin() * d * CELL * 0.8);
	if c.y > WATER_Y - 30.0 || c.y < 400.0 || ((c.x - B.x).abs() < 3.5 * CELL && (c.y - B.y).abs() < 3.5 * CELL) { continue; }
	let w = 30.0 + 40.0 * chance::of(64, k, 3);
	rubble.append(shapes::closed([c + vec2(-w / 2.0, -w * 0.3), c + vec2(w / 2.0, -w * 0.35), c + vec2(w * 0.4, w * 0.3), c + vec2(-w * 0.45, w * 0.32)]));
}
court.add(rubble, inks::contour(INK, 1.1));
court.add(rubble.hatch(4.0, #{ angle: [2.2], fill_rule: "nonzero" }), inks::fine(SEPIA, 0.8));
leaf.figures.add(court);

// The letters scored on the flags the hounds crossed; a space is a flag
// they crossed with nothing on it.
let scored = new_canvas();
for (ch, k) in TRAIL.chars() {
	if ch == ' ' { continue; }
	scored.add(leaves::centred(ch.to_string(), centre_of.call(tour[k]), leaves::figures_hand(40.0, BLOOD)));
}
// The tracks: a print at every bound, pointing on; in from the reeds
// to the first flag, and out from the last to the water.
let prints = new_path();
let L = TRAIL.len();
for k in 0..L - 1 {
	let a = centre_of.call(tour[k]);
	let b = centre_of.call(tour[k + 1]);
	prints.append(pawprint.call((a + b) / 2.0, (b - a).normalized(), 32.0));
}
let first = centre_of.call(tour[0]);
let enter = (first - B).normalized();
prints.append(pawprint.call(first + enter * 92.0, -enter, 32.0));
prints.append(pawprint.call(first + enter * 160.0, -enter, 32.0));
let last = centre_of.call(tour[L - 1]);
let to_water = vec2(last.x + 40.0, WATER_Y + 20.0);
let out = (to_water - last).normalized();
let steps = ((to_water - last).length() / 66.0).to_int();
for k in 1..steps {
	prints.append(pawprint.call(last + out * (k * 66.0), out, 32.0));
}
engraving::mass(scored, prints, new_path(), INK, 0.9);
leaf.figures.add(scored);

// The water, come up over the court's foot, black.
let edge = shapes::steps(24, |j| {
	let x = 1200.0 - j * 50.0;
	vec2(x, WATER_Y + 14.0 * (x * 0.013).sin() + 8.0 * chance::either_way(65, j, 1))
});
let water = shapes::closed(edge + [vec2(0, 1340), vec2(1200, 1340)]);
let wet = new_canvas();
let pale = shapes::join([[300.0, 1260.0, 70.0, 1.5, 0.4], [880.0, 1240.0, 90.0, 1.8, 2.2]].map(|w| {
	shapes::sliver(shapes::whorl_points(vec2(w[0], w[1]), w[2] * 0.1, w[2], w[3], 0.3, w[4]), 3.6)
}));
for r in [30.0, 54.0, 82.0] {
	pale.append(shapes::sliver(shapes::arc_points(vec2(to_water.x, WATER_Y + 24), r, 0.1, PI() - 0.1, 24).map(|p| vec2(p.x, WATER_Y + 24 + (p.y - WATER_Y - 24) * 0.25)), 3.0));
}
engraving::black_water(wet, water, pale, INK, #{ gap: 2.8, weight: 2.0, seed: 61 });
let rushes = new_canvas();
engraving::mass(rushes, gothic::reeds(WATER_Y + 6, 20, 260, 170, 24, 61).append(gothic::reeds(WATER_Y + 4, 980, 1190, 180, 22, 62)), new_path(), INK, 1.2);
leaf.figures.add(wet).add(rushes);

leaves::jot(leaf, "In from the reeds to the first flag, every bound a knight's move, and out into the water. Read the flags in the order they were crossed.", vec2(70, 1380), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the game-book ----

let leaf = mb::leaf("tally", 63, #{ foxing: 130, stains: 2 });
let book = new_canvas();
book.add(leaves::centred("THE GAME-BOOK", vec2(540, 104), leaves::title_hand(46.0, INK)));
// The ledger: night, what was taken, the tally.
let TOP = 170.0;
let ROW = 60.0;
let cols = [100.0, 320.0, 860.0, 1100.0];
let ruled = shapes::join(shapes::steps(8, |r| new_line(vec2(cols[0], TOP + r * ROW), vec2(cols[3], TOP + r * ROW))));
book.add(ruled, inks::detail(INK, 1.0));
for x in cols { book.add(new_line(vec2(x, TOP), vec2(x, TOP + 8.0 * ROW)), inks::detail(INK, 1.0)); }
book.add(new_line(vec2(cols[0], TOP + ROW - 4), vec2(cols[3], TOP + ROW - 4)), inks::contour(BLOOD, 1.2));
for (h, k) in ["NIGHT", "TAKEN", "TALLY"] {
	book.add(leaves::centred(h, vec2((cols[k] + cols[k + 1]) / 2.0, TOP + ROW / 2.0), leaves::figures_hand(22.0, INK)));
}
let nights = ["OCT 2", "OCT 19", "NOV 1", "NOV 13", "NOV 30", "DEC 21", "JAN 6"];
for (night, r) in nights {
	let mid_y = TOP + (r + 1.5) * ROW;
	book.add(leaves::centred(night, vec2((cols[0] + cols[1]) / 2.0, mid_y), leaves::figures_hand(24.0, INK)));
	for q in 0..4 {
		let n = numbers[(r * 4 + q) * 2] * 10 + numbers[(r * 4 + q) * 2 + 1];
		let text = if n < 10 { `0${n}` } else { n.to_string() };
		book.add(leaves::centred(text, vec2(cols[1] + 70.0 + q * 128.0, mid_y), leaves::figures_hand(30.0, BLOOD)));
	}
	// The tally: a stroke for each, struck through.
	let marks = shapes::join(shapes::times(4, |t| new_line(vec2(cols[2] + 40 + t * 16, mid_y - 16), vec2(cols[2] + 38 + t * 16, mid_y + 16))));
	marks.append(new_line(vec2(cols[2] + 30, mid_y + 12), vec2(cols[2] + 106, mid_y - 12)));
	book.add(marks, inks::contour(INK, 1.6));
}
leaves::place(leaf, book, 10);

// The roll: each name and its numbers, three columns down the page.
let rolled = new_canvas();
rolled.add(leaves::centred("THE ROLL", vec2(600, 720), leaves::title_hand(36.0, INK)));
let names = "-ABCDEFGHIJKLMNOPQRSTUVWXYZ";
for (list, s) in roll {
	let col = s / 9;
	let row = s % 9;
	let at = vec2(90.0 + col * 360.0, 780.0 + row * 62.0);
	rolled.add(leaves::centred(names.sub_string(s, 1), at + vec2(12, 14), leaves::figures_hand(26.0, INK)));
	let said = list.map(|n| if n < 10 { `0${n}` } else { n.to_string() });
	let line = "";
	for (t, k) in said { line += if k == 0 { t } else { " " + t }; }
	rolled.add(write_block(line, at + vec2(36, 0), 300, leaves::figures_hand(16.0, INK)).canvas);
}
let roll_rules = new_path();
for c in 1..3 { roll_rules.append(new_line(vec2(80.0 + c * 360.0, 770), vec2(80.0 + c * 360.0, 1330))); }
rolled.add(roll_rules, inks::detail(INK, 0.9));
leaves::place(leaf, rolled, 8);

// A knife of black glass left lying across the book's foot.
let knife = gothic::glass_knife(vec2(760, 1350), vec2(1110, 1270), 56.0, 120.0, 11, INK);
leaves::place(leaf, knife.canvas, 8);

leaves::jot(leaf, "Each name has many numbers, as many as it is common; find each in the roll.", vec2(70, 1430), 1060, #{ size: 27, ink: BLOOD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, content("mere"), #{ size: 20.0 }));
