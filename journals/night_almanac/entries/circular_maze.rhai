// The circular maze (moonlit_maze, circular_maze, symbol_rows): a sentence
// hidden along the one way to the heart of a round maze, locked with a key
// word that is written as moons. He drew it as a garden plan: the maze in
// its wall, the moon he went in by worked out in the corner, the key in
// moons under it, his notes, and his account round them.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/book" as book;

const INK = inks::black();
const BLOOD = inks::blood();
const SEPIA = book::sepia();

seed(1717);

let made = chain("moonlit_maze", #{ rings: 5, seed: 11 })
	.run(#{ "vig.plain_text": symbols(27, "THE WAY OUT IS THE WAY IN"), "vig.key": symbols(27, "MOON") });

let leaf = book::leaf("circular_maze", 17);

// The maze, its walls heavy, in a wall of black with the days ticked round
// it, on the lines he set it out by.
let heart = vec2(540, 820);
book::construction(leaf.under, heart, 590, 560, SEPIA);
let maze = illustration("circular_maze", #{ size: 1000.0, glyph_size: 52.0, editor: "maze" })
	.run(#{ maze: made.get("lab.maze").value(), wall_style: #{ style: inks::contour(INK, 2.4) } })[0];
leaf.figures.add(maze.translate_by(heart - vec2(500, 500)));
book::fill_ring(leaf.figures, heart, 490.0, 507.0, INK);
leaf.figures.add(new_circle(heart, 516.0), inks::contour(INK, 1.8));
leaf.figures.add(plates::ticks(heart, 364, 519.0, 527.0, 28, 536.0), inks::fine(INK, 0.9));
book::clear(leaf, new_circle(heart, 540), 6);

// The moon he went in by, worked out: its disc set out with the compass,
// the dark side inked solid.
let moon_at = vec2(1040, 175);
book::construction(leaf.under, moon_at, 120, 0.0, SEPIA);
leaf.under.add(new_circle(moon_at, 104), inks::pencil(SEPIA, 0.8));
leaf.figures.add(inks::moon(art::moon(moon_at, 88.0, 0.86), INK, 2.0, #{ gap: 2.6 }));
book::clear(leaf, new_circle(moon_at, 124), 4);

book::jot(leaf, "The garden at the dark of the moon. I went in by the moon and I came out by the moon, and I was not the same.", vec2(64, 60), 800, #{ size: 34, turn: -0.01 });
book::jot(leaf, "Two moons to a letter. The way in is the way out.", vec2(70, 1486), 1000, #{ size: 32, ink: BLOOD, turn: 0.01 });

// The key, as moons, ruled under.
let key = illustration("symbol_rows", #{ group: 2, size: 60.0, per_row: 8, gap: 10.0, group_gap: 26.0 })
	.run(#{ symbols: made.get("nights.digits").value(), symbol_set: symbol_set("black_moons") })[0];
book::place(leaf, key.translate_by(vec2(70, 1390)), 10);
leaf.figures.add(new_line(vec2(60, 1468), vec2(610, 1468)), inks::contour(INK, 1.4));
book::clear(leaf, new_rectangle(vec2(335, 1468), 560, 16), 4);

// The account, round it all.
journal_entry.add(book::bind(leaf, content("watch")));
