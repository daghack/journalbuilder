import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import type { ChainFile, ChainRun, ChainType, LockOption, LockType, RenderPreview, RendererType } from '../scribe';
import { FileService, RHAI_FILES, type OpenedFile } from '../services/file.service';
import { ToolsService, failed } from '../services/tools.service';
import { WorkspaceService } from '../services/workspace.service';
import * as chain from './chain-model';
import type { ChainModel, ChainNode, PortsByNode, RendererNode, TestValue } from './chain-model';
import { chainSource, hasExample, journalChain, journalSnippet } from './chain-source';

const DRAFT_KEY = 'journal-editor.lock-draft';
const RENDERER_DRAFT_KEY = 'journal-editor.renderer-draft';

/** A renderer file being written in the editor. */
export interface RendererDraft {
  name: string;
  source: string;
}

/** A new renderer file to start from. */
export function rendererTemplate(name: string): string {
  return `// ${name}: what it draws, and how.

fn options() {
\t#{ size: 40.0 }
}

fn inputs(o) {
\t#{ text: "Base27Symbol" }
}

fn render(o, i) {
\tlet c = new_canvas();
\tlet style = new_style(new_brush_style("#2b1d12", "Ink Pen", 1.6), new_handwriting_style());
\tc.add(write_line(\`\${i.text.len()} symbols\`, vec2(0, 0), #{ style: style, type: new_type_style(glyphset("plain"), o.size) }));
\tc
}

// What the editor previews it with.
fn example() {
\t#{ text: symbols(27, "HELLO").symbols() }
}
`;
}
const HISTORY = 200;

/** A message for the panel: what happened, or went wrong. */
export interface Note {
  kind: 'ok' | 'error' | 'note';
  text: string;
}

/**
 * The chain being built in the lock editor: its model, each lock's ports
 * (which depend on its options, so the renderer works them out), and the
 * test run, made again whenever the chain or its test changes. As it's
 * edited, the chain is a chain file in the workspace under its name, so
 * journals using chain(name) follow along, as they do font edits.
 */
@Injectable()
export class LockEditorService {
  private readonly tools = inject(ToolsService);
  private readonly workspace = inject(WorkspaceService);
  private readonly files = inject(FileService);

  readonly types = signal<LockType[]>([]);
  readonly rendererTypes = signal<RendererType[]>([]);
  /** The chain files: compiled in, and the workspace's (this one among them, as it's edited). */
  readonly chainTypes = signal<ChainType[]>([]);
  /** What each renderer in the chain draws of the last run, by renderer. */
  readonly previews = signal<Record<string, RenderPreview | { error: string }>>({});
  /** The renderer file being written, if one is: the middle shows it in place of the chain. */
  readonly editing = signal<RendererDraft | null>(readRendererDraft());
  readonly editingPreview = signal<RenderPreview | { error: string } | null>(null);
  readonly editingSaved = signal<string | null>(null);
  /** Each value kind's example, rhai that makes one, by kind name. */
  readonly kinds = signal<Map<string, string | null>>(new Map());
  readonly typesError = signal<string | null>(null);
  readonly model = signal<ChainModel>(readDraft() ?? chain.emptyChain());
  readonly ports = signal<PortsByNode>({});
  /** The lock picked in the graph, and the wire (by the input it feeds). */
  readonly selected = signal<string | null>(null);
  readonly selectedWire = signal<string | null>(null);
  readonly result = signal<ChainRun | { error: string } | null>(null);
  readonly running = signal(false);
  readonly note = signal<Note | null>(null);
  /** Counts chains opened or started, for the graph to fit each in view. */
  readonly opened = signal(0);
  private handle: OpenedFile['handle'] = null;

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
    const unplaced = { ...model, nodes: model.nodes.map((n) => ({ ...n, x: 0, y: 0 })), renderers: [], renderWires: [], test: {} };
    return chainSource(unplaced, { ports: this.ports(), optionKinds: (t) => this.optionKinds(t) });
  });

  /** Why the chain can't run yet, if it can't. */
  readonly blocked = computed<string | null>(() => {
    const model = this.model();
    if (!model.nodes.length) return 'Add a lock from the left to start a chain.';
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
  private draftTimer: ReturnType<typeof setTimeout> | null = null;
  private liveTimer: ReturnType<typeof setTimeout> | null = null;
  private previewTimer: ReturnType<typeof setTimeout> | null = null;
  private editingTimer: ReturnType<typeof setTimeout> | null = null;
  /** The name the chain was last put in the workspace under, as it's edited: renamed, it moves. */
  private liveName: string | null = null;
  /**
   * The chain as opened, or as kept from the last visit: it goes into the
   * workspace only once it's edited, so an old draft never stands in for a
   * chain file that's changed since.
   */
  private readonly untouched = signal<ChainModel | null>(this.model());
  /** The chain as last opened, and the workspace's copy it was read from (null: the file compiled in), for Revert. */
  private readonly loaded = signal<{ name: string; model: ChainModel; source: string | null } | null>(null);
  /** Whether the chain being edited is one of webapp/library/chains/'s. */
  readonly builtin = computed(() => this.chainTypes().some((c) => c.name === this.model().name && c.builtin));
  /**
   * Whether there's something to revert: this browser's copy of a chain in
   * webapp/library/chains/, or edits since a chain only this browser has was opened.
   */
  readonly canRevert = computed(() => {
    if (this.builtin()) return this.model().name in this.workspace.chains() || this.model() !== this.untouched();
    const loaded = this.loaded();
    return loaded !== null && this.model() !== loaded.model;
  });
  private previewAsked = 0;
  private asked = 0;

  constructor() {
    // The lock and renderer types, again whenever the workspace's files change.
    effect(() => {
      this.workspace.locks();
      this.workspace.renderers();
      untracked(() => void this.loadTypes());
    });
    // The workspace cleared: nothing of this browser's is left to edit.
    effect(() => {
      if (!this.workspace.cleared()) return;
      untracked(() => {
        if (this.liveTimer) clearTimeout(this.liveTimer);
        const empty = chain.emptyChain('my_chain_1');
        this.untouched.set(empty);
        this.loaded.set(null);
        this.liveName = null;
        this.update(() => empty);
        this.closeRenderer();
        this.selected.set(null);
        this.note.set(null);
        this.opened.update((n) => n + 1);
      });
    });
    // The chain files likewise.
    effect(() => {
      this.workspace.chains();
      untracked(() => void this.loadChains());
    });
    // Each lock's and renderer's ports, when its type or options change (or its file does).
    effect(() => {
      const model = this.model();
      this.workspace.locks();
      this.workspace.renderers();
      untracked(() => this.refreshPorts(model));
    });
    // The chain as it's edited goes into the workspace under its name, so
    // journals using it (chain("name")) follow along, as font edits do.
    effect(() => {
      const model = this.model();
      const ports = this.ports();
      if (this.liveTimer) clearTimeout(this.liveTimer);
      this.liveTimer = setTimeout(() => untracked(() => this.pushLive(model, ports)), 400);
    });
    // What each renderer draws of the last run.
    effect(() => {
      const result = this.result();
      const model = this.model();
      const ports = this.ports();
      if (this.previewTimer) clearTimeout(this.previewTimer);
      this.previewTimer = setTimeout(() => untracked(() => void this.refreshPreviews(result, model, ports)), 150);
    });
    // A renderer file being written: previewed with its example() as it's
    // typed, and (when it draws) put in the workspace, so journals follow.
    effect(() => {
      const draft = this.editing();
      try {
        if (draft) localStorage.setItem(RENDERER_DRAFT_KEY, JSON.stringify(draft));
        else localStorage.removeItem(RENDERER_DRAFT_KEY);
      } catch {
        // Not kept, then.
      }
      if (this.editingTimer) clearTimeout(this.editingTimer);
      if (!draft) return;
      this.editingTimer = setTimeout(() => untracked(() => void this.previewEditing(draft)), 300);
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
    // Keep the chain between visits.
    effect(() => {
      const model = this.model();
      if (this.draftTimer) clearTimeout(this.draftTimer);
      this.draftTimer = setTimeout(() => {
        try {
          localStorage.setItem(DRAFT_KEY, JSON.stringify(model));
        } catch {
          // Not kept, then.
        }
      }, 300);
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
      this.rendererTypes.set(await this.tools.rendererTypes());
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

  private refreshPorts(model: ChainModel) {
    const ids = new Set(model.nodes.map((n) => n.id));
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
    // into the workspace as it's edited mustn't ask for them all again.
    const locks = this.workspace.locks();
    const renderers = this.workspace.renderers();
    for (const renderer of model.renderers) {
      const options = chain.resolvedOptions(model, renderer);
      const key = JSON.stringify(['renderer', renderer.type, options, renderers[renderer.type] ?? null]);
      if (this.portKeys.get(renderer.id) === key) continue;
      this.portKeys.set(renderer.id, key);
      void this.tools.rendererPorts(renderer.type, options).then(
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
      const key = JSON.stringify([node.type, options, locks[node.type] ?? null]);
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
    this.update((m) => ({ ...m, nodes: m.nodes.map(place), renderers: m.renderers.map(place) }), `move:${id}`);
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

  /** The last run's values for renderer `renderer`'s inputs, or null if it can't draw yet. */
  private rendererInputs(renderer: RendererNode, result: ChainRun | { error: string } | null, model: ChainModel, ports: PortsByNode): Record<string, unknown> | null {
    if (!result || failed(result)) return null;
    const p = chain.portsOf(ports, renderer.id);
    if (!p) return null;
    const trace = new Map(result.trace);
    const inputs: Record<string, unknown> = {};
    for (const [name] of p.inputs) {
      const wire = model.renderWires.find((w) => w.to === `${renderer.id}.${name}`);
      const value = wire ? trace.get(wire.from) : undefined;
      if (!value) return null;
      inputs[name] = value.json;
    }
    return inputs;
  }

  private async refreshPreviews(result: ChainRun | { error: string } | null, model: ChainModel, ports: PortsByNode) {
    const asked = ++this.previewAsked;
    const previews: Record<string, RenderPreview | { error: string }> = {};
    for (const renderer of model.renderers) {
      const inputs = this.rendererInputs(renderer, result, model, ports);
      if (!inputs) continue;
      previews[renderer.id] = await this.tools.renderPreview(renderer.type, null, chain.resolvedOptions(model, renderer), inputs).catch((err: Error) => ({ error: err.message }));
    }
    if (asked === this.previewAsked) this.previews.set(previews);
  }

  /**
   * Put the chain in the workspace under its name, as it is: a journal
   * using chain("name") runs it as it's edited, mistakes and all. Renamed,
   * it moves.
   */
  private pushLive(model: ChainModel, ports: PortsByNode) {
    const name = model.name;
    // Its values are written by their ports' types, so wait for the ports.
    if (!/^[A-Za-z0-9_]+$/.test(name) || !model.nodes.length || !model.nodes.every((n) => ports[n.id]) || model === untracked(() => this.untouched())) return;
    if (this.liveName && this.liveName !== name) this.workspace.removeChain(this.liveName);
    this.workspace.setChain(name, chainSource(model, { ports, optionKinds: (t) => this.optionKinds(t) }));
    this.liveName = name;
  }

  // ---- renderer files ----

  /** Write a new renderer file. */
  newRenderer() {
    const names = new Set(this.rendererTypes().map((t) => t.name));
    let name = 'my_renderer';
    for (let n = 2; names.has(name); n++) name = `my_renderer${n}`;
    this.editing.set({ name, source: rendererTemplate(name) });
    this.editingSaved.set(null);
  }

  /** Edit renderer `name`'s file (the workspace's, if it has one). */
  async editRenderer(name: string) {
    const answer = await this.tools.rendererSource(name).catch((err: Error) => ({ error: err.message }));
    if (failed(answer)) return this.note.set({ kind: 'error', text: `Couldn't open ${name}: ${answer.error}` });
    this.editing.set({ name, source: answer.source });
    this.editingSaved.set(null);
  }

  setRendererSource(source: string) {
    const draft = this.editing();
    if (draft && draft.source !== source) this.editing.set({ ...draft, source });
  }

  setRendererName(name: string) {
    const draft = this.editing();
    if (draft && draft.name !== name) this.editing.set({ ...draft, name });
  }

  closeRenderer() {
    this.editing.set(null);
    this.editingPreview.set(null);
  }

  private async previewEditing(draft: RendererDraft) {
    if (!/^[A-Za-z0-9_]+$/.test(draft.name)) {
      this.editingPreview.set({ error: "An illustration's name is its file's: letters, digits and _ only." });
      return;
    }
    const preview = await this.tools.renderPreview(draft.name, draft.source, {}, null).catch((err: Error) => ({ error: err.message }));
    if (this.editing() !== draft) return;
    this.editingPreview.set(preview);
    // It draws: journals and chains use it as it is now.
    if (!failed(preview)) this.workspace.setRenderer(draft.name, draft.source);
  }

  /** Save the illustration file: into the workspace, and to a file to put in webapp/library/illustrations/. */
  async saveRenderer() {
    const draft = this.editing();
    if (!draft) return;
    const preview = await this.tools.renderPreview(draft.name, draft.source, {}, null).catch((err: Error) => ({ error: err.message }));
    if (failed(preview)) return this.editingSaved.set(`${draft.name}.rhai doesn't draw its example: ${preview.error}`);
    this.workspace.setRenderer(draft.name, draft.source);
    try {
      await this.files.save(draft.source, `${draft.name}.rhai`, null, RHAI_FILES);
    } catch (err) {
      return this.editingSaved.set(`Added ${draft.name} to this browser's illustrations, but couldn't write the file: ${(err as Error).message}`);
    }
    this.editingSaved.set(`${draft.name} is an illustration in this browser now: in the palette, and in journals as lock.attach("x", "${draft.name}"). To add it to the repository, put ${draft.name}.rhai in webapp/library/illustrations/ and rebuild the renderer (npm run wasm).`);
  }

  /** Remove a renderer type only this browser has. */
  removeWorkspaceRenderer(name: string) {
    this.workspace.removeRenderer(name);
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

  setName(name: string) {
    this.update((m) => ({ ...m, name }), 'name');
  }

  setDescription(description: string) {
    this.update((m) => ({ ...m, description }), 'description');
  }

  // ---- files ----

  /** Start a new chain, under a name no chain has; the last one stays in the workspace. */
  newChain() {
    const names = new Set(this.chainTypes().map((c) => c.name));
    let n = 1;
    while (names.has(`my_chain_${n}`)) n++;
    this.update(() => chain.emptyChain(`my_chain_${n}`));
    this.liveName = null;
    this.handle = null;
    this.selected.set(null);
    this.note.set(null);
    this.opened.update((n) => n + 1);
  }

  /** Edit chain file `name` (one compiled in or in the workspace). */
  async openChain(name: string) {
    await this.read(name, null);
  }

  /** Open a chain file from disk. */
  async openFile() {
    const opened = await this.files.open(RHAI_FILES);
    if (!opened) return;
    const name = opened.name.replace(/\.rhai$/, '');
    if (await this.read(name, opened.text)) this.handle = opened.handle;
  }

  private async read(name: string, source: string | null): Promise<boolean> {
    let file: ChainFile | { error: string };
    try {
      file = await this.tools.readChain(name, source);
    } catch (err) {
      file = { error: (err as Error).message };
    }
    if (failed(file)) {
      this.note.set({ kind: 'error', text: `Couldn't open ${name}: ${file.error}` });
      return false;
    }
    const model = await this.modelOf(file);
    this.untouched.set(model);
    this.loaded.set({ name, model, source: source === null ? (this.workspace.chains()[name] ?? null) : null });
    this.update(() => model);
    // Editing it edits that chain: renamed, it moves.
    this.liveName = name;
    this.handle = null;
    this.selected.set(null);
    this.opened.update((n) => n + 1);
    this.note.set(file.promotionsLost
      ? { kind: 'note', text: `${name}'s chain(o) works its options out, which the editor can't show: they're read with their defaults, and editing writes them as values.` }
      : { kind: 'ok', text: `Editing ${name}: journals using chain("${name}") follow along.` });
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

  /**
   * Throw away this browser's edits to the chain. One in webapp/library/chains/ is
   * read again from there, its copy here gone, so journals use the file;
   * one only this browser has goes back to how it was opened.
   */
  async revert() {
    const name = this.model().name;
    if (this.builtin()) {
      if (this.liveTimer) clearTimeout(this.liveTimer);
      this.workspace.removeChain(name);
      if (await this.read(name, null)) this.note.set({ kind: 'ok', text: `${name} is back as it is in webapp/library/chains/; journals use that again.` });
      return;
    }
    const loaded = this.loaded();
    if (!loaded) return;
    const current = this.model().name;
    this.update(() => loaded.model);
    this.untouched.set(loaded.model);
    if (current !== loaded.name) this.workspace.removeChain(current);
    if (loaded.source === null) this.workspace.removeChain(loaded.name);
    else this.workspace.setChain(loaded.name, loaded.source);
    this.liveName = loaded.name;
    this.selected.set(null);
    this.note.set({ kind: 'ok', text: `${loaded.name} is back as it was opened.` });
  }

  /** Save the chain file, to put in webapp/library/chains/; it's in this browser's workspace already. */
  async save() {
    const model = this.model();
    const name = model.name;
    if (!/^[A-Za-z0-9_]+$/.test(name)) return this.note.set({ kind: 'error', text: 'A chain\'s name is its file\'s: letters, digits and _ only.' });
    const source = chainSource(model, { ports: this.ports(), optionKinds: (t) => this.optionKinds(t) });
    this.workspace.setChain(name, source);
    this.liveName = name;
    let saved: OpenedFile | null = null;
    try {
      saved = await this.files.save(source, `${name}.rhai`, this.handle, RHAI_FILES);
    } catch (err) {
      return this.note.set({ kind: 'error', text: `Couldn't write ${name}.rhai: ${(err as Error).message}` });
    }
    if (saved) this.handle = saved.handle;
    const result = this.result();
    const unchecked = !hasExample(model, this.ports()) ? ' It has no example yet: give every input a test value, and cargo test checks it.'
      : result && failed(result) ? ` Its test run fails, though: ${result.error}` : '';
    this.note.set({
      kind: 'ok',
      text: `Saved ${name}.rhai. To add it to the repository, put it in webapp/library/chains/ and rebuild the renderer (npm run wasm).${unchecked}`,
    });
  }

  /** The chain as journal script, for the clipboard: chain("name") in use, or the chain built lock by lock. */
  snippet(kind: 'chain' | 'script'): string {
    return kind === 'chain'
      ? journalSnippet(this.model(), this.ports())
      : journalChain(this.model(), { ports: this.ports(), optionKinds: (t) => this.optionKinds(t) });
  }

  /** Remove a lock type only this browser has. */
  removeWorkspaceLock(name: string) {
    this.workspace.removeLock(name);
  }

  /** Remove a chain file only this browser has (or this browser's edit of one compiled in). */
  removeWorkspaceChain(name: string) {
    this.workspace.removeChain(name);
    if (this.liveName === name) this.liveName = null;
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

function readRendererDraft(): RendererDraft | null {
  try {
    const draft = JSON.parse(localStorage.getItem(RENDERER_DRAFT_KEY) ?? 'null');
    return draft && typeof draft.name === 'string' && typeof draft.source === 'string' ? draft : null;
  } catch {
    return null;
  }
}

function readDraft(): ChainModel | null {
  try {
    const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null');
    if (!draft || !Array.isArray(draft.nodes) || !Array.isArray(draft.wires)) return null;
    // Drafts from before chains were chain files named their ports.
    const { inputNames: _, outputNames: __, ...rest } = draft;
    const renderers = (draft.renderers ?? []).map((r: RendererNode) => ({ ...r, promoted: r.promoted ?? {} }));
    return { ...chain.emptyChain(), ...rest, renderers };
  } catch {
    return null;
  }
}
