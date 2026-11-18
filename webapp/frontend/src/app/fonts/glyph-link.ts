// Linked glyphs and reference pictures: each is put in place as the engine
// puts a linked glyph's parent (src/journal/fonts.rs), every point `p`
// moved to `move + R(turn)·K(skew)·S(scale)·p`, about the glyph's origin.
// A linked glyph is the glyph it's linked to, as that is now, so placed,
// with strokes of its own over it; a reference is a picture fitted in
// -1..1 and placed, for the editor to show behind a glyph.

import { MAX_SKEW, chain, decompose, normalizeTurn, rotate, scale, skew, translate, type Affine } from '../journal/canvas-edits';
import type { Font, Glyph } from './font-file';
import { formatPath, num, parsePath, type GlyphPath, type Point, type Segment } from './glyph-path';

/** Where a linked glyph's parent, or a reference, is put; every part may be left out. */
export interface Placement {
  move?: [number, number];
  turn?: number;
  scale?: [number, number];
  skew?: number;
}

/** A glyph drawn as another of its glyphset (`from`), placed, with strokes of its own (`add`, path data). */
export interface GlyphLink extends Placement {
  from: string;
  add?: string;
}

/** A picture (images/IMAGE.png or .jpg) shown behind a glyph to draw over: fitted in -1..1, placed, faded. */
export interface Reference extends Placement {
  image: string;
  opacity?: number;
}

/** A reference's opacity when it doesn't give one (the file's default is 1; a new one starts faint). */
export const NEW_REFERENCE_OPACITY = 0.4;

const PLACEMENT_KEYS = ['move', 'turn', 'scale', 'skew'] as const;

/** The map `p` puts points through. */
export function placementAffine(p: Placement): Affine {
  const [sx, sy] = p.scale ?? [1, 1];
  return chain(scale(sx, sy), skew(p.skew ?? 0), rotate(p.turn ?? 0), translate(p.move ?? [0, 0]));
}

/**
 * `p` placed as the map `m` places (its other settings kept): a flip goes
 * on whichever axis leaves the smaller turn, so a mirror reads as one.
 * Unchanged if `m` flattens the plane.
 */
export function withPlacement<T extends Placement>(p: T, m: Affine): T {
  const ways = [decompose(m, false), decompose(m, true)].filter((w) => w !== null);
  if (!ways.length) return p;
  const best = ways.reduce((a, b) => (Math.abs(b.turn) < Math.abs(a.turn) - 1e-6 ? b : a));
  return cleanPlacement({ ...p, move: [m[4], m[5]], turn: best.turn, skew: best.skew, scale: best.scale });
}

const round = (v: number) => Number(num(v));

/** `p` without the parts that leave points where they are, its numbers as the file writes them. */
export function cleanPlacement<T extends Placement>(p: T): T {
  const out = { ...p };
  for (const key of PLACEMENT_KEYS) delete out[key];
  if (p.move && (round(p.move[0]) !== 0 || round(p.move[1]) !== 0)) out.move = [round(p.move[0]), round(p.move[1])];
  if (p.turn !== undefined && round(normalizeTurn(p.turn)) !== 0) out.turn = round(normalizeTurn(p.turn));
  // Never a scale of 0, nor a skew near a right angle: the engine refuses those.
  const nonzero = (s: number) => round(s) || (s < 0 ? -0.0001 : 0.0001);
  if (p.scale && (round(p.scale[0]) !== 1 || round(p.scale[1]) !== 1)) out.scale = [nonzero(p.scale[0]), nonzero(p.scale[1])];
  if (p.skew !== undefined && round(p.skew) !== 0) out.skew = round(Math.max(-MAX_SKEW, Math.min(MAX_SKEW, p.skew)));
  return out;
}

/** `target` placed as `p` places, its other settings kept. */
export function replacePlacement<T extends Placement>(target: T, p: Placement): T {
  const out = { ...target };
  for (const key of PLACEMENT_KEYS) delete out[key];
  return { ...out, ...cleanPlacement(p) };
}

/** What's wrong with `p`, as the engine would say; null if nothing. */
export function placementProblem(p: Placement): string | null {
  const numbers = [...(p.move ?? []), ...(p.scale ?? []), p.turn ?? 0, p.skew ?? 0];
  if (numbers.some((v) => typeof v !== 'number' || !Number.isFinite(v))) return 'every number should be finite';
  if (p.scale?.includes(0)) return "a scale can't be 0";
  if (Math.abs(p.skew ?? 0) >= 89) return `the skew should be under 89 degrees either way, got ${p.skew}`;
  return null;
}

/** `path` with every point, controls too, put through `m`. */
export function placePath(path: GlyphPath, m: Affine): GlyphPath {
  const f = (q: Point): Point => ({ x: m[0] * q.x + m[2] * q.y + m[4], y: m[1] * q.x + m[3] * q.y + m[5] });
  return path.map((stroke) => ({
    start: f(stroke.start),
    closed: stroke.closed,
    segments: stroke.segments.map((s): Segment =>
      s.kind === 'L' ? { kind: 'L', to: f(s.to) }
        : s.kind === 'Q' ? { kind: 'Q', c: f(s.c), to: f(s.to) }
          : { kind: 'C', c1: f(s.c1), c2: f(s.c2), to: f(s.to) }),
  }));
}

// ---- reading and writing ----

/** `value`, a placement's parts, checked for their types; throws, saying `what` it's of, if they're wrong. */
function readPlacement(what: string, value: Record<string, unknown>): Placement {
  const pair = (key: string) => {
    const v = value[key];
    if (v === undefined) return undefined;
    if (!Array.isArray(v) || v.length !== 2 || v.some((n) => typeof n !== 'number')) throw new Error(`${what}: ${key} should be [x, y]`);
    return v as [number, number];
  };
  const number = (key: string) => {
    const v = value[key];
    if (v !== undefined && typeof v !== 'number') throw new Error(`${what}: ${key} should be a number`);
    return v as number | undefined;
  };
  const out: Placement = { move: pair('move'), turn: number('turn'), scale: pair('scale'), skew: number('skew') };
  for (const key of PLACEMENT_KEYS) if (out[key] === undefined) delete out[key];
  return out;
}

function checkKeys(what: string, value: Record<string, unknown>, allowed: readonly string[]) {
  const unknown = Object.keys(value).find((key) => !allowed.includes(key));
  if (unknown !== undefined) throw new Error(`${what}: '${unknown}' isn't one of ${allowed.join(', ')}`);
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** A linked glyph as the file has it (JSON text); throws if it isn't one. */
export function readLink(what: string, raw: string): GlyphLink {
  const value: unknown = JSON.parse(raw);
  if (!isObject(value) || typeof value['from'] !== 'string') throw new Error(`${what} should be path data, a list of it, or a linked glyph, { "from": ... }`);
  checkKeys(what, value, ['from', ...PLACEMENT_KEYS, 'add']);
  if (value['add'] !== undefined && typeof value['add'] !== 'string') throw new Error(`${what}: add should be path data`);
  const link: GlyphLink = { from: value['from'], ...readPlacement(what, value) };
  if (value['add'] !== undefined) link.add = value['add'] as string;
  return link;
}

/** A glyphset's references (JSON text), by glyph key; throws if they aren't. */
export function readReferences(raw: string): [string, Reference][] {
  const value: unknown = JSON.parse(raw);
  if (!isObject(value)) throw new Error('references should map glyphs to their pictures');
  return Object.entries(value).map(([key, entry]) => {
    const what = `the reference for '${key}'`;
    if (!isObject(entry) || typeof entry['image'] !== 'string') throw new Error(`${what} should be { "image": NAME, ... }`);
    checkKeys(what, entry, ['image', ...PLACEMENT_KEYS, 'opacity']);
    if (entry['opacity'] !== undefined && typeof entry['opacity'] !== 'number') throw new Error(`${what}: opacity should be a number`);
    const reference: Reference = { image: entry['image'], ...readPlacement(what, entry) };
    if (entry['opacity'] !== undefined) reference.opacity = entry['opacity'] as number;
    return [key, reference];
  });
}

/** A placement's parts as the file writes them: `"move": [0.1, 0]`, ... */
function placementParts(p: Placement): string[] {
  const parts: string[] = [];
  if (p.move) parts.push(`"move": [${num(p.move[0])}, ${num(p.move[1])}]`);
  if (p.turn !== undefined) parts.push(`"turn": ${num(p.turn)}`);
  if (p.scale) parts.push(`"scale": [${num(p.scale[0])}, ${num(p.scale[1])}]`);
  if (p.skew !== undefined) parts.push(`"skew": ${num(p.skew)}`);
  return parts;
}

/** A linked glyph as the file has it, on one line. */
export function writeLink(link: GlyphLink): string {
  const parts = [`"from": ${JSON.stringify(link.from)}`, ...placementParts(link)];
  if (link.add !== undefined) parts.push(`"add": ${JSON.stringify(link.add)}`);
  return `{${parts.join(', ')}}`;
}

/** A reference as the file has it, on one line. */
export function writeReference(reference: Reference): string {
  const parts = [`"image": ${JSON.stringify(reference.image)}`, ...placementParts(reference)];
  if (reference.opacity !== undefined) parts.push(`"opacity": ${num(reference.opacity)}`);
  return `{${parts.join(', ')}}`;
}

// ---- drawing ----

/** A glyph's variants as drawn, or why they can't be. */
export type Drawn = { paths: GlyphPath[]; error: null } | { paths: null; error: string };

/**
 * Every glyph of `font` as it's drawn, by key: a linked glyph as the one
 * it's linked to, placed, its own strokes over each variant; as the
 * engine draws them, failing where it would.
 */
export function drawnGlyphs(font: Font): Map<string, Drawn> {
  const byKey = new Map(font.glyphs.map((glyph) => [glyph.key, glyph]));
  const done = new Map<string, Drawn>();
  const draw = (glyph: Glyph, seen: string[]): Drawn => {
    const known = done.get(glyph.key);
    if (known) return known;
    if (seen.includes(glyph.key)) return { paths: null, error: `glyphs are linked in a loop: ${[...seen, glyph.key].join(' -> ')}` };
    let drawn: Drawn;
    try {
      drawn = { paths: drawLinkOrPaths(glyph, byKey, (parent) => draw(parent, [...seen, glyph.key])), error: null };
    } catch (err) {
      drawn = { paths: null, error: (err as Error).message };
    }
    // A loop is only known once it's come back round: don't keep what's
    // drawn partway round one.
    if (!seen.length || drawn.paths) done.set(glyph.key, drawn);
    return drawn;
  };
  for (const glyph of font.glyphs) draw(glyph, []);
  return done;
}

function drawLinkOrPaths(glyph: Glyph, byKey: Map<string, Glyph>, parentOf: (glyph: Glyph) => Drawn): GlyphPath[] {
  const link = glyph.link;
  if (!link) return glyph.variants.map(parsePath);
  const parent = byKey.get(link.from);
  if (!parent) throw new Error(`'${glyph.key}' is linked to '${link.from}', which isn't a glyph`);
  const problem = placementProblem(link);
  if (problem) throw new Error(problem);
  const own = parsePath(link.add ?? '');
  const drawn = parentOf(parent);
  if (drawn.error !== null) throw new Error(drawn.error.startsWith('glyphs are linked in a loop') ? drawn.error : `its parent '${link.from}' can't be drawn`);
  const m = placementAffine(link);
  return drawn.paths.map((path) => [...placePath(path, m), ...own]);
}

/** A glyph's variants as path data, as drawn; none if they can't be. */
export function drawnData(drawn: Drawn | undefined): string[] {
  return drawn?.paths?.map(formatPath) ?? [];
}

/** The glyphs linked to `key`, directly or through others, and `key` itself. */
export function linkedTo(font: Font, key: string): Set<string> {
  const out = new Set([key]);
  for (let grew = true; grew;) {
    grew = false;
    for (const glyph of font.glyphs) {
      if (glyph.link && out.has(glyph.link.from) && !out.has(glyph.key)) {
        out.add(glyph.key);
        grew = true;
      }
    }
  }
  return out;
}

/** The ink box a picture `width` by `height` is fitted to, before it's placed: its longer side -1 to 1, centred. */
export function referenceBox(width: number, height: number): { x: number; y: number; w: number; h: number } {
  const k = 2 / Math.max(width, height, 1e-9);
  return { x: (-width * k) / 2, y: (-height * k) / 2, w: width * k, h: height * k };
}
