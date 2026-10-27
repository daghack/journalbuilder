import { formatHsl, isColour, parseColour } from './colour';

describe('colours', () => {
  it('tells colours from other strings', () => {
    for (const c of ['#9b2318', '#fff', '#ffff', '#9b2318cc', 'rgb(0, 0, 0)', 'rgba(0, 0, 0, 0.7)', 'hsl(120, 100%, 50%)', 'hsla(120, 100%, 50%, 0.8)', 'HSL(120 100% 50% / 0.5)']) {
      expect(isColour(c), c).toBe(true);
    }
    for (const s of ['black', 'darkred', 'lorem', '#12', '#ggg', 'rgb(0, 0)', 'hsl(120, 100%)', 'rgb(1 2 3 4)', '', 3]) {
      expect(isColour(s), String(s)).toBe(false);
    }
  });

  it('reads hex, rgb and hsl as hsl', () => {
    expect(parseColour('#00ff00')).toEqual({ h: 120, s: 1, l: 0.5, a: 1 });
    expect(parseColour('#f00')).toEqual({ h: 0, s: 1, l: 0.5, a: 1 });
    expect(parseColour('#0000ff80')).toEqual({ h: 240, s: 1, l: 0.5, a: 128 / 255 });
    expect(parseColour('rgba(0, 0, 0, 0.7)')).toEqual({ h: 0, s: 0, l: 0, a: 0.7 });
    expect(parseColour('hsla(200, 50%, 25%, 0.4)')).toEqual({ h: 200, s: 0.5, l: 0.25, a: 0.4 });
    expect(parseColour('hsl(-30, 100%, 50%)')!.h).toBe(330);
    const brick = parseColour('#9b2318')!;
    expect(brick.h).toBeCloseTo(5.04, 2);
    expect(brick.s).toBeCloseTo(0.732, 3);
    expect(brick.l).toBeCloseTo(0.351, 3);
  });

  it('writes hsl, with alpha only when see-through', () => {
    expect(formatHsl({ h: 120, s: 1, l: 0.5, a: 1 })).toBe('hsl(120, 100%, 50%)');
    expect(formatHsl({ h: 4.6, s: 0.731, l: 0.351, a: 0.456 })).toBe('hsla(5, 73%, 35%, 0.46)');
    expect(formatHsl({ h: 359.7, s: 0, l: 1, a: 0.999 })).toBe('hsl(0, 0%, 100%)');
    expect(parseColour(formatHsl({ h: 200, s: 0.5, l: 0.25, a: 0.4 }))).toEqual({ h: 200, s: 0.5, l: 0.25, a: 0.4 });
  });
});
