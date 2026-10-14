import * as edit from './glyph-edit';
import { formatPath, parsePath } from './glyph-path';

const path = (d: string) => parsePath(d);
const d = (p: ReturnType<typeof parsePath>) => formatPath(p);

describe('glyph edits', () => {
  it('moves an anchor with the cubic controls beside it', () => {
    const base = path('M 0 0 C 0 1 1 1 1 0 C 1 -1 2 -1 2 0');
    const moved = edit.movePoints(base, [{ s: 0, i: 0, role: 'to' }], { x: 0.5, y: 0 });
    expect(d(moved)).toBe('M 0 0 C 0 1 1.5 1 1.5 0 C 1.5 -1 2 -1 2 0');
    // A control picked too moves once, not twice.
    const both = edit.movePoints(base, [{ s: 0, i: 0, role: 'to' }, { s: 0, i: 0, role: 'c2' }], { x: 0.5, y: 0 });
    expect(d(both)).toBe(d(moved));
  });

  it('joins the segments either side of a deleted anchor, and simplifies a curve losing a control', () => {
    expect(d(edit.deletePoints(path('M 0 0 L 1 0 L 2 1'), [{ s: 0, i: 0, role: 'to' }]))).toBe('M 0 0 L 2 1');
    expect(d(edit.deletePoints(path('M 0 0 L 1 0 L 2 1'), [{ s: 0, i: -1, role: 'start' }]))).toBe('M 1 0 L 2 1');
    expect(d(edit.deletePoints(path('M 0 0 Q 1 1 2 0'), [{ s: 0, i: 0, role: 'c' }]))).toBe('M 0 0 L 2 0');
    // A stroke left with nothing to draw goes.
    expect(d(edit.deletePoints(path('M 0 0 L 1 0 M 5 5 L 6 6'), [{ s: 0, i: 0, role: 'to' }]))).toBe('M 5 5 L 6 6');
  });

  it('breaks a stroke where a segment is deleted', () => {
    expect(d(edit.deleteSegment(path('M 0 0 L 1 0 L 2 0 L 3 0'), { s: 0, i: 1 }))).toBe('M 0 0 L 1 0 M 2 0 L 3 0');
  });

  it('splits a segment, picking the anchor added', () => {
    const split = edit.splitAt(path('M 0 0 L 2 0'), { s: 0, i: 0 }, 0.5);
    expect(d(split.path)).toBe('M 0 0 L 1 0 L 2 0');
    expect(split.anchor).toEqual({ s: 0, i: 0, role: 'to' });
  });

  it('carries on the stroke whose last anchor is picked', () => {
    const base = path('M 0 0 L 1 0 M 5 5 L 6 6');
    expect(edit.strokeToExtend(base, [{ s: 0, i: 0, role: 'to' }])).toBe(0);
    expect(edit.strokeToExtend(base, [{ s: 0, i: -1, role: 'start' }])).toBe(1);
    const extended = edit.extend(base, 0, 'C', { x: 1, y: 3 });
    expect(d(extended.path)).toBe('M 0 0 L 1 0 C 1 1 1 2 1 3 M 5 5 L 6 6');
    expect(extended.anchor).toEqual({ s: 0, i: 1, role: 'to' });
  });

  it('reverses a stroke, curves and all', () => {
    expect(d(edit.reverse(path('M 0 0 L 1 0 C 1 1 2 1 2 0'), 0))).toBe('M 2 0 C 2 1 1 1 1 0 L 0 0');
  });
});
