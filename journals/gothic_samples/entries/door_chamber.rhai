// The chamber: her own room. On the left, the worst night: she lies in
// the four-poster, held down by what squats on her chest, its head turned
// to whoever looks; past it, between the curtains, the door, open a
// finger's width with light behind it; and on the bedpost, cut in her own
// hand that she doesn't remember, places in her account, a word and a
// letter each (book_cipher). On the right, the three glasses of her
// dressing table, one covered with a sheet, and written on them from the
// other side, backwards, a rail to a glass (rail_fence).

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

seed(2301);

// ---- the secrets (chains/door_chamber) ----

let secrets = chain("door_chamber").run();
let cut = secrets.text("post.references").split(" ");
let railed = secrets.get("glasses.cipher_text").value();
// How many letters each rail holds: down three rails and up again, a
// zigzag of four.
let counts = [0, 0, 0];
for i in 0..23 {
	let r = i % 4;
	counts[if r == 3 { 1 } else { r }] += 1;
}

// ---- the left page: the nightmare ----

let leaf = nb::leaf("nightmare", 91, #{ foxing: 120, stains: 1 });
let FLOOR = 1300.0;
let tube = |pts, ws| shapes::closed(shapes::tube(pts, ws));

// What stands in front, worked out first so what's behind can be cut
// away from it.
let coverlet_top = shapes::curve([vec2(150, 905), vec2(250, 902), vec2(330, 876), vec2(430, 854), vec2(560, 872), vec2(680, 860), vec2(800, 870), vec2(880, 856), vec2(950, 870), vec2(990, 852), vec2(1000, 880)], 6);
let hem = shapes::steps(40, |j| {
	let x = 1000.0 - 850.0 * j / 40.0;
	vec2(x + 6.0 * (j % 2), 1140 + 8.0 * (x * 0.05).sin())
});
let coverlet = shapes::closed(coverlet_top + [vec2(1006, 1120)] + hem);
let pillow = shapes::closed(shapes::loop_curve([vec2(168, 840), vec2(240, 818), vec2(330, 826), vec2(372, 850), vec2(350, 900), vec2(250, 908), vec2(172, 900)], 6));
// Her head on the pillow, face up, in profile.
let face_pts = shapes::curve([vec2(226, 846), vec2(240, 836), vec2(254, 832), vec2(262, 830), vec2(270, 816), vec2(276, 816), vec2(281, 830), vec2(289, 829), vec2(293, 826), vec2(298, 832), vec2(304, 831), vec2(312, 836), vec2(326, 848), vec2(346, 860)], 6);
let head = shapes::closed(face_pts + shapes::curve([vec2(350, 880), vec2(300, 892), vec2(246, 898), vec2(214, 888), vec2(206, 866), vec2(214, 850)], 6));
// Her arm, hanging over the side, the hand trailing.
let arm = tube.call([vec2(352, 882), vec2(364, 990), vec2(370, 1090), vec2(376, 1180), vec2(380, 1236)], [24.0, 22.0, 19.0, 15.0, 13.0]);
let fingers = shapes::join([-8.0, -3.0, 2.0, 7.0].map(|f| tube.call([vec2(380 + f, 1230), vec2(381 + f * 1.3, 1262), vec2(384 + f * 1.5, 1290 - f.abs())], [5.0, 4.0, 2.6])));

// What sits on her: gaunt, black, its knees up, a hand at her throat,
// its head turned round to look out of the page.
let it = new_path();
it.append(tube.call([vec2(492, 852), vec2(500, 790), vec2(486, 734), vec2(458, 702), vec2(436, 690)], [36.0, 44.0, 42.0, 34.0, 24.0]));
it.append(tube.call([vec2(488, 848), vec2(436, 772), vec2(424, 760)], [30.0, 22.0, 18.0]));
it.append(tube.call([vec2(424, 760), vec2(432, 820), vec2(444, 862)], [18.0, 13.0, 10.0]));
it.append(tube.call([vec2(444, 860), vec2(424, 866), vec2(408, 866)], [10.0, 6.0, 2.0]));
it.append(tube.call([vec2(452, 706), vec2(402, 762), vec2(372, 818), vec2(334, 846)], [18.0, 12.0, 10.0, 8.0]));
for f in [[-24, -8], [-26, 2], [-22, 10], [-14, 14]] {
	it.append(tube.call([vec2(334, 846), vec2(334 + f[0] * 0.6, 846 + f[1] * 0.6), vec2(334 + f[0], 846 + f[1])], [6.0, 3.0, 0.8]));
}
it.append(tube.call([vec2(470, 714), vec2(500, 760), vec2(520, 802), vec2(530, 846)], [16.0, 11.0, 9.0, 6.0]));
let skull = vec2(420, 650);
it.append(shapes::closed(shapes::loop_curve([skull + vec2(-26, 4), skull + vec2(-22, -26), skull + vec2(0, -38), skull + vec2(22, -32), skull + vec2(30, -6), skull + vec2(26, 22), skull + vec2(10, 48), skull + vec2(-8, 52), skull + vec2(-20, 30)], 6)));
// Knuckles of its spine, standing out along its back.
for j in 0..6 {
	let t = j / 5.0;
	let at = vec2(496 - 40 * t * t, 800 - 100 * t) + vec2(14, 0);
	it.append(new_circle(at, 7 - 2 * t));
}
it.append(tube.call([skull + vec2(4, 30), vec2(432, 698), vec2(440, 712)], [22.0, 20.0, 22.0]));
// What's left bare in it: its eyes, its grin, its ribs and spine.
let its_bare = new_path();
its_bare.append(shapes::sliver([skull + vec2(-18, -8), skull + vec2(-11, -4), skull + vec2(-4, -3)], 3.4));
its_bare.append(shapes::sliver([skull + vec2(5, -5), skull + vec2(12, -7), skull + vec2(19, -12)], 3.4));
its_bare.append(shapes::sliver(shapes::curve([skull + vec2(-16, 24), skull + vec2(-4, 34), skull + vec2(8, 32), skull + vec2(18, 20)], 4), 3.0));
for j in 0..4 {
	let y = 740.0 + j * 18.0;
	its_bare.append(shapes::sliver([vec2(478, y), vec2(490, y + 4), vec2(502, y + 2)], 2.2));
}
let PX = 1040.0;
let post_shape = new_path();
post_shape.append(shapes::closed(shapes::curve([vec2(PX - 12, 212), vec2(PX - 22, 300), vec2(PX - 30, 380), vec2(PX - 16, 450), vec2(PX - 22, 520), vec2(PX - 36, 580)], 6) + shapes::curve([vec2(PX + 36, 580), vec2(PX + 22, 520), vec2(PX + 16, 450), vec2(PX + 30, 380), vec2(PX + 22, 300), vec2(PX + 12, 212)], 6)));
post_shape.append(shapes::closed([vec2(PX - 36, 580), vec2(PX + 36, 580), vec2(PX + 36, 1110), vec2(PX - 36, 1110)]));
post_shape.append(shapes::closed(shapes::curve([vec2(PX - 36, 1110), vec2(PX - 46, 1170), vec2(PX - 30, 1230), vec2(PX - 20, FLOOR)], 6) + shapes::curve([vec2(PX + 20, FLOOR), vec2(PX + 30, 1230), vec2(PX + 46, 1170), vec2(PX + 36, 1110)], 6)));
let grip = vec2(334, 846);
it = it.translate_by(vec2(-grip.x, -grip.y)).scale_by(vec2(1.5, 1.5)).translate_by(grip);
its_bare = its_bare.translate_by(vec2(-grip.x, -grip.y)).scale_by(vec2(1.5, 1.5)).translate_by(grip);
let silhouette = new_path().append(coverlet).append(pillow).append(head).append(arm).append(fingers).append(it).append(post_shape);

// The room behind: the wall, close-hatched in the dark, and in it the
// door.
let back = new_canvas();
let DOOR = [vec2(770, 366), vec2(906, 826)];
let architrave = new_rectangle((DOOR[0] + DOOR[1]) / 2.0 + vec2(0, -12), 186, 486);
let wall = new_rectangle(vec2(600, 600), 1080, 640);
back.add(shapes::behind(wall.hatch(4.0, #{ angle: [0.0], roughness: 1.0 }).erase_inside(architrave), silhouette), inks::fine(SEPIA, 0.8));
back.add(shapes::behind(wall.hatch(5.0, #{ angle: [1.1], roughness: 1.0 }).erase_inside(architrave), silhouette), inks::fine(SEPIA, 0.7));
back.add(shapes::behind(architrave, silhouette), inks::contour(INK, 1.8));
back.add(shapes::behind(new_rectangle((DOOR[0] + DOOR[1]) / 2.0 + vec2(0, -6), 160, 472), silhouette), inks::detail(INK, 1.0));
let door = shapes::closed([DOOR[0], vec2(DOOR[1].x, DOOR[0].y), DOOR[1], vec2(DOOR[0].x, DOOR[1].y)]);
// Open a finger's width, with light behind it.
let gap = new_rectangle(vec2(DOOR[0].x + 7, (DOOR[0].y + DOOR[1].y) / 2.0), 11, DOOR[1].y - DOOR[0].y - 4);
engraving::mass(back, door, new_path().append(gap).append(silhouette), INK, 2.0);
let glow = shapes::join(shapes::times(14, |j| {
	let y = DOOR[0].y + 20 + j * 32.0;
	new_line(vec2(DOOR[0].x - 6, y), vec2(DOOR[0].x - 40 - 20 * (j % 2), y + 6 - 12 * (j % 3)))
}));
back.add(shapes::behind(glow, silhouette), inks::fine(SEPIA, 0.9));
leaf.figures.add(back);

// The bed's hangings: the tester, its scalloped valance, the curtains
// tied back.
let hangings = new_canvas();
let drapes = new_path();
drapes.append(shapes::closed(shapes::curve([vec2(150, 270), vec2(420, 270), vec2(400, 520), vec2(330, 700), vec2(300, 730), vec2(360, 900), vec2(420, 1150), vec2(150, 1150)], 6)));
drapes.append(shapes::closed(shapes::curve([vec2(980, 270), vec2(1060, 270), vec2(1060, 1150), vec2(1000, 1150), vec2(990, 760), vec2(1000, 700), vec2(970, 420)], 6)));
let valance = [vec2(100, 214), vec2(1100, 214)];
for j in 0..=10 {
	let x = 1100.0 - 100.0 * j;
	valance.push(vec2(x, 270));
	if j < 10 { valance.push(vec2(x - 50, 296)); }
}
drapes.append(shapes::closed(valance));
let folds = shapes::join([190.0, 240.0, 300.0, 360.0].map(|x| {
	shapes::sliver(shapes::curve([vec2(x, 300), vec2(x + (330 - x) * 0.6, 640), vec2(318 + (x - 270) * 0.1, 730), vec2(x + 20, 1000), vec2(x + 30, 1140)], 6), 3.0)
}));
folds.append(shapes::sliver(shapes::curve([vec2(1020, 300), vec2(1012, 600), vec2(1018, 900), vec2(1026, 1140)], 6), 3.0));
for j in 0..10 { folds.append(shapes::sliver([vec2(150 + 100 * j, 228), vec2(150 + 100 * j, 260), vec2(150 + 100 * j, 286)], 2.6)); }
engraving::mass(hangings, drapes, new_path().append(folds).append(silhouette), INK, 2.0);
hangings.add(new_rectangle(vec2(600, 186), 1020, 50), inks::contour(INK, 2.2));
hangings.add(shapes::behind(new_rectangle(vec2(600, 186), 1020, 50).hatch(3.0, #{ angle: [0.0] }), silhouette), inks::detail(INK, 0.8));
// The tieback, its cord and tassel.
hangings.add(new_polyline(shapes::curve([vec2(296, 716), vec2(330, 722), vec2(350, 740), vec2(346, 790)], 6)), inks::contour(INK, 2.2));
hangings.add(shapes::shape(shapes::tube([vec2(346, 790), vec2(344, 820), vec2(346, 846)], [8.0, 16.0, 20.0])), inks::contour(INK, 1.4));
leaf.figures.add(hangings);

// The posts: the head post behind, the foot post in front, turned, and
// cut into.
let wood = new_canvas();
engraving::mass(wood, shapes::closed([vec2(116, 212), vec2(142, 212), vec2(142, FLOOR), vec2(116, FLOOR)]), silhouette, INK, 2.0);
let headboard = shapes::closed([vec2(130, 1150), vec2(130, 600), vec2(146, 560), vec2(162, 600), vec2(162, 1150)]);
engraving::mass(wood, headboard, silhouette, INK, 2.0);
leaf.figures.add(wood);

// The bed: the coverlet and its folds, the pillow, her, and under the
// bed the dark.
let bed = new_canvas();
let under_bed = shapes::closed([vec2(150, 1140), vec2(1000, 1140), vec2(1000, FLOOR), vec2(150, FLOOR)]);
engraving::velvet(bed, under_bed, INK, 2.4);
let floor = new_rectangle(vec2(600, 1450), 1160, 300);
let note = leaves::note("Cut in the foot-post in my own hand, that I do not remember: a word of this book and a letter of it, each.", vec2(80, 1420), 900, #{ size: 28, ink: COLD, turn: -0.006 });
bed.add(shapes::behind(floor.hatch(4.0, #{ angle: [0.0], roughness: 1.0 }).erase_inside(note.hole), post_shape), inks::fine(INK, 0.8));
let planks = new_path();
for j in -8..24 { planks.append(new_line(vec2(600 + j * 60 - 300, FLOOR), vec2(600 + (j * 60 - 300) * 1.6, 1600))); }
bed.add(shapes::behind(planks.erase_outside(floor).erase_inside(note.hole), post_shape), inks::detail(INK, 0.9));
for x in [180.0, 980.0] {
	let leg = shapes::shape(shapes::tube([vec2(x, 1140), vec2(x - 4, 1200), vec2(x + 6, 1250), vec2(x, FLOOR)], [30.0, 22.0, 26.0, 16.0]));
	engraving::mass(bed, leg, new_path(), INK, 1.8);
}
bed.add(shapes::behind(coverlet, post_shape), inks::contour(INK, 2.0));
let cover_folds = shapes::join([240.0, 320.0, 470.0, 560.0, 650.0, 760.0, 860.0, 950.0].map(|x| {
	let top = coverlet_top[((x - 150.0) / 850.0 * (coverlet_top.len() - 1)).to_int()];
	new_polyline(shapes::curve([top + vec2(0, 14), vec2(x + 10, 1000), vec2(x - 6, 1080), vec2(x + 4, 1134)], 6))
}));
bed.add(shapes::behind(cover_folds.erase_inside(arm, #{ fill_rule: "nonzero" }), post_shape), inks::detail(INK, 1.0));
bed.add(coverlet.hatch(3.2, #{ angle: [1.2], roughness: 0.8 }).erase_inside(shapes::shape(coverlet_top.map(|p| p + vec2(0, 60)) + [vec2(1000, 1000), vec2(150, 1000)])).erase_inside(arm, #{ fill_rule: "nonzero" }).erase_inside(post_shape, #{ fill_rule: "nonzero" }), inks::detail(INK, 0.8));
bed.add(coverlet.hatch(3.6, #{ angle: [0.3], roughness: 0.8 }).erase_inside(new_rectangle(vec2(575, 960), 900, 260)).erase_inside(arm, #{ fill_rule: "nonzero" }).erase_inside(post_shape, #{ fill_rule: "nonzero" }), inks::fine(INK, 0.8));
let fringe = shapes::join(hem.map(|p| new_line(p, p + vec2(0, 14))));
bed.add(shapes::behind(fringe, post_shape), inks::detail(INK, 1.0));
bed.add(pillow.erase_inside(head, #{ fill_rule: "nonzero" }), inks::contour(INK, 1.6));
bed.add(pillow.hatch(3.0, #{ angle: [0.4] }).erase_inside(new_ellipse(vec2(270, 840), 110, 30)).erase_inside(head, #{ fill_rule: "nonzero" }), inks::detail(INK, 0.8));
// Her: the profile, the closed eye, her hair spilt over the pillow and
// down the side.
bed.add(head, inks::contour(INK, 1.6));
bed.add(new_polyline(shapes::curve([vec2(252, 846), vec2(260, 850), vec2(268, 847)], 4)), inks::detail(INK, 1.2));
bed.add(new_polyline(shapes::curve([vec2(250, 838), vec2(260, 836), vec2(268, 839)], 4)), inks::detail(INK, 1.0));
let hair = shapes::join(shapes::times(16, |j| {
	let s = j / 15.0;
	new_polyline(shapes::curve([vec2(226 - 8 * s, 850 + 30 * s), vec2(196 - 20 * s, 880 + 20 * s), vec2(176 - 10 * s, 930 + 30 * s), vec2(170 + 14 * s, 1010 + 60 * s), vec2(186 + 20 * s, 1080 + 50 * s)], 6))
}));
bed.add(hair, new_style(new_brush_style(INK, "Strands", 3.2, #{ min_pressure: 0.4, max_pressure: 0.9, pressure_profile: "taper" }), new_handwriting_style()));
bed.add(arm, inks::contour(INK, 1.6)).add(fingers, inks::contour(INK, 1.2));
bed.add(engraving::shade(arm, 2.8, #{ angle: [0.2], lit: vec2(-8, 0) }), inks::detail(INK, 0.8));
// It: black, all but its eyes and its grin and the light along its ribs.
engraving::mass(bed, it, its_bare, INK, 1.8);
leaf.figures.add(bed);

// The foot post, turned, square where it was cut into.
let post = new_canvas();
post.add(post_shape, inks::contour(INK, 1.8));
for y in [580.0, 600.0, 1090.0, 1110.0, 450.0, 1170.0] { post.add(new_line(vec2(PX - 40, y), vec2(PX + 40, y)), inks::detail(INK, 1.1)); }
// Where it was cut, the wood's shading pared away.
let carved = new_path().append(new_rectangle(vec2(PX - 18, 760), 44, 1200));
for (r, k) in cut {
	let written = write_line(r, vec2(0, 0), leaves::figures_hand(21.0, COLD));
	let b = written.bounds();
	carved.append(new_rectangle(vec2(PX, 660 + k * 84), b.max.x - b.min.x + 10, b.max.y - b.min.y + 8));
	post.add(written.translate_by(vec2(PX, 660 + k * 84) - b.center));
	post.add(new_line(vec2(PX - 24, 684 + k * 84), vec2(PX + 22, 686 + k * 84)), inks::fine(INK, 0.8));
}
post.add(post_shape.hatch(2.8, #{ angle: [1.5], roughness: 0.6, fill_rule: "nonzero" }).erase_inside(carved, #{ fill_rule: "nonzero" }), inks::detail(INK, 0.9));
leaf.figures.add(post);

leaves::jot(leaf, "The worst of them. I could not move or cry out. Over its shoulder, the door, and light behind it.", vec2(110, 64), 980, #{ size: 28, turn: 0.004 });
leaf.figures.add(note.canvas);

journal_entry.add(leaves::bind(leaf, ""));

// ---- the right page: the glasses on the table ----

let leaf = nb::leaf("vanity", 93, #{ foxing: 130, stains: 1 });
let lift = vec2(0, -40);
let TABLE = 1080.0;

// A glass with a round head: from x0 to x1, springing at `spring`, its
// foot at `foot`, `grow` out from the glass itself.
let pane = |x0, x1, spring, foot, grow| {
	let mid = (x0 + x1) / 2.0;
	let half = (x1 - x0) / 2.0 + grow;
	shapes::shape([vec2(mid - half, foot + grow)] + shapes::arc_points(vec2(mid, spring), half, PI(), 2.0 * PI(), 24) + [vec2(mid + half, foot + grow)])
};
let panes = [[270.0, 430.0, 640.0], [450.0, 750.0, 560.0], [770.0, 930.0, 640.0]];
let FOOT = 1050.0;
let frames = shapes::join(panes.map(|p| pane.call(p[0], p[1], p[2], FOOT, 20.0)));
// The sheet over the right-hand glass.
let sheet = shapes::closed(shapes::curve([vec2(752, 610), vec2(782, 552), vec2(850, 532), vec2(914, 546), vec2(956, 596), vec2(970, 700), vec2(978, 860), vec2(990, 1010), vec2(956, 1018), vec2(930, 900), vec2(900, 808), vec2(860, 770), vec2(816, 750), vec2(778, 742), vec2(756, 724)], 6));

// The wall: paper with a sprig, and the table under the glasses.
let room = new_canvas();
let wall = new_rectangle(vec2(600, 620), 1120, 920);
let sprigs = new_path();
for row in 0..14 {
	for col in 0..17 {
		let at = vec2(70 + col * 66, 190 + row * 64 + (col % 2) * 32);
		sprigs.append(shapes::shape(shapes::foil_points(at, 6.0, 4, 0.5, 0.0)));
	}
}
room.add(wall.hatch(5.0, #{ angle: [0.0], roughness: 1.0 }).erase_inside(frames.copy().translate_by(lift)).erase_inside(sheet.copy().translate_by(lift), #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.7));
room.add(sprigs.erase_inside(frames.copy().translate_by(lift)).erase_inside(sheet.copy().translate_by(lift), #{ fill_rule: "nonzero" }), inks::fine(SEPIA, 0.9));
leaf.figures.add(room);

let glasses = new_canvas();
let letters = symbol_set("letters", #{ color: COLD, size: 42.0 });
let from = 0;
for (p, k) in panes {
	let glass = pane.call(p[0], p[1], p[2], FOOT, 0.0);
	let mid = (p[0] + p[1]) / 2.0;
	gothic::glints(glasses, shapes::behind(glass, sheet), vec2(mid, 800), 300.0, SEPIA);
	glasses.add(shapes::behind(pane.call(p[0], p[1], p[2], FOOT, 20.0), sheet), inks::contour(INK, 2.2));
	glasses.add(shapes::behind(pane.call(p[0], p[1], p[2], FOOT, 13.0), sheet), inks::detail(INK, 1.0));
	glasses.add(shapes::behind(glass, sheet), inks::contour(INK, 1.5));
	glasses.add(shapes::behind(pane.call(p[0], p[1], p[2], FOOT, 20.0), sheet).hatch(3.0, #{ angle: [0.8] }).erase_inside(glass).erase_inside(new_rectangle(vec2(mid - 30, 700), (p[1] - p[0]) * 0.7, 1000)), inks::detail(INK, 0.8));
	// Its rail, as written from the far side.
	let n = counts[k];
	let these = railed.extract(from, n);
	from += n;
	let top = if k == 1 { 466.0 } else if k == 0 { 720.0 } else { 800.0 };
	let step = if k == 1 { 56.0 } else if k == 0 { 58.0 } else { 46.0 };
	for (s, j) in these {
		glasses.add(gothic::reflected(new_canvas().add(write_at(letters.text([s]), vec2(mid, top + j * step), #{}))));
	}
}
// Hinges between them.
for x in [440.0, 760.0] {
	for y in [700.0, 960.0] { glasses.add(new_rectangle(vec2(x, y), 10, 34), inks::contour(INK, 1.4)); }
}
leaves::place(leaf, glasses.translate_by(lift), 8);

// The sheet, white, its folds, shaded where it falls away.
let cloth = new_canvas();
cloth.add(sheet, inks::contour(INK, 1.8));
let creases = shapes::join([[800.0, 560.0, 790.0, 730.0], [850.0, 540.0, 846.0, 760.0], [900.0, 552.0, 912.0, 820.0], [940.0, 590.0, 956.0, 980.0], [960.0, 700.0, 974.0, 1000.0]].map(|c| {
	new_polyline(shapes::curve([vec2(c[0], c[1]), vec2((c[0] + c[2]) / 2.0 + 6, (c[1] + c[3]) / 2.0), vec2(c[2], c[3])], 6))
}));
cloth.add(creases, inks::detail(INK, 1.0));
cloth.add(sheet.hatch(3.2, #{ angle: [1.2], roughness: 0.6, fill_rule: "nonzero" }).erase_inside(new_ellipse(vec2(830, 640), 90, 120)), inks::detail(INK, 0.8));
leaves::place(leaf, cloth.translate_by(lift), 8);

// The table: its top, its drawers with their rings, its legs, and what's
// on it: a candle, a hand-glass laid face down, a brush, the drops she
// sleeps by.
let table = new_canvas();
let top_slab = shapes::closed([vec2(150, TABLE), vec2(1050, TABLE), vec2(1060, TABLE + 32), vec2(140, TABLE + 32)]);
table.add(top_slab, inks::contour(INK, 2.0));
table.add(new_rectangle(vec2(600, TABLE + 24), 916, 14).hatch(2.6, #{ angle: [0.0] }), inks::detail(INK, 0.8));
let apron = new_rectangle(vec2(600, TABLE + 82), 860, 96);
table.add(apron, inks::contour(INK, 1.8));
for x in [380.0, 820.0] {
	table.add(new_rectangle(vec2(x, TABLE + 82), 300, 66), inks::detail(INK, 1.2));
	table.add(new_circle(vec2(x, TABLE + 82), 10), inks::contour(INK, 1.4));
	table.add(new_circle(vec2(x, TABLE + 70), 3), inks::fill(INK, 1.5));
}
table.add(apron.hatch(3.4, #{ angle: [0.2] }).erase_inside(new_rectangle(vec2(380, TABLE + 82), 300, 66)).erase_inside(new_rectangle(vec2(820, TABLE + 82), 300, 66)), inks::fine(INK, 0.8));
for side in [-1.0, 1.0] {
	let x = 600.0 + side * 410;
	let leg = shapes::shape(shapes::tube([vec2(x, TABLE + 130), vec2(x + side * 26, TABLE + 190), vec2(x + side * 4, TABLE + 270), vec2(x - side * 6, TABLE + 330), vec2(x + side * 14, TABLE + 356)], [40.0, 34.0, 20.0, 16.0, 24.0]));
	table.add(leg, inks::contour(INK, 1.8));
	table.add(engraving::shade(leg, 2.8, #{ angle: [0.6], lit: vec2(-side * 8, 0) }), inks::detail(INK, 0.8));
}
let under = shapes::shape([vec2(190, TABLE + 130), vec2(1010, TABLE + 130), vec2(980, TABLE + 356), vec2(220, TABLE + 356)]);
table.add(under.hatch(4.0, #{ angle: [0.0] }).erase_inside(new_rectangle(vec2(190, 1400), 90, 400)).erase_inside(new_rectangle(vec2(1010, 1400), 90, 400)), inks::fine(INK, 0.8));
// The candle.
let cx = 200.0;
table.add(new_ellipse(vec2(cx, TABLE - 4), 34, 7), inks::contour(INK, 1.6));
let stick = shapes::closed([vec2(cx - 8, TABLE - 8), vec2(cx - 8, TABLE - 90), vec2(cx + 8, TABLE - 90), vec2(cx + 8, TABLE - 8)]);
table.add(stick, inks::contour(INK, 1.4));
table.add(new_ellipse(vec2(cx, TABLE - 92), 18, 5), inks::contour(INK, 1.4));
let candle = shapes::closed([vec2(cx - 9, TABLE - 94), vec2(cx - 9, TABLE - 180), vec2(cx + 9, TABLE - 180), vec2(cx + 9, TABLE - 94)]);
table.add(candle, inks::contour(INK, 1.4));
let flame = shapes::shape(shapes::tube([vec2(cx, TABLE - 186), vec2(cx + 1, TABLE - 200), vec2(cx - 1, TABLE - 216), vec2(cx + 2, TABLE - 232)], [7.0, 11.0, 7.0, 1.0]));
table.add(flame, inks::contour(INK, 1.0));
table.add(new_line(vec2(cx, TABLE - 180), vec2(cx, TABLE - 188)), inks::contour(INK, 2.4));
let glow = shapes::join(shapes::times(18, |j| {
	let a = 2.0 * PI() * j / 18.0;
	new_line(vec2(cx, TABLE - 206) + shapes::polar(a) * 40, vec2(cx, TABLE - 206) + shapes::polar(a) * (60 + 20 * (j % 3)))
}));
table.add(glow.erase_inside(candle), inks::pencil(SEPIA, 1.0));
// The hand-glass, face down.
let back = new_ellipse(vec2(330, TABLE - 10), 52, 12);
engraving::mass(table, shapes::closed(shapes::ellipse_points(vec2(330, TABLE - 10), 52, 12, 40)), new_path(), INK, 1.6);
table.add(shapes::shape(shapes::tube([vec2(380, TABLE - 8), vec2(420, TABLE - 6), vec2(450, TABLE - 5)], [10.0, 8.0, 12.0])), inks::contour(INK, 1.4));
// A brush.
table.add(new_ellipse(vec2(860, TABLE - 9), 40, 9), inks::contour(INK, 1.4));
let bristles = shapes::join(shapes::times(12, |j| new_line(vec2(826 + j * 6, TABLE - 14), vec2(826 + j * 6, TABLE - 22))));
table.add(bristles, inks::detail(INK, 0.9));
table.add(shapes::shape(shapes::tube([vec2(898, TABLE - 8), vec2(940, TABLE - 6), vec2(968, TABLE - 5)], [9.0, 8.0, 10.0])), inks::contour(INK, 1.3));
// The drops.
let bottle = shapes::closed(shapes::curve([vec2(978, TABLE), vec2(974, TABLE - 60), vec2(982, TABLE - 84), vec2(994, TABLE - 92), vec2(994, TABLE - 112), vec2(1006, TABLE - 112), vec2(1006, TABLE - 92), vec2(1018, TABLE - 84), vec2(1026, TABLE - 60), vec2(1022, TABLE)], 4));
table.add(bottle, inks::contour(INK, 1.5));
table.add(bottle.hatch(2.6, #{ angle: [1.3], fill_rule: "nonzero" }).erase_inside(new_rectangle(vec2(1000, TABLE - 40), 40, 34)).erase_inside(new_rectangle(vec2(986, TABLE - 50), 8, 80)), inks::detail(INK, 0.9));
table.add(new_rectangle(vec2(1000, TABLE - 40), 40, 34), inks::detail(INK, 1.0));
table.add(shapes::closed([vec2(992, TABLE - 112), vec2(1008, TABLE - 112), vec2(1010, TABLE - 128), vec2(990, TABLE - 128)]), inks::contour(INK, 1.3));
table.add(leaves::centred("TINCT.", vec2(1000, TABLE - 46), leaves::figures_hand(9.0, INK)));
table.add(leaves::centred("OPII", vec2(1000, TABLE - 34), leaves::figures_hand(9.0, INK)));
leaves::place(leaf, table.translate_by(lift), 8);

leaves::jot(leaf, "The glasses on my table. I covered one with a sheet. It wrote beneath it all the same.", vec2(110, 70), 980, #{ size: 28, turn: 0.004 });
leaves::jot(leaf, "Written from the other side: a rail to each glass. Down the three and up again, as a snake goes.", vec2(80, 1452), 1040, #{ size: 27, ink: COLD, turn: -0.004 });

journal_entry.add(leaves::bind(leaf, content("nights"), #{ size: 20.0 }));
