// The mere-keeper's book: its inks, and its leaf, gone soft and dark at
// its edges where the water got in, and tide-marked. The rest of what its
// pages are drawn with is the library's: lib/leaves (the leaf, notes,
// binding, hands), lib/shapes, lib/engraving (black water, the moon's
// road), lib/gothic (reeds, swords, knives of black glass).
// import "lib/mere" as mb;
//
//   let leaf = mb::leaf("mere", 41);
//   leaves::place(leaf, figure, 8);
//   leaves::jot(leaf, "It does not move.", vec2(80, 60), 500);
//   journal_entry.add(leaves::bind(leaf, content("mere")));

import "lib/leaves" as leaves;

// Black glass: the figures and the wall.
fn ink() { "#121115" }
// Oxblood: the puzzle marks, and one note a page.
fn blood() { "#7e1913" }
// Ink gone grey-brown: pencil work, faint writing, the glass's sheen.
fn sepia() { "#4c4038" }

// A leaf of the book (leaves::leaf), in its ink, its edges rotted by damp
// (burn 0.7, greyer than fire's) and two tide-lines low down.
fn leaf(name, seed) { leaf(name, seed, #{}) }
fn leaf(name, seed, o) {
	leaves::leaf(name, seed, #{ ink: ink(), tide: 2, burn: 0.7, char: "#17150f", brown: "rgba(58, 54, 36, 0.5)", brown_width: 12.0, rim: "#24211a" } + o)
}
