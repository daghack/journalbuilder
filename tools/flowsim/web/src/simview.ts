// The flow simulator's view: the stage, the tools, the bar and the panels
// round an engine (wasm), run a frame at a time. Its page (main.ts) hosts
// one, and so does the journal editor's Fluid tab, each giving it the
// elements to build its parts in.

import { FlowSim } from '../wasm-pkg/flowsim.js';
import { Exporter, download } from './exporter';
import type { Edit } from './form';
import { History } from './history';
import { type Obj, type SceneState, type Stats, type Vec, clone, dist, moveBy, moving } from './model';
import { type PanelHost, type ViewSettings, buildPanel } from './panel';
import { load, save, saveText } from './store';
import { TOOLS, type ToolName, create, dragHandle, handles, hits, nearestPath, places, shape } from './tools';
import { type Arrows, type PathBundle, Renderer, bundle } from './view';

const VIEW_KEY = 'flowsim.view';
const PANEL_KEY = 'flowsim.panel';

type Drag =
  | { type: 'create'; obj: Obj; p0: Vec; moved: boolean }
  | { type: 'move'; obj: Obj; last: Vec }
  | { type: 'handle'; obj: Obj; id: string }
  | { type: 'stir'; last: Vec; time: number }
  | { type: 'drop'; last: Vec };

const DEFAULT_VIEW: ViewSettings = {
  mode: 'dye',
  arrows: false,
  recorded: true,
  live: true,
  particles: true,
  objects: true,
  brush: 40,
};

/** The elements a view builds its parts in. Each gets the class `flowsim`
 * and its part's (`bar`, `rail`, ...), which style.css styles. */
export interface ViewParts {
  /** The buttons go at its end. */
  bar: HTMLElement;
  rail: HTMLElement;
  stage: HTMLElement;
  panel: HTMLElement;
  status: HTMLElement;
  /** Where its keyboard shortcuts are listened for: the window on a page
   * of its own, an element round the parts when it's one of many things. */
  keys?: HTMLElement | Window;
}

export interface ViewOptions {
  /** Keeps the scene in the browser under this key as it changes. */
  sceneKey?: string;
  /** The bar's file buttons (New, Open, Save scene, Export SVG) and the
   * path ones (Bake): for the page of its own. Without them it's a fluid
   * to shape, as the Fluid tab has it. */
  files?: boolean;
  /** Called whenever the fluid changes: each step while it runs, and
   * freezing, stirring or moving things while it's frozen. */
  onFieldChange?: () => void;
  /** Called a moment after the scene (objects or settings) changes. */
  onSceneChange?: () => void;
  /** Called when the flow is run or frozen. */
  onRunChange?: (frozen: boolean) => void;
}

/** The page drawn faintly under the fluid, `rect` its box in domain units. */
interface Backdrop {
  image: HTMLCanvasElement;
  rect: [number, number, number, number];
}

const BUTTONS: Record<string, { label: string; title: string; primary?: boolean }> = {
  run: { label: 'Freeze', title: 'Run or freeze the flow (Space)', primary: true },
  step: { label: 'Step', title: 'Run the flow one step' },
  reset: { label: 'Reset fluid', title: 'Still the fluid. Frozen, this gives the smooth flow the edges and obstacles alone make' },
  bake: { label: 'Bake paths', title: "Keep the paths the seeds trace now as recorded paths (B)" },
  'clear-paths': { label: 'Clear paths', title: 'Remove every recorded path' },
  'clear-particles': { label: 'Clear particles', title: 'Remove the moving particles' },
  undo: { label: 'Undo', title: 'Undo (Ctrl/⌘ Z)' },
  redo: { label: 'Redo', title: 'Redo (Ctrl/⌘ Shift Z)' },
  new: { label: 'New', title: 'Start again from the default scene' },
  open: { label: 'Open…', title: 'Open a saved scene' },
  save: { label: 'Save scene', title: 'Save the scene, recorded paths and all, as JSON' },
  export: { label: 'Export SVG', title: 'Export the paths as SVG', primary: true },
};

const FULL_BAR = ['run', 'step', 'reset', '|', 'bake', 'clear-paths', 'clear-particles', '|', 'undo', 'redo', '-', 'new', 'open', 'save', 'export'];
const FLUID_BAR = ['run', 'step', 'reset', '|', 'clear-paths', 'clear-particles', '|', 'undo', 'redo'];

function sceneState(json: string): SceneState {
  const s = JSON.parse(json) as SceneState & { paths?: unknown };
  return { flow: s.flow, particles: s.particles, objects: s.objects };
}

export class FlowSimView implements PanelHost {
  tool: ToolName = 'select';
  selection: number | null = null;
  selectedPath: number | null = null;
  hover: number | null = null;
  view: ViewSettings = { ...DEFAULT_VIEW, ...(load<Partial<ViewSettings>>(VIEW_KEY) ?? {}) };
  state: SceneState;
  readonly history: History;
  private renderer: Renderer;
  private canvas = document.createElement('canvas');
  private rail: HTMLElement;
  private panelRoot: HTMLElement;
  private statusEl: HTMLElement;
  private toastEl = document.createElement('div');
  private keys: HTMLElement | Window;
  private panelOpen: Record<string, boolean> = load(PANEL_KEY) ?? {};
  private field = document.createElement('canvas');
  private fieldCtx = this.field.getContext('2d')!;
  private arrows: Arrows | null = null;
  private recorded: PathBundle | null = null;
  private live: PathBundle | null = null;
  private trails: PathBundle | null = null;
  private heads: Float32Array | null = null;
  private versions = { field: -1, paths: -1, live: -1, objects: -1 };
  private fieldStale = true;
  private dirty = true;
  private drag: Drag | null = null;
  private pointer: Vec | null = null;
  private stepMs = 0;
  private lastStatus = 0;
  private saveTimer = 0;
  private saveWarned = false;
  private toastTimer = 0;
  private exporter: Exporter | null = null;
  private buttons: Record<string, HTMLButtonElement> = {};
  private backdrop: Backdrop | null = null;
  private frameId = 0;
  private resize: ResizeObserver;
  private gone = false;

  constructor(
    readonly sim: FlowSim,
    parts: ViewParts,
    private options: ViewOptions = {},
  ) {
    for (const [name, el] of Object.entries(parts)) {
      if (el instanceof HTMLElement) el.classList.add('flowsim', name);
    }
    this.rail = parts.rail;
    this.panelRoot = parts.panel;
    this.statusEl = parts.status;
    this.keys = parts.keys ?? window;
    this.canvas.tabIndex = 0;
    this.toastEl.className = 'toast';
    this.toastEl.setAttribute('role', 'status');
    parts.stage.append(this.canvas, this.toastEl);
    this.state = sceneState(sim.scene());
    this.renderer = new Renderer(this.canvas);
    this.history = new History(
      () => this.sim,
      () => JSON.stringify(this.state),
      (json) => this.restoreScene(json),
      () => this.updateButtons(),
    );
    if (options.files) this.exporter = new Exporter(() => this.sim, FlowSim.default_svg_options());
    this.buildRail();
    this.buildBar(parts.bar);
    this.wireCanvas();
    this.keys.addEventListener('keydown', this.onKey as EventListener);
    this.resize = new ResizeObserver(() => (this.dirty = true));
    this.resize.observe(parts.stage);
    this.rebuildPanel();
    this.updateButtons();
    this.frameId = requestAnimationFrame(this.frame);
  }

  /** Stops it: no more frames, keys or saves. The engine is the host's. */
  destroy(): void {
    this.gone = true;
    cancelAnimationFrame(this.frameId);
    clearTimeout(this.saveTimer);
    this.resize.disconnect();
    this.keys.removeEventListener('keydown', this.onKey as EventListener);
  }

  // --- for a host ---

  /** Draws `svg` faintly under the fluid, in the box `rect` (`[x, y, width,
   * height]` in domain units); `null` for nothing. */
  setBackdrop(svg: string | null, rect: [number, number, number, number] = [0, 0, 1, 1]): void {
    if (!svg) {
      this.backdrop = null;
      this.dirty = true;
      return;
    }
    const image = new Image();
    const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
    image.onload = () => {
      URL.revokeObjectURL(url);
      // Drawn once, in a light ink that shows on the dark fluid.
      const k = Math.min(2048 / Math.max(rect[2], rect[3]), 4);
      const c = document.createElement('canvas');
      c.width = Math.max(1, Math.round(rect[2] * k));
      c.height = Math.max(1, Math.round(rect[3] * k));
      const ctx = c.getContext('2d')!;
      ctx.drawImage(image, 0, 0, c.width, c.height);
      ctx.globalCompositeOperation = 'source-in';
      ctx.fillStyle = '#f4efe6';
      ctx.fillRect(0, 0, c.width, c.height);
      this.backdrop = { image: c, rect };
      this.dirty = true;
    };
    image.onerror = () => URL.revokeObjectURL(url);
    image.src = url;
  }

  /** Makes an alpha raster an obstacle: `nx` × `ny` square cells `cell`
   * across from the domain's corner, row by row, solid where at least half
   * opaque, grown by `grow`; `null` for none. Whether any of it is solid. */
  setMask(alpha: Uint8Array | null, nx = 0, ny = 0, cell = 1, grow = 0): boolean {
    const any = alpha ? this.sim.set_mask(nx, ny, cell, alpha, grow) : (this.sim.clear_mask(), false);
    this.dirty = true;
    return any;
  }

  /** Looks closer: `zoom` times closer than fitting the domain in, with
   * the domain point `center` in the middle (null for the domain's own). */
  setZoom(zoom: number, center: [number, number] | null): void {
    this.renderer.zoom = Math.max(zoom, 1e-3);
    this.renderer.center = center;
    this.dirty = true;
  }

  /** The domain point under a point of the screen. */
  domainAt(clientX: number, clientY: number): [number, number] {
    return this.renderer.toDomain(clientX, clientY);
  }

  /** Screen pixels to a domain unit, as drawn now. */
  get unitScale(): number {
    return this.renderer.scale;
  }

  /** Sets the domain's size, in domain units. */
  setDomain(width: number, height: number): void {
    if (this.state.flow.width === width && this.state.flow.height === height) return;
    this.state.flow.width = width;
    this.state.flow.height = height;
    this.pushSettings();
    this.rebuildPanel();
  }

  /** Opens a scene (as `sim.scene()` gives it), forgetting the undo history. */
  loadScene(json: string): void {
    this.sim.set_scene(json);
    this.reloadFromSim();
  }

  /** Freezes the flow as a kept field (`sim.field_json()`'s) has it. */
  loadField(json: string): void {
    this.sim.load_field(json);
    this.updateButtons();
    this.dirty = true;
  }

  get frozen(): boolean {
    return this.sim.frozen();
  }

  /** Freezes the flow if it's running, as its Freeze button does. */
  freeze(): void {
    if (!this.sim.frozen()) this.toggleRun();
  }

  // --- PanelHost ---

  get flow() {
    return this.state.flow;
  }
  get particles() {
    return this.state.particles;
  }

  selected(): Obj | null {
    return this.state.objects.find((o) => o.id === this.selection) ?? null;
  }

  pathPoints(index: number): number {
    const r = this.recorded;
    if (!r || index + 1 >= r.offsets.length) return 0;
    return r.offsets[index + 1] - r.offsets[index];
  }

  objectEdit: Edit = {
    begin: () => this.history.recordScene(),
    change: () => this.pushObjects(),
  };

  settingsEdit: Edit = {
    begin: () => this.history.recordScene(),
    change: () => this.pushSettings(),
  };

  viewEdit: Edit = {
    begin: () => {},
    change: () => {
      save(VIEW_KEY, this.view);
      this.fieldStale = true;
      this.dirty = true;
    },
  };

  deleteSelection(): void {
    if (this.selection !== null) {
      this.history.recordScene();
      this.state.objects = this.state.objects.filter((o) => o.id !== this.selection);
      this.selection = null;
      this.pushObjects();
      this.rebuildPanel();
    } else if (this.selectedPath !== null) {
      this.history.recordPaths();
      this.sim.delete_path(this.selectedPath);
      this.selectedPath = null;
      this.rebuildPanel();
    }
  }

  duplicateSelection(): void {
    const o = this.selected();
    if (!o) return;
    this.history.recordScene();
    const copy = clone(o);
    copy.id = this.nextId();
    moveBy(copy, 30, 30);
    this.state.objects.push(copy);
    this.selection = copy.id;
    this.pushObjects();
    this.rebuildPanel();
  }

  preset(name: 'tunnel' | 'tank'): void {
    this.history.recordScene();
    this.state.flow.edges =
      name === 'tunnel'
        ? { left: 'inflow', right: 'open', top: 'wall', bottom: 'wall' }
        : { left: 'wall', right: 'wall', top: 'wall', bottom: 'wall' };
    if (name === 'tank') this.state.flow.dye = 'none';
    else this.state.flow.dye = 'stripes';
    this.pushSettings();
    this.sim.reset_fluid();
    this.rebuildPanel();
    this.toast(name === 'tunnel' ? 'Wind tunnel: in on the left, out on the right' : 'Closed tank: stir it, or add jets and vortices');
  }

  // --- the scene ---

  private nextId(): number {
    return this.state.objects.reduce((m, o) => Math.max(m, o.id), 0) + 1;
  }

  private pushObjects(): void {
    try {
      this.sim.set_objects(JSON.stringify(this.state.objects));
    } catch (e) {
      this.toast(String(e));
    }
    this.dirty = true;
    this.scheduleSave(1000);
  }

  private pushSettings(): void {
    try {
      this.sim.set_settings(JSON.stringify({ flow: this.state.flow, particles: this.state.particles }));
    } catch (e) {
      this.toast(String(e));
    }
    this.fieldStale = true;
    this.dirty = true;
    this.scheduleSave(1000);
  }

  private restoreScene(json: string): void {
    this.state = JSON.parse(json) as SceneState;
    if (!this.selected()) this.selection = null;
    this.pushSettings();
    this.pushObjects();
    this.rebuildPanel();
  }

  /** Takes the scene from the engine, after it has loaded a new one. */
  private reloadFromSim(): void {
    this.state = sceneState(this.sim.scene());
    this.selection = null;
    this.selectedPath = null;
    this.history.clear();
    this.versions = { field: -1, paths: -1, live: -1, objects: -1 };
    this.fieldStale = true;
    this.dirty = true;
    this.rebuildPanel();
    this.updateButtons();
    this.scheduleSave(200);
  }

  private scheduleSave(delay: number): void {
    if (this.saveTimer || this.gone) return;
    this.saveTimer = window.setTimeout(() => {
      this.saveTimer = 0;
      this.options.onSceneChange?.();
      const key = this.options.sceneKey;
      if (key && !saveText(key, this.sim.scene()) && !this.saveWarned) {
        this.saveWarned = true;
        this.toast('Too much to keep in this browser: use Save scene to keep it in a file', 5000);
      }
    }, delay);
  }

  // --- the bar, rail and keys ---

  private buildRail(): void {
    const rail = this.rail;
    rail.setAttribute('aria-label', 'Tools');
    let group = '';
    for (const t of TOOLS) {
      if (t.group !== group) {
        group = t.group;
        rail.append(Object.assign(document.createElement('h3'), { textContent: group }));
      }
      const b = document.createElement('button');
      b.type = 'button';
      b.dataset['tool'] = t.name;
      b.title = `${t.label}${t.key ? ` (${t.key.toUpperCase()})` : ''}: ${t.hint}`;
      b.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${t.icon}</svg><span>${t.label}</span>`;
      b.addEventListener('click', () => this.setTool(t.name));
      rail.append(b);
    }
    this.setTool('select');
  }

  private setTool(tool: ToolName): void {
    this.tool = tool;
    for (const b of this.rail.querySelectorAll<HTMLButtonElement>('button')) {
      b.classList.toggle('active', b.dataset['tool'] === tool);
    }
    this.canvas.classList.toggle('pointer', tool === 'select' || tool === 'delete');
    this.dirty = true;
    if (this.selection === null && this.selectedPath === null) this.rebuildPanel();
  }

  private buildBar(bar: HTMLElement): void {
    for (const id of this.options.files ? FULL_BAR : FLUID_BAR) {
      if (id === '|' || id === '-') {
        bar.append(Object.assign(document.createElement('span'), { className: id === '|' ? 'sep' : 'grow' }));
        continue;
      }
      const { label, title, primary } = BUTTONS[id];
      const b = Object.assign(document.createElement('button'), { type: 'button', textContent: label, title });
      if (primary) b.classList.add('primary');
      this.buttons[id] = b;
      bar.append(b);
    }
    const on = (id: string, f: () => void) => this.buttons[id]?.addEventListener('click', f);
    on('run', () => this.toggleRun());
    on('step', () => {
      this.sim.step_once(this.state.flow.speed / 60);
      this.dirty = true;
    });
    on('reset', () => {
      this.sim.reset_fluid();
      this.toast(this.sim.frozen() ? 'Fluid stilled: the frozen flow is now the smooth flow round everything' : 'Fluid stilled');
    });
    on('bake', () => this.bake());
    on('clear-paths', () => {
      const some = (b: PathBundle | null) => !!b && b.offsets.length > 1;
      if (!some(this.recorded) && !some(this.live) && !some(this.trails)) return;
      this.history.recordPaths();
      this.sim.clear_paths();
      this.selectedPath = null;
      this.rebuildPanel();
      if (some(this.live)) this.toast('Cleared. Seeds trace their paths again when something changes');
    });
    on('clear-particles', () => this.sim.clear_particles());
    on('undo', () => this.history.undo());
    on('redo', () => this.history.redo());
    on('new', () => {
      if (!window.confirm('Start a new scene? Recorded paths not saved to a file will be lost.')) return;
      this.sim.set_scene(FlowSim.default_scene());
      this.reloadFromSim();
      this.toast('New scene: an empty tank');
    });
    if (!this.options.files) return;
    const file = Object.assign(document.createElement('input'), { type: 'file', accept: '.json,application/json', hidden: true });
    bar.append(file);
    on('open', () => file.click());
    file.addEventListener('change', async () => {
      const f = file.files?.[0];
      file.value = '';
      if (!f) return;
      try {
        this.sim.set_scene(await f.text());
        this.reloadFromSim();
        this.toast(`Opened ${f.name}`);
      } catch (e) {
        this.toast(`Couldn't open ${f.name}: ${e}`, 5000);
      }
    });
    on('save', () => download('flow-scene.json', this.sim.scene(), 'application/json'));
    on('export', () => this.exporter?.open());
  }

  private toggleRun(): void {
    if (this.sim.frozen()) {
      this.sim.unfreeze();
      this.toast('Running');
    } else {
      this.sim.freeze();
      this.toast('Frozen: seeds trace their paths through the still flow');
    }
    this.updateButtons();
    this.dirty = true;
    this.options.onRunChange?.(this.sim.frozen());
  }

  private bake(): void {
    if (!this.options.files) return;
    this.history.recordPaths();
    const n = this.sim.bake();
    this.toast(n ? `Kept ${n} path${n === 1 ? '' : 's'}` : 'Nothing to keep: freeze the flow and place seeds first');
  }

  private updateButtons(): void {
    const frozen = this.sim.frozen();
    const run = this.buttons['run'];
    if (run) run.textContent = frozen ? 'Run' : 'Freeze';
    if (this.buttons['bake']) this.buttons['bake'].disabled = !frozen;
    if (this.buttons['undo']) this.buttons['undo'].disabled = !this.history?.canUndo;
    if (this.buttons['redo']) this.buttons['redo'].disabled = !this.history?.canRedo;
  }

  private onKey = (e: KeyboardEvent): void => {
    const target = e.target as HTMLElement;
    if (target.closest?.('input, select, textarea, dialog, [contenteditable]')) return;
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      if (e.shiftKey) this.history.redo();
      else this.history.undo();
      return;
    }
    if (mod && e.key.toLowerCase() === 'y') {
      e.preventDefault();
      this.history.redo();
      return;
    }
    if (mod) return;
    if (e.key === ' ') {
      e.preventDefault();
      this.toggleRun();
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      this.deleteSelection();
    } else if (e.key === 'Escape') {
      this.selection = null;
      this.selectedPath = null;
      this.setTool('select');
      this.rebuildPanel();
    } else if (e.key.toLowerCase() === 'b') {
      this.bake();
    } else {
      const t = TOOLS.find((t) => t.key === e.key.toLowerCase());
      if (t) this.setTool(t.name);
    }
  };

  // --- the canvas ---

  private topHit(p: Vec): Obj | null {
    const px = this.renderer.px();
    // Small things over big: seeds and forces before obstacles.
    const order = [...this.state.objects].reverse();
    const rank = (o: Obj) => (o.kind === 'pebble' || o.kind === 'rect' || o.kind === 'wall' ? 1 : 0);
    order.sort((a, b) => rank(a) - rank(b));
    return order.find((o) => hits(o, p, px)) ?? null;
  }

  private wireCanvas(): void {
    const c = this.canvas;
    c.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      // So its keys work, when they're listened for round it.
      c.focus({ preventScroll: true });
      c.setPointerCapture(e.pointerId);
      const p = this.renderer.toDomain(e.clientX, e.clientY);
      this.pointer = p;
      this.pointerDown(p);
      this.dirty = true;
    });
    c.addEventListener('pointermove', (e) => {
      const p = this.renderer.toDomain(e.clientX, e.clientY);
      this.pointer = p;
      this.pointerMove(p);
      this.dirty = true;
    });
    const up = (e: PointerEvent) => {
      // The last move may not have arrived before the button came up.
      if (this.drag) this.pointerMove(this.renderer.toDomain(e.clientX, e.clientY));
      this.pointerUp();
      this.dirty = true;
    };
    c.addEventListener('pointerup', up);
    c.addEventListener('pointercancel', up);
    c.addEventListener('pointerleave', () => {
      if (!this.drag) {
        this.pointer = null;
        this.hover = null;
        this.dirty = true;
      }
    });
  }

  private pointerDown(p: Vec): void {
    const px = this.renderer.px();
    const tool = this.tool;
    if (tool === 'select') {
      const sel = this.selected();
      const handle = sel && handles(sel, px).find((h) => dist(h.pos, p) <= 8 * px);
      if (sel && handle) {
        this.history.recordScene();
        this.drag = { type: 'handle', obj: sel, id: handle.id };
        this.hold(sel);
        return;
      }
      const hit = this.topHit(p);
      if (hit) {
        this.selection = hit.id;
        this.selectedPath = null;
        this.history.recordScene();
        this.drag = { type: 'move', obj: hit, last: p };
        this.hold(hit);
        this.canvas.classList.add('moving');
      } else {
        this.selection = null;
        this.selectedPath = this.recorded ? nearestPath(this.recorded.points, this.recorded.offsets, p, 6 * px) : null;
      }
      this.rebuildPanel();
    } else if (tool === 'delete') {
      const hit = this.topHit(p);
      if (hit) {
        this.selection = hit.id;
        this.selectedPath = null;
      } else {
        this.selection = null;
        this.selectedPath = this.recorded ? nearestPath(this.recorded.points, this.recorded.offsets, p, 6 * px) : null;
      }
      this.deleteSelection();
    } else if (tool === 'stir') {
      this.drag = { type: 'stir', last: p, time: performance.now() };
    } else if (tool === 'drop') {
      if (!this.sim.drop_particle(p[0], p[1])) this.toast('Particles start in the fluid, inside the frame');
      this.drag = { type: 'drop', last: p };
    } else if (places(tool)) {
      const obj = create(tool, this.nextId(), p);
      if (!obj) return;
      this.history.recordScene();
      this.state.objects.push(obj);
      this.selection = obj.id;
      this.selectedPath = null;
      this.drag = { type: 'create', obj, p0: p, moved: false };
      this.hold(obj);
      this.pushObjects();
      this.rebuildPanel();
    }
  }

  private pointerMove(p: Vec): void {
    const px = this.renderer.px();
    const d = this.drag;
    if (!d) {
      if (this.tool === 'select' || this.tool === 'delete') {
        const hit = this.topHit(p);
        const id = hit?.id ?? null;
        if (id !== this.hover) this.hover = id;
      }
      return;
    }
    switch (d.type) {
      case 'create':
        if (!d.moved && dist(d.p0, p) > 4 * px) d.moved = true;
        if (d.moved) {
          shape(d.obj, d.p0, p, px);
          this.pushObjects();
        }
        break;
      case 'move':
        moveBy(d.obj, p[0] - d.last[0], p[1] - d.last[1]);
        d.last = p;
        this.pushObjects();
        break;
      case 'handle':
        dragHandle(d.obj, d.id, p);
        this.pushObjects();
        break;
      case 'stir': {
        const now = performance.now();
        const dt = Math.max((now - d.time) / 1000, 1 / 240);
        let vx = (p[0] - d.last[0]) / dt;
        let vy = (p[1] - d.last[1]) / dt;
        const speed = Math.hypot(vx, vy);
        const most = 1200;
        if (speed > most) {
          vx *= most / speed;
          vy *= most / speed;
        }
        this.sim.stir(p[0], p[1], vx, vy, this.view.brush);
        d.last = p;
        d.time = now;
        break;
      }
      case 'drop':
        if (dist(d.last, p) >= 14 * px) {
          this.sim.drop_particle(p[0], p[1]);
          d.last = p;
        }
        break;
    }
  }

  /** Keeps a mover still while it's dragged. */
  private hold(o: Obj | null): void {
    this.sim.hold(o && moving(o) ? o.id : undefined);
  }

  private pointerUp(): void {
    const d = this.drag;
    this.drag = null;
    this.canvas.classList.remove('moving');
    this.hold(null);
    if (!d) return;
    if (d.type === 'create' && d.obj.kind === 'wall' && d.obj.points.length < 2) {
      const [x, y] = d.p0;
      d.obj.points = [
        [x - 60, y],
        [x + 60, y],
      ];
      this.pushObjects();
    }
    if (d.type === 'create' || d.type === 'move' || d.type === 'handle') this.rebuildPanel();
  }

  // --- each frame ---

  /** Runs `n` frames at once, for checking the page while it's hidden. */
  advance(n: number): string {
    for (let i = 0; i < n; i++) {
      this.sim.tick(this.state.flow.speed / 60);
      this.sync();
    }
    this.renderer.fit([this.state.flow.width, this.state.flow.height]);
    this.draw();
    this.updateStatus();
    return this.statusEl.textContent ?? '';
  }

  private frame = (): void => {
    if (this.gone) return;
    const t0 = performance.now();
    this.sim.tick(this.state.flow.speed / 60);
    this.stepMs = this.stepMs * 0.9 + (performance.now() - t0) * 0.1;
    this.sync();
    if (this.renderer.fit([this.state.flow.width, this.state.flow.height])) this.dirty = true;
    if (this.dirty) {
      this.dirty = false;
      this.draw();
    }
    if (t0 - this.lastStatus > 400) {
      this.lastStatus = t0;
      this.updateStatus();
    }
    this.frameId = requestAnimationFrame(this.frame);
  };

  private sync(): void {
    const sim = this.sim;
    const fv = sim.field_version();
    if (fv !== this.versions.field || this.fieldStale) {
      const changed = fv !== this.versions.field;
      this.versions.field = fv;
      this.fieldStale = false;
      this.updateField();
      this.dirty = true;
      if (changed) this.options.onFieldChange?.();
    }
    const pv = sim.paths_version();
    if (pv !== this.versions.paths) {
      this.versions.paths = pv;
      const set = sim.recorded_paths();
      this.recorded = bundle({ points: set.points, offsets: set.offsets, closed: set.closed });
      set.free();
      if (this.selectedPath !== null && this.selectedPath + 1 >= this.recorded.offsets.length) {
        this.selectedPath = null;
        this.rebuildPanel();
      }
      this.dirty = true;
      this.scheduleSave(4000);
    }
    const ov = sim.objects_version();
    if (ov !== this.versions.objects) {
      this.versions.objects = ov;
      this.syncMovers();
    }
    const lv = sim.live_version();
    if (lv !== this.versions.live) {
      this.versions.live = lv;
      const set = sim.live_paths();
      this.live = bundle({ points: set.points, offsets: set.offsets, closed: set.closed });
      set.free();
      this.dirty = true;
    }
    const n = sim.particle_count() + sim.mover_count();
    if (n > 0 || this.trails) {
      if (n === 0) {
        this.trails = null;
        this.heads = null;
      } else {
        const set = sim.particle_trails(n <= 40 ? 1_000_000 : 60);
        this.trails = bundle({ points: set.points, offsets: set.offsets, closed: set.closed });
        set.free();
        this.heads = sim.particle_positions();
      }
      this.dirty = true;
    }
  }

  /** Moves the moving objects to where the engine has them, and drops
   * those that have left. */
  private syncMovers(): void {
    const data = this.sim.movers();
    const at = new Map<number, number>();
    for (let i = 0; i < data.length; i += 3) at.set(data[i], i);
    let gone = false;
    this.state.objects = this.state.objects.filter((o) => {
      if (!moving(o) || !('pos' in o)) return true;
      const i = at.get(o.id);
      if (i === undefined) {
        gone = true;
        return false;
      }
      // In place: a drag holds on to the object.
      o.pos[0] = data[i + 1];
      o.pos[1] = data[i + 2];
      return true;
    });
    if (gone && this.selection !== null && !this.selected()) {
      this.selection = null;
      this.rebuildPanel();
    }
    this.dirty = true;
    this.scheduleSave(4000);
  }

  private updateField(): void {
    const w = this.sim.grid_width();
    const h = this.sim.grid_height();
    if (this.field.width !== w || this.field.height !== h) {
      this.field.width = w;
      this.field.height = h;
    }
    const rgba = this.sim.render(this.view.mode);
    const image = new ImageData(new Uint8ClampedArray(rgba.buffer as ArrayBuffer, rgba.byteOffset, rgba.byteLength), w, h);
    this.fieldCtx.putImageData(image, 0, 0);
    if (this.view.arrows) {
      const cols = 64;
      const rows = Math.max(4, Math.round((cols * this.state.flow.height) / this.state.flow.width));
      this.arrows = { cols, rows, data: this.sim.velocities(cols, rows), reference: Math.max(this.state.flow.inflow_speed, 40) };
    } else {
      this.arrows = null;
    }
  }

  private draw(): void {
    const brush =
      this.tool === 'stir' && this.pointer ? { pos: this.pointer, radius: this.view.brush } : null;
    this.renderer.draw({
      domain: [this.state.flow.width, this.state.flow.height],
      field: this.field,
      backdrop: this.backdrop,
      objects: this.state.objects,
      selection: this.selection,
      hover: this.hover,
      selectedPath: this.selectedPath,
      recorded: this.recorded,
      live: this.live,
      trails: this.trails,
      heads: this.heads,
      arrows: this.arrows,
      show: {
        recorded: this.view.recorded,
        live: this.view.live,
        particles: this.view.particles,
        objects: this.view.objects,
      },
      brush,
    });
  }

  private updateStatus(): void {
    const s = JSON.parse(this.sim.stats()) as Stats;
    const parts = [
      `<b>${s.frozen ? 'Frozen' : 'Running'}</b>`,
      `grid ${s.grid[0]} × ${s.grid[1]}`,
      `${this.stepMs.toFixed(1)} ms a frame`,
      `${s.particles} particle${s.particles === 1 ? '' : 's'} moving`,
      ...(s.movers ? [`${s.movers} moving force${s.movers === 1 ? '' : 's'}`] : []),
      `${s.recorded_paths} recorded path${s.recorded_paths === 1 ? '' : 's'} (${s.recorded_points} points)`,
    ];
    if (s.frozen) {
      parts.push(`${s.live_paths} live path${s.live_paths === 1 ? '' : 's'}`);
      const names: Record<string, string> = {
        left_domain: 'left',
        hit_obstacle: 'hit obstacles',
        captured: 'caught',
        stalled: 'stalled',
        closed: 'closed',
        max_length: 'hit max length',
      };
      const ends = Object.entries(s.ends)
        .map(([k, n]) => `${n} ${names[k] ?? k}`)
        .join(', ');
      if (ends) parts.push(ends);
      parts.push(`divergence ${s.divergence.toExponential(1)}`);
    }
    this.statusEl.innerHTML = parts.join(' · ');
  }

  private rebuildPanel(): void {
    buildPanel(this.panelRoot, this, this.panelOpen);
    save(PANEL_KEY, this.panelOpen);
  }

  toast(message: string, ms = 2200): void {
    this.toastEl.textContent = message;
    this.toastEl.classList.add('show');
    clearTimeout(this.toastTimer);
    this.toastTimer = window.setTimeout(() => this.toastEl.classList.remove('show'), ms);
  }
}
