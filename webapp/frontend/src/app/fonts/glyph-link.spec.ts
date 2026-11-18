import { rotate, scale } from '../journal/canvas-edits';
import { checkFonts, parseFont, serializeFont } from './font-file';
import { drawnData, drawnGlyphs, linkedTo, placementAffine, replacePlacement, withPlacement, type GlyphLink, type Reference } from './glyph-link';

const GLYPHSET = `{
\t"references": {
\t\t"d": {"image": "scan", "move": [0.1, -0.2], "scale": [1.5, 1.5], "opacity": 0.4}
\t},
\t"glyphs": {
\t\t"d": ["M 0.5 -1 L 0.5 0", "M 0.4 -1 L 0.4 0"],
\t\t"b": {"from": "d", "scale": [-1, 1]},
\t\t"p": {"from": "b", "move": [0, 0.5], "turn": 90, "add": "M 0 0 L 0.1 0"}
\t}
}
`;

describe('linked glyphs and references', () => {
  it('reads and writes them as the file has them', () => {
    const font = parseFont('f', GLYPHSET);
    expect(font.glyphs.map((g) => g.link?.from ?? null)).toEqual([null, 'd', 'b']);
    expect(font.glyphs[2].link).toEqual({ from: 'b', move: [0, 0.5], turn: 90, add: 'M 0 0 L 0.1 0' });
    expect(font.glyphs[0].reference).toEqual({ image: 'scan', move: [0.1, -0.2], scale: [1.5, 1.5], opacity: 0.4 });
    expect(serializeFont(font)).toBe(GLYPHSET);
    expect(checkFonts([font])).toEqual([]);
  });

  it('draws a linked glyph as its parent, placed, with its own strokes, as the engine does', () => {
    const drawn = drawnGlyphs(parseFont('f', GLYPHSET));
    expect(drawnData(drawn.get('b'))).toEqual(['M -0.5 -1 L -0.5 0', 'M -0.4 -1 L -0.4 0']);
    // Turned clockwise (y down), then moved; its own strokes where they are.
    expect(drawnData(drawn.get('p'))[0]).toBe('M 1 0 L 0 0 M 0 0 L 0.1 0');
    expect(linkedTo(parseFont('f', GLYPHSET), 'd')).toEqual(new Set(['d', 'b', 'p']));
  });

  it('says what the engine says of links that go nowhere or loop', () => {
    const font = parseFont('f', '{"glyphs": {"a": {"from": "b"}, "b": {"from": "a"}, "c": {"from": "z"}, "e": {"from": "c", "scale": [0, 1]}}}');
    expect(checkFonts([font]).map((p) => p.message)).toEqual([
      'glyphs are linked in a loop: a -> b -> a',
      'glyphs are linked in a loop: b -> a -> b',
      "glyph 'c' is linked to 'z', which isn't a glyph",
      "glyph 'e': a scale can't be 0",
    ]);
    const drawn = drawnGlyphs(font);
    expect(drawn.get('a')?.error).toBe('glyphs are linked in a loop: a -> b -> a');
    expect(drawn.get('c')?.error).toBe("'c' is linked to 'z', which isn't a glyph");
    expect(() => parseFont('f', '{"glyphs": {"a": {"from": "b", "rotate": 3}}}')).toThrowError(/'rotate' isn't one of/);
    expect(() => parseFont('f', '{"references": {"z": {"image": "x"}}, "glyphs": {}}')).toThrowError(/references gives 'z', which isn't a glyph/);
  });

  it('reads a map back into a placement, a mirror as a mirror', () => {
    expect(withPlacement<GlyphLink>({ from: 'd' }, scale(-1, 1))).toEqual({ from: 'd', scale: [-1, 1] });
    expect(withPlacement<GlyphLink>({ from: 'd' }, scale(1, -1))).toEqual({ from: 'd', scale: [1, -1] });
    const p = { move: [0.2, -0.1] as [number, number], turn: 30, scale: [1.5, 0.5] as [number, number], skew: 10 };
    const m = placementAffine(p);
    const back = placementAffine(withPlacement({}, m));
    back.forEach((v, i) => expect(v).toBeCloseTo(m[i], 3));
    expect(withPlacement<Reference>({ image: 'x' }, rotate(90))).toEqual({ image: 'x', turn: 90 });
    expect(replacePlacement<GlyphLink>({ from: 'd', turn: 5, add: 'M 0 0' }, { scale: [2, 2] })).toEqual({ from: 'd', add: 'M 0 0', scale: [2, 2] });
  });
});
