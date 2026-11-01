import { between, gradientCss, isGradient, layoutOf, stopsOf, withLayout, withStops } from './gradient';

describe('gradients as data', () => {
  it('tells gradients from other values', () => {
    expect(isGradient({ colors: ['red', 'blue'] })).toBe(true);
    for (const v of ['red', 3, null, undefined, ['red', 'blue'], { from: [0, 0] }]) expect(isGradient(v), String(v)).toBe(false);
  });

  it('places colours as new_gradient does', () => {
    expect(stopsOf({ colors: ['a', 'b', 'c'] })).toEqual([{ at: 0, colour: 'a' }, { at: 0.5, colour: 'b' }, { at: 1, colour: 'c' }]);
    expect(stopsOf({ colors: ['a', [0.8, 'b'], 'c', 'd'] }).map((s) => +s.at.toFixed(4))).toEqual([0, 0.8, 0.9, 1]);
    expect(stopsOf({ colors: [[0.2, 'a'], [0.6, 'b']] }).map((s) => s.at)).toEqual([0.2, 0.6]);
  });

  it('writes colours bare when they are evenly spaced, and in order', () => {
    const g = { colors: [] };
    expect(withStops(g, [{ at: 0, colour: 'a' }, { at: 0.5, colour: 'b' }, { at: 1, colour: 'c' }]).colors).toEqual(['a', 'b', 'c']);
    expect(withStops(g, [{ at: 1, colour: 'c' }, { at: 0.25, colour: 'b' }, { at: 0, colour: 'a' }]).colors).toEqual([[0, 'a'], [0.25, 'b'], [1, 'c']]);
  });

  it('is laid out one way at a time', () => {
    const colors = ['a', 'b'];
    expect(layoutOf(withLayout({ colors }, 'along'))).toBe('along');
    expect(layoutOf(withLayout({ colors }, 'down'))).toBe('down');
    expect(layoutOf(withLayout({ colors }, 'across'))).toBe('across');
    expect(layoutOf(withLayout({ colors }, 'out'))).toBe('out');
    expect(withLayout(withLayout({ colors }, 'out'), 'across')).toEqual({ colors, from: [0, 0], to: [1000, 0] });
  });

  it('shows as a CSS gradient', () => {
    expect(gradientCss({ colors: ['red', [0.25, 'gold'], 'blue'] })).toBe('linear-gradient(to right, red 0%, gold 25%, blue 100%)');
  });

  it('finds a colour between two, the short way round the hues', () => {
    expect(between('hsl(350, 100%, 50%)', 'hsl(30, 100%, 50%)')).toBe('hsl(10, 100%, 50%)');
    expect(between('hsl(0, 0%, 0%)', 'hsl(0, 0%, 100%)')).toBe('hsl(0, 0%, 50%)');
    expect(between('gold', '#000')).toBe('gold');
  });
});
