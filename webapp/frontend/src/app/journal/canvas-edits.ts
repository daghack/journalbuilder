// The visual editor's edits to the canvases an entry commits to it
// (`canvas.commit_to_editor(name)`), kept in `edits/ENTRY.json`, and the
// geometry of editing them: what the engine does with an edit
// (src/journal/rhaientries/edits.rs), and how a gesture on a page becomes
// one.

import type { Committed, Corners, Point } from '../scribe';

/** The edits file version this editor writes and reads. */
export const EDITS_VERSION = 1;
/** The most an edit skews, in degrees (the engine takes under 89). */
export const MAX_SKEW = 85;

/** An ink override: colour, opacity (multiplied), and line weight (multiplied). */
export interface InkEdit {
  color?: string;
  opacity?: number;
  weight?: number;
}

/**
 * One canvas's edit. Applied when it's committed: the corners pinned
 * (`warp`, each corner's offset over the box it had), then moved, turned,
 * scaled and skewed about the box's centre `C`, `C + move + R(turn)·K(skew)·S(scale)·(p − C)`,
 * then re-inked.
 */
export interface CanvasEdit {
  move?: Point;
  turn?: number;
  scale?: Point;
  skew?: number;
  warp?: Corners;
  hidden?: boolean;
  locked?: boolean;
  ink?: InkEdit;
  /** For a symbolic canvas: the symbols it's drawn as, and the symbol set they're of, in place of the script's. */
  symbols?: number[];
  symbol_set?: string;
}

export interface EditsFile {
  version: number;
  canvases: Record<string, CanvasEdit>;
}

/** The edits in an edits file's `text`; none for no file, or one that doesn't parse. */
export function parseEdits(text: string | null): EditsFile {
  if (text === null) return { version: EDITS_VERSION, canvases: {} };
  try {
    const file = JSON.parse(text) as Partial<EditsFile>;
    if (file && typeof file === 'object' && file.version === EDITS_VERSION) {
      const canvases = file.canvases && typeof file.canvases === 'object' ? file.canvases : {};
      return { version: EDITS_VERSION, canvases };
    }
  } catch {
    // Not JSON: the run says why; the editor starts from none.
  }
  return { version: EDITS_VERSION, canvases: {} };
}

/** Whether an edits file's `text` is one the editor reads (or there's none). */
export function editsReadable(text: string | null): boolean {
  if (text === null) return true;
  try {
    return (JSON.parse(text) as Partial<EditsFile>)?.version === EDITS_VERSION;
  } catch {
    return false;
  }
}

/** `x` rounded to `places` decimals, without a -0. */
function round(x: number, places = 3): number {
  const r = Math.round(x * 10 ** places) / 10 ** places;
  return r === 0 ? 0 : r;
}

/** `edit` without its identity values, numbers rounded; null if nothing's left. */
export function cleanEdit(edit: CanvasEdit): CanvasEdit | null {
  const out: CanvasEdit = {};
  if (edit.move && (round(edit.move[0]) !== 0 || round(edit.move[1]) !== 0)) out.move = [round(edit.move[0]), round(edit.move[1])];
  if (edit.turn !== undefined) {
    const turn = round(normalizeTurn(edit.turn));
    if (turn !== 0) out.turn = turn;
  }
  // A scale is never 0, nor a skew near a right angle: the engine refuses those.
  const nonzero = (s: number) => round(s, 4) || (s < 0 ? -0.0001 : 0.0001);
  if (edit.scale && (round(edit.scale[0], 4) !== 1 || round(edit.scale[1], 4) !== 1)) out.scale = [nonzero(edit.scale[0]), nonzero(edit.scale[1])];
  if (edit.skew !== undefined && round(edit.skew) !== 0) out.skew = round(Math.max(-MAX_SKEW, Math.min(MAX_SKEW, edit.skew)));
  if (edit.warp && edit.warp.some(([x, y]) => round(x) !== 0 || round(y) !== 0)) {
    out.warp = edit.warp.map(([x, y]) => [round(x), round(y)]) as Corners;
  }
  if (edit.hidden) out.hidden = true;
  if (edit.locked) out.locked = true;
  if (edit.symbols?.length) out.symbols = edit.symbols.map((s) => Math.max(0, Math.round(s)));
  if (edit.symbol_set) out.symbol_set = edit.symbol_set;
  if (edit.ink) {
    const ink: InkEdit = {};
    if (edit.ink.color) ink.color = edit.ink.color;
    if (edit.ink.opacity !== undefined && round(edit.ink.opacity) !== 1) ink.opacity = round(Math.min(1, Math.max(0, edit.ink.opacity)));
    if (edit.ink.weight !== undefined && round(edit.ink.weight) !== 1) ink.weight = round(Math.max(0.01, edit.ink.weight));
    if (Object.keys(ink).length) out.ink = ink;
  }
  return Object.keys(out).length ? out : null;
}

/** The text of an edits file holding `file`'s edits, cleaned; null if none's left (the file goes). */
export function serializeEdits(file: EditsFile): string | null {
  const canvases: Record<string, CanvasEdit> = {};
  for (const name of Object.keys(file.canvases).sort()) {
    const edit = cleanEdit(file.canvases[name]);
    if (edit) canvases[name] = edit;
  }
  if (!Object.keys(canvases).length) return null;
  // Points on a line of their own: [12, -4], and a warp's [[0, 0], [6, -3], ...].
  const text = JSON.stringify({ version: EDITS_VERSION, canvases }, null, 2)
    .replace(/\[\s+(-?[\d.e+-]+),\s+(-?[\d.e+-]+)\s+\]/g, '[$1, $2]')
    .replace(/\[\s+(\[[^\[\]\n]*\](?:,\s+\[[^\[\]\n]*\])*)\s+\]/g, (_, inner: string) => `[${inner.split(/,\s+(?=\[)/).join(', ')}]`);
  return `${text}\n`;
}

/**
 * The edits file `text` with only the edits for canvases `names` (the ones
 * a run committed); null if none's left. A file the editor can't read is
 * left as it is.
 */
export function pruneEdits(text: string | null, names: string[]): string | null {
  if (text === null || !editsReadable(text)) return text;
  const file = parseEdits(text);
  const kept = new Set(names);
  const gone = Object.keys(file.canvases).filter((name) => !kept.has(name));
  if (!gone.length) return text;
  for (const name of gone) delete file.canvases[name];
  return serializeEdits(file);
}

// ---- affine maps ----

/** An affine map as SVG's `matrix(a b c d e f)`: x' = a x + c y + e, y' = b x + d y + f. */
export type Affine = [number, number, number, number, number, number];

export const IDENTITY: Affine = [1, 0, 0, 1, 0, 0];

export function apply(m: Affine, [x, y]: Point): Point {
  const [a, b, c, d, e, f] = m;
  return [a * x + c * y + e, b * x + d * y + f];
}

/** `m`'s linear part applied to the vector `v`. */
export function applyLinear(m: Affine, [x, y]: Point): Point {
  const [a, b, c, d] = m;
  return [a * x + c * y, b * x + d * y];
}

/** `m`, then `next`. */
export function then(m: Affine, next: Affine): Affine {
  const [a, b, c, d, e, f] = next;
  const [p, q, r, s, t, u] = m;
  return [a * p + c * q, b * p + d * q, a * r + c * s, b * r + d * s, a * t + c * u + e, b * t + d * u + f];
}

/** Each of `maps` in turn. */
export function chain(...maps: Affine[]): Affine {
  return maps.reduce(then, IDENTITY);
}

export function determinant(m: Affine): number {
  return m[0] * m[3] - m[1] * m[2];
}

/** The map back; null if `m` flattens the plane. */
export function inverse(m: Affine): Affine | null {
  const det = determinant(m);
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;
  const [a, b, c, d, e, f] = m;
  return [d / det, -b / det, -c / det, a / det, (c * f - d * e) / det, (b * e - a * f) / det];
}

export function translate([x, y]: Point): Affine {
  return [1, 0, 0, 1, x, y];
}

/** Turned `degrees` about the origin, clockwise on a page (y down). */
export function rotate(degrees: number): Affine {
  const r = (degrees * Math.PI) / 180;
  return [Math.cos(r), Math.sin(r), -Math.sin(r), Math.cos(r), 0, 0];
}

export function scale(x: number, y: number): Affine {
  return [x, 0, 0, y, 0, 0];
}

/** Skewed sideways by `degrees`: points further down go further right. */
export function skew(degrees: number): Affine {
  return [1, 0, Math.tan((degrees * Math.PI) / 180), 1, 0, 0];
}

/** `m` about `centre`: moved there, mapped, and moved back. */
export function about(m: Affine, [x, y]: Point): Affine {
  return chain(translate([-x, -y]), m, translate([x, y]));
}

export function isIdentity(m: Affine, epsilon = 1e-9): boolean {
  return m.every((v, i) => Math.abs(v - IDENTITY[i]) < epsilon);
}

/** `m` as an SVG transform. */
export function svgMatrix(m: Affine): string {
  return `matrix(${m.map((v) => +v.toFixed(6)).join(' ')})`;
}

/** An angle in degrees, in (-180, 180]. */
export function normalizeTurn(degrees: number): number {
  let d = degrees % 360;
  if (d > 180) d -= 360;
  if (d <= -180) d += 360;
  return d;
}

// ---- edits as maps ----

/** The centre of a box `[x, y, width, height]`. */
export function boxCentre(box: [number, number, number, number]): Point {
  return [box[0] + box[2] / 2, box[1] + box[3] / 2];
}

/** A box's corners: top left, top right, bottom right, bottom left. */
export function boxCorners([x, y, w, h]: [number, number, number, number]): Corners {
  return [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
}

/** The affine part of `edit` for a canvas whose box is centred on `centre` (as the engine has it). */
export function editAffine(edit: CanvasEdit, centre: Point): Affine {
  const [sx, sy] = edit.scale ?? [1, 1];
  const linear = chain(scale(sx, sy), skew(edit.skew ?? 0), rotate(edit.turn ?? 0));
  return then(about(linear, centre), translate(edit.move ?? [0, 0]));
}

/**
 * The turn, skew and scale whose `R·K·S` is `m`'s linear part, with the
 * horizontal scale negative if `flipX` (a flip kept where it was); null if
 * `m` flattens the plane.
 */
export function decompose(m: Affine, flipX = false): { turn: number; skew: number; scale: Point } | null {
  const [a, b, c, d] = m;
  if (Math.abs(determinant(m)) < 1e-12) return null;
  // The first column is R·(sx, 0); the rest, Rᵀ·(second column), is (tan(k)·sy, sy).
  const length = Math.hypot(a, b);
  const sx = flipX ? -length : length;
  const turn = (Math.atan2(b / sx, a / sx) * 180) / Math.PI;
  const r = (turn * Math.PI) / 180;
  const [cos, sin] = [Math.cos(r), Math.sin(r)];
  const u12 = cos * c + sin * d;
  const sy = -sin * c + cos * d;
  const skewDegrees = (Math.atan(u12 / sy) * 180) / Math.PI;
  return { turn: normalizeTurn(turn), skew: skewDegrees, scale: [sx, sy] };
}

/**
 * `edit` with its move, turn, scale and skew made the affine map `m` (for
 * a canvas whose box is centred on `centre`), its other settings kept.
 * Unchanged if `m` flattens the plane.
 */
export function withAffine(edit: CanvasEdit, m: Affine, centre: Point): CanvasEdit {
  const parts = decompose(m, (edit.scale?.[0] ?? 1) < 0);
  if (!parts) return edit;
  const moved = apply(m, centre);
  return { ...edit, move: [moved[0] - centre[0], moved[1] - centre[1]], turn: parts.turn, skew: parts.skew, scale: parts.scale };
}

/**
 * The affine map taking `from`'s corners to `to`'s (from three of them);
 * when `from` is flat (a line, a point), the nearest a turn, a uniform
 * scale and a move get.
 */
export function solveAffine(from: Corners, to: Corners): Affine {
  const [p0, p1, , p3] = from;
  const [q0, q1, , q3] = to;
  const u: Point = [p1[0] - p0[0], p1[1] - p0[1]];
  const v: Point = [p3[0] - p0[0], p3[1] - p0[1]];
  const u2: Point = [q1[0] - q0[0], q1[1] - q0[1]];
  const v2: Point = [q3[0] - q0[0], q3[1] - q0[1]];
  const det = u[0] * v[1] - u[1] * v[0];
  let linear: Affine;
  if (Math.abs(det) > 1e-9) {
    // [u2 v2] · [u v]⁻¹
    const inv: Affine = [v[1] / det, -u[1] / det, -v[0] / det, u[0] / det, 0, 0];
    linear = then(inv, [u2[0], u2[1], v2[0], v2[1], 0, 0]);
  } else {
    // Flat: a similarity from whichever edge has length.
    const [e, e2] = Math.hypot(...u) >= Math.hypot(...v) ? [u, u2] : [v, v2];
    const length = Math.hypot(...e);
    if (length < 1e-9) {
      linear = IDENTITY;
    } else {
      const k = Math.hypot(...e2) / length;
      const turn = Math.atan2(e2[1], e2[0]) - Math.atan2(e[1], e[0]);
      linear = [k * Math.cos(turn), k * Math.sin(turn), -k * Math.sin(turn), k * Math.cos(turn), 0, 0];
    }
  }
  const moved = applyLinear(linear, p0);
  return [linear[0], linear[1], linear[2], linear[3], q0[0] - moved[0], q0[1] - moved[1]];
}

/** Where a committed canvas is drawn on a page, and how it got there. */
export interface Placement {
  name: string;
  /** Its box when committed. */
  box: [number, number, number, number];
  /** Its edit's affine map. */
  edit: Affine;
  /** What the script did to it after the commit, solved from where its box ended up. */
  after: Affine;
  /** Its box's corners on the page (not warped). */
  corners: Corners;
}

/** Where `committed` is drawn on page `page`: once for each copy there. */
export function placements(committed: Committed, edit: CanvasEdit, page: number): Placement[] {
  const { box, local } = committed;
  if (!box || !local) return [];
  const m = editAffine(edit, boxCentre(box));
  return committed.shown.filter((s) => s.page === page).map((s) => ({
    name: committed.name, box, edit: m, after: solveAffine(local, s.corners), corners: s.corners,
  }));
}

/**
 * The edit that makes a canvas placed at `placement` look moved by `t` on
 * the page: the new edit map is `after⁻¹ · t · after · edit`.
 */
export function editForGesture(edit: CanvasEdit, placement: Placement, t: Affine): CanvasEdit {
  const back = inverse(placement.after);
  if (!back) return edit;
  const m = chain(placement.edit, placement.after, t, back);
  return withAffine(edit, m, boxCentre(placement.box));
}

/** Where a placed canvas's corners are, its warp included. */
export function warpCorners(edit: CanvasEdit, placement: Placement): Corners {
  const map = then(placement.edit, placement.after);
  const offsets = edit.warp ?? [[0, 0], [0, 0], [0, 0], [0, 0]];
  return boxCorners(placement.box).map((corner, i) => apply(map, [corner[0] + offsets[i][0], corner[1] + offsets[i][1]])) as Corners;
}

/** `edit` with corner `i` of a canvas placed at `placement` moved `by` on the page. */
export function editForWarp(edit: CanvasEdit, placement: Placement, i: number, by: Point): CanvasEdit {
  const back = inverse(then(placement.edit, placement.after));
  if (!back) return edit;
  const d = applyLinear(back, by);
  const warp = (edit.warp ?? [[0, 0], [0, 0], [0, 0], [0, 0]]).map((p) => [...p]) as Corners;
  warp[i] = [warp[i][0] + d[0], warp[i][1] + d[1]];
  return { ...edit, warp };
}

// ---- boxes on the page ----

/** A box on the page, `[left, top, right, bottom]`. */
export type Bounds = [number, number, number, number];

export function boundsOf(points: Point[]): Bounds {
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

export function unionBounds(boxes: Bounds[]): Bounds {
  return [Math.min(...boxes.map((b) => b[0])), Math.min(...boxes.map((b) => b[1])), Math.max(...boxes.map((b) => b[2])), Math.max(...boxes.map((b) => b[3]))];
}

/** A line snapped to: vertical at `x`, or horizontal at `y`. */
export type Guide = { x: number } | { y: number };

/**
 * How far to move `box` (already moved) to bring its edges or centre onto
 * the nearest of `xs` and `ys` within `within`, and the guides it lands on.
 */
export function snapBox(box: Bounds, xs: number[], ys: number[], within: number): { by: Point; guides: Guide[] } {
  const near = (own: number[], targets: number[]) => {
    let best: { d: number; at: number } | null = null;
    for (const o of own) {
      for (const t of targets) {
        const d = t - o;
        if (Math.abs(d) <= within && (!best || Math.abs(d) < Math.abs(best.d))) best = { d, at: t };
      }
    }
    return best;
  };
  const x = near([box[0], (box[0] + box[2]) / 2, box[2]], xs);
  const y = near([box[1], (box[1] + box[3]) / 2, box[3]], ys);
  const guides: Guide[] = [];
  if (x) guides.push({ x: x.at });
  if (y) guides.push({ y: y.at });
  return { by: [x?.d ?? 0, y?.d ?? 0], guides };
}

/** The lines boxes snap to: each box's edges and centre. */
export function snapLines(boxes: Bounds[]): { xs: number[]; ys: number[] } {
  return {
    xs: boxes.flatMap((b) => [b[0], (b[0] + b[2]) / 2, b[2]]),
    ys: boxes.flatMap((b) => [b[1], (b[1] + b[3]) / 2, b[3]]),
  };
}

export type Alignment = 'left' | 'centre' | 'right' | 'top' | 'middle' | 'bottom';

/** How far to move each of `boxes` to line them up (`how`) with `to`. */
export function align(boxes: Bounds[], how: Alignment, to: Bounds): Point[] {
  return boxes.map((b) => {
    switch (how) {
      case 'left': return [to[0] - b[0], 0];
      case 'right': return [to[2] - b[2], 0];
      case 'centre': return [(to[0] + to[2]) / 2 - (b[0] + b[2]) / 2, 0];
      case 'top': return [0, to[1] - b[1]];
      case 'bottom': return [0, to[3] - b[3]];
      case 'middle': return [0, (to[1] + to[3]) / 2 - (b[1] + b[3]) / 2];
    }
  });
}

/** How far to move each of `boxes` to space them evenly across (`x`) or down, the outermost staying put. */
export function distribute(boxes: Bounds[], axis: 'x' | 'y'): Point[] {
  const [lo, hi] = axis === 'x' ? [0, 2] : [1, 3];
  const order = boxes.map((b, i) => i).sort((i, j) => boxes[i][lo] + boxes[i][hi] - boxes[j][lo] - boxes[j][hi]);
  const moves: Point[] = boxes.map(() => [0, 0]);
  if (boxes.length < 3) return moves;
  const span = boxes[order[order.length - 1]][hi] - boxes[order[0]][lo];
  const sizes = order.reduce((sum, i) => sum + boxes[i][hi] - boxes[i][lo], 0);
  const gap = (span - sizes) / (boxes.length - 1);
  let at = boxes[order[0]][lo];
  for (const i of order) {
    const d = at - boxes[i][lo];
    moves[i] = axis === 'x' ? [d, 0] : [0, d];
    at += boxes[i][hi] - boxes[i][lo] + gap;
  }
  return moves;
}
