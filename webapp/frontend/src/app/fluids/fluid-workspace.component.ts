import { Component, ElementRef, computed, effect, inject, signal, untracked, viewChild, type OnDestroy } from '@angular/core';
import init, { FlowSim } from '@flowsim/wasm-pkg/flowsim.js';
import { FlowSimView } from '@flowsim/src/simview';
import { entryPath, nameOf } from '../journal/journal-files';
import { PaneTabDirective, ThreePaneComponent, type PaneTab } from '../layout/three-pane.component';
import { OptionsFormComponent } from '../resources/options-form.component';
import type { Box, FluidPrepared, FluidResult, JournalPayload, RawField } from '../scribe';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { RenderService } from '../services/render.service';
import { ShellService } from '../shell/shell.service';
import { FluidEngine } from './fluid-engine.service';
import { EFFECTS, EFFECT_FIELDS, EFFECT_INFO, fluidSettings, fluidText, parseFluid, type EffectName, type FluidDoc } from './fluid-file';

/** Where the flow simulator's wasm is served (see angular.json's assets). */
const flowsimWasm = () => new URL('wasm/flowsim_bg.wasm', document.baseURI).href;
let flowsimReady: Promise<unknown> | null = null;

/** Mask cells across the box's longer side. */
const MASK_CELLS = 400;
/** The least time between distorting the page while the fluid runs. */
const APPLY_EVERY_MS = 80;
/** The most cells across the field the page is distorted by while the
 * fluid runs, for speed; frozen, it's distorted by the whole field. */
const LIVE_CELLS = 64;
/** Cells across a new fluid's simulator: few, for speed. */
const NEW_RESOLUTION = 64;

/** `field` averaged down to at most `cells` cells across (the few past a
 * whole number of coarse cells dropped), or itself if it's no wider. */
function coarsened(field: RawField, cells: number): RawField {
  const k = Math.ceil(field.nx / cells);
  if (k <= 1) return field;
  const { nx, ny } = field;
  const cx = Math.max(2, Math.floor(nx / k));
  const cy = Math.max(2, Math.floor(ny / k));
  const share = 1 / (k * k);
  const pressure = new Float32Array(cx * cy);
  const dye = new Float32Array(cx * cy);
  for (let j = 0; j < cy; j++) {
    for (let i = 0; i < cx; i++) {
      let p = 0;
      let d = 0;
      for (let b = 0; b < k; b++) {
        const row = (j * k + b) * nx + i * k;
        for (let a = 0; a < k; a++) {
          p += field.pressure[row + a];
          d += field.dye[row + a];
        }
      }
      pressure[i + j * cx] = p * share;
      dye[i + j * cx] = d * share;
    }
  }
  // Faces: the mean of the fine faces along each coarse one.
  const u = new Float32Array((cx + 1) * cy);
  for (let j = 0; j < cy; j++) {
    for (let i = 0; i <= cx; i++) {
      let sum = 0;
      for (let b = 0; b < k; b++) sum += field.u[i * k + (j * k + b) * (nx + 1)];
      u[i + j * (cx + 1)] = sum / k;
    }
  }
  const v = new Float32Array(cx * (cy + 1));
  for (let j = 0; j <= cy; j++) {
    for (let i = 0; i < cx; i++) {
      let sum = 0;
      for (let a = 0; a < k; a++) sum += field.v[i * k + a + j * k * nx];
      v[i + j * cx] = sum / k;
    }
  }
  return { nx: cx, ny: cy, h: field.h * k, u, v, pressure, dye };
}

/** The simulator's default scene, at a new fluid's resolution. */
function newScene(): string {
  const scene = JSON.parse(FlowSim.default_scene());
  scene.flow.resolution = NEW_RESOLUTION;
  return JSON.stringify(scene);
}
/** The zoom buttons' steps, times closer than fitting. */
const ZOOMS = [1, 1.5, 2, 3, 4, 6, 8, 12, 16];
const MAX_ZOOM = 16;

/** How far a wheel event scrolls along an axis, in pixels. */
function wheelPixels(event: WheelEvent, axis: 'x' | 'y'): number {
  const delta = axis === 'x' ? event.deltaX : event.deltaY;
  return event.deltaMode === WheelEvent.DOM_DELTA_LINE ? delta * 16 : event.deltaMode === WheelEvent.DOM_DELTA_PAGE ? delta * 400 : delta;
}

function sameFiles(a: JournalPayload, b: JournalPayload): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((key) => a[key] === b[key]);
}

/** A URL for `svg`, or null. */
function svgUrl(svg: string | null | undefined): string | null {
  return svg ? URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })) : null;
}

/**
 * A fluid file: the flow simulator run over a page of an entry, and the
 * page distorted by the fluid, side by side, live as the fluid changes,
 * and zoomed together. The left tab sets the page, the box the fluid lies over, the
 * canvases it flows round and its effects; the right pane is the
 * simulator's settings. The field, scene and settings are kept in the file.
 */
@Component({
  selector: 'app-fluid-workspace',
  imports: [ThreePaneComponent, PaneTabDirective, OptionsFormComponent],
  providers: [FluidEngine],
  template: `
    <app-three-pane key="fluids" [tabs]="tabs" [rightFraction]="0.22" [rightMin]="280">
      <aside appPaneTab="fluid" class="settings" aria-label="Fluid settings">
        @if (doc(); as d) {
          <section>
            <h3>The page</h3>
            <label>Entry
              <select (change)="setEntry($any($event.target).value)">
                @for (e of entries(); track e) { <option [selected]="e === entry()">{{ e }}</option> }
              </select>
            </label>
            <label>Page
              <select (change)="setPage(+$any($event.target).value)">
                @for (p of prepared()?.pages ?? []; track p.index) { <option [value]="p.index" [selected]="p.index === d.preview.page">{{ p.label }}</option> }
              </select>
            </label>
            @if (uses(); as u) {
              <p class="hint">The entry adds it {{ u }}; the live fluid stands in for it there.</p>
            } @else {
              <label>Add it to
                <select (change)="setTarget($any($event.target).value || null)">
                  <option value="" [selected]="d.preview.target === null">the whole page</option>
                  @for (c of prepared()?.canvases ?? []; track c.label) { <option [value]="c.label" [selected]="c.label === d.preview.target">canvas {{ c.label }}</option> }
                </select>
              </label>
            }
          </section>
          <section>
            <h3>Its box</h3>
            <div class="pairs">
              <span>at</span>
              <input type="number" step="any" [value]="d.at?.[0] ?? 0" (change)="setBox('at', 0, $any($event.target).value)" aria-label="x" />
              <input type="number" step="any" [value]="d.at?.[1] ?? 0" (change)="setBox('at', 1, $any($event.target).value)" aria-label="y" />
              <span>size</span>
              <input type="number" step="any" min="1" [value]="d.size?.[0] ?? ''" (change)="setBox('size', 0, $any($event.target).value)" aria-label="width" />
              <input type="number" step="any" min="1" [value]="d.size?.[1] ?? ''" (change)="setBox('size', 1, $any($event.target).value)" aria-label="height" />
            </div>
            <div class="buttons">
              <button type="button" (click)="fit(null)" title="The box round the whole page">Fit the page</button>
              @if (d.preview.target; as t) { <button type="button" (click)="fit(t)" title="The box round the canvas it's added to">Fit canvas {{ t }}</button> }
            </div>
            <p class="hint">In canvas units. Changing its size starts the fluid afresh.</p>
          </section>
          <section>
            <h3>Obstacles</h3>
            <p class="hint">The fluid flows round the ink of the canvases ticked.</p>
            @for (c of prepared()?.canvases ?? []; track c.label) {
              <label class="check"><input type="checkbox" [checked]="d.obstacles.canvases.includes(c.label)" (change)="setObstacle(c.label, $any($event.target).checked)" /> canvas {{ c.label }}</label>
            }
            <label>Grown by <input type="number" min="0" step="0.5" [value]="d.obstacles.grow" (change)="setGrow(+$any($event.target).value)" /></label>
            @if (maskNote(); as note) { <p class="hint">{{ note }}</p> }
          </section>
          <section>
            <h3>Effects</h3>
            @for (e of effectNames; track e) {
              <div class="effect">
                <label class="check"><input type="checkbox" [checked]="!!d.effects[e]" (change)="setEffectOn(e, $any($event.target).checked)" /> <b>{{ info[e].label }}</b> <code>{{ e }}</code></label>
                @if (d.effects[e]; as values) {
                  <p class="hint">{{ info[e].hint }}</p>
                  <app-options-form [fields]="fields[e]" [values]="values" (changed)="setEffect(e, $event.name, $event.value)" />
                }
              </div>
            }
          </section>
          <section>
            <h3>In the entry</h3>
            <pre>{{ snippet() }}</pre>
          </section>
        }
      </aside>
      <section center class="centre">
        <div class="top">
          <div #bar class="simbar"></div>
          <span class="zoombar" title="Ctrl or ⌘ and scroll (or pinch) over either view to zoom there; scroll, or drag the result, to move about">
            <button type="button" (click)="zoomBy(-1)" [disabled]="zoom() <= 1" title="Zoom out" aria-label="Zoom out">−</button>
            <button type="button" class="zoom" (click)="fitView()" title="Fit both views">{{ zoom() === 1 && !focus() ? 'Fit' : percent() }}</button>
            <button type="button" (click)="zoomBy(1)" [disabled]="zoom() >= maxZoom" title="Zoom in" aria-label="Zoom in">+</button>
          </span>
          <label class="check"><input type="checkbox" [checked]="showResult()" (change)="showResult.set($any($event.target).checked)" /> Result</label>
        </div>
        <div class="views">
          <div class="view fluid">
            <nav #rail></nav>
            <div #stage class="stage"></div>
          </div>
          @if (showResult()) {
            <figure class="view">
              <figcaption>Result @if (preparing()) { <em>running the entry…</em> }
                <span class="switch">
                  <button type="button" [class.on]="mode() === 'svg'" (click)="mode.set('svg')">SVG</button>
                  <button type="button" [class.on]="mode() === 'darkly'" (click)="mode.set('darkly')">Darkly</button>
                </span>
                @if (mode() === 'darkly') { <button type="button" (click)="renderDarkly()" [disabled]="darkly().busy">Render</button> }
              </figcaption>
              <div #pic class="pic" [class.panning]="panning()" (wheel)="wheelResult($event)" (pointerdown)="panStart($event)"
                   (pointermove)="panMove($event)" (pointerup)="panEnd($event)" (pointercancel)="panEnd($event)">
                @let place = picPlace();
                @if (problem(); as p) { <p class="error">{{ p }}</p> }
                @else if (mode() === 'svg') {
                  @if (result().error; as e) { <p class="error">{{ e }}</p> }
                  @else if (result().url && place) {
                    <img [src]="result().url" alt="The page distorted by the fluid" draggable="false"
                         [style.left.px]="place.left" [style.top.px]="place.top" [style.width.px]="place.width" />
                  }
                  @else { <p class="hint">Nothing drawn yet.</p> }
                } @else {
                  @if (darkly().error; as e) { <p class="error">{{ e }}</p> }
                  @if (darkly().busy) { <p class="hint">Rendering… {{ darkly().progress }}%</p> }
                  @if (darkly().url && place) {
                    <img [src]="darkly().url" alt="The page distorted by the fluid, rendered with Darkly" draggable="false"
                         [style.left.px]="place.left" [style.top.px]="place.top" [style.width.px]="place.width" />
                  }
                  @else if (!darkly().busy && !darkly().error) { <p class="hint">Render draws the page as journalbuild does, with the fluid as it is now.</p> }
                }
              </div>
            </figure>
          }
        </div>
        <div #status></div>
      </section>
      <aside right #panel></aside>
    </app-three-pane>
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .settings { overflow-y: auto; padding: 4px 10px 16px; background: var(--surface); font-size: 12px; }
    section { border-bottom: 1px solid var(--border); padding: 8px 0; display: flex; flex-direction: column; gap: 6px; }
    h3 { margin: 0; font-size: 12px; }
    label { display: flex; flex-direction: column; gap: 2px; }
    label.check { flex-direction: row; align-items: center; gap: 6px; }
    .pairs { display: grid; grid-template-columns: auto 1fr 1fr; gap: 4px 6px; align-items: center; }
    .pairs input { min-width: 0; }
    .buttons { display: flex; gap: 6px; flex-wrap: wrap; }
    .effect { display: flex; flex-direction: column; gap: 4px; padding: 4px 0; }
    code, pre { font-family: var(--mono); font-size: 11px; }
    pre { margin: 0; white-space: pre-wrap; background: var(--stage-bg); color: #f4efe6; padding: 6px 8px; border-radius: 4px; }
    .hint { margin: 0; color: var(--muted); }
    .error { margin: 0; color: var(--danger); white-space: pre-wrap; }
    .centre { display: flex; flex-direction: column; min-height: 0; min-width: 0; }
    .top { display: flex; align-items: center; gap: 10px; border-bottom: 1px solid var(--border); background: var(--surface); padding-right: 10px; }
    .simbar { flex: 1; min-width: 0; border-bottom: 0 !important; }
    .views { flex: 1; min-height: 0; display: flex; gap: 2px; background: var(--border); }
    .view { flex: 1 1 0; min-width: 0; margin: 0; display: flex; flex-direction: column; background: var(--stage-bg); }
    /* The stage as wide as the result, beside its tool rail. */
    .view.fluid { flex: 1 1 70px; flex-direction: row; }
    .view.fluid nav { flex: none; width: 70px; position: relative; }
    .stage { flex: 1; position: relative; }
    aside[right] { position: relative; }
    figcaption { display: flex; align-items: center; gap: 8px; padding: 4px 8px; background: var(--surface); font-size: 12px; font-weight: 600; }
    figcaption em { font-weight: normal; color: var(--muted); }
    .switch { display: inline-flex; }
    .switch button { border-radius: 0; padding: 2px 8px; }
    .switch button.on { background: var(--accent); color: white; }
    .pic { flex: 1; min-height: 0; position: relative; overflow: hidden; padding: 8px; display: flex; flex-direction: column; justify-content: center; gap: 6px; cursor: grab; touch-action: none; }
    .pic.panning { cursor: grabbing; }
    .pic p { position: relative; z-index: 1; }
    .pic img { position: absolute; display: block; max-width: none; height: auto; background: #fbfaf7; box-shadow: 0 1px 6px rgb(0 0 0 / 0.25); user-select: none; }
    .zoombar { display: inline-flex; align-items: center; gap: 2px; }
    .zoombar button { padding: 2px 8px; }
    .zoombar .zoom { min-width: 52px; }
  `,
})
export class FluidWorkspaceComponent implements OnDestroy {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly shell = inject(ShellService);
  private readonly render = inject(RenderService);
  private readonly engine = inject(FluidEngine);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);

  private readonly barEl = viewChild<ElementRef<HTMLElement>>('bar');
  private readonly railEl = viewChild<ElementRef<HTMLElement>>('rail');
  private readonly stageEl = viewChild<ElementRef<HTMLElement>>('stage');
  private readonly panelEl = viewChild<ElementRef<HTMLElement>>('panel');
  private readonly statusEl = viewChild<ElementRef<HTMLElement>>('status');
  private readonly picEl = viewChild<ElementRef<HTMLElement>>('pic');

  protected readonly tabs: PaneTab[] = [{ id: 'fluid', label: 'Fluid', title: 'The page, the box, obstacles and effects' }];
  protected readonly effectNames = EFFECTS;
  protected readonly fields = EFFECT_FIELDS;
  protected readonly info = EFFECT_INFO;

  /** The fluid file shown, which stays while another kind's file shows. */
  protected readonly path = this.shell.lastOpened(['fluids']);
  protected readonly name = computed(() => nameOf(this.path() ?? ''));
  /** Its settings as the tab has them. */
  protected readonly doc = signal<FluidDoc | null>(null);
  protected readonly prepared = signal<FluidPrepared | null>(null);
  protected readonly preparing = signal(false);
  protected readonly result = signal<{ url: string | null; error: string | null; view: Box | null }>({ url: null, error: null, view: null });
  protected readonly darkly = signal<{ url: string | null; error: string | null; busy: boolean; progress: number }>({ url: null, error: null, busy: false, progress: 0 });
  protected readonly mode = signal<'svg' | 'darkly'>('svg');
  protected readonly showResult = signal(true);
  protected readonly maskNote = signal<string | null>(null);
  /** How many times closer than fitting, in both views; the canvas point in their middle, or null for each's own. */
  protected readonly zoom = signal(1);
  protected readonly focus = signal<[number, number] | null>(null);
  protected readonly maxZoom = MAX_ZOOM;
  protected readonly percent = computed(() => `${Math.round(this.zoom() * 100)}%`);
  private readonly picSize = signal({ width: 0, height: 0 });
  /** What the result shows of the canvas: the page's view. */
  private readonly pageView = computed(() => this.result().view ?? this.prepared()?.view ?? null);
  /** The result's pixels to a canvas unit. */
  private readonly resultScale = computed(() => {
    const view = this.pageView();
    const { width, height } = this.picSize();
    if (!view || !width) return 1;
    // As the simulator fits its domain, 16 pixels in from each side, so the two match.
    return Math.max(0.01, Math.min((width - 32) / view[2], (height - 32) / view[3])) * this.zoom();
  });

  protected readonly entries = computed(() => this.store.manifest().entries);
  protected readonly entry = computed(() => {
    const chosen = this.doc()?.preview.entry;
    return chosen && this.entries().includes(chosen) ? chosen : (this.entries()[0] ?? null);
  });
  private readonly entrySource = computed(() => {
    const entry = this.entry();
    return entry ? (this.store.files()[entryPath(entry)] ?? null) : null;
  });
  /** The journal's files besides the entries and this fluid: when they change, the page is made again. */
  private readonly otherFiles = computed(() => {
    const own = this.path();
    return Object.fromEntries(Object.entries(this.store.resources()).filter(([path]) => path !== own));
  }, { equal: sameFiles });
  private readonly page = computed(() => this.doc()?.preview.page ?? 0);

  protected readonly problem = computed(() => {
    const p = this.prepared();
    if (!this.entry()) return 'The journal has no entries: add one to shape a fluid over its page.';
    return p?.error ? p.error.message : null;
  });
  /** Where the entry adds the fluid, said; null if it doesn't. */
  protected readonly uses = computed(() => {
    const u = this.prepared()?.uses;
    if (!u || (!u.page && !u.canvases.length)) return null;
    const places = [...(u.page ? ['to the page'] : []), ...u.canvases.map((c) => `to canvas ${c}`)];
    return places.join(' and ');
  });
  protected readonly snippet = computed(() => {
    const call = `fluid(${JSON.stringify(this.name())})`;
    if (this.uses()) return `The entry adds it already:\n${call}`;
    const target = this.doc()?.preview.target;
    return target === null || target === undefined
      ? `page.add(${call});`
      : `// on the canvas named ${JSON.stringify(target)}\ncanvas.add(${call});`;
  });

  private view: FlowSimView | null = null;
  /** The simulator's parts, made here and put in the template's places, so a template drawn again (as HMR does) keeps them. */
  private readonly parts = {
    bar: document.createElement('div'),
    rail: document.createElement('nav'),
    stage: document.createElement('div'),
    panel: document.createElement('div'),
    status: document.createElement('div'),
  };
  private readonly viewReady = signal(false);
  /** The file's text as last written or read here, so the tab's own writes aren't read back. */
  private known: string | null = null;
  /** The field as last kept in the file, and the simulator's field version it was kept at. */
  private keptField: unknown = null;
  private keptVersion = -1;
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private prepareTimer: ReturnType<typeof setTimeout> | null = null;
  private prepareGeneration = 0;
  private maskKey = '';
  private applying = false;
  private applyWanted = false;
  private lastApply = 0;
  private darklyRun = 0;

  constructor() {
    // A file opened, or changed elsewhere (an import, say).
    effect(() => {
      const path = this.path();
      const text = path ? (this.store.files()[path] ?? this.library.files()[path] ?? null) : null;
      if (path && text !== this.known) untracked(() => this.open(text));
    });
    // The page made again when what makes it changes.
    effect(() => {
      this.entrySource();
      this.page();
      this.name();
      this.otherFiles();
      this.engine.restarts();
      if (this.viewReady()) untracked(() => this.schedulePrepare());
    });
    // The simulator's parts in their places, whenever the places are drawn.
    effect(() => {
      const places = { bar: this.barEl(), rail: this.railEl(), stage: this.stageEl(), panel: this.panelEl(), status: this.statusEl() };
      for (const [name, place] of Object.entries(places) as [keyof typeof places, ElementRef<HTMLElement> | undefined][]) {
        const part = this.parts[name];
        if (place && part.parentElement !== place.nativeElement) place.nativeElement.replaceChildren(part);
      }
    });
    for (const part of [this.parts.rail, this.parts.stage, this.parts.panel]) part.style.cssText = 'position: absolute; inset: 0;';
    // Both views looking at the same place.
    effect(() => {
      const zoom = this.zoom();
      const focus = this.focus();
      const at = this.doc()?.at ?? [0, 0];
      if (this.viewReady()) this.view?.setZoom(zoom, focus ? [focus[0] - at[0], focus[1] - at[1]] : null);
    });
    // The result's size followed, to place the page in it.
    effect((cleanup) => {
      const pic = this.picEl()?.nativeElement;
      if (!pic) return;
      const observer = new ResizeObserver(([entry]) => this.picSize.set({ width: entry.contentRect.width + 16, height: entry.contentRect.height + 16 }));
      observer.observe(pic);
      cleanup(() => observer.disconnect());
    });
    this.parts.stage.addEventListener('wheel', this.wheelStage, { passive: false });
    void this.startView();
  }

  ngOnDestroy() {
    this.parts.stage.removeEventListener('wheel', this.wheelStage);
    for (const timer of [this.saveTimer, this.prepareTimer]) if (timer) clearTimeout(timer);
    this.save();
    this.view?.destroy();
    this.view?.sim.free();
    this.view = null;
    this.setResult(null, null);
    this.setDarkly({ url: null });
  }

  // ---- the simulator ----

  private async startView() {
    flowsimReady ??= init({ module_or_path: flowsimWasm() });
    try {
      await flowsimReady;
    } catch (err) {
      this.result.set({ url: null, error: `The flow simulator didn't load: ${String(err)}`, view: null });
      return;
    }
    const sim = new FlowSim('');
    this.view = new FlowSimView(sim, { ...this.parts, keys: this.host.nativeElement }, {
      onFieldChange: () => this.fieldChanged(),
      onSceneChange: () => this.scheduleSave(400),
      onRunChange: (frozen) => frozen && this.scheduleSave(0),
    });
    if (this.doc()) this.loadIntoSim();
    this.viewReady.set(true);
  }

  /** Puts the file's scene and field into the simulator. */
  private loadIntoSim() {
    const view = this.view;
    const d = this.doc();
    if (!view || !d) return;
    view.loadScene(d.scene ? JSON.stringify(d.scene) : newScene());
    if (d.size) view.setDomain(d.size[0], d.size[1]);
    if (d.field) {
      try {
        view.loadField(JSON.stringify(d.field));
      } catch (err) {
        this.result.set({ url: null, error: `The kept field didn't load (${String(err)}); run the fluid and freeze it again.`, view: null });
      }
    }
    this.keptVersion = view.sim.field_version();
    this.maskKey = '';
    // A file with no field yet gets one, so entries can read it.
    if (!d.field) this.scheduleSave(0);
  }

  private fieldChanged() {
    if (this.view?.frozen) this.scheduleSave(800);
    this.requestApply();
  }

  // ---- the file ----

  private open(text: string | null) {
    this.known = text;
    const d = parseFluid(text);
    this.doc.set(d);
    this.keptField = d.field;
    this.prepared.set(null);
    this.setResult(null, null);
    this.setDarkly({ url: null, error: null });
    this.loadIntoSim();
  }

  /** The file's text now: the field as frozen (or as last kept while it runs) and the scene. */
  private text(): string | null {
    const d = this.doc();
    const view = this.view;
    if (!d || !view) return null;
    const version = view.sim.field_version();
    if ((view.frozen || !this.keptField) && version !== this.keptVersion) {
      this.keptField = JSON.parse(view.sim.field_json());
      this.keptVersion = version;
    }
    return fluidText({ ...d, field: this.keptField ?? JSON.parse(view.sim.field_json()), scene: JSON.parse(view.sim.scene()) });
  }

  private scheduleSave(delay: number) {
    if (this.saveTimer) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      this.save();
    }, delay);
  }

  private save() {
    const path = this.path();
    const text = this.text();
    if (!path || text === null || text === this.known) return;
    this.known = text;
    this.store.setFile(path, text);
  }

  private change(f: (d: FluidDoc) => FluidDoc) {
    const d = this.doc();
    if (!d) return;
    this.doc.set(f(d));
    this.scheduleSave(400);
  }

  // ---- the page ----

  private schedulePrepare() {
    if (this.prepareTimer) clearTimeout(this.prepareTimer);
    this.prepareTimer = setTimeout(() => {
      this.prepareTimer = null;
      void this.prepare();
    }, 250);
  }

  private async prepare() {
    const generation = ++this.prepareGeneration;
    const source = this.entrySource();
    const path = this.path();
    const text = this.text();
    if (source === null || !path || text === null) {
      this.prepared.set({ pages: [], warnings: [], error: null });
      return;
    }
    this.preparing.set(true);
    let prepared: FluidPrepared;
    try {
      prepared = await this.engine.prepare(source, this.page(), this.name(), { ...this.otherFiles(), [path]: text });
    } catch (err) {
      prepared = { pages: [], warnings: [], error: { message: err instanceof Error ? err.message : String(err) } };
    }
    if (generation !== this.prepareGeneration) return;
    this.preparing.set(false);
    this.prepared.set(prepared);
    if (prepared.error) return;
    const d = this.doc();
    if (d && (!d.at || !d.size)) this.fit(d.preview.target);
    this.maskKey = '';
    this.placePage();
  }

  /** The page under the fluid, its ink as obstacles, and the result, after the box or page changes. */
  private placePage() {
    const p = this.prepared();
    const d = this.doc();
    if (!this.view || !p || p.error || !d?.at) return;
    const [ax, ay] = d.at;
    this.view.setBackdrop(p.svg ?? null, p.view ? [p.view[0] - ax, p.view[1] - ay, p.view[2], p.view[3]] : undefined);
    void this.updateMask();
    this.requestApply();
  }

  private async updateMask() {
    const p = this.prepared();
    const d = this.doc();
    const view = this.view;
    if (!view || !p || p.error || !d?.at || !d.size) return;
    const labels = new Set((p.canvases ?? []).map((c) => c.label));
    const canvases = d.obstacles.canvases.filter((c) => labels.has(c));
    const key = JSON.stringify([canvases, d.obstacles.grow, d.at, d.size, this.prepareGeneration]);
    if (key === this.maskKey) return;
    this.maskKey = key;
    if (!canvases.length) {
      view.setMask(null);
      this.maskNote.set(null);
      return;
    }
    const cell = Math.max(d.size[0], d.size[1]) / MASK_CELLS;
    const nx = Math.max(2, Math.ceil(d.size[0] / cell));
    const ny = Math.max(2, Math.ceil(d.size[1] / cell));
    try {
      const alpha = await this.engine.mask(canvases, [d.at[0], d.at[1], nx * cell, ny * cell], nx, ny);
      if (key !== this.maskKey || this.view !== view) return;
      const any = view.setMask(alpha, nx, ny, cell, d.obstacles.grow);
      this.maskNote.set(any ? null : 'None of their ink falls in the box.');
    } catch (err) {
      this.maskNote.set(`The obstacles couldn't be drawn: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  // ---- the result ----

  private requestApply() {
    const p = this.prepared();
    if (!this.view || !p || p.error || !this.doc()?.size) return;
    if (this.applying) {
      this.applyWanted = true;
      return;
    }
    void this.apply();
  }

  private async apply() {
    const view = this.view;
    const d = this.doc();
    if (!view || !d) return;
    this.applying = true;
    this.applyWanted = false;
    const wait = this.lastApply + APPLY_EVERY_MS - performance.now();
    if (wait > 0) await new Promise((resolve) => setTimeout(resolve, wait));
    this.lastApply = performance.now();
    const snapshot = view.sim.field_raw();
    const raw: RawField = { nx: snapshot.nx, ny: snapshot.ny, h: snapshot.h, u: snapshot.u, v: snapshot.v, pressure: snapshot.pressure, dye: snapshot.dye };
    snapshot.free();
    const field = view.frozen ? raw : coarsened(raw, LIVE_CELLS);
    let result: FluidResult;
    try {
      result = await this.engine.apply(field, fluidSettings(this.doc() ?? d), this.doc()?.preview.target ?? null, !view.frozen);
    } catch (err) {
      result = { svg: null, view: null, error: { message: err instanceof Error ? err.message : String(err) } };
    }
    if (this.view === view) this.setResult(result.svg, result.error?.message ?? null, result.view ?? null);
    this.applying = false;
    if (this.applyWanted) this.requestApply();
  }

  private setResult(svg: string | null, error: string | null, view: Box | null = null) {
    const old = this.result().url;
    if (old) URL.revokeObjectURL(old);
    this.result.set({ url: svgUrl(svg), error, view: view ?? this.result().view });
  }

  private setDarkly(patch: Partial<{ url: string | null; error: string | null; busy: boolean; progress: number }>) {
    const old = this.darkly().url;
    if ('url' in patch && old && old !== patch.url) URL.revokeObjectURL(old);
    this.darkly.update((d) => ({ ...d, ...patch }));
  }

  protected async renderDarkly() {
    const source = this.entrySource();
    const path = this.path();
    // A render is of a frozen fluid: one running is frozen as it is now, as the SVG beside it shows it.
    this.view?.freeze();
    const text = this.text();
    if (source === null || !path || text === null) return;
    const run = ++this.darklyRun;
    this.setDarkly({ busy: true, error: null, progress: 0 });
    try {
      await this.render.renderPages(source, [this.page()], { scale: 1, parchment: false, fluid: { name: this.name(), target: this.doc()?.preview.target ?? null } },
        (_, png) => run === this.darklyRun && this.setDarkly({ url: png ? URL.createObjectURL(png) : null }),
        (p) => run === this.darklyRun && this.setDarkly({ progress: Math.round(p.fraction * 100) }),
        { [path]: text });
      if (run === this.darklyRun) this.setDarkly({ busy: false });
    } catch (err) {
      if (run === this.darklyRun) this.setDarkly({ busy: false, error: err instanceof Error ? err.message : String(err) });
    }
  }

  // ---- looking closer ----

  /** Where the result shows the page: its view's top-left corner and width, in the result's pixels. */
  protected readonly picPlace = computed(() => {
    const view = this.pageView();
    const { width, height } = this.picSize();
    if (!view || !width) return null;
    const s = this.resultScale();
    const [cx, cy] = this.focus() ?? [view[0] + view[2] / 2, view[1] + view[3] / 2];
    return { left: width / 2 + (view[0] - cx) * s, top: height / 2 + (view[1] - cy) * s, width: view[2] * s };
  });

  /** Zooms in or out a step, about the middle of what's shown (the fluid's box when it's fitted). */
  protected zoomBy(step: number) {
    const current = this.zoom();
    const next = step > 0 ? ZOOMS.find((z) => z > current + 1e-6) : [...ZOOMS].reverse().find((z) => z < current - 1e-6);
    const box = this.boxCentre();
    if (next !== undefined && box) this.zoomAbout(next, this.focus() ?? box, this.focus() ?? box);
  }

  protected fitView() {
    this.zoom.set(1);
    this.focus.set(null);
  }

  protected wheelResult(event: WheelEvent) {
    const view = this.pageView();
    const pic = this.picEl()?.nativeElement;
    if (!view || !pic) return;
    event.preventDefault();
    const s = this.resultScale();
    const centre = this.focus() ?? [view[0] + view[2] / 2, view[1] + view[3] / 2];
    if (event.ctrlKey || event.metaKey) {
      const r = pic.getBoundingClientRect();
      const at: [number, number] = [centre[0] + (event.clientX - r.left - r.width / 2) / s, centre[1] + (event.clientY - r.top - r.height / 2) / s];
      this.zoomAbout(this.zoom() * Math.exp(-wheelPixels(event, 'y') / 300), at, centre);
    } else {
      this.moveFocus(centre, wheelPixels(event, 'x') / s, wheelPixels(event, 'y') / s);
    }
  }

  private wheelStage = (event: WheelEvent) => {
    const view = this.view;
    const centre = this.boxCentre();
    if (!view || !centre) return;
    event.preventDefault();
    const at = this.doc()?.at ?? [0, 0];
    const from = this.focus() ?? centre;
    if (event.ctrlKey || event.metaKey) {
      const [x, y] = view.domainAt(event.clientX, event.clientY);
      this.zoomAbout(this.zoom() * Math.exp(-wheelPixels(event, 'y') / 300), [x + at[0], y + at[1]], from);
    } else {
      this.moveFocus(from, wheelPixels(event, 'x') / view.unitScale, wheelPixels(event, 'y') / view.unitScale);
    }
  };

  protected readonly panning = signal(false);
  private panFrom: { x: number; y: number; focus: [number, number] } | null = null;

  protected panStart(event: PointerEvent) {
    const view = this.pageView();
    if (event.button !== 0 || !view) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    this.panning.set(true);
    this.panFrom = { x: event.clientX, y: event.clientY, focus: this.focus() ?? [view[0] + view[2] / 2, view[1] + view[3] / 2] };
  }

  protected panMove(event: PointerEvent) {
    const from = this.panFrom;
    if (!from) return;
    const s = this.resultScale();
    this.moveFocus(from.focus, (from.x - event.clientX) / s, (from.y - event.clientY) / s);
  }

  protected panEnd(_event: PointerEvent) {
    this.panning.set(false);
    this.panFrom = null;
  }

  /** Zooms to `zoom`, keeping the canvas point `at` where it is on screen, from a view centred on `from`. */
  private zoomAbout(zoom: number, at: [number, number], from: [number, number]) {
    const next = Math.min(MAX_ZOOM, Math.max(1, zoom));
    if (next <= 1 + 1e-3) {
      this.fitView();
      return;
    }
    const k = this.zoom() / next;
    this.zoom.set(next);
    this.focus.set(this.clampFocus([at[0] + (from[0] - at[0]) * k, at[1] + (from[1] - at[1]) * k]));
  }

  private moveFocus(from: [number, number], dx: number, dy: number) {
    this.focus.set(this.clampFocus([from[0] + dx, from[1] + dy]));
  }

  /** Keeps the middle of what's shown on the page or the fluid's box. */
  private clampFocus([x, y]: [number, number]): [number, number] {
    const view = this.pageView();
    const d = this.doc();
    const boxes = [view, d?.at && d.size ? [d.at[0], d.at[1], d.size[0], d.size[1]] : null].filter((b): b is Box => !!b);
    if (!boxes.length) return [x, y];
    const x0 = Math.min(...boxes.map((b) => b[0]));
    const y0 = Math.min(...boxes.map((b) => b[1]));
    const x1 = Math.max(...boxes.map((b) => b[0] + b[2]));
    const y1 = Math.max(...boxes.map((b) => b[1] + b[3]));
    return [Math.min(x1, Math.max(x0, x)), Math.min(y1, Math.max(y0, y))];
  }

  private boxCentre(): [number, number] | null {
    const d = this.doc();
    return d?.at && d.size ? [d.at[0] + d.size[0] / 2, d.at[1] + d.size[1] / 2] : null;
  }

  // ---- the settings ----

  protected setEntry(entry: string) {
    this.change((d) => ({ ...d, preview: { ...d.preview, entry, page: 0 } }));
  }

  protected setPage(page: number) {
    this.change((d) => ({ ...d, preview: { ...d.preview, page } }));
  }

  protected setTarget(target: string | null) {
    this.change((d) => ({ ...d, preview: { ...d.preview, target } }));
    this.requestApply();
  }

  protected setBox(which: 'at' | 'size', i: 0 | 1, text: string) {
    const n = Number(text);
    const d = this.doc();
    if (!d || text.trim() === '' || !Number.isFinite(n) || (which === 'size' && n < 1)) return;
    const box: [number, number] = [...(d[which] ?? (which === 'at' ? [0, 0] : [d.size?.[0] ?? 1600, d.size?.[1] ?? 900]))];
    box[i] = n;
    this.setDomain(which === 'at' ? box : (d.at ?? [0, 0]), which === 'size' ? box : (d.size ?? box));
  }

  /** Fits the box round the canvas labelled `target`, or the whole page. */
  protected fit(target: string | null) {
    const p = this.prepared();
    if (!p || p.error) return;
    const bounds = target === null ? null : (p.canvases?.find((c) => c.label === target)?.bounds ?? null);
    const box: Box | null = bounds ?? (p.size ? [0, 0, p.size[0], p.size[1]] : (p.view ?? null));
    if (!box) return;
    const round = (n: number) => Math.round(n * 100) / 100;
    this.setDomain([round(box[0]), round(box[1])], [round(Math.max(1, box[2])), round(Math.max(1, box[3]))]);
  }

  private setDomain(at: [number, number], size: [number, number]) {
    this.change((d) => ({ ...d, at, size }));
    this.view?.setDomain(size[0], size[1]);
    this.placePage();
  }

  protected setObstacle(label: string, on: boolean) {
    this.change((d) => {
      const canvases = d.obstacles.canvases.filter((c) => c !== label);
      return { ...d, obstacles: { ...d.obstacles, canvases: on ? [...canvases, label] : canvases } };
    });
    void this.updateMask();
  }

  protected setGrow(grow: number) {
    if (!Number.isFinite(grow) || grow < 0) return;
    this.change((d) => ({ ...d, obstacles: { ...d.obstacles, grow } }));
    void this.updateMask();
  }

  protected setEffectOn(effect: EffectName, on: boolean) {
    this.change((d) => {
      const effects = { ...d.effects };
      if (on) effects[effect] = {};
      else delete effects[effect];
      return { ...d, effects };
    });
    this.requestApply();
  }

  protected setEffect(effect: EffectName, name: string, value: unknown) {
    this.change((d) => {
      const settings = { ...(d.effects[effect] ?? {}) };
      if (value === undefined) delete settings[name];
      else settings[name] = value;
      return { ...d, effects: { ...d.effects, [effect]: settings } };
    });
    this.requestApply();
  }
}
