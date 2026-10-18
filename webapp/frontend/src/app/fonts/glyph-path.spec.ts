import fontsJson from '../../../generated/fixtures/fonts-json';
import { parseFonts } from './font-file';
import { convertSegment, ellipse, formatPath, nearestOnSegment, num, parsePath, pointAt, splitSegment, type GlyphPath, type Point, type Segment } from './glyph-path';

/** Every point of a path, in order. */
function points(path: GlyphPath): Point[] {
  return path.flatMap((stroke) => [stroke.start, ...stroke.segments.flatMap((s) => (s.kind === 'L' ? [s.to] : s.kind === 'Q' ? [s.c, s.to] : [s.c1, s.c2, s.to]))]);
}

describe('glyph paths', () => {
  it('reads and writes every glyph of the repository fonts', () => {
    let variants = 0;
    for (const font of parseFonts(fontsJson)) {
      for (const glyph of font.glyphs) {
        for (const d of glyph.variants) {
          const path = parsePath(d);
          const again = parsePath(formatPath(path));
          const [a, b] = [points(path), points(again)];
          expect(b.length).toBe(a.length);
          a.forEach((p, i) => {
            expect(Math.abs(p.x - b[i].x)).toBeLessThan(1e-4);
            expect(Math.abs(p.y - b[i].y)).toBeLessThan(1e-4);
          });
          expect(again.map((s) => s.closed)).toEqual(path.map((s) => s.closed));
          variants++;
        }
      }
    }
    expect(variants).toBeGreaterThan(390);
  });

  it('makes every command absolute', () => {
    expect(formatPath(parsePath('m 1 1 l 1 0 h 1 v 1 H 0 V 0 z'))).toBe('M 1 1 L 2 1 L 3 1 L 3 2 L 0 2 L 0 0 Z');
    expect(formatPath(parsePath('M 0 0 C 1 0 2 1 2 2 S 3 4 4 4'))).toBe('M 0 0 C 1 0 2 1 2 2 C 2 3 3 4 4 4');
    expect(formatPath(parsePath('M 0 0 Q 1 0 1 1 T 2 2'))).toBe('M 0 0 Q 1 0 1 1 Q 1 2 2 2');
    expect(formatPath(parsePath('M0,0 1,1 2,0'))).toBe('M 0 0 L 1 1 L 2 0');
  });

  it('fails as paths.rs does', () => {
    expect(() => parsePath('L 0 0')).toThrow("path data must start with a moveto, not 'L'");
    expect(() => parsePath('M 0 0 A 1 1 0 0 0 1 1')).toThrow("arcs ('A'/'a') aren't supported");
    expect(() => parsePath('M 0 0 L 1')).toThrow("'L' needs more numbers");
    expect(() => parsePath('M 0 0 # 1')).toThrow("unexpected '#' at byte 6");
  });

  it('writes numbers as num() does', () => {
    // As Python's "%.4f" % v: 0.12345 is a little over, 0.04375 a little under, 0.03125 exactly half.
    expect([0.12345, -0.00001, 1.0, 0.1, -2.5, 0.03125, -0.03125, 0.04375, 2.00015].map(num))
      .toEqual(['0.1235', '0', '1', '0.1', '-2.5', '0.0312', '-0.0312', '0.0437', '2.0002']);
  });

  it('splits curves where they are', () => {
    const from = { x: 0, y: 0 };
    const cubic: Segment = { kind: 'C', c1: { x: 0, y: 1 }, c2: { x: 1, y: 1 }, to: { x: 1, y: 0 } };
    const [first, second] = splitSegment(from, cubic, 0.3);
    const mid = pointAt(from, cubic, 0.3);
    expect(first.to).toEqual(mid);
    for (const t of [0.25, 0.5, 0.75]) {
      const a = pointAt(from, first, t), b = pointAt(from, cubic, 0.3 * t);
      expect(Math.abs(a.x - b.x) + Math.abs(a.y - b.y)).toBeLessThan(1e-9);
      const c = pointAt(mid, second, t), d = pointAt(from, cubic, 0.3 + 0.7 * t);
      expect(Math.abs(c.x - d.x) + Math.abs(c.y - d.y)).toBeLessThan(1e-9);
    }
  });

  it('turns lines and quadratics into the same cubics', () => {
    const from = { x: 0, y: 0 };
    const quad: Segment = { kind: 'Q', c: { x: 1, y: 2 }, to: { x: 2, y: 0 } };
    const cubic = convertSegment(from, quad, 'C');
    for (const t of [0.1, 0.5, 0.9]) {
      const a = pointAt(from, quad, t), b = pointAt(from, cubic, t);
      expect(Math.abs(a.x - b.x) + Math.abs(a.y - b.y)).toBeLessThan(1e-9);
    }
    expect(convertSegment(from, cubic, 'Q')).toEqual(quad);
    expect(nearestOnSegment(from, quad, pointAt(from, quad, 0.4)).t).toBeCloseTo(0.4, 3);
  });

  it('draws ellipses as fonts.json has them', () => {
    // An ellipse of radii 0.5 and 0.25 about the origin.
    expect(formatPath([ellipse(0, 0, 0.5, 0.25)])).toBe(
      'M 0 -0.25 C 0.2761 -0.25 0.5 -0.1381 0.5 0 C 0.5 0.1381 0.2761 0.25 0 0.25 C -0.2761 0.25 -0.5 0.1381 -0.5 0 C -0.5 -0.1381 -0.2761 -0.25 0 -0.25',
    );
  });
});
