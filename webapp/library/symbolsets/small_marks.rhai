// small_marks: four little marks to set between things, a lozenge (0), a
// dot (1), a cross (2) and a ring (3), each drawn in ink to fill its
// glyph's box and turned as the glyph is. Option: ink.

import "lib/inks" as inks;

fn options() {
	#{ ink: "#2b1d12" }
}

fn symbols(o) {
	#{ glyphset: "plain", glyph: "O", count: 4, size: 14.0 }
}

fn draw(o, symbol, glyph) {
	let r = glyph.scale / 2.0;
	let at = glyph.position;
	let c = new_canvas();
	let pen = inks::band(o.ink, max(r * 0.22, 0.6));
	if symbol == 0 {
		c.add(new_polygon(at, r, 4, #{ angle: glyph.rotation }), pen);
	} else if symbol == 1 {
		let dot = new_circle(at, r * 0.55);
		let gap = max(r * 0.3, 1.2);
		c.add(inks::solid(dot, gap), inks::fill(o.ink, gap));
		c.add(dot, pen);
	} else if symbol == 2 {
		let a = glyph.rotation;
		let along = vec2(a.cos(), a.sin()) * r;
		let across = vec2(-a.sin(), a.cos()) * r;
		c.add(new_path().append(new_line(at - along, at + along)).append(new_line(at - across, at + across)), pen);
	} else {
		c.add(new_path().append(new_circle(at, r)).append(new_circle(at, r * 0.5)), pen);
	}
	c
}
