#!/usr/bin/env python3
"""Builds webapp/fonts/black_priests.json, the black priests' symbols: the
alch_b13 set (Asset 1-13, filled shapes) as handwritten strokes, keyed a
to m.

Each stroke is a list of points through which a pen passes smoothly; a "|"
between points makes a corner there. Points are in the units the symbols
were traced in (each source scaled to 1.6 high, centred). Every glyph is
then stretched to HEIGHT by WIDTH and centred, so they all share one
aspect ratio, that of a typical capital letter."""

import json, os

HEIGHT = 1.5
WIDTH = HEIGHT * 0.7

GLYPHS = {
	# 1: a loop with its tail swinging down to the right, and a drop
	"a": [
		[(0.12, -0.12), (0.33, -0.38), (0.22, -0.65), (-0.1, -0.72), (-0.36, -0.5), (-0.25, -0.25), (0.05, -0.12), (0.22, 0.1), (0.3, 0.35), (0.18, 0.6)],
		[(-0.13, 0.0), (-0.2, 0.25)],
	],
	# 2: a bar over a backwards 3 (its prong apart), and a tall stroke beside them
	"b": [
		[(-0.45, -0.55), (-0.05, -0.6), (0.3, -0.62)],
		[(-0.58, -0.15), (-0.1, -0.25), (0.2, -0.1), (0.15, 0.2), (0.0, 0.55), (-0.25, 0.6), (-0.6, 0.45)],
		[(-0.3, 0.22), (0.02, 0.12)],
		[(0.45, -0.65), (0.47, 0.0), (0.48, 0.6)],
	],
	# 3: a hook over a zigzag, a leg, and a tall stroke at the right
	"c": [
		[(-0.22, -0.48), (0.05, -0.63), (0.3, -0.4), (0.5, 0.0)],
		[(-0.68, -0.18), "|", (-0.45, 0.25), "|", (0.3, -0.3)],
		[(0.0, 0.0), (0.2, 0.3), (0.2, 0.65)],
		[(0.55, -0.6), (0.57, 0.0), (0.57, 0.7)],
	],
	# 4: a wave on top, a stem bending out of it, and a diagonal; the
	# source's c is dropped, which made it read as i
	"d": [
		[(-0.75, -0.38), (-0.3, -0.37), (0.25, -0.5), (0.5, -0.58), (0.65, -0.42)],
		[(-0.55, -0.33), (-0.25, -0.2), (-0.2, 0.1), (-0.2, 0.6)],
		[(-0.2, 0.0), (0.65, 0.42)],
	],
	# 5: a great arc over into a stem that kicks back, and a dot
	"e": [
		[(-0.33, 0.58), (-0.55, 0.05), (-0.45, -0.42), (-0.05, -0.62), (0.4, -0.58), (0.55, -0.5), (0.4, -0.38), (0.35, -0.2), (0.33, 0.2), (0.27, 0.58), "|", (0.05, 0.2)],
		[(-0.05, -0.2), (0.05, -0.15)],
	],
	# 6: a peak with a flat arm, and a long stroke curving away below
	"f": [
		[(-0.3, -0.35), (-0.02, -0.66), (0.12, -0.45), (0.45, -0.27)],
		[(-0.05, -0.5), (0.1, -0.15), (0.1, 0.2), (-0.22, 0.68)],
	],
	# 7: a bar turning down into the right post, and two more posts
	"g": [
		[(-0.68, -0.55), (0.0, -0.6), (0.55, -0.62), (0.65, -0.5), (0.57, -0.3), (0.57, 0.6)],
		[(-0.45, -0.45), (-0.45, 0.62)],
		[(0.0, -0.5), (0.0, 0.5), (0.08, 0.65)],
	],
	# 8: a bar doubling back into a long curving stroke
	"h": [
		[(-0.47, -0.6), (0.0, -0.53), (0.5, -0.68), "|", (0.1, -0.4), (0.25, 0.0), (0.2, 0.4), (-0.03, 0.72)],
	],
	# 9: a bar, a post with a flag, and a bowl
	"i": [
		[(-0.47, -0.5), (0.38, -0.53), (0.37, -0.42)],
		[(-0.23, -0.48), (-0.23, 0.55)],
		[(-0.23, -0.08), (-0.5, 0.0)],
		[(0.2, 0.0), (0.05, 0.12), (0.15, 0.4), (0.42, 0.48)],
	],
	# 10: a bar, a stem kicking out right, a crossbar with a hook, a tick
	"j": [
		[(-0.55, -0.55), (0.05, -0.62), (0.5, -0.6), (0.5, -0.45)],
		[(0.1, -0.48), (0.12, 0.0), (0.2, 0.3), (0.42, 0.62)],
		[(-0.12, -0.25), "|", (-0.37, 0.0), "|", (0.1, -0.08), (0.5, -0.08)],
		[(-0.15, 0.33), (0.15, 0.22)],
	],
	# 11: a bar over a short leg and a long one, and a mark below
	"k": [
		[(-0.58, -0.6), (0.0, -0.58), (0.58, -0.57)],
		[(-0.12, -0.45), (-0.12, 0.1)],
		[(0.12, -0.42), (0.25, 0.0), (0.22, 0.6)],
		[(-0.1, 0.37), (-0.05, 0.6)],
	],
	# 12: a bar folding down into a post and a long leg, and a post
	# curling into it
	"l": [
		[(-0.58, -0.7), (0.35, -0.73), "|", (0.15, -0.58), (0.05, -0.3), (0.02, 0.0), (0.25, 0.25), (0.52, 0.62)],
		[(-0.5, -0.65), (-0.4, -0.1), (-0.32, 0.2), (-0.1, 0.07), (0.02, 0.0)],
	],
	# 13: a beak and an arc round into a loop and away down a leg, and a
	# stem; the source's bar across the loop and stub are dropped
	"m": [
		[(-0.55, -0.38), "|", (-0.45, -0.6), (0.1, -0.66), (0.38, -0.48), (0.33, -0.15), (0.0, 0.02), "|", (0.2, 0.12), (0.24, 0.62)],
		[(-0.05, -0.47), (-0.03, 0.05), (-0.18, 0.25)],
	],
}


def segments(stroke):
	"""Splits a stroke at its corners into runs of points."""
	runs, run = [], []
	for p in stroke:
		if p == "|":
			runs.append(run)
			run = [run[-1]]
		else:
			run.append(p)
	runs.append(run)
	return runs


def path(strokes, f):
	"""The SVG path: each run of points a Catmull-Rom spline as cubics."""
	out = []
	for stroke in strokes:
		first = True
		for run in segments(stroke):
			pts = [f(p) for p in run]
			if first:
				out.append("M %.3f %.3f" % pts[0])
				first = False
			if len(pts) == 2:
				out.append("L %.3f %.3f" % pts[1])
				continue
			ext = [pts[0]] + pts + [pts[-1]]
			for i in range(1, len(ext) - 2):
				p0, p1, p2, p3 = ext[i - 1], ext[i], ext[i + 1], ext[i + 2]
				c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
				c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
				out.append("C %.3f %.3f %.3f %.3f %.3f %.3f" % (c1 + c2 + p2))
	return " ".join(out)


def normalised(strokes):
	pts = [p for s in strokes for p in s if p != "|"]
	xs, ys = [p[0] for p in pts], [p[1] for p in pts]
	cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
	kx = WIDTH / (max(xs) - min(xs))
	ky = HEIGHT / (max(ys) - min(ys))
	return lambda p: ((p[0] - cx) * kx, (p[1] - cy) * ky)


def raw(strokes):
	return lambda p: p


if __name__ == "__main__":
	import sys
	here = os.path.dirname(os.path.abspath(__file__))
	fonts = os.path.join(here, "..", "webapp", "fonts")
	# --traced writes the strokes unscaled, to lay over the sources.
	scale = raw if "--traced" in sys.argv else normalised
	glyphs = {k: path(v, scale(v)) for k, v in GLYPHS.items()}
	out = sys.argv[-1] if sys.argv[-1].endswith(".json") else os.path.join(fonts, "black_priests.json")
	with open(out, "w") as f:
		json.dump({"character_padding": 0.35, "glyphs": glyphs}, f, indent="\t")
		f.write("\n")
