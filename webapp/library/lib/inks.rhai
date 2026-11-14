// The puzzle plates' pens: a few ways of putting ink down, shared so the
// plates look like one hand's work. Each gives a style in the ink it's
// handed (a colour, a gradient), sized in canvas units as brush styles
// are: a Rough Ink brush of size 1 draws a line about 3 across at full
// pressure. At the end, the art tools' moons, stars and eyes inked with
// them.
// import "lib/inks" as inks;

// The plates' two inks: iron-gall near-black, and blood.
fn black() { "#2b1d12" }
fn blood() { "#9b2318" }

// A hand that goes over each line `passes` times, the goings-over ending
// up to `stray` apart, every line reshaped by up to `warp` along its way.
fn hand(passes, stray, warp) {
	new_handwriting_style(#{ passes: passes, pass_end_jitter: stray, stroke_warp: warp, stroke_warp_length: 60.0 })
}

// Structure: the lines a drawing stands on. Rough ink, swelling to its
// middle and thinning away, pooling at the foot of each stroke, gone over
// twice by a hand that doesn't quite retrace itself.
fn contour(ink, size) {
	new_style(new_brush_style(ink, "Rough Ink", size, #{
		min_pressure: 0.25, max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15,
		pool: 0.4, pool_depth: 0.4, color_jitter: 0.03,
	}), hand(2, size * 1.6, size * 1.2))
}

// A broad, heavy stroke for bands and masses of dark: rough ink pressed
// hard its whole length, gone over twice.
fn band(ink, size) {
	new_style(new_brush_style(ink, "Rough Ink", size, #{
		min_pressure: 0.6, max_pressure: 0.88, pressure_profile: "constant", pressure_wander: 0.12, color_jitter: 0.04,
	}), hand(2, size * 0.35, size * 0.3))
}

// Detail: ticks, small rings, the lines inside things. An ink pen, light
// to heavy along each stroke, once, a little unsteady.
fn detail(ink, size) {
	new_style(new_brush_style(ink, "Ink Pen", size, #{
		min_pressure: 0.3, max_pressure: 0.75, pressure_wander: 0.25, color_jitter: 0.03,
	}), hand(1, 0.0, size * 0.8))
}

// The finest line: hatching, hairs, rays. An ink pen barely touching,
// thinning to nothing at both ends.
fn fine(ink, size) {
	new_style(new_brush_style(ink, "Ink Pen", size, #{
		min_pressure: 0.15, max_pressure: 0.55, pressure_profile: "taper", pressure_wander: 0.3,
	}), hand(1, 0.0, size * 0.5))
}

// A dark fill's pen: for the hatching `solid` gives, a stroke as wide as
// its lines are apart, so they close up into one mass, ragged at the edge.
fn fill(ink, gap) {
	new_style(new_brush_style(ink, "Rough Ink", gap / 3.2, #{
		min_pressure: 0.75, max_pressure: 0.95, pressure_profile: "constant", color_jitter: 0.05,
	}), hand(1, 0.0, 0.0))
}

// Construction lines: a pencil's, faint, ruled once.
fn pencil(ink, size) {
	new_style(new_brush_style(ink, "Pencil", size, #{ min_pressure: 0.3, max_pressure: 0.6, pressure_profile: "constant" }), hand(1, 0.0, 0.0))
}

// A wash of colour under the lines: watercolour, thin, feathering out.
fn wash(ink, size) {
	new_style(new_brush_style(ink, "Smooth Watercolor", size, #{ min_pressure: 0.15, max_pressure: 0.35, pressure_profile: "taper" }), hand(1, 0.0, 0.0))
}

// A stain: rough watercolour laid on wet, blotching at its edges.
fn stain(ink, size) {
	new_style(new_brush_style(ink, "Rough Watercolor", size, #{ min_pressure: 0.2, max_pressure: 0.5, pressure_profile: "taper", pressure_wander: 0.4 }), hand(1, 0.0, 0.0))
}

// Soot: charcoal, grainy, for shadow that isn't ink.
fn soot(ink, size) {
	new_style(new_brush_style(ink, "Charcoal", size, #{ min_pressure: 0.25, max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.3 }), hand(1, 0.0, 0.0))
}

// A closed shape's inside as hatching `gap` apart, two ways across, to
// draw with `fill(ink, gap)`: a mass of dark. Shapes inside others cut
// holes in them, as path.hatch has it.
fn solid(shape, gap) {
	shape.hatch(gap, #{ angle: [0.6, 0.6 + PI() / 2.0], roughness: gap * 0.4 })
}

// ---- the art tools' drawings, inked ----

// An art::moon, inked, as a canvas: its dark side a solid mass, its
// outline and terminator in contour `size` wide. o.gap is the dark's
// hatching gap (size * 3).
fn moon(m, ink, size) { moon(m, ink, size, #{}) }
fn moon(m, ink, size, o) {
	let gap = if "gap" in o { o.gap } else { size * 3.0 };
	let c = new_canvas();
	if !m.dark.is_empty { c.add(solid(m.dark, gap), fill(ink, gap)); }
	c.add(m.outline, contour(ink, size));
	if !m.terminator.is_empty { c.add(m.terminator, contour(ink, size)); }
	c
}

// An art::star of radius `r`, inked, as a canvas: its wash, dots and tears
// in `blood`, its dark a solid mass, its rays fine, its body in contour,
// each as wide as the star's size calls for.
fn star(s, ink, blood, r) {
	let c = new_canvas();
	if !s.wash.is_empty { c.add(s.wash, wash(blood, max(r * 0.12, 1.0))); }
	if !s.dark.is_empty {
		let gap = max(r * 0.08, 2.0);
		c.add(solid(s.dark, gap), fill(ink, gap));
	}
	c.add(s.rays, fine(ink, max(r * 0.014, 0.5)));
	c.add(s.body, contour(ink, max(r * 0.022, 0.6)));
	c.add(s.dots, detail(blood, max(r * 0.016, 0.5)));
	c.add(s.tears, contour(blood, max(r * 0.024, 0.7)));
	c
}

// An art::eye, inked, as a canvas: its iris washed and ringed in `iris`,
// its lids and lashes in `ink`, the lines about `size` wide.
fn eye(e, ink, iris, size) {
	let c = new_canvas();
	c.add(e.wash, wash(iris, size * 10.0));
	c.add(e.rim, detail(iris, max(size, 0.6)));
	c.add(e.accent, detail(iris, max(size, 0.6)));
	c.add(e.lines, contour(ink, max(size, 0.8)));
	c
}
