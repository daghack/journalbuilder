import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { kindOf, nameOf, pathOf, VALID_NAME } from '../journal/journal-files';
import { template } from '../journal/templates';
import type { ChainFile, ChainRun, ChainType, IllustrationType, LayoutType, LockOption, LockType, RenderPreview } from '../scribe';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ToolsService, failed } from '../services/tools.service';
import { ShellService } from '../shell/shell.service';
import * as chain from './chain-model';
import type { ChainModel, ChainNode, PortsByNode, RendererNode, SourceKind, SourceNode, TestValue } from './chain-model';
import { chainSource, journalChain, journalSnippet } from './chain-source';

const HISTORY = 200;

/** A message for the panel: what happened, or went wrong. */
export interface Note {
  kind: 'ok' | 'error' | 'note';
  text: string;
}

/**
 * The chain being edited in the chain editor: the chains/NAME.rhai file the
 * sidebar opened, as a model; each lock's ports (which depend on its
 * options, so the engine works them out); and the test run, made again
 * whenever the chain or its test changes. Edits are written back to the
 * file once the ports are known: a library chain edited gives the journal a
 * copy that stands in for it.
 */
@Injectable()
export class LockEditorService {
  private readonly tools = inject(ToolsService);
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly shell = inject(ShellService);

  readonly types = signal<LockType[]>([]);
  readonly rendererTypes = signal<IllustrationType[]>([]);
  /** The text styles and layouts sources draw on, with their options' defaults. */
  readonly styleTypes = signal<LayoutType[]>([]);
  readonly layoutTypes = signal<LayoutType[]>([]);
  /** The chain files: the journal's over the library's. */
  readonly chainTypes = signal<ChainType[]>([]);
  /** What each illustration in the chain draws of the last run, by illustration. */
  readonly previews = signal<Record<string, RenderPreview | { error: string }>>({});
  /** Each value kind's example, rhai that makes one, by kind name. */
  readonly kinds = signal<Map<string, string | null>>(new Map());
  readonly typesError = signal<string | null>(null);
  readonly model = signal<ChainModel>(chain.emptyChain());
  readonly ports = signal<PortsByNode>({});
  /** The lock picked in the graph, and the wire (by the input it feeds). */
  readonly selected = signal<string | null>(null);
  readonly selectedWire = signal<string | null>(null);
  readonly result = signal<ChainRun | { error: string } | null>(null);
  readonly running = signal(false);
  readonly note = signal<Note | null>(null);
  /** Counts chains opened, for the graph to fit each in view. */
  readonly opened = signal(0);

  /** The chain file edited: the last chains/ file the sidebar opened. */
  readonly path = signal<string | null>(null);
  /** Its text: the journal's, or the library's until it's changed. */
  readonly text = computed(() => {
    const path = this.path();
    return path ? (this.store.files()[path] ?? this.library.files()[path] ?? null) : null;
  });
  /** Whether the journal has the file (else it's the library's). */
  readonly own = computed(() => (this.path() ?? '') in this.store.files());
  /** Whether the library has a chain of its name: the journal's stands in for it. */
  readonly builtin = computed(() => this.library.has(this.path() ?? ''));

  private undoStack: ChainModel[] = [];
  private redoStack: ChainModel[] = [];
  private coalescing: string | null = null;
  readonly canUndo = signal(false);
  readonly canRedo = signal(false);

  readonly typeByName = computed(() => new Map(this.types().map((t) => [t.name, t])));
  readonly rendererByName = computed(() => new Map(this.rendererTypes().map((t) => [t.name, t])));
  /** The renderer picked in the graph, if a renderer is what's picked. */
  readonly renderer = computed(() => this.model().renderers.find((r) => r.id === this.selected()) ?? null);
  readonly inputs = computed(() => chain.openInputs(this.model(), this.ports()));
  readonly outputs = computed(() => chain.openOutputs(this.model(), this.ports()));
  readonly node = computed(() => this.model().nodes.find((n) => n.id === this.selected()) ?? null);
  /** The source picked in the graph, if a source is what's picked. */
  readonly source = computed(() => this.model().sources.find((s) => s.id === this.selected()) ?? null);
  /** The chain's nodes whose files stand in for the library's, by id: the graph marks them. */
  readonly overrides = computed(() => {
    const model = this.model();
    const files = this.store.files();
    const library = this.library.files();
    const types = this.typeByName();
    const illustrations = this.rendererByName();
    const ids = new Set<string>();
    for (const node of model.nodes) if (types.get(node.type)?.own && types.get(node.type)?.builtin) ids.add(node.id);
    for (const r of model.renderers) if (illustrations.get(r.type)?.own && illustrations.get(r.type)?.builtin) ids.add(r.id);
    for (const source of model.sources) {
      const path = this.sourcePath(source);
      if (path && path in files && path in library) ids.add(source.id);
    }
    return ids;
  });
  /** The chain's options' values for a test run: their defaults. */
  readonly optionValues = computed(() => Object.fromEntries(this.model().options.filter((o) => o.default !== undefined).map((o) => [o.name, o.default])));
  /** Problems with a lock's ports, by lock. */
  readonly nodeErrors = computed(() => {
    const errors: Record<string, string> = {};
    const locks = new Set(this.model().nodes.map((n) => n.id));
    for (const [id, p] of Object.entries(this.ports())) if ('error' in p && locks.has(id)) errors[id] = p.error;
    return errors;
  });
  /** Problems with a renderer's file or options, by renderer. */
  readonly rendererErrors = computed(() => {
    const errors: Record<string, string> = {};
    for (const renderer of this.model().renderers) {
      const p = this.ports()[renderer.id];
      if (p && 'error' in p) errors[renderer.id] = p.error;
    }
    return errors;
  });

  /** The chain file to run (no example, nodes unplaced: moving a lock needn't run it again). */
  private readonly runSource = computed(() => {
    const model = this.model();
    // Renderers aren't part of the lock: changing one needn't run it again.
    const unplaced = { ...model, nodes: model.nodes.map((n) => ({ ...n, x: 0, y: 0 })), sources: model.sources.map((s) => ({ ...s, x: 0, y: 0 })), renderers: [], renderWires: [], test: {} };
    return chainSource(unplaced, { ports: this.ports(), optionKinds: (t) => this.optionKinds(t) });
  });

  /** Why the chain can't run yet, if it can't. */
  readonly blocked = computed<string | null>(() => {
    const model = this.model();
    if (!model.nodes.length) return 'Add a lock from the Library tab to start a chain.';
    const missingPorts = model.nodes.find((n) => !this.ports()[n.id]);
    if (missingPorts) return 'Working out the locks\' ports…';
    const errors = Object.keys(this.nodeErrors());
    if (errors.length) return `Fix ${errors.join(', ')} first.`;
    if (!this.outputs().length) return 'The chain has no outputs.';
    const missing = this.inputs().filter((p) => !model.test[p.port]);
    if (missing.length) return `Give ${missing.map((p) => p.port).join(', ')} a test value to run the chain.`;
    return null;
  });

  private portKeys = new Map<string, string>();
  private runTimer: ReturnType<typeof setTimeout> | null = null;
  private writeTimer: ReturnType<typeof setTimeout> | null = null;
  private previewTimer: ReturnType<typeof setTimeout> | null = null;
  /** The model as last read from the file, and the text it was read from or last written as. */
  private untouched: ChainModel | null = null;
  private known: string | null = null;
  private previewAsked = 0;
  private asked = 0;
  private reading = 0;

  constructor() {
    // The lock and illustration types, again whenever the journal's files change.
    effect(() => {
      this.store.resources();
      untracked(() => {
        void this.loadTypes();
        void this.loadChains();
      });
    });
    // The chain the sidebar opens.
    effect(() => {
      const path = this.shell.selected();
      if (!path || kindOf(path) !== 'chains') return;
      untracked(() => {
        if (path !== this.path()) this.path.set(path);
      });
    });
    // Its file, read when it's opened, and again when it changes other than by the editor.
    effect(() => {
      const path = this.path();
      const text = this.text();
      if (path === null || text === null) return;
      untracked(() => {
        if (text !== this.known) void this.read(path, text);
      });
    });
    // Each lock's and illustration's ports, when its type or options change (or its file does).
    effect(() => {
      const model = this.model();
      const files = this.store.resources();
      untracked(() => this.refreshPorts(model, files));
    });
    // The chain as it's edited goes into its file, so journals using it
    // (chain("name")) follow along.
    effect(() => {
      const model = this.model();
      const ports = this.ports();
      if (this.writeTimer) clearTimeout(this.writeTimer);
      this.writeTimer = setTimeout(() => untracked(() => this.write(model, ports)), 400);
    });
    // What each illustration draws of the last run.
    effect(() => {
      const result = this.result();
      const model = this.model();
      const ports = this.ports();
      if (this.previewTimer) clearTimeout(this.previewTimer);
      this.previewTimer = setTimeout(() => untracked(() => void this.refreshPreviews(result, model, ports)), 150);
    });
    // Run the test whenever the chain or its test case changes.
    effect(() => {
      const source = this.runSource();
      const blocked = this.blocked();
      const test = this.model().test;
      const options = this.optionValues();
      if (this.runTimer) clearTimeout(this.runTimer);
      if (blocked) {
        untracked(() => this.result.set(null));
        return;
      }
      this.runTimer = setTimeout(() => void this.runTest(source, options, test), 250);
    });
  }

  optionKinds(type: string): Record<string, LockOption['kind']> {
    return Object.fromEntries((this.typeByName().get(type)?.options ?? []).map((o) => [o.name, o.kind]));
  }

  private async loadTypes() {
    try {
      const types = await this.tools.lockTypes();
      if (failed(types)) throw new Error(types.error);
      this.types.set(types);
      this.rendererTypes.set(await this.tools.illustrationTypes());
      this.styleTypes.set(await this.tools.styleTypes());
      this.layoutTypes.set(await this.tools.layoutTypes());
      this.kinds.set(new Map((await this.tools.valueKinds()).map((k) => [k.name, k.example])));
      this.typesError.set(null);
    } catch (err) {
      this.typesError.set((err as Error).message);
    }
  }

  private async loadChains() {
    const chains = await this.tools.chainTypes().catch((err: Error) => ({ error: err.message }));
    if (!failed(chains)) this.chainTypes.set(chains);
  }

  private refreshPorts(model: ChainModel, files: Record<string, string>) {
    const ids = new Set([...model.nodes, ...model.sources, ...model.renderers].map((n) => n.id));
    let changed = false;
    const ports = { ...this.ports() };
    for (const id of Object.keys(ports)) {
      if (!ids.has(id)) {
        delete ports[id];
        this.portKeys.delete(id);
        changed = true;
      }
    }
    if (changed) this.ports.set(ports);
    // A lock's ports change only when its own file does: the chain going
    // into its file as it's edited mustn't ask for them all again.
    for (const source of model.sources) {
      const path = this.sourcePath(source);
      const key = JSON.stringify(['source', source.kind, source.name, source.base ?? null, path, path ? files[path] ?? null : null]);
      if (this.portKeys.get(source.id) === key) continue;
      this.portKeys.set(source.id, key);
      this.ports.update((p) => ({ ...p, [source.id]: this.sourcePorts(source) }));
    }
    for (const renderer of model.renderers) {
      const options = chain.resolvedOptions(model, renderer);
      const key = JSON.stringify(['illustration', renderer.type, options, files[pathOf('illustrations', renderer.type)] ?? null]);
      if (this.portKeys.get(renderer.id) === key) continue;
      this.portKeys.set(renderer.id, key);
      void this.tools.illustrationPorts(renderer.type, options).then(
        (answer) => {
          if (this.portKeys.get(renderer.id) !== key) return;
          const ports = failed(answer) ? answer : { inputs: answer.inputs, outputs: [], options };
          this.ports.update((p) => ({ ...p, [renderer.id]: ports }));
        },
        (err: Error) => {
          if (this.portKeys.get(renderer.id) === key) this.ports.update((p) => ({ ...p, [renderer.id]: { error: err.message } }));
        },
      );
    }
    for (const node of model.nodes) {
      const options = chain.resolvedOptions(model, node);
      const key = JSON.stringify([node.type, options, files[pathOf('locks', node.type)] ?? null]);
      if (this.portKeys.get(node.id) === key) continue;
      this.portKeys.set(node.id, key);
      void this.tools.lockPorts(node.type, options).then(
        (answer) => {
          if (this.portKeys.get(node.id) === key) this.ports.update((p) => ({ ...p, [node.id]: answer }));
        },
        (err: Error) => {
          if (this.portKeys.get(node.id) === key) this.ports.update((p) => ({ ...p, [node.id]: { error: err.message } }));
        },
      );
    }
  }

  private async runTest(source: string, options: Record<string, unknown>, test: Record<string, TestValue>) {
    const asked = ++this.asked;
    this.running.set(true);
    const inputs = Object.fromEntries(this.inputs().map((p) => [p.port, test[p.port]]));
    let result: ChainRun | { error: string };
    try {
      result = await this.tools.runChain(source, options, inputs);
    } catch (err) {
      result = { error: (err as Error).message };
    }
    if (asked !== this.asked) return;
    this.running.set(false);
    this.result.set(result);
  }

  // ---- changes ----

  update(change: (model: ChainModel) => ChainModel, coalesce: string | null = null) {
    const before = this.model();
    const after = change(before);
    if (after === before) return;
    if (coalesce === null || coalesce !== this.coalescing) {
      this.undoStack.push(before);
      if (this.undoStack.length > HISTORY) this.undoStack.shift();
    }
    this.coalescing = coalesce;
    this.redoStack = [];
    this.model.set(after);
    this.history();
  }

  settle() {
    this.coalescing = null;
  }

  undo() {
    const before = this.undoStack.pop();
    if (!before) return;
    this.redoStack.push(this.model());
    this.coalescing = null;
    this.model.set(before);
    this.history();
  }

  redo() {
    const after = this.redoStack.pop();
    if (!after) return;
    this.undoStack.push(this.model());
    this.coalescing = null;
    this.model.set(after);
    this.history();
  }

  private history() {
    this.canUndo.set(this.undoStack.length > 0);
    this.canRedo.set(this.redoStack.length > 0);
  }

  /** A lock of `type` added at (x, y), with its required options given a start. */
  addNode(type: string, x: number, y: number) {
    const spec = this.typeByName().get(type);
    const options: Record<string, unknown> = {};
    for (const option of spec?.options ?? []) {
      if (option.default !== undefined) continue;
      options[option.name] = startingValue(option);
    }
    const id = chain.freshId(this.model(), type);
    this.update((m) => ({ ...m, nodes: [...m.nodes, { id, type, options, promoted: {}, x: Math.round(x), y: Math.round(y) }] }));
    this.selected.set(id);
    this.selectedWire.set(null);
  }

  moveNode(id: string, x: number, y: number) {
    const place = <T extends { id: string; x: number; y: number }>(n: T): T => (n.id === id ? { ...n, x: Math.round(x), y: Math.round(y) } : n);
    this.update((m) => ({ ...m, nodes: m.nodes.map(place), sources: m.sources.map(place), renderers: m.renderers.map(place) }), `move:${id}`);
  }

  removeNode(id: string) {
    this.update((m) => chain.removeNode(m, id));
    if (this.selected() === id) this.selected.set(null);
  }

  renameNode(id: string, to: string): string | null {
    if (to === id) return null;
    if (!to || to.includes('.')) return "a lock's name must be non-empty, without '.'";
    if (this.model().nodes.some((n) => n.id === to) || this.model().renderers.some((r) => r.id === to)) return `there's already a lock or illustration named '${to}'`;
    this.update((m) => chain.renameNode(m, id, to));
    if (this.selected() === id) this.selected.set(to);
    return null;
  }

  setOption(id: string, option: string, value: unknown) {
    this.update((m) => ({
      ...m,
      nodes: m.nodes.map((n) => {
        if (n.id !== id) return n;
        const options = { ...n.options };
        if (value === undefined) delete options[option];
        else options[option] = value;
        return { ...n, options };
      }),
    }), `option:${id}:${option}`);
  }

  /** The kind, choices and default of lock or renderer `id`'s `option`, as a chain option takes them. */
  private optionSpec(id: string, option: string): { kind: LockOption['kind']; choices?: unknown[]; default?: unknown } | null {
    const model = this.model();
    const node = model.nodes.find((n) => n.id === id);
    if (node) return this.typeByName().get(node.type)?.options?.find((o) => o.name === option) ?? null;
    const renderer = model.renderers.find((r) => r.id === id);
    const defaults = renderer ? this.rendererByName().get(renderer.type)?.options : undefined;
    if (!defaults || !(option in defaults)) return null;
    const value = defaults[option];
    const kind = typeof value === 'number' ? (Number.isInteger(value) ? 'integer' : 'number')
      : typeof value === 'boolean' ? 'bool' : Array.isArray(value) ? 'array' : 'string';
    return { kind, default: value };
  }

  /** Let the chain set lock (or renderer) `id`'s `option`: it becomes one of the chain's own options. */
  promote(id: string, option: string) {
    const model = this.model();
    const owner = model.nodes.find((n) => n.id === id) ?? model.renderers.find((r) => r.id === id);
    const spec = this.optionSpec(id, option);
    if (!owner || !spec) return;
    let name = option;
    for (let n = 2; model.options.some((o) => o.name === name); n++) name = `${option}${n}`;
    const value = option in owner.options ? owner.options[option] : spec.default;
    const promote = <T extends ChainNode | RendererNode>(n: T): T => {
      if (n.id !== id) return n;
      const { [option]: _, ...options } = n.options;
      return { ...n, options, promoted: { ...n.promoted, [option]: name } };
    };
    this.update((m) => ({
      ...m,
      options: [...m.options, { name, kind: spec.kind, default: value, choices: spec.choices }],
      nodes: m.nodes.map(promote),
      renderers: m.renderers.map(promote),
    }));
  }

  /** Give lock (or renderer) `id`'s `option` a value of its own again. */
  demote(id: string, option: string) {
    this.update((m) => {
      const owner = m.nodes.find((n) => n.id === id) ?? m.renderers.find((r) => r.id === id);
      const chainOption = owner?.promoted[option];
      const value = m.options.find((o) => o.name === chainOption)?.default;
      const demote = <T extends ChainNode | RendererNode>(n: T): T => {
        if (n.id !== id) return n;
        const { [option]: _, ...promoted } = n.promoted;
        return { ...n, promoted, options: value === undefined ? n.options : { ...n.options, [option]: value } };
      };
      const nodes = m.nodes.map(demote);
      const renderers = m.renderers.map(demote);
      const used = new Set([...nodes, ...renderers].flatMap((n) => Object.values(n.promoted)));
      return { ...m, nodes, renderers, options: m.options.filter((o) => used.has(o.name)) };
    });
  }

  setChainOption(name: string, patch: Partial<chain.ChainOption>) {
    this.update((m) => {
      const options = m.options.map((o) => (o.name === name ? { ...o, ...patch } : o));
      if (patch.name === undefined || patch.name === name) return { ...m, options };
      const rename = <T extends ChainNode | RendererNode>(n: T): T => ({ ...n, promoted: Object.fromEntries(Object.entries(n.promoted).map(([k, v]) => [k, v === name ? patch.name! : v])) });
      return { ...m, options, nodes: m.nodes.map(rename), renderers: m.renderers.map(rename) };
    }, `chain-option:${name}`);
  }

  /** Wire `from` to `to` (a lock's input, or a renderer's), unless it can't be. Returns why not. */
  connect(from: string, to: string): string | null {
    const problem = chain.wireProblem(this.model(), this.ports(), from, to);
    if (problem) return problem;
    if (chain.isRenderer(this.model(), chain.splitPort(to)[0])) this.update((m) => chain.connectRenderer(m, from, to));
    else this.update((m) => chain.connect(m, from, to));
    return null;
  }

  removeWire(to: string) {
    this.update((m) => ({ ...m, wires: m.wires.filter((w) => w.to !== to), renderWires: m.renderWires.filter((w) => w.to !== to) }));
    if (this.selectedWire() === to) this.selectedWire.set(null);
  }

  // ---- renderers ----

  /** A renderer of `type` attached at (x, y). */
  addRenderer(type: string, x: number, y: number) {
    const id = chain.freshId(this.model(), type);
    this.update((m) => ({ ...m, renderers: [...m.renderers, { id, type, options: {}, promoted: {}, x: Math.round(x), y: Math.round(y) }] }));
    this.selected.set(id);
    this.selectedWire.set(null);
  }

  setRendererOption(id: string, option: string, value: unknown) {
    this.update((m) => ({
      ...m,
      renderers: m.renderers.map((r) => {
        if (r.id !== id) return r;
        const options = { ...r.options };
        if (value === undefined) delete options[option];
        else options[option] = value;
        return { ...r, options };
      }),
    }), `renderer-option:${id}:${option}`);
  }

  /**
   * The last run's values for renderer `renderer`'s inputs, or null if it
   * can't draw yet. A source's value is written as `{ $source }`, for the
   * engine to make (a style's bundle, say), so a chain of sources alone draws.
   */
  private rendererInputs(renderer: RendererNode, result: ChainRun | { error: string } | null, model: ChainModel, ports: PortsByNode): Record<string, unknown> | null {
    const p = chain.portsOf(ports, renderer.id);
    if (!p) return null;
    const trace = new Map(result && !failed(result) ? result.trace : []);
    const inputs: Record<string, unknown> = {};
    for (const [name] of p.inputs) {
      const wire = model.renderWires.find((w) => w.to === `${renderer.id}.${name}`);
      if (!wire) return null;
      const source = model.sources.find((s) => `${s.id}.${chain.SOURCE_PORT}` === wire.from);
      if (source) {
        inputs[name] = { $source: sourceSpec(source) };
        continue;
      }
      const value = trace.get(wire.from);
      if (!value) return null;
      inputs[name] = value.json;
    }
    return inputs;
  }

  // ---- sources ----

  /** The file a source draws on, if there is one: content's of whichever extension it has. */
  sourcePath(source: Pick<SourceNode, 'kind' | 'name'>): string | null {
    const files = this.store.files();
    const library = this.library.files();
    const candidates = source.kind === 'content' ? ['txt', 'md', 'json'].map((ext) => pathOf('content', source.name, ext))
      : [pathOf(chain.sourceDir(source.kind), source.name)];
    return candidates.find((path) => path in files || path in library) ?? null;
  }

  /** A source's port, as the engine types it: symbols or String for content, TextStyle, Glyphset or Layout. */
  private sourcePorts(source: SourceNode): { inputs: [string, string][]; outputs: [string, string][]; options: Record<string, unknown> } | { error: string } {
    const path = this.sourcePath(source);
    if (!path) return { error: `there's no ${source.kind} named '${source.name}'` };
    let type: string;
    if (source.kind === 'content') {
      if (path.endsWith('.json')) {
        if (source.base !== undefined) return { error: `${path} is symbols in its own base already: give text content a base` };
        try {
          const base = JSON.parse(this.store.files()[path] ?? this.library.files()[path]).base;
          type = `Base${base}Symbol`;
        } catch {
          return { error: `${path} isn't symbols` };
        }
      } else {
        type = source.base !== undefined ? `Base${source.base}Symbol` : 'String';
      }
    } else {
      type = source.kind === 'style' ? 'TextStyle' : source.kind === 'glyphset' ? 'Glyphset' : 'Layout';
    }
    return { inputs: [], outputs: [[chain.SOURCE_PORT, type]], options: {} };
  }

  /** A source of `kind` drawing on file `name`, added at (x, y). */
  addSource(kind: SourceKind, name: string, x: number, y: number) {
    const id = chain.freshId(this.model(), name);
    this.update((m) => ({ ...m, sources: [...m.sources, { id, kind, name, options: {}, x: Math.round(x), y: Math.round(y) }] }));
    this.selected.set(id);
    this.selectedWire.set(null);
  }

  /** Change source `id`: its file, options or base. */
  setSource(id: string, patch: Partial<Pick<SourceNode, 'name' | 'options' | 'base'>>) {
    this.update((m) => ({ ...m, sources: m.sources.map((s) => (s.id === id ? { ...s, ...patch } : s)) }), `source:${id}`);
  }

  private async refreshPreviews(result: ChainRun | { error: string } | null, model: ChainModel, ports: PortsByNode) {
    const asked = ++this.previewAsked;
    const previews: Record<string, RenderPreview | { error: string }> = {};
    for (const renderer of model.renderers) {
      const inputs = this.rendererInputs(renderer, result, model, ports);
      if (!inputs) continue;
      previews[renderer.id] = await this.tools.illustrationPreview(renderer.type, null, chain.resolvedOptions(model, renderer), inputs).catch((err: Error) => ({ error: err.message }));
    }
    if (asked === this.previewAsked) this.previews.set(previews);
  }

  /**
   * Write the chain into its file, as it is: a journal using
   * chain("name") runs it as it's edited, mistakes and all. A chain only
   * read isn't written, so a library chain isn't copied unless it's changed.
   */
  private write(model: ChainModel, ports: PortsByNode) {
    const path = this.path();
    // Its values are written by their ports' types, so wait for the ports.
    if (!path || model === this.untouched || !model.nodes.every((n) => ports[n.id])) return;
    const source = chainSource(model, { ports, optionKinds: (t) => this.optionKinds(t) });
    this.known = source;
    if (source !== this.text()) this.store.setFile(path, source);
  }

  // ---- illustration and chain files ----

  /** Make a new file of `kind` (asking its name), and open it. */
  newFile(kind: 'illustrations' | 'chains' | 'locks') {
    const what = kind === 'illustrations' ? 'illustration' : kind === 'chains' ? 'chain' : 'lock type';
    const name = prompt(`New ${what}'s name (letters, digits and _):`)?.trim();
    if (!name) return;
    if (!VALID_NAME.test(name)) return this.note.set({ kind: 'error', text: 'A name is letters, digits and _ only.' });
    const path = pathOf(kind, name);
    if (!(path in this.store.files())) this.store.setFile(path, template(kind, name, 'rhai'));
    this.shell.open(path);
  }

  /** Open file `path` in its editor. */
  openFile(path: string) {
    this.shell.open(path);
  }

  /** Fix input `port` to `value` in the chain itself, or (undefined) leave it open. */
  setFixed(port: string, value: TestValue | undefined) {
    this.update((m) => {
      const fixed = { ...m.fixed };
      if (value === undefined) delete fixed[port];
      else fixed[port] = value;
      return { ...m, fixed };
    }, `fixed:${port}`);
  }

  /** Give open input `port` a test value, or (undefined) none. */
  setTest(port: string, value: TestValue | undefined) {
    this.update((m) => {
      const test = { ...m.test };
      if (value === undefined) delete test[port];
      else test[port] = value;
      return { ...m, test };
    }, `test:${port}`);
  }

  setDescription(description: string) {
    this.update((m) => ({ ...m, description }), 'description');
  }

  // ---- the file ----

  /** Read chain file `path` (`source`, its text) as the model to edit. */
  private async read(path: string, source: string): Promise<boolean> {
    const reading = ++this.reading;
    const name = nameOf(path);
    let file: ChainFile | { error: string };
    try {
      file = await this.tools.readChain(name, source);
    } catch (err) {
      file = { error: (err as Error).message };
    }
    if (reading !== this.reading) return false;
    this.known = source;
    if (failed(file)) {
      this.note.set({ kind: 'error', text: `Couldn't read ${path}: ${file.error}. Fix it in a text editor, or remove it.` });
      return false;
    }
    const model = await this.modelOf(file);
    if (reading !== this.reading) return false;
    const reopened = this.model().name === model.name;
    this.untouched = model;
    this.model.set(model);
    if (!reopened) {
      this.undoStack = [];
      this.redoStack = [];
      this.history();
      this.selected.set(null);
      this.opened.update((n) => n + 1);
    }
    this.note.set(file.promotionsLost
      ? { kind: 'note', text: `${name}'s chain(o) works its options out, which the editor can't show: they're read with their defaults, and editing writes them as values.` }
      : null);
    return true;
  }

  /** A chain file read back as the editor's model. */
  private async modelOf(file: ChainFile): Promise<ChainModel> {
    const split = (given: Record<string, unknown> | undefined) => {
      const options: Record<string, unknown> = {};
      const promoted: Record<string, string> = {};
      for (const [name, value] of Object.entries(given ?? {})) {
        if (value && typeof value === 'object' && '$option' in value) promoted[name] = (value as { $option: string }).$option;
        else options[name] = value;
      }
      return { options, promoted };
    };
    const nodes: ChainNode[] = Object.entries(file.chain.locks).map(([id, lock], i) => {
      const [x, y] = file.layout?.[id] ?? [120 + 360 * i, 60];
      return { id, type: lock.type, ...split(lock.options), x, y };
    });
    const model: ChainModel = {
      ...chain.emptyChain(file.name),
      description: file.description,
      options: file.options.map((o) => ({ name: o.name, kind: o.kind, default: o.default, choices: o.choices })),
      nodes,
      wires: (file.chain.connect ?? []).map(([from, to]) => ({ from, to })),
    };
    for (const [id, spec] of Object.entries(file.chain.sources ?? {})) {
      const [x, y] = file.layout?.[id] ?? [-240, 60 + 140 * model.sources.length];
      const options = Object.fromEntries(Object.entries(spec.options ?? {}).filter(([, v]) => !(v && typeof v === 'object' && '$option' in v)));
      model.sources.push({ id, kind: spec.type, name: spec.name, options, ...(spec.base !== undefined ? { base: spec.base } : {}), x, y });
    }
    for (const [id, spec] of Object.entries(file.chain.illustrations ?? {})) {
      const [x, y] = file.layout?.[id] ?? [120 + 360 * nodes.length, 300];
      model.renderers.push({ id, type: spec.type, ...split(spec.options), x, y });
      for (const [input, port] of Object.entries(spec.inputs ?? {})) model.renderWires.push({ from: port, to: `${id}.${input}` });
    }
    // Values are read back as JSON: symbols become text once their ports' types are known.
    const ports: PortsByNode = {};
    for (const node of nodes) {
      ports[node.id] = await this.tools.lockPorts(node.type, chain.resolvedOptions(model, node)).catch((err: Error) => ({ error: err.message }));
    }
    for (const [port, value] of Object.entries(file.chain.fixed ?? {})) {
      model.fixed[port] = chain.testValueOf(value, chain.portType(ports, port, 'inputs'));
    }
    for (const [port, value] of Object.entries(file.example ?? {})) {
      model.test[port] = chain.testValueOf(value, chain.portType(ports, port, 'inputs'));
    }
    return model;
  }

  /** Rename the chain: its file. Returns why it can't be, or null. */
  rename(name: string): string | null {
    const path = this.path();
    if (!path || name === nameOf(path)) return null;
    if (!VALID_NAME.test(name)) return "a chain's name is its file's: letters, digits and _ only";
    if (!this.own()) return "it's the library's: change it first, so the journal has a copy to rename";
    const to = pathOf('chains', name);
    const problem = this.store.renameFile(path, to);
    if (problem) return problem;
    this.model.update((m) => ({ ...m, name }));
    this.untouched = this.model();
    this.path.set(to);
    this.shell.open(to);
    return null;
  }

  /** Give the journal a copy of the library's chain, to change. */
  copyIn() {
    const path = this.path();
    const text = this.text();
    if (path && text !== null && !this.own()) this.store.setFile(path, text);
  }

  /** Remove the journal's copy of a library chain: the library's is used again. */
  useLibrary() {
    const path = this.path();
    if (!path || !this.own() || !this.builtin()) return;
    if (this.writeTimer) clearTimeout(this.writeTimer);
    this.store.removeFile(path);
  }

  /** The chain as journal script, for the clipboard: chain("name") in use, or the chain built lock by lock. */
  snippet(kind: 'chain' | 'script'): string {
    return kind === 'chain'
      ? journalSnippet(this.model(), this.ports())
      : journalChain(this.model(), { ports: this.ports(), optionKinds: (t) => this.optionKinds(t) });
  }

}

/** A value to start a required option at, so a new lock has ports to show. */
function startingValue(option: LockOption): unknown {
  switch (option.kind) {
    case 'integer':
      return /base/.test(option.name) ? 27 : /count/.test(option.name) ? 2 : 1;
    case 'number':
      return 1;
    case 'bool':
      return false;
    case 'array':
      return [];
    case 'one_of':
      return option.choices?.[0];
    default:
      return /order/.test(option.name) ? 'ABCDEFGHIJKLMNOPQRSTUVWXYZ' : '';
  }
}

/** A source as a chain file's sources write it, for the engine. */
function sourceSpec(source: SourceNode): Record<string, unknown> {
  return {
    type: source.kind,
    name: source.name,
    ...(Object.keys(source.options).length && (source.kind === 'style' || source.kind === 'layout') ? { options: source.options } : {}),
    ...(source.base !== undefined && source.kind === 'content' ? { base: source.base } : {}),
  };
}
