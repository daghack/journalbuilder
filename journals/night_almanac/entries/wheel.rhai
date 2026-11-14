// The wheel of the thirteen months (month_wheel): the frontispiece. Twelve
// moons round a star: read in order from the star, two to a letter, they
// spell a word. Under it, the moons in their order as he kept them, his
// note on how to read the wheel, and his account walled round them.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/book" as book;

const INK = inks::black();
const BLOOD = inks::blood();
const SEPIA = book::sepia();

seed(1414);

// The word, as moons: each letter two digits of eight phases.
let phases = new_lock("wheel").add("w", "symbols_to_digits", #{ input_base: 27, output_base: 8, width: 2 })
	.run(#{ symbols: symbols(27, "BEHIND") }).get("digits").value();

let leaf = book::leaf("wheel", 14);

// The wheel, and the frame's rules run on past its corners where he ruled
// them.
let at = vec2(80, 22);
let size = 1040.0;
book::place(leaf, illustration("month_wheel", #{ size: size, editor: "wheel" }).run(#{ marks: phases })[0].translate_by(at), 6);
let ruled = new_path();
for y in [at.y + size * 0.012, at.y + size - size * 0.012] { ruled.append(new_line(vec2(30, y), vec2(leaf.size.x - 30, y))); }
for x in [at.x + size * 0.012, at.x + size - size * 0.012] { ruled.append(new_line(vec2(x, 8), vec2(x, at.y + size + 70))); }
leaf.under.add(ruled, inks::pencil(SEPIA, 0.9));
leaf.avoid.add(ruled, #{ margin: 3 });

// The moons in their order, new to waning: the key, dark to bright and
// dark again, ruled under.
let moons = symbol_set("black_moons");
let key = new_canvas();
for d in 0..8 {
	key.add(write_at(moons.text([d]), vec2(150 + d * 56, 1190), #{ type: #{ glyph_scale: 42.0 } }));
}
plates::double_rule(key, vec2(118, 1228), vec2(586, 1228), 8.0, INK, 1.4);
book::place(leaf, key, 10);

// His notes on it.
book::jot(leaf, "Old to old. The star first. Two to a letter.", vec2(660, 1150), 440, #{ size: 40, ink: BLOOD, turn: -0.02, margin: 6 });
book::jot(leaf, "I have drawn the year as they turn it: twelve moons and the star, and in the middle the one that does not turn.", vec2(118, 1270), 520, #{ turn: 0.01, margin: 6 });

// The account, under the wheel.
let top = 1078.0;
let bottom = leaf.size.y - 40;
journal_entry.add(book::bind(leaf, content("watch"), #{ area: new_rectangle(vec2(leaf.size.x / 2.0, (top + bottom) / 2.0), leaf.size.x - 80, bottom - top) }));
