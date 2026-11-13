// dread_stars: five stars that aren't right, a kind to a symbol: an eye in
// lashes (0), a spiral (1), a thing that weeps (2), a burst of needles (3)
// and a banded disc (4). Each is art::star, its dark filled, its washes and
// tears in blood and the rest in ink, sized to sit in its glyph's box (its
// rays reach as far as the box does). The glyph's turn turns the star.
// Options: ink and blood (its two inks), seed (which stars they are, the
// same for a symbol whatever its place), reach (how far the rays go beyond
// the body, in its radii, from 1: more is spikier, and a smaller body).

import "lib/inks" as inks;

fn options() {
	#{ ink: "#2b1d12", blood: "#9b2318", seed: 0, reach: 1.75 }
}

fn symbols(o) {
	#{ glyphset: "plain", glyph: "O", count: 5, size: 48.0 }
}

fn draw(o, symbol, glyph) {
	let kinds = ["lashed", "spiral", "weeping", "needle", "banded"];
	let r = glyph.scale * 0.5 / o.reach;
	let star = art::star(glyph.position, r, #{ kind: kinds[symbol], reach: o.reach, angle: glyph.rotation, seed: o.seed + 7 * symbol + 1 });
	let c = new_canvas();
	if !star.wash.is_empty { c.add(star.wash, inks::wash(o.blood, max(r * 0.12, 1.0))); }
	if !star.dark.is_empty {
		let gap = max(r * 0.08, 2.0);
		c.add(inks::solid(star.dark, gap), inks::fill(o.ink, gap));
	}
	c.add(star.rays, inks::fine(o.ink, max(r * 0.014, 0.5)));
	c.add(star.body, inks::contour(o.ink, max(r * 0.022, 0.6)));
	c.add(star.dots, inks::detail(o.blood, max(r * 0.016, 0.5)));
	c.add(star.tears, inks::contour(o.blood, max(r * 0.024, 0.7)));
	c
}
