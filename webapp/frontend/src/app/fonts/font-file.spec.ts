import fontsJson from '../../../generated/fixtures/fonts-json';
import { checkFonts, fallbackLoop, jsonString, parseFonts, serializeFonts, withOption, type FontsFile } from './font-file';
import { formatPath, parsePath } from './glyph-path';

describe('the fonts file', () => {
  it('writes config/fonts.json back as it was', () => {
    expect(serializeFonts(parseFonts(fontsJson))).toBe(fontsJson);
  });

  it('keeps glyph keys in file order, integer-like ones included', () => {
    const fonts = parseFonts(fontsJson);
    const ternary = fonts.find((font) => font.name === 'balanced_ternary')!;
    const section = fontsJson.slice(fontsJson.indexOf('"balanced_ternary"'));
    const inFile = [...section.slice(0, section.indexOf('\t\t}')).matchAll(/^\t\t\t"([^"]+)":/gm)].map((m) => m[1]);
    // JSON.parse would move "0".."13" first.
    expect(ternary.glyphs.map((glyph) => glyph.key)).toEqual(inFile);
    expect(Object.keys(JSON.parse(fontsJson).balanced_ternary.glyphs)).not.toEqual(inFile);
    expect(fonts.map((font) => font.name)).toEqual(Object.keys(JSON.parse(fontsJson)));
  });

  it('changes one line when one variant changes', () => {
    const fonts = parseFonts(fontsJson);
    const plain = fonts.find((font) => font.name === 'plain')!;
    const a = plain.glyphs.find((glyph) => glyph.key === 'A')!;
    a.variants = [formatPath(parsePath('M -0.5 0.7 L 0 -0.7 L 0.5 0.7'))];
    const before = fontsJson.split('\n');
    const after = serializeFonts(fonts).split('\n');
    expect(after.length).toBe(before.length);
    const changed = after.filter((line, i) => line !== before[i]);
    expect(changed).toEqual(['\t\t\t"A": "M -0.5 0.7 L 0 -0.7 L 0.5 0.7",']);
  });

  it('writes strings as Python does', () => {
    expect(jsonString('é"\\\n', false)).toBe('"é\\"\\\\\\n"');
    expect(jsonString('é\u007f', true)).toBe('"\\u00e9\\u007f"');
  });

  it('adds options last and removes them', () => {
    const [font] = parseFonts('{"f": {"fallback": "g", "glyphs": {"A": "M 0 0 L 1 1"}}, "g": {"glyphs": {}}}');
    expect(withOption(font, 'space_width', '0.4').options).toEqual([['fallback', '"g"'], ['space_width', '0.4']]);
    expect(withOption(font, 'fallback', undefined).options).toEqual([]);
  });
});

describe('checking fonts', () => {
  const fonts = (text: string): FontsFile => parseFonts(text);

  it('finds nothing wrong with the repository fonts', () => {
    expect(checkFonts(parseFonts(fontsJson))).toEqual([]);
  });

  it('rejects what FontBook::from_bytes rejects', () => {
    const messages = checkFonts(fonts(`{
      "a": {"character_padding": -1, "space_width": 0, "fallback": "nope", "glyphs": {"A": [], "B": "L 0 0", "C": "M 0 0 A 1 1 0 0 0 1 1"}},
      "b": {"fallback": "c", "glyphs": {}},
      "c": {"fallback": "b", "glyphs": {}}
    }`)).map((problem) => problem.message);
    expect(messages).toEqual([
      'character_padding must be a number of at least 0, got -1',
      'space_width must be a positive number, got 0',
      "glyph 'A' has no variants",
      "glyph 'B': path data must start with a moveto, not 'L'",
      "glyph 'C': arcs ('A'/'a') aren't supported",
      "falls back to 'nope', which isn't a font",
      'fonts fall back in a loop: b -> c -> b',
      'fonts fall back in a loop: c -> b -> c',
    ]);
    expect(fallbackLoop(fonts('{"x": {"fallback": "x", "glyphs": {}}}'), 'x')).toEqual(['x', 'x']);
  });
});
