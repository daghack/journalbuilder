import { StringStream } from '@codemirror/language';
import { describe, expect, it } from 'vitest';
import { rhaiParser, type RhaiState } from './rhai-language';

const parser = rhaiParser(() => new Set(['new_canvas', 'art::snake_path']));

/** Each line's tokens as [text, token type] pairs, the state carried across lines. */
function tokens(source: string): [string, string | null][] {
  const state: RhaiState = parser.startState!(2);
  const out: [string, string | null][] = [];
  for (const line of source.split('\n')) {
    const stream = new StringStream(line, 2, 2);
    while (!stream.eol()) {
      const type = parser.token(stream, state);
      const text = stream.current();
      if (text.trim()) out.push([text, type]);
      stream.start = stream.pos;
    }
  }
  return out;
}

const typeOf = (source: string, text: string) => tokens(source).find(([t]) => t === text)?.[1];

describe('rhai tokens', () => {
  it('tells keywords, definitions, calls and the API apart', () => {
    const source = 'fn draw(c) { let p = new_canvas(); helper(p); c.add_path(p, style); }';
    expect(typeOf(source, 'fn')).toBe('keyword');
    expect(typeOf(source, 'draw')).toBe('def');
    expect(typeOf(source, 'let')).toBe('keyword');
    expect(typeOf(source, 'new_canvas')).toBe('api');
    expect(typeOf(source, 'helper')).toBe('call');
    expect(typeOf(source, 'add_path')).toBe('method');
  });

  it('reads namespaced API calls', () => {
    const t = tokens('art::snake_path(a, b)');
    expect(t[0]).toEqual(['art', 'namespace']);
    expect(t.find(([text]) => text === 'snake_path')?.[1]).toBe('api');
  });

  it('reads numbers, atoms, constants and properties', () => {
    expect(typeOf('let x = 0x1F;', '0x1F')).toBe('number');
    expect(typeOf('let x = 1_000.5e3;', '1_000.5e3')).toBe('number');
    expect(typeOf('let t = true;', 'true')).toBe('atom');
    expect(typeOf('const W = 1200.0;', 'W')).toBe('variableName.constant');
    expect(typeOf('brush.size = 2;', 'size')).toBe('property');
    expect(typeOf('this.x', 'this')).toBe('self');
  });

  it('carries block comments across lines, nested', () => {
    const t = tokens('/* a /* b */\nstill */ let x');
    expect(t.map(([, type]) => type)).toEqual(['comment', 'comment', 'keyword', 'variableName']);
  });

  it('reads strings, and code inside a template string', () => {
    const t = tokens('let s = `a ${ x + #{ k: 1 }.k } b`;');
    expect(t.find(([text]) => text === '${')?.[1]).toBe('interpolation');
    expect(t.find(([text]) => text === 'x')?.[1]).toBe('variableName');
    expect(t.find(([text]) => text === '#{')?.[1]).toBe('mapOpen');
    expect(t.filter(([, type]) => type === 'interpolation').length).toBe(2);
    expect(t.at(-2)).toEqual([' b`', 'string']);
    expect(typeOf('print("say \\"hi\\"")', '"say \\"hi\\""')).toBe('string');
    expect(typeOf("let c = 'x';", "'x'")).toBe('string');
  });

  it('ends a line comment at the line', () => {
    const t = tokens('// note\nlet a');
    expect(t).toEqual([['// note', 'comment'], ['let', 'keyword'], ['a', 'variableName']]);
  });
});
