// The first codex: seven rings of balanced-ternary glyphs, the ring sizes
// running through the first primes up to 27. Each ring holds the zero glyph
// and then its letters from both ends of the alphabet, a.. and ..z, which
// balanced ternary maps to +1.. and ..-1.

let size = 30.0;
let ring_spacing = size * 3.0;
let ring_sizes = [2, 3, 5, 7, 11, 13, 27];
let alphabet = "abcdefghijklmnopqrstuvwxyz";
let fill_radius = size * 0.75;

// Nib sizes are in canvas units, like everything else; 2.15 keeps the look
// of the legacy render (which fit this page to 1080 px with a 1.75 nib).
let brush = new_brush("Calligraphy", 0.15, 0.25, 2.15);

// Five faint passes trace each ring.
let guide = new_style("hsla(150, 30%, 50%, 0.1)", brush, #{ width: 2.5, passes: 5 });
let ink = new_style("rgba(0, 0, 0, 0.7)", brush, #{ width: 2.5 });
let wash = new_style("hsla(150, 30%, 70%, 0.3)", brush, #{ width: 2.0 });

let font = ternary_font("balanced_ternary");
let codex = new_canvas();

for (ring_size, ring) in ring_sizes {
	let circle = new_circle(vec2(0.0, 0.0), 100.0 + ring * ring_spacing);
	let half = ring_size / 2;
	let letters = " " + alphabet.sub_string(0, half) + alphabet.sub_string(alphabet.len() - half, half);
	codex.add(circle, guide);

	// From the top, with golden-angle turns to keep the rings from lining
	// up; each glyph turned to the ring.
	let turn = ring * 137.5 / 360.0;
	let glyphs = font.glyphs(letters);
	for (at, i) in circle.points_along(glyphs.len(), #{ start: (0.75 + turn) % 1.0 }) {
		// A washed disc behind each glyph, hatched three ways.
		let disc = new_circle(at.position, fill_radius);
		for pass in 0..3 {
			codex.add(disc.hatch(3.0, #{ angle: random_range(0.0, 2.0 * PI()), roughness: 3.0 }), wash);
		}
		let g = glyphs[i];
		g.scale = size;
		g.position = at.position;
		g.rotation = at.angle;
		codex.add(g, ink);
	}
}

// The legacy page had no SVG styling.
journal_entry.add(new_page(#{ name: "first_codex", svg_style: () }).add(codex));
