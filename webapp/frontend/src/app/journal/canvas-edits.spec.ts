import type { Committed, Corners, Point } from '../scribe';
import {
  IDENTITY, about, align, apply, boxCorners, chain, cleanEdit, decompose, distribute, editAffine, editForGesture, editForWarp,
  inverse, parseEdits, placements, pruneEdits, rotate, scale, serializeEdits, skew, snapBox, solveAffine, translate, warpCorners,
  withAffine, type Affine, type CanvasEdit,
} from './canvas-edits';

const close = (a: number[], b: number[], digits = 6) => {
  expect(a.length).toBe(b.length);
  a.forEach((v, i) => expect(v).toBeCloseTo(b[i], digits));
};
const closePoints = (a: Point[], b: Point[]) => a.forEach((p, i) => close(p, b[i], 4));

describe('canvas edits', () => {
  it('read and write their file, leaving out what does nothing', () => {
    const text = serializeEdits({
      version: 1,
      canvases: {
        hero: { move: [12, -4.00001], turn: 375, scale: [1, -1], skew: 0, hidden: false, ink: { opacity: 1, color: '#7a1f1f' } },
        idle: { move: [0, 0], scale: [1, 1] },
      },
    });
    expect(JSON.parse(text!)).toEqual({ version: 1, canvases: { hero: { move: [12, -4], turn: 15, scale: [1, -1], ink: { color: '#7a1f1f' } } } });
    expect(parseEdits(text).canvases['hero'].turn).toBe(15);
    expect(text).toContain('"move": [12, -4],');
    const warped = serializeEdits({ version: 1, canvases: { w: { warp: [[0, 0], [6.5, -3], [0, 0], [-1e-7, 2]] } } });
    expect(warped).toContain('"warp": [[0, 0], [6.5, -3], [0, 0], [0, 2]]');
    expect(JSON.parse(warped!).canvases.w.warp[1]).toEqual([6.5, -3]);
    expect(serializeEdits({ version: 1, canvases: { idle: {} } })).toBeNull();
    expect(parseEdits('not json').canvases).toEqual({});
    expect(parseEdits(null).canvases).toEqual({});
    expect(cleanEdit({ scale: [0, 2], skew: 120 })).toEqual({ scale: [0.0001, 2], skew: 85 });
  });

  it('are pruned to the canvases a run committed', () => {
    const text = serializeEdits({ version: 1, canvases: { a: { turn: 5 }, b: { turn: 6 } } });
    expect(pruneEdits(text, ['a', 'b'])).toBe(text);
    expect(JSON.parse(pruneEdits(text, ['a'])!).canvases).toEqual({ a: { turn: 5 } });
    expect(pruneEdits(text, [])).toBeNull();
    expect(pruneEdits(null, [])).toBeNull();
    expect(pruneEdits('{ broken', [])).toBe('{ broken');
  });

  it('map as the engine maps: scale, skew, turn about the centre, then move', () => {
    const edit: CanvasEdit = { move: [5, 0], turn: 90, scale: [2, 1] };
    // (11, 0) is 1 right of the centre (10, 0): doubled, turned clockwise to below it, moved 5.
    close(apply(editAffine(edit, [10, 0]), [11, 0]), [15, 2]);
    close(apply(editAffine({}, [3, 4]), [7, 8]), [7, 8]);
  });

  it('decompose back to their turn, skew and scale', () => {
    for (const [turn, sk, sx, sy] of [[30, 10, 2, 0.5], [-120, -20, 1, 3], [0, 0, -1, 1], [170, 40, -2, -0.25]]) {
      const m = chain(scale(sx, sy), skew(sk), rotate(turn));
      const parts = decompose(m, sx < 0)!;
      close([parts.turn, parts.skew, ...parts.scale], [turn, sk, sx, sy]);
    }
    expect(decompose([1, 0, 2, 0, 0, 0])).toBeNull();
  });

  it('take a map back to an edit about the same centre', () => {
    const centre: Point = [50, 20];
    const m = chain(about(chain(scale(1.5, -1), skew(12), rotate(33)), centre), translate([7, -3]));
    const edit = withAffine({ hidden: true }, m, centre);
    expect(edit.hidden).toBe(true);
    close(editAffine(edit, centre), m);
  });

  it('solve what the script did after the commit, flat boxes too', () => {
    const local: Corners = boxCorners([0, 0, 10, 5]);
    const after: Affine = chain(rotate(30), scale(2, 2), translate([100, 40]));
    const shown = local.map((p) => apply(after, p)) as Corners;
    close(solveAffine(local, shown), after);
    const line: Corners = boxCorners([0, 0, 10, 0]);
    close(solveAffine(line, line.map((p) => apply(after, p)) as Corners), after);
    close(solveAffine(boxCorners([3, 3, 0, 0]), boxCorners([5, 4, 0, 0])), translate([2, 1]));
  });

  it('make a gesture on the page the edit that draws it so', () => {
    // A canvas boxed 0..10 x 0..10, edited, then turned and moved by the script.
    const box: [number, number, number, number] = [0, 0, 10, 10];
    const edit: CanvasEdit = { turn: 10, scale: [1.2, 1.2] };
    const after: Affine = chain(rotate(45), translate([100, 0]));
    const m = editAffine(edit, [5, 5]);
    const local = boxCorners(box).map((p) => apply(m, p)) as Corners;
    const committed: Committed = { name: 'c', line: 1, box, local, shown: [{ page: 0, corners: local.map((p) => apply(after, p)) as Corners }, { page: 1, corners: local }] };
    const [placed] = placements(committed, edit, 0);
    expect(placements(committed, edit, 2)).toEqual([]);
    const gesture = about(chain(rotate(20), scale(0.5, 2)), [110, 3]);
    const next = editForGesture(edit, placed, gesture);
    const drawn = (e: CanvasEdit) => boxCorners(box).map((p) => apply(after, apply(editAffine(e, [5, 5]), p)));
    closePoints(drawn(next), drawn(edit).map((p) => apply(gesture, p)));
  });

  it('pin corners where they are dragged on the page', () => {
    const box: [number, number, number, number] = [0, 0, 10, 10];
    const edit: CanvasEdit = { turn: 90 };
    const m = editAffine(edit, [5, 5]);
    const local = boxCorners(box).map((p) => apply(m, p)) as Corners;
    const after = translate([20, 0]);
    const committed: Committed = { name: 'c', line: 1, box, local, shown: [{ page: 0, corners: local.map((p) => apply(after, p)) as Corners }] };
    const [placed] = placements(committed, edit, 0);
    const before = warpCorners(edit, placed);
    const next = editForWarp(edit, placed, 1, [3, 4]);
    const corners = warpCorners(next, { ...placed });
    closePoints(corners, before.map((p, i) => (i === 1 ? [p[0] + 3, p[1] + 4] : p)));
  });

  it('snap, align and distribute boxes', () => {
    expect(snapBox([12, 0, 22, 10], [10, 50], [100], 3)).toEqual({ by: [-2, 0], guides: [{ x: 10 }] });
    expect(snapBox([12, 0, 22, 10], [10], [11], 3)).toEqual({ by: [-2, 1], guides: [{ x: 10 }, { y: 11 }] });
    expect(align([[0, 0, 10, 10], [20, 5, 30, 25]], 'right', [0, 0, 100, 100])).toEqual([[90, 0], [70, 0]]);
    expect(align([[0, 0, 10, 10]], 'middle', [0, 0, 100, 100])).toEqual([[0, 45]]);
    expect(distribute([[0, 0, 10, 1], [50, 0, 60, 1], [12, 0, 22, 1]], 'x')).toEqual([[0, 0], [0, 0], [13, 0]]);
    expect(distribute([[0, 0, 1, 1], [5, 5, 6, 6]], 'y')).toEqual([[0, 0], [0, 0]]);
  });

  it('invert maps', () => {
    const m = chain(rotate(17), skew(5), translate([3, 9]));
    close(chain(m, inverse(m)!), IDENTITY);
  });
});
