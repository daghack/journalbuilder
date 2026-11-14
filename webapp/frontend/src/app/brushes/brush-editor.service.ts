import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { splitPath } from '../journal/journal-files';
import type { BrushFile, BrushInfo } from '../scribe';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ToolsService, failed } from '../services/tools.service';
import { ShellService } from '../shell/shell.service';
import { NODE_WIDTH, NodeTypes, addNode, layout, type Point } from './brush-model';

/** How long a run of changes of one thing (a slider's drag) counts as one, to undo. */
const MERGE_MS = 700;

/** What's kept of a brush file while other files show. */
interface Kept {
  positions: Record<string, Point>;
  undo: BrushFile[];
  redo: BrushFile[];
}

/**
 * The brush file open in the brush editor: read as JSON, changed a step
 * at a time (each undoable), and written back as Darkly writes brushes,
 * giving the journal a copy of a library brush once it's changed. Where
 * its nodes sit is the editor's: brush files don't keep it.
 */
@Injectable()
export class BrushEditorService {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly tools = inject(ToolsService);
  private readonly shell = inject(ShellService);

  /** The file edited, which stays while another kind's shows. */
  readonly path = this.shell.lastOpened(['brushes']);
  readonly fileName = computed(() => splitPath(this.path() ?? '')?.name ?? '');
  readonly own = computed(() => (this.path() ?? '') in this.store.files());
  readonly builtin = computed(() => this.library.has(this.path() ?? ''));
  readonly showing = computed(() => this.shell.selected() === this.path());
  readonly text = computed(() => {
    const path = this.path();
    return path ? (this.store.files()[path] ?? this.library.files()[path] ?? '') : '';
  });

  /** The node types brushes are made of; null till they're loaded. */
  readonly types = signal<NodeTypes | null>(null);
  /** The brush, as JSON; null till it's read, or if it can't be. */
  readonly brush = signal<BrushFile | null>(null);
  /** Why the file isn't a brush, if it isn't. */
  readonly readError = signal<string | null>(null);
  /** Why the brush can't be drawn with, if it can't (it's still kept). */
  readonly problem = signal<string | null>(null);
  /** Where each node sits. */
  readonly positions = signal<Record<string, Point>>({});
  readonly selected = signal<string | null>(null);
  readonly selectedWire = signal<string | null>(null);
  /** Every brush file and its name, for telling when two share one. */
  readonly infos = signal<BrushInfo[]>([]);
  /** Why the types didn't load, if they didn't. */
  readonly loadError = signal<string | null>(null);

  /** What styles call the brush: its name, or its file's. */
  readonly brushName = computed(() => this.brush()?.name?.trim() || this.fileName());
  /** Another brush file whose brush is called the same, if one is. */
  readonly clash = computed(() => {
    const name = this.brushName();
    const others = this.infos().filter((info) => info.name === name && info.file !== this.fileName());
    // A library brush the journal's stands in front of by its name is meant to be.
    const own = others.find((info) => info.own);
    if (own) return { file: own.file, own: true };
    const library = others.find((info) => !info.own);
    return library ? { file: library.file, own: false } : null;
  });

  private readonly kept = new Map<string, Kept>();
  private readonly undoStack = signal<BrushFile[]>([]);
  private readonly redoStack = signal<BrushFile[]>([]);
  readonly canUndo = computed(() => this.undoStack().length > 0);
  readonly canRedo = computed(() => this.redoStack().length > 0);
  /** The last change's key and when it was, to merge a run of them. */
  private last: { key: string; at: number } | null = null;
  /** The text last written to each file, which needn't be read back. */
  private readonly written = new Map<string, string>();
  private writes = 0;
  private reads = 0;

  constructor() {
    this.tools.brushNodeTypes().then(
      (types) => this.types.set(new NodeTypes(types)),
      (err: Error) => this.loadError.set(err.message),
    );
    // The brushes' names, each time it shows: the journal's may have changed.
    effect(() => {
      if (!this.showing()) return;
      this.store.files();
      untracked(() => this.tools.brushTypes().then((infos) => this.infos.set(infos), () => {}));
    });
    // Read the file when it's opened, or changed other than here.
    effect(() => {
      const path = this.path();
      const text = this.text();
      const types = this.types();
      if (!path || !types) return;
      untracked(() => this.read(path, text, types));
    });
  }

  private async read(path: string, text: string, types: NodeTypes) {
    if (this.written.get(path) === text && this.brush() && this.current === path) return;
    const read = ++this.reads;
    const answer = await this.tools.readBrush(text, this.fileName()).catch((err: Error) => ({ error: err.message }));
    if (read !== this.reads || this.path() !== path) return;
    this.switchTo(path);
    if (failed(answer)) {
      this.brush.set(null);
      this.readError.set(answer.error);
      this.problem.set(null);
      return;
    }
    this.readError.set(null);
    this.problem.set(answer.problem);
    this.brush.set(answer.brush);
    this.place(answer.brush, types);
  }

  /** The file whose state the signals hold. */
  private current: string | null = null;

  /** Keep the state of the file shown till now, and take up `path`'s. */
  private switchTo(path: string) {
    if (this.current === path) return;
    if (this.current) this.kept.set(this.current, { positions: this.positions(), undo: this.undoStack(), redo: this.redoStack() });
    const kept = this.kept.get(path);
    this.positions.set(kept?.positions ?? {});
    this.undoStack.set(kept?.undo ?? []);
    this.redoStack.set(kept?.redo ?? []);
    this.selected.set(null);
    this.selectedWire.set(null);
    this.last = null;
    this.current = path;
  }

  /** Lay out the nodes that have no place yet: all of them, if none has. */
  private place(brush: BrushFile, types: NodeTypes) {
    const positions = this.positions();
    const ids = Object.keys(brush.nodes);
    const placed = ids.filter((id) => positions[id]);
    if (placed.length === ids.length && Object.keys(positions).length === ids.length) return;
    if (!placed.length) {
      this.positions.set(layout(brush, types));
      return;
    }
    const fresh = layout(brush, types);
    this.positions.set(Object.fromEntries(ids.map((id) => [id, positions[id] ?? fresh[id]])));
  }

  /** Lay every node out afresh. */
  tidy() {
    const brush = this.brush();
    const types = this.types();
    if (brush && types) this.positions.set(layout(brush, types));
  }

  /** Where the middle of the graph's view is, in its world: the graph tells it. */
  viewCentre: () => Point = () => ({ x: 0, y: 0 });

  /** Add a node of `type`, at `at` or the middle of the view, and pick it. */
  add(type: string, at?: Point) {
    const brush = this.brush();
    const types = this.types();
    if (!brush || !types) return;
    const { brush: next, id } = addNode(brush, types, type);
    const middle = this.viewCentre();
    this.move(id, at ?? { x: Math.round(middle.x - NODE_WIDTH / 2), y: Math.round(middle.y - 60) });
    this.change(next);
    this.selected.set(id);
  }

  move(id: string, at: Point) {
    this.positions.update((p) => ({ ...p, [id]: at }));
  }

  /**
   * The brush changed to `next`: one step to undo, or part of the last if
   * it's a change of the same `key` (a slider dragged) soon after.
   */
  change(next: BrushFile, key: string | null = null) {
    const brush = this.brush();
    if (!brush || next === brush) return;
    const now = Date.now();
    const merge = key !== null && this.last?.key === key && now - this.last.at < MERGE_MS;
    if (!merge) {
      this.undoStack.update((u) => [...u, brush].slice(-200));
      this.redoStack.set([]);
    }
    this.last = key === null ? null : { key, at: now };
    this.brush.set(next);
    const types = this.types();
    if (types) this.place(next, types);
    void this.write(next);
  }

  undo() {
    const brush = this.brush();
    const undo = this.undoStack();
    if (!brush || !undo.length) return;
    this.redoStack.update((r) => [...r, brush]);
    this.undoStack.set(undo.slice(0, -1));
    this.last = null;
    this.brush.set(undo[undo.length - 1]);
    void this.write(undo[undo.length - 1]);
  }

  redo() {
    const brush = this.brush();
    const redo = this.redoStack();
    if (!brush || !redo.length) return;
    this.undoStack.update((u) => [...u, brush]);
    this.redoStack.set(redo.slice(0, -1));
    this.last = null;
    this.brush.set(redo[redo.length - 1]);
    void this.write(redo[redo.length - 1]);
  }

  /** Write `brush` to the file, as Darkly writes brushes. */
  private async write(brush: BrushFile) {
    const path = this.path();
    if (!path) return;
    const write = ++this.writes;
    const answer = await this.tools.writeBrush(brush, this.fileName()).catch((err: Error) => ({ error: err.message }));
    if (write !== this.writes) return;
    if (failed(answer)) {
      this.problem.set(answer.error);
      return;
    }
    this.problem.set(answer.problem);
    if (answer.yaml === this.text()) return;
    this.written.set(path, answer.yaml);
    this.store.setFile(path, answer.yaml);
  }

  /** Replace the brush with the one in `yaml` (a file's text, Darkly's copied brush), as one step; or say why it can't be. */
  async paste(yaml: string): Promise<string | null> {
    const answer = await this.tools.readBrush(yaml, this.fileName()).catch((err: Error) => ({ error: err.message }));
    if (failed(answer)) return answer.error;
    // A graph copied alone keeps the brush's name and description.
    const brush = this.brush();
    const next: BrushFile = { ...answer.brush };
    for (const key of ['name', 'description', 'author'] as const) if (next[key] === undefined && brush?.[key]) next[key] = brush[key];
    // One that can't be written back (a node of no type there is) isn't taken.
    const written = await this.tools.writeBrush(next, this.fileName()).catch((err: Error) => ({ error: err.message }));
    if (failed(written)) return written.error;
    this.positions.set({});
    this.change(next);
    return null;
  }

  /** Copy the library's file into the journal, to change. */
  copyIn() {
    const path = this.path();
    if (path && !this.own()) this.store.setFile(path, this.text());
  }

  /** Remove the journal's copy: the library's is used again. */
  useLibrary() {
    const path = this.path();
    if (path && this.own() && this.builtin()) this.store.removeFile(path);
  }
}
