import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { pruneEdits } from '../journal/canvas-edits';
import { editsPath, entryPath, splitPath } from '../journal/journal-files';
import { keep, stored } from '../layout/pane-state';
import type { Committed, Evaluation, PageView, ScriptError } from '../scribe';
import { ShellService } from '../shell/shell.service';
import { JournalStore } from './journal-store.service';
import { RenderService } from './render.service';

/** How long typing pauses before the script runs again. */
const DEBOUNCE_MS = 250;

/** A page as the editor shows it. */
export interface Page extends PageView {
  /** Its SVG, as an object URL; until a new one arrives, the last one made. */
  svgUrl: string | null;
  /** That SVG's text. */
  svg: string | null;
  /** The visual editor's edits (the edits file's text) the run that made `svg` had. */
  svgEdits: string | null;
  /** Whether `svgUrl` is from an older run of the script. */
  stale: boolean;
}

/**
 * The entry being edited, what running it (with the journal's files) makes,
 * and the page shown. The entry is the last one the sidebar opened.
 */
@Injectable({ providedIn: 'root' })
export class JournalService {
  private readonly render = inject(RenderService);
  private readonly store = inject(JournalStore);
  private readonly shell = inject(ShellService);

  /** The entry being edited, by name. */
  readonly entry = signal<string | null>(null);
  /** Its script. */
  readonly source = computed(() => {
    const entry = this.entry();
    return entry === null ? '' : (this.store.files()[entryPath(entry)] ?? '');
  });
  /** The visual editor's edits to its canvases: its edits file's text, null without one. */
  readonly edits = computed(() => {
    const entry = this.entry();
    return entry === null ? null : (this.store.files()[editsPath(entry)] ?? null);
  });

  readonly pages = signal<Page[]>([]);
  readonly selectedIndex = signal(0);
  readonly selected = computed<Page | null>(() => this.pages()[this.selectedIndex()] ?? null);
  /** The last run's error; the pages are then the last good run's. */
  readonly error = signal<ScriptError | null>(null);
  readonly evaluation = signal<Evaluation | null>(null);
  readonly evaluating = signal(false);
  /** The script whose pages show (the last one that ran without error). */
  readonly shownSource = signal('');
  /** The edits the pages shown were made with. */
  readonly shownEdits = signal<string | null>(null);
  /** The canvases the run shown committed to the visual editor, and where they are. */
  readonly committed = signal<Committed[]>([]);
  /**
   * Told when a run drops edits for canvases the script no longer commits:
   * the entry, and its edits file's text before (the visual editor makes
   * it a step to undo).
   */
  onPruned: ((entry: string, before: string) => void) | null = null;

  /** The generation of the run whose pages show. */
  private shown = 0;
  private latest = 0;
  /** The scripts and edits of runs not yet shown, by generation. */
  private runs = new Map<number, { source: string; edits: string | null }>();
  private debounce: ReturnType<typeof setTimeout> | null = null;
  /** The script, edits and journal files the last run had. */
  private ran: { source: string; edits: string | null; files: unknown } | null = null;
  /** The page each entry last showed, to come back to. */
  private readonly pageShown = new Map<string, number>();

  constructor() {
    this.render.connect({
      evaluated: (generation, evaluation) => this.onEvaluated(generation, evaluation),
      svg: (generation, index, svg) => this.onSvg(generation, index, svg),
    });
    effect(() => {
      this.store.loads();
      this.pageShown.clear();
    });
    // The entry the sidebar opens.
    effect(() => {
      const split = splitPath(this.shell.selected() ?? '');
      if (split?.kind !== 'entries' || split.name === untracked(() => this.entry())) return;
      untracked(() => this.show(split.name));
    });
    // No entry opened yet (a reload on another file), or it's gone: the one
    // last opened, else the first, for the preview.
    effect(() => {
      if (!this.store.restored()) return;
      const entries = this.store.manifest().entries;
      const entry = this.entry();
      if (entry !== null && entries.includes(entry)) return;
      const kept = stored<string | null>('entry', null);
      const next = kept !== null && entries.includes(kept) ? kept : (entries[0] ?? null);
      if (next !== entry) untracked(() => this.show(next));
    });
    effect(() => {
      const entry = this.entry();
      if (entry !== null) keep('entry', entry);
    });
    // Run the entry again when it, its edits or the journal's other files
    // change, while it shows (no need to while another file is edited,
    // unless it's previewed); at once unless the script changed.
    effect(() => {
      const source = this.source();
      const edits = this.edits();
      const files = this.store.resources();
      const showing = splitPath(this.shell.selected() ?? '')?.kind === 'entries';
      if (!(showing || this.shell.previewOpen()) || this.entry() === null) return;
      untracked(() => {
        if (this.ran && this.ran.source === source && this.ran.edits === edits && this.ran.files === files) return;
        if (this.debounce) clearTimeout(this.debounce);
        this.debounce = setTimeout(() => this.run(), this.ran?.source === source ? 0 : DEBOUNCE_MS);
      });
    });
  }

  /** The entry's script changed. */
  setSource(source: string) {
    const entry = this.entry();
    if (entry !== null) this.store.setFile(entryPath(entry), source);
  }

  select(index: number) {
    const count = this.pages().length;
    if (count === 0) return;
    const clamped = Math.max(0, Math.min(count - 1, index));
    this.selectedIndex.set(clamped);
    this.render.focus(this.latest, clamped);
  }

  /** The journal's and entry's names, for exports. */
  readonly baseName = computed(() => {
    const entry = this.entry() ?? 'entry';
    const journal = this.store.name();
    return journal === entry ? journal : `${journal}_${entry}`;
  });

  private show(entry: string | null) {
    const last = this.entry();
    if (last !== null) this.pageShown.set(last, this.selectedIndex());
    this.entry.set(entry);
    this.selectedIndex.set((entry !== null && this.pageShown.get(entry)) || 0);
    this.clearPages();
  }

  private run() {
    this.debounce = null;
    this.evaluating.set(true);
    const source = this.source();
    const edits = this.edits();
    this.ran = { source, edits, files: this.store.resources() };
    this.latest = this.render.evaluate(source, edits, this.selectedIndex());
    this.runs.set(this.latest, { source, edits });
  }

  private clearPages() {
    for (const page of this.pages()) if (page.svgUrl) URL.revokeObjectURL(page.svgUrl);
    this.pages.set([]);
    this.error.set(null);
    this.evaluation.set(null);
    this.shownSource.set('');
    this.shownEdits.set(null);
    this.committed.set([]);
    this.ran = null;
    // Runs of the entry shown before aren't shown.
    this.shown = this.latest + 1;
  }

  private onEvaluated(generation: number, evaluation: Evaluation) {
    if (generation === this.latest) this.evaluating.set(false);
    if (generation < this.shown) return;
    this.evaluation.set(evaluation);
    this.error.set(evaluation.error);
    const { source, edits } = this.runs.get(generation) ?? { source: '', edits: null };
    for (const run of this.runs.keys()) if (run <= generation) this.runs.delete(run);
    if (evaluation.error) return;
    this.shown = generation;
    this.shownSource.set(source);
    this.shownEdits.set(edits);
    this.committed.set(evaluation.committed ?? []);
    // Keep each page's last SVG until its new one comes.
    const old = this.pages();
    const pages = evaluation.pages.map((page): Page => {
      const before = old[page.index];
      const kept = before && page.view && before.label === page.label ? before : null;
      return { ...page, svgUrl: kept?.svgUrl ?? null, svg: kept?.svg ?? null, svgEdits: kept?.svgEdits ?? null, stale: kept?.svgUrl != null };
    });
    for (const page of old) {
      if (page.svgUrl && !pages.some((p) => p.svgUrl === page.svgUrl)) URL.revokeObjectURL(page.svgUrl);
    }
    this.pages.set(pages);
    if (this.selectedIndex() >= pages.length) this.selectedIndex.set(Math.max(0, pages.length - 1));
    if (generation === this.latest) this.prune(source, edits, evaluation.committed ?? []);
  }

  /**
   * Drop the edits for canvases the script no longer commits, once a run
   * of the script and edits as they now are has shown which it does.
   */
  private prune(source: string, edits: string | null, committed: Committed[]) {
    const entry = this.entry();
    if (entry === null || edits === null || source !== this.source() || edits !== this.edits()) return;
    const pruned = pruneEdits(edits, committed.map((c) => c.name));
    if (pruned === edits) return;
    if (pruned === null) this.store.removeFile(editsPath(entry));
    else this.store.setFile(editsPath(entry), pruned);
    this.onPruned?.(entry, edits);
  }

  private onSvg(generation: number, index: number, svg: string | null) {
    if (generation !== this.shown) return;
    this.pages.update((pages) => pages.map((page) => {
      if (page.index !== index) return page;
      if (page.svgUrl) URL.revokeObjectURL(page.svgUrl);
      const svgUrl = svg ? URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })) : null;
      return { ...page, svgUrl, svg, svgEdits: this.shownEdits(), stale: false };
    }));
  }
}
