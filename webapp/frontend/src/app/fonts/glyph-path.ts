// Glyph paths: SVG path data read into strokes of lines, quadratic and
// cubic curves, edited, and written back. Reads what src/journal/paths.rs
// reads (M L H V C S Q T Z, absolute or relative; no arcs), and writes
// absolute commands with numbers written as fonts.json has them.

export interface Point {
  x: number;
  y: number;
}

export type Segment =
  | { kind: 'L'; to: Point }
  | { kind: 'Q'; c: Point; to: Point }
  | { kind: 'C'; c1: Point; c2: Point; to: Point };

/** A pen stroke: where it starts, its segments, and whether it closes back to the start. */
export interface Stroke {
  start: Point;
  segments: Segment[];
  closed: boolean;
}

export type GlyphPath = Stroke[];

/** Read path data, failing as paths.rs does on what it can't read. */
export function parsePath(d: string): GlyphPath {
  const strokes: Stroke[] = [];
  let pos = 0;
  let current: Point = { x: 0, y: 0 };
  let start: Point = { x: 0, y: 0 };
  let lastCubic: Point | null = null;
  let lastQuad: Point | null = null;
  let cmd: string | null = null;
  let stroke: Stroke | null = null;

  const skip = () => {
    while (pos < d.length && /[\s,]/.test(d[pos])) pos++;
  };
  const number = (c: string): number => {
    skip();
    const match = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?/.exec(d.slice(pos));
    if (!match) throw new Error(pos >= d.length ? `'${c}' needs more numbers` : `expected a number for '${c}' at byte ${pos}`);
    pos += match[0].length;
    return Number(match[0]);
  };
  const point = (c: string, origin: Point): Point => {
    const x = number(c);
    const y = number(c);
    return { x: origin.x + x, y: origin.y + y };
  };
  const segment = (segment: Segment) => {
    if (!stroke || stroke.closed) {
      // A segment after Z starts again from where the stroke began.
      stroke = { start: { ...start }, segments: [], closed: false };
      strokes.push(stroke);
    }
    stroke.segments.push(segment);
  };

  for (;;) {
    skip();
    if (pos >= d.length) break;
    const next = d[pos];
    if (/[a-zA-Z]/.test(next)) {
      pos++;
      if (cmd === null && next !== 'M' && next !== 'm') throw new Error(`path data must start with a moveto, not '${next}'`);
      cmd = next;
      if (next === 'Z' || next === 'z') {
        if (stroke) stroke.closed = true;
        current = { ...start };
        lastCubic = lastQuad = null;
        continue;
      }
    } else if (!/[0-9.+-]/.test(next)) {
      throw new Error(`unexpected '${next}' at byte ${pos}`);
    }
    if (cmd === null) throw new Error('path data must start with a moveto');
    const c: string = cmd;
    const origin: Point = c === c.toLowerCase() ? current : { x: 0, y: 0 };
    let cubic: Point | null = null;
    let quad: Point | null = null;
    switch (c.toUpperCase()) {
      case 'M':
        current = point(c, origin);
        start = { ...current };
        stroke = { start: { ...current }, segments: [], closed: false };
        strokes.push(stroke);
        cmd = c === 'm' ? 'l' : 'L';
        break;
      case 'L':
        current = point(c, origin);
        segment({ kind: 'L', to: current });
        break;
      case 'H':
        current = { x: origin.x + number(c), y: current.y };
        segment({ kind: 'L', to: current });
        break;
      case 'V':
        current = { x: current.x, y: origin.y + number(c) };
        segment({ kind: 'L', to: current });
        break;
      case 'C':
      case 'S': {
        const c1: Point = c.toUpperCase() === 'C' ? point(c, origin) : reflect(lastCubic, current);
        const c2: Point = point(c, origin);
        const to: Point = point(c, origin);
        segment({ kind: 'C', c1, c2, to });
        current = to;
        cubic = c2;
        break;
      }
      case 'Q':
      case 'T': {
        const control: Point = c.toUpperCase() === 'Q' ? point(c, origin) : reflect(lastQuad, current);
        const to: Point = point(c, origin);
        segment({ kind: 'Q', c: control, to });
        current = to;
        quad = control;
        break;
      }
      case 'Z':
        throw new Error(`unexpected number after '${c}' at byte ${pos}`);
      case 'A':
        throw new Error("arcs ('A'/'a') aren't supported");
      default:
        throw new Error(`unknown path command '${c}'`);
    }
    lastCubic = cubic;
    lastQuad = quad;
  }
  return strokes;
}

function reflect(control: Point | null, about: Point): Point {
  return control ? { x: 2 * about.x - control.x, y: 2 * about.y - control.y } : { ...about };
}

/**
 * A number as fonts.json has it: four decimals at most,
 * trailing zeros and point dropped, never "-0". Exact halves round to even,
 * as Python's "%.4f" does.
 */
export function num(v: number): string {
  // toFixed rounds the exact value, as "%.4f" does, but an exact half up.
  let fixed = v.toFixed(4);
  const exact = Math.abs(v).toFixed(30);
  const point = exact.indexOf('.');
  const half = exact[point + 5] === '5' && /^0*$/.test(exact.slice(point + 6));
  if (half && Number(exact[point + 4]) % 2 === 0) fixed = (v < 0 ? '-' : '') + exact.slice(0, point + 5);
  let s = fixed.replace(/0+$/, '').replace(/\.$/, '');
  if (s === '-0' || s === '') s = '0';
  return s;
}

const pt = (p: Point) => `${num(p.x)} ${num(p.y)}`;

/** Path data for `path`: absolute commands, numbers as num() writes them. */
export function formatPath(path: GlyphPath): string {
  const parts: string[] = [];
  for (const stroke of path) {
    parts.push(`M ${pt(stroke.start)}`);
    for (const s of stroke.segments) {
      if (s.kind === 'L') parts.push(`L ${pt(s.to)}`);
      else if (s.kind === 'Q') parts.push(`Q ${pt(s.c)} ${pt(s.to)}`);
      else parts.push(`C ${pt(s.c1)} ${pt(s.c2)} ${pt(s.to)}`);
    }
    if (stroke.closed) parts.push('Z');
  }
  return parts.join(' ');
}

// ---- geometry ----

export const lerp = (a: Point, b: Point, t: number): Point => ({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
export const dist = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);

/** Where segment `index` of `stroke` starts. */
export function segmentStart(stroke: Stroke, index: number): Point {
  return index === 0 ? stroke.start : stroke.segments[index - 1].to;
}

/** The point at `t` along a segment from `from`. */
export function pointAt(from: Point, s: Segment, t: number): Point {
  if (s.kind === 'L') return lerp(from, s.to, t);
  if (s.kind === 'Q') return lerp(lerp(from, s.c, t), lerp(s.c, s.to, t), t);
  const a = lerp(from, s.c1, t), b = lerp(s.c1, s.c2, t), c = lerp(s.c2, s.to, t);
  return lerp(lerp(a, b, t), lerp(b, c, t), t);
}

/** A segment from `from` split at `t` (de Casteljau): the two halves. */
export function splitSegment(from: Point, s: Segment, t: number): [Segment, Segment] {
  if (s.kind === 'L') {
    const m = lerp(from, s.to, t);
    return [{ kind: 'L', to: m }, { kind: 'L', to: { ...s.to } }];
  }
  if (s.kind === 'Q') {
    const a = lerp(from, s.c, t), b = lerp(s.c, s.to, t), m = lerp(a, b, t);
    return [{ kind: 'Q', c: a, to: m }, { kind: 'Q', c: b, to: { ...s.to } }];
  }
  const a = lerp(from, s.c1, t), b = lerp(s.c1, s.c2, t), c = lerp(s.c2, s.to, t);
  const ab = lerp(a, b, t), bc = lerp(b, c, t), m = lerp(ab, bc, t);
  return [{ kind: 'C', c1: a, c2: ab, to: m }, { kind: 'C', c1: bc, c2: c, to: { ...s.to } }];
}

/** A segment from `from` as another kind: exactly, where it can be (L or Q to C), else as near as it goes. */
export function convertSegment(from: Point, s: Segment, kind: Segment['kind']): Segment {
  if (s.kind === kind) return s;
  if (kind === 'L') return { kind: 'L', to: { ...s.to } };
  if (kind === 'C') {
    if (s.kind === 'L') return { kind: 'C', c1: lerp(from, s.to, 1 / 3), c2: lerp(from, s.to, 2 / 3), to: { ...s.to } };
    const q = s as Extract<Segment, { kind: 'Q' }>;
    return { kind: 'C', c1: lerp(from, q.c, 2 / 3), c2: lerp(q.to, q.c, 2 / 3), to: { ...q.to } };
  }
  if (s.kind === 'L') return { kind: 'Q', c: lerp(from, s.to, 0.5), to: { ...s.to } };
  // A cubic's nearest quadratic: the control both its controls point to, on average.
  const c = s as Extract<Segment, { kind: 'C' }>;
  return {
    kind: 'Q',
    c: { x: (3 * (c.c1.x + c.c2.x) - from.x - c.to.x) / 4, y: (3 * (c.c1.y + c.c2.y) - from.y - c.to.y) / 4 },
    to: { ...c.to },
  };
}

/** The nearest point on a segment to `p`: its `t` and how far it is. */
export function nearestOnSegment(from: Point, s: Segment, p: Point): { t: number; distance: number } {
  let best = { t: 0, distance: Infinity };
  const steps = s.kind === 'L' ? 1 : 32;
  if (s.kind === 'L') {
    const dx = s.to.x - from.x, dy = s.to.y - from.y;
    const len = dx * dx + dy * dy;
    const t = len === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - from.x) * dx + (p.y - from.y) * dy) / len));
    return { t, distance: dist(lerp(from, s.to, t), p) };
  }
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const d = dist(pointAt(from, s, t), p);
    if (d < best.distance) best = { t, distance: d };
  }
  // Refine around the best sample.
  let step = 1 / steps;
  for (let i = 0; i < 12; i++) {
    step /= 2;
    for (const t of [best.t - step, best.t + step]) {
      if (t < 0 || t > 1) continue;
      const d = dist(pointAt(from, s, t), p);
      if (d < best.distance) best = { t, distance: d };
    }
  }
  return best;
}

/** An ellipse as a stroke of four cubics, starting at angle `startDeg` (0 is right, 90 down). */
export function ellipse(cx: number, cy: number, rx: number, ry: number, startDeg = -90): Stroke {
  const rad = (a: number) => (a * Math.PI) / 180;
  const at = (a: number): Point => ({ x: cx + rx * Math.cos(rad(a)), y: cy + ry * Math.sin(rad(a)) });
  const pieces = 4;
  const step = 360 / pieces;
  const k = (4 / 3) * Math.tan(rad(step) / 4);
  const segments: Segment[] = [];
  for (let i = 0; i < pieces; i++) {
    const t0 = rad(startDeg + i * step), t1 = rad(startDeg + (i + 1) * step);
    const p0 = { x: cx + rx * Math.cos(t0), y: cy + ry * Math.sin(t0) };
    const p1 = { x: cx + rx * Math.cos(t1), y: cy + ry * Math.sin(t1) };
    segments.push({
      kind: 'C',
      c1: { x: p0.x - k * rx * Math.sin(t0), y: p0.y + k * ry * Math.cos(t0) },
      c2: { x: p1.x + k * rx * Math.sin(t1), y: p1.y - k * ry * Math.cos(t1) },
      to: p1,
    });
  }
  return { start: at(startDeg), segments, closed: false };
}

/** A dot: a stroke too short to see as a line. */
export function dot(at: Point): Stroke {
  return { start: { x: at.x, y: at.y - 0.035 }, segments: [{ kind: 'L', to: { x: at.x, y: at.y + 0.035 } }], closed: false };
}

/** The box around every point of a path, controls included; null if it has none. */
export function bounds(path: GlyphPath): { minX: number; minY: number; maxX: number; maxY: number } | null {
  const points = path.flatMap((stroke) => [stroke.start, ...stroke.segments.flatMap(controlsAndEnd)]);
  if (!points.length) return null;
  return {
    minX: Math.min(...points.map((p) => p.x)),
    minY: Math.min(...points.map((p) => p.y)),
    maxX: Math.max(...points.map((p) => p.x)),
    maxY: Math.max(...points.map((p) => p.y)),
  };
}

/** A segment's control points, then where it ends. */
export function controlsAndEnd(s: Segment): Point[] {
  return s.kind === 'L' ? [s.to] : s.kind === 'Q' ? [s.c, s.to] : [s.c1, s.c2, s.to];
}

/** Where a path's ink goes, top and bottom, sampled along its curves (controls left out). */
export function inkExtent(path: GlyphPath): { top: number; bottom: number } | null {
  let top = Infinity, bottom = -Infinity;
  for (const stroke of path) {
    let from = stroke.start;
    top = Math.min(top, from.y);
    bottom = Math.max(bottom, from.y);
    for (const s of stroke.segments) {
      for (let i = 1; i <= 8; i++) {
        const p = pointAt(from, s, i / 8);
        top = Math.min(top, p.y);
        bottom = Math.max(bottom, p.y);
      }
      from = s.to;
    }
  }
  return Number.isFinite(top) ? { top, bottom } : null;
}

/** A deep copy of a path. */
export function clonePath(path: GlyphPath): GlyphPath {
  return path.map((stroke) => ({
    start: { ...stroke.start },
    closed: stroke.closed,
    segments: stroke.segments.map((s): Segment =>
      s.kind === 'L' ? { kind: 'L', to: { ...s.to } }
        : s.kind === 'Q' ? { kind: 'Q', c: { ...s.c }, to: { ...s.to } }
          : { kind: 'C', c1: { ...s.c1 }, c2: { ...s.c2 }, to: { ...s.to } }),
  }));
}
