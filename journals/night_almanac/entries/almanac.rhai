// The Night Almanac: a sample of the puzzle kit as finished pages, each a
// plate or two, a wall of the watcher's own account in an old hand
// wrapped round them, his notes in a quick cursive, and what age has done
// to the paper. The account and the notes are placeholders (content/):
// put your own in their place.
//
//   sightings   two sightings of three wandering stars, a year apart
//               (star_alignment, star_sightings): the night they'll stand
//               in one line is the secret

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/book" as book;

const INK = inks::black();
const BLOOD = inks::blood();

seed(1313);

// ---- sightings: where the three stood, two years running ----

let seen = new_lock("watch").add("stars", "star_alignment", #{ sightings: 2 })
	.run(#{ alignment: symbols(13, "JE") }).get("sightings");
// Every mark on the plates can be changed in the visual editor: the
// editor names them FIRST.PLATE.SLOT_N and LAST.PLATE.SLOT_N, and what it
// saves is in edits/almanac.json (here, the last plate's first corner).
let first = illustration("star_sightings", #{ size: 430.0, seed: 4, splatter: 0, editor: "first" }).run(#{ sightings: seen })[0];
let last = illustration("star_sightings", #{ size: 800.0, seed: 4, splatter: 0, editor: "last" }).run(#{ sightings: seen })[1];

let leaf = book::leaf("sightings", 13, #{ fold: "across", foxing: 150 });

// The account's wall, and what stands in it: nothing leaves a gutter too
// narrow to write in.
const LEFT = 50.0;
const RIGHT = 1150.0;
const TOP = 55.0;
const BOTTOM = 1545.0;
let first_at = vec2(64, 70);
let last_at = vec2(RIGHT - 813, 566);
book::place(leaf, first.translate_by(first_at), 11);
book::place(leaf, last.translate_by(last_at), 13);
book::jot(leaf, "The near one: five houses. It gains three every year.", vec2(524, TOP + 8), RIGHT - 540, #{ size: 36, turn: 0.02, margin: 0 });
book::jot(leaf, "The weeper gains five. The middle one loses one.", vec2(LEFT + 6, 556), 250, #{ size: 31, turn: -0.03, margin: 0 });
book::jot(leaf, "One line. All three. Count again.", vec2(LEFT + 6, 1400), 390, #{ size: 38, ink: BLOOD, turn: 0.02, margin: 0 });

// Pencil lines from plate to plate: the same sky, a year on.
let ruled = new_path();
ruled.append(new_line(first_at + vec2(215, 430), last_at + vec2(120, 0)));
ruled.append(new_line(first_at + vec2(430, 215), last_at + vec2(400, 0)));
ruled.append(new_line(first_at + vec2(430, 430), last_at + vec2(260, 0)));
leaf.under.add(ruled, inks::pencil(INK, 1.4));

// How each one creeps: its houses in a ring, and the step it takes in a
// year (thirteen months round an orbit of five is three houses on) drawn
// from house to house till it's back where it began.
for (period, k) in seen.value().periods {
	let at = vec2(600 + k * 210, 1452);
	let step = 13 % period;
	leaf.figures.add(new_circle(at, 56), inks::contour(INK, 1.4));
	leaf.figures.add(plates::ticks(at, period, 56, 66, 0, 0.0), inks::contour(INK, 1.6));
	leaf.figures.add(new_polygon(at, 54, period, #{ step: max(step, 1) }), inks::detail(BLOOD, 1.3));
	let star = art::star(at, 9.0, #{ kind: ["lashed", "spiral", "weeping", "needle", "banded"][k % 5], reach: 1.7, seed: 4 + 7 * k + 1, tears: 0 });
	leaf.figures.add(inks::star(star, INK, BLOOD, 9.0));
	leaf.figures.add(write_line(`${step} of ${period}`, at + vec2(-36, 92), book::note_hand(26.0, BLOOD)));
}
// The wheels' strip, under the last plate.
book::clear(leaf, new_rectangle(vec2((465 + RIGHT) / 2.0, (1385 + BOTTOM) / 2.0), RIGHT - 465, BOTTOM - 1385), 0);

journal_entry.add(book::bind(leaf, content("watch"), #{
	area: new_rectangle(vec2((LEFT + RIGHT) / 2.0, (TOP + BOTTOM) / 2.0), RIGHT - LEFT, BOTTOM - TOP),
	hand: book::old_hand(25.0),
}));
