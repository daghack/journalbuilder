import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import type { ChainFile, ChainRun, LockOption, LockType } from '../scribe';
import { FileService, RHAI_FILES, type OpenedFile } from '../services/file.service';
import { ToolsService, failed } from '../services/tools.service';
import { WorkspaceService } from '../services/workspace.service';
import * as chain from './chain-model';
import type { ChainModel, ChainNode, PortsByNode, TestValue } from './chain-model';
import { chainSource, exampleProblem, journalChain, journalSnippet } from './chain-source';

const DRAFT_KEY = 'journal-editor.lock-draft';
const HISTORY = 200;

/** A message for the panel: what happened, or went wrong. */
export interface Note {
  kind: 'ok' | 'error' | 'note';
  text: string;
}

/**
 * The chain being built in the lock editor: its model, each lock's ports
 * (which depend on its options, so the renderer works them out), and the
 * test run, made again whenever the chain or its test changes.
 */
@Injectable()
export class LockEditorService {
  private readonly tools = inject(ToolsService);
  private readonly workspace = inject(WorkspaceService);
  private readonly files = inject(FileService);

  readonly types = signal<LockType[]>([]);
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
  readonly inputs = computed(() => chain.openInputs(this.model(), this.ports()));
  readonly outputs = computed(() => chain.openOutputs(this.model(), this.ports()));
  readonly node = computed(() => this.model().nodes.find((n) => n.id === this.selected()) ?? null);
  /** The chain's options' values for a test run: their defaults. */
  readonly optionValues = computed(() => Object.fromEntries(this.model().options.filter((o) => o.default !== undefined).map((o) => [o.name, o.default])));
  /** Problems with a lock's ports, by lock. */
  readonly nodeErrors = computed(() => {
    const errors: Record<string, string> = {};
    for (const [id, p] of Object.entries(this.ports())) if ('error' in p) errors[id] = p.error;
    return errors;
  });

  /** The chain as a lock file to run (no examples, nodes unplaced: moving a lock needn't run it again). */
  private readonly runSource = computed(() => {
    const model = this.model();
    const unplaced = { ...model, nodes: model.nodes.map((n) => ({ ...n, x: 0, y: 0 })) };
    return chainSource(unplaced, { ports: this.ports(), optionKinds: (t) => this.optionKinds(t), examples: false });
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
    const missing = this.inputs().filter((p) => !model.test[p.name]);
    if (missing.length) return `Give ${missing.map((p) => p.name).join(', ')} a test value to run the chain.`;
    return null;
  });

  private portKeys = new Map<string, string>();
  private runTimer: ReturnType<typeof setTimeout> | null = null;
  private draftTimer: ReturnType<typeof setTimeout> | null = null;
  private asked = 0;

  constructor() {
    // The lock types, again whenever the workspace's lock files change.
    effect(() => {
      this.workspace.locks();
      untracked(() => void this.loadTypes());
    });
    // Each lock's ports, when its type or options change (or a lock file does).
    effect(() => {
      const model = this.model();
      this.workspace.locks();
      untracked(() => this.refreshPorts(model));
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
      this.typesError.set(null);
    } catch (err) {
      this.typesError.set((err as Error).message);
    }
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
    const version = this.workspace.version();
    for (const node of model.nodes) {
      const options = chain.resolvedOptions(model, node);
      const key = JSON.stringify([node.type, options, version]);
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
    const inputs = Object.fromEntries(this.inputs().map((p) => [p.name, test[p.name]]));
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
    this.update((m) => ({ ...m, nodes: m.nodes.map((n) => (n.id === id ? { ...n, x: Math.round(x), y: Math.round(y) } : n)) }), `move:${id}`);
  }

  removeNode(id: string) {
    this.update((m) => chain.removeNode(m, id));
    if (this.selected() === id) this.selected.set(null);
  }

  renameNode(id: string, to: string): string | null {
    if (to === id) return null;
    if (!to || to.includes('.')) return "a lock's name must be non-empty, without '.'";
    if (this.model().nodes.some((n) => n.id === to)) return `there's already a lock named '${to}'`;
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

  /** Let the chain set lock `id`'s `option`: it becomes one of the chain's own options. */
  promote(id: string, option: string) {
    const model = this.model();
    const node = model.nodes.find((n) => n.id === id);
    const spec = this.typeByName().get(node?.type ?? '')?.options?.find((o) => o.name === option);
    if (!node || !spec) return;
    let name = option;
    for (let n = 2; model.options.some((o) => o.name === name); n++) name = `${option}${n}`;
    const value = option in node.options ? node.options[option] : spec.default;
    this.update((m) => ({
      ...m,
      options: [...m.options, { name, kind: spec.kind, default: value, choices: spec.choices }],
      nodes: m.nodes.map((n) => {
        if (n.id !== id) return n;
        const { [option]: _, ...options } = n.options;
        return { ...n, options, promoted: { ...n.promoted, [option]: name } };
      }),
    }));
  }

  /** Give lock `id`'s `option` a value of its own again. */
  demote(id: string, option: string) {
    this.update((m) => {
      const node = m.nodes.find((n) => n.id === id);
      const chainOption = node?.promoted[option];
      const value = m.options.find((o) => o.name === chainOption)?.default;
      const nodes = m.nodes.map((n) => {
        if (n.id !== id) return n;
        const { [option]: _, ...promoted } = n.promoted;
        return { ...n, promoted, options: value === undefined ? n.options : { ...n.options, [option]: value } };
      });
      const used = new Set(nodes.flatMap((n) => Object.values(n.promoted)));
      return { ...m, nodes, options: m.options.filter((o) => used.has(o.name)) };
    });
  }

  setChainOption(name: string, patch: Partial<chain.ChainOption>) {
    this.update((m) => {
      const options = m.options.map((o) => (o.name === name ? { ...o, ...patch } : o));
      let nodes = m.nodes;
      if (patch.name !== undefined && patch.name !== name) {
        nodes = nodes.map((n) => ({ ...n, promoted: Object.fromEntries(Object.entries(n.promoted).map(([k, v]) => [k, v === name ? patch.name! : v])) }));
      }
      return { ...m, options, nodes };
    }, `chain-option:${name}`);
  }

  /** Wire `from` to `to`, unless it can't be. Returns why not. */
  connect(from: string, to: string): string | null {
    const problem = chain.wireProblem(this.model(), this.ports(), from, to);
    if (problem) return problem;
    this.update((m) => chain.connect(m, from, to));
    return null;
  }

  removeWire(to: string) {
    this.update((m) => ({ ...m, wires: m.wires.filter((w) => w.to !== to) }));
    if (this.selectedWire() === to) this.selectedWire.set(null);
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

  setPortName(side: 'inputNames' | 'outputNames', port: string, name: string) {
    this.update((m) => {
      const names = { ...m[side] };
      if (name) names[port] = name;
      else delete names[port];
      // The test case follows an input to its new name.
      let test = m.test;
      if (side === 'inputNames') {
        const old = chain.openInputs(m, this.ports()).find((p) => p.port === port)?.name;
        if (old && old in test && name) {
          const { [old]: value, ...rest } = test;
          test = { ...rest, [name]: value };
        }
      }
      return { ...m, [side]: names, test };
    }, `name:${side}:${port}`);
  }

  setTest(name: string, value: TestValue | undefined) {
    this.update((m) => {
      const test = { ...m.test };
      if (value === undefined) delete test[name];
      else test[name] = value;
      return { ...m, test };
    }, `test:${name}`);
  }

  setName(name: string) {
    this.update((m) => ({ ...m, name }), 'name');
  }

  setDescription(description: string) {
    this.update((m) => ({ ...m, description }), 'description');
  }

  // ---- files ----

  newChain() {
    this.update(() => chain.emptyChain());
    this.handle = null;
    this.selected.set(null);
    this.note.set(null);
    this.opened.update((n) => n + 1);
  }

  /** Open composite lock type `type` (one compiled in or in the workspace). */
  async openType(type: string) {
    await this.read(type, null);
  }

  /** Open a composite lock file from disk. */
  async openFile() {
    const opened = await this.files.open(RHAI_FILES);
    if (!opened) return;
    const name = opened.name.replace(/\.rhai$/, '');
    if (await this.read(name, opened.text)) this.handle = opened.handle;
  }

  private async read(type: string, source: string | null): Promise<boolean> {
    let file: ChainFile | { error: string };
    try {
      file = await this.tools.readChain(type, source);
    } catch (err) {
      file = { error: (err as Error).message };
    }
    if (failed(file)) {
      this.note.set({ kind: 'error', text: `Couldn't open ${type}: ${file.error}` });
      return false;
    }
    const model = await this.modelOf(file);
    this.update(() => model);
    this.handle = null;
    this.selected.set(null);
    this.opened.update((n) => n + 1);
    this.note.set(file.promotionsLost
      ? { kind: 'note', text: `${type}'s chain(o) works its options out, which the editor can't show: they're read with their defaults, and saving writes them as values.` }
      : { kind: 'ok', text: `Opened ${type}.` });
    return true;
  }

  /** A chain file read back as the editor's model. */
  private async modelOf(file: ChainFile): Promise<ChainModel> {
    const nodes: ChainNode[] = Object.entries(file.chain.locks).map(([id, lock], i) => {
      const options: Record<string, unknown> = {};
      const promoted: Record<string, string> = {};
      for (const [name, value] of Object.entries(lock.options ?? {})) {
        if (value && typeof value === 'object' && '$option' in value) promoted[name] = (value as { $option: string }).$option;
        else options[name] = value;
      }
      const [x, y] = file.layout?.[id] ?? [120 + 360 * i, 60];
      return { id, type: lock.type, options, promoted, x, y };
    });
    const model: ChainModel = {
      name: file.name,
      description: file.description,
      options: file.options.map((o) => ({ name: o.name, kind: o.kind, default: o.default, choices: o.choices })),
      nodes,
      wires: (file.chain.connect ?? []).map(([from, to]) => ({ from, to })),
      fixed: {},
      inputNames: Object.fromEntries(Object.entries(file.chain.inputs).map(([name, port]) => [port, name])),
      outputNames: Object.fromEntries(Object.entries(file.chain.outputs).map(([name, port]) => [port, name])),
      test: {},
    };
    // Values are read back as JSON: symbols become text once their ports' types are known.
    const ports: PortsByNode = {};
    for (const node of nodes) {
      ports[node.id] = await this.tools.lockPorts(node.type, chain.resolvedOptions(model, node)).catch((err: Error) => ({ error: err.message }));
    }
    for (const [port, value] of Object.entries(file.chain.fixed ?? {})) {
      model.fixed[port] = chain.testValueOf(value, chain.portType(ports, port, 'inputs'));
    }
    const example = file.examples?.[0];
    for (const [name, value] of Object.entries(example?.inputs ?? {})) {
      const port = file.chain.inputs[name];
      model.test[name] = chain.testValueOf(value, port ? chain.portType(ports, port, 'inputs') : null);
    }
    return model;
  }

  /**
   * Save the chain as a lock type: checked (its example run, locked and
   * unlocked), added to the workspace so journals and other chains can use
   * it, and written to a file to put in locks/.
   */
  async save() {
    const model = this.model();
    const name = model.name;
    if (!/^[A-Za-z0-9_]+$/.test(name)) return this.note.set({ kind: 'error', text: 'A lock type\'s name is its file\'s: letters, digits and _ only.' });
    const existing = this.typeByName().get(name);
    if (existing && !existing.composite && !existing.workspace) {
      return this.note.set({ kind: 'error', text: `There's a lock file named ${name} already; pick another name.` });
    }
    if (model.nodes.some((n) => n.type === name)) return this.note.set({ kind: 'error', text: `The chain can't include a ${name}: it would include itself.` });
    const problem = exampleProblem(model, this.ports());
    if (problem) return this.note.set({ kind: 'error', text: `Lock files need an example that runs: ${problem}.` });
    // Run the chain as it is now (the last test run may be a moment behind) for the outputs to pin.
    const inputs = Object.fromEntries(this.inputs().map((p) => [p.name, model.test[p.name]]));
    const result = await this.tools.runChain(this.runSource(), this.optionValues(), inputs).catch((err: Error) => ({ error: err.message }));
    if (failed(result)) return this.note.set({ kind: 'error', text: `Lock files need an example that runs: ${result.error}.` });
    const outputs = Object.fromEntries(result.outputs.map(([n, v]) => [n, chain.pinnable(v)]).filter(([, v]) => v !== null)) as Record<string, TestValue>;
    const source = chainSource(model, { ports: this.ports(), optionKinds: (t) => this.optionKinds(t), outputs });
    const checked = await this.tools.checkLock(name, source).catch((err: Error) => ({ error: err.message }));
    if (failed(checked)) return this.note.set({ kind: 'error', text: `${name}.rhai doesn't pass its example: ${checked.error}` });
    this.workspace.setLock(name, source);
    let saved: OpenedFile | null = null;
    try {
      saved = await this.files.save(source, `${name}.rhai`, this.handle, RHAI_FILES);
    } catch (err) {
      return this.note.set({ kind: 'error', text: `Added ${name} to this browser's lock types, but couldn't write the file: ${(err as Error).message}` });
    }
    if (saved) this.handle = saved.handle;
    this.note.set({
      kind: 'ok',
      text: `${name} is a lock type now in this browser: in the palette, and in journals as .add("x", "${name}", #{...}). `
        + `To add it to the repository, put ${name}.rhai in locks/, add a row for it to the showcase's locks page (tests/showcase.rs checks), and rebuild the renderer (npm run wasm).`,
    });
  }

  /** The chain as journal script, for the clipboard: the saved type in use, or the chain built lock by lock. */
  snippet(kind: 'type' | 'chain'): string {
    return kind === 'type'
      ? journalSnippet(this.model(), this.ports())
      : journalChain(this.model(), { ports: this.ports(), optionKinds: (t) => this.optionKinds(t) });
  }

  /** Remove a lock type only this browser has. */
  removeWorkspaceLock(name: string) {
    this.workspace.removeLock(name);
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

function readDraft(): ChainModel | null {
  try {
    const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null');
    return draft && Array.isArray(draft.nodes) && Array.isArray(draft.wires) ? { ...chain.emptyChain(), ...draft } : null;
  } catch {
    return null;
  }
}
