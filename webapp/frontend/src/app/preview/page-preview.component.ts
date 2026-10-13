import { Component, ElementRef, computed, effect, inject, signal, untracked, viewChild, type OnDestroy } from '@angular/core';
import { JournalService } from '../services/journal.service';
import { RenderService, type RenderProgress } from '../services/render.service';

/** A Darkly render of a page: its PNG as an object URL, `width` pixels across at `scale` pixels per unit. */
interface DarklyImage {
  key: string;
  url: string | null;
  scale: number;
  width: number;
}

const ZOOMS = [0.1, 0.15, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4];
/** Darkly renders kept, newest last. */
const CACHE_SIZE = 24;

/** The middle: the picked page, large, as SVG or (on demand) rendered with Darkly. */
@Component({
  selector: 'app-page-preview',
  template: `
    <div class="bar">
      <span class="title">{{ page()?.label ?? 'No page' }}</span>
      <div class="segmented" role="group" aria-label="Preview">
        <button type="button" [class.on]="mode() === 'svg'" (click)="mode.set('svg')" title="Strokes as SVG, redrawn as you type">SVG</button>
        <button type="button" [class.on]="mode() === 'darkly'" (click)="mode.set('darkly')" title="Rendered with Darkly's brushes on the GPU">Darkly</button>
      </div>
      @if (mode() === 'darkly') {
        <label>Scale
          <select (change)="scale.set(+$any($event.target).value)">
            @for (s of scales; track s) { <option [value]="s" [selected]="s === scale()">{{ s }}</option> }
          </select>
        </label>
        <label><input type="checkbox" [checked]="parchment()" (change)="parchment.set($any($event.target).checked)" /> Parchment</label>
      }
      <span class="spacer"></span>
      <button type="button" (click)="zoomBy(-1)" title="Zoom out" aria-label="Zoom out">−</button>
      <button type="button" class="zoom" (click)="zoom.set('fit')" title="Fit the page">{{ zoom() === 'fit' ? 'Fit' : percent() }}</button>
      <button type="button" (click)="zoomBy(1)" title="Zoom in" aria-label="Zoom in">+</button>
    </div>
    @if (mode() === 'darkly' && (progress() || darklyError())) {
      <div class="status" [class.error]="darklyError()">
        @if (darklyError(); as error) {
          <span>{{ error }}</span>
          <button type="button" (click)="retry()">Retry</button>
        } @else if (progress(); as p) {
          <span>{{ p.fraction < 0.85 ? 'Rendering with Darkly…' : 'Finishing…' }} {{ (p.fraction * 100).toFixed(0) }}%</span>
          <progress [value]="p.fraction" max="1"></progress>
          <button type="button" (click)="cancel()">Cancel</button>
        }
      </div>
    }
    <div class="stage" #stage (wheel)="onWheel($event)">
      @if (page(); as page) {
        @if (shown(); as image) {
          <img [src]="image.url" [alt]="page.label" [class.stale]="image.stale"
               [style.width.px]="image.width" [class.paper]="!image.ownPaper" />
        } @else if (page.view) {
          <div class="placeholder" [style.width.px]="displayWidth(page.view[2])" [style.aspect-ratio]="page.view[2] + ' / ' + page.view[3]">drawing…</div>
        } @else {
          <p class="empty">Nothing is drawn on this page.</p>
        }
      } @else {
        <p class="empty">{{ journal.error() ? 'The script has an error; fix it to see its pages.' : 'The script has made no pages yet.' }}</p>
      }
    </div>
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-width: 0; min-height: 0; background: var(--stage-bg); }
    .bar { display: flex; align-items: center; gap: 8px; padding: 6px 10px; background: var(--surface); border-bottom: 1px solid var(--border); font-size: 12px; flex-wrap: wrap; }
    .title { font-weight: 600; margin-right: 6px; }
    .spacer { flex: 1; }
    .segmented { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
    .segmented button { border: none; border-radius: 0; }
    .segmented button.on { background: var(--accent); color: var(--accent-fg); }
    .zoom { min-width: 52px; }
    label { display: inline-flex; gap: 4px; align-items: center; }
    .status { display: flex; align-items: center; gap: 10px; padding: 6px 10px; font-size: 12px; background: var(--surface-2); border-bottom: 1px solid var(--border); }
    .status.error { color: var(--danger); }
    .status progress { flex: 1; max-width: 260px; }
    .stage { flex: 1; overflow: auto; display: grid; place-items: center; padding: 32px; min-height: 0; }
    img { display: block; max-width: none; box-shadow: var(--page-shadow); }
    img.paper { background: var(--paper); }
    img.stale { opacity: 0.6; }
    .placeholder { background: var(--paper); color: var(--paper-muted); display: grid; place-items: center; box-shadow: var(--page-shadow); font-size: 12px; }
    .empty { color: var(--stage-fg); font-size: 13px; }
  `,
})
export class PagePreviewComponent implements OnDestroy {
  protected readonly journal = inject(JournalService);
  private readonly render = inject(RenderService);
  private readonly stage = viewChild.required<ElementRef<HTMLElement>>('stage');

  protected readonly scales = [0.25, 0.5, 0.75, 1];
  protected readonly mode = signal<'svg' | 'darkly'>('svg');
  protected readonly scale = signal(0.5);
  protected readonly parchment = signal(false);
  protected readonly zoom = signal<'fit' | number>('fit');
  private readonly stageSize = signal({ width: 0, height: 0 });
  private readonly observer = new ResizeObserver(([entry]) => {
    this.stageSize.set({ width: entry.contentRect.width, height: entry.contentRect.height });
  });

  protected readonly page = this.journal.selected;
  protected readonly progress = signal<RenderProgress | null>(null);
  protected readonly darklyError = signal<string | null>(null);
  private readonly cache = signal<DarklyImage[]>([]);
  private rendering: string | null = null;

  /** What the current page's Darkly render is keyed by. */
  private readonly darklyKey = computed(() => {
    const page = this.page();
    if (!page) return null;
    return `${hash(this.journal.shownSource())}|${page.index}|${this.scale()}|${this.parchment()}`;
  });

  /** Pixels per canvas unit on screen. */
  private readonly unitScale = computed(() => {
    const zoom = this.zoom();
    if (zoom !== 'fit') return zoom;
    const view = this.page()?.view;
    const { width, height } = this.stageSize();
    if (!view || !width) return 0.5;
    return Math.max(0.02, Math.min((width - 64) / view[2], (height - 64) / view[3]));
  });
  protected readonly percent = computed(() => `${Math.round(this.unitScale() * 100)}%`);

  protected readonly shown = computed(() => {
    const page = this.page();
    if (!page) return null;
    // In Darkly mode, the page's Darkly render once it's done; till then, its SVG.
    const key = this.darklyKey();
    const darkly = this.mode() === 'darkly' ? this.cache().find((c) => c.key === key) : undefined;
    if (darkly?.url) {
      return { url: darkly.url, stale: false, width: darkly.width * this.unitScale() / darkly.scale, ownPaper: this.parchment() };
    }
    if (!page.svgUrl || !page.view) return null;
    return { url: page.svgUrl, stale: page.stale, width: page.view[2] * this.unitScale(), ownPaper: false };
  });

  constructor() {
    effect(() => this.observer.observe(this.stage().nativeElement));
    // In Darkly mode, render what's shown when it changes (one at a time; the newest waits).
    effect(() => {
      if (this.mode() !== 'darkly') return;
      const key = this.darklyKey();
      if (key && !this.cache().some((c) => c.key === key)) untracked(() => this.startDarkly());
    });
  }

  protected displayWidth(units: number) {
    return units * this.unitScale();
  }

  protected zoomBy(step: number) {
    const current = this.unitScale();
    const next = step > 0 ? ZOOMS.find((z) => z > current + 1e-6) : [...ZOOMS].reverse().find((z) => z < current - 1e-6);
    this.zoom.set(next ?? current);
  }

  protected onWheel(event: WheelEvent) {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    const next = this.unitScale() * Math.exp(-event.deltaY / 300);
    this.zoom.set(Math.min(4, Math.max(0.05, next)));
  }

  private startDarkly() {
    if (this.rendering) return;
    const key = this.darklyKey();
    const page = this.page();
    if (!key || !page) return;
    if (!page.view) {
      this.store({ key, url: null, scale: this.scale(), width: 0 });
      return;
    }
    this.rendering = key;
    this.darklyError.set(null);
    this.progress.set({ page: 0, pages: 1, fraction: 0 });
    const scale = this.scale();
    let png: Blob | null = null;
    this.render.renderPages(this.journal.shownSource(), [page.index], { scale, parchment: this.parchment() },
      (_, image) => (png = image),
      (p) => this.progress.set(p),
    ).then(async () => {
      const width = png ? (await createImageBitmap(png)).width : 0;
      this.store({ key, url: png ? URL.createObjectURL(png) : null, scale, width });
    }).then(
      () => undefined,
      (err: Error) => this.darklyError.set(err.message === 'cancelled' ? 'Render cancelled.' : err.message),
    ).finally(() => {
      this.rendering = null;
      this.progress.set(null);
      // Something newer to render?
      const current = this.darklyKey();
      if (this.mode() === 'darkly' && !this.darklyError() && current && !this.cache().some((c) => c.key === current)) this.startDarkly();
    });
  }

  private store(image: DarklyImage) {
    this.cache.update((cache) => {
      const next = [...cache.filter((c) => c.key !== image.key), image];
      for (const old of next.splice(0, Math.max(0, next.length - CACHE_SIZE))) {
        if (old.url) URL.revokeObjectURL(old.url);
      }
      return next;
    });
  }

  protected cancel() {
    this.render.cancelRenders();
  }

  protected retry() {
    this.darklyError.set(null);
    this.startDarkly();
  }

  ngOnDestroy() {
    this.observer.disconnect();
  }
}

/** FNV-1a of `text`, to key renders by script. */
function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(36) + text.length.toString(36);
}
