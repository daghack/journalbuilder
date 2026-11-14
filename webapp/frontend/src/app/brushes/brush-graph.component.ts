import { NgTemplateOutlet } from '@angular/common';
import { Component, ElementRef, computed, effect, inject, signal, viewChild } from '@angular/core';
import type { BrushPortDef, BrushValue, WireType } from '../scribe';
import { BrushEditorService } from './brush-editor.service';
import {
  NODE_WIDTH, ROW, connect, disconnect, fromDisplay, inputRange, inputValue, isExposed, nameNode, nodeRows, portOffset, removeNode,
  setInput, toDisplay, toggleExposed, truthy, unitSuffix, wireText, wiredInto, wires, type Point, type Wire,
} from './brush-model';
import { CurveEditorComponent, type CurvePoints } from './curve-editor.component';

/** What a wire's colour says it carries, as Darkly colours them. */
export const WIRE_COLORS: Record<WireType, string> = {
  Scalar: '#8fc1e3', Int: '#7fd1b9', Bool: '#e38f8f', Enum: '#c9a227', String: '#b9a3e3', Curve: '#e2b45a', Vec2: '#a3d977', Vec4: '#e3a3d0',
};

/** The type of node the palette drags, as drag data. */
export const NODE_DRAG = 'application/x-brush-node';

type Drag =
  | { kind: 'node'; id: string; from: Point; at: Point; moved: boolean }
  /** A wire being drawn from `from` (an output, or an input to wire back from). */
  | { kind: 'wire'; node: string; port: string; dir: 'Input' | 'Output'; type: WireType; to: Point }
  | { kind: 'pan'; from: Point; pan: Point };

/**
 * The middle of the brush editor: the brush's nodes, outputs on the right
 * and inputs on the left, each input set on the node unless something's
 * wired to it. Drag a node by its title; drag from a port's dot to another's
 * to wire them (from a wired input to take its wire off and move it); drag
 * the background to pan, and scroll to zoom. Delete takes the picked node or
 * wire away; double-click a title to name the node.
 */
@Component({
  selector: 'app-brush-graph',
  imports: [CurveEditorComponent, NgTemplateOutlet],
  host: { tabindex: '0', '(keydown)': 'onKey($event)' },
  template: `
    <div class="toolbar">
      <button type="button" [disabled]="!editor.canUndo()" (click)="editor.undo()" title="Undo (Ctrl/⌘ Z)">Undo</button>
      <button type="button" [disabled]="!editor.canRedo()" (click)="editor.redo()" title="Redo (Ctrl/⌘ Shift Z)">Redo</button>
      <button type="button" [disabled]="!editor.selected() && !editor.selectedWire()" (click)="deleteSelected()" title="Take the picked node or wire away (Delete)">Delete</button>
      <button type="button" (click)="editor.tidy(); fit()" title="Lay the nodes out afresh, from the pen on the left to the paint on the right">Tidy</button>
      <span class="status" [class.bad]="!!status().bad" [title]="status().text">{{ status().text }}</span>
      <span class="spacer"></span>
      <button type="button" (click)="zoomBy(1 / 1.2)" aria-label="Zoom out" title="Zoom out">−</button>
      <button type="button" (click)="fit()" title="Fit the brush">Fit</button>
      <button type="button" (click)="zoomBy(1.2)" aria-label="Zoom in" title="Zoom in">+</button>
    </div>
    <div #area class="area" (pointerdown)="downBackground($event)" (pointermove)="move($event)" (pointerup)="up($event)"
         (wheel)="wheel($event)" (dragover)="dragOver($event)" (drop)="drop($event)">
      <div class="world" [style.transform]="'translate(' + pan().x + 'px, ' + pan().y + 'px) scale(' + zoom() + ')'">
        <svg class="wires" width="1" height="1">
          @for (wire of wirePaths(); track wire.key) {
            <path class="hit" [attr.d]="wire.d" (pointerdown)="pickWire($event, wire.key)" />
            <path class="wire" [class.picked]="editor.selectedWire() === wire.key" [attr.d]="wire.d" [style.stroke]="wire.color" />
          }
          @if (pending(); as p) { <path class="wire pending" [attr.d]="p.d" [style.stroke]="p.color" /> }
        </svg>
        @for (node of nodes(); track node.id) {
          <div class="node" [class.picked]="editor.selected() === node.id" [class.terminal]="node.terminal"
               [style.left.px]="node.x" [style.top.px]="node.y" [style.width.px]="width" [style.height.px]="node.height">
            <div class="head" (pointerdown)="downNode($event, node.id)" (dblclick)="rename(node.id)" [title]="node.about">
              <span class="title">{{ node.title }}</span>
              @if (node.title !== node.typeName) { <span class="type">{{ node.typeName }}</span> }
              <span class="id">{{ node.id }}</span>
              <button type="button" class="x" (pointerdown)="$event.stopPropagation()" (click)="removeNode(node.id)" [attr.aria-label]="'Remove ' + node.id" title="Take the node away">×</button>
            </div>
            @if (node.comment) { <p class="comment">{{ node.comment }}</p> }
            @for (row of node.rows; track row.dir + row.def.name) {
              <div class="row" [class.out]="row.dir === 'Output'" [style.top.px]="row.y" [style.height.px]="row.height"
                   [attr.data-node]="node.id" [attr.data-port]="row.def.name" [attr.data-dir]="row.dir" [title]="row.def.description">
                @if (row.dir === 'Output') {
                  <span class="label">{{ row.label }}</span>
                  <span class="dot out" [style.background]="row.color" (pointerdown)="downPort($event, node.id, row.def, 'Output')"></span>
                } @else {
                  @if (row.def.wirable) {
                    <span class="dot in" [style.background]="row.wired ? row.color : 'transparent'" [style.border-color]="row.color"
                          (pointerdown)="downPort($event, node.id, row.def, 'Input')"></span>
                  }
                  <span class="label">{{ row.label }}</span>
                  @if (row.wired; as from) {
                    <span class="from">← {{ from }}</span>
                  } @else {
                    @switch (row.def.wire_type) {
                      @case ('Scalar') { <ng-container *ngTemplateOutlet="number; context: { $implicit: row, node: node.id }" /> }
                      @case ('Int') { <ng-container *ngTemplateOutlet="number; context: { $implicit: row, node: node.id }" /> }
                      @case ('Bool') {
                        <input type="checkbox" class="check" [checked]="row.on" (pointerdown)="$event.stopPropagation()" (change)="set(node.id, row.def, $any($event.target).checked)" />
                      }
                      @case ('Enum') {
                        <select class="choice" [value]="row.index" (pointerdown)="$event.stopPropagation()" (change)="set(node.id, row.def, +$any($event.target).value)">
                          @for (option of row.def.enum_options ?? []; track $index) { <option [value]="$index" [selected]="$index === row.index">{{ option }}</option> }
                        </select>
                      }
                      @case ('String') {
                        <input type="text" class="text" [value]="row.text" spellcheck="false" (pointerdown)="$event.stopPropagation()" (change)="set(node.id, row.def, $any($event.target).value)" />
                      }
                      @case ('Curve') {
                        <app-curve-editor class="curve" [points]="row.curve" (changed)="setCurve(node.id, row.def, $event)" />
                      }
                    }
                  }
                  @if (row.def.exposable) {
                    <button type="button" class="expose" [class.on]="row.exposed" (pointerdown)="$event.stopPropagation()" (click)="expose(node.id, row.def)"
                            [title]="row.exposed ? 'On the brush bar, when the brush is used in Darkly: click to take it off' : 'Put it on the brush bar, for when the brush is used in Darkly'">◉</button>
                  }
                }
              </div>
            }
          </div>
        }
      </div>
      @if (editor.brush() && !nodes().length) {
        <p class="empty">Drag node types here from the Nodes tab, or click one, then wire outputs to inputs.</p>
      }
      @if (wireError(); as error) { <p class="wire-error">{{ error }}</p> }
    </div>

    <ng-template #number let-row let-node="node">
      <input type="range" class="slider" [min]="row.range[0]" [max]="row.range[1]" [step]="row.step" [value]="row.number"
             (pointerdown)="$event.stopPropagation()" (input)="setNumber(node, row.def, +$any($event.target).value, false)" />
      <input type="number" class="num" [step]="row.step" [value]="row.shown" (pointerdown)="$event.stopPropagation()"
             (change)="setNumber(node, row.def, +$any($event.target).value, true)" />
      <span class="unit">{{ row.unit }}</span>
    </ng-template>
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-height: 0; min-width: 0; outline: none; background: var(--stage-bg); }
    .toolbar { display: flex; align-items: center; gap: 6px; padding: 6px 8px; background: var(--surface); border-bottom: 1px solid var(--border); font-size: 12px; }
    .spacer { flex: 1; }
    .status { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; margin-left: 8px; }
    .status.bad { color: var(--danger); }
    .area { position: relative; flex: 1; overflow: hidden; touch-action: none; cursor: grab; user-select: none; -webkit-user-select: none;
      background-image: radial-gradient(circle, rgb(255 255 255 / 0.18) 1px, transparent 1px); background-size: 24px 24px; }
    .world { position: absolute; left: 0; top: 0; transform-origin: 0 0; }
    .wires { position: absolute; left: 0; top: 0; overflow: visible; }
    .wire { fill: none; stroke-width: 2; pointer-events: none; }
    .wire.picked { stroke: #f2c94c !important; stroke-width: 3.5; }
    .wire.pending { stroke-dasharray: 6 4; }
    .hit { fill: none; stroke: transparent; stroke-width: 12; cursor: pointer; pointer-events: stroke; }
    .node { position: absolute; background: var(--surface); color: var(--fg); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 4px 14px rgb(0 0 0 / 0.25); font-size: 11px; }
    .node.terminal { border-color: #c9a227; }
    .node.picked { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent), 0 4px 14px rgb(0 0 0 / 0.25); }
    .head { display: flex; gap: 6px; align-items: center; height: 30px; padding: 0 6px 0 10px; border-bottom: 1px solid var(--border); cursor: move; background: var(--surface-2); border-radius: 8px 8px 0 0; }
    .node.terminal .head { background: color-mix(in srgb, #c9a227 16%, var(--surface-2)); }
    .head .title { font-weight: 700; font-size: 12px; white-space: nowrap; }
    .head .type, .head .id { color: var(--muted); font-family: var(--mono); font-size: 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .head .id { margin-left: auto; }
    .head .x { border: 0; background: transparent; color: var(--muted); cursor: pointer; padding: 0 4px; font-size: 14px; }
    .head .x:hover { color: var(--danger); }
    .comment { position: absolute; top: -18px; left: 4px; margin: 0; color: var(--stage-fg); font-style: italic; white-space: nowrap; }
    .row { position: absolute; left: 0; right: 0; display: flex; align-items: center; gap: 5px; padding: 0 8px 0 12px; box-sizing: border-box; flex-wrap: wrap; align-content: flex-start; }
    .row:not(.out) { padding-top: 3px; }
    .row.out { justify-content: flex-end; padding: 0 12px 0 8px; }
    .label { font-family: var(--mono); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 96px; line-height: 18px; }
    .from { color: var(--muted); font-family: var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; min-width: 0; text-align: right; }
    .dot { position: absolute; top: 6px; width: 9px; height: 9px; border-radius: 50%; border: 2px solid var(--surface); cursor: crosshair; }
    .dot.in { left: -7px; border-width: 2px; border-style: solid; box-sizing: content-box; }
    .dot.out { right: -7px; }
    .slider { flex: 1; min-width: 40px; height: 14px; margin: 0; }
    .num { width: 46px; font: 10px var(--mono); padding: 0 2px; height: 16px; }
    .unit { color: var(--muted); width: 12px; }
    .check { margin: 0 0 0 auto; }
    .choice, .text { flex: 1; min-width: 0; font-size: 10px; height: 18px; }
    .curve { flex-basis: 100%; margin-top: 2px; }
    .expose { border: 0; background: transparent; color: var(--border); cursor: pointer; padding: 0; font-size: 11px; line-height: 18px; margin-left: auto; }
    .slider + .num + .unit + .expose, .check + .expose, .choice + .expose, .text + .expose, .from + .expose { margin-left: 0; }
    .expose.on { color: #c9a227; }
    .empty { position: absolute; inset: 40% 0 auto; text-align: center; color: var(--stage-fg); }
    .wire-error { position: absolute; left: 10px; bottom: 10px; margin: 0; padding: 4px 8px; border-radius: 4px; background: var(--danger-bg); color: var(--danger); font-size: 12px; }
  `,
})
export class BrushGraphComponent {
  protected readonly editor = inject(BrushEditorService);
  private readonly area = viewChild.required<ElementRef<HTMLElement>>('area');
  protected readonly width = NODE_WIDTH;
  protected readonly pan = signal<Point>({ x: 40, y: 40 });
  protected readonly zoom = signal(0.9);
  protected readonly drag = signal<Drag | null>(null);
  protected readonly wireError = signal<string | null>(null);
  private errorTimer: ReturnType<typeof setTimeout> | null = null;
  /** The file last fitted to the view, so a newly opened one is. */
  private fitted: string | null = null;

  protected readonly status = computed(() => {
    const e = this.editor;
    if (e.loadError()) return { text: `The node types didn't load: ${e.loadError()}`, bad: true };
    if (e.readError()) return { text: `It isn't a Darkly brush: ${e.readError()}`, bad: true };
    if (e.problem()) return { text: `It can't be drawn with yet: ${e.problem()}`, bad: true };
    const brush = e.brush();
    if (!brush) return { text: 'Reading…' };
    return { text: `${Object.keys(brush.nodes).length} nodes, ${wires(brush).length} wires` };
  });

  protected readonly nodes = computed(() => {
    const brush = this.editor.brush();
    const types = this.editor.types();
    const positions = this.editor.positions();
    if (!brush || !types) return [];
    return Object.entries(brush.nodes).map(([id, node]) => {
      const type = types.get(node.type);
      const { rows, height } = nodeRows(brush, types, id);
      const at = positions[id] ?? { x: 0, y: 0 };
      return {
        id,
        x: at.x,
        y: at.y,
        height,
        title: node.name || type?.display_name || node.type,
        typeName: type?.display_name ?? `${node.type} (no such type)`,
        about: type?.description ?? '',
        comment: node.comment ?? '',
        terminal: !!type?.is_terminal,
        rows: rows.map((row) => {
          const def = row.def;
          const value = row.dir === 'Input' ? inputValue(brush, types, id, def.name) : undefined;
          const wire = row.dir === 'Input' ? wiredInto(brush, id, def.name) : undefined;
          const range = inputRange(brush, id, def).map((v) => toDisplay(def.unit_type, v)) as [number, number];
          const number = typeof value === 'number' ? toDisplay(def.unit_type, value) : 0;
          const step = def.wire_type === 'Int' ? 1 : def.step > 0 ? toDisplay(def.unit_type, def.step) : (range[1] - range[0]) / 200 || 0.01;
          return {
            ...row,
            label: def.label || def.name,
            color: WIRE_COLORS[def.wire_type] ?? '#888',
            wired: wire ? `${wire.from}.${wire.fromPort}` : null,
            exposed: row.dir === 'Input' && isExposed(brush, id, def.name),
            range,
            step,
            number,
            shown: def.wire_type === 'Int' ? Math.round(number) : Math.round(number * 1000) / 1000,
            unit: unitSuffix(def.unit_type),
            on: truthy(value),
            index: Number(value ?? 0),
            text: typeof value === 'string' ? value : '',
            curve: (Array.isArray(value) ? value : [[0, 0], [1, 1]]) as CurvePoints,
          };
        }),
      };
    });
  });

  /** Where a port's dot is, in the world. */
  private dot(id: string, port: string, dir: 'Input' | 'Output'): Point | null {
    const brush = this.editor.brush();
    const types = this.editor.types();
    const at = this.editor.positions()[id];
    if (!brush || !types || !at) return null;
    const offset = portOffset(brush, types, id, port, dir);
    return offset ? { x: at.x + offset.x, y: at.y + offset.y } : null;
  }

  protected readonly wirePaths = computed(() => {
    const brush = this.editor.brush();
    const types = this.editor.types();
    if (!brush || !types) return [];
    return wires(brush).flatMap((wire) => {
      const from = this.dot(wire.from, wire.fromPort, 'Output');
      const to = this.dot(wire.to, wire.toPort, 'Input');
      if (!from || !to) return [];
      const type = types.port(brush.nodes[wire.from].type, wire.fromPort, 'Output')?.wire_type ?? 'Scalar';
      return [{ key: wireText(wire), d: curve(from, to), color: WIRE_COLORS[type] }];
    });
  });

  protected readonly pending = computed(() => {
    const drag = this.drag();
    if (drag?.kind !== 'wire') return null;
    const start = this.dot(drag.node, drag.port, drag.dir);
    if (!start) return null;
    return { d: drag.dir === 'Output' ? curve(start, drag.to) : curve(drag.to, start), color: WIRE_COLORS[drag.type] };
  });

  constructor() {
    this.editor.viewCentre = () => {
      const rect = this.area().nativeElement.getBoundingClientRect();
      return this.world({ clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 });
    };
    // A newly opened brush is fitted to the view once it shows and its nodes have places.
    effect(() => {
      const path = this.editor.path();
      const placed = this.nodes().length > 0;
      if (!path || !placed || !this.editor.showing() || path === this.fitted) return;
      setTimeout(() => {
        if (!this.area().nativeElement.clientWidth) return;
        this.fitted = path;
        this.fit();
      });
    });
  }

  /** A point of the pointer's, in the world. */
  private world(event: { clientX: number; clientY: number }): Point {
    const rect = this.area().nativeElement.getBoundingClientRect();
    const pan = this.pan();
    return { x: (event.clientX - rect.left - pan.x) / this.zoom(), y: (event.clientY - rect.top - pan.y) / this.zoom() };
  }

  protected downBackground(event: PointerEvent) {
    if (event.button !== 0) return;
    this.editor.selected.set(null);
    this.editor.selectedWire.set(null);
    this.area().nativeElement.setPointerCapture(event.pointerId);
    this.drag.set({ kind: 'pan', from: { x: event.clientX, y: event.clientY }, pan: this.pan() });
  }

  protected downNode(event: PointerEvent, id: string) {
    if (event.button !== 0) return;
    event.stopPropagation();
    this.editor.selected.set(id);
    this.editor.selectedWire.set(null);
    this.area().nativeElement.setPointerCapture(event.pointerId);
    this.drag.set({ kind: 'node', id, from: this.world(event), at: this.editor.positions()[id] ?? { x: 0, y: 0 }, moved: false });
  }

  protected downPort(event: PointerEvent, id: string, def: BrushPortDef, dir: 'Input' | 'Output') {
    if (event.button !== 0) return;
    event.stopPropagation();
    const brush = this.editor.brush();
    if (!brush) return;
    this.area().nativeElement.setPointerCapture(event.pointerId);
    // A wired input gives up its wire, to be dropped somewhere else.
    const wired = dir === 'Input' ? wiredInto(brush, id, def.name) : undefined;
    if (wired) {
      this.editor.change(disconnect(brush, id, def.name));
      this.drag.set({ kind: 'wire', node: wired.from, port: wired.fromPort, dir: 'Output', type: def.wire_type, to: this.world(event) });
      return;
    }
    this.drag.set({ kind: 'wire', node: id, port: def.name, dir, type: def.wire_type, to: this.world(event) });
  }

  protected pickWire(event: PointerEvent, key: string) {
    event.stopPropagation();
    this.editor.selectedWire.set(key);
    this.editor.selected.set(null);
  }

  protected move(event: PointerEvent) {
    const drag = this.drag();
    if (!drag) return;
    if (drag.kind === 'pan') {
      this.pan.set({ x: drag.pan.x + event.clientX - drag.from.x, y: drag.pan.y + event.clientY - drag.from.y });
    } else if (drag.kind === 'node') {
      const at = this.world(event);
      this.editor.move(drag.id, { x: Math.round(drag.at.x + at.x - drag.from.x), y: Math.round(drag.at.y + at.y - drag.from.y) });
    } else {
      this.drag.set({ ...drag, to: this.world(event) });
    }
  }

  protected up(event: PointerEvent) {
    const drag = this.drag();
    this.drag.set(null);
    const area = this.area().nativeElement;
    if (area.hasPointerCapture(event.pointerId)) area.releasePointerCapture(event.pointerId);
    if (drag?.kind !== 'wire') return;
    const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>('[data-port]');
    if (!target) return;
    const node = target.dataset['node']!;
    const port = target.dataset['port']!;
    const dir = target.dataset['dir'] as 'Input' | 'Output';
    if (dir === drag.dir) return this.say(dir === 'Input' ? 'Wire an output to an input.' : 'Wire an input to an output.');
    const wire: Wire = drag.dir === 'Output'
      ? { from: drag.node, fromPort: drag.port, to: node, toPort: port }
      : { from: node, fromPort: port, to: drag.node, toPort: drag.port };
    const brush = this.editor.brush();
    const types = this.editor.types();
    if (!brush || !types) return;
    const next = connect(brush, types, wire);
    if (typeof next === 'string') return this.say(`Can't wire that: ${next}.`);
    this.editor.change(next);
  }

  private say(error: string) {
    this.wireError.set(error);
    if (this.errorTimer) clearTimeout(this.errorTimer);
    this.errorTimer = setTimeout(() => this.wireError.set(null), 3500);
  }

  protected wheel(event: WheelEvent) {
    event.preventDefault();
    this.zoomBy(Math.exp(-event.deltaY * 0.0015), event);
  }

  /** Zoom by `factor` about the pointer, or the middle of the view. */
  protected zoomBy(factor: number, about?: { clientX: number; clientY: number }) {
    const rect = this.area().nativeElement.getBoundingClientRect();
    const at = about ?? { clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 };
    const zoom = Math.min(2.5, Math.max(0.2, this.zoom() * factor));
    const world = this.world(at);
    this.zoom.set(zoom);
    this.pan.set({ x: at.clientX - rect.left - world.x * zoom, y: at.clientY - rect.top - world.y * zoom });
  }

  /** Fit every node in the view. */
  protected fit() {
    const nodes = this.nodes();
    const rect = this.area().nativeElement.getBoundingClientRect();
    if (!nodes.length || !rect.width) return;
    const x0 = Math.min(...nodes.map((n) => n.x)) - 30;
    const y0 = Math.min(...nodes.map((n) => n.y)) - 30;
    const x1 = Math.max(...nodes.map((n) => n.x + NODE_WIDTH)) + 30;
    const y1 = Math.max(...nodes.map((n) => n.y + n.height)) + 30;
    const zoom = Math.min(1.2, Math.max(0.2, Math.min(rect.width / (x1 - x0), rect.height / (y1 - y0))));
    this.zoom.set(zoom);
    this.pan.set({ x: (rect.width - (x1 - x0) * zoom) / 2 - x0 * zoom, y: (rect.height - (y1 - y0) * zoom) / 2 - y0 * zoom });
  }

  protected dragOver(event: DragEvent) {
    if (event.dataTransfer?.types.includes(NODE_DRAG)) event.preventDefault();
  }

  protected drop(event: DragEvent) {
    const type = event.dataTransfer?.getData(NODE_DRAG);
    if (!type) return;
    event.preventDefault();
    const at = this.world(event);
    this.editor.add(type, { x: Math.round(at.x - NODE_WIDTH / 2), y: Math.round(at.y - ROW) });
  }

  protected set(id: string, def: BrushPortDef, value: BrushValue) {
    const brush = this.editor.brush();
    const types = this.editor.types();
    if (brush && types) this.editor.change(setInput(brush, types, id, def.name, value), `${id}.${def.name}`);
  }

  /** A number input set from what it shows (a percentage, degrees...): kept as the port takes it. */
  protected setNumber(id: string, def: BrushPortDef, shown: number, typed: boolean) {
    if (!Number.isFinite(shown)) return;
    let value = fromDisplay(def.unit_type, shown);
    if (def.wire_type === 'Int') value = Math.round(value);
    else value = Math.round(value * 1e6) / 1e6;
    if (typed) value = Math.min(def.max, Math.max(def.min, value));
    this.set(id, def, value);
  }

  protected setCurve(id: string, def: BrushPortDef, points: CurvePoints) {
    this.set(id, def, points);
  }

  protected expose(id: string, def: BrushPortDef) {
    const brush = this.editor.brush();
    if (brush) this.editor.change(toggleExposed(brush, id, def.name));
  }

  protected rename(id: string) {
    const brush = this.editor.brush();
    if (!brush) return;
    const name = prompt(`A name for ${id}, shown in place of its type's (empty for none):`, brush.nodes[id]?.name ?? '');
    if (name !== null) this.editor.change(nameNode(brush, id, name));
  }

  protected removeNode(id: string) {
    const brush = this.editor.brush();
    if (!brush) return;
    this.editor.change(removeNode(brush, id));
    if (this.editor.selected() === id) this.editor.selected.set(null);
  }

  protected deleteSelected() {
    const brush = this.editor.brush();
    if (!brush) return;
    const node = this.editor.selected();
    const wire = this.editor.selectedWire();
    if (node) this.removeNode(node);
    else if (wire) {
      this.editor.change({ ...brush, connections: (brush.connections ?? []).filter((w) => w !== wire) });
      this.editor.selectedWire.set(null);
    }
  }

  protected onKey(event: KeyboardEvent) {
    if ((event.target as HTMLElement | null)?.closest('input, textarea, select')) return;
    if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault();
      this.deleteSelected();
    } else if (event.key === 'Escape') {
      this.drag.set(null);
    }
  }
}

/** A wire's curve from an output at `a` to an input at `b`. */
function curve(a: Point, b: Point): string {
  const reach = Math.max(40, Math.abs(b.x - a.x) * 0.5);
  return `M ${a.x} ${a.y} C ${a.x + reach} ${a.y}, ${b.x - reach} ${b.y}, ${b.x} ${b.y}`;
}
