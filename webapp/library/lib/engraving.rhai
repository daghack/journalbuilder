// Darks as an engraver lays them: shapes massed into solid ink with holes
// left bare (masses, darks, a mezzotint's velvet), shadow sides hatched
// (shade), and water cut at night, level lines laid so close they close
// up black, with the moon's road and whatever else catches the light left
// bare in it.
// import "lib/engraving" as engraving;
//
//   let pool = shapes::closed(shapes::ellipse_points(vec2(600, 900), 300, 80, 80));
//   let road = engraving::moon_road(600.0, 840.0, 960.0, 60.0, 7);
//   engraving::black_water(page, pool, road, ink);
//   engraving::mass(page, rock, shapes::sliver(ridge, 4.0), ink, 1.2);

import "lib/inks" as inks;
import "lib/chance" as chance;
import "lib/shapes" as shapes;

// `outline`, shapes (a path of closed subpaths, each wound clockwise), as
// one mass of solid `ink`, merged where they overlap, with `holes` (a
// path) left bare, added to c. gap is how close it's hatched, and so how
// coarse its edge.
fn mass(c, outline, holes, ink, gap) {
	let lines = outline.hatch(gap, #{ angle: [0.6, 0.6 + PI() / 2.0], roughness: gap * 0.4, fill_rule: "nonzero" });
	if !holes.is_empty { lines.erase_inside(holes, #{ fill_rule: "nonzero" }); }
	c.add(lines, new_style(new_brush_style(ink, "Rough Ink", gap / 2.5, #{ min_pressure: 0.8, max_pressure: 0.95, pressure_profile: "constant", color_jitter: 0.04 }), new_handwriting_style()));
}

// A closed shape filled with solid `ink`, added to c: hatched close two
// ways and stroked as wide as the gap, so it closes up ragged-edged. gap
// (3 is usual) is how coarse.
fn dark(c, region, ink, gap) {
	c.add(inks::solid(region, gap), inks::fill(ink, gap));
}

// Hatching across `region`, `gap` apart, as region.hatch lays it (o:
// angle, roughness, fill_rule, as hatch takes them), with its lit side
// left bare: o.lit, a vec toward the light, leaves bare what the region
// moved that far covers, so the hatching lies along its shadow side, a
// crescent of it. Gives the path, to add in a pen.
fn shade(region, gap) { shade(region, gap, #{}) }
fn shade(region, gap, o) {
	let hatch = o;
	let lit = hatch.remove("lit");
	let lines = region.hatch(gap, hatch);
	if lit != () { lines.erase_inside(region.copy().translate_by(lit), #{ fill_rule: "nonzero" }); }
	lines
}

// The velvet black of a mezzotint, laid along a shape's hatching, in
// `ink`: for the deepest darks, the voids of arches and the yew.
fn velvet(c, region, ink, gap) {
	c.add(region.hatch(gap, #{ angle: [1.2], roughness: gap * 0.5 }), new_style(new_brush_style(ink, "Mezzotint", gap * 0.9, #{
		min_pressure: 0.7, max_pressure: 0.95, pressure_profile: "constant",
	}), new_handwriting_style()));
	dark(c, region, ink, gap * 0.8);
}

// Water as the engraver cuts it at night: level lines `gap` apart across
// `region` (a closed path), wavering a little, laid so close they close
// up black in `ink`; `holes` (a path) left bare: the moon's road, a
// ripple's rim, a reflection. o: gap (2.4), wave (0.8, how far the lines
// waver), weight (2.1, how many times thinner than the gap the lines are
// drawn), seed (0).
fn black_water(c, region, holes, ink) { black_water(c, region, holes, ink, #{}) }
fn black_water(c, region, holes, ink, o) {
	let gap = o.gap ?? 2.4;
	let wave = o.wave ?? 0.8;
	let s = (o.seed ?? 0) * 1.0;
	let b = region.bounds();
	let lines = new_path();
	let n = ((b.max.x - b.min.x) / 24.0).to_int() + 2;
	let y = b.min.y + gap * 0.5;
	while y < b.max.y {
		let pts = [];
		for j in 0..=n {
			let x = b.min.x - 4.0 + (b.max.x - b.min.x + 8.0) * j / n;
			pts.push(vec2(x, y + wave * (x * 0.031 + y * 0.17 + s).sin() + wave * 0.5 * (x * 0.083 - y * 0.05).sin()));
		}
		lines.append(new_polyline(pts));
		y += gap;
	}
	lines.erase_outside(region, #{ fill_rule: "nonzero" });
	if !holes.is_empty { lines.erase_inside(holes, #{ fill_rule: "nonzero" }); }
	c.add(lines, new_style(new_brush_style(ink, "Rough Ink", gap / (o.weight ?? 2.1), #{ min_pressure: 0.75, max_pressure: 0.95, pressure_profile: "constant", color_jitter: 0.03 }), new_handwriting_style()));
}

// The moon's road on still water: short level glints in a column under
// x from y0 down to y1, w across far off and wider near, as slivers to
// leave bare in black water. o.split (0): a half-width about x left dark,
// where something standing in the water throws its reflection.
fn moon_road(x, y0, y1, w, seed) { moon_road(x, y0, y1, w, seed, #{}) }
fn moon_road(x, y0, y1, w, seed, o) {
	let split = o.split ?? 0.0;
	let road = new_path();
	let y = y0;
	let k = 0;
	while y < y1 {
		let f = (y - y0) / (y1 - y0);
		let half = w * (0.5 + 0.9 * f) * (0.35 + 0.65 * chance::of(seed, k, 1));
		let mid = x + w * 0.35 * chance::either_way(seed, k, 2);
		let thick = 3.0 + 4.0 * f;
		if split > 0.0 && mid - half < x + split && mid + half > x - split {
			if mid - half < x - split - 6.0 { road.append(shapes::sliver([vec2(mid - half, y), vec2((mid - half + x - split) / 2.0, y + 0.3), vec2(x - split, y)], thick)); }
			if mid + half > x + split + 6.0 { road.append(shapes::sliver([vec2(x + split, y), vec2((mid + half + x + split) / 2.0, y + 0.3), vec2(mid + half, y)], thick)); }
		} else {
			road.append(shapes::sliver([vec2(mid - half, y), vec2(mid, y + 0.3), vec2(mid + half, y)], thick));
		}
		y += 6.0 + 12.0 * f * (0.5 + chance::of(seed, k, 3));
		k += 1;
	}
	road
}
