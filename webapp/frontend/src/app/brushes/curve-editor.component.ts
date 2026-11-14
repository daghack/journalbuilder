import { Component, computed, input, output, signal } from '@angular/core';

/** A curve's points, [x, y], from 0 to 1 across, in order. */
export type CurvePoints = [number, number][];

const W = 200;
const H = 96;
const PAD = 6;

/**
 * The curve `points` make as Darkly draws it, sampled `count` times
 * across: a natural cubic spline through them (straight between two).
 */
export function sampleCurve(points: CurvePoints, count = 64): [number, number][] {
  const p = [...points].sort((a, b) => a[0] - b[0]);
  if (p.length < 2) return p;
  const n = p.length;
  // Second derivatives of the natural spline: zero at the ends.
  const m = new Array<number>(n).fill(0);
  if (n > 2) {
    const h = p.slice(1).map((q, i) => Math.max(q[0] - p[i][0], 1e-6));
    const a: number[] = [], b: number[] = [], c: number[] = [], d: number[] = [];
    for (let i = 1; i < n - 1; i++) {
      a.push(h[i - 1]);
      b.push(2 * (h[i - 1] + h[i]));
      c.push(h[i]);
      d.push(6 * ((p[i + 1][1] - p[i][1]) / h[i] - (p[i][1] - p[i - 1][1]) / h[i - 1]));
    }
    for (let i = 1; i < b.length; i++) {
      const w = a[i] / b[i - 1];
      b[i] -= w * c[i - 1];
      d[i] -= w * d[i - 1];
    }
    for (let i = b.length - 1; i >= 0; i--) m[i + 1] = (d[i] - (i + 1 < b.length ? c[i] * m[i + 2] : 0)) / b[i];
  }
  const at = (x: number): number => {
    if (x <= p[0][0]) return p[0][1];
    if (x >= p[n - 1][0]) return p[n - 1][1];
    let i = 0;
    while (i < n - 2 && x > p[i + 1][0]) i++;
    const h = Math.max(p[i + 1][0] - p[i][0], 1e-6);
    const t = (x - p[i][0]) / h;
    const s = 1 - t;
    return s * p[i][1] + t * p[i + 1][1] + ((s * s * s - s) * m[i] + (t * t * t - t) * m[i + 1]) * (h * h) / 6;
  };
  return Array.from({ length: count + 1 }, (_, k) => [k / count, at(k / count)] as [number, number]);
}

/**
 * A curve input: drag a point to move it, click where there's none to add
 * one, double-click one (not an end) to take it away. Each move is sent as
 * it's made.
 */
@Component({
  selector: 'app-curve-editor',
  host: { '(pointerdown)': '$event.stopPropagation()' },
  template: `
    <svg [attr.viewBox]="'0 0 ' + w + ' ' + h" [attr.width]="w" [attr.height]="h"
         (pointerdown)="down($event)" (pointermove)="move($event)" (pointerup)="up($event)" (dblclick)="remove($event)">
      <rect class="frame" [attr.x]="pad" [attr.y]="pad" [attr.width]="w - 2 * pad" [attr.height]="h - 2 * pad" />
      <line class="grid" [attr.x1]="pad" [attr.y1]="h - pad" [attr.x2]="w - pad" [attr.y2]="pad" />
      <polyline class="curve" [attr.points]="line()" />
      @for (p of points(); track $index) {
        <circle class="point" [class.on]="dragging() === $index" [attr.cx]="sx(p[0])" [attr.cy]="sy(p[1])" r="4.5" [attr.data-index]="$index" />
      }
    </svg>
  `,
  styles: `
    :host { display: block; }
    svg { display: block; touch-action: none; cursor: crosshair; }
    .frame { fill: var(--editor-bg, #1e1b18); stroke: var(--border); }
    .grid { stroke: var(--border); stroke-dasharray: 3 3; }
    .curve { fill: none; stroke: #e2b45a; stroke-width: 1.6; }
    .point { fill: var(--surface); stroke: #e2b45a; stroke-width: 1.5; cursor: grab; }
    .point.on { fill: #e2b45a; }
  `,
})
export class CurveEditorComponent {
  readonly points = input.required<CurvePoints>();
  readonly changed = output<CurvePoints>();

  protected readonly w = W;
  protected readonly h = H;
  protected readonly pad = PAD;
  protected readonly dragging = signal<number | null>(null);
  protected readonly line = computed(() => sampleCurve(this.points()).map(([x, y]) => `${this.sx(x)},${this.sy(y)}`).join(' '));

  protected sx(x: number) {
    return PAD + x * (W - 2 * PAD);
  }
  protected sy(y: number) {
    return H - PAD - Math.min(1, Math.max(0, y)) * (H - 2 * PAD);
  }

  private at(event: PointerEvent | MouseEvent): [number, number] {
    const svg = (event.currentTarget as SVGSVGElement).getBoundingClientRect();
    const x = ((event.clientX - svg.left) / svg.width) * W;
    const y = ((event.clientY - svg.top) / svg.height) * H;
    const clamp = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 1000) / 1000;
    return [clamp((x - PAD) / (W - 2 * PAD)), clamp((H - PAD - y) / (H - 2 * PAD))];
  }

  protected down(event: PointerEvent) {
    const index = (event.target as Element).getAttribute('data-index');
    const svg = event.currentTarget as SVGSVGElement;
    svg.setPointerCapture(event.pointerId);
    if (index !== null) {
      this.dragging.set(Number(index));
      return;
    }
    // A new point, where the click was, among the others.
    const [x, y] = this.at(event);
    const points = [...this.points(), [x, y] as [number, number]].sort((a, b) => a[0] - b[0]);
    this.dragging.set(points.findIndex((p) => p[0] === x && p[1] === y));
    this.changed.emit(points);
  }

  protected move(event: PointerEvent) {
    const i = this.dragging();
    if (i === null) return;
    const points = this.points().map((p) => [...p] as [number, number]);
    let [x, y] = this.at(event);
    // The ends stay at the ends; the rest between their neighbours.
    if (i === 0) x = 0;
    else if (i === points.length - 1) x = 1;
    else x = Math.min(points[i + 1][0] - 0.001, Math.max(points[i - 1][0] + 0.001, x));
    points[i] = [x, y];
    this.changed.emit(points);
  }

  protected up(event: PointerEvent) {
    (event.currentTarget as SVGSVGElement).releasePointerCapture(event.pointerId);
    this.dragging.set(null);
  }

  protected remove(event: MouseEvent) {
    const index = (event.target as Element).getAttribute('data-index');
    const points = this.points();
    if (index === null || Number(index) === 0 || Number(index) === points.length - 1) return;
    this.changed.emit(points.filter((_, i) => i !== Number(index)));
  }
}
