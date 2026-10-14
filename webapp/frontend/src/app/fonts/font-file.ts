// config/fonts.json as the font editor holds it: fonts, their options and
// glyphs in file order, written back as scripts/plain_font.py writes it
// (`write_fonts`), so a glyph changed is a line changed.

import { parsePath } from './glyph-path';

/** A font: its options other than glyphs, and its glyphs, in file order. */
export interface Font {
  name: string;
  /** Each option's key and its JSON text as written, e.g. `['space_width', '0.31']`. */
  options: [string, string][];
  glyphs: Glyph[];
}

/** A glyph: its key (a character, or a name like "basic_eye") and its variants' path data. */
export interface Glyph {
  key: string;
  variants: string[];
  /** Written as a list, even with one variant. */
  list: boolean;
}

export type FontsFile = Font[];

/** The options a font can have besides its glyphs (see FontEntry in src/journal/fonts.rs). */
export const OPTION_KEYS = ['character_padding', 'space_width', 'fallback'] as const;
export type OptionKey = (typeof OPTION_KEYS)[number];

/** A space's width in fonts that don't give one, in font sizes (DEFAULT_SPACE_WIDTH). */
export const DEFAULT_SPACE_WIDTH = 0.5;

/** The glyph drawn for a character no font has (FALLBACK_GLYPH). */
export const FALLBACK_GLYPH = { font: 'symbols', key: 'X' };

// ---- reading ----

/** A JSON value, keeping objects' key order and the text each value was written as. */
type Json =
  | { kind: 'object'; entries: [string, Json][]; raw: string }
  | { kind: 'array'; items: Json[]; raw: string }
  | { kind: 'string'; value: string; raw: string }
  | { kind: 'other'; raw: string };

/** Parse JSON text, keeping key order (JSON.parse puts integer-like keys first). */
export function parseJson(text: string): Json {
  let pos = 0;
  const fail = (what: string): never => {
    const before = text.slice(0, pos);
    const line = before.split('\n').length;
    const column = pos - before.lastIndexOf('\n');
    throw new Error(`${what} at line ${line}, column ${column}`);
  };
  const space = () => {
    while (pos < text.length && ' \t\n\r'.includes(text[pos])) pos++;
  };
  const value = (): Json => {
    space();
    const start = pos;
    const c = text[pos];
    if (c === '{') {
      pos++;
      const entries: [string, Json][] = [];
      space();
      if (text[pos] === '}') {
        pos++;
        return { kind: 'object', entries, raw: text.slice(start, pos) };
      }
      for (;;) {
        space();
        if (text[pos] !== '"') fail('expected a key');
        const key = string();
        space();
        if (text[pos] !== ':') fail("expected ':'");
        pos++;
        entries.push([key, value()]);
        space();
        if (text[pos] === ',') {
          pos++;
          continue;
        }
        if (text[pos] === '}') {
          pos++;
          return { kind: 'object', entries, raw: text.slice(start, pos) };
        }
        fail("expected ',' or '}'");
      }
    }
    if (c === '[') {
      pos++;
      const items: Json[] = [];
      space();
      if (text[pos] === ']') {
        pos++;
        return { kind: 'array', items, raw: text.slice(start, pos) };
      }
      for (;;) {
        items.push(value());
        space();
        if (text[pos] === ',') {
          pos++;
          continue;
        }
        if (text[pos] === ']') {
          pos++;
          return { kind: 'array', items, raw: text.slice(start, pos) };
        }
        fail("expected ',' or ']'");
      }
    }
    if (c === '"') {
      const s = string();
      return { kind: 'string', value: s, raw: text.slice(start, pos) };
    }
    const match = /^(-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?|true|false|null)/.exec(text.slice(pos, pos + 64));
    if (!match) fail('expected a value');
    pos += match![0].length;
    return { kind: 'other', raw: match![0] };
  };
  const string = (): string => {
    const start = pos;
    pos++;
    while (pos < text.length && text[pos] !== '"') {
      if (text[pos] === '\\') pos++;
      else if (text.charCodeAt(pos) < 0x20) fail('a control character in a string');
      pos++;
    }
    if (pos >= text.length) fail('an unterminated string');
    pos++;
    return JSON.parse(text.slice(start, pos));
  };
  const parsed = value();
  space();
  if (pos < text.length) fail('unexpected text after the JSON');
  return parsed;
}

/** A fonts.json, in file order. Throws if it isn't one. */
export function parseFonts(text: string): FontsFile {
  const root = parseJson(text);
  if (root.kind !== 'object') throw new Error('a fonts file is an object of fonts by name');
  return root.entries.map(([name, entry]) => {
    if (entry.kind !== 'object') throw new Error(`font '${name}' should be an object, { glyphs, ... }`);
    const options: [string, string][] = [];
    let glyphs: Glyph[] | null = null;
    for (const [key, value] of entry.entries) {
      if (key !== 'glyphs') {
        options.push([key, value.raw]);
        continue;
      }
      if (value.kind !== 'object') throw new Error(`font '${name}': glyphs should be an object of glyphs by key`);
      glyphs = value.entries.map(([glyphKey, variants]): Glyph => {
        if (variants.kind === 'string') return { key: glyphKey, variants: [variants.value], list: false };
        if (variants.kind === 'array' && variants.items.every((item) => item.kind === 'string')) {
          return { key: glyphKey, variants: variants.items.map((item) => (item as { value: string }).value), list: true };
        }
        throw new Error(`font '${name}': glyph '${glyphKey}' should be path data, or a list of it`);
      });
    }
    if (!glyphs) throw new Error(`font '${name}' has no glyphs`);
    return { name, options, glyphs };
  });
}

// ---- writing ----

/** A string as Python's json.dumps writes it, ASCII only if `ascii`. */
export function jsonString(text: string, ascii: boolean): string {
  const json = JSON.stringify(text);
  return ascii ? json.replace(/[\u007f-￿]/g, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`) : json;
}

/** The fonts file as write_fonts writes it: one glyph to a line. */
export function serializeFonts(fonts: FontsFile): string {
  const lines = ['{'];
  fonts.forEach((font, i) => {
    lines.push(`\t${jsonString(font.name, true)}: {`);
    for (const [key, raw] of font.options) lines.push(`\t\t${jsonString(key, true)}: ${raw},`);
    lines.push('\t\t"glyphs": {');
    font.glyphs.forEach((glyph, j) => {
      const comma = j < font.glyphs.length - 1 ? ',' : '';
      const value = glyph.list || glyph.variants.length !== 1
        ? `[${glyph.variants.map((d) => jsonString(d, false)).join(', ')}]`
        : jsonString(glyph.variants[0], false);
      lines.push(`\t\t\t${jsonString(glyph.key, false)}: ${value}${comma}`);
    });
    lines.push('\t\t}');
    lines.push('\t}' + (i < fonts.length - 1 ? ',' : ''));
  });
  lines.push('}');
  return lines.join('\n') + '\n';
}

// ---- options ----

/** Option `key`'s JSON text, or undefined. */
export function optionRaw(font: Font, key: string): string | undefined {
  return font.options.find(([k]) => k === key)?.[1];
}

/** Option `key` as a number, if it's set. */
export function numberOption(font: Font, key: OptionKey): number | undefined {
  const raw = optionRaw(font, key);
  if (raw === undefined) return undefined;
  const value = JSON.parse(raw);
  return typeof value === 'number' ? value : NaN;
}

/** The font a font falls back to, if any. */
export function fallbackOf(font: Font): string | undefined {
  const raw = optionRaw(font, 'fallback');
  if (raw === undefined) return undefined;
  const value = JSON.parse(raw);
  return typeof value === 'string' ? value : undefined;
}

/** `font` with option `key` set to the JSON text `raw`, or removed with undefined; new options go last. */
export function withOption(font: Font, key: string, raw: string | undefined): Font {
  const at = font.options.findIndex(([k]) => k === key);
  const options = font.options.slice();
  if (raw === undefined) {
    if (at >= 0) options.splice(at, 1);
  } else if (at >= 0) {
    options[at] = [key, raw];
  } else {
    options.push([key, raw]);
  }
  return { ...font, options };
}

// ---- checking ----

/** A problem with the fonts, where it is, and what's wrong. */
export interface FontProblem {
  font: string;
  glyph?: string;
  variant?: number;
  option?: string;
  message: string;
}

/**
 * What FontBook::from_bytes (src/journal/fonts.rs) would reject: the
 * renderer is only given fonts with none.
 */
export function checkFonts(fonts: FontsFile): FontProblem[] {
  const problems: FontProblem[] = [];
  const names = new Set<string>();
  for (const font of fonts) {
    const problem = (message: string, where: Partial<FontProblem> = {}) => problems.push({ font: font.name, message, ...where });
    if (names.has(font.name)) problem(`there are two fonts named '${font.name}'`);
    names.add(font.name);
    for (const [key, raw] of font.options) {
      if (!(OPTION_KEYS as readonly string[]).includes(key)) {
        problem(`'${key}' isn't a font option; they're ${OPTION_KEYS.join(', ')}`, { option: key });
        continue;
      }
      let value: unknown;
      try {
        value = JSON.parse(raw);
      } catch {
        problem(`${key} isn't valid JSON: ${raw}`, { option: key });
        continue;
      }
      if (key === 'character_padding' && !(typeof value === 'number' && Number.isFinite(value) && value >= 0)) {
        problem(`character_padding must be a number of at least 0, got ${raw}`, { option: key });
      } else if (key === 'space_width' && !(typeof value === 'number' && Number.isFinite(value) && value > 0)) {
        problem(`space_width must be a positive number, got ${raw}`, { option: key });
      } else if (key === 'fallback' && typeof value !== 'string') {
        problem(`fallback must be a font's name, got ${raw}`, { option: key });
      }
    }
    const keys = new Set<string>();
    for (const glyph of font.glyphs) {
      if (keys.has(glyph.key)) problem(`there are two glyphs '${glyph.key}'`, { glyph: glyph.key });
      keys.add(glyph.key);
      if (glyph.variants.length === 0) problem(`glyph '${glyph.key}' has no variants`, { glyph: glyph.key });
      glyph.variants.forEach((d, variant) => {
        try {
          parsePath(d);
        } catch (err) {
          problem(`glyph '${glyph.key}': ${(err as Error).message}`, { glyph: glyph.key, variant });
        }
      });
    }
  }
  const byName = new Map(fonts.map((font) => [font.name, font]));
  for (const font of fonts) {
    const fallback = fallbackOf(font);
    if (fallback !== undefined && !byName.has(fallback)) {
      problems.push({ font: font.name, option: 'fallback', message: `falls back to '${fallback}', which isn't a font` });
    }
  }
  for (const font of fonts) {
    const loop = fallbackLoop(fonts, font.name);
    if (loop) problems.push({ font: font.name, option: 'fallback', message: `fonts fall back in a loop: ${loop.join(' -> ')}` });
  }
  return problems;
}

/** The loop of fallbacks starting at `name`, if its fallbacks loop. */
export function fallbackLoop(fonts: FontsFile, name: string): string[] | null {
  const byName = new Map(fonts.map((font) => [font.name, font]));
  const seen: string[] = [];
  let current: string | undefined = name;
  while (current !== undefined && byName.has(current)) {
    if (seen.includes(current)) return [...seen, current];
    seen.push(current);
    current = fallbackOf(byName.get(current)!);
  }
  return null;
}

/** The glyph keys a font has, with those its fallbacks add. */
export function coveredKeys(fonts: FontsFile, name: string): Set<string> {
  const byName = new Map(fonts.map((font) => [font.name, font]));
  const keys = new Set<string>();
  const seen = new Set<string>();
  let current: string | undefined = name;
  while (current !== undefined && byName.has(current) && !seen.has(current)) {
    seen.add(current);
    const font: Font = byName.get(current)!;
    for (const glyph of font.glyphs) keys.add(glyph.key);
    current = fallbackOf(font);
  }
  return keys;
}

/** Whether a font's glyphs are keyed -13 to 13, as ternary_font's are. */
export function isTernary(font: Font): boolean {
  return font.glyphs.length > 0 && font.glyphs.every((glyph) => /^-?\d+$/.test(glyph.key) && Math.abs(Number(glyph.key)) <= 13);
}
