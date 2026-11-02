#!/usr/bin/env python3
"""Builds webapp/library/glyphsets/the_four_divine.json: the base 4 digits,
brush-written symbols (filled shapes) as pen strokes, keyed a, i, l and v
as their sources are named.

They're the base 13 digits' hand (scripts/base13_font.py) on a broader
footing: heavy, tapering brush strokes, about a seventh as wide as the
symbols are high, built of several bars and bowls, with teardrop ticks.
The strokes here are their centrelines, traced from the shapes'
skeletons, routed in writing order and cleaned up to read apart. A tick
is written as a comma is: across its head and round down its tail.

Each stroke is a list of points through which the pen passes smoothly; a
"|" between points makes a corner there. Points are in the units the
symbols were traced in (each source 2 high, centred, y down). Every glyph
is then scaled to HEIGHT and its width drawn halfway to ASPECT, so the
set keeps a square box but each symbol some of its own breadth; their
curves are simplified (glyph_curves.py)."""

import os
from glyph_curves import path, write_glyphset

HEIGHT = 1.6
ASPECT = 1.0



def tick(lobe, node, tip, tail):
	"""A teardrop tick, as a comma's written: from its tip back across its
	head and round away down its tail."""
	return [tip, node, lobe, tail]


GLYPHS = {
	# a hook down into a stem and a bowl swung under it, bars from the
	# stem to a second stem, an arch over to a third, a bar on top
	"a": [
		[(-0.82, -0.4), (-0.75, -0.55), (-0.48, -0.59), (-0.36, -0.5), (-0.3, -0.37), (-0.33, -0.1), (-0.36, 0.22), "|", (-0.6, 0.27), (-0.68, 0.37), (-0.65, 0.59), (-0.54, 0.74), (-0.3, 0.86), (-0.14, 0.97)],
		[(-0.3, -0.37), (0.02, -0.4)],
		[(-0.36, 0.22), (0.02, 0.25)],
		[(0.02, -0.4), (0.09, -0.15), (0.06, 0.16), (0.02, 0.26), (-0.05, 0.43), (-0.12, 0.5)],
		[(0.02, -0.4), (0.12, -0.57), (0.21, -0.64), (0.49, -0.675), (0.6, -0.64), (0.67, -0.54), (0.7, -0.42), (0.64, -0.1), (0.53, 0.1), (0.49, 0.48), (0.42, 0.6), (0.33, 0.7)],
		[(0.56, 0.0), (0.39, -0.08), (0.19, -0.09), (0.1, -0.15)],
		[(0.2, -0.87), (0.38, -0.92), (0.62, -0.93), (0.81, -0.98)],
	],
	# a hook down into a stem and a bowl swung under it, an arch over to
	# a second stem, a bar from the first rising to a fork: a stroke up
	# and a hook down; and two ticks
	"i": [
		[(-1.1, -0.61), (-1.07, -0.74), (-0.95, -0.83), (-0.75, -0.86), (-0.62, -0.82), (-0.5, -0.7), (-0.48, -0.5), (-0.5, 0.08), "|", (-0.92, 0.08), (-1.03, 0.14), (-1.06, 0.3), (-1.0, 0.55), (-0.82, 0.72), (-0.55, 0.97)],
		[(-0.5, -0.71), (-0.4, -0.81), (-0.24, -0.825), (-0.1, -0.82), (0.0, -0.73), (0.07, -0.51), (0.04, 0.08)],
		[(-0.5, 0.08), (-0.3, 0.15), (-0.07, 0.18), (0.05, 0.09), (0.44, -0.04), "|", (0.57, -0.58), (0.67, -0.7)],
		[(0.44, -0.04), (0.5, 0.24), (0.58, 0.33), (0.8, 0.3), (0.91, 0.25), (0.98, 0.15), (1.0, 0.03)],
		tick((0.81, -0.8), (0.96, -0.75), (1.13, -0.82), (0.82, -0.65)),
		tick((0.81, -0.43), (0.96, -0.39), (1.13, -0.46), (0.82, -0.29)),
	],
	# two arches over a stem, a bowl swung left from its foot, a bar out
	# right, and two ticks
	"l": [
		[(-0.95, -0.27), (-0.8, -0.47), (-0.6, -0.75), (-0.4, -0.79), (-0.18, -0.79), (0.03, -0.7), "|", (0.35, -0.81), (0.7, -0.785), (0.84, -0.73), (1.04, -0.59)],
		[(0.03, -0.7), (0.02, -0.2), (-0.02, -0.1), (-0.06, 0.055), (-0.2, 0.24)],
		[(0.02, -0.2), (-0.26, -0.21), (-0.69, 0.05), (-0.84, 0.19), (-0.87, 0.28), (-0.79, 0.47), (-0.7, 0.55), (-0.56, 0.57), (-0.36, 0.5)],
		[(0.03, -0.185), (0.59, -0.133), (0.72, -0.065), (0.88, 0.06)],
		tick((-0.22, 0.5), (-0.1, 0.49), (-0.02, 0.39), (-0.18, 0.65)),
		tick((-0.31, 0.87), (-0.16, 0.86), (-0.06, 0.77), (-0.26, 0.98)),
	],
	# a stroke up into a wave, a stem down to a fork: a leg out right and
	# one back left into a loop and away; a stroke down the loop; two
	# ticks
	"v": [
		[(-0.34, -0.535), (-0.1, -0.44), (-0.02, -0.36), "|", (0.02, -0.44), (0.15, -0.48), (0.33, -0.44), (0.54, -0.35), (0.69, -0.32), (0.78, -0.36), (0.82, -0.42), (0.82, -0.54), (0.8, -0.61)],
		[(-0.02, -0.36), (-0.01, -0.18), "|", (0.15, -0.13), (0.51, 0.13), (0.62, 0.255), (0.64, 0.405)],
		[(-0.01, -0.18), (-0.13, -0.04), (-0.25, 0.05), "|", (-0.66, 0.06), (-0.73, 0.08), (-0.78, 0.14), (-0.76, 0.29), (-0.71, 0.45), (-0.63, 0.52), (-0.46, 0.6), (-0.395, 0.667), (-0.38, 0.8), (-0.33, 0.87), (-0.17, 0.89), (0.03, 0.84), (0.25, 0.64), (0.385, 0.55)],
		[(-0.25, 0.05), (-0.27, 0.28), (-0.395, 0.667)],
		tick((-0.28, -0.83), (-0.13, -0.82), (-0.01, -0.92), (-0.24, -0.69)),
		tick((-0.08, 0.39), (0.06, 0.39), (0.18, 0.29), (-0.06, 0.53)),
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
	out = sys.argv[-1] if sys.argv[-1].endswith(".json") else os.path.join(glyphsets, "the_four_divine.json")
	write_glyphset(out, {"character_padding": 0.4, "glyphs": glyphs})
