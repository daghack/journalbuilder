// Colours as journals write them: CSS strings, which the engine parses
// (hex, rgb(a), hsl(a) and named; named are left out here, so ordinary
// string options aren't taken for colours).

/** A colour as the picker works on it: hue 0-360, saturation, lightness and alpha 0-1. */
export interface Hsla {
  h: number;
  s: number;
  l: number;
  a: number;
}

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const NUMBER = '[+-]?(?:\\d+\\.?\\d*|\\.\\d+)';
const SEP = '(?:\\s*,\\s*|\\s+)';
/** `rgb(...)`, `rgba(...)`, `hsl(...)` or `hsla(...)`: 3 or 4 numbers, commas or spaces between, maybe `/` before alpha. */
const FUNCTION = new RegExp(
  `^(rgba?|hsla?)\\(\\s*(${NUMBER})(deg)?(%?)${SEP}(${NUMBER})(%?)${SEP}(${NUMBER})(%?)(?:\\s*[,/]\\s*(${NUMBER})(%?))?\\s*\\)$`,
  'i',
);

/** Whether `v` is a colour written as hex, rgb(a) or hsl(a). */
export function isColour(v: unknown): v is string {
  return typeof v === 'string' && parseColour(v) !== null;
}

/** `text` as HSL with alpha, or null if it isn't a hex, rgb(a) or hsl(a) colour. */
export function parseColour(text: string): Hsla | null {
  const t = text.trim();
  const hex = HEX.exec(t);
  if (hex) {
    let digits = hex[1];
    if (digits.length <= 4) digits = [...digits].map((d) => d + d).join('');
    const byte = (i: number) => parseInt(digits.slice(i, i + 2), 16) / 255;
    return fromRgb(byte(0), byte(2), byte(4), digits.length === 8 ? byte(6) : 1);
  }
  const f = FUNCTION.exec(t);
  if (!f) return null;
  const [, name, x, deg, xPct, y, yPct, z, zPct, alpha, alphaPct] = f;
  const a = alpha === undefined ? 1 : clamp(Number(alpha) / (alphaPct ? 100 : 1), 0, 1);
  if (name.toLowerCase().startsWith('hsl')) {
    if (xPct) return null;
    return { h: wrap(Number(x)), s: clamp(Number(y) / 100, 0, 1), l: clamp(Number(z) / 100, 0, 1), a };
  }
  if (deg) return null;
  const channel = (v: string, pct: string) => clamp(Number(v) / (pct ? 100 : 255), 0, 1);
  return fromRgb(channel(x, xPct), channel(y, yPct), channel(z, zPct), a);
}

/** `hsl(h, s%, l%)`, or `hsla(h, s%, l%, a)` when it's see-through. */
export function formatHsl({ h, s, l, a }: Hsla): string {
  const hsl = `${wrap(round(h, 0))}, ${round(clamp(s, 0, 1) * 100, 0)}%, ${round(clamp(l, 0, 1) * 100, 0)}%`;
  const alpha = round(clamp(a, 0, 1), 2);
  return alpha < 1 ? `hsla(${hsl}, ${alpha})` : `hsl(${hsl})`;
}

function fromRgb(r: number, g: number, b: number, a: number): Hsla {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return { h: 0, s: 0, l, a };
  const s = d / (1 - Math.abs(2 * l - 1));
  const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return { h: wrap(h * 60), s, l, a };
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

function wrap(degrees: number): number {
  return ((degrees % 360) + 360) % 360;
}

function round(v: number, places: number): number {
  const k = 10 ** places;
  return Math.round(v * k) / k;
}
