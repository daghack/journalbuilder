import { Component, ElementRef, computed, effect, inject, signal, viewChild, type OnDestroy } from '@angular/core';
import { FontEditorService } from './font-editor.service';
import * as edit from './glyph-edit';
import type { PointRef } from './glyph-edit';
import { bounds, dist, dot, ellipse, formatPath, nearestOnSegment, parsePath, type GlyphPath, type Point } from './glyph-path';

/** How near, in screen pixels, a click must be to a point or curve to take it. */
const HIT_PX = 8;
/** Grid steps snapped to, in glyph units. */
const SNAP = 0.05;

type Drag =
  | { kind: 'points'; from: Point; base: GlyphPath; refs: PointRef[]; lead: Point; moved: boolean }
  | { kind: 'marquee'; from: Point; to: Point; add: boolean }
  | { kind: 'ellipse'; centre: Point; base: GlyphPath; to: Point }
  | { kind: 'pan'; x: number; y: number; centre: Point };

/**
 * The glyph variant shown, large, to draw on: its points and curves to pick
 * and drag, with the pen tools adding to it. Glyph units, y down; the
 * font's guides and the glyph's other variants show behind.
 */
@Component({
  selector: 'app-glyph-canvas',
  host: { tabindex: '0', '(keydown)': 'onKey($event)', '[class.panning]': 'drag()?.kind === "pan"' },
  template: `
    <svg #svg [attr.viewBox]="viewBox()" (pointerdown)="down($event)" (pointermove)="move($event)" (pointerup)="up($event)"
         (pointercancel)="drag.set(null)" (dblclick)="doubleClick($event)" (wheel)="wheel($event)" [class]="'tool-' + editor.tool()">
      <g class="grid">
        @for (line of grid(); track line.key) {
          <line [attr.x1]="line.x1" [attr.y1]="line.y1" [attr.x2]="line.x2" [attr.y2]="line.y2" [class]="line.kind" />
        }
      </g>
      @for (guide of guideLines(); track guide.name) {
        <line class="guide" [attr.x1]="frame().x" [attr.x2]="frame().x + frame().w" [attr.y1]="guide.y" [attr.y2]="guide.y" />
        <text class="guide-label" [attr.x]="frame().x + 6 / scale()" [attr.y]="guide.y - 4 / scale()" [attr.font-size]="11 / scale()">{{ guide.name }}</text>
      }
      @if (editor.onion()) {
        @for (d of siblings(); track $index) { <path class="onion" [attr.d]="d" /> }
      }
      @if (shownData(); as d) {
        <path class="ink" [attr.d]="d" />
        <path class="centre" [attr.d]="d" />
      }
      @if (pickedSegmentData(); as d) { <path class="picked-segment" [attr.d]="d" /> }
      @for (h of handles(); track $index) {
        <line class="handle" [attr.x1]="h.a.x" [attr.y1]="h.a.y" [attr.x2]="h.b.x" [attr.y2]="h.b.y" />
      }
      @for (p of points(); track p.key) {
        @if (p.anchor) {
          <rect [attr.x]="p.point.x - r()" [attr.y]="p.point.y - r()" [attr.width]="2 * r()" [attr.height]="2 * r()"
                class="anchor" [class.picked]="p.picked" [class.start]="p.start" />
        } @else {
          <circle [attr.cx]="p.point.x" [attr.cy]="p.point.y" [attr.r]="r() * 0.9" class="control" [class.picked]="p.picked" />
        }
      }
      @if (pendingStart(); as p) {
        <circle class="pending" [attr.cx]="p.x" [attr.cy]="p.y" [attr.r]="r() * 1.4" />
      }
      @if (marquee(); as m) {
        <rect class="marquee" [attr.x]="m.x" [attr.y]="m.y" [attr.width]="m.w" [attr.height]="m.h" />
      }
    </svg>
    @if (editor.path()?.error; as error) {
      <p class="unreadable">This variant's path data doesn't read: {{ error }}. Fix it in the panel on the right.</p>
    } @else if (!editor.variant()) {
      <p class="unreadable">Pick a glyph on the left, or add one.</p>
    }
    <span class="readout">{{ readout() }}</span>
  `,
  styles: `
    :host { position: relative; display: block; min-height: 0; background: var(--paper); outline: none; overflow: hidden; }
    :host(:focus-visible) { box-shadow: inset 0 0 0 2px var(--accent); }
    svg { display: block; width: 100%; height: 100%; touch-action: none; user-select: none; }
    svg.tool-select { cursor: default; }
    svg:not(.tool-select) { cursor: crosshair; }
    :host(.panning) svg { cursor: grabbing; }
    .grid line { stroke: #e9e4da; stroke-width: 1px; vector-effect: non-scaling-stroke; }
    .grid line.major { stroke: #d6cfc2; }
    .grid line.axis { stroke: #b9b0a2; }
    .guide { stroke: #c9a227; stroke-width: 1px; stroke-dasharray: 6 4; vector-effect: non-scaling-stroke; }
    .guide-label { fill: #9a7c12; font-family: var(--font); }
    .onion { fill: none; stroke: #d7cfc3; stroke-width: 2px; vector-effect: non-scaling-stroke; }
    .ink { fill: none; stroke: #2b1d12; stroke-opacity: 0.18; stroke-width: 0.09; stroke-linecap: round; stroke-linejoin: round; }
    .centre { fill: none; stroke: #2b1d12; stroke-width: 1.5px; vector-effect: non-scaling-stroke; }
    .picked-segment { fill: none; stroke: #9b2318; stroke-width: 3px; vector-effect: non-scaling-stroke; }
    .handle { stroke: #8a8378; stroke-width: 1px; vector-effect: non-scaling-stroke; }
    .anchor { fill: #fff; stroke: #2b1d12; stroke-width: 1.5px; vector-effect: non-scaling-stroke; }
    .anchor.start { fill: #2b1d12; }
    .control { fill: #fff; stroke: #5a3d8a; stroke-width: 1.5px; vector-effect: non-scaling-stroke; }
    .picked { fill: #9b2318; stroke: #9b2318; }
    .pending { fill: #9b2318; stroke: none; }
    .marquee { fill: rgb(155 35 24 / 0.08); stroke: #9b2318; stroke-width: 1px; stroke-dasharray: 4 3; vector-effect: non-scaling-stroke; }
    .unreadable { position: absolute; left: 12px; right: 12px; top: 12px; margin: 0; font-size: 12px; color: var(--danger); background: var(--danger-bg); padding: 6px 8px; border-radius: 4px; }
    .readout { position: absolute; right: 8px; bottom: 6px; font: 11px var(--mono); color: #6b645a; pointer-events: none; }
  `,
})
export class GlyphCanvasComponent implements OnDestroy {
  protected readonly editor = inject(FontEditorService);
  private readonly svg = viewChild.required<ElementRef<SVGSVGElement>>('svg');

  /** The middle of the view, in glyph units, and pixels per unit. */
  private readonly centre = signal<Point>({ x: 0, y: 0 });
  protected readonly scale = signal(200);
  private readonly size = signal({ width: 600, height: 500 });
  private readonly observer = new ResizeObserver(([entry]) => {
    const first = this.size().width === 600 && this.size().height === 500;
    this.size.set({ width: entry.contentRect.width, height: entry.contentRect.height });
    if (first) this.fit();
  });

  protected readonly drag = signal<Drag | null>(null);
  /** Where a stroke begun with the stroke tool starts, until its first segment's added. */
  protected readonly pendingStart = signal<Point | null>(null);
  /** The path while it's dragged, before it's committed. */
  private readonly draft = signal<GlyphPath | null>(null);
  protected readonly cursor = signal<Point | null>(null);

  protected readonly shown = computed(() => this.draft() ?? this.editor.current());
  protected readonly shownData = computed(() => {
    const path = this.shown();
    return path ? formatPath(path) : null;
  });
  protected readonly frame = computed(() => {
    const { width, height } = this.size();
    const scale = this.scale();
    const c = this.centre();
    return { x: c.x - width / 2 / scale, y: c.y - height / 2 / scale, w: width / scale, h: height / scale };
  });
  protected readonly viewBox = computed(() => {
    const f = this.frame();
    return `${f.x} ${f.y} ${f.w} ${f.h}`;
  });
  /** A point's marker radius, in glyph units. */
  protected readonly r = computed(() => 4 / this.scale());

  protected readonly grid = computed(() => {
    const f = this.frame();
    const step = this.scale() > 150 ? 0.1 : 0.5;
    const lines: { key: string; x1: number; y1: number; x2: number; y2: number; kind: string }[] = [];
    const kind = (v: number) => (Math.abs(v) < 1e-9 ? 'axis' : Math.abs(v / 0.5 - Math.round(v / 0.5)) < 1e-6 ? 'major' : 'minor');
    for (let x = Math.ceil(f.x / step) * step; x <= f.x + f.w; x += step) {
      lines.push({ key: `x${x.toFixed(2)}`, x1: x, x2: x, y1: f.y, y2: f.y + f.h, kind: kind(x) });
    }
    for (let y = Math.ceil(f.y / step) * step; y <= f.y + f.h; y += step) {
      lines.push({ key: `y${y.toFixed(2)}`, x1: f.x, x2: f.x + f.w, y1: y, y2: y, kind: kind(y) });
    }
    return lines;
  });

  protected readonly guideLines = computed(() => {
    const g = this.editor.guides();
    const names: [string, number | undefined][] = [['cap height', g.cap], ['x-height', g.xHeight], ['baseline', g.baseline], ['descender', g.descender]];
    return names.filter((n): n is [string, number] => n[1] !== undefined).map(([name, y]) => ({ name, y }));
  });

  protected readonly siblings = computed(() => {
    const glyph = this.editor.glyph();
    const selection = this.editor.selection();
    if (!glyph || !selection) return [];
    return glyph.variants.filter((_, i) => i !== selection.variant);
  });

  protected readonly points = computed(() => {
    const path = this.shown();
    if (!path) return [];
    const picked = new Set(this.editor.picked());
    return edit.allPoints(path).map(({ ref, point }) => ({
      key: edit.refKey(ref), point, anchor: edit.isAnchor(ref), start: ref.role === 'start', picked: picked.has(edit.refKey(ref)),
    }));
  });

  protected readonly handles = computed(() => {
    const path = this.shown();
    if (!path) return [];
    return edit.segmentsOf(path).flatMap(({ from, segment }) =>
      segment.kind === 'L' ? []
        : segment.kind === 'Q' ? [{ a: from, b: segment.c }, { a: segment.c, b: segment.to }]
          : [{ a: from, b: segment.c1 }, { a: segment.c2, b: segment.to }]);
  });

  protected readonly pickedSegmentData = computed(() => {
    const ref = this.editor.pickedSegment();
    const path = this.shown();
    if (!ref || !path) return null;
    const found = edit.segmentsOf(path).find((s) => s.ref.s === ref.s && s.ref.i === ref.i);
    return found ? edit.segmentData(found.from, found.segment) : null;
  });

  protected readonly marquee = computed(() => {
    const drag = this.drag();
    if (drag?.kind !== 'marquee') return null;
    return {
      x: Math.min(drag.from.x, drag.to.x), y: Math.min(drag.from.y, drag.to.y),
      w: Math.abs(drag.to.x - drag.from.x), h: Math.abs(drag.to.y - drag.from.y),
    };
  });

  protected readonly readout = computed(() => {
    const c = this.cursor();
    const picked = this.editor.pickedRefs();
    const path = this.shown();
    const one = picked.length === 1 && path ? edit.getPoint(path, picked[0]) : null;
    const at = one ?? c;
    return at ? `${one ? 'picked ' : ''}${at.x.toFixed(3)}, ${at.y.toFixed(3)}` : '';
  });

  constructor() {
    effect(() => this.observer.observe(this.svg().nativeElement));
    // A different glyph: fit it in view.
    effect(() => {
      this.editor.selection();
      this.editor.fontName();
      this.pendingStart.set(null);
      queueMicrotask(() => this.fit());
    });
  }

  ngOnDestroy() {
    this.observer.disconnect();
  }

  /** Show the glyph, its siblings and the guides, whole. */
  fit() {
    const glyph = this.editor.glyph();
    let box = { minX: -1, minY: -1, maxX: 1, maxY: 1 };
    for (const d of glyph?.variants ?? []) {
      try {
        const b = bounds(parsePath(d));
        if (b) box = { minX: Math.min(box.minX, b.minX), minY: Math.min(box.minY, b.minY), maxX: Math.max(box.maxX, b.maxX), maxY: Math.max(box.maxY, b.maxY) };
      } catch {
        // Unreadable: fit the rest.
      }
    }
    const { width, height } = this.size();
    const w = box.maxX - box.minX, h = box.maxY - box.minY;
    this.scale.set(Math.max(20, Math.min((width - 40) / (w * 1.15), (height - 40) / (h * 1.15))));
    this.centre.set({ x: (box.minX + box.maxX) / 2, y: (box.minY + box.maxY) / 2 });
  }

  zoom(factor: number) {
    this.scale.update((s) => Math.min(5000, Math.max(20, s * factor)));
  }

  /** Where a pointer is, in glyph units. */
  private at(event: MouseEvent): Point {
    const rect = this.svg().nativeElement.getBoundingClientRect();
    const f = this.frame();
    return { x: f.x + (event.clientX - rect.left) / this.scale(), y: f.y + (event.clientY - rect.top) / this.scale() };
  }

  private snapped(p: Point, event: MouseEvent): Point {
    if (!this.editor.snap() || event.altKey) return p;
    return { x: Math.round(p.x / SNAP) * SNAP, y: Math.round(p.y / SNAP) * SNAP };
  }

  /** The point nearest `p`, if one's near enough to take. */
  private pointAt(p: Point): PointRef | null {
    const path = this.shown();
    if (!path) return null;
    let best: { ref: PointRef; d: number } | null = null;
    for (const { ref, point } of edit.allPoints(path)) {
      // Anchors before controls on top of them.
      const d = dist(point, p) * this.scale() - (edit.isAnchor(ref) ? 0.5 : 0);
      if (d <= HIT_PX && (!best || d < best.d)) best = { ref, d };
    }
    return best?.ref ?? null;
  }

  private segmentAt(p: Point): { ref: edit.SegmentRef; t: number } | null {
    const path = this.shown();
    if (!path) return null;
    let best: { ref: edit.SegmentRef; t: number; d: number } | null = null;
    for (const { ref, from, segment } of edit.segmentsOf(path)) {
      const near = nearestOnSegment(from, segment, p);
      const d = near.distance * this.scale();
      if (d <= HIT_PX && (!best || d < best.d)) best = { ref, t: near.t, d };
    }
    return best;
  }

  protected down(event: PointerEvent) {
    const host = this.svg().nativeElement.parentElement as HTMLElement;
    host.focus({ preventScroll: true });
    if (event.button === 1 || (event.button === 0 && event.shiftKey && event.altKey)) {
      event.preventDefault();
      this.drag.set({ kind: 'pan', x: event.clientX, y: event.clientY, centre: this.centre() });
      this.svg().nativeElement.setPointerCapture(event.pointerId);
      return;
    }
    if (event.button !== 0) return;
    const path = this.editor.current();
    if (!path) return;
    this.svg().nativeElement.setPointerCapture(event.pointerId);
    const p = this.at(event);
    const tool = this.editor.tool();
    this.editor.settle();

    if (tool === 'select') {
      const hit = this.pointAt(p);
      if (hit) {
        const key = edit.refKey(hit);
        let picked = this.editor.picked();
        if (event.shiftKey) picked = picked.includes(key) ? picked.filter((k) => k !== key) : [...picked, key];
        else if (!picked.includes(key)) picked = [key];
        this.editor.picked.set(picked);
        this.editor.pickedSegment.set(null);
        const lead = edit.getPoint(path, hit)!;
        this.drag.set({ kind: 'points', from: p, base: path, refs: picked.map(edit.parseRef), lead, moved: false });
        return;
      }
      const segment = this.segmentAt(p);
      if (segment) {
        this.editor.pickedSegment.set(segment.ref);
        this.editor.picked.set([]);
        return;
      }
      this.drag.set({ kind: 'marquee', from: p, to: p, add: event.shiftKey });
      if (!event.shiftKey) {
        this.editor.picked.set([]);
        this.editor.pickedSegment.set(null);
      }
      return;
    }

    const q = this.snapped(p, event);
    if (tool === 'dot') {
      this.editor.setPath([...path, dot(q)]);
      return;
    }
    if (tool === 'ellipse') {
      this.drag.set({ kind: 'ellipse', centre: q, base: path, to: q });
      return;
    }
    if (tool === 'stroke') {
      // Where the next stroke begins; it's added with its first segment.
      this.pendingStart.set(q);
      this.editor.picked.set([]);
      this.editor.pickedSegment.set(null);
      this.editor.tool.set('L');
      return;
    }
    // Pen tools: carry a stroke on (or the one just begun); then drag the new point into place.
    const start = this.pendingStart();
    this.pendingStart.set(null);
    const added = start
      ? edit.extend(edit.beginStroke(path, start).path, path.length, tool, q)
      : edit.extend(path, edit.strokeToExtend(path, this.editor.pickedRefs()), tool, q);
    this.draft.set(added.path);
    this.editor.picked.set([edit.refKey(added.anchor)]);
    this.editor.pickedSegment.set(null);
    this.drag.set({ kind: 'points', from: p, base: added.path, refs: [added.anchor], lead: q, moved: true });
  }

  protected move(event: PointerEvent) {
    const p = this.at(event);
    this.cursor.set(p);
    const drag = this.drag();
    if (!drag) return;
    if (drag.kind === 'pan') {
      this.centre.set({ x: drag.centre.x - (event.clientX - drag.x) / this.scale(), y: drag.centre.y - (event.clientY - drag.y) / this.scale() });
    } else if (drag.kind === 'marquee') {
      this.drag.set({ ...drag, to: p });
    } else if (drag.kind === 'ellipse') {
      const to = this.snapped(p, event);
      this.drag.set({ ...drag, to });
      this.draft.set([...drag.base, this.ellipseFor(drag.centre, to, event.shiftKey)]);
    } else {
      if (!drag.moved && dist(p, drag.from) * this.scale() < 3) return;
      // Snap the point grabbed; the others keep their places relative to it.
      const target = this.snapped({ x: drag.lead.x + p.x - drag.from.x, y: drag.lead.y + p.y - drag.from.y }, event);
      this.drag.set({ ...drag, moved: true });
      this.draft.set(edit.movePoints(drag.base, drag.refs, { x: target.x - drag.lead.x, y: target.y - drag.lead.y }));
    }
  }

  protected up(event: PointerEvent) {
    const drag = this.drag();
    this.drag.set(null);
    const draft = this.draft();
    this.draft.set(null);
    if (!drag) return;
    if (drag.kind === 'marquee') {
      const box = this.marqueeOf(drag);
      const path = this.shown();
      if (!path) return;
      const inside = edit.allPoints(path)
        .filter(({ point }) => point.x >= box.x && point.x <= box.x + box.w && point.y >= box.y && point.y <= box.y + box.h)
        .map(({ ref }) => edit.refKey(ref));
      this.editor.picked.set(drag.add ? [...new Set([...this.editor.picked(), ...inside])] : inside);
    } else if (drag.kind === 'ellipse') {
      const small = dist(drag.centre, drag.to) * this.scale() < 4;
      const shape = small ? ellipse(drag.centre.x, drag.centre.y, 0.2, 0.2) : this.ellipseFor(drag.centre, drag.to, event.shiftKey);
      this.editor.setPath([...drag.base, shape]);
    } else if (drag.kind === 'points' && drag.moved && draft) {
      this.editor.setPath(draft);
    }
  }

  private marqueeOf(drag: Extract<Drag, { kind: 'marquee' }>) {
    return { x: Math.min(drag.from.x, drag.to.x), y: Math.min(drag.from.y, drag.to.y), w: Math.abs(drag.to.x - drag.from.x), h: Math.abs(drag.to.y - drag.from.y) };
  }

  private ellipseFor(centre: Point, to: Point, circle: boolean) {
    let rx = Math.abs(to.x - centre.x), ry = Math.abs(to.y - centre.y);
    if (circle) rx = ry = Math.max(rx, ry);
    return ellipse(centre.x, centre.y, Math.max(rx, 0.01), Math.max(ry, 0.01));
  }

  /** Double-click a curve to add a point there. */
  protected doubleClick(event: MouseEvent) {
    if (this.editor.tool() !== 'select') return;
    const hit = this.segmentAt(this.at(event));
    const path = this.editor.current();
    if (!hit || !path) return;
    const split = edit.splitAt(path, hit.ref, hit.t);
    this.editor.setPath(split.path);
    this.editor.pickedSegment.set(null);
    this.editor.picked.set([edit.refKey(split.anchor)]);
  }

  protected wheel(event: WheelEvent) {
    event.preventDefault();
    if (event.ctrlKey || event.metaKey) {
      // Zoom about the pointer.
      const before = this.at(event);
      this.zoom(Math.exp(-event.deltaY / 300));
      const after = this.at(event);
      this.centre.update((c) => ({ x: c.x + before.x - after.x, y: c.y + before.y - after.y }));
    } else {
      this.centre.update((c) => ({ x: c.x + event.deltaX / this.scale(), y: c.y + event.deltaY / this.scale() }));
    }
  }

  protected onKey(event: KeyboardEvent) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const step = event.shiftKey ? 0.1 : 0.01;
    const arrows: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const tools: Record<string, Parameters<FontEditorService['tool']['set']>[0]> = { v: 'select', m: 'stroke', l: 'L', q: 'Q', c: 'C', d: 'dot', e: 'ellipse' };
    if (event.key in arrows) {
      event.preventDefault();
      this.editor.nudge(...arrows[event.key]);
    } else if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault();
      this.editor.deletePicked();
    } else if (event.key === 'Escape') {
      this.pendingStart.set(null);
      this.editor.picked.set([]);
      this.editor.pickedSegment.set(null);
      this.editor.tool.set('select');
    } else if (event.key.toLowerCase() in tools) {
      this.editor.tool.set(tools[event.key.toLowerCase()]);
    } else if (event.key === 'f') {
      this.fit();
    }
  }
}
