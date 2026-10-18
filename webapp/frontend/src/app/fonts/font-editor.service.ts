import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { FileService, JSON_FILES, type OpenedFile } from '../services/file.service';
import { ToolsService } from '../services/tools.service';
import { WorkspaceService, hash } from '../services/workspace.service';
import { ShellService } from '../shell/shell.service';
import { checkFonts, parseFonts, serializeFonts, withOption, type Font, type FontsFile, type Glyph } from './font-file';
import * as edit from './glyph-edit';
import type { PointRef, SegmentRef } from './glyph-edit';
import { formatPath, inkExtent, parsePath, type GlyphPath, type Segment } from './glyph-path';

const DRAFT_KEY = 'journal-editor.fonts-draft';
/** How long edits pause before journals are given them. */
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

interface Draft {
  name: string;
  text: string;
  font: string;
}

/** Where a font's guides go, in glyph units (y down). */
export interface Guides {
  cap?: number;
  xHeight?: number;
  baseline?: number;
  descender?: number;
}

/**
 * The fonts being edited: the whole fonts.json, the font and glyph variant
 * shown, the tool in hand, and undo. Every change that leaves the fonts
 * valid is given to journals (the workspace), once edits pause.
 */
@Injectable()
export class FontEditorService {
  private readonly tools = inject(ToolsService);
  private readonly files = inject(FileService);
  private readonly workspace = inject(WorkspaceService);
  private readonly shell = inject(ShellService);

  readonly file = signal<FontsFile | null>(null);
  readonly fileName = signal('fonts.json');
  /** Why the fonts couldn't be loaded, if they couldn't. */
  readonly loadError = signal<string | null>(null);
  /** The built-in fonts.json, once fetched. */
  readonly builtin = signal<string | null>(null);
  private handle: OpenedFile['handle'] = null;
  /** The file as last loaded or exported. */
  private readonly savedText = signal<string | null>(null);

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
  readonly dirty = computed(() => this.text() !== null && this.text() !== this.savedText());
  readonly problems = computed(() => {
    const file = this.file();
    return file ? checkFonts(file) : [];
  });
  /** Whether the fonts are the built-in ones, unchanged. */
  readonly unchanged = computed(() => this.text() !== null && this.text() === this.builtin());
  /** The built-in fonts, read; null until fetched. */
  private readonly builtinFile = computed(() => {
    const builtin = this.builtin();
    return builtin === null ? null : parseFonts(builtin);
  });
  /** The built-in font of the shown font's name; null if there's none. */
  readonly builtinFont = computed(() => this.builtinFile()?.find((font) => font.name === this.fontName()) ?? null);
  /** Whether the font shown is as built in (or isn't built in, so has nothing to go back to). */
  readonly fontUnchanged = computed(() => {
    const font = this.font();
    const builtin = this.builtinFont();
    return !font || !builtin || serializeFonts([font]) === serializeFonts([builtin]);
  });

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
    // Give journals the fonts once edits pause, if they're valid; the
    // built-in fonts unchanged are no edit at all.
    effect(() => {
      const text = this.text();
      const ok = this.problems().length === 0;
      const builtin = this.builtin();
      if (text === null || !ok || builtin === null) return;
      if (this.pushTimer) clearTimeout(this.pushTimer);
      this.pushTimer = setTimeout(() => {
        this.workspace.setFonts(text === builtin ? null : text, hash(builtin));
      }, PUSH_DELAY_MS);
    });
    // Keep the draft between visits.
    effect(() => {
      const text = this.text();
      const font = this.fontName();
      if (text === null) return;
      const draft: Draft = { name: this.fileName(), text, font };
      try {
        localStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
      } catch {
        // No storage: the draft lasts this visit.
      }
    });
    // Another editor asked for a font: show it, once the fonts are loaded.
    effect(() => {
      const name = this.shell.fontRequest();
      const file = this.file();
      if (name === null || !file) return;
      untracked(() => {
        if (file.some((font) => font.name === name)) this.pickFont(name);
        this.shell.fontRequest.set(null);
      });
    });
    // A different glyph or variant: nothing of it is picked.
    effect(() => {
      this.selection();
      this.fontName();
      this.picked.set([]);
      this.pickedSegment.set(null);
    });
    void this.start();
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

  /** Pick up where the last visit left off, or start from the built-in fonts. */
  private async start() {
    const builtin = await this.tools.builtinFonts().catch((err: Error) => {
      this.loadError.set(`the renderer didn't load: ${err.message}`);
      return null;
    });
    this.builtin.set(builtin);
    const draft = readDraft();
    const text = draft?.text ?? this.workspace.fonts() ?? builtin;
    if (text === null) return;
    this.load(text, draft?.name ?? 'fonts.json', null, draft?.font);
    // The draft was saved as it was; it's only unsaved if it differs from the built-in fonts.
    if (builtin !== null) this.savedText.set(builtin);
    if (this.workspace.fonts() !== null && this.workspace.fontsBase() !== null && builtin !== null && this.workspace.fontsBase() !== hash(builtin)) {
      this.loadError.set('webapp/config/fonts.json has changed since these edits began; they may undo newer changes. Revert to the built-in fonts to start again from it.');
    }
  }

  /** Show `text` as the fonts file `name`. */
  load(text: string, name: string, handle: OpenedFile['handle'], font?: string) {
    let file: FontsFile;
    try {
      file = parseFonts(text);
    } catch (err) {
      this.loadError.set(`${name} isn't a fonts file: ${(err as Error).message}`);
      return;
    }
    this.loadError.set(null);
    this.file.set(file);
    this.fileName.set(name);
    this.handle = handle;
    this.savedText.set(serializeFonts(file));
    this.undoStack = [];
    this.redoStack = [];
    this.updateHistory();
    const names = file.map((f) => f.name);
    this.pickFont(font && names.includes(font) ? font : names.includes(this.fontName()) ? this.fontName() : (names[0] ?? ''));
  }

  async open() {
    const opened = await this.files.open(JSON_FILES);
    if (opened) this.load(opened.text, opened.name, opened.handle);
  }

  /** Put the font shown back as it is in webapp/config/fonts.json, as one undo step; the other fonts keep their edits. */
  revertFont() {
    const builtin = this.builtinFont();
    if (!builtin || this.fontUnchanged()) return;
    this.updateFont(() => structuredClone(builtin));
    this.settle();
    this.keepSelection();
  }

  /** Start again from webapp/config/fonts.json as built in, every font. */
  revert() {
    const builtin = this.builtin();
    if (builtin === null) return;
    this.load(builtin, 'fonts.json', null);
    this.workspace.setFonts(null);
  }

  /** Save the fonts file where it came from (or where the user picks), or download it. */
  async export() {
    const text = this.text();
    if (text === null) return;
    const saved = await this.files.save(text, this.fileName(), this.handle, JSON_FILES);
    if (!saved) return;
    this.handle = saved.handle;
    this.fileName.set(saved.name);
    this.savedText.set(text);
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
    this.pickFont(name);
    return null;
  }

  renameFont(name: string): string | null {
    const file = this.file();
    const old = this.fontName();
    if (!file || name === old) return null;
    const problem = fontNameProblem(file, name);
    if (problem) return problem;
    const raw = JSON.stringify(old);
    // Fonts falling back to it follow it to its new name.
    this.update((f) => f.map((font) => {
      const renamed = font.name === old ? { ...font, name } : font;
      return renamed.options.some(([k, v]) => k === 'fallback' && v === raw) ? withOption(renamed, 'fallback', JSON.stringify(name)) : renamed;
    }));
    this.fontName.set(name);
    return null;
  }

  deleteFont() {
    const name = this.fontName();
    this.update((file) => file.filter((font) => font.name !== name));
    const first = this.file()?.[0]?.name;
    if (first !== undefined) this.pickFont(first);
  }
}

function fontNameProblem(file: FontsFile, name: string): string | null {
  if (!name) return 'a font needs a name';
  if (file.some((font) => font.name === name)) return `there's already a font named '${name}'`;
  return null;
}

function readDraft(): Draft | null {
  try {
    const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null');
    return draft && typeof draft.text === 'string' && typeof draft.name === 'string' ? draft : null;
  } catch {
    return null;
  }
}
