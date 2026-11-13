// The Night Almanac: a sample of the puzzle kit as finished pages, each a
// plate or two, a wall of the watcher's own account in an old hand
// wrapped round them, notes in a plainer and more hurried one, and what
// was spilt on it. The account and the notes are placeholders (content/):
// put your own in their place.
//
//   sightings   two sightings of three wandering stars, a year apart
//               (star_alignment, star_sightings): the night they'll stand
//               in one line is the secret

import "lib/inks" as inks;
import "lib/plates" as plates;

const W = 1200.0;
const H = 1600.0;
const INK = inks::black();
const BLOOD = inks::blood();

seed(1313);

// ---- hands ----

// The old hand the account is in: a glyph for a letter, in a pen that
// runs dry and is dipped again, so the wall of it is lighter and darker
// by turns.
const OLD = glyphset("eldritch");
fn old_hand(size) {
	#{
		style: new_style(new_brush_style(global::INK, "Rough Ink", size * 0.035, #{
			min_pressure: 0.35, max_pressure: 0.66, pressure_profile: "taper", pressure_wander: 0.15,
			pool: 0.25, pool_depth: 0.4, color_jitter: 0.05, ink_load: 0.36, redip_every: 70,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.04, glyph_position_jitter: [size * 0.03, size * 0.05], glyph_scale_jitter: 0.05, baseline_wander: size * 0.08 })),
		type: new_type_style(global::OLD, size, #{ glyph_tracking: 0.02 }),
		paragraph: new_paragraph_style(#{ line_height: 1.18, clearance: 5.0, line_start_jitter: size * 0.3, line_gap_jitter: size * 0.08 }),
	}
}

// The hurried hand of the notes: capitals in blood, gone over and over.
fn note_hand(size) {
	#{
		style: new_style(new_brush_style(global::BLOOD, "Rough Ink", size * 0.03, #{
			min_pressure: 0.4, max_pressure: 0.75, pressure_profile: "taper", color_jitter: 0.06,
		}), new_handwriting_style(#{ passes: 3, pass_end_jitter: size * 0.09, glyph_rotation_jitter: 0.07, glyph_position_jitter: size * 0.05, baseline_wander: size * 0.12 })),
		type: new_type_style(glyphset("plain"), size, #{ glyph_tracking: 0.03 }),
		paragraph: new_paragraph_style(#{ line_height: 1.25, line_start_jitter: size * 0.4 }),
	}
}

// A text as the old hand writes it: its letters and spaces alone.
fn old_text(text) {
	let out = "";
	let gap = false;
	for ch in text.to_lower().chars() {
		if ch >= 'a' && ch <= 'z' {
			if gap && out != "" { out += " "; }
			out += ch;
			gap = false;
		} else {
			gap = true;
		}
	}
	out
}

// Stains under everything: wet rings and patches, the colour of old tea.
fn stains(c, spots) {
	for spot in spots {
		let tea = `rgba(104, 66, 24, ${spot[3]})`;
		c.add(new_arc(vec2(spot[0], spot[1]), spot[2], 0.3, 5.4), inks::stain(tea, spot[2] * 0.11));
		c.add(new_arc(vec2(spot[0], spot[1]), spot[2] * 0.45, 2.0, 7.4), inks::stain(tea, spot[2] * 0.2));
	}
}

// A note in a box cleared for it, turned a little: #{ canvas, hole }.
fn note(text, at, width, height, size, turn) {
	let box = new_rectangle(at + vec2(width / 2.0, height / 2.0), width, height);
	let written = write_block(text, at + vec2(size * 0.4, size * 0.9), width - size * 0.8, note_hand(size)).canvas;
	#{ canvas: written.rotate_around(at + vec2(width / 2.0, height / 2.0), turn), hole: box }
}

// ---- sightings: where the three stood, two years running ----

let watch = new_lock("watch").add("stars", "star_alignment", #{ sightings: 2 });
let seen = watch.run(#{ "stars.alignment": symbols(13, "JE") }).get("stars.sightings");
// Every mark on the plates can be changed in the visual editor: the
// editor names them FIRST.PLATE.SLOT_N and LAST.PLATE.SLOT_N, and what it
// saves is in edits/almanac.json (here, the last plate's first corner).
let first = illustration("star_sightings", #{ size: 430.0, seed: 4, splatter: 0, editor: "first" }).run(#{ sightings: seen })[0];
let last = illustration("star_sightings", #{ size: 800.0, seed: 4, splatter: 16, editor: "last" }).run(#{ sightings: seen })[1];

let page = new_canvas();
stains(page, [[980, 300, 150, 0.2], [210, 1180, 190, 0.17], [900, 1480, 120, 0.15]]);

// The account's wall, and what stands in it: nothing leaves a gutter too
// narrow to write in.
const LEFT = 50.0;
const RIGHT = 1150.0;
const TOP = 55.0;
const BOTTOM = 1545.0;
let first_at = vec2(64, 70);
let last_at = vec2(RIGHT - 813, 566);
let notes = [
	note("THE NEAR ONE. FIVE HOUSES. IT GAINS THREE EVERY YEAR", vec2(507, TOP), RIGHT - 507, 150, 34, 0.025),
	note("THE WEEPER GAINS FIVE. THE MIDDLE ONE LOSES ONE", vec2(LEFT, 540), 270, 250, 31, -0.035),
	note("ONE LINE. ALL THREE. COUNT AGAIN", vec2(LEFT, 1392), 410, BOTTOM - 1392, 36, 0.025),
];

// How each one creeps: its houses in a ring, and the step it takes in a
// year (thirteen months round an orbit of five is three houses on) drawn
// from house to house till it's back where it began.
let seen_data = seen.value();
let wheels = [];
for (period, k) in seen_data.periods {
	wheels.push(#{ at: vec2(600 + k * 210, 1452), period: period, step: 13 % period });
}

// The account, walled round the plates and the notes.
let wall = new_rectangle(vec2((LEFT + RIGHT) / 2.0, (TOP + BOTTOM) / 2.0), RIGHT - LEFT, BOTTOM - TOP);
wall.append(new_rectangle(first_at + vec2(215, 215), 430 + 22, 430 + 22));
wall.append(new_rectangle(last_at + vec2(400, 400), 800 + 26, 800 + 26));
for n in notes { wall.append(n.hole); }
// The wheels' strip, under the last plate.
wall.append(new_rectangle(vec2((465 + RIGHT) / 2.0, (1385 + BOTTOM) / 2.0), RIGHT - 465, BOTTOM - 1385));
let account = old_text(content("watch"));
page.add(write_inside_path(account + " " + account, wall, old_hand(25.0)).canvas);

// Pencil lines from plate to plate: the same sky, a year on.
let ruled = new_path();
ruled.append(new_line(first_at + vec2(215, 430), last_at + vec2(120, 0)));
ruled.append(new_line(first_at + vec2(430, 215), last_at + vec2(400, 0)));
ruled.append(new_line(first_at + vec2(430, 430), last_at + vec2(260, 0)));
page.add(ruled, inks::pencil(INK, 1.4));

page.add(first.translate_by(first_at));
page.add(last.translate_by(last_at));
for n in notes { page.add(n.canvas); }
for (w, k) in wheels {
	page.add(new_circle(w.at, 56), inks::contour(INK, 1.4));
	page.add(plates::ticks(w.at, w.period, 56, 66, 0, 0.0), inks::contour(INK, 1.6));
	page.add(new_polygon(w.at, 54, w.period, #{ step: max(w.step, 1) }), inks::detail(BLOOD, 1.3));
	let star = art::star(w.at, 9.0, #{ kind: ["lashed", "spiral", "weeping", "needle", "banded"][k % 5], reach: 1.7, seed: 4 + 7 * k + 1, tears: 0 });
	page.add(inks::solid(star.dark, 2.0), inks::fill(INK, 2.0));
	page.add(star.body, inks::contour(INK, 0.6));
	page.add(star.rays, inks::fine(INK, 0.5));
	page.add(write_line(`+${w.step} OF ${w.period}`, w.at + vec2(-58, 86), note_hand(24.0)));
}
// What was spilt, across the wall and the first plate's edge.
for lot in plates::splatter(vec2(560, 430), 210.0, 26.0, 16, 77) {
	page.add(lot.path, inks::band(BLOOD, lot.size));
}

journal_entry.add(new_page(#{ name: "sightings", size: vec2(W, H) }).add(page));
