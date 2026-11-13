// moons: the moon in its phases, a phase to a symbol: symbol d of phases
// is the moon d / phases of the way through its month (in 8, 0 is new, 1
// the waxing crescent, 2 the first quarter, 3 the waxing gibbous, 4 full,
// and 5 to 7 the same waning). Each is its outline and the line between
// light and dark, its dark side hatched. Options: phases (8), ink.

import "lib/inks" as inks;

fn options() {
	#{ phases: 8, ink: "#2b1d12" }
}

fn symbols(o) {
	#{ glyphset: "plain", glyph: "O", count: o.phases, size: 90.0 }
}

fn draw(o, symbol, glyph) {
	let r = glyph.scale / 2.0;
	let moon = art::moon(glyph.position, r, symbol * 1.0 / o.phases);
	let c = new_canvas();
	let gap = max(r * 0.07, 1.5);
	if !moon.dark.is_empty { c.add(moon.dark.hatch(gap, #{ angle: 0.8 }), inks::fine(o.ink, max(r * 0.012, 0.5))); }
	c.add(moon.outline, inks::contour(o.ink, max(r * 0.02, 0.7)));
	if !moon.terminator.is_empty { c.add(moon.terminator, inks::contour(o.ink, max(r * 0.02, 0.7))); }
	c
}
