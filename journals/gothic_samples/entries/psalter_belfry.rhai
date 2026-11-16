// The belfry: the last of his secrets, rung. On the left, the tenor,
// Great Ysolt, hanging in her wheel, and under her the board of the
// ringing chamber: the 26 triples of thirteen bells (the Steiner triple
// system S(2, 3, 13)), a letter each, the key to the ringing. On the
// right, the ropes, and what the bells rang the night after she walked,
// as he took it down three bells at a time (steiner_triples, points 3),
// each bell then turned by the key the stars give (steiner_cipher, key
// the alignment's year and month, J E): to read it, turn each bell back
// by the same key and find its triangle on the board.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/psalter" as ps;

const INK = ps::ink();
const BLOOD = ps::blood();
const SEPIA = ps::sepia();

seed(1601);

// ---- the secret (chains/psalter_belfry) ----

let rung = chain("psalter_belfry").run();
let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
let every = rung.get("board.points");

let rings_pen = #{ style: inks::contour(INK, 1.3) };
let triangle_pen = #{ style: inks::contour(BLOOD, 2.2) };

// ---- the left page: the tenor ----

let leaf = ps::leaf("tenor", 61, #{ foxing: 120, stains: 2 });
let C = 600.0;
let AXLE = vec2(C, 445);
let RW = 330.0;
let TOP = 495.0;
let BR = 250.0;
let BH = 430.0;
let LIP = TOP + BH;
let MY = 30.0;
let FLOOR = 1000.0;
let on_bell = |u, v| vec2(C + u * BR, TOP + v * BH);

// Her profile, from the crown down to the lip, as the founders turn it.
let profile = [[0.34, 0.0], [0.45, 0.025], [0.51, 0.08], [0.535, 0.2], [0.555, 0.36], [0.6, 0.52], [0.68, 0.66], [0.8, 0.78], [0.93, 0.88], [1.0, 0.95], [0.985, 1.0]];
let width_at = |v| {
	let u = profile[0][0];
	for k in 1..profile.len() {
		let a = profile[k - 1];
		let b = profile[k];
		if v >= a[1] && v <= b[1] { u = a[0] + (b[0] - a[0]) * (v - a[1]) / (b[1] - a[1]); }
	}
	u
};
let right = [];
for p in profile { right.push(on_bell.call(p[0], p[1])); }
right = shapes::curve(right, 6);
let outline = [];
for p in right { outline.push(p); }
for j in 1..16 {
	let a = PI() * j / 16.0;
	outline.push(vec2(C + 0.985 * BR * a.cos(), LIP + MY * a.sin()));
}
for k in 0..right.len() {
	let p = right[right.len() - 1 - k];
	outline.push(vec2(2.0 * C - p.x, p.y));
}
let bell = shapes::closed(outline);

// A band about her, as seen from under: an arc bowed up, v0 to v1.
let band = |v0, v1| {
	let top = [];
	let bottom = [];
	let w0 = width_at.call(v0) * BR - 6.0;
	let w1 = width_at.call(v1) * BR - 6.0;
	for j in 0..=12 {
		let f = j / 6.0 - 1.0;
		top.push(vec2(C + f * w0, TOP + v0 * BH - 10.0 * (1.0 - f * f)));
		bottom.push(vec2(C - f * w1, TOP + v1 * BH - 10.0 * (1.0 - f * f)));
	}
	shapes::shape(top + bottom)
};

// The headstock, the beam she's bolted to, its gudgeons on the frame.
let headstock = new_path();
headstock.append(shapes::closed([vec2(180, AXLE.y - 22), vec2(230, AXLE.y - 30), vec2(C + 370, AXLE.y - 30), vec2(1020, AXLE.y - 22), vec2(1020, AXLE.y + 22), vec2(C + 370, AXLE.y + 28), vec2(230, AXLE.y + 28), vec2(180, AXLE.y + 22)]));
// The canons, her crown, between beam and bell.
headstock.append(shapes::closed([vec2(C - 70, AXLE.y + 20), vec2(C + 70, AXLE.y + 20), vec2(C + 84, TOP + 6), vec2(C - 84, TOP + 6)]));
let frame = new_path();
for side in [-1.0, 1.0] {
	let x = C + side * 420;
	frame.append(shapes::closed([vec2(x - 26, AXLE.y - 70), vec2(x + 26, AXLE.y - 70), vec2(x + 26, FLOOR), vec2(x - 26, FLOOR)]));
	frame.append(shapes::closed([vec2(x - side * 26, 640), vec2(x - side * 26, 600), vec2(C + side * 250, FLOOR), vec2(C + side * 212, FLOOR)]));
}
frame.append(shapes::closed([vec2(110, FLOOR), vec2(1090, FLOOR), vec2(1090, FLOOR + 42), vec2(110, FLOOR + 42)]));

// What stands in front: the wheel's lines are cut away behind it.
let front = new_path().append(bell).append(headstock).append(frame);
let behind = |p| p.erase_inside(front, #{ fill_rule: "nonzero" });

// The wheel the rope turns her by.
let wheel = new_canvas();
let rim = shapes::join([RW, RW - 6.0, RW - 30.0, RW - 36.0].map(|r| new_circle(AXLE, r)));
let rim_shade = new_circle(AXLE, RW - 6.0).hatch(5.0, #{ angle: 0.5 }).erase_inside(new_circle(AXLE, RW - 30.0));
let spokes = new_path();
for k in 0..6 {
	let a = PI() / 6.0 + PI() * k / 3.0;
	let way = shapes::polar(a);
	let across = vec2(-a.sin(), a.cos());
	for s in [-1.0, 1.0] {
		spokes.append(new_line(AXLE + way * 40 + across * s * 11, AXLE + way * (RW - 36) + across * s * 7));
	}
}
spokes = spokes.erase_inside(new_circle(AXLE, 44));
rim.append(new_circle(AXLE, 44)).append(new_circle(AXLE, 36));
wheel.add(behind.call(rim_shade), inks::pencil(SEPIA, 1.0));
wheel.add(behind.call(rim), inks::contour(INK, 2.0));
wheel.add(behind.call(spokes), inks::contour(INK, 1.8));
leaves::place(leaf, wheel, 8);

// The rope, off the rim and down through the floor.
let rope = new_canvas();
let rx = C + RW - 3.0;
let cord = new_path().append(new_line(vec2(rx - 5, AXLE.y), vec2(rx - 5, FLOOR))).append(new_line(vec2(rx + 5, AXLE.y), vec2(rx + 5, FLOOR)));
let twist = new_path();
let y = AXLE.y + 6.0;
while y < FLOOR - 6.0 { twist.append(new_line(vec2(rx - 5, y + 6), vec2(rx + 5, y))); y += 9.0; }
rope.add(behind.call(cord), inks::contour(INK, 2.0)).add(behind.call(twist), inks::contour(INK, 1.4));
leaves::place(leaf, rope, 6);

// Her, dark, and the light on her: the near lip, the bands with her
// words, where the light runs down her side, and the crack.
let light = new_path();
let lip = shapes::steps(24, |j| {
	let a = PI() + PI() * j / 24.0;
	vec2(C + 0.97 * BR * a.cos(), LIP + 2.0 + MY * 0.85 * a.sin())
});
light.append(shapes::sliver(lip, 5.0));
let upper_band = band.call(0.13, 0.24);
let lower_band = band.call(0.76, 0.84);
light.append(upper_band).append(lower_band);
for fw in [[0.72, 7.0], [0.6, 3.5], [-0.82, 3.0]] {
	let run = shapes::steps(8, |j| {
		let v = 0.3 + 0.62 * j / 8.0;
		on_bell.call(-fw[0] * width_at.call(v), v)
	});
	light.append(shapes::sliver(run, fw[1]));
}
let crack = [];
for p in [[0.42, 0.985], [0.4, 0.9], [0.45, 0.83], [0.39, 0.75], [0.42, 0.68], [0.37, 0.6], [0.39, 0.56]] { crack.push(on_bell.call(p[0], p[1])); }
light.append(shapes::sliver(crack, 4.0));
// The clapper's ball, in the dark of her mouth.
let ball = vec2(C + 24, LIP + 10);
light.append(new_circle(ball, 27).erase_inside(new_circle(ball, 23)));
// Light on the timber: the grain, the gudgeons, the straps on the crown.
for (y, k) in [AXLE.y - 12, AXLE.y + 2, AXLE.y + 14] {
	light.append(shapes::sliver([vec2(250 + k * 40, y), vec2(450, y + 1), vec2(760, y - 1), vec2(960 - k * 30, y)], 2.6));
}
for side in [-1.0, 1.0] {
	light.append(new_circle(vec2(C + side * 420, AXLE.y), 13));
	light.append(shapes::sliver([vec2(C + side * 50, AXLE.y - 26), vec2(C + side * 52, AXLE.y + 20), vec2(C + side * 56, TOP + 4)], 5.0));
	let x = C + side * 420;
	light.append(shapes::sliver([vec2(x + 10, AXLE.y + 40), vec2(x + 12, 760), vec2(x + 9, FLOOR - 20)], 3.0));
}
light.append(shapes::sliver([vec2(130, FLOOR + 14), vec2(600, FLOOR + 16), vec2(1070, FLOOR + 13)], 2.6));
let tenor = new_canvas();
engraving::mass(tenor, front, light, INK, 2.0);
// Her words: VIVOS VOCO, MORTUOS PLANGO (I call the living, I mourn the
// dead).
for said in [["MORTUOS PLANGO", 0.185, 24.0], ["VIVOS VOCO", 0.80, 26.0]] {
	tenor.add(leaves::centred(said[0], vec2(C, TOP + said[1] * BH - 8.0), leaves::carved_hand(said[2], INK)));
}
for x in [-1.0, 1.0] {
	for v in [0.185, 0.80] {
		let w = width_at.call(v) * BR - 20.0;
		tenor.add(new_circle(vec2(C + x * w, TOP + v * BH - 2.0), 4), inks::fill(INK, 2.0));
	}
}
leaves::place(leaf, tenor, 10);

// The board in the ringing chamber: the 26 triples, a letter each.
let board = new_canvas();
let key = illustration("steiner_wheels", #{ per_row: 9, size: 74.0, gap: 26.0, row_gap: 10.0, label_size: 25.0 })
	.run(#{ points: every, labels: alphabet, ring_style: rings_pen, line_style: triangle_pen, label_style: leaves::figures_hand(26.0, INK) })[0];
let kb = key.bounds();
let board_box = [vec2(110, 1060), vec2(1090, 1470)];
let mid = (board_box[0] + board_box[1]) / 2.0;
board.add(key.translate_by(mid + vec2(0, 4) - kb.center));
let outer = new_rectangle(mid, 980, 410);
let inner = new_rectangle(mid, 944, 374);
let timber = new_canvas();
engraving::mass(timber, outer, new_path().append(inner), INK, 2.0);
board.add(timber);
board.add(new_rectangle(mid, 932, 362), inks::detail(INK, 1.2));
for sx in [-1.0, 1.0] {
	for sy in [-1.0, 1.0] {
		let corner = mid + vec2(sx * 481, sy * 196);
		board.add(new_circle(corner, 5), inks::fill("#e8dcc0", 2.0));
	}
}
leaves::place(leaf, board, 10);

leaves::jot(leaf, "The tenor, Great Ysolt. Cast the year they drowned her, and her name cast on her.", vec2(76, 60), 330, #{ size: 27, turn: -0.01 });
leaves::jot(leaf, "Each letter, three bells rung at once. So the board in the chamber.", vec2(830, 70), 300, #{ size: 27, ink: BLOOD, turn: 0.015 });

journal_entry.add(leaves::bind(leaf, content("psalter"), #{ size: 20.0 }));

// ---- the right page: the ringing ----

let leaf = ps::leaf("ringing", 67, #{ foxing: 130 });

// The chamber's ceiling, the ropes through it.
let ceiling = new_canvas();
let beam = shapes::closed([vec2(30, 60), vec2(1170, 60), vec2(1170, 140), vec2(30, 140)]);
let grain = shapes::join([80.0, 98.0, 118.0].map(|y, k| {
	shapes::sliver([vec2(60 + k * 70, y), vec2(400, y + 2), vec2(800, y - 1), vec2(1140 - k * 50, y + 1)], 2.6)
}));
let xs = [175.0, 345.0, 515.0, 685.0, 855.0, 1025.0];
for x in xs { grain.append(new_circle(vec2(x, 132), 16).erase_inside(new_circle(vec2(x, 132), 11))); }
engraving::mass(ceiling, beam, grain, INK, 2.0);
leaves::place(leaf, ceiling, 10);

// Six ropes, their sallies caught at every height, as if hands were on
// them.
let tops = [430.0, 600.0, 470.0, 560.0, 410.0, 520.0];
let sways = [8.0, -12.0, 5.0, 14.0, -6.0, 10.0];
for (x, k) in xs {
	let ropes = new_canvas();
	let s = tops[k];
	let lean = sways[k];
	let line = shapes::curve([vec2(x, 140), vec2(x + lean * 0.4, (140 + s) / 2.0), vec2(x + lean, s)], 8);
	let cord = new_path().append(new_polyline(line));
	let ticks = new_path();
	for j in 1..line.len() {
		if j % 2 == 0 {
			let p = line[j];
			ticks.append(new_line(p + vec2(-4, 4), p + vec2(4, -2)));
		}
	}
	ropes.add(new_path().append(cord.copy().translate_by(vec2(-3.5, 0))).append(cord.copy().translate_by(vec2(3.5, 0))), inks::contour(INK, 1.3)).add(ticks, inks::detail(INK, 0.9));
	// The sally, striped wool.
	let foot = vec2(x + lean, s);
	let sally_pts = [foot, foot + vec2(lean * 0.07, 40), foot + vec2(lean * 0.14, 80), foot + vec2(lean * 0.2, 120), foot + vec2(lean * 0.3, 160)];
	let sally = shapes::shape(shapes::tube(sally_pts, [16.0, 42.0, 44.0, 40.0, 18.0]));
	let stripes = shapes::join(shapes::times(6, |j| {
		let y0 = s + 18.0 + j * 27.0;
		shapes::shape([vec2(x - 40, y0), vec2(x + 40, y0 - 12), vec2(x + 40, y0 + 1), vec2(x - 40, y0 + 13)])
	}));
	engraving::mass(ropes, sally, stripes, INK, 1.8);
	ropes.add(stripes.hatch(4.0, #{ angle: 0.8 }).erase_outside(sally), inks::pencil(SEPIA, 1.0));
	// The tail end, looped up and caught.
	let tail = foot + vec2(lean * 0.3, 158);
	let tail_line = shapes::curve([tail, tail + vec2(lean * 0.1, 50), tail + vec2(lean * 0.1 + 20, 74), tail + vec2(lean * 0.1 + 30, 40), tail + vec2(lean * 0.1 + 10, 4)], 8);
	let tail_cord = new_polyline(tail_line);
	ropes.add(new_path().append(tail_cord.copy().translate_by(vec2(-3, 0))).append(tail_cord.copy().translate_by(vec2(3, 0))), inks::contour(INK, 1.2));
	leaves::place(leaf, ropes, 8);
	// The ones swinging, in pencil.
	if k % 2 == 1 {
		for d in [18.0, 30.0] {
			leaf.under.add(new_polyline(shapes::arc_points(vec2(x + lean, 140), s - 140 + 60, PI() / 2.0 + 0.05, PI() / 2.0 + 0.16, 8)).translate_by(vec2(d, 0)), inks::pencil(SEPIA, 1.0));
		}
	}
}

// A lantern on a chain from the beam, between the ropes, guttering.
let lamp = new_canvas();
let hook = vec2(600, 140);
let links = new_path();
let y = 146.0;
while y < 300.0 { links.append(new_ellipse(vec2(600, y + 6), 4, 7)); y += 12.0; }
lamp.add(links, inks::contour(INK, 1.3));
let cage = new_path();
cage.append(shapes::closed([vec2(570, 312), vec2(600, 296), vec2(630, 312), vec2(636, 324), vec2(564, 324)]));
cage.append(shapes::closed([vec2(566, 324), vec2(634, 324), vec2(626, 420), vec2(574, 420)]));
cage.append(shapes::closed([vec2(564, 420), vec2(636, 420), vec2(628, 434), vec2(572, 434)]));
let panes = new_path();
panes.append(shapes::closed([vec2(577, 332), vec2(596, 332), vec2(594, 412), vec2(581, 412)]));
panes.append(shapes::closed([vec2(604, 332), vec2(623, 332), vec2(619, 412), vec2(606, 412)]));
engraving::mass(lamp, cage, panes, INK, 1.8);
let flame = shapes::shape(shapes::tube([vec2(600, 404), vec2(601, 388), vec2(598, 372), vec2(602, 358)], [7.0, 10.0, 6.0, 1.0]));
lamp.add(flame, inks::contour(INK, 1.0));
lamp.add(new_line(vec2(600, 405), vec2(600, 412)), inks::contour(INK, 2.6));
for k in 0..14 {
	let a = 2.0 * PI() * k / 14.0;
	let way = shapes::polar(a);
	lamp.add(new_line(vec2(600, 372) + way * 54, vec2(600, 372) + way * 70), inks::pencil(SEPIA, 1.0));
}
leaves::place(leaf, lamp, 10);

// What they rang, three at a time, as he took it down.
let night = illustration("steiner_wheels", #{ per_row: 6, size: 132.0, gap: 40.0, row_gap: 26.0 })
	.run(#{ points: rung.get("bells.cipher_text"), ring_style: rings_pen, line_style: triangle_pen })[0];
let nb = night.bounds();
leaves::place(leaf, night.translate_by(vec2(600, 1110) - nb.center), 14);

leaves::jot(leaf, "The ringing chamber, the night after she walked. No hand on them. I took them down three at a time, as they fell.", vec2(80, 1340), 1040, #{ size: 28, turn: 0.005 });
leaves::jot(leaf, "Undo each by the night of the stars, its year and its month by turns: a bell and its key make the third.", vec2(80, 1436), 1040, #{ size: 28, ink: BLOOD, turn: -0.005 });

journal_entry.add(leaves::bind(leaf, content("psalter"), #{ size: 20.0 }));
