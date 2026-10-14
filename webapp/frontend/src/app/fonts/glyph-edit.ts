// What the glyph canvas does to a path: points found, moved and deleted,
// segments split, converted and deleted, strokes begun, carried on and
// closed. Each takes a path and gives back a new one.

import { clonePath, convertSegment, segmentStart, splitSegment, type GlyphPath, type Point, type Segment } from './glyph-path';

/** A point of a path: stroke `s`'s start (`i` -1), or segment `i`'s end or control. */
export interface PointRef {
  s: number;
  i: number;
  role: 'start' | 'to' | 'c' | 'c1' | 'c2';
}

/** A segment of a path: segment `i` of stroke `s`. */
export interface SegmentRef {
  s: number;
  i: number;
}

export const refKey = (ref: PointRef) => `${ref.s}.${ref.i}.${ref.role}`;

export function parseRef(key: string): PointRef {
  const [s, i, role] = key.split('.');
  return { s: Number(s), i: Number(i), role: role as PointRef['role'] };
}

export const isAnchor = (ref: PointRef) => ref.role === 'start' || ref.role === 'to';

export function getPoint(path: GlyphPath, ref: PointRef): Point | null {
  const stroke = path[ref.s];
  if (!stroke) return null;
  if (ref.role === 'start') return stroke.start;
  const segment = stroke.segments[ref.i] as unknown as Record<string, Point> | undefined;
  return segment?.[ref.role] ?? null;
}

function setPoint(path: GlyphPath, ref: PointRef, p: Point) {
  const stroke = path[ref.s];
  if (ref.role === 'start') stroke.start = p;
  else (stroke.segments[ref.i] as unknown as Record<string, Point>)[ref.role] = p;
}

/** Every point of a path, anchors and controls, with where it is. */
export function allPoints(path: GlyphPath): { ref: PointRef; point: Point }[] {
  const points: { ref: PointRef; point: Point }[] = [];
  path.forEach((stroke, s) => {
    points.push({ ref: { s, i: -1, role: 'start' }, point: stroke.start });
    stroke.segments.forEach((segment, i) => {
      if (segment.kind === 'Q') points.push({ ref: { s, i, role: 'c' }, point: segment.c });
      if (segment.kind === 'C') {
        points.push({ ref: { s, i, role: 'c1' }, point: segment.c1 });
        points.push({ ref: { s, i, role: 'c2' }, point: segment.c2 });
      }
      points.push({ ref: { s, i, role: 'to' }, point: segment.to });
    });
  });
  return points;
}

/** The anchor at the end of segment `i` (or the start, for -1). */
const anchor = (s: number, i: number): PointRef => (i < 0 ? { s, i: -1, role: 'start' } : { s, i, role: 'to' });

/** The cubic controls beside an anchor, which move with it. */
function attachedControls(path: GlyphPath, ref: PointRef): PointRef[] {
  const stroke = path[ref.s];
  if (!stroke) return [];
  const at = ref.role === 'start' ? -1 : ref.i;
  const controls: PointRef[] = [];
  if (at >= 0 && stroke.segments[at]?.kind === 'C') controls.push({ s: ref.s, i: at, role: 'c2' });
  if (stroke.segments[at + 1]?.kind === 'C') controls.push({ s: ref.s, i: at + 1, role: 'c1' });
  return controls;
}

/** `refs` moved by `delta` from where they are in `base`, anchors taking their cubic controls along. */
export function movePoints(base: GlyphPath, refs: PointRef[], delta: Point): GlyphPath {
  const path = clonePath(base);
  const keys = new Set(refs.map(refKey));
  for (const ref of refs) if (isAnchor(ref)) for (const control of attachedControls(base, ref)) keys.add(refKey(control));
  for (const key of keys) {
    const ref = parseRef(key);
    const p = getPoint(base, ref);
    if (p) setPoint(path, ref, { x: p.x + delta.x, y: p.y + delta.y });
  }
  return path;
}

/** Points removed: an anchor joins the segments either side of it; a control simplifies its segment. */
export function deletePoints(base: GlyphPath, refs: PointRef[]): GlyphPath {
  let path = clonePath(base);
  // Last first, so earlier references still hold.
  const sorted = [...refs].sort((a, b) => b.s - a.s || b.i - a.i);
  for (const ref of sorted) {
    const stroke = path[ref.s];
    if (!stroke) continue;
    if (ref.role === 'start') {
      const first = stroke.segments.shift();
      if (first) stroke.start = first.to;
    } else if (ref.role === 'to') {
      const next = stroke.segments[ref.i + 1];
      if (next) {
        // Join this segment and the next: from this one's start to the next one's end.
        stroke.segments.splice(ref.i, 2, next);
      } else {
        stroke.segments.splice(ref.i, 1);
      }
    } else {
      const segment = stroke.segments[ref.i];
      if (!segment) continue;
      const from = segmentStart(stroke, ref.i);
      stroke.segments[ref.i] = convertSegment(from, segment, segment.kind === 'C' ? 'Q' : 'L');
    }
  }
  path = path.filter((stroke) => stroke.segments.length > 0);
  return path;
}

/** A segment removed, its stroke broken in two there. */
export function deleteSegment(base: GlyphPath, ref: SegmentRef): GlyphPath {
  const path = clonePath(base);
  const stroke = path[ref.s];
  if (!stroke?.segments[ref.i]) return path;
  const before = { start: stroke.start, segments: stroke.segments.slice(0, ref.i), closed: false };
  const after = { start: stroke.segments[ref.i].to, segments: stroke.segments.slice(ref.i + 1), closed: false };
  path.splice(ref.s, 1, ...[before, after].filter((s) => s.segments.length > 0));
  return path;
}

/** A segment split at `t`: the path, and the anchor added. */
export function splitAt(base: GlyphPath, ref: SegmentRef, t: number): { path: GlyphPath; anchor: PointRef } {
  const path = clonePath(base);
  const stroke = path[ref.s];
  const segment = stroke?.segments[ref.i];
  if (!segment) return { path, anchor: anchor(ref.s, ref.i) };
  const halves = splitSegment(segmentStart(stroke, ref.i), segment, t);
  stroke.segments.splice(ref.i, 1, ...halves);
  return { path, anchor: { s: ref.s, i: ref.i, role: 'to' } };
}

export function convert(base: GlyphPath, ref: SegmentRef, kind: Segment['kind']): GlyphPath {
  const path = clonePath(base);
  const stroke = path[ref.s];
  const segment = stroke?.segments[ref.i];
  if (segment) stroke.segments[ref.i] = convertSegment(segmentStart(stroke, ref.i), segment, kind);
  return path;
}

/** A new stroke begun at `p`. */
export function beginStroke(base: GlyphPath, p: Point): { path: GlyphPath; anchor: PointRef } {
  const path = clonePath(base);
  path.push({ start: { ...p }, segments: [], closed: false });
  return { path, anchor: { s: path.length - 1, i: -1, role: 'start' } };
}

/** A segment of `kind` to `p` added to the end of stroke `s` (or a new stroke begun, if there isn't one). */
export function extend(base: GlyphPath, s: number, kind: Segment['kind'], p: Point): { path: GlyphPath; anchor: PointRef } {
  if (!base[s]) return beginStroke(base, p);
  const path = clonePath(base);
  const stroke = path[s];
  const from = stroke.segments.length ? stroke.segments[stroke.segments.length - 1].to : stroke.start;
  const line: Segment = { kind: 'L', to: { ...p } };
  stroke.segments.push(convertSegment(from, line, kind));
  return { path, anchor: { s, i: stroke.segments.length - 1, role: 'to' } };
}

/** The stroke a pen tool carries on: the one whose last anchor is selected, else the last. */
export function strokeToExtend(path: GlyphPath, selected: PointRef[]): number {
  for (const ref of selected) {
    const stroke = path[ref.s];
    if (!stroke || !isAnchor(ref)) continue;
    const last = stroke.segments.length - 1;
    if ((ref.role === 'start' && last < 0) || (ref.role === 'to' && ref.i === last)) return ref.s;
  }
  return path.length - 1;
}

export function setClosed(base: GlyphPath, s: number, closed: boolean): GlyphPath {
  const path = clonePath(base);
  if (path[s]) path[s].closed = closed;
  return path;
}

/** A stroke drawn the other way: pooling and pressure follow the pen. */
export function reverse(base: GlyphPath, s: number): GlyphPath {
  const path = clonePath(base);
  const stroke = path[s];
  if (!stroke) return path;
  const anchors = [stroke.start, ...stroke.segments.map((segment) => segment.to)];
  const segments: Segment[] = [];
  for (let i = stroke.segments.length - 1; i >= 0; i--) {
    const segment = stroke.segments[i];
    const to = anchors[i];
    segments.push(segment.kind === 'L' ? { kind: 'L', to } : segment.kind === 'Q' ? { kind: 'Q', c: segment.c, to } : { kind: 'C', c1: segment.c2, c2: segment.c1, to });
  }
  path[s] = { start: anchors[anchors.length - 1], segments, closed: stroke.closed };
  return path;
}

/** The segments of a path, each with where it starts, for drawing and hit tests. */
export function segmentsOf(path: GlyphPath): { ref: SegmentRef; from: Point; segment: Segment }[] {
  return path.flatMap((stroke, s) => stroke.segments.map((segment, i) => ({ ref: { s, i }, from: segmentStart(stroke, i), segment })));
}

/** One segment as path data, to draw it picked out. */
export function segmentData(from: Point, s: Segment): string {
  const p = (q: Point) => `${q.x} ${q.y}`;
  if (s.kind === 'L') return `M ${p(from)} L ${p(s.to)}`;
  if (s.kind === 'Q') return `M ${p(from)} Q ${p(s.c)} ${p(s.to)}`;
  return `M ${p(from)} C ${p(s.c1)} ${p(s.c2)} ${p(s.to)}`;
}
