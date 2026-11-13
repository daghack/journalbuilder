// star_sightings: Sightings drawn as plates of the sky, one to each
// sighting: what star_alignment hides a date in. At the middle is the
// star the others circle; round it, banded dark and light, each wandering
// star's orbit, a tick at every house, and the star in its house that
// month, each star a kind of its own so it's known from plate to plate
// (an eye in lashes, a spiral, a thing that weeps, a burst of needles, a
// banded disc, from the innermost out). One line runs from the middle up
// through every orbit's first house: where they'll all stand together.
//
// Round the orbits is the year, a band of thirteen months marked off in
// weeks, the sighting's month filled in, and round that the months'
// glyphs, the sighting's in blood. Under the sky, to the left, is the
// year in the same glyphs, and to the right the month again. A star at
// each corner and a ruled frame finish the plate.
//
// Each canvas is size across and down. Every family of marks is a symbol
// set and the symbols that go in its places, set by wire or by hand (the
// design slots):
//   markers_set, wanderers, centre   the stars: a wanderer each (symbols,
//                                    default 0, 1, 2 ... of the set) and the
//                                    one they circle (default 4); the
//                                    unwired set is dread_stars, whose five
//                                    are an eye in lashes, a spiral, a
//                                    thing that weeps, a burst of needles
//                                    and a banded disc
//   corners_set, corners             the four corners' marks (default 1 4 1
//                                    4 of dread_stars)
//   months_set                       the thirteen months' glyphs round the
//                                    rim, and the year and month under the
//                                    sky: unwired, the priests'
//   month_marks_set, month_marks     the marks between the months (13,
//                                    default all 0 of small_marks, a lozenge)
// Each place's symbol is checked against its set. With editor, each mark is
// committed to the visual editor as EDITOR.PLATE.SLOT_N (sky.0.corners_2:
// the third corner of the first plate), so it can be changed there; two
// uses in one entry need two editors. The rest is drawn in ink and blood,
// lib/inks' pens; seed picks how each blot is drawn, and splatter how much
// ink was spilt on each plate.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/chance" as chance;

fn options() {
	#{ size: 900.0, seed: 0, splatter: 14, editor: "", ink: "#2b1d12", blood: "#9b2318" }
}

fn inputs(o) {
	#{
		sightings: "Sightings",
		markers_set: "SymbolSet", wanderers: "Any", centre: "Any",
		corners_set: "SymbolSet", corners: "Any",
		months_set: "SymbolSet",
		month_marks_set: "SymbolSet", month_marks: "Any",
	}
}

fn fallbacks(o) {
	let stars = symbol_set("dread_stars", #{ ink: o.ink, blood: o.blood });
	#{
		markers_set: stars, wanderers: [], centre: [],
		corners_set: stars, corners: [],
		months_set: symbol_set("priests"),
		month_marks_set: symbol_set("small_marks", #{ ink: o.ink }), month_marks: [],
	}
}

fn render(o, i) {
	let seen = i.sightings;
	if type_of(seen) != "map" || !("periods" in seen) || !("sightings" in seen) {
		throw "star_sightings: sightings should be #{ periods, sightings }, as star_alignment gives";
	}
	let plates = [];
	for (sighting, n) in seen.sightings {
		plates.push(plate(o, i, seen.periods, sighting, n));
	}
	plates
}

fn example() {
	// Three stars, of periods 5, 7 and 8, a year apart: they align in
	// year 9, month 4.
	#{ sightings: #{ periods: [5, 7, 8], sightings: [
		#{ date: [2, 12], houses: [2, 1, 5] },
		#{ date: [3, 12], houses: [0, 0, 2] },
	] } }
}

// A mark of `set`, symbol `symbol`, `size` across, at `at`, turned, in
// `style` over the set's unless (); committed to the visual editor as
// EDITOR.PLATE.NAME if the editor option names one.
fn mark(c, o, plate, name, set, symbols, at, size, turn, style) {
	let settings = #{ type: #{ glyph_scale: size }, turn: turn };
	if style != () { settings.style = style; }
	let m = write_at(set.text(symbols), at, settings);
	if o.editor != "" { m = m.commit_to_editor(`${o.editor}.${plate}.${name}`); }
	c.add(m);
}

// One sighting's plate.
fn plate(o, i, periods, sighting, n) {
	let s = o.size;
	let centre = vec2(s / 2.0, s / 2.0);
	let stars = periods.len();
	let month = sighting.date[sighting.date.len() - 1];
	let months = i.months_set;
	if months.base < 13 { throw `star_sightings: months_set '${months.name}' has ${months.base} symbols; the thirteen months need 13`; }
	let base = i.markers_set.base;
	let wanderers = plates::slot(i.wanderers, stars, i.markers_set, "wanderers", |k| k % base);
	let hub = plates::slot(i.centre, 1, i.markers_set, "centre", |k| 4 % base)[0];
	let corner_base = i.corners_set.base;
	let corner_symbols = plates::slot(i.corners, 4, i.corners_set, "corners", |k| [1, 4, 1, 4][k] % corner_base);
	let between = plates::slot(i.month_marks, 13, i.month_marks_set, "month_marks", |k| 0);

	// From the middle out: the orbits, a wall, the year's band, the
	// months' glyphs, the rim.
	let inmost = s * 0.1;
	let wall = s * 0.328;
	let band_in = s * 0.34;
	let band_out = s * 0.392;
	let rim = s * 0.455;
	let apart = (wall - s * 0.012 - inmost) / stars;
	let orbit = |k| inmost + apart * (k + 0.5);

	// Where each wandering star is, and the disc cleared for it.
	let medal = apart * 0.62;
	let places = [];
	let cleared = new_path();
	// The night's bands are broad: cut wider, so none of one shows inside.
	let cleared_wide = new_path();
	for k in 0..stars {
		let at = centre + plates::toward(sighting.houses[k] * 1.0 / periods[k]) * orbit.call(k);
		places.push(at);
		cleared.append(new_circle(at, medal));
		cleared_wide.append(new_circle(at, medal + apart * 0.2));
	}
	let middle = inmost * 0.84;
	// The way in: a channel up through the night, from the middle to the wall.
	let way_in = new_rectangle(centre + vec2(0, -(wall + middle) / 2.0), s * 0.022, wall - middle + s * 0.02);

	// In blood, under the ink: each orbit's line, the year's weeks, the
	// sighting's month filled in, and the line they'll all stand on.
	let red = new_path();
	for k in 0..stars { red.append(new_circle(centre, orbit.call(k))); }
	red.erase_inside(cleared);
	let c = new_canvas();
	c.add(red, inks::detail(o.blood, s * 0.0011));
	// The year's days, 28 to a month, every seventh a week's.
	c.add(plates::ticks(centre, 364, band_in + s * 0.005, band_in + (band_out - band_in) * 0.55, 7, band_out - s * 0.005), inks::fine(o.blood, s * 0.0011));
	let filled = new_path()
		.move_to(centre + plates::toward(month / 13.0) * band_in)
		.line_to(centre + plates::toward(month / 13.0) * band_out)
		.line_to(centre + plates::toward((month + 0.5) / 13.0) * band_out)
		.line_to(centre + plates::toward((month + 1.0) / 13.0) * band_out)
		.line_to(centre + plates::toward((month + 1.0) / 13.0) * band_in)
		.line_to(centre + plates::toward((month + 0.5) / 13.0) * band_in)
		.close();
	c.add(inks::solid(filled, s * 0.004), inks::fill(o.blood, s * 0.004));

	// The night between the orbits: broad dark bands, cut where a star is.
	let night = new_path();
	for k in 0..stars + 1 { night.append(new_circle(centre, inmost + apart * k)); }
	night.erase_inside(cleared_wide);
	night.erase_inside(way_in);
	c.add(night, inks::band(o.ink, apart * 0.3 / 3.0));
	// Every orbit's houses, the first the longest.
	let houses = new_path();
	for k in 0..stars {
		houses.append(plates::ticks(centre, periods[k], orbit.call(k) - apart * 0.2, orbit.call(k) + apart * 0.2, periods[k], orbit.call(k) + apart * 0.3));
	}
	houses.erase_inside(cleared);
	houses.erase_inside(way_in);
	c.add(houses, inks::contour(o.ink, s * 0.002));
	// The line they'll all stand on, up the way in.
	let line = new_line(centre + vec2(0, -middle), centre + vec2(0, -band_in));
	line.erase_inside(cleared);
	c.add(line, inks::contour(o.blood, s * 0.003));

	// The wall, the year's band and the rim; the months marked off.
	let heavy = new_path().append(new_circle(centre, band_in)).append(new_circle(centre, band_out)).append(new_circle(centre, rim));
	c.add(heavy, inks::band(o.ink, s * 0.0022));
	c.add(new_circle(centre, rim + s * 0.012), inks::detail(o.ink, s * 0.0012));
	c.add(plates::ticks(centre, 13, band_in, band_out, 0, 0.0), inks::contour(o.ink, s * 0.002));
	c.add(plates::ticks(centre, 13, band_out, band_out + s * 0.014, 0, 0.0), inks::detail(o.ink, s * 0.0012));

	// The months' glyphs, the sighting's in blood, and between them a mark
	// where each month gives way to the next.
	let size = rim - band_out;
	let glyph_at = (band_out + rim) / 2.0;
	for k in 0..13 {
		let turn = (k + 0.5) / 13.0;
		mark(c, o, n, `month_${k}`, months, [k], centre + plates::toward(turn) * glyph_at, size, 2.0 * PI() * turn, if k == month { #{ color: o.blood } } else { () });
		mark(c, o, n, `month_mark_${k}`, i.month_marks_set, [between[k]], centre + plates::toward(k / 13.0) * glyph_at, s * 0.016, 2.0 * PI() * k / 13.0, ());
	}

	// The star they circle, and the wanderers, each in its cleared disc.
	mark(c, o, n, "centre", i.markers_set, [hub], centre, middle * 1.75, 0.0, ());
	for k in 0..stars {
		c.add(new_circle(places[k], medal), inks::contour(o.ink, s * 0.0018));
		mark(c, o, n, `wanderer_${k}`, i.markers_set, [wanderers[k]], places[k], medal * 1.82, 0.0, ());
	}

	// A glory of fine rays out from the rim, as far as the frame lets
	// them, clear of the corners' stars and the date.
	let glory = new_path();
	for k in 0..420 {
		let way = plates::toward((k + chance::of(o.seed, k, 70) * 0.6) / 420.0);
		let long = s * (0.02 + 0.17 * chance::of(o.seed, k, 71) ** 1.6);
		glory.append(new_line(centre + way * (rim + s * 0.024), centre + way * (rim + s * 0.024 + long)));
	}
	let inside = s * 0.04;
	glory.erase_outside(new_rectangle(centre, s - 2.0 * inside, s - 2.0 * inside));
	let kept = new_path();
	for corner in [[-1, -1], [1, -1], [1, 1], [-1, 1]] {
		kept.append(new_circle(centre + vec2(corner[0], corner[1]) * s * 0.415, s * 0.055));
	}
	for side in [-1, 1] {
		kept.append(new_rectangle(centre + vec2(side * s * 0.285, s * 0.418), s * 0.13, s * 0.075));
	}
	glory.erase_inside(kept);
	c.add(glory, inks::fine(o.ink, s * 0.0011));

	// A star at each corner, and pencil lines from corner to rim.
	let ruled = new_path();
	for (corner, q) in [[-1, -1], [1, -1], [1, 1], [-1, 1]] {
		let at = centre + vec2(corner[0], corner[1]) * s * 0.415;
		let way = (centre - at).normalized();
		ruled.append(new_line(at + way * s * 0.06, centre - way * (rim + s * 0.02)));
		c.add(new_circle(at, s * 0.047), inks::contour(o.ink, s * 0.0016));
		mark(c, o, n, `corner_${q}`, i.corners_set, [corner_symbols[q]], at, s * 0.091, 0.0, ());
	}
	c.add(ruled, inks::pencil(o.ink, s * 0.0016));

	// The year, lower left, and the month again, lower right.
	let year = sighting.date.extract(0, sighting.date.len() - 1);
	mark(c, o, n, "year", months, year, vec2(centre.x - s * 0.285, centre.y + s * 0.418), size * 0.9, 0.0, #{ color: o.blood });
	mark(c, o, n, "month", months, [month], vec2(centre.x + s * 0.285, centre.y + s * 0.418), size * 0.9, 0.0, #{ color: o.blood });

	// The frame, and what was spilt.
	let frame = plates::frame(s, s, s * 0.018, s * 0.014, s * 0.03);
	c.add(frame.rules, inks::contour(o.ink, s * 0.0016));
	c.add(frame.ticks, inks::detail(o.ink, s * 0.001));
	if o.splatter > 0 {
		let a = 2.0 * PI() * chance::of(o.seed, n, 60);
		let from = centre + vec2(a.cos(), a.sin()) * s * (0.25 + 0.2 * chance::of(o.seed, n, 61));
		for lot in plates::splatter(from, s * 0.22, s * 0.022, o.splatter, o.seed + n * 13) {
			c.add(lot.path, inks::band(o.blood, lot.size));
		}
	}
	c
}
