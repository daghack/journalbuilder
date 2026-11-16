// The sexton's psalter: its inks, and its leaf, burnt at the edges. The
// rest of what its pages are drawn with is the library's: lib/leaves (the
// leaf, notes, binding, hands), lib/shapes, lib/engraving, lib/gothic.
// import "lib/psalter" as ps;
//
//   let leaf = ps::leaf("nave", 21);
//   leaves::place(leaf, figure, 8);
//   leaves::jot(leaf, "She walks at vespers.", vec2(80, 60), 500);
//   journal_entry.add(leaves::bind(leaf, content("psalter")));

import "lib/leaves" as leaves;

// Lamp-black, deeper than the plates' iron gall: the figures and the wall.
fn ink() { "#1a120c" }
// Old blood, gone brown at the edges: the puzzle marks, and one note a
// page.
fn blood() { "#8a1a12" }
// Iron gall gone brown: pencil work, faint writing.
fn sepia() { "#5b3e22" }

// A leaf of the psalter (leaves::leaf), in its ink, burnt at its edges.
fn leaf(name, seed) { leaf(name, seed, #{}) }
fn leaf(name, seed, o) { leaves::leaf(name, seed, #{ ink: ink() } + o) }
