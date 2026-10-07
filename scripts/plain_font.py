#!/usr/bin/env python3
"""Writes the plain font into config/symbols.json.

Plain is a simple single-stroke sans for text that should read easily:
capitals and digits as `plain_upper_{c}`, lowercase as `plain_lower_{c}`
(use it as `new_fontset(["plain_upper"], ["plain_lower"])`), punctuation
with the capitals. Glyphs are drawn here as strokes of lines and elliptical
arcs; running the script replaces every `plain_` glyph in the dictionary
with them, leaving the others as they are.

Glyph space runs -1..1 with y down. Capitals and digits stand from the cap
line (-0.7) to the baseline (0.7); lowercase from the x-height (-0.28),
with ascenders to -0.8 and descenders to 1.05. Each glyph has a `bearing`:
room either side of its strokes when text is set. `plain_upper_ ` and
`plain_lower_ ` are blanks, the font's spaces.
"""

import json
import math
import os
import sys

SYMBOLS = os.path.join(os.path.dirname(__file__), "..", "config", "symbols.json")

CAP, BASE = -0.7, 0.7
XH = -0.28
ASC, DESC = -0.8, 1.05
MID = (XH + BASE) / 2  # middle of the x-height
BEARING = 0.16
# Spaces are this wide, rather than the default 1.0: words read as words.
SPACE = 0.62


def num(v):
    s = "%.4f" % v
    s = s.rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


class Path:
    def __init__(self):
        self.parts = []
        self.at = None

    def M(self, x, y):
        self.parts.append("M %s %s" % (num(x), num(y)))
        self.at = (x, y)
        return self

    def L(self, *points):
        for x, y in points:
            self.parts.append("L %s %s" % (num(x), num(y)))
            self.at = (x, y)
        return self

    def C(self, x1, y1, x2, y2, x, y):
        self.parts.append("C %s" % " ".join(num(v) for v in (x1, y1, x2, y2, x, y)))
        self.at = (x, y)
        return self

    def line(self, *points):
        """A stroke through the points."""
        self.M(*points[0])
        return self.L(*points[1:])

    def arc(self, cx, cy, rx, ry, a0, a1, join=False):
        """An elliptical arc from angle a0 to a1 (degrees; 0 is right, 90
        down), as cubics of at most 90 degrees. Starts a new stroke unless
        `join`, when it carries on from where the pen is (with a line if it
        isn't already at the arc's start)."""
        point = lambda a: (cx + rx * math.cos(math.radians(a)), cy + ry * math.sin(math.radians(a)))
        start = point(a0)
        if not join or self.at is None:
            self.M(*start)
        elif math.dist(self.at, start) > 1e-6:
            self.L(start)
        pieces = max(1, math.ceil(abs(a1 - a0) / 90.0))
        step = (a1 - a0) / pieces
        k = 4.0 / 3.0 * math.tan(math.radians(step) / 4.0)
        for i in range(pieces):
            t0 = math.radians(a0 + i * step)
            t1 = math.radians(a0 + (i + 1) * step)
            p0 = (cx + rx * math.cos(t0), cy + ry * math.sin(t0))
            p1 = (cx + rx * math.cos(t1), cy + ry * math.sin(t1))
            c1 = (p0[0] - k * rx * math.sin(t0), p0[1] + k * ry * math.cos(t0))
            c2 = (p1[0] + k * rx * math.sin(t1), p1[1] - k * ry * math.cos(t1))
            self.C(c1[0], c1[1], c2[0], c2[1], p1[0], p1[1])
        return self

    def ellipse(self, cx, cy, rx, ry, start=-90):
        return self.arc(cx, cy, rx, ry, start, start + 360)

    def dot(self, x, y):
        """A dot: a stroke too short to see as a line."""
        return self.line((x, y - 0.035), (x, y + 0.035))

    def __str__(self):
        return " ".join(self.parts)


def P():
    return Path()


# Lowercase bowls: centred on the x-height's middle. Arches (n, m, h) are
# ARY tall.
BR, BRY = 0.37, (BASE - XH) / 2
ARY = 0.36


def xy(t):
    """The height `t` of the way down from the x-height to the baseline."""
    return XH + t * (BASE - XH)


def lower():
    g = {}
    arch = XH + ARY
    g["a"] = P().ellipse(-0.02, MID, BR, BRY, 0).line((BR - 0.02, XH), (BR - 0.02, BASE))
    g["b"] = P().line((-BR, ASC), (-BR, BASE)).ellipse(0.0, MID, BR, BRY, 180)
    g["c"] = P().arc(0.02, MID, BR, BRY, -45, -315)
    g["d"] = P().ellipse(0.0, MID, BR, BRY, 0).line((BR, ASC), (BR, BASE))
    g["e"] = P().line((-BR, MID), (BR, MID)).arc(0.0, MID, BR, BRY, 0, -310, join=True)
    g["f"] = P().M(-0.08, BASE).L((-0.08, -0.5)).arc(0.17, -0.5, 0.25, 0.28, 180, 300, join=True).line((-0.3, XH), (0.24, XH))
    g["g"] = (P().ellipse(-0.02, MID, BR, BRY, 0)
              .M(BR - 0.02, XH).L((BR - 0.02, 0.74)).arc(-0.04, 0.74, BR + 0.02, 0.3, 0, 150, join=True))
    g["h"] = P().line((-0.33, ASC), (-0.33, BASE)).arc(0.0, arch, 0.33, ARY, 180, 360).L((0.33, BASE))
    g["i"] = P().line((0.0, XH), (0.0, BASE)).dot(0.0, XH - 0.3)
    g["j"] = P().M(0.08, XH).L((0.08, 0.75)).arc(-0.14, 0.75, 0.22, 0.28, 0, 160, join=True).dot(0.08, XH - 0.3)
    g["k"] = P().line((-0.3, ASC), (-0.3, BASE)).line((0.3, XH), (-0.3, xy(0.68))).line((-0.12, xy(0.5)), (0.34, BASE))
    g["l"] = P().M(-0.06, ASC).L((-0.06, 0.5)).arc(0.12, 0.5, 0.18, 0.2, 180, 90, join=True)
    g["m"] = (P().line((-0.5, XH), (-0.5, BASE))
              .arc(-0.25, arch, 0.25, ARY, 180, 360).L((0.0, BASE))
              .arc(0.25, arch, 0.25, ARY, 180, 360).L((0.5, BASE)))
    g["n"] = P().line((-0.33, XH), (-0.33, BASE)).arc(0.0, arch, 0.33, ARY, 180, 360).L((0.33, BASE))
    g["o"] = P().ellipse(0.0, MID, 0.39, BRY)
    g["p"] = P().line((-BR, XH), (-BR, DESC)).ellipse(0.0, MID, BR, BRY, 180)
    g["q"] = P().ellipse(0.0, MID, BR, BRY, 0).line((BR, XH), (BR, DESC))
    g["r"] = P().line((-0.22, XH), (-0.22, BASE)).arc(0.08, XH + 0.42, 0.3, 0.42, 180, 290)
    # An S as heights from the x-height (0) to the baseline (1).
    s_curve = [(0.29, 0.13), (0.2, -0.04), (-0.32, -0.07), (-0.3, 0.24), (-0.28, 0.53), (0.32, 0.42),
               (0.32, 0.73), (0.32, 1.07), (-0.22, 1.07), (-0.33, 0.84)]
    sx = [(x, xy(t)) for x, t in s_curve]
    g["s"] = P().M(*sx[0]).C(*sx[1], *sx[2], *sx[3]).C(*sx[4], *sx[5], *sx[6]).C(*sx[7], *sx[8], *sx[9])
    g["t"] = P().M(-0.08, -0.62).L((-0.08, 0.5)).arc(0.12, 0.5, 0.2, 0.2, 180, 90, join=True).L((0.26, BASE)).line((-0.3, XH), (0.24, XH))
    g["u"] = P().M(-0.33, XH).L((-0.33, BASE - 0.4)).arc(0.0, BASE - 0.4, 0.33, 0.4, 180, 0, join=True).line((0.33, XH), (0.33, BASE))
    g["v"] = P().line((-0.37, XH), (0.0, BASE), (0.37, XH))
    g["w"] = P().line((-0.53, XH), (-0.26, BASE), (0.0, xy(0.25)), (0.26, BASE), (0.53, XH))
    g["x"] = P().line((-0.34, XH), (0.34, BASE)).line((0.34, XH), (-0.34, BASE))
    # The left arm meets the right one at the baseline.
    t = (BASE - XH) / (DESC - XH)
    g["y"] = P().line((-0.37, XH), (0.37 - 0.5 * t, BASE)).line((0.37, XH), (-0.13, DESC))
    g["z"] = P().line((-0.33, XH), (0.33, XH), (-0.33, BASE), (0.35, BASE))
    return g


def upper():
    g = {}
    g["a"] = P().line((-0.45, BASE), (0.0, CAP), (0.45, BASE)).line((-0.28, 0.22), (0.28, 0.22))
    g["b"] = (P().line((-0.36, CAP), (-0.36, BASE))
              .M(-0.36, CAP).L((0.06, CAP)).arc(0.06, -0.36, 0.3, 0.34, -90, 90, join=True).L((-0.36, -0.02))
              .M(-0.36, -0.02).L((0.1, -0.02)).arc(0.1, 0.34, 0.32, 0.36, -90, 90, join=True).L((-0.36, BASE)))
    g["c"] = P().arc(0.04, 0.0, 0.46, 0.7, -42, -318)
    g["d"] = P().line((-0.38, CAP), (-0.38, BASE)).M(-0.38, CAP).L((-0.02, CAP)).arc(-0.02, 0.0, 0.42, 0.7, -90, 90, join=True).L((-0.38, BASE))
    g["e"] = P().line((0.34, CAP), (-0.34, CAP), (-0.34, BASE), (0.36, BASE)).line((-0.34, 0.0), (0.24, 0.0))
    g["f"] = P().line((0.34, CAP), (-0.32, CAP), (-0.32, BASE)).line((-0.32, 0.0), (0.22, 0.0))
    g["g"] = P().arc(0.04, 0.0, 0.46, 0.7, -42, -338).L((0.47, 0.06), (0.1, 0.06))
    g["h"] = P().line((-0.38, CAP), (-0.38, BASE)).line((0.38, CAP), (0.38, BASE)).line((-0.38, 0.0), (0.38, 0.0))
    g["i"] = P().line((0.0, CAP), (0.0, BASE)).line((-0.17, CAP), (0.17, CAP)).line((-0.17, BASE), (0.17, BASE))
    g["j"] = P().M(0.22, CAP).L((0.22, 0.36)).arc(-0.07, 0.36, 0.29, 0.34, 0, 165, join=True)
    g["k"] = P().line((-0.36, CAP), (-0.36, BASE)).line((0.38, CAP), (-0.36, 0.12)).line((-0.1, -0.08), (0.42, BASE))
    g["l"] = P().line((-0.32, CAP), (-0.32, BASE), (0.34, BASE))
    g["m"] = P().line((-0.47, BASE), (-0.47, CAP), (0.0, 0.32), (0.47, CAP), (0.47, BASE))
    g["n"] = P().line((-0.38, BASE), (-0.38, CAP), (0.38, BASE), (0.38, CAP))
    g["o"] = P().ellipse(0.0, 0.0, 0.47, 0.7)
    g["p"] = P().line((-0.36, CAP), (-0.36, BASE)).M(-0.36, CAP).L((0.04, CAP)).arc(0.04, -0.33, 0.33, 0.37, -90, 90, join=True).L((-0.36, 0.04))
    g["q"] = P().ellipse(0.0, 0.0, 0.47, 0.7).line((0.1, 0.34), (0.5, 0.82))
    g["r"] = (P().line((-0.36, CAP), (-0.36, BASE)).M(-0.36, CAP).L((0.04, CAP))
              .arc(0.04, -0.33, 0.33, 0.37, -90, 90, join=True).L((-0.36, 0.04)).line((-0.02, 0.04), (0.4, BASE)))
    g["s"] = (P().M(0.36, -0.5)
              .C(0.24, -0.76, -0.38, -0.78, -0.37, -0.36)
              .C(-0.36, 0.02, 0.38, -0.08, 0.39, 0.32)
              .C(0.4, 0.8, -0.3, 0.82, -0.42, 0.46))
    g["t"] = P().line((-0.42, CAP), (0.42, CAP)).line((0.0, CAP), (0.0, BASE))
    g["u"] = P().M(-0.38, CAP).L((-0.38, 0.24)).arc(0.0, 0.24, 0.38, 0.46, 180, 0, join=True).L((0.38, CAP))
    g["v"] = P().line((-0.45, CAP), (0.0, BASE), (0.45, CAP))
    g["w"] = P().line((-0.62, CAP), (-0.31, BASE), (0.0, -0.22), (0.31, BASE), (0.62, CAP))
    g["x"] = P().line((-0.4, CAP), (0.4, BASE)).line((0.4, CAP), (-0.4, BASE))
    g["y"] = P().line((-0.42, CAP), (0.0, 0.02), (0.42, CAP)).line((0.0, 0.02), (0.0, BASE))
    g["z"] = P().line((-0.36, CAP), (0.38, CAP), (-0.38, BASE), (0.4, BASE))

    # Digits, as tall as capitals.
    g["0"] = P().ellipse(0.0, 0.0, 0.37, 0.7).line((0.22, -0.48), (-0.22, 0.48))
    g["1"] = P().line((-0.22, -0.44), (0.04, CAP), (0.04, BASE)).line((-0.22, BASE), (0.3, BASE))
    g["2"] = (P().M(-0.33, -0.4)
              .C(-0.3, -0.78, 0.36, -0.82, 0.35, -0.36)
              .C(0.34, -0.08, 0.0, 0.2, -0.36, BASE)
              .L((0.38, BASE)))
    g["3"] = P().arc(0.0, -0.36, 0.32, 0.34, 205, 450).arc(0.0, 0.35, 0.37, 0.37, -90, 155, join=True)
    g["4"] = P().line((0.16, BASE), (0.16, CAP), (-0.4, 0.3), (0.42, 0.3))
    g["5"] = (P().line((0.34, CAP), (-0.27, CAP), (-0.31, -0.06))
              .C(0.0, -0.24, 0.4, -0.08, 0.38, 0.28)
              .C(0.36, 0.78, -0.2, 0.82, -0.38, 0.5))
    g["6"] = (P().M(0.3, -0.6)
              .C(-0.22, -0.82, -0.4, -0.24, -0.38, 0.28)
              .arc(0.0, 0.29, 0.38, 0.41, 180, 540, join=True))
    g["7"] = P().line((-0.36, CAP), (0.38, CAP), (-0.08, BASE))
    g["8"] = P().ellipse(0.0, -0.35, 0.3, 0.35, 90).ellipse(0.0, 0.34, 0.36, 0.36, -90)
    g["9"] = (P().ellipse(0.0, -0.29, 0.37, 0.41, 0)
              .M(0.37, -0.29).C(0.4, 0.3, 0.2, 0.82, -0.3, 0.6))

    # Punctuation.
    g["."] = P().dot(0.0, 0.64)
    g[","] = P().M(0.02, 0.6).C(0.04, 0.72, 0.0, 0.82, -0.08, 0.9)
    g[";"] = P().dot(0.02, 0.0).M(0.02, 0.6).C(0.04, 0.72, 0.0, 0.82, -0.08, 0.9)
    g[":"] = P().dot(0.0, 0.0).dot(0.0, 0.64)
    g["!"] = P().line((0.0, CAP), (0.0, 0.34)).dot(0.0, 0.64)
    g["?"] = (P().M(-0.3, -0.42)
              .C(-0.3, -0.82, 0.34, -0.84, 0.33, -0.4)
              .C(0.32, -0.12, 0.0, -0.06, 0.0, 0.3)
              .dot(0.0, 0.64))
    g["'"] = P().line((0.0, CAP), (0.0, -0.38))
    g["\""] = P().line((-0.1, CAP), (-0.1, -0.38)).line((0.1, CAP), (0.1, -0.38))
    g["-"] = P().line((-0.24, 0.16), (0.24, 0.16))
    g["—"] = P().line((-0.62, 0.16), (0.62, 0.16))
    g["_"] = P().line((-0.4, 0.86), (0.4, 0.86))
    g["("] = P().arc(0.36, 0.08, 0.36, 0.86, 240, 120)
    g[")"] = P().arc(-0.36, 0.08, 0.36, 0.86, -60, 60)
    g["["] = P().line((0.14, CAP - 0.08), (-0.12, CAP - 0.08), (-0.12, 0.92), (0.14, 0.92))
    g["]"] = P().line((-0.14, CAP - 0.08), (0.12, CAP - 0.08), (0.12, 0.92), (-0.14, 0.92))
    g["{"] = (P().M(0.18, CAP - 0.08).C(-0.08, CAP - 0.08, 0.04, 0.06, -0.18, 0.08)
              .C(0.04, 0.1, -0.08, 0.92, 0.18, 0.92))
    g["}"] = (P().M(-0.18, CAP - 0.08).C(0.08, CAP - 0.08, -0.04, 0.06, 0.18, 0.08)
              .C(-0.04, 0.1, 0.08, 0.92, -0.18, 0.92))
    g["/"] = P().line((0.3, CAP - 0.05), (-0.3, BASE + 0.1))
    g["#"] = (P().line((-0.1, -0.5), (-0.2, 0.6)).line((0.2, -0.5), (0.1, 0.6))
              .line((-0.34, -0.15), (0.38, -0.15)).line((-0.38, 0.25), (0.34, 0.25)))
    g["+"] = P().line((-0.3, 0.16), (0.3, 0.16)).line((0.0, -0.14), (0.0, 0.46))
    g["="] = P().line((-0.3, 0.04), (0.3, 0.04)).line((-0.3, 0.3), (0.3, 0.3))
    g["*"] = (P().line((0.0, -0.62), (0.0, -0.18)).line((-0.2, -0.52), (0.2, -0.28))
              .line((0.2, -0.52), (-0.2, -0.28)))
    g["<"] = P().line((0.3, -0.14), (-0.3, 0.16), (0.3, 0.46))
    g[">"] = P().line((-0.3, -0.14), (0.3, 0.16), (-0.3, 0.46))
    g["%"] = (P().ellipse(-0.24, -0.4, 0.14, 0.2).ellipse(0.24, 0.4, 0.14, 0.2)
              .line((0.34, CAP), (-0.34, BASE)))
    g["&"] = (P().M(0.42, BASE)
              .C(0.0, 0.3, -0.3, -0.1, -0.3, -0.4)
              .C(-0.3, -0.78, 0.22, -0.78, 0.22, -0.42)
              .C(0.22, -0.18, -0.42, 0.0, -0.42, 0.36)
              .C(-0.42, 0.82, 0.18, 0.86, 0.38, 0.22))
    return g


def main():
    # One glyph per line: edit the lines, so the other glyphs' lines stay
    # exactly as they were.
    with open(SYMBOLS, encoding="utf-8") as f:
        lines = f.read().split("\n")
    entries = [line.strip().rstrip(",") for line in lines if line.strip().startswith("{")]
    kept = [e for e in entries if not json.loads(e)["name"].startswith("plain_")]
    glyphs = [("plain_upper", upper()), ("plain_lower", lower())]
    for prefix, chars in glyphs:
        kept.append(json.dumps({"name": "%s_ " % prefix, "blank": SPACE}))
        for char, path in chars.items():
            kept.append(json.dumps({"name": "%s_%s" % (prefix, char), "path": str(path), "bearing": BEARING}, ensure_ascii=False))
    with open(SYMBOLS, "w", encoding="utf-8") as f:
        f.write("[\n" + ",\n".join("\t" + e for e in kept) + "\n]\n")
    json.load(open(SYMBOLS, encoding="utf-8"))  # still valid
    print("wrote %d plain glyphs to %s" % (sum(len(c) for _, c in glyphs), os.path.normpath(SYMBOLS)), file=sys.stderr)


if __name__ == "__main__":
    main()
