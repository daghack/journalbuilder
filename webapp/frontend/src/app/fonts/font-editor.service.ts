import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { kindOf, nameOf, pathOf, splitPath } from '../journal/journal-files';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ShellService } from '../shell/shell.service';
import { checkFonts, parseFont, serializeFont, serializeFonts, withOption, type Font, type FontsFile, type Glyph } from './font-file';
import * as edit from './glyph-edit';
import type { PointRef, SegmentRef } from './glyph-edit';
import { formatPath, inkExtent, parsePath, type GlyphPath, type Segment } from './glyph-path';

/** How long edits pause before the journal is given them. */
const PUSH_DELAY_MS = 400;
/** Undo steps kept. */
const HISTORY = 200;

/** What the canvas does when clicked. */
export type Tool = 'select' | 'stroke' | 'L' | 'Q' | 'C' | 'dot' | 'ellipse';

/** Which glyph and variant are being drawn, by index in the font. */
export interface Selection {
  glyph: number;
  variant: number;
}

/** Where a font's guides go, in glyph units (y down). */
export interface Guides {
  cap?: number;
  xHeight?: number;
  baseline?: number;
  descender?: number;
}

/**
 * The glyphsets being edited: every one the journal has (its own glyphset
 * files over the library's), the glyphset and glyph variant shown, the
 * tool in hand, and undo. Every change that leaves the glyphsets valid is
 * given to the journal once edits pause, as the glyphset files that differ
 * from the library's: changing a library glyphset gives the journal a copy
 * that stands in for it.
 */
@Injectable()
export class FontEditorService {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly shell = inject(ShellService);

  readonly file = signal<FontsFile | null>(null);
  /** Why the glyphsets couldn't be loaded, if they couldn't. */
  readonly loadError = signal<string | null>(null);
  /** The library's glyphsets, read; null until the library is. */
  private readonly builtinFile = computed<FontsFile | null>(() => {
    if (!this.library.loaded()) return null;
    return fontsOf(this.library.files(), {});
  });
  /** The glyphset files the editor last read or wrote, by path, to tell the journal's changes from its own. */
  private synced: Record<string, string> = {};

  readonly fontName = signal<string>('plain');
  readonly selection = signal<Selection | null>(null);
  readonly tool = signal<Tool>('select');
  readonly snap = signal(true);

  private undoStack: FontsFile[] = [];
  private redoStack: FontsFile[] = [];
  readonly canUndo = signal(false);
  readonly canRedo = signal(false);
  /** Edits made one after another under the same key (a drag, typing) undo as one. */
  private coalescing: string | null = null;

  readonly text = computed(() => {
    const file = this.file();
    return file ? serializeFonts(file) : null;
  });
  /** The glyphsets as glyphset files, by path, for previews to run with. */
  readonly previewFiles = computed(() => {
    const file = this.file();
    return file ? Object.fromEntries(file.map((font) => [pathOf('glyphsets', font.name), serializeFont(font)])) : null;
  });
  readonly problems = computed(() => {
    const file = this.file();
    return file ? checkFonts(file) : [];
  });
  /** The library's names for its glyphsets. */
  readonly builtinNames = computed(() => (this.builtinFile() ?? []).map((font) => font.name));
  /** The library's glyphset of the shown one's name; null if there's none. */
  readonly builtinFont = computed(() => this.builtinFile()?.find((font) => font.name === this.fontName()) ?? null);
  /** The shown glyphset's file. */
  readonly fontPath = computed(() => pathOf('glyphsets', this.fontName()));
  /** Whether the journal has the shown glyphset's file (else it's the library's). */
  readonly fontOwn = computed(() => this.fontPath() in this.store.files());

  readonly font = computed<Font | null>(() => this.file()?.find((font) => font.name === this.fontName()) ?? null);
  readonly glyph = computed<Glyph | null>(() => {
    const selection = this.selection();
    return selection ? (this.font()?.glyphs[selection.glyph] ?? null) : null;
  });
  readonly variant = computed<string | null>(() => {
    const selection = this.selection();
    return selection ? (this.glyph()?.variants[selection.variant] ?? null) : null;
  });
  /** The variant shown, read; or why it can't be. */
  readonly path = computed<{ path: GlyphPath; error: null } | { path: null; error: string } | null>(() => {
    const d = this.variant();
    if (d === null) return null;
    try {
      return { path: parsePath(d), error: null };
    } catch (err) {
      return { path: null, error: (err as Error).message };
    }
  });

  /** Where the font's capitals, x-height, baseline and descenders sit, from its glyphs. */
  readonly guides = computed<Guides>(() => {
    const font = this.font();
    if (!font) return {};
    const extent = (key: string) => {
      const glyph = font.glyphs.find((g) => g.key === key);
      if (!glyph) return null;
      try {
        return inkExtent(parsePath(glyph.variants[0]));
      } catch {
        return null;
      }
    };
    const median = (values: number[]) => {
      if (!values.length) return undefined;
      const sorted = [...values].sort((a, b) => a - b);
      return sorted[Math.floor(sorted.length / 2)];
    };
    const capitals = [...'ABDEFHIKLMNPRTUVWXZ'].map(extent).filter((e) => e !== null);
    const lower = [...'acemnorsuvwxz'].map(extent).filter((e) => e !== null);
    const descending = [...'gjpqy'].map(extent).filter((e) => e !== null);
    return {
      cap: median(capitals.map((e) => e.top)),
      baseline: median([...capitals, ...lower].map((e) => e.bottom)),
      xHeight: median(lower.map((e) => e.top)),
      descender: median(descending.map((e) => e.bottom)),
    };
  });

  /** The points picked on the canvas (see glyph-edit's refKey), and the segment. */
  readonly picked = signal<string[]>([]);
  readonly pickedSegment = signal<SegmentRef | null>(null);
  /** Whether the glyph's other variants show faintly behind it. */
  readonly onion = signal(true);

  private pushTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    // The journal's glyphset files, and the library's, as they change:
    // read again unless they're what the editor last read or wrote.
    effect(() => {
      const own = this.glyphsetFiles(this.store.files());
      const library = this.library.files();
      if (!this.library.loaded()) return;
      untracked(() => {
        const current = { ...this.glyphsetFiles(library), ...own };
        if (this.file() && sameFiles(current, this.synced)) return;
        this.read(own, library);
      });
    });
    // Give the journal the glyphsets once edits pause, if they're valid.
    effect(() => {
      const file = this.file();
      const ok = this.problems().length === 0;
      if (!file || !ok) return;
      if (this.pushTimer) clearTimeout(this.pushTimer);
      this.pushTimer = setTimeout(() => untracked(() => this.push(file)), PUSH_DELAY_MS);
    });
    // The glyphset the sidebar opens.
    effect(() => {
      const path = this.shell.selected();
      const file = this.file();
      if (!path || kindOf(path) !== 'glyphsets' || !file) return;
      const name = nameOf(path);
      untracked(() => {
        if (name !== this.fontName() && file.some((font) => font.name === name)) this.pickFont(name);
      });
    });
    // A different glyph or variant: nothing of it is picked.
    effect(() => {
      this.selection();
      this.fontName();
      this.picked.set([]);
      this.pickedSegment.set(null);
    });
  }

  // ---- the canvas's edits, to the variant shown ----

  /** The variant shown, if it reads. */
  readonly current = computed(() => this.path()?.path ?? null);

  /** Change the variant shown with `change`. */
  editPath(change: (path: GlyphPath) => GlyphPath, coalesce: string | null = null) {
    const path = this.current();
    if (path) this.setPath(change(path), coalesce);
  }

  pickedRefs(): PointRef[] {
    return this.picked().map(edit.parseRef);
  }

  /** Delete what's picked: the segment, or the points. */
  deletePicked() {
    const segment = this.pickedSegment();
    if (segment) {
      this.editPath((path) => edit.deleteSegment(path, segment));
    } else if (this.picked().length) {
      const refs = this.pickedRefs();
      this.editPath((path) => edit.deletePoints(path, refs));
    }
    this.picked.set([]);
    this.pickedSegment.set(null);
  }

  convertPicked(kind: Segment['kind']) {
    const segment = this.pickedSegment();
    if (segment) this.editPath((path) => edit.convert(path, segment, kind));
  }

  /** Split the picked segment in half, picking the anchor added. */
  splitPicked() {
    const segment = this.pickedSegment();
    const path = this.current();
    if (!segment || !path) return;
    const split = edit.splitAt(path, segment, 0.5);
    this.setPath(split.path);
    this.pickedSegment.set(null);
    this.picked.set([edit.refKey(split.anchor)]);
  }

  /** The stroke what's picked belongs to. */
  pickedStroke(): number | null {
    return this.pickedSegment()?.s ?? this.pickedRefs()[0]?.s ?? null;
  }

  toggleClosed() {
    const s = this.pickedStroke();
    const path = this.current();
    if (s === null || !path?.[s]) return;
    this.setPath(edit.setClosed(path, s, !path[s].closed));
  }

  reversePicked() {
    const s = this.pickedStroke();
    if (s === null) return;
    this.editPath((path) => edit.reverse(path, s));
    this.picked.set([]);
    this.pickedSegment.set(null);
  }

  /** Move what's picked by (dx, dy), arrow presses in a row undoing as one. */
  nudge(dx: number, dy: number) {
    const refs = this.pickedRefs();
    if (!refs.length) return;
    this.editPath((path) => edit.movePoints(path, refs, { x: dx, y: dy }), 'nudge');
  }

  /** Every glyphset file of `files`, by path. */
  private glyphsetFiles(files: Record<string, string>): Record<string, string> {
    return Object.fromEntries(Object.entries(files).filter(([path]) => kindOf(path) === 'glyphsets'));
  }

  /** Read the glyphsets afresh: the journal's files over the library's. */
  private read(own: Record<string, string>, library: Record<string, string>) {
    let file: FontsFile;
    try {
      file = fontsOf(library, own);
    } catch (err) {
      this.loadError.set((err as Error).message);
      return;
    }
    this.loadError.set(null);
    this.synced = { ...this.glyphsetFiles(library), ...own };
    const keep = this.selection();
    this.file.set(file);
    this.undoStack = [];
    this.redoStack = [];
    this.updateHistory();
    const names = file.map((f) => f.name);
    const wanted = splitPath(this.shell.selected() ?? '')?.kind === 'glyphsets' ? nameOf(this.shell.selected()!) : this.fontName();
    if (keep && wanted === this.fontName() && names.includes(wanted)) {
      this.selection.set(keep);
      this.keepSelection();
    } else {
      this.pickFont(names.includes(wanted) ? wanted : (names[0] ?? ''));
    }
  }

  /**
   * Give the journal the glyphsets as they are: a file for each that isn't
   * the library's as it is, written where it differs from the journal's;
   * the journal's files of glyphsets no longer there removed.
   */
  private push(file: FontsFile) {
    const library = this.library.files();
    const files = this.store.files();
    const builtin = new Map((this.builtinFile() ?? []).map((font) => [font.name, serializeFont(font)]));
    const names = new Set(file.map((font) => font.name));
    for (const font of file) {
      const path = pathOf('glyphsets', font.name);
      const text = serializeFont(font);
      const own = files[path];
      if (own === undefined && builtin.get(font.name) === text) continue;
      if (own !== undefined && own === text) continue;
      this.store.setFile(path, text);
    }
    for (const path of Object.keys(this.glyphsetFiles(files))) {
      if (!names.has(nameOf(path))) this.store.removeFile(path);
    }
    this.synced = { ...this.glyphsetFiles(library), ...this.glyphsetFiles(this.store.files()) };
  }

  /** Remove the journal's copy of the glyphset shown: the library's is used again. */
  useLibraryFont() {
    if (!this.fontOwn() || !this.builtinFont()) return;
    if (this.pushTimer) clearTimeout(this.pushTimer);
    this.store.removeFile(this.fontPath());
  }

  /** Show glyphset `name`, and open its file. */
  openFont(name: string) {
    this.pickFont(name);
    if (name) this.shell.open(pathOf('glyphsets', name));
  }

  pickFont(name: string) {
    this.fontName.set(name);
    const font = this.file()?.find((f) => f.name === name);
    this.selection.set(font && font.glyphs.length ? { glyph: 0, variant: 0 } : null);
  }

  select(glyph: number, variant = 0) {
    this.selection.set({ glyph, variant });
  }

  // ---- changes ----

  /** Change the fonts with `change`, as one undo step (or part of the last, under the same `coalesce` key). */
  update(change: (file: FontsFile) => FontsFile, coalesce: string | null = null) {
    const before = this.file();
    if (!before) return;
    const after = change(before);
    if (after === before) return;
    if (coalesce === null || coalesce !== this.coalescing) {
      this.undoStack.push(before);
      if (this.undoStack.length > HISTORY) this.undoStack.shift();
    }
    this.coalescing = coalesce;
    this.redoStack = [];
    this.file.set(after);
    this.updateHistory();
  }

  /** End any run of coalesced edits: the next is a step of its own. */
  settle() {
    this.coalescing = null;
  }

  undo() {
    const before = this.undoStack.pop();
    if (!before) return;
    this.redoStack.push(this.file()!);
    this.coalescing = null;
    this.file.set(before);
    this.keepSelection();
    this.updateHistory();
  }

  redo() {
    const after = this.redoStack.pop();
    if (!after) return;
    this.undoStack.push(this.file()!);
    this.coalescing = null;
    this.file.set(after);
    this.keepSelection();
    this.updateHistory();
  }

  private updateHistory() {
    this.canUndo.set(this.undoStack.length > 0);
    this.canRedo.set(this.redoStack.length > 0);
  }

  /** After undo, keep the selection inside the font (it may have lost glyphs or variants). */
  private keepSelection() {
    const font = this.font();
    const selection = this.selection();
    if (!font) {
      const first = this.file()?.[0]?.name;
      if (first !== undefined) this.pickFont(first);
      return;
    }
    if (!selection) return;
    const glyph = Math.min(selection.glyph, font.glyphs.length - 1);
    if (glyph < 0) return this.selection.set(null);
    const variant = Math.min(selection.variant, font.glyphs[glyph].variants.length - 1);
    this.selection.set({ glyph, variant: Math.max(0, variant) });
  }

  /** Change the font shown. */
  updateFont(change: (font: Font) => Font, coalesce: string | null = null) {
    const name = this.fontName();
    this.update((file) => file.map((font) => (font.name === name ? change(font) : font)), coalesce);
  }

  /** Change the glyph shown. */
  updateGlyph(change: (glyph: Glyph) => Glyph, coalesce: string | null = null) {
    const selection = this.selection();
    if (!selection) return;
    this.updateFont((font) => ({ ...font, glyphs: font.glyphs.map((glyph, i) => (i === selection.glyph ? change(glyph) : glyph)) }), coalesce);
  }

  /** Set the variant shown to `path`. */
  setPath(path: GlyphPath, coalesce: string | null = null) {
    this.setVariantText(formatPath(path), coalesce);
  }

  /** Set the variant shown to the path data `d`, as written. */
  setVariantText(d: string, coalesce: string | null = null) {
    const selection = this.selection();
    if (!selection) return;
    this.updateGlyph((glyph) => ({ ...glyph, variants: glyph.variants.map((v, i) => (i === selection.variant ? d : v)) }), coalesce);
  }

  setOption(key: string, raw: string | undefined) {
    this.updateFont((font) => withOption(font, key, raw), `option:${this.fontName()}:${key}`);
  }

  /** Add a glyph keyed `key` (blank), and show it. */
  addGlyph(key: string): string | null {
    const font = this.font();
    if (!font) return 'no font is open';
    if (!key) return 'a glyph needs a key: a character, or a name';
    if (font.glyphs.some((g) => g.key === key)) return `${font.name} already has '${key}'`;
    this.updateFont((f) => ({ ...f, glyphs: [...f.glyphs, { key, variants: ['M 0 0 L 0 0'], list: false }] }));
    this.select(font.glyphs.length, 0);
    return null;
  }

  renameGlyph(key: string): string | null {
    const font = this.font();
    const glyph = this.glyph();
    if (!font || !glyph) return null;
    if (!key) return 'a glyph needs a key';
    if (key !== glyph.key && font.glyphs.some((g) => g.key === key)) return `${font.name} already has '${key}'`;
    this.updateGlyph((g) => ({ ...g, key }));
    return null;
  }

  deleteGlyph() {
    const selection = this.selection();
    if (!selection) return;
    this.updateFont((font) => ({ ...font, glyphs: font.glyphs.filter((_, i) => i !== selection.glyph) }));
    this.keepSelection();
  }

  /** Copy the variant shown as a new variant, and show it. */
  duplicateVariant() {
    const selection = this.selection();
    const glyph = this.glyph();
    if (!selection || !glyph) return;
    this.updateGlyph((g) => ({ ...g, list: true, variants: [...g.variants, g.variants[selection.variant]] }));
    this.select(selection.glyph, glyph.variants.length);
  }

  deleteVariant() {
    const selection = this.selection();
    const glyph = this.glyph();
    if (!selection || !glyph || glyph.variants.length <= 1) return;
    this.updateGlyph((g) => ({ ...g, variants: g.variants.filter((_, i) => i !== selection.variant) }));
    this.select(selection.glyph, Math.max(0, selection.variant - 1));
  }

  /** A new, empty font, shown. */
  addFont(name: string, from: Font | null = null): string | null {
    const file = this.file();
    if (!file) return 'no fonts are open';
    const problem = fontNameProblem(file, name);
    if (problem) return problem;
    const font: Font = from ? { ...from, name } : { name, options: [], glyphs: [] };
    this.update((f) => [...f, font]);
    this.openFont(name);
    return null;
  }

  /** Give the journal a copy of the glyphset shown, though it's unchanged: a file of its own to change. */
  copyFont() {
    const font = this.font();
    if (font && !this.fontOwn()) this.store.setFile(this.fontPath(), serializeFont(font));
  }

  renameFont(name: string): string | null {
    const file = this.file();
    const old = this.fontName();
    if (!file || name === old) return null;
    if (this.builtinFont()) return `the library has ${old}: duplicate it under the new name instead`;
    const problem = fontNameProblem(file, name);
    if (problem) return problem;
    const raw = JSON.stringify(old);
    // Fonts falling back to it follow it to its new name.
    this.update((f) => f.map((font) => {
      const renamed = font.name === old ? { ...font, name } : font;
      return renamed.options.some(([k, v]) => k === 'fallback' && v === raw) ? withOption(renamed, 'fallback', JSON.stringify(name)) : renamed;
    }));
    this.fontName.set(name);
    this.shell.open(pathOf('glyphsets', name));
    return null;
  }

  /** Delete the glyphset shown, if it's the journal's alone (the library's would come back). */
  deleteFont() {
    const name = this.fontName();
    if (this.builtinFont()) return;
    this.update((file) => file.filter((font) => font.name !== name));
    const first = this.file()?.[0]?.name;
    if (first !== undefined) this.openFont(first);
  }
}

function fontNameProblem(file: FontsFile, name: string): string | null {
  if (!name) return 'a font needs a name';
  if (file.some((font) => font.name === name)) return `there's already a font named '${name}'`;
  return null;
}

/** The glyphsets of `own` (glyphset files by path) over `library`'s, sorted by name. Throws, naming the file, if one doesn't read. */
function fontsOf(library: Record<string, string>, own: Record<string, string>): FontsFile {
  const files = { ...library, ...own };
  return Object.keys(files).filter((path) => kindOf(path) === 'glyphsets').sort().map((path) => {
    try {
      return parseFont(nameOf(path), files[path]);
    } catch (err) {
      throw new Error(`${path} isn't a glyphset: ${(err as Error).message}`);
    }
  });
}

/** Two sets of files alike, path for path. */
function sameFiles(a: Record<string, string>, b: Record<string, string>): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((key) => a[key] === b[key]);
}
