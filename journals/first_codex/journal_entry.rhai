// The first codex: seven rings of balanced-ternary glyphs, the ring sizes
// running through the first primes up to 27. Each ring holds the zero glyph
// and then its letters from both ends of the alphabet, a.. and ..z, which
// balanced ternary maps to +1.. and ..-1.

let symbol_scale = 30.0;
let ring_spacing = symbol_scale * 3.0;
let ring_sizes = [2, 3, 5, 7, 11, 13, 27];
let alphabet = "abcdefghijklmnopqrstuvwxyz";
let fill_radius = symbol_scale * 0.75;

// The legacy renderer fit this page to 1080 px. It renders at its natural
// size now, so the nib grows by the same ratio to keep the look.
let outer_radius = 100.0 + (ring_sizes.len() - 1) * ring_spacing;
let nib = 1.75 * 2.0 * (outer_radius + fill_radius) / 1080.0;
let brush = new_brush("Calligraphy", 0.15, 0.25, nib);

let guide = new_strokestyle(2.5, "hsla(150, 30%, 50%, 0.1)", brush);
let ink = new_strokestyle(2.5, "rgba(0, 0, 0, 0.7)", brush);
let wash = new_strokestyle(2.0, "hsla(150, 30%, 70%, 0.3)", brush);

let font = ternary_font("balanced_ternary");
let origin = new_vec(0.0, 0.0);
let codex = new_canvas();

for (ring_size, ring) in ring_sizes {
	let radius = 100.0 + ring * ring_spacing;
	let half = ring_size / 2;
	let letters = " " + alphabet.sub_string(0, half) + alphabet.sub_string(alphabet.len() - half, half);

	// Five faint passes trace the ring.
	for pass in 0..5 {
		codex.add_strokepath(new_circle(origin, radius), guide);
	}

	// Golden-angle turns keep the rings from lining up.
	let rotation = ring * 137.5 * PI() / 180.0;
	let glyphs = font.glyphs(letters);
	for (g, i) in glyphs {
		let angle = i * 2.0 * PI() / glyphs.len() - PI() / 2.0 + rotation;
		let center = new_vec(radius * angle.cos(), radius * angle.sin());

		// A washed disc behind each glyph, hatched three ways.
		let disc = new_circle(center, fill_radius);
		for pass in 0..3 {
			codex.add_strokepath(disc.hatch(3.0, random_range(0.0, 2.0 * PI()), 3.0), wash);
		}

		g.scale = symbol_scale;
		g.position = center;
		g.rotation = angle + PI() / 2.0;
		codex.add_glyph(g, ink);
	}
}

let page = new_page();
page.name = "first_codex";
// The legacy page had no plain styling.
page.plain_style = ();
page.add_canvas(codex);
journal_entry.add_page(page);
