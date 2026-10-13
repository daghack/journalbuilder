import { Injectable, computed, inject, signal } from '@angular/core';
import type { Evaluation, PageView, ScriptError } from '../scribe';
import { FileService, type OpenedFile } from './file.service';
import { RenderService } from './render.service';

/** How long typing pauses before the script runs again. */
const DEBOUNCE_MS = 250;
const DRAFT_KEY = 'journal-editor.draft';

/** A page as the editor shows it. */
export interface Page extends PageView {
  /** Its SVG, as an object URL; until a new one arrives, the last one made. */
  svgUrl: string | null;
  /** Whether `svgUrl` is from an older run of the script. */
  stale: boolean;
}

interface Draft {
  name: string;
  source: string;
  saved: string | null;
}

/** The script being edited, what running it makes, and the page shown. */
@Injectable({ providedIn: 'root' })
export class JournalService {
  private readonly render = inject(RenderService);
  private readonly files = inject(FileService);

  readonly source = signal('');
  readonly fileName = signal('untitled.rhai');
  /** The source as last opened or saved; null if never. */
  private readonly savedSource = signal<string | null>(null);
  readonly dirty = computed(() => this.source() !== (this.savedSource() ?? ''));
  private fileHandle: OpenedFile['handle'] = null;

  readonly pages = signal<Page[]>([]);
  readonly selectedIndex = signal(0);
  readonly selected = computed<Page | null>(() => this.pages()[this.selectedIndex()] ?? null);
  /** The last run's error; the pages are then the last good run's. */
  readonly error = signal<ScriptError | null>(null);
  readonly evaluation = signal<Evaluation | null>(null);
  readonly evaluating = signal(false);
  /** The script whose pages show (the last one that ran without error). */
  readonly shownSource = signal('');

  /** The generation of the run whose pages show. */
  private shown = 0;
  private latest = 0;
  /** The scripts of runs not yet shown, by generation. */
  private runs = new Map<number, string>();
  private debounce: ReturnType<typeof setTimeout> | null = null;
  private draftTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    this.render.connect({
      evaluated: (generation, evaluation) => this.onEvaluated(generation, evaluation),
      svg: (generation, index, svg) => this.onSvg(generation, index, svg),
    });
    addEventListener('beforeunload', (event) => {
      if (this.dirty()) event.preventDefault();
    });
  }

  /** Start with the draft from last time, or `fallback` if there isn't one. */
  async restore(fallback: () => Promise<{ name: string; source: string }>) {
    const draft = readDraft();
    if (draft) {
      this.fileName.set(draft.name);
      this.savedSource.set(draft.saved);
      this.setSource(draft.source, true);
    } else {
      const { name, source } = await fallback();
      this.load(name, source, null);
    }
  }

  /** The script changed: run it again once typing pauses. */
  setSource(source: string, now = false) {
    this.source.set(source);
    if (this.debounce) clearTimeout(this.debounce);
    this.debounce = setTimeout(() => this.run(), now ? 0 : DEBOUNCE_MS);
    if (this.draftTimer) clearTimeout(this.draftTimer);
    this.draftTimer = setTimeout(() => this.saveDraft(), 500);
  }

  select(index: number) {
    const count = this.pages().length;
    if (count === 0) return;
    const clamped = Math.max(0, Math.min(count - 1, index));
    this.selectedIndex.set(clamped);
    this.render.focus(this.latest, clamped);
  }

  /** Show `source` as a newly opened script, `name`. */
  load(name: string, source: string, handle: OpenedFile['handle']) {
    this.fileHandle = handle;
    this.fileName.set(name);
    this.savedSource.set(source);
    this.selectedIndex.set(0);
    this.setSource(source, true);
  }

  async open() {
    const file = await this.files.open();
    if (file) this.load(file.name, file.text, file.handle);
  }

  /** Save to the file it came from, or (`as`, or if there isn't one) where the user picks. */
  async save(as = false) {
    const source = this.source();
    const saved = await this.files.save(source, this.fileName(), as ? null : this.fileHandle);
    if (!saved) return;
    this.fileHandle = saved.handle;
    this.fileName.set(saved.name);
    this.savedSource.set(source);
    this.saveDraft();
  }

  /** The file name without its extension, for exports. */
  readonly baseName = computed(() => this.fileName().replace(/\.[^.]*$/, '') || 'journal');

  private run() {
    this.debounce = null;
    this.evaluating.set(true);
    const source = this.source();
    this.latest = this.render.evaluate(source, this.selectedIndex());
    this.runs.set(this.latest, source);
  }

  private onEvaluated(generation: number, evaluation: Evaluation) {
    if (generation === this.latest) this.evaluating.set(false);
    if (generation < this.shown) return;
    this.evaluation.set(evaluation);
    this.error.set(evaluation.error);
    const source = this.runs.get(generation) ?? '';
    for (const run of this.runs.keys()) if (run <= generation) this.runs.delete(run);
    if (evaluation.error) return;
    this.shown = generation;
    this.shownSource.set(source);
    // Keep each page's last SVG until its new one comes.
    const old = this.pages();
    const pages = evaluation.pages.map((page): Page => {
      const before = old[page.index];
      const keep = before && page.view && before.label === page.label ? before.svgUrl : null;
      return { ...page, svgUrl: keep, stale: keep !== null };
    });
    for (const page of old) {
      if (page.svgUrl && !pages.some((p) => p.svgUrl === page.svgUrl)) URL.revokeObjectURL(page.svgUrl);
    }
    this.pages.set(pages);
    if (this.selectedIndex() >= pages.length) this.selectedIndex.set(Math.max(0, pages.length - 1));
  }

  private onSvg(generation: number, index: number, svg: string | null) {
    if (generation !== this.shown) return;
    this.pages.update((pages) => pages.map((page) => {
      if (page.index !== index) return page;
      if (page.svgUrl) URL.revokeObjectURL(page.svgUrl);
      const svgUrl = svg ? URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })) : null;
      return { ...page, svgUrl, stale: false };
    }));
  }

  private saveDraft() {
    const draft: Draft = { name: this.fileName(), source: this.source(), saved: this.savedSource() };
    try {
      localStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
    } catch {
      // No storage (private window, or full): the draft just isn't kept.
    }
  }
}

function readDraft(): Draft | null {
  try {
    const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null');
    return draft && typeof draft.source === 'string' && typeof draft.name === 'string' ? draft : null;
  } catch {
    return null;
  }
}
