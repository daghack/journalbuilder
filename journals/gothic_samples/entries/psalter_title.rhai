// The title: the psalter's frontispiece. A pointed arch of three orders,
// crocketed, a bramble climbing its jambs; inside, the title in
// blackletter, a skull over crossed bones, MEMENTO MORI on a ribbon, and
// whose book it is. His account walls the leaf outside the arch.

import "lib/inks" as inks;
import "lib/shapes" as shapes;
import "lib/leaves" as leaves;
import "lib/engraving" as engraving;
import "lib/psalter" as ps;

const INK = ps::ink();
const BLOOD = ps::blood();
const SEPIA = ps::sepia();

seed(1101);

let leaf = ps::leaf("title", 11, #{ stains: 2, foxing: 120 });
let C = 600.0;
let SPRING = 760.0;
let FOOT = 1440.0;
let left = vec2(170, SPRING);
let right = vec2(1030, SPRING);
let SHARP = 0.62;

// `pts` back to front, as a new array.
let backward = |pts| { let r = []; for k in 0..pts.len() { r.push(pts[pts.len() - 1 - k]); } r };

// An order of the arch, `d` inside it, run down its jambs to the foot.
let order = |d| {
	let pts = shapes::arch_order(left, right, SHARP, d, 28);
	[vec2(pts[0].x, FOOT)] + pts + [vec2(pts[pts.len() - 1].x, FOOT)]
};

// ---- the arch ----

let arch = new_canvas();
let outer = order.call(-44.0);
let hollow = shapes::closed(order.call(-30.0) + backward.call(shapes::arch_order(left, right, SHARP, -14.0, 28)));
// Dog-tooth in the hollow: little four-leaved stars left bare.
let teeth = new_path();
let along = order.call(-22.0);
for at in new_polyline(along).points_along(48) {
	teeth.append(shapes::shape(shapes::foil_points(at.position, 6.5, 4, 0.55, PI() / 4.0)));
}
engraving::mass(arch, hollow, teeth, INK, 1.8);
arch.add(new_polyline(outer), inks::contour(INK, 2.4));
arch.add(new_polyline(order.call(-38.0)), inks::detail(INK, 1.0));
arch.add(new_polyline(order.call(-6.0)), inks::detail(INK, 1.1));
arch.add(new_polyline(order.call(0.0)), inks::contour(INK, 1.8));
// The outer moulding shaded.
let band = shapes::shape(order.call(-44.0) + backward.call(order.call(-30.0)));
arch.add(band.hatch(4.0, #{ angle: [0.9], roughness: 1.0 }), inks::fine(SEPIA, 0.8));

// Crockets up the outer order: leaves curling out from the stone.
let rim = new_polyline(shapes::arch_order(left, right, SHARP, -44.0, 28));
let crockets = new_path();
let along_rim = rim.points_along(13);
for (p, k) in along_rim {
	if k == 0 || k == 6 || k == 12 { continue; }
	let at = p.position;
	let tangent = shapes::polar(p.angle);
	let outward = vec2(tangent.y, -tangent.x);
	crockets.append(shapes::shape(shapes::tube([at, at + outward * 16 + tangent * 6, at + outward * 26 + tangent * 18, at + outward * 18 + tangent * 26], [12.0, 10.0, 7.0, 2.0])));
}
let apex = along_rim[6].position;
// The finial: a trefoil on a stem.
crockets.append(shapes::closed([apex + vec2(-6, 2), apex + vec2(-5, -46), apex + vec2(5, -46), apex + vec2(6, 2)]));
crockets.append(shapes::closed(shapes::foil_points(apex + vec2(0, -66), 26, 3, 0.4, -PI() / 2.0)));
let fin = new_canvas();
engraving::dark(fin, crockets, INK, 2.0);
arch.add(fin);

// Capitals at the springing, and the plinth.
let stones = new_path();
for side in [-1.0, 1.0] {
	let x = C + side * 451;
	stones.append(shapes::closed([vec2(x - 40, SPRING - 22), vec2(x + 40, SPRING - 22), vec2(x + 30, SPRING + 8), vec2(x - 30, SPRING + 8)]));
	stones.append(shapes::closed([vec2(x - 44, FOOT - 6), vec2(x + 44, FOOT - 6), vec2(x + 50, FOOT + 34), vec2(x - 50, FOOT + 34)]));
}
let capitals = new_canvas();
engraving::mass(capitals, stones, new_path(), INK, 2.0);
arch.add(capitals);
arch.add(new_line(vec2(130, FOOT + 34), vec2(1070, FOOT + 34)), inks::contour(INK, 2.0));
leaves::place(leaf, arch, 12);

// ---- the title ----

let words = new_canvas();
for line in [["THE", 380.0, 46.0], ["OSSUARY", 470.0, 88.0], ["PSALTER", 580.0, 88.0]] {
	words.add(leaves::centred(line[0], vec2(C, line[1]), leaves::title_hand(line[2], INK)));
}
// A rule under it, a lozenge at its middle.
words.add(new_line(vec2(380, 650), vec2(578, 650)), inks::contour(INK, 1.6));
words.add(new_line(vec2(622, 650), vec2(820, 650)), inks::contour(INK, 1.6));
let lozenge = shapes::shape([vec2(C, 638), vec2(C + 14, 650), vec2(C, 662), vec2(C - 14, 650)]);
words.add(lozenge, inks::contour(INK, 1.6));
engraving::dark(words, lozenge, INK, 2.0);
leaf.figures.add(words);

// ---- the skull, the bones behind it ----

let S = vec2(C, 880);
let K = 0.8;
let rel = |x, y| S + vec2(x * K, y * K);
let halves = |pts| {
	let r = [];
	for p in pts { r.push(rel.call(p[0], p[1])); }
	r = shapes::curve(r, 6);
	let all = [];
	for p in r { all.push(p); }
	for k in 1..r.len() - 1 {
		let p = r[r.len() - 1 - k];
		all.push(vec2(2.0 * S.x - p.x, p.y));
	}
	all
};
let skull_pts = halves.call([[0, -170], [58, -164], [104, -140], [138, -100], [157, -48], [160, 2], [153, 46], [143, 78], [152, 98], [142, 120], [118, 130], [100, 146], [92, 176], [84, 202], [80, 236], [74, 264], [54, 284], [28, 294], [0, 296]]);
let skull = shapes::closed(skull_pts);

// The bones, crossed behind: each a shaft and a knuckle of two knobs at
// either end, outlined as one.
let bone = |a, b| {
	let way = (b - a).normalized();
	let across = vec2(-way.y, way.x);
	let m = (a + b) / 2.0;
	let parts = [shapes::closed([a + across * 13, m + across * 9, b + across * 13, b - across * 13, m - across * 9, a - across * 13])];
	for end in [[a, -1.0], [b, 1.0]] {
		for s in [-1.0, 1.0] {
			parts.push(new_circle(end[0] + across * s * 12 + way * end[1] * 4, 17));
		}
	}
	let whole = shapes::join(parts.map(|p| p));
	let outline = new_path();
	for (p, k) in parts {
		let others = new_path();
		for (q, j) in parts { if j != k { others.append(q); } }
		outline.append(p.erase_inside(others, #{ fill_rule: "nonzero" }));
	}
	#{ whole: whole, outline: outline, shade: whole.hatch(3.5, #{ angle: [0.3], fill_rule: "nonzero" }).erase_outside(shapes::shape([a - across * 40 - way * 30, b - across * 40 + way * 30, b - across * 2 + way * 30, a - across * 2 - way * 30])) }
};
let B = vec2(C, 1010);
let under = bone.call(B + vec2(-250, -150), B + vec2(250, 150));
let over = bone.call(B + vec2(250, -150), B + vec2(-250, 150));
let bones = new_canvas();
bones.add(shapes::behind(under.outline.erase_inside(over.whole, #{ fill_rule: "nonzero" }), skull), inks::contour(INK, 2.2));
bones.add(shapes::behind(under.shade.erase_inside(over.whole, #{ fill_rule: "nonzero" }), skull), inks::detail(INK, 0.8));
bones.add(shapes::behind(over.outline, skull), inks::contour(INK, 2.2));
bones.add(shapes::behind(over.shade, skull), inks::detail(INK, 0.8));
leaf.figures.add(bones);

// The skull itself: drawn in line, shaded to the right, its hollows dark.
let head = new_canvas();
head.add(skull, inks::contour(INK, 2.6));
// Lit from the upper left: hatched where the light falls off, crossed
// where it's gone.
head.add(skull.hatch(3.4, #{ angle: [0.95], roughness: 0.8 }).erase_inside(new_ellipse(rel.call(-30, -46), 140 * K, 196 * K)).erase_inside(new_rectangle(rel.call(-6, 234), 120 * K, 62 * K)), inks::detail(INK, 0.9));
head.add(skull.hatch(3.8, #{ angle: [2.3], roughness: 0.8 }).erase_inside(new_ellipse(rel.call(-22, -36), 158 * K, 218 * K)).erase_inside(new_rectangle(rel.call(0, 234), 132 * K, 62 * K)), inks::detail(INK, 0.8));
let socket = |c, flip| {
	let pts = [];
	for p in [[-32, -24], [-4, -30], [28, -22], [38, 2], [30, 26], [0, 30], [-26, 22], [-36, -2]] {
		pts.push(rel.call(c[0] + p[0] * flip, c[1] + p[1]));
	}
	shapes::closed(shapes::loop_curve(pts, 6))
};
let hollows = new_path();
hollows.append(socket.call([62.0, 46.0], 1.0)).append(socket.call([-62.0, 46.0], -1.0));
// The brows over the sockets and the hollows under the cheekbones, hatched.
let shaded = new_path();
for flip in [-1.0, 1.0] {
	let c = rel.call(flip * 62, 46);
	let brow = socket.call([flip * 62, 46.0], flip).scale_by(vec2(1.4, 1.5)).translate_by(vec2(c.x * -0.4, c.y * -0.5));
	shaded.append(brow.hatch(3.2, #{ angle: [flip * 0.5 + 1.57], roughness: 0.6 }).erase_inside(socket.call([flip * 62, 46.0], flip), #{ fill_rule: "nonzero" })
		.erase_inside(new_rectangle(c + vec2(0, 60), 300, 120)));
	head.add(socket.call([flip * 62, 46.0], flip).scale_by(vec2(1.15, 1.15)).translate_by(c * -0.15), inks::detail(INK, 0.9));
	shaded.append(shapes::shape([rel.call(flip * 88, 128), rel.call(flip * 146, 116), rel.call(flip * 132, 150), rel.call(flip * 96, 206), rel.call(flip * 84, 186)]).hatch(3.0, #{ angle: [flip * 0.9], roughness: 0.6 }));
	shaded.append(shapes::shape([rel.call(flip * 132, 44), rel.call(flip * 158, 30), rel.call(flip * 156, 70), rel.call(flip * 140, 84)]).hatch(3.0, #{ angle: [0.3], roughness: 0.6 }));
}
head.add(shaded, inks::detail(INK, 0.8));
let nose = [];
for p in [[0, 96], [15, 116], [20, 140], [10, 152], [0, 142], [-10, 152], [-20, 140], [-15, 116]] { nose.push(rel.call(p[0], p[1])); }
hollows.append(shapes::closed(nose));
// The teeth, each drawn, a dark seam where the rows meet.
hollows.append(shapes::closed([rel.call(-64, 232), rel.call(64, 232), rel.call(58, 237), rel.call(-58, 237)]));
let teeth = new_path();
for k in 0..8 {
	let x = -56.0 + k * 14.0;
	let lean = x * 0.06;
	teeth.append(shapes::shape(shapes::loop_curve([rel.call(x + 1, 206), rel.call(x + 13, 206), rel.call(x + 12 + lean, 231), rel.call(x + 7 + lean, 233), rel.call(x + 2 + lean, 231)], 4)));
	teeth.append(shapes::shape(shapes::loop_curve([rel.call(x + 2 + lean, 238), rel.call(x + 7 + lean, 236), rel.call(x + 12 + lean, 238), rel.call(x + 12, 262), rel.call(x + 2, 262)], 4)));
}
head.add(teeth, inks::detail(INK, 1.0));
// The hollows under the cheekbones, and the temples.
for flip in [-1.0, 1.0] {
	hollows.append(shapes::closed([rel.call(flip * 96, 136), rel.call(flip * 120, 132), rel.call(flip * 92, 196), rel.call(flip * 84, 176)]));
}
engraving::mass(head, hollows, new_path(), INK, 1.8);
// The sutures and a crack.
head.add(new_polyline(shapes::curve([rel.call(-120, -96), rel.call(-70, -112), rel.call(-30, -104), rel.call(0, -118), rel.call(36, -106), rel.call(80, -116), rel.call(122, -96)], 4)), inks::detail(INK, 1.0));
head.add(new_polyline([rel.call(-46, -160), rel.call(-38, -132), rel.call(-50, -110), rel.call(-36, -82), rel.call(-44, -62)]), inks::detail(INK, 1.2));
head.add(new_polyline(shapes::curve([rel.call(-74, 210), rel.call(-70, 236), rel.call(-62, 270)], 4)), inks::detail(INK, 1.0));
head.add(new_polyline(shapes::curve([rel.call(74, 210), rel.call(70, 236), rel.call(62, 270)], 4)), inks::detail(INK, 1.0));
leaf.figures.add(head);

// ---- the ribbon ----

let ribbon = new_canvas();
let RY = 1200.0;
let top = [];
let bottom = [];
for j in 0..=16 {
	let x = 330.0 + 540.0 * j / 16.0;
	let y = RY + 10.0 * (PI() * j / 8.0).sin();
	top.push(vec2(x, y));
	bottom.push(vec2(x, y + 58));
}
let face = shapes::shape(top + backward.call(bottom));
for side in [-1.0, 1.0] {
	let x0 = C + side * 270;
	let x1 = C + side * 340;
	let tail = shapes::shape([vec2(x0, RY + 14), vec2(x1, RY + 14), vec2(x1 - side * 22, RY + 44), vec2(x1, RY + 74), vec2(x0, RY + 74)]);
	let fold = shapes::shape([vec2(x0, RY + 58), vec2(x0 - side * 10, RY + 74), vec2(x0, RY + 74)]);
	ribbon.add(tail.erase_inside(face, #{ fill_rule: "nonzero" }), inks::contour(INK, 1.8));
	ribbon.add(tail.hatch(4.0, #{ angle: [0.5] }).erase_inside(face, #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.9));
	let shut = new_canvas();
	engraving::dark(shut, fold, INK, 1.5);
	ribbon.add(shut);
}
ribbon.add(face, inks::contour(INK, 2.0));
ribbon.add(leaves::centred("MEMENTO MORI", vec2(C, RY + 31), leaves::carved_hand(36.0, INK)));
leaf.figures.add(ribbon);

// ---- the bramble up the jambs ----

for side in [-1.0, 1.0] {
	let x = C + side * 404;
	let briar = new_polyline(shapes::curve([vec2(x, FOOT - 10), vec2(x + side * 8, 1250), vec2(x - side * 6, 1050), vec2(x + side * 4, 880), vec2(x - side * 10, 720)], 16));
	let plant = art::bramble_along(briar, 26.0, #{ seed: if side < 0.0 { 3 } else { 8 }, origin: vec2(x, FOOT - 10), thickness: 2.0 });
	leaf.figures.add(new_canvas().add(plant, #{ vines: inks::contour(INK, 1.3), leaves: inks::detail(INK, 0.9), flowers: inks::detail(INK, 1.0) }));
}

// ---- whose book ----

let whose = leaves::note("The sexton's book, of Saint Ysolt-under-Fen.", vec2(236, 1282), 760, #{ ink: INK, size: 29 });
leaf.figures.add(whose.canvas);
let warning = leaves::note("Do not ring the bells.", vec2(440, 1352), 360, #{ size: 32, ink: BLOOD, turn: -0.015 });
leaf.figures.add(warning.canvas);

journal_entry.add(leaves::bind(leaf, content("psalter"), #{ size: 20.0 }));
