// The index of the thirteen (steiner_sieve, triads): the watcher's copy
// of every three of the thirteen he saw rise together, each a seal of three
// medallions with a letter and a number, walled in his account. Every two
// points are bound together in one true seal and only one; the rest are
// lies. Gather the true seals and read their letters by number.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/book" as book;

const INK = inks::black();
const BLOOD = inks::blood();
const SEPIA = book::sepia();

seed(1515);

// The secret, a letter to each of the system's 26 triples.
let heap = new_lock("index").add("s", "steiner_sieve", #{ seed: 5, decoys: 10 })
	.run(#{ plain_text: symbols(27, "COUNT THE HOUSES AGAIN NOW") }).get("cards").value();

let leaf = book::leaf("index", 15, #{ foxing: 170 });

// ---- the table of the thirteen ----
// A banded disc: its rules, the days, the rule written round, the thirteen
// in their medallions, the thirteen-pointed star three ways, and what
// watches at the heart.
let centre = vec2(330, 335);
book::construction(leaf.under, centre, 300, 292, SEPIA);
let table = illustration("banded_disc", #{ size: 540.0, seed: 17, editor: "table" })
	.run(#{ inscription: "every two of them once never none never twice", text_style: book::loud_hand(22, INK) })[0];
book::place(leaf, table.translate_by(centre - vec2(270, 270)), 4);

// ---- the ribbon ----
let band = book::ribbon([vec2(640, 30), vec2(668, 210), vec2(645, 430), vec2(560, 610), vec2(380, 690), vec2(180, 680), vec2(30, 700)], 46.0,
	"the true ones keep the rule and the liars break it", book::loud_hand(28, INK), INK);
leaf.figures.add(band.canvas);
book::clear(leaf, band.hole, 4);

// ---- his note ----
book::jot(leaf, "Twenty-six of them are true. The rest are liars, and they lie in the way I can't yet see. Only one seal binds any two. Read the true ones by their numbers, the least first.",
	vec2(720, 70), 420, #{ size: 27, turn: 0.02, margin: 6 });

// ---- one seal, worked out ----
// A triple on the ring of thirteen, as he worked it: the ring, its places,
// the three joined, and the lines he ruled to find them.
let worked = vec2(940, 500);
book::construction(leaf.under, worked, 150, 0.0, SEPIA);
plates::double_ring(leaf.figures, worked, 104, 6, INK, 1.6);
leaf.figures.add(plates::ticks(worked, 13, 98, 112, 0, 0.0), inks::detail(INK, 1.0));
let corners = plates::around(worked, 98, 13);
let three = [0, 3, 9];
let tri = new_path().move_to(corners[three[0]]);
for p in [three[1], three[2], three[0]] { tri.line_to(corners[p]); }
leaf.figures.add(tri, inks::detail(BLOOD, 1.4));
let points = symbol_set("priests");
let places = plates::around(worked, 134, 13);
let spokes = plates::around(worked, 96, 13);
for k in 0..13 {
	leaf.figures.add(write_at(points.text([k]), places[k], #{ type: #{ glyph_scale: 28.0 } }));
	leaf.under.add(new_line(worked, spokes[k]), inks::pencil(SEPIA, 0.6));
}
book::clear(leaf, new_circle(worked, 146), 2);
book::jot(leaf, "0, 3, 9: the first", worked + vec2(60, 132), 200, #{ size: 22, turn: -0.03, margin: 4 });

// ---- the seals ----
for seal in illustration("triads", #{ seed: 3, editor: "cards", cell_width: 170.0, cell_height: 134.0 }).run(#{ cards: heap }) {
	book::place(leaf, seal.translate_by(vec2(46, 728)), 5);
}

// ---- the account, walled round it all ----
journal_entry.add(book::bind(leaf, content("thirteen"), #{ size: 14.0 }));
