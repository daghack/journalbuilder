// Gradients as data: what new_gradient takes, as one value, as a colour
// option of a file can be given one (and as the engine lists one that's a
// default).

import { parseColour, formatHsl } from './colour';
import { rhaiFloat, rhaiString } from './rhai-literal';

/** A gradient: its colours, each a CSS colour or `[position, colour]`, and how it's laid out. */
export interface GradientData {
  colors: (string | [number, string])[];
  from?: [number, number];
  to?: [number, number];
  center?: [number, number];
  radius?: number;
  vertical?: boolean;
}

/** What strokes can be drawn in. */
export type Paint = string | GradientData;

/** How a gradient's laid over strokes. */
export type Layout = 'along' | 'down' | 'across' | 'out';

/** A colour of a gradient, placed from 0 to 1. */
export interface Stop {
  at: number;
  colour: string;
}

export const LAYOUTS: { layout: Layout; label: string; title: string }[] = [
  { layout: 'along', label: 'Along each stroke', title: 'From each stroke\'s start to its end' },
  { layout: 'down', label: 'Down each glyph', title: 'From the top of each glyph to its bottom' },
  { layout: 'across', label: 'Across the page', title: 'From one point of the canvas to another' },
  { layout: 'out', label: 'Out from a point', title: 'Out from a point of the canvas to a radius' },
];

export function isGradient(v: unknown): v is GradientData {
  return typeof v === 'object' && v !== null && Array.isArray((v as GradientData).colors);
}

/** A gradient's colours, placed as new_gradient places them: unplaced ends at 0 and 1, others evenly between placed neighbours. */
export function stopsOf(g: GradientData): Stop[] {
  const given = g.colors.map((c) => (typeof c === 'string' ? { at: null as number | null, colour: c } : { at: c[0] as number | null, colour: c[1] }));
  if (!given.length) return [];
  const last = given.length - 1;
  given[0].at ??= 0;
  given[last].at ??= 1;
  for (let i = 0; i < last;) {
    let next = i + 1;
    while (given[next].at === null) next++;
    const [a, b] = [given[i].at!, given[next].at!];
    for (let k = i + 1; k < next; k++) given[k].at = a + ((b - a) * (k - i)) / (next - i);
    i = next;
  }
  return given.map(({ at, colour }) => ({ at: at!, colour }));
}

/** `g` with these colours, in order of place: bare if they're evenly spaced, so what's written reads simply. */
export function withStops(g: GradientData, stops: Stop[]): GradientData {
  const sorted = [...stops].sort((a, b) => a.at - b.at);
  const even = sorted.every(({ at }, i) => Math.abs(at - i / (sorted.length - 1)) < 1e-3);
  return { ...g, colors: sorted.map(({ at, colour }) => (even ? colour : [at, colour])) };
}

export function layoutOf(g: GradientData): Layout {
  return g.vertical ? 'down' : g.from || g.to ? 'across' : g.center ? 'out' : 'along';
}

/** `g` laid out another way, from `size` canvas units across: across all of it, or out from its middle. */
export function withLayout(g: GradientData, layout: Layout, size = 1000): GradientData {
  const colors = g.colors;
  switch (layout) {
    case 'along': return { colors };
    case 'down': return { colors, vertical: true };
    case 'across': return { colors, from: [0, 0], to: [size, 0] };
    case 'out': return { colors, center: [size / 2, size / 10], radius: size / 2 };
  }
}

/** The colour between two, a start for one put between them: as CSS, or the first as it's written if either isn't a colour this can read. */
export function between(a: string, b: string): string {
  const [x, y] = [parseColour(a), parseColour(b)];
  if (!x || !y) return a;
  const turn = ((y.h - x.h + 540) % 360) - 180;
  return formatHsl({ h: x.h + turn / 2, s: (x.s + y.s) / 2, l: (x.l + y.l) / 2, a: (x.a + y.a) / 2 });
}

/** The colours as a CSS gradient across a swatch. */
export function gradientCss(g: GradientData): string {
  const stops = stopsOf(g).map(({ at, colour }) => `${colour} ${+(at * 100).toFixed(2)}%`);
  return `linear-gradient(to right, ${stops.join(', ')})`;
}

/** A colour or gradient as rhai source: a string literal, or a call of new_gradient. */
export function paintSource(paint: Paint): string {
  if (typeof paint === 'string') return rhaiString(paint);
  const point = ([x, y]: [number, number]) => `vec2(${rhaiFloat(x)}, ${rhaiFloat(y)})`;
  const colors = paint.colors.map((c) => (typeof c === 'string' ? rhaiString(c) : `[${rhaiFloat(c[0])}, ${rhaiString(c[1])}]`));
  const options = [
    paint.vertical ? 'vertical: true' : null,
    paint.from ? `from: ${point(paint.from)}` : null,
    paint.to ? `to: ${point(paint.to)}` : null,
    paint.center ? `center: ${point(paint.center)}` : null,
    paint.radius !== undefined ? `radius: ${rhaiFloat(paint.radius)}` : null,
  ].filter((o) => o !== null);
  return `new_gradient([${colors.join(', ')}]${options.length ? `, #{ ${options.join(', ')} }` : ''})`;
}
