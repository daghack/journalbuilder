import { Component, ElementRef, computed, effect, inject, signal, untracked, viewChild } from '@angular/core';
import { failed } from '../services/tools.service';
import { compatible, portType, portsOf, splitPort } from './chain-model';
import { LockEditorService } from './lock-editor.service';

/** A lock's size in the graph: its width, its title's height, and a port row's. */
const WIDTH = 220;
const HEAD = 30;
const ROW = 22;

interface Point {
  x: number;
  y: number;
}

type Drag =
  | { kind: 'node'; id: string; from: Point; at: Point }
  | { kind: 'wire'; port: string; side: 'out' | 'in'; to: Point }
  | { kind: 'pan'; from: Point; pan: Point };

/**
 * The middle: the chain's locks as boxes with their inputs on the left and
 * outputs on the right, wires between them. Drag a lock by its title; drag
 * from a port to another to wire them; drag the background to pan.
 */
@Component({
  selector: 'app-lock-graph',
  host: { tabindex: '0', '(keydown)': 'onKey($event)' },
  template: `
    <div class="toolbar">
      <button type="button" [disabled]="!editor.canUndo()" (click)="editor.undo()" title="Undo (Ctrl/⌘ Z)">Undo</button>
      <button type="button" [disabled]="!editor.canRedo()" (click)="editor.redo()" title="Redo (Ctrl/⌘ Shift Z)">Redo</button>
      <button type="button" [disabled]="!editor.selected() && !editor.selectedWire()" (click)="deleteSelected()" title="Delete the picked lock or wire (Delete)">Delete</button>
      <span class="sep"></span>
      <span class="status" [class.bad]="status().bad">{{ status().text }}</span>
      <span class="spacer"></span>
      <button type="button" (click)="zoomBy(1 / 1.2)" aria-label="Zoom out" title="Zoom out">−</button>
      <button type="button" (click)="fit()" title="Fit the chain">Fit</button>
      <button type="button" (click)="zoomBy(1.2)" aria-label="Zoom in" title="Zoom in">+</button>
    </div>
    <div #area class="area" (pointerdown)="downBackground($event)" (pointermove)="move($event)" (pointerup)="up($event)"
         (wheel)="wheel($event)" (dragover)="$event.preventDefault()" (drop)="drop($event)">
      <div class="world" [style.transform]="'translate(' + pan().x + 'px, ' + pan().y + 'px) scale(' + zoom() + ')'">
        <svg class="wires" width="1" height="1">
          @for (wire of wires(); track wire.to) {
            <path class="hit" [attr.d]="wire.d" (pointerdown)="pickWire($event, wire.to)" />
            <path class="wire" [class.picked]="editor.selectedWire() === wire.to" [attr.d]="wire.d" />
            @if (wire.label) {
              <text class="value" [attr.x]="wire.mid.x" [attr.y]="wire.mid.y - 5">{{ wire.label }}</text>
            }
          }
          @if (pending(); as d) { <path class="wire pending" [attr.d]="d" /> }
        </svg>
        @for (node of nodes(); track node.id) {
          <div class="node" [class.picked]="editor.selected() === node.id" [class.broken]="node.error"
               [style.left.px]="node.x" [style.top.px]="node.y" [style.width.px]="width">
            <div class="head" (pointerdown)="downNode($event, node.id)">
              <span class="id">{{ node.id }}</span>
              <span class="type">{{ node.type }}</span>
            </div>
            @if (node.error) {
              <p class="error">{{ node.error }}</p>
            }
            @for (row of node.rows; track $index) {
              <div class="row">
                @if (row.input; as p) {
                  <span class="port in" [attr.data-port]="p.port" data-side="in" [class.dim]="dim(p.port, 'in', p.type)"
                        (pointerdown)="downPort($event, p.port, 'in')" [title]="p.port + ': ' + p.type">
                    <span class="dot" [class.open]="p.open" [class.fixed]="p.fixed"></span>{{ p.name }}
                  </span>
                } @else { <span></span> }
                @if (row.output; as p) {
                  <span class="port out" [attr.data-port]="p.port" data-side="out" [class.dim]="dim(p.port, 'out', p.type)"
                        (pointerdown)="downPort($event, p.port, 'out')" [title]="p.port + ': ' + p.type">
                    {{ p.name }}<span class="dot" [class.open]="p.open"></span>
                  </span>
                } @else { <span></span> }
              </div>
            }
          </div>
          @for (tag of node.tags; track tag.port) {
            <span class="tag" [class.out]="tag.side === 'out'" [class.fixed]="tag.fixed"
                  [style.left.px]="tag.side === 'in' ? node.x - 8 : node.x + width + 8" [style.top.px]="tag.y">{{ tag.text }}</span>
          }
        }
      </div>
      @if (!editor.model().nodes.length) {
        <p class="empty">Drag lock types here from the left, or click one, then wire outputs to inputs.</p>
      }
      @if (wireError(); as error) { <p class="wire-error">{{ error }}</p> }
    </div>
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-height: 0; min-width: 0; outline: none; background: var(--stage-bg); }
    .spacer { flex: 1; }
    .status { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
    .status.bad { color: var(--danger); }
    .area { position: relative; flex: 1; overflow: hidden; touch-action: none; cursor: grab;
      background-image: radial-gradient(circle, rgb(255 255 255 / 0.18) 1px, transparent 1px); background-size: 24px 24px; }
    .world { position: absolute; left: 0; top: 0; transform-origin: 0 0; }
    .wires { position: absolute; left: 0; top: 0; overflow: visible; }
    .wire { fill: none; stroke: #f2efe9; stroke-width: 2; pointer-events: none; }
    .wire.picked { stroke: #f2c94c; stroke-width: 3; }
    .wire.pending { stroke-dasharray: 6 4; }
    .hit { fill: none; stroke: transparent; stroke-width: 12; cursor: pointer; pointer-events: stroke; }
    .value { fill: #f2efe9; font: 11px var(--mono); text-anchor: middle; paint-order: stroke; stroke: #3d3935; stroke-width: 3px; }
    .node { position: absolute; background: var(--surface); color: var(--fg); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 4px 14px rgb(0 0 0 / 0.25); font-size: 12px; }
    .node.picked { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent), 0 4px 14px rgb(0 0 0 / 0.25); }
    .node.broken { border-color: var(--danger); }
    .head { display: flex; gap: 6px; align-items: baseline; height: 30px; padding: 0 10px; border-bottom: 1px solid var(--border); cursor: move; background: var(--surface-2); border-radius: 8px 8px 0 0; }
    .head .id { font-weight: 700; line-height: 30px; }
    .head .type { color: var(--muted); font-family: var(--mono); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .error { margin: 0; padding: 4px 10px; color: var(--danger); font-size: 11px; background: var(--danger-bg); }
    .row { display: grid; grid-template-columns: 1fr 1fr; height: 22px; align-items: center; }
    .port { display: flex; align-items: center; gap: 6px; font: 11px var(--mono); cursor: crosshair; white-space: nowrap; overflow: visible; }
    .port.out { justify-content: flex-end; }
    .port.dim { opacity: 0.3; }
    .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--muted); border: 2px solid var(--surface); flex: none; }
    .port.in .dot { margin-left: -6px; }
    .port.out .dot { margin-right: -6px; }
    .dot.open { background: var(--accent); }
    .dot.fixed { background: #c9a227; }
    .tag { position: absolute; transform: translate(-100%, -50%); font: 11px var(--mono); color: #f2efe9; white-space: nowrap; pointer-events: none; }
    .tag.out { transform: translate(0, -50%); }
    .tag.fixed { color: #f2c94c; }
    .empty { position: absolute; inset: 0; display: grid; place-items: center; margin: 0; color: var(--stage-fg); font-size: 13px; pointer-events: none; }
    .wire-error { position: absolute; left: 12px; bottom: 12px; margin: 0; padding: 6px 8px; border-radius: 4px; font-size: 12px; color: var(--danger); background: var(--danger-bg); }
  `,
})
export class LockGraphComponent {
  protected readonly editor = inject(LockEditorService);
  private readonly area = viewChild.required<ElementRef<HTMLElement>>('area');
  protected readonly width = WIDTH;

  protected readonly pan = signal<Point>({ x: 20, y: 20 });
  protected readonly zoom = signal(1);
  protected readonly drag = signal<Drag | null>(null);
  protected readonly wireError = signal<string | null>(null);
  private errorTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    // A chain opened: show all of it, once its locks have their ports.
    effect(() => {
      if (this.editor.opened() === 0) return;
      untracked(() => setTimeout(() => this.fit(), 300));
    });
  }

  /** Each value along the chain from the last run, by port. */
  private readonly trace = computed(() => {
    const result = this.editor.result();
    return result && !failed(result) ? new Map(result.trace) : new Map();
  });

  protected readonly status = computed(() => {
    const blocked = this.editor.blocked();
    if (blocked) return { text: blocked, bad: false };
    if (this.editor.running()) return { text: 'Running…', bad: false };
    const result = this.editor.result();
    if (!result) return { text: '', bad: false };
    if (failed(result)) return { text: result.error, bad: true };
    if (result.unlockError) return { text: `Locks, but doesn't unlock: ${result.unlockError}`, bad: true };
    return result.roundTrip ? { text: 'Locks and unlocks back to the test inputs ✓', bad: false } : { text: "Unlocking doesn't give the test inputs back ✗", bad: true };
  });

  protected readonly nodes = computed(() => {
    const model = this.editor.model();
    const ports = this.editor.ports();
    const errors = this.editor.nodeErrors();
    const wiredIn = new Set(model.wires.map((w) => w.to));
    const wiredOut = new Set(model.wires.map((w) => w.from));
    const inputs = new Map(this.editor.inputs().map((p) => [p.port, p.name]));
    const outputs = new Map(this.editor.outputs().map((p) => [p.port, p.name]));
    return model.nodes.map((node) => {
      const p = portsOf(ports, node.id);
      const ins = (p?.inputs ?? []).map(([name, type]) => {
        const port = `${node.id}.${name}`;
        return { port, name, type, open: !wiredIn.has(port) && !(port in model.fixed), fixed: port in model.fixed };
      });
      const outs = (p?.outputs ?? []).map(([name, type]) => {
        const port = `${node.id}.${name}`;
        return { port, name, type, open: !wiredOut.has(port) };
      });
      const rows = Array.from({ length: Math.max(ins.length, outs.length) }, (_, i) => ({ input: ins[i] ?? null, output: outs[i] ?? null }));
      const top = node.y + HEAD + (errors[node.id] ? 20 : 0);
      const tags = [
        ...ins.map((input, i) => {
          const fixed = model.fixed[input.port];
          const text = fixed ? `= ${'text' in fixed ? JSON.stringify(fixed.text) : JSON.stringify(fixed.json)}` : inputs.get(input.port);
          return text ? { port: input.port, side: 'in' as const, y: top + i * ROW + ROW / 2, text: fixed ? text : `${text} →`, fixed: !!fixed } : null;
        }),
        ...outs.map((output, i) => {
          const name = outputs.get(output.port);
          return name ? { port: output.port, side: 'out' as const, y: top + i * ROW + ROW / 2, text: `→ ${name}`, fixed: false } : null;
        }),
      ].filter((t) => t !== null);
      return { ...node, rows, error: errors[node.id] ?? null, tags };
    });
  });

  /** Where port "lock.name" sits, on its side of its lock. */
  private portPoint(port: string, side: 'in' | 'out'): Point | null {
    const [id, name] = splitPort(port);
    const node = this.editor.model().nodes.find((n) => n.id === id);
    const p = portsOf(this.editor.ports(), id);
    if (!node || !p) return null;
    const list = side === 'in' ? p.inputs : p.outputs;
    const i = list.findIndex(([n]) => n === name);
    if (i < 0) return null;
    const top = node.y + HEAD + (this.editor.nodeErrors()[id] ? 20 : 0);
    return { x: side === 'in' ? node.x : node.x + WIDTH, y: top + i * ROW + ROW / 2 };
  }

  protected readonly wires = computed(() => {
    const trace = this.trace();
    return this.editor.model().wires.flatMap((wire) => {
      const a = this.portPoint(wire.from, 'out');
      const b = this.portPoint(wire.to, 'in');
      if (!a || !b) return [];
      const value = trace.get(wire.from);
      const text = value ? (value.text ?? JSON.stringify(value.json)) : '';
      return [{ to: wire.to, d: curve(a, b), mid: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }, label: text.length > 18 ? `${text.slice(0, 17)}…` : text }];
    });
  });

  protected readonly pending = computed(() => {
    const drag = this.drag();
    if (drag?.kind !== 'wire') return null;
    const at = this.portPoint(drag.port, drag.side);
    if (!at) return null;
    return drag.side === 'out' ? curve(at, drag.to) : curve(drag.to, at);
  });

  /** While wiring, ports that can't take the wire fade. */
  protected dim(port: string, side: 'in' | 'out', type: string): boolean {
    const drag = this.drag();
    if (drag?.kind !== 'wire' || drag.side === side) return false;
    const other = portType(this.editor.ports(), drag.port, drag.side === 'out' ? 'outputs' : 'inputs');
    if (!other) return true;
    const [a, b] = drag.side === 'out' ? [drag.port, port] : [port, drag.port];
    return !compatible(other, type) || splitPort(a)[0] === splitPort(b)[0];
  }

  /** A pointer's place in the graph. */
  private at(event: MouseEvent): Point {
    const rect = this.area().nativeElement.getBoundingClientRect();
    return { x: (event.clientX - rect.left - this.pan().x) / this.zoom(), y: (event.clientY - rect.top - this.pan().y) / this.zoom() };
  }

  protected downBackground(event: PointerEvent) {
    if (event.target !== this.area().nativeElement && !(event.target as HTMLElement).classList.contains('world')) return;
    this.editor.selected.set(null);
    this.editor.selectedWire.set(null);
    this.area().nativeElement.setPointerCapture(event.pointerId);
    this.drag.set({ kind: 'pan', from: { x: event.clientX, y: event.clientY }, pan: this.pan() });
  }

  protected downNode(event: PointerEvent, id: string) {
    event.stopPropagation();
    (this.area().nativeElement.parentElement as HTMLElement).focus({ preventScroll: true });
    this.editor.selected.set(id);
    this.editor.selectedWire.set(null);
    this.editor.settle();
    const node = this.editor.model().nodes.find((n) => n.id === id)!;
    this.area().nativeElement.setPointerCapture(event.pointerId);
    this.drag.set({ kind: 'node', id, from: this.at(event), at: { x: node.x, y: node.y } });
  }

  protected downPort(event: PointerEvent, port: string, side: 'in' | 'out') {
    event.stopPropagation();
    this.area().nativeElement.setPointerCapture(event.pointerId);
    this.drag.set({ kind: 'wire', port, side, to: this.at(event) });
  }

  protected pickWire(event: PointerEvent, to: string) {
    event.stopPropagation();
    (this.area().nativeElement.parentElement as HTMLElement).focus({ preventScroll: true });
    this.editor.selectedWire.set(to);
    this.editor.selected.set(null);
  }

  protected move(event: PointerEvent) {
    const drag = this.drag();
    if (!drag) return;
    if (drag.kind === 'pan') {
      this.pan.set({ x: drag.pan.x + event.clientX - drag.from.x, y: drag.pan.y + event.clientY - drag.from.y });
    } else if (drag.kind === 'node') {
      const p = this.at(event);
      this.editor.moveNode(drag.id, drag.at.x + p.x - drag.from.x, drag.at.y + p.y - drag.from.y);
    } else {
      this.drag.set({ ...drag, to: this.at(event) });
    }
  }

  protected up(event: PointerEvent) {
    const drag = this.drag();
    this.drag.set(null);
    if (drag?.kind !== 'wire') return;
    const target = document.elementFromPoint(event.clientX, event.clientY)?.closest('[data-port]') as HTMLElement | null;
    if (!target) return;
    const port = target.dataset['port']!;
    const side = target.dataset['side'] as 'in' | 'out';
    if (side === drag.side) return;
    const [from, to] = drag.side === 'out' ? [drag.port, port] : [port, drag.port];
    const problem = this.editor.connect(from, to);
    this.showError(problem);
  }

  private showError(problem: string | null) {
    this.wireError.set(problem ? `Can't wire that: ${problem}.` : null);
    if (this.errorTimer) clearTimeout(this.errorTimer);
    if (problem) this.errorTimer = setTimeout(() => this.wireError.set(null), 4000);
  }

  protected drop(event: DragEvent) {
    event.preventDefault();
    const type = event.dataTransfer?.getData('application/x-lock-type');
    if (!type) return;
    const p = this.at(event);
    this.editor.addNode(type, p.x - WIDTH / 2, p.y - HEAD / 2);
  }

  protected wheel(event: WheelEvent) {
    event.preventDefault();
    if (event.ctrlKey || event.metaKey) {
      const rect = this.area().nativeElement.getBoundingClientRect();
      const before = this.at(event);
      this.zoom.update((z) => Math.min(2.5, Math.max(0.25, z * Math.exp(-event.deltaY / 300))));
      this.pan.set({ x: event.clientX - rect.left - before.x * this.zoom(), y: event.clientY - rect.top - before.y * this.zoom() });
    } else {
      this.pan.update((p) => ({ x: p.x - event.deltaX, y: p.y - event.deltaY }));
    }
  }

  protected zoomBy(factor: number) {
    this.zoom.update((z) => Math.min(2.5, Math.max(0.25, z * factor)));
  }

  /** Show every lock. */
  fit() {
    const nodes = this.editor.model().nodes;
    const rect = this.area().nativeElement.getBoundingClientRect();
    if (!nodes.length || !rect.width) return this.pan.set({ x: 20, y: 20 });
    const minX = Math.min(...nodes.map((n) => n.x)) - 120, maxX = Math.max(...nodes.map((n) => n.x)) + WIDTH + 140;
    const minY = Math.min(...nodes.map((n) => n.y)) - 40, maxY = Math.max(...nodes.map((n) => n.y)) + 220;
    const zoom = Math.min(1.5, Math.max(0.25, Math.min(rect.width / (maxX - minX), rect.height / (maxY - minY))));
    this.zoom.set(zoom);
    this.pan.set({ x: -minX * zoom, y: -minY * zoom });
  }

  protected deleteSelected() {
    const wire = this.editor.selectedWire();
    const node = this.editor.selected();
    if (wire) this.editor.removeWire(wire);
    else if (node) this.editor.removeNode(node);
  }

  protected onKey(event: KeyboardEvent) {
    if ((event.target as HTMLElement).closest('input, textarea, select')) return;
    if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault();
      this.deleteSelected();
    } else if (event.key === 'Escape') {
      this.editor.selected.set(null);
      this.editor.selectedWire.set(null);
    }
  }
}

/** A wire from an output at `a` to an input at `b`. */
function curve(a: Point, b: Point): string {
  const dx = Math.max(40, Math.abs(b.x - a.x) / 2);
  return `M ${a.x} ${a.y} C ${a.x + dx} ${a.y} ${b.x - dx} ${b.y} ${b.x} ${b.y}`;
}
