// black_moons: the moon in its phases (the library's moons), but its dark
// side solid black, so the lit part shows as parchment: eight by default,
// symbol d the moon d / phases of the way through its month. Options:
// phases (8), ink.

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
	inks::moon(moon, o.ink, max(r * 0.03, 0.8), #{ gap: max(r * 0.09, 1.6) })
}
