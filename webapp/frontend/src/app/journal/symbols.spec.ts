import { lenientText, readSymbols, symbolsIn, symbolsOf, textOf, typedText, writeSymbols } from './symbols';

describe('symbols content', () => {
  it('reads and writes symbols as the engine does', () => {
    expect(symbolsOf(27, 'Hi there')).toEqual([8, 9, 0, 20, 8, 5, 18, 5]);
    expect(textOf(27, [8, 9, 0, 20])).toBe('HI T');
    expect(symbolsOf(10, '407')).toEqual([4, 0, 7]);
    expect(textOf(3, [0, 1, 2])).toBe('ABC');
    expect(() => symbolsOf(3, 'D')).toThrow("'D' isn't a base-3 symbol (letters A-C)");
    expect(() => symbolsOf(27, 'ß')).toThrow("'ß' isn't a base-27 symbol");
  });

  it('fits text to a base as the engine does', () => {
    // As src/journal/rhaientries/api_shape_tests.rs: lenient_symbols_keep_what_the_base_writes.
    expect(lenientText(27, "  It's a — test, 2 go!")).toBe('ITS A TEST GO');
    expect(lenientText(10, 'a1-2 3')).toBe('123');
  });

  it('checks a file of symbols', () => {
    const file = readSymbols('{ "base": 27, "symbols": [8, 9] }');
    expect(writeSymbols(file)).toBe('{ "base": 27, "symbols": [8, 9] }\n');
    expect(typedText(file)).toBe('HI');
    expect(() => readSymbols('{ "base": 3, "symbols": [4] }')).toThrow("symbol 4 (item 0) is outside base 3's 0-2");
  });

  it('checks a file of text, lenient unless strict', () => {
    const file = readSymbols('{ "base": 27, "text": "Hi, there" }');
    expect(symbolsIn(file)).toEqual(symbolsOf(27, 'HI THERE'));
    expect(writeSymbols(file)).toBe('{ "base": 27, "text": "Hi, there" }\n');
    const strict = readSymbols('{ "base": 27, "text": "Hi, there", "strict": true }');
    expect(writeSymbols(strict)).toBe('{ "base": 27, "text": "Hi, there", "strict": true }\n');
    expect(() => symbolsIn(strict)).toThrow("',' isn't a base-27 symbol");
    expect(() => readSymbols('{ "base": 27, "text": "HI", "symbols": [8, 9] }')).toThrow('a symbols file is');
    expect(() => readSymbols('{ "base": 27, "symbols": [8, 9], "strict": true }')).toThrow('a symbols file is');
  });
});
