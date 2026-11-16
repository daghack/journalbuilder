// The threshold: what knocks. On the left, the thirteen who watch, in a
// ring as the almanac sets them, and the shapes they make in threes, the
// 26 triples of the Steiner triple system S(2, 3, 13): a letter each, the
// key to the knocking. On the right, her own door the night it came to
// it: knocks of three scored into the wood from the other side
// (steiner_triples, points 3), each turned by the key the watchers' night
// gives (steiner_cipher, key the alignment's year and month, L C), and
// light under the door, and in the light two feet.

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

seed(2401);

// ---- the secret (chains/door_threshold) ----

let secrets = chain("door_threshold").run();
let scored = secrets.get("knocks.cipher_text").value();
let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
let every = secrets.get("asterisms.points");

// Which way place p of thirteen is from a ring's middle, clockwise from
// the top, as steiner_wheels sets them.
let toward = |p| {
	let a = -PI() / 2.0 + 2.0 * PI() * p / 13.0;
	shapes::polar(a)
};

// ---- the left page: the thirteen ----

let leaf = nb::leaf("thirteen", 101, #{ foxing: 120, stains: 1 });
let O = vec2(600, 536);
let R = 270.0;

let ring = new_canvas();
// Every triple, faintly: the web the thirteen are caught in.
let web = new_path();
for k in 0..13 {
	for t in [[0, 1, 4], [0, 2, 7]] {
		let corners = [];
		for d in t { corners.push(O + toward.call((k + d) % 13) * R); }
		web.append(new_polyline(corners, #{ closed: true }));
	}
}
let stars = shapes::join(shapes::times(13, |p| new_circle(O + toward.call(p) * R, 40)));
ring.add(web.erase_inside(stars), inks::fine(SEPIA, 0.8));
ring.add(new_circle(O, R + 70), inks::contour(INK, 1.8));
ring.add(new_circle(O, R + 80), inks::detail(INK, 1.0));
ring.add(new_circle(O, R - 62).erase_inside(stars), inks::detail(INK, 1.0));
let degrees = shapes::join(shapes::times(104, |j| {
	let a = 2.0 * PI() * j / 104.0;
	let way = shapes::polar(a);
	new_line(O + way * (R + 70), O + way * (R + (if j % 8 == 0 { 58.0 } else { 64.0 })))
}));
ring.add(degrees, inks::detail(INK, 1.0));
// The thirteen themselves, each with its number in the priests' signs.
let priests = symbol_set("priests", #{ color: INK, size: 34.0 });
for p in 0..13 {
	let at = O + toward.call(p) * R;
	ring.add(gothic::watcher(at, 22.0, 60 + p * 5, INK, #{ reach: 1.8 }));
	ring.add(write_at(priests.text([p]), O + toward.call(p) * (R + 106), #{}));
}
// Where they look: all of them, at the middle.
ring.add(gothic::watcher(O, 30.0, 7, INK, #{ kind: "banded", reach: 2.4 }));
leaves::place(leaf, ring, 10);

let title = write_line("THE THIRTEEN WHO WATCH", vec2(0, 0), leaves::title_hand(44.0, INK));
let tb = title.bounds();
leaf.figures.add(title.translate_by(vec2(600, 96) - tb.center));
leaves::clear(leaf, new_rectangle(vec2(600, 96), tb.max.x - tb.min.x + 40, 80), 4);

// The shapes in threes, a letter each, on a plate.
let plate = new_canvas();
let key = illustration("steiner_wheels", #{ per_row: 9, size: 76.0, gap: 26.0, row_gap: 10.0, label_size: 25.0 })
	.run(#{ points: every, labels: alphabet, ring_style: #{ style: inks::contour(INK, 1.3) }, line_style: #{ style: inks::contour(COLD, 1.7) }, label_style: leaves::figures_hand(25.0, INK) })[0];
let kb = key.bounds();
let mid = vec2(600, 1170);
plate.add(key.translate_by(mid - kb.center));
plate.add(new_rectangle(mid, 960, 400), inks::contour(INK, 2.0));
plate.add(new_rectangle(mid, 944, 384), inks::detail(INK, 1.0));
for sx in [-1.0, 1.0] {
	for sy in [-1.0, 1.0] { plate.add(gothic::watcher(mid + vec2(sx * 480, sy * 200), 9.0, 30 + (sx + sy * 2.0).to_int(), INK, #{ kind: "needle", reach: 2.6 })); }
}
leaves::place(leaf, plate, 10);

leaves::jot(leaf, "The thirteen who watch, and the shapes they make in threes, a letter to each, as the almanac has them.", vec2(80, 1394), 1040, #{ size: 27, turn: 0.004 });
leaves::jot(leaf, "Three knocks are three of them, and three of them are a letter.", vec2(80, 1482), 1040, #{ size: 27, ink: COLD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, content("nights"), #{ size: 20.0 }));

// ---- the right page: the knocking ----

let leaf = nb::leaf("knocking", 103, #{ foxing: 110, stains: 1 });
let DOOR = [vec2(160, 20), vec2(1040, 1044)];
let SILL = 1078.0;
let VP = vec2(600, 560);

// What was scored into it: a ring to each knock of three, ticked for the
// thirteen, and the three it knocked joined, all cut pale out of the
// black.
let centres = shapes::times(scored.len() / 3, |k| {
	let row = k / 5;
	let col = k % 5;
	vec2(600.0 + (col - 2) * 170.0 + 10.0 * chance::either_way(103, k, 1), 190 + row * 210 + 8.0 * chance::either_way(103, k, 2))
});
let outer = new_path();
let inner = new_path();
let cuts = new_path();
for (at, k) in centres {
	let r = 46.0;
	outer.append(new_circle(at, r));
	inner.append(new_circle(at, r - 4.5));
	for p in 0..13 {
		let way = toward.call(p);
		let long = if p == 0 { 1.42 } else { 1.22 };
		cuts.append(shapes::sliver([at + way * (r - 1), at + way * (r * (1.0 + long) / 2.0), at + way * r * long], 3.2));
	}
	let places = [];
	for p in scored.extract(k * 3, 3) {
		if !(p in places) { places.push(p); }
	}
	let corners = [];
	for p in places { corners.push(at + toward.call(p) * (r - 4.5)); }
	for c in corners { cuts.append(new_circle(c, 7.5)); }
	if corners.len() > 1 {
		for j in 0..corners.len() {
			if corners.len() == 2 && j == 1 { break; }
			let a = corners[j];
			let b = corners[(j + 1) % corners.len()];
			cuts.append(shapes::sliver([a, (a + b) / 2.0, b], 5.2));
		}
	}
}
// And low down, where a hand would be, the marks of nails.
for g in [[300.0, 862.0, 4, 0.14], [846.0, 880.0, 3, -0.1]] {
	for n in 0..g[2] {
		let top = vec2(g[0] + n * 26.0 + 4.0 * chance::either_way(105, n, 1), g[1] + 14.0 * chance::either_way(105, n, 2) + abs(n - 1.5) * 10.0);
		let way = vec2(g[3], 1.0).normalized();
		let len = 150.0 + 30.0 * chance::either_way(105, n, 3);
		cuts.append(shapes::sliver([top, top + way * len * 0.5 + vec2(4.0, 0.0), top + way * len], 6.5));
	}
}
let door = shapes::closed([DOOR[0], vec2(DOOR[1].x, DOOR[0].y), DOOR[1], vec2(DOOR[0].x, DOOR[1].y)]);
// The door's own edges: a bevel all round, and its two leaves' meeting.
let bevel = new_path();
bevel.append(new_rectangle(vec2(600, DOOR[0].y + 30), 820, 3)).append(new_rectangle(vec2(600, DOOR[1].y - 30), 820, 3));
bevel.append(new_rectangle(vec2(DOOR[0].x + 30, 540), 3, 980)).append(new_rectangle(vec2(DOOR[1].x - 30, 540), 3, 980));
let gap = 2.0;
// (Erasing changes a path in place: each part hatched afresh.)
let grain = || door.hatch(gap, #{ angle: [0.6, 0.6 + PI() / 2.0], roughness: gap * 0.4, fill_rule: "nonzero" });
let black = new_style(new_brush_style(INK, "Rough Ink", gap / 2.5, #{ min_pressure: 0.8, max_pressure: 0.95, pressure_profile: "constant", color_jitter: 0.04 }), new_handwriting_style());
let wood = new_canvas();
wood.add(grain.call().erase_inside(outer, #{ fill_rule: "nonzero" }).erase_inside(cuts, #{ fill_rule: "nonzero" }).erase_inside(bevel), black);
wood.add(grain.call().erase_outside(inner, #{ fill_rule: "nonzero" }).erase_inside(cuts, #{ fill_rule: "nonzero" }), black);
// The frame round it and the wall either side.
let wall = new_canvas();
let jambs = new_path().append(new_rectangle(vec2(130, 540), 44, 1080)).append(new_rectangle(vec2(1070, 540), 44, 1080));
wall.add(jambs, inks::contour(INK, 1.8));
wall.add(jambs.hatch(3.0, #{ angle: [1.1] }), inks::detail(INK, 0.8));
for x in [40.0, 1110.0] { wall.add(new_rectangle(vec2(x + 25, 540), 50, 1080).hatch(4.0, #{ angle: [0.0], roughness: 1.0 }), inks::fine(SEPIA, 0.8)); }
leaf.figures.add(wall).add(wood);

// Under it, the light, and in the light two feet: whoever stands behind.
let under = new_canvas();
let feet = [[512.0, 584.0], [636.0, 706.0]];
for f in feet {
	engraving::mass(under, shapes::closed([vec2(f[0], DOOR[1].y), vec2(f[1], DOOR[1].y), vec2(f[1] - 4, SILL), vec2(f[0] + 4, SILL)]), new_path(), INK, 1.4);
}
under.add(new_line(vec2(DOOR[0].x, SILL), vec2(DOOR[1].x, SILL)), inks::contour(INK, 2.2));
under.add(new_line(vec2(110, SILL + 14), vec2(1090, SILL + 14)), inks::contour(INK, 1.4));
// The floor in front, dark but for where the light falls out, and the
// feet's shadows long across it.
let said = leaves::note("The night it came to my own door. Twenty knocks of three, and every one scored into the wood from the other side.", vec2(80, 1380), 1040, #{ ink: INK, size: 27, turn: 0.004 });
let warned = leaves::note("Undo each by the watchers' night, its year and its month by turns: a star and its key make the third.", vec2(80, 1468), 1040, #{ size: 27, ink: COLD, turn: -0.004 });
let notes = new_path().append(said.hole).append(warned.hole);
let edge = [vec2(-20, SILL + 14), vec2(1220, SILL + 14)];
for j in 0..=24 {
	let x = 1220.0 - j * 1240.0 / 24.0;
		let off = (x - 600.0) / 620.0;
	edge.push(vec2(x, 1350.0 + 26.0 * chance::either_way(104, j, 1) - 60.0 * off * off));
}
let floor = shapes::closed(edge);
let fan = shapes::shape([vec2(DOOR[0].x, SILL + 14), vec2(DOOR[1].x, SILL + 14), vec2(1220, 1560), vec2(-20, 1560)]);
let boards = new_path();
for j in -10..26 {
	let foot = vec2(-900 + j * 110, 1800);
	let t = (SILL + 14 - VP.y) / (foot.y - VP.y);
	boards.append(new_line(VP + (foot - VP) * t, foot));
}
under.add(boards.erase_outside(floor).erase_inside(notes), inks::detail(INK, 1.0));
let shadows = shapes::join(feet.map(|f| {
	shapes::closed([vec2(f[0] + 4, SILL + 14), vec2(f[1] - 4, SILL + 14), vec2(600 + (f[1] - 600) * 2.4, 1560), vec2(600 + (f[0] - 600) * 2.4, 1560)])
}));
under.add(floor.hatch(4.4, #{ angle: [0.02], roughness: 1.0 }).erase_inside(fan).erase_inside(notes), inks::fine(INK, 0.8));
under.add(floor.hatch(3.0, #{ angle: [0.02, 2.4], roughness: 1.0 }).erase_outside(shadows, #{ fill_rule: "nonzero" }).erase_inside(notes), inks::detail(INK, 0.9));
leaf.figures.add(under);
leaf.figures.add(said.canvas).add(warned.canvas);

journal_entry.add(leaves::bind(leaf, ""));
