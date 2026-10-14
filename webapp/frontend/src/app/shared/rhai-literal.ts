// Values written as rhai source, for scripts the editors generate.

/** `text` as a rhai string literal. */
export function rhaiString(text: string): string {
  let out = '"';
  for (const ch of text) {
    const code = ch.codePointAt(0)!;
    if (ch === '\\') out += '\\\\';
    else if (ch === '"') out += '\\"';
    else if (ch === '\n') out += '\\n';
    else if (ch === '\t') out += '\\t';
    else if (ch === '\r') out += '\\r';
    else if (code < 0x20 || code === 0x7f) out += `\\u${code.toString(16).padStart(4, '0')}`;
    else out += ch;
  }
  return out + '"';
}

/** Whether `name` can be written bare as a rhai map key or variable. */
export function isIdentifier(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}

/** A map key: bare if it can be, else quoted. */
export function rhaiKey(name: string): string {
  return isIdentifier(name) ? name : rhaiString(name);
}

/** A number as a rhai float literal (rhai tells 1 from 1.0). */
export function rhaiFloat(n: number): string {
  const s = String(n);
  return /[.eE]/.test(s) ? s : `${s}.0`;
}

/** A JSON-like value as rhai: strings, numbers (as `float` says), bools, arrays, maps. */
export function rhaiValue(value: unknown, float = false): string {
  if (value === null || value === undefined) return '()';
  if (typeof value === 'string') return rhaiString(value);
  if (typeof value === 'number') return float ? rhaiFloat(value) : Number.isInteger(value) ? String(value) : rhaiFloat(value);
  if (typeof value === 'boolean') return String(value);
  if (Array.isArray(value)) return `[${value.map((v) => rhaiValue(v)).join(', ')}]`;
  const entries = Object.entries(value as Record<string, unknown>);
  return entries.length ? `#{ ${entries.map(([k, v]) => `${rhaiKey(k)}: ${rhaiValue(v)}`).join(', ')} }` : '#{}';
}
