import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import {
  about, align, boundsOf, distribute, editForGesture, editsReadable, parseEdits, placements, scale, serializeEdits, then, translate, unionBounds,
  type Affine, type Alignment, type Bounds, type CanvasEdit, type Placement,
} from '../journal/canvas-edits';
import type { Corners, SymbolicInfo } from '../scribe';
import { editsPath } from '../journal/journal-files';
import { JournalStore } from './journal-store.service';
import { JournalService } from './journal.service';

/** Undo steps kept. */
const HISTORY = 200;

export type EditTool = 'select' | 'warp';

/**
 * A gesture's move on the page, drawn on the SVG (and the boxes) until the
 * run of the edits it made shows: the canvases `names` (and what's inside
 * them) mapped by `t`. `edits` is the edits file's text it left.
 */
export interface Layer {
  names: string[];
  t: Affine;
  edits: string | null;
}

/** A committed canvas drawn on the page shown: where, and what it is. */
export interface Placed {
  placement: Placement;
  /** Its box's corners as drawn now (a gesture waiting on its run moves them). */
  corners: Corners;
  bounds: Bounds;
  edit: CanvasEdit;
  /** Which copy of it this is on the page. */
  copy: number;
}

/**
 * The visual editor: the canvases the entry commits to it, the selection
 * and tool, and its edits (`edits/ENTRY.json`) with undo and redo.
 */
@Injectable({ providedIn: 'root' })
export class VisualEditService {
  private readonly journal = inject(JournalService);
  private readonly store = inject(JournalStore);

  readonly committed = this.journal.committed;
  /** The entry's edits; none if its file isn't one this editor reads. */
  readonly file = computed(() => parseEdits(this.journal.edits()));
  /** Whether its edits file is one this editor can change. */
  readonly readable = computed(() => editsReadable(this.journal.edits()));
  readonly selection = signal<string[]>([]);
  readonly tool = signal<EditTool>('select');
  /** Gestures drawn ahead of their runs, oldest first. */
  readonly layers = signal<Layer[]>([]);
  /** For each committed canvas, the ones it's inside (from the page's SVG). */
  readonly ancestors = signal<Map<string, Set<string>>>(new Map());
  readonly canUndo = signal(false);
  readonly canRedo = signal(false);

  private undoStack: (string | null)[] = [];
  private redoStack: (string | null)[] = [];
  private coalescing: string | null = null;
  /** The edits undo or redo last put back, until something else is changed. */
  private restored: string | null | undefined = undefined;

  /** The page shown's index. */
  private readonly pageIndex = computed(() => this.journal.selected()?.index ?? -1);

  /** The committed canvases drawn on the page shown, each copy. */
  readonly placed = computed<Placed[]>(() => {
    const page = this.pageIndex();
    const canvases = this.file().canvases;
    const layers = this.pending(this.journal.shownEdits());
    return this.committed().flatMap((c) => {
      const edit = canvases[c.name] ?? {};
      return placements(c, edit, page).map((placement, copy) => {
        const t = this.layerMap(c.name, layers);
        const corners = placement.corners.map((p) => applyMap(t, p)) as Corners;
        return { placement, corners, bounds: boundsOf(corners), edit, copy };
      });
    });
  });

  constructor() {
    // A new entry, or journal: a new history.
    effect(() => {
      this.journal.entry();
      this.store.loads();
      untracked(() => this.forget());
    });
    // Canvases no longer committed leave the selection.
    effect(() => {
      const names = new Set(this.committed().map((c) => c.name));
      const selection = untracked(() => this.selection());
      if (selection.some((n) => !names.has(n)) && untracked(() => !this.journal.evaluating())) {
        this.selection.set(selection.filter((n) => names.has(n)));
      }
    });
    // Gestures are done with once the pages and boxes shown both have their edits.
    effect(() => {
      const layers = this.layers();
      const last = layers[layers.length - 1];
      if (!last) return;
      const page = this.journal.selected();
      if (this.journal.shownEdits() === last.edits && (!page?.view || page.svgEdits === last.edits)) this.layers.set([]);
    });
    // Gestures are drawn on the page they were made on.
    effect(() => {
      this.pageIndex();
      untracked(() => this.layers.set([]));
    });
    // A prune is a step to undo, unless it prunes what undo or redo just put
    // back (the script still doesn't commit it): that step's spent.
    this.journal.onPruned = (entry, before) => {
      if (entry !== this.journal.entry()) return;
      if (before === this.restored) {
        this.restored = undefined;
        return;
      }
      this.push(before);
      this.coalescing = null;
    };
  }

  /** The gestures drawn on what was made with `edits`: those after the last that made them. */
  pending(edits: string | null): Layer[] {
    const layers = this.layers();
    let i = layers.length - 1;
    while (i >= 0 && layers[i].edits !== edits) i--;
    return layers.slice(i + 1);
  }

  /** How `layers` move canvas `name` (or what it's in) on the page. */
  layerMap(name: string, layers: Layer[]): Affine | null {
    const outer = this.ancestors().get(name);
    let t: Affine | null = null;
    for (const layer of layers) {
      if (layer.names.some((n) => n === name || outer?.has(n))) t = t ? then(t, layer.t) : layer.t;
    }
    return t;
  }

  edit(name: string): CanvasEdit {
    return this.file().canvases[name] ?? {};
  }

  // ---- selection ----

  select(name: string, add = false) {
    if (add) this.selection.update((s) => (s.includes(name) ? s.filter((n) => n !== name) : [...s, name]));
    else this.selection.set([name]);
  }

  /**
   * The canvases selected that aren't inside another selected one (which
   * moves them already).
   */
  outermost(names = this.selection()): string[] {
    const ancestors = this.ancestors();
    return names.filter((n) => !names.some((m) => m !== n && ancestors.get(n)?.has(m)));
  }

  /** The first copy on the page shown of each canvas `names`. */
  placedOf(names: string[]): Placed[] {
    const placed = this.placed();
    return names.map((n) => placed.find((p) => p.placement.name === n)).filter((p): p is Placed => !!p);
  }

  // ---- changes ----

  /**
   * Change the edits with `change` (given a copy of them, by name), as one
   * undo step, or part of the last under the same `coalesce` key. Returns
   * the edits file's text after.
   */
  update(change: (canvases: Record<string, CanvasEdit>) => void, coalesce: string | null = null): string | null {
    const entry = this.journal.entry();
    const before = this.journal.edits();
    if (entry === null || !this.readable()) return before;
    const file = parseEdits(before);
    const canvases = structuredClone(file.canvases);
    change(canvases);
    const after = serializeEdits({ ...file, canvases });
    if (after === before) return before;
    if (coalesce === null || coalesce !== this.coalescing) this.push(before);
    this.coalescing = coalesce;
    this.restored = undefined;
    this.redoStack = [];
    this.write(after);
    this.history();
    return after;
  }

  /** Change canvas `name`'s edit. */
  updateOne(name: string, change: (edit: CanvasEdit) => CanvasEdit, coalesce: string | null = null) {
    this.layers.set([]);
    this.update((canvases) => {
      canvases[name] = change(canvases[name] ?? {});
    }, coalesce);
  }

  /** End a run of coalesced changes: the next is a step of its own. */
  settle() {
    this.coalescing = null;
  }

  /**
   * Move the canvases `names` on the page shown as `t` maps each copy
   * given (its first, else), as a gesture drawn ahead of its run if
   * `live`. Canvases inside others moved are left to them.
   */
  transform(names: string[], t: (placed: Placed) => Affine, live = false) {
    const moving = this.outermost(names);
    const placed = this.placedOf(moving);
    if (!placed.length) return;
    const maps = new Map(placed.map((p) => [p.placement.name, t(p)]));
    if (!live) this.layers.set([]);
    const edits = this.update((canvases) => {
      for (const p of placed) {
        const name = p.placement.name;
        canvases[name] = editForGesture(canvases[name] ?? {}, p.placement, maps.get(name)!);
      }
    });
    if (live && placed.length) {
      // One layer per map; a gesture moves them all alike.
      const t0 = maps.get(placed[0].placement.name)!;
      this.layers.update((layers) => [...layers, { names: placed.map((p) => p.placement.name), t: t0, edits }]);
    }
  }

  /** Move the canvases `names` by `by` on the page. */
  nudge(names: string[], by: [number, number]) {
    if (by[0] === 0 && by[1] === 0) return;
    this.transform(names, () => translate(by), true);
  }

  flip(names: string[], axis: 'x' | 'y') {
    this.transform(names, (p) => about(axis === 'x' ? scale(-1, 1) : scale(1, -1), centreOf(p.bounds)));
  }

  /** Line the canvases `names` up (`how`), with the page's box if there's one, else with each other. */
  align(names: string[], how: Alignment, page: Bounds | null) {
    const placed = this.placedOf(this.outermost(names));
    if (!placed.length) return;
    const to = placed.length === 1 && page ? page : unionBounds(placed.map((p) => p.bounds));
    const moves = new Map(placed.map((p, i) => [p.placement.name, align(placed.map((q) => q.bounds), how, to)[i]]));
    this.transform(names, (p) => translate(moves.get(p.placement.name)!));
  }

  distribute(names: string[], axis: 'x' | 'y') {
    const placed = this.placedOf(this.outermost(names));
    const moves = distribute(placed.map((p) => p.bounds), axis);
    const by = new Map(placed.map((p, i) => [p.placement.name, moves[i]]));
    this.transform(names, (p) => translate(by.get(p.placement.name)!));
  }

  /** Scale the canvas `name` to fill `page` as far as it can, centred on it; or just centre it, if not `fit`. */
  centre(name: string, page: Bounds, fit: boolean) {
    this.transform([name], (p) => {
      const [w, h] = [p.bounds[2] - p.bounds[0], p.bounds[3] - p.bounds[1]];
      const k = fit && w > 0 && h > 0 ? Math.min((page[2] - page[0]) / w, (page[3] - page[1]) / h) : 1;
      const from = centreOf(p.bounds);
      const to = centreOf(page);
      return then(about(scale(k, k), from), translate([to[0] - from[0], to[1] - from[1]]));
    });
  }

  /** Undo canvas `name`'s placing, warp and ink (not its being hidden or locked). */
  reset(name: string) {
    this.updateOne(name, (edit) => ({ hidden: edit.hidden, locked: edit.locked }));
  }

  /**
   * Draw the symbolic `name` as `symbols` of symbol set `set`, as an
   * undoable edit; what the script made it as is no edit at all.
   */
  setSymbolic(name: string, symbolic: SymbolicInfo, set: string, symbols: number[]) {
    const sameSet = set === symbolic.script_symbol_set;
    const sameSymbols = symbols.length === symbolic.script_symbols.length && symbols.every((s, i) => s === symbolic.script_symbols[i]);
    this.updateOne(name, (edit) => ({ ...edit, symbol_set: sameSet ? undefined : set, symbols: sameSymbols ? undefined : symbols }));
  }

  // ---- history ----

  undo() {
    if (!this.undoStack.length) return;
    this.redoStack.push(this.journal.edits());
    this.restore(this.undoStack.pop()!);
  }

  redo() {
    if (!this.redoStack.length) return;
    this.undoStack.push(this.journal.edits());
    this.restore(this.redoStack.pop()!);
  }

  private restore(text: string | null) {
    this.coalescing = null;
    this.restored = text;
    this.layers.set([]);
    this.write(text);
    this.history();
  }

  private push(before: string | null) {
    this.undoStack.push(before);
    if (this.undoStack.length > HISTORY) this.undoStack.shift();
    this.redoStack = [];
    this.history();
  }

  private write(text: string | null) {
    const entry = this.journal.entry();
    if (entry === null) return;
    if (text === null) this.store.removeFile(editsPath(entry));
    else this.store.setFile(editsPath(entry), text);
  }

  private history() {
    this.canUndo.set(this.undoStack.length > 0);
    this.canRedo.set(this.redoStack.length > 0);
  }

  private forget() {
    this.undoStack = [];
    this.redoStack = [];
    this.coalescing = null;
    this.layers.set([]);
    this.selection.set([]);
    this.history();
  }
}

function applyMap(t: Affine | null, p: [number, number]): [number, number] {
  if (!t) return p;
  const [a, b, c, d, e, f] = t;
  return [a * p[0] + c * p[1] + e, b * p[0] + d * p[1] + f];
}

export function centreOf(b: Bounds): [number, number] {
  return [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2];
}
