#!/usr/bin/env python3
"""Builds webapp/library/glyphsets/thirteen_black_priests.json: the base 13
digits, brush-written symbols (filled shapes) as pen strokes, keyed by the
digits they stand for: 0 to 8, then A, E, f and s.

The originals are a broad brush's: heavy strokes, about a seventh as wide
as the symbols are high, swelling in the middle and tapering at both ends,
with a teardrop tick (a brush pressed and flicked away) marking several.
The strokes here are their centrelines, traced from the shapes'
skeletons, routed in writing order and cleaned up to read apart; A and E,
rings like 4 under a bar and a tick, are new symbols built from the
others' strokes. A tick is written as a comma is: across its head and
round down its tail.

Each stroke is a list of points through which the pen passes smoothly; a
"|" between points makes a corner there, and a stroke ending where it
began closes on itself. Points are in the units the symbols were traced
in (each source 2 high, centred, y down). Every glyph is then scaled to
HEIGHT and its width drawn halfway to ASPECT, so the set keeps a narrow,
upright box but each symbol some of its own breadth; their curves are
simplified (glyph_curves.py)."""

import os
from glyph_curves import path, write_glyphset

HEIGHT = 1.6
ASPECT = 0.6



def tick(lobe, node, tip, tail):
	"""A teardrop tick, as a comma's written: from its tip back across its
	head and round away down its tail."""
	return [tip, node, lobe, tail]


GLYPHS = {
	# a bar, a diagonal down from it into a Ʒ's bowl and tail, and a tick
	"0": [
		[(-0.4, -0.39), (-0.16, -0.455), (0.15, -0.465), (0.4, -0.54)],
		[(0.15, -0.46), (0.06, -0.27), (-0.07, -0.06)],
		[(-0.36, 0.18), (-0.28, 0.01), (-0.07, -0.06), (0.12, 0.05), (0.19, 0.26), (0.17, 0.5), (0.12, 0.74), (-0.04, 0.96)],
		tick((-0.07, -0.82), (0.07, -0.8), (0.21, -0.91), (-0.05, -0.65)),
	],
	# a tall bowl and a short one, side by side
	"1": [
		[(-0.17, -0.95), (-0.37, -0.72), (-0.46, -0.4), (-0.5, 0.0), (-0.5, 0.4), (-0.47, 0.73), (-0.37, 0.86), (-0.18, 0.86), (0.03, 0.76), (0.2, 0.56), (0.33, 0.47)],
		[(0.24, -0.96), (0.14, -0.84), (0.1, -0.6), (0.07, -0.15), (0.1, 0.07), (0.2, 0.17), (0.45, 0.13), (0.59, 0.01), (0.63, -0.17)],
	],
	# an arch down into a stem, a stroke swung from it round into a bowl,
	# and a tick below
	"2": [
		[(-0.43, -0.36), (-0.3, -0.58), (-0.17, -0.82), (0.12, -0.89), (0.3, -0.82), (0.39, -0.6), (0.36, -0.32), (0.33, -0.07), (0.2, 0.14)],
		[(0.36, -0.33), (0.14, -0.29), (-0.1, -0.13), (-0.3, 0.04), (-0.37, 0.2), (-0.32, 0.34), (-0.2, 0.46), (-0.05, 0.47), (0.08, 0.4)],
		tick((0.05, 0.82), (0.2, 0.81), (0.3, 0.71), (0.11, 0.96)),
	],
	# two hooks, one above the other, and a tick between them
	"3": [
		[(-0.24, -0.11), (-0.4, -0.18), (-0.47, -0.4), (-0.46, -0.64), (-0.3, -0.77), (-0.05, -0.81), (0.25, -0.77), (0.54, -0.6)],
		[(-0.23, 0.98), (-0.38, 0.92), (-0.45, 0.74), (-0.45, 0.5), (-0.33, 0.36), (-0.12, 0.3), (0.2, 0.33), (0.42, 0.42), (0.59, 0.57)],
		tick((0.16, -0.15), (0.28, -0.16), (0.37, -0.28), (0.2, 0.0)),
	],
	# a ring, its left side heavy
	"4": [
		[(0.05, -0.87), (-0.24, -0.82), (-0.36, -0.55), (-0.39, -0.2), (-0.38, 0.3), (-0.33, 0.55), (-0.2, 0.76), (-0.04, 0.83), (0.17, 0.78), (0.28, 0.55), (0.35, 0.1), (0.36, -0.35), (0.31, -0.65), (0.2, -0.82), (0.05, -0.87)],
	],
	# a hood over and down into a falling stem, and a bar into it
	"5": [
		[(-0.61, -0.28), (-0.45, -0.5), (-0.33, -0.75), (-0.1, -0.86), (0.22, -0.86), (0.42, -0.73), (0.49, -0.48), (0.43, -0.15), (0.34, 0.38), (0.24, 0.7), (0.03, 0.96)],
		[(-0.4, 0.03), (-0.33, -0.12), (-0.17, -0.185), (0.12, -0.2), (0.4, -0.12)],
	],
	# a bar curling up at its end, a stem from it to a second bar, a long
	# stroke down from that and back, and a tick
	"6": [
		[(-0.46, -0.75), (-0.3, -0.63), (-0.05, -0.54), (0.32, -0.53), (0.45, -0.61), (0.45, -0.8), (0.36, -0.95), (0.22, -0.99)],
		[(-0.04, -0.53), (-0.02, -0.36)],
		[(0.2, -0.31), (-0.05, -0.34), (-0.34, -0.22), "|", (-0.3, -0.12), (-0.14, 0.14), (0.0, 0.43), (0.06, 0.74), (-0.01, 0.87), (-0.15, 0.9), (-0.33, 0.85), (-0.45, 0.69)],
		tick((0.11, -0.06), (0.25, -0.06), (0.37, -0.16), (0.15, 0.07)),
	],
	# a stroke down to a fork, a leg curling back left and one right, and
	# two ticks
	"7": [
		[(-0.1, -0.54), (-0.09, -0.3), (-0.02, 0.0), (0.04, 0.3), "|", (-0.1, 0.55), (-0.26, 0.81), (-0.5, 0.78), (-0.68, 0.64), (-0.73, 0.42)],
		[(0.04, 0.3), (0.17, 0.43), (0.31, 0.56), (0.5, 0.53), (0.67, 0.39), (0.71, 0.24), (0.67, 0.06)],
		tick((0.0, -0.72), (0.18, -0.75), (0.31, -0.91), (0.09, -0.57)),
		tick((0.19, -0.33), (0.38, -0.36), (0.51, -0.51), (0.29, -0.18)),
	],
	# a hook down into a stem and a tail swung right, a bowl to its left,
	# and a tick
	"8": [
		[(-0.12, -0.98), (0.07, -0.91), (0.19, -0.78), (0.23, -0.56), (0.17, -0.38), (0.0, -0.23), "|", (-0.03, 0.05), (-0.12, 0.34), (-0.18, 0.6), (-0.13, 0.81), (0.06, 0.87), (0.3, 0.82), (0.6, 0.57), (0.82, 0.42)],
		[(0.0, -0.23), (-0.5, -0.21), (-0.67, -0.12), (-0.66, 0.11), (-0.58, 0.3), (-0.38, 0.46), (-0.18, 0.57)],
		tick((0.2, 0.22), (0.39, 0.21), (0.55, 0.09), (0.27, 0.38)),
	],
	# a bar curling up at its end (6's), a stem down from it turning out
	# at its foot (f's), and a tick by the stem; the source's A was a ring
	# under a bar, too like 4
	"A": [
		[(-0.45, -0.7), (-0.1, -0.77), (0.28, -0.78), (0.44, -0.87), (0.4, -0.98)],
		[(-0.05, -0.76), (-0.1, -0.2), (-0.12, 0.4), (-0.04, 0.76), (0.15, 0.86), (0.42, 0.76)],
		tick((0.12, -0.05), (0.26, -0.05), (0.38, -0.16), (0.17, 0.08)),
	],
	# two posts under a bar, the right one's foot swung out, and a tick
	# between them (7's); the source's E was a ring under a tick, too like 4
	"E": [
		[(-0.48, -0.72), (0.0, -0.78), (0.45, -0.8)],
		[(-0.3, -0.75), (-0.32, 0.0), (-0.3, 0.7)],
		[(0.25, -0.78), (0.22, 0.0), (0.26, 0.6), (0.4, 0.8), (0.55, 0.84)],
		tick((-0.12, 0.34), (0.0, 0.33), (0.1, 0.2), (-0.06, 0.48)),
	],
	# a stroke down into a fork: a leg away to the left and an arch to the
	# right, its foot turned out; and a tick above
	"f": [
		[(-0.03, -0.49), (-0.14, -0.27), (-0.17, -0.05), (-0.22, 0.2), (-0.3, 0.52), (-0.49, 0.7)],
		[(-0.15, -0.03), (0.08, 0.02), (0.14, 0.22), (0.1, 0.55), (0.1, 0.77), (0.24, 0.87), (0.5, 0.79)],
		tick((-0.29, -0.79), (-0.11, -0.79), (0.04, -0.91), (-0.22, -0.63)),
	],
	# a hook down into a stem and a long tail, a bowl to its right with a
	# foot, and a bar across
	"s": [
		[(-0.62, -0.65), (-0.54, -0.82), (-0.26, -0.87), (-0.09, -0.79), (-0.01, -0.62), (-0.04, -0.25), (-0.08, 0.09), "|", (-0.34, 0.15), (-0.46, 0.27), (-0.43, 0.5), (-0.3, 0.68), (-0.02, 0.83), (0.18, 0.97)],
		[(-0.01, -0.62), (0.08, -0.67), (0.29, -0.67), (0.41, -0.55), (0.45, -0.3), (0.41, 0.03), (0.33, 0.25), (0.21, 0.4)],
		[(-0.08, 0.09), (0.15, 0.1), (0.38, 0.12)],
	],
}


def placed(strokes):
	"""Traced units to glyph units: HEIGHT high, as wide as halfway (in
	proportion) between its own breadth and ASPECT, centred."""
	pts = [p for s in strokes for p in s if p != "|"]
	xs, ys = [p[0] for p in pts], [p[1] for p in pts]
	cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
	w, h = max(xs) - min(xs), max(ys) - min(ys)
	ky = HEIGHT / h
	kx = HEIGHT * ((w / h) * ASPECT) ** 0.5 / w
	return lambda p: ((p[0] - cx) * kx, (p[1] - cy) * ky)


def raw(strokes):
	return lambda p: p


if __name__ == "__main__":
	import sys
	here = os.path.dirname(os.path.abspath(__file__))
	glyphsets = os.path.join(here, "..", "webapp", "library", "glyphsets")
	# --traced writes the strokes unscaled, to lay over the sources.
	scale = raw if "--traced" in sys.argv else placed
	glyphs = {k: path(v, scale(v)) for k, v in GLYPHS.items()}
	out = sys.argv[-1] if sys.argv[-1].endswith(".json") else os.path.join(glyphsets, "thirteen_black_priests.json")
	write_glyphset(out, {"character_padding": 0.35, "glyphs": glyphs})
