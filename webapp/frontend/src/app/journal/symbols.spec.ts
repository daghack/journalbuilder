import { readSymbols, symbolsOf, textOf, writeSymbols } from './symbols';

describe('symbols content', () => {
  it('reads and writes symbols as the engine does', () => {
    expect(symbolsOf(27, 'Hi there')).toEqual([8, 9, 0, 20, 8, 5, 18, 5]);
    expect(textOf(27, [8, 9, 0, 20])).toBe('HI T');
    expect(symbolsOf(10, '407')).toEqual([4, 0, 7]);
    expect(textOf(3, [0, 1, 2])).toBe('ABC');
    expect(() => symbolsOf(3, 'D')).toThrow("'D' isn't a base-3 symbol (letters A-C)");
  });

  it('checks a file', () => {
    const file = readSymbols('{ "base": 27, "symbols": [8, 9] }');
    expect(writeSymbols(file)).toBe('{ "base": 27, "symbols": [8, 9] }\n');
    expect(() => readSymbols('{ "base": 3, "symbols": [4] }')).toThrow("symbol 4 (item 0) is outside base 3's 0-2");
  });
});
