// Symbols content files, content/NAME.json: `{ "base": 27, "symbols": [8, 9] }`,
// written as the engine reads and writes symbols as text (the repository's
// src/lockbuilder: base_n_symbols and symbols_to_string).

export interface SymbolsFile {
  base: number;
  symbols: number[];
}

/** `text` as symbols in `base`, or what character doesn't fit it. */
export function symbolsOf(base: number, text: string): number[] {
  return [...text].map((c) => {
    const upper = c.toUpperCase();
    const letter = upper >= 'A' && upper <= 'Z' ? upper.charCodeAt(0) - 65 : -1;
    const symbol = base === 10 ? (c >= '0' && c <= '9' ? Number(c) : -1)
      : base === 27 ? (c === ' ' ? 0 : letter >= 0 ? letter + 1 : -1)
      : letter >= 0 && letter < base ? letter : -1;
    if (symbol < 0) throw new Error(`'${c}' isn't a base-${base} symbol (${symbolHelp(base)})`);
    return symbol;
  });
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
export function readSymbols(text: string): SymbolsFile {
  const value = JSON.parse(text) as Partial<SymbolsFile>;
  if (!value || typeof value !== 'object' || typeof value.base !== 'number' || !Array.isArray(value.symbols)) {
    throw new Error('a symbols file is { "base": N, "symbols": [...] }');
  }
  if (!Number.isInteger(value.base) || value.base < 2 || value.base > 27) throw new Error(`the base must be a whole number from 2 to 27, got ${value.base}`);
  const bad = value.symbols.findIndex((s) => !Number.isInteger(s) || s < 0 || s >= value.base!);
  if (bad >= 0) throw new Error(`symbol ${JSON.stringify(value.symbols[bad])} (item ${bad}) is outside base ${value.base}'s 0-${value.base - 1}`);
  return { base: value.base, symbols: value.symbols as number[] };
}

/** A symbols file as it's written. */
export function writeSymbols(file: SymbolsFile): string {
  return `{ "base": ${file.base}, "symbols": [${file.symbols.join(', ')}] }\n`;
}
