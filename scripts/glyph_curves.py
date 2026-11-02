"""Strokes to glyph paths, for the glyphset scripts.

A stroke is a list of points through which a pen passes smoothly; a "|"
between points makes a corner there, and a stroke whose last point is its
first closes smoothly on itself. Each run of points between corners is a
Catmull-Rom spline through them, which `path` then simplifies: it fits as
few cubic Béziers to the spline as keep within `tolerance` of it (Philip
Schneider's algorithm, from Graphics Gems), so a stroke traced through
many points comes out as a few smooth curves."""

import json, math

TOLERANCE = 0.01


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


def spline(pts, closed=False):
	"""A Catmull-Rom spline through pts as cubics (p0, c1, c2, p3)."""
	if closed:
		ext = [pts[-2]] + pts + [pts[1]]
	else:
		ext = [pts[0]] + pts + [pts[-1]]
	out = []
	for i in range(1, len(ext) - 2):
		p0, p1, p2, p3 = ext[i - 1], ext[i], ext[i + 1], ext[i + 2]
		c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
		c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
		out.append((p1, c1, c2, p2))
	return out


def _add(a, b): return (a[0] + b[0], a[1] + b[1])
def _sub(a, b): return (a[0] - b[0], a[1] - b[1])
def _mul(a, k): return (a[0] * k, a[1] * k)
def _dot(a, b): return a[0] * b[0] + a[1] * b[1]
def _len(a): return math.hypot(a[0], a[1])


def _unit(a):
	n = _len(a)
	return (a[0] / n, a[1] / n) if n else (0.0, 0.0)


def _bez(b, t):
	s = 1 - t
	return (s * s * s * b[0][0] + 3 * s * s * t * b[1][0] + 3 * s * t * t * b[2][0] + t * t * t * b[3][0],
		s * s * s * b[0][1] + 3 * s * s * t * b[1][1] + 3 * s * t * t * b[2][1] + t * t * t * b[3][1])


def _bez_d(b, t):
	s = 1 - t
	return (3 * (s * s * (b[1][0] - b[0][0]) + 2 * s * t * (b[2][0] - b[1][0]) + t * t * (b[3][0] - b[2][0])),
		3 * (s * s * (b[1][1] - b[0][1]) + 2 * s * t * (b[2][1] - b[1][1]) + t * t * (b[3][1] - b[2][1])))


def _bez_dd(b, t):
	s = 1 - t
	return (6 * (s * (b[2][0] - 2 * b[1][0] + b[0][0]) + t * (b[3][0] - 2 * b[2][0] + b[1][0])),
		6 * (s * (b[2][1] - 2 * b[1][1] + b[0][1]) + t * (b[3][1] - 2 * b[2][1] + b[1][1])))


def _sample(cubics, n=24):
	pts = [cubics[0][0]]
	for b in cubics:
		pts += [_bez(b, i / n) for i in range(1, n + 1)]
	return pts


def _chord(pts):
	u = [0.0]
	for a, b in zip(pts, pts[1:]):
		u.append(u[-1] + _len(_sub(b, a)))
	return [x / u[-1] for x in u]


def _generate(pts, u, t1, t2):
	"""The cubic from pts[0] to pts[-1], leaving along t1 and arriving
	along t2, nearest pts at parameters u (least squares)."""
	first, last = pts[0], pts[-1]
	c00 = c01 = c11 = x0 = x1 = 0.0
	for p, t in zip(pts, u):
		s = 1 - t
		a1, a2 = _mul(t1, 3 * s * s * t), _mul(t2, 3 * s * t * t)
		c00 += _dot(a1, a1); c01 += _dot(a1, a2); c11 += _dot(a2, a2)
		tmp = _sub(p, _add(_mul(first, s * s * s + 3 * s * s * t), _mul(last, 3 * s * t * t + t * t * t)))
		x0 += _dot(a1, tmp); x1 += _dot(a2, tmp)
	det = c00 * c11 - c01 * c01
	alpha1 = (x0 * c11 - x1 * c01) / det if abs(det) > 1e-12 else 0.0
	alpha2 = (c00 * x1 - c01 * x0) / det if abs(det) > 1e-12 else 0.0
	seg = _len(_sub(last, first))
	if alpha1 < 1e-6 * seg or alpha2 < 1e-6 * seg:
		alpha1 = alpha2 = seg / 3
	return (first, _add(first, _mul(t1, alpha1)), _add(last, _mul(t2, alpha2)), last)


def _error(pts, b, u):
	worst, at = 0.0, len(pts) // 2
	for i in range(1, len(pts) - 1):
		d = _len(_sub(_bez(b, u[i]), pts[i]))
		if d > worst:
			worst, at = d, i
	return worst, at


def _reparameterise(pts, b, u):
	out = []
	for p, t in zip(pts, u):
		d = _sub(_bez(b, t), p)
		d1, d2 = _bez_d(b, t), _bez_dd(b, t)
		den = _dot(d1, d1) + _dot(d, d2)
		out.append(min(1.0, max(0.0, t - _dot(d, d1) / den)) if den else t)
	return out


def _fit(pts, t1, t2, tol):
	if len(pts) == 2:
		d = _len(_sub(pts[1], pts[0])) / 3
		return [(pts[0], _add(pts[0], _mul(t1, d)), _add(pts[1], _mul(t2, d)), pts[1])]
	u = _chord(pts)
	b = _generate(pts, u, t1, t2)
	err, at = _error(pts, b, u)
	if err < tol:
		return [b]
	if err < tol * 4:
		for _ in range(20):
			u = _reparameterise(pts, b, u)
			b = _generate(pts, u, t1, t2)
			err, at = _error(pts, b, u)
			if err < tol:
				return [b]
	mid = _unit(_sub(pts[at - 1], pts[at + 1]))
	return _fit(pts[: at + 1], t1, mid, tol) + _fit(pts[at:], _mul(mid, -1), t2, tol)


def fit(cubics, tol, closed=False):
	"""As few cubics as keep within tol of these, leaving and arriving as
	they do (a closed run leaving and arriving along one tangent)."""
	pts = _sample(cubics)
	t1 = _unit(_sub(cubics[0][1], cubics[0][0]))
	t2 = _unit(_sub(cubics[-1][2], cubics[-1][3]))
	if closed:
		t1 = _unit(_sub(t1, t2))
		t2 = _mul(t1, -1)
	return _fit(pts, t1, t2, tol)


def path(strokes, f, tolerance=TOLERANCE, digits=4):
	"""The SVG path of these strokes, each point placed by f (into glyph
	units, so tolerance is in glyph units), its curves simplified."""
	num = "%." + str(digits) + "f"
	pair = num + " " + num
	out = []
	for stroke in strokes:
		first = True
		for run in segments(stroke):
			pts = [f(p) for p in run]
			if first:
				out.append("M " + pair % pts[0])
				first = False
			if len(pts) == 1:
				continue
			if len(pts) == 2:
				out.append("L " + pair % pts[1])
				continue
			closed = len(pts) > 3 and run[0] == run[-1] and "|" not in stroke
			cubics = spline(pts, closed)
			if tolerance:
				cubics = fit(cubics, tolerance, closed)
			for _, c1, c2, p in cubics:
				out.append("C " + " ".join([pair % c1, pair % c2, pair % p]))
	return " ".join(out)


def write_glyphset(out, font):
	"""Writes a glyphset file as the web editor does: a line for each
	setting and each glyph, its variants on it."""
	settings = [f"\t{json.dumps(k)}: {json.dumps(v)}," for k, v in font.items() if k != "glyphs"]
	lines = [f"\t\t{json.dumps(k, ensure_ascii=False)}: {json.dumps(v, ensure_ascii=False)}" for k, v in font["glyphs"].items()]
	with open(out, "w") as f:
		f.write("\n".join(["{", *settings, '\t"glyphs": {', ",\n".join(lines), "\t}", "}"]) + "\n")
