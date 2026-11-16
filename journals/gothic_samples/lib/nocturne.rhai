// The sleeper's night-book of the black door: its inks, and its leaf,
// scorched at the edges. The rest of what its pages are drawn with is the
// library's: lib/leaves (the leaf, notes, binding, hands), lib/shapes,
// lib/engraving, lib/gothic (the stars that watch, the looking-glasses,
// writing as a glass shows it).
// import "lib/nocturne" as nb;
//
//   let leaf = nb::leaf("door", 71);
//   leaves::place(leaf, figure, 8);
//   leaves::jot(leaf, "It has no knob.", vec2(80, 60), 500);
//   journal_entry.add(leaves::bind(leaf, content("nights")));

import "lib/leaves" as leaves;

// Blue-black, the night's own: the figures and the wall.
fn ink() { "#16131c" }
// Prussian blue, cold as the glass: the puzzle marks, and one note a page.
fn cold() { "#203a72" }
// Ink gone grey-brown: pencil work, faint writing, the glass's sheen.
fn sepia() { "#4c4038" }

// A leaf of the night-book (leaves::leaf), in its ink, scorched at its
// edges.
fn leaf(name, seed) { leaf(name, seed, #{}) }
fn leaf(name, seed, o) { leaves::leaf(name, seed, #{ ink: ink() } + o) }
