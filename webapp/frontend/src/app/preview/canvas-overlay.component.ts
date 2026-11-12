import { Component, ElementRef, computed, effect, inject, input, signal, viewChild } from '@angular/core';
import {
  about, boundsOf, chain, editForWarp, inverse, isIdentity, rotate, scale, snapBox, snapLines, svgMatrix, then, translate, unionBounds, warpCorners,
  type Affine, type Bounds, type CanvasEdit, type Guide, type Placement,
} from '../journal/canvas-edits';
import type { Corners, Point } from '../scribe';
import type { Page } from '../services/journal.service';
import { VisualEditService, type Placed } from '../services/visual-edit.service';

/** A handle's size on screen, in pixels. */
const HANDLE_PX = 9;
/** How far above its box the turning handle sits, in pixels. */
const TURN_PX = 26;
/** How near, in pixels, a box snaps to a line. */
const SNAP_PX = 6;

type Handle =
  | { kind: 'corner'; i: number }
  | { kind: 'edge'; i: number }
  | { kind: 'turn' }
  | { kind: 'warp'; name: string; i: number };

/** A press on the page, while it's held. */
type Gesture =
  | { kind: 'move'; from: Point; names: string[]; bounds: Bounds; others: Bounds[] }
  | { kind: 'corner' | 'edge'; i: number; from: Point; names: string[]; frame: Corners }
  | { kind: 'turn'; from: Point; names: string[]; centre: Point }
  | { kind: 'warp'; from: Point; name: string; i: number; edit: CanvasEdit; placement: Placement; key: string }
  | { kind: 'marquee'; from: Point; to: Point; add: boolean };

let gestures = 0;

/**
 * The page's SVG, inline, with the canvases committed to the visual editor
 * boxed over it, to select and move, turn, scale, stretch, skew and pin
 * with handles. While a handle's held, the canvases' strokes (their
 * `<g data-commit>` groups) are moved by an SVG transform: no script runs
 * until it's let go.
 */
@Component({
  selector: 'app-canvas-overlay',
  template: `
    <div class="frame" [style.width.px]="size()[0]" [style.height.px]="size()[1]">
      <div #art class="art"></div>
      <svg class="handles" [attr.viewBox]="viewBox()" (pointerdown)="down($event)" (pointermove)="move($event)"
           (pointerup)="up()" (pointercancel)="cancel()" (lostpointercapture)="cancel()">
        @for (item of items(); track item.key) {
          <polygon class="box" [attr.points]="points(item.corners)" [class.selected]="item.selected"
                   [class.hidden-ink]="item.edit.hidden" [class.locked]="item.edit.locked" [class.copy]="item.copy > 0">
            <title>{{ item.name }}{{ item.edit.locked ? ' (locked)' : '' }}{{ item.edit.hidden ? ' (hidden)' : '' }}</title>
          </polygon>
        }
        @if (frame(); as f) {
          @if (edit.tool() === 'select') {
            <line class="stem" [attr.x1]="f.topMid[0]" [attr.y1]="f.topMid[1]" [attr.x2]="f.turn[0]" [attr.y2]="f.turn[1]" />
            <circle class="handle turn" data-handle="turn" [attr.cx]="f.turn[0]" [attr.cy]="f.turn[1]" [attr.r]="unit() * 5" />
            @for (c of f.corners; track $index) {
              <rect class="handle" [attr.data-handle]="'corner-' + $index" [attr.x]="c[0] - unit() * 4.5" [attr.y]="c[1] - unit() * 4.5"
                    [attr.width]="unit() * 9" [attr.height]="unit() * 9" [style.cursor]="$index % 2 ? 'nesw-resize' : 'nwse-resize'" />
            }
            @for (m of f.edges; track $index) {
              @if (m) {
                <rect class="handle edge" [attr.data-handle]="'edge-' + $index" [attr.x]="m[0] - unit() * 3.5" [attr.y]="m[1] - unit() * 3.5"
                      [attr.width]="unit() * 7" [attr.height]="unit() * 7" [style.cursor]="$index % 2 ? 'ew-resize' : 'ns-resize'" />
              }
            }
          }
        }
        @if (edit.tool() === 'warp') {
          @for (w of warpHandles(); track w.name + w.i) {
            <circle class="handle warp" [attr.data-handle]="'warp-' + w.i + '-' + w.name" [attr.cx]="w.at[0]" [attr.cy]="w.at[1]" [attr.r]="unit() * 5" />
          }
        }
        @for (g of guides(); track $index) {
          @if ('x' in g) {
            <line class="guide" [attr.x1]="g.x" [attr.x2]="g.x" [attr.y1]="view()[1] - 1e4" [attr.y2]="view()[1] + 1e4" />
          } @else {
            <line class="guide" [attr.y1]="g.y" [attr.y2]="g.y" [attr.x1]="view()[0] - 1e4" [attr.x2]="view()[0] + 1e4" />
          }
        }
        @if (marquee(); as m) {
          <rect class="marquee" [attr.x]="m[0]" [attr.y]="m[1]" [attr.width]="m[2] - m[0]" [attr.height]="m[3] - m[1]" />
        }
      </svg>
    </div>
  `,
  styles: `
    :host { display: block; }
    .frame { position: relative; background: var(--paper); box-shadow: var(--page-shadow); }
    .art, .handles { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; }
    .art ::ng-deep > svg { display: block; width: 100%; height: 100%; overflow: visible; }
    .handles { touch-action: none; }
    .box { fill: transparent; stroke: #2a7ab8; stroke-width: 1; stroke-dasharray: 4 3; vector-effect: non-scaling-stroke; cursor: move; opacity: 0.55; }
    .box:hover { opacity: 1; }
    .box.selected { stroke-dasharray: none; opacity: 1; stroke-width: 1.5; }
    .box.copy { stroke-dasharray: 2 4; }
    .box.hidden-ink { stroke: #8a8378; }
    .box.locked { cursor: default; stroke: #8a8378; pointer-events: none; }
    .handle { fill: #ffffff; stroke: #2a7ab8; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
    .handle.turn { cursor: grab; }
    .handle.warp { fill: #2a7ab8; cursor: crosshair; }
    .stem { stroke: #2a7ab8; stroke-width: 1; vector-effect: non-scaling-stroke; }
    .guide { stroke: #d6336c; stroke-width: 1; vector-effect: non-scaling-stroke; pointer-events: none; }
    .marquee { fill: rgb(42 122 184 / 0.08); stroke: #2a7ab8; stroke-width: 1; stroke-dasharray: 3 2; vector-effect: non-scaling-stroke; }
  `,
})
export class CanvasOverlayComponent {
  protected readonly edit = inject(VisualEditService);
  readonly page = input.required<Page>();
  /** Pixels per canvas unit. */
  readonly scale = input.required<number>();
  private readonly art = viewChild.required<ElementRef<HTMLElement>>('art');

  /** What's held, if anything. */
  private readonly gesture = signal<Gesture | null>(null);
  /** The held gesture's map on the page. */
  private readonly live = signal<{ names: string[]; t: Affine } | null>(null);
  protected readonly guides = signal<Guide[]>([]);
  /** Counts the page's SVG put in, for the transforms to follow. */
  private readonly drawn = signal(0);

  protected readonly view = computed(() => this.page().view ?? [0, 0, 1, 1]);
  protected readonly viewBox = computed(() => this.view().join(' '));
  protected readonly size = computed(() => [this.view()[2] * this.scale(), this.view()[3] * this.scale()]);
  /** A screen pixel, in canvas units. */
  protected readonly unit = computed(() => 1 / this.scale());

  /** The page's box: its set size, else what's drawn on it. */
  private readonly pageBox = computed<Bounds>(() => {
    const page = this.page();
    if (page.size) return [0, 0, page.size[0], page.size[1]];
    const [x, y, w, h] = this.view();
    return [x, y, x + w, y + h];
  });

  /** The boxes drawn: each committed canvas's copies on the page, moved by what's held. */
  protected readonly items = computed(() => {
    const selection = new Set(this.edit.selection());
    const live = this.live();
    return this.edit.placed().map((p) => {
      const name = p.placement.name;
      const t = live && this.moves(name, live.names) ? live.t : null;
      const corners = t ? (p.corners.map((c) => applyTo(t, c)) as Corners) : p.corners;
      return { key: `${name}#${p.copy}`, name, corners, bounds: boundsOf(corners), edit: p.edit, copy: p.copy, selected: selection.has(name), placed: p };
    });
  });

  /** The selection's handles: round the one canvas's box, or round all of theirs. */
  protected readonly frame = computed(() => {
    const items = this.items().filter((i) => i.selected && i.copy === 0 && !i.edit.locked);
    if (!items.length) return null;
    let corners: Corners;
    if (items.length === 1) corners = items[0].corners;
    else {
      const [l, t, r, b] = unionBounds(items.map((i) => i.bounds));
      corners = [[l, t], [r, t], [r, b], [l, b]];
    }
    const mid = (a: Point, b: Point): Point => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    const centre = mid(corners[0], corners[2]);
    const topMid = mid(corners[0], corners[1]);
    const out = [topMid[0] - centre[0], topMid[1] - centre[1]];
    const length = Math.hypot(out[0], out[1]);
    const turn: Point = length > 1e-9
      ? [topMid[0] + (out[0] / length) * TURN_PX * this.unit(), topMid[1] + (out[1] / length) * TURN_PX * this.unit()]
      : [topMid[0], topMid[1] - TURN_PX * this.unit()];
    // An edge's handle only where the box has depth to stretch.
    const flatX = Math.hypot(corners[1][0] - corners[0][0], corners[1][1] - corners[0][1]) < 1e-6;
    const flatY = Math.hypot(corners[3][0] - corners[0][0], corners[3][1] - corners[0][1]) < 1e-6;
    const edges = [0, 1, 2, 3].map((i) => (i % 2 ? flatX : flatY) ? null : mid(corners[i], corners[(i + 1) % 4]));
    return { corners, centre, topMid, turn, edges, names: items.map((i) => i.name) };
  });

  /** The warp tool's handles: each selected canvas's corners, pinned. */
  protected readonly warpHandles = computed(() => {
    if (this.edit.tool() !== 'warp') return [];
    const live = this.live();
    return this.items().filter((i) => i.selected && i.copy === 0 && !i.edit.locked).flatMap((item) => {
      const corners = warpCorners(item.edit, item.placed.placement);
      return corners.map((at, i) => ({ name: item.name, i, at: live?.names.includes(item.name) ? applyTo(live.t, at) : at }));
    });
  });

  protected readonly marquee = computed(() => {
    const g = this.gesture();
    return g?.kind === 'marquee' ? boundsOf([g.from, g.to]) : null;
  });

  constructor() {
    // The page's SVG, inline.
    effect(() => {
      const svg = this.page().svg;
      const art = this.art().nativeElement;
      art.replaceChildren(...(svg ? [inlineSvg(svg)] : []));
      this.edit.ancestors.set(ancestorsIn(art));
      this.drawn.update((n) => n + 1);
    });
    // Its committed canvases' strokes moved by gestures waiting on their runs, and the one held.
    effect(() => {
      this.drawn();
      const layers = this.edit.pending(this.page().svgEdits);
      const live = this.live();
      for (const group of this.art().nativeElement.querySelectorAll<SVGGElement>('g[data-commit]')) {
        const names = (group.getAttribute('data-commit') ?? '').split(' ');
        let t: Affine | null = null;
        for (const layer of layers) if (layer.names.some((n) => names.includes(n))) t = t ? then(t, layer.t) : layer.t;
        if (live && live.names.some((n) => names.includes(n))) t = t ? then(t, live.t) : live.t;
        if (t && !isIdentity(t)) group.setAttribute('transform', svgMatrix(t));
        else group.removeAttribute('transform');
      }
    });
  }

  /** Whether moving the canvases `names` moves canvas `name`: it's one of them, or inside one. */
  private moves(name: string, names: string[]): boolean {
    const outer = this.edit.ancestors().get(name);
    return names.some((n) => n === name || outer?.has(n));
  }

  protected points(corners: Corners): string {
    return corners.map((c) => `${c[0]},${c[1]}`).join(' ');
  }

  /** The canvas point under the pointer. */
  private at(event: PointerEvent): Point {
    const rect = (event.currentTarget as SVGSVGElement).getBoundingClientRect();
    const [x, y] = this.view();
    return [(event.clientX - rect.left) / this.scale() + x, (event.clientY - rect.top) / this.scale() + y];
  }

  protected down(event: PointerEvent) {
    if (event.button !== 0 || !this.edit.readable()) return;
    const svg = event.currentTarget as SVGSVGElement;
    const from = this.at(event);
    const handle = parseHandle((event.target as Element).closest('[data-handle]')?.getAttribute('data-handle') ?? null);
    const frame = this.frame();
    let gesture: Gesture | null = null;
    if (handle?.kind === 'warp') {
      const item = this.items().find((i) => i.name === handle.name && i.copy === 0);
      if (item) gesture = { kind: 'warp', from, name: item.name, i: handle.i, edit: item.edit, placement: item.placed.placement, key: `warp-${++gestures}` };
    } else if (handle && frame) {
      gesture = handle.kind === 'turn'
        ? { kind: 'turn', from, names: frame.names, centre: frame.centre }
        : { kind: handle.kind, i: handle.i, from, names: frame.names, frame: frame.corners };
    } else {
      const hit = this.hit(from);
      if (!hit) {
        gesture = { kind: 'marquee', from, to: from, add: event.shiftKey };
      } else if (event.shiftKey) {
        this.edit.select(hit.name, true);
      } else {
        if (!hit.selected) this.edit.select(hit.name);
        const names = this.edit.selection().filter((n) => !this.edit.edit(n).locked);
        const moving = this.items().filter((i) => this.moves(i.name, names));
        const others = this.items().filter((i) => !moving.includes(i) && !this.moves(i.name, moving.map((m) => m.name)) && !i.edit.hidden).map((i) => i.bounds);
        const firsts = moving.filter((m) => m.copy === 0);
        if (moving.length) gesture = { kind: 'move', from, names, bounds: unionBounds((firsts.length ? firsts : moving).map((m) => m.bounds)), others };
      }
    }
    if (!gesture) return;
    event.preventDefault();
    svg.setPointerCapture(event.pointerId);
    this.gesture.set(gesture);
  }

  protected move(event: PointerEvent) {
    const g = this.gesture();
    if (!g) return;
    const p = this.at(event);
    switch (g.kind) {
      case 'marquee':
        this.gesture.set({ ...g, to: p });
        return;
      case 'warp': {
        const next = editForWarp(g.edit, g.placement, g.i, [p[0] - g.from[0], p[1] - g.from[1]]);
        this.edit.updateOne(g.name, () => next, g.key);
        return;
      }
      case 'move': {
        let by: Point = [p[0] - g.from[0], p[1] - g.from[1]];
        if (event.shiftKey) by = Math.abs(by[0]) >= Math.abs(by[1]) ? [by[0], 0] : [0, by[1]];
        let guides: Guide[] = [];
        if (!event.altKey) {
          const moved: Bounds = [g.bounds[0] + by[0], g.bounds[1] + by[1], g.bounds[2] + by[0], g.bounds[3] + by[1]];
          const lines = snapLines([this.pageBox(), ...g.others]);
          const snap = snapBox(moved, lines.xs, lines.ys, SNAP_PX * this.unit());
          by = [by[0] + snap.by[0], by[1] + snap.by[1]];
          guides = snap.guides;
        }
        this.guides.set(guides);
        this.live.set({ names: g.names, t: translate(by) });
        return;
      }
      case 'turn': {
        const a0 = Math.atan2(g.from[1] - g.centre[1], g.from[0] - g.centre[0]);
        const a1 = Math.atan2(p[1] - g.centre[1], p[0] - g.centre[0]);
        let degrees = ((a1 - a0) * 180) / Math.PI;
        if (event.shiftKey) degrees = Math.round(degrees / 15) * 15;
        this.live.set({ names: g.names, t: about(rotate(degrees), g.centre) });
        return;
      }
      case 'corner':
      case 'edge': {
        const t = frameGesture(g, p, event);
        if (t) this.live.set({ names: g.names, t });
        return;
      }
    }
  }

  protected up() {
    const g = this.gesture();
    if (!g) return;
    const live = this.live();
    if (g.kind === 'marquee') {
      const box = boundsOf([g.from, g.to]);
      const tiny = (box[2] - box[0]) * this.scale() < 3 && (box[3] - box[1]) * this.scale() < 3;
      const hits = tiny ? [] : this.items().filter((i) => !i.edit.locked && overlaps(i.bounds, box)).map((i) => i.name);
      const names = [...new Set(hits)];
      if (g.add) this.edit.selection.update((s) => [...new Set([...s, ...names])]);
      else this.edit.selection.set(names);
    } else if (g.kind === 'warp') {
      this.edit.settle();
    } else if (live && !isIdentity(live.t, 1e-6)) {
      this.edit.transform(live.names, () => live.t, true);
    }
    this.end();
  }

  protected cancel() {
    if (this.gesture()?.kind === 'warp') this.edit.settle();
    this.end();
  }

  private end() {
    this.gesture.set(null);
    this.live.set(null);
    this.guides.set([]);
  }

  /** The canvas under `p` that can be picked: the smallest box holding it. */
  private hit(p: Point) {
    const hits = this.items().filter((i) => !i.edit.locked && inside(i.corners, p));
    return hits.sort((a, b) => area(a.corners) - area(b.corners))[0] ?? null;
  }
}

function parseHandle(name: string | null): Handle | null {
  if (!name) return null;
  if (name === 'turn') return { kind: 'turn' };
  const [kind, i, ...rest] = name.split('-');
  if (kind === 'warp') return { kind: 'warp', i: +i, name: rest.join('-') };
  if (kind === 'corner' || kind === 'edge') return { kind, i: +i };
  return null;
}

function applyTo(t: Affine, [x, y]: Point): Point {
  return [t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5]];
}

/**
 * A corner (scale, uniformly unless Shift) or edge (stretch, or skew with
 * Ctrl/⌘) drag of the box `frame`, about the opposite corner or edge (its
 * centre with Alt), as a map on the page.
 */
function frameGesture(g: Extract<Gesture, { kind: 'corner' | 'edge' }>, p: Point, event: PointerEvent): Affine | null {
  const [tl, tr, , bl] = g.frame;
  // The box's frame: the unit square to the page.
  const f: Affine = [tr[0] - tl[0], tr[1] - tl[1], bl[0] - tl[0], bl[1] - tl[1], tl[0], tl[1]];
  const back = inverse(f);
  const unitCorners: Point[] = [[0, 0], [1, 0], [1, 1], [0, 1]];
  if (g.kind === 'corner') {
    const own = unitCorners[g.i];
    const anchor: Point = event.altKey ? [0.5, 0.5] : unitCorners[(g.i + 2) % 4];
    const pageAnchor = applyTo(f, anchor);
    const c0 = g.frame[g.i];
    if (!event.shiftKey || !back) {
      // Uniform: as far along the diagonal as the pointer is.
      const d = [c0[0] - pageAnchor[0], c0[1] - pageAnchor[1]];
      const len2 = d[0] * d[0] + d[1] * d[1];
      if (len2 < 1e-12) return null;
      const k = ((p[0] - pageAnchor[0]) * d[0] + (p[1] - pageAnchor[1]) * d[1]) / len2;
      return about(scale(k, k), pageAnchor);
    }
    const q = applyTo(back, p);
    const kx = own[0] !== anchor[0] ? (q[0] - anchor[0]) / (own[0] - anchor[0]) : 1;
    const ky = own[1] !== anchor[1] ? (q[1] - anchor[1]) / (own[1] - anchor[1]) : 1;
    return chain(back, about(scale(kx, ky), anchor), f);
  }
  if (!back) return null;
  const q = applyTo(back, p);
  const q0 = applyTo(back, g.from);
  // Edges: 0 top, 1 right, 2 bottom, 3 left.
  const vertical = g.i % 2 === 0;
  const anchor = event.altKey ? 0.5 : g.i === 0 || g.i === 3 ? 1 : 0;
  let s: Affine;
  if (event.ctrlKey || event.metaKey) {
    if (vertical) {
      const k = (q[0] - q0[0]) / ((g.i === 0 ? 0 : 1) - anchor || 1);
      s = [1, 0, k, 1, -k * anchor, 0];
    } else {
      const k = (q[1] - q0[1]) / ((g.i === 3 ? 0 : 1) - anchor || 1);
      s = [1, k, 0, 1, 0, -k * anchor];
    }
  } else if (vertical) {
    const from = (g.i === 0 ? 0 : 1) - anchor;
    s = about(scale(1, from ? (q[1] - anchor) / from : 1), [0, anchor]);
  } else {
    const from = (g.i === 3 ? 0 : 1) - anchor;
    s = about(scale(from ? (q[0] - anchor) / from : 1, 1), [anchor, 0]);
  }
  return chain(back, s, f);
}

function inside(corners: Corners, p: Point): boolean {
  // Flat boxes are picked within a few units of their line.
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = corners[i];
    const b = corners[(i + 1) % 4];
    const cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    if (Math.abs(cross) < 1e-9) continue;
    if (sign === 0) sign = Math.sign(cross);
    else if (Math.sign(cross) !== sign) return false;
  }
  if (area(corners) < 1e-6) {
    const [l, t, r, b] = boundsOf(corners);
    const pad = 4;
    return p[0] >= l - pad && p[0] <= r + pad && p[1] >= t - pad && p[1] <= b + pad;
  }
  return true;
}

function area(c: Corners): number {
  let sum = 0;
  for (let i = 0; i < 4; i++) sum += c[i][0] * c[(i + 1) % 4][1] - c[(i + 1) % 4][0] * c[i][1];
  return Math.abs(sum) / 2;
}

function overlaps(a: Bounds, b: Bounds): boolean {
  return a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3];
}

/** The page's SVG `text` as an element to put in the page: nothing that runs, and its layers not clipped (moved ink may leave them). */
function inlineSvg(text: string): Element {
  const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
  const root = doc.documentElement;
  if (root.nodeName !== 'svg' || doc.querySelector('parsererror')) return document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const bad of root.querySelectorAll('script, foreignObject')) bad.remove();
  for (const el of [root, ...root.querySelectorAll('*')]) {
    for (const attr of [...el.attributes]) if (/^on/i.test(attr.name)) el.removeAttribute(attr.name);
  }
  for (const svg of [root, ...root.querySelectorAll('svg')]) svg.setAttribute('overflow', 'visible');
  root.removeAttribute('width');
  root.removeAttribute('height');
  return document.importNode(root, true);
}

/** For each committed canvas in `art`'s SVG, the ones it's inside (its groups name them after it). */
function ancestorsIn(art: HTMLElement): Map<string, Set<string>> {
  const map = new Map<string, Set<string>>();
  for (const group of art.querySelectorAll('g[data-commit]')) {
    const names = (group.getAttribute('data-commit') ?? '').split(' ');
    names.forEach((name, i) => {
      const set = map.get(name) ?? new Set<string>();
      for (const outer of names.slice(i + 1)) set.add(outer);
      map.set(name, set);
    });
  }
  return map;
}
