// Symbols content files, content/NAME.json: their text, `{ "base": 27,
// "text": "Hello, world" }`, made to fit the base as they're read (or,
// with "strict": true, written in it already), or the symbols themselves,
// `{ "base": 27, "symbols": [8, 9] }`; read as the engine reads and writes
// symbols as text (the repository's src/lockbuilder: base_n_symbols and
// symbols_to_string; src/journal/rhaientries/content.rs: read_symbols).

export type SymbolsFile = { base: number; text: string; strict?: boolean } | { base: number; symbols: number[] };

/** `text` as symbols in `base`, or what character doesn't fit it. */
export function symbolsOf(base: number, text: string): number[] {
  return [...text].map((c) => {
    const upper = c.toUpperCase();
    const letter = upper.length === 1 && upper >= 'A' && upper <= 'Z' ? upper.charCodeAt(0) - 65 : -1;
    const symbol = base === 10 ? (c >= '0' && c <= '9' ? Number(c) : -1)
      : base === 27 ? (c === ' ' ? 0 : letter >= 0 ? letter + 1 : -1)
      : letter >= 0 && letter < base ? letter : -1;
    if (symbol < 0) throw new Error(`'${c}' isn't a base-${base} symbol (${symbolHelp(base)})`);
    return symbol;
  });
}

/**
 * `text` made to fit `base`, as `symbols(base, text, #{ lenient: true })`
 * makes it: in capitals, what the base can't write dropped, and runs of
 * spaces and dashes one space where the base has one, not at either end.
 */
export function lenientText(base: number, text: string): string {
  const fits = (c: string) => {
    try {
      symbolsOf(base, c);
      return true;
    } catch {
      return false;
    }
  };
  const space = fits(' ');
  let out = '';
  let gap = false;
  for (const c of text.toUpperCase()) {
    if (c !== ' ' && fits(c)) {
      if (gap && out) out += ' ';
      gap = false;
      out += c;
    } else if (space && (/\s/.test(c) || c === '-' || c === '—' || c === '–')) {
      gap = true;
    }
  }
  return out;
}

/** A symbols file's symbols: its text made to fit its base (or, strict, as written), or the symbols it holds. */
export function symbolsIn(file: SymbolsFile): number[] {
  if ('symbols' in file) return file.symbols;
  return symbolsOf(file.base, file.strict ? file.text : lenientText(file.base, file.text));
}

/** A symbols file's text, as typed; or, for one holding symbols, its symbols as text. */
export function typedText(file: SymbolsFile): string {
  return 'symbols' in file ? textOf(file.base, file.symbols) : file.text;
}

/** Symbols in `base` as text: digits, letters, or letters and spaces. */
export function textOf(base: number, symbols: number[]): string {
  if (base === 10) return symbols.join('');
  if (base === 27) return symbols.map((s) => (s === 0 ? ' ' : String.fromCharCode(64 + s))).join('');
  return symbols.map((s) => String.fromCharCode(65 + s)).join('');
}

/** What symbols base `base` writes as text. */
export function symbolHelp(base: number): string {
  return base === 10 ? 'digits 0-9' : base === 27 ? 'letters and spaces' : `letters A-${String.fromCharCode(64 + base)}`;
}

/** A symbols file's text, or what's wrong with it. */
export function readSymbols(json: string): SymbolsFile {
  const value = JSON.parse(json) as { base?: unknown; text?: unknown; strict?: unknown; symbols?: unknown };
  const shape = 'a symbols file is { "base": N, "text": "..." } or { "base": N, "symbols": [...] }';
  if (!value || typeof value !== 'object' || typeof value.base !== 'number') throw new Error(shape);
  const base = value.base;
  if (!Number.isInteger(base) || base < 2 || base > 27) throw new Error(`the base must be a whole number from 2 to 27, got ${base}`);
  const extra = Object.keys(value).find((k) => !['base', 'text', 'strict', 'symbols'].includes(k));
  if (extra) throw new Error(`a symbols file has no '${extra}'`);
  if (typeof value.text === 'string' && value.symbols === undefined) {
    if (value.strict !== undefined && typeof value.strict !== 'boolean') throw new Error('strict is true or false');
    return value.strict ? { base, text: value.text, strict: true } : { base, text: value.text };
  }
  if (!Array.isArray(value.symbols) || value.text !== undefined || value.strict !== undefined) throw new Error(shape);
  const symbols = value.symbols;
  const bad = symbols.findIndex((s) => !Number.isInteger(s) || s < 0 || s >= base);
  if (bad >= 0) throw new Error(`symbol ${JSON.stringify(symbols[bad])} (item ${bad}) is outside base ${base}'s 0-${base - 1}`);
  return { base, symbols: symbols as number[] };
}

/** A symbols file as it's written. */
export function writeSymbols(file: SymbolsFile): string {
  if ('symbols' in file) return `{ "base": ${file.base}, "symbols": [${file.symbols.join(', ')}] }\n`;
  return `{ "base": ${file.base}, "text": ${JSON.stringify(file.text)}${file.strict ? ', "strict": true' : ''} }\n`;
}
