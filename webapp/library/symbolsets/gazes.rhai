// gazes: eyes (art::eye) each looking a way, a way to a symbol. With ways
// 3, 0 to 2 look left, ahead and right; with 5, ahead, up, right, down and
// left; with 9, ahead and then up and clockwise round the eight points of
// the compass. An eye looking sideways is level, up or down stood on end,
// aslant tilted, its iris gone to that corner. Options: ways (9), ink and
// iris (its two inks), seed.

import "lib/inks" as inks;

fn options() {
	#{ ways: 9, ink: "#2b1d12", iris: "#9b2318", seed: 0 }
}

fn symbols(o) {
	if !(o.ways in [3, 5, 9]) { throw `ways should be 3, 5 or 9, got ${o.ways}`; }
	#{ glyphset: "plain", glyph: "O", count: o.ways, size: 110.0 }
}

// Which way symbol d of ways looks: across and down, each -1, 0 or 1.
fn gaze(ways, d) {
	if ways == 3 { return [d - 1, 0]; }
	let all = if ways == 5 {
		[[0, 0], [0, -1], [1, 0], [0, 1], [-1, 0]]
	} else {
		[[0, 0], [0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]]
	};
	all[d]
}

fn draw(o, symbol, glyph) {
	let way = gaze(o.ways, symbol);
	let look = #{ openness: 0.42, messiness: 0.3, seed: o.seed + symbol, gaze: 0.0 };
	let turn = glyph.rotation;
	if way != [0, 0] {
		let toward = vec2(way[0], way[1]).angle();
		look.gaze = 1.0;
		look.gaze_angle = toward + turn;
		look.angle = turn + if way[1] == 0 { 0.0 } else { toward };
	} else {
		look.angle = turn;
	}
	let eye = art::eye(glyph.position, glyph.scale, look);
	let iris = glyph.scale * 0.42 / 2.0;
	let c = new_canvas();
	c.add(eye.wash, inks::wash(o.iris, iris * 0.3));
	c.add(eye.rim, inks::detail(o.iris, max(iris * 0.03, 0.6)));
	c.add(eye.accent, inks::detail(o.iris, max(iris * 0.03, 0.6)));
	c.add(eye.lines, inks::contour(o.ink, max(iris * 0.03, 0.8)));
	c
}
