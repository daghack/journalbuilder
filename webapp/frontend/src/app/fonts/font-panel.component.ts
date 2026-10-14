import { Component, computed, effect, inject, signal } from '@angular/core';
import { FontEditorService } from './font-editor.service';
import { FALLBACK_GLYPH, fallbackLoop, fallbackOf, optionRaw, parseFonts, withOption } from './font-file';

/** The right: which fonts are open and which font is shown, its options, the glyph drawn, and the export. */
@Component({
  selector: 'app-font-panel',
  template: `
    <section class="panel-section">
      <h3>Fonts file</h3>
      <div class="row">
        <span class="name">{{ editor.fileName() }}@if (editor.dirty()) {<span class="dirty" title="Changes not exported"> ●</span>}</span>
      </div>
      <div class="row">
        <button type="button" class="primary" (click)="editor.export()" [disabled]="!editor.file() || editor.problems().length > 0"
                title="Save the whole fonts file: back where it came from in Chromium browsers, else as a download. Put it in config/fonts.json.">Export fonts.json</button>
        <button type="button" (click)="editor.open()" title="Open a fonts file">Open…</button>
        <button type="button" (click)="revert()" [disabled]="editor.unchanged()" title="Start again from config/fonts.json as built in">Revert</button>
      </div>
      @if (editor.loadError(); as error) { <p class="note-error">{{ error }}</p> }
      <p class="hint">
        @if (editor.problems().length) {
          Journals keep the last fonts without problems until these are fixed.
        } @else if (editor.unchanged()) {
          These are the built-in fonts. Edits apply to journals as you make them.
        } @else {
          Journals use these fonts as edited (in this browser only, until exported).
        }
      </p>
    </section>

    @if (editor.file(); as file) {
      <section class="panel-section">
        <h3>Font</h3>
        <select aria-label="Font" (change)="editor.pickFont($any($event.target).value)">
          @for (font of file; track font.name) {
            <option [value]="font.name" [selected]="font.name === editor.fontName()">{{ font.name }} ({{ font.glyphs.length }})</option>
          }
        </select>
        <div class="row">
          <input type="text" aria-label="Font name" placeholder="name" [value]="nameDraft() ?? editor.fontName()" (input)="nameDraft.set($any($event.target).value)" (keydown.enter)="rename()" />
          <button type="button" (click)="rename()" [disabled]="nameDraft() === null || nameDraft() === editor.fontName()">Rename</button>
        </div>
        <div class="row">
          <input type="text" aria-label="New font's name" placeholder="new font's name" [value]="newName()" (input)="newName.set($any($event.target).value)" />
          <button type="button" (click)="add(false)" [disabled]="!newName()" title="A new font with no glyphs">New</button>
          <button type="button" (click)="add(true)" [disabled]="!newName()" title="A copy of this font">Duplicate</button>
        </div>
        @if (fontError(); as error) { <p class="problem">{{ error }}</p> }
        <button type="button" class="danger" (click)="editor.deleteFont()" [disabled]="file.length < 2">Delete {{ editor.fontName() }}</button>
      </section>

      @if (editor.font(); as font) {
        <section class="panel-section">
          <h3>Options</h3>
          <label class="field">
            <span>character_padding</span>
            <input type="number" step="0.01" min="0" placeholder="0" [value]="numeric('character_padding')"
                   (change)="setNumber('character_padding', $any($event.target).value)" />
            <span class="hint">Room between neighbouring glyphs' outlines, in font sizes.</span>
            @if (problem('character_padding'); as p) { <span class="problem">{{ p }}</span> }
          </label>
          <label class="field">
            <span>space_width</span>
            <input type="number" step="0.01" min="0" placeholder="0.5" [value]="numeric('space_width')"
                   (change)="setNumber('space_width', $any($event.target).value)" />
            <span class="hint">A space's width, in font sizes (0.5 if not set).</span>
            @if (problem('space_width'); as p) { <span class="problem">{{ p }}</span> }
          </label>
          <label class="field">
            <span>fallback</span>
            <select (change)="setFallback($any($event.target).value)">
              <option value="" [selected]="!fallback()">none</option>
              @for (name of fallbackChoices(); track name.name) {
                <option [value]="name.name" [selected]="name.name === fallback()" [disabled]="name.loops">{{ name.name }}{{ name.loops ? ' (loops)' : '' }}</option>
              }
            </select>
            <span class="hint">Where to look for characters this font hasn't got.</span>
            @if (problem('fallback'); as p) { <span class="problem">{{ p }}</span> }
          </label>
        </section>
      }

      @if (editor.glyph(); as glyph) {
        <section class="panel-section">
          <h3>Glyph</h3>
          <div class="row">
            <input type="text" class="key" aria-label="Glyph key" [value]="keyDraft() ?? glyph.key" (input)="keyDraft.set($any($event.target).value)" (keydown.enter)="renameGlyph()" />
            <button type="button" (click)="renameGlyph()" [disabled]="keyDraft() === null || keyDraft() === glyph.key">Rename</button>
            <span class="hint">variant {{ (editor.selection()?.variant ?? 0) + 1 }} of {{ glyph.variants.length }}</span>
          </div>
          @if (glyphError(); as error) { <p class="problem">{{ error }}</p> }
          <div class="row">
            <button type="button" (click)="editor.duplicateVariant()" title="Add a copy of this variant to draw differently">Duplicate variant</button>
            <button type="button" (click)="editor.deleteVariant()" [disabled]="glyph.variants.length < 2">Delete variant</button>
            <button type="button" class="danger" (click)="editor.deleteGlyph()">Delete glyph</button>
          </div>
          <label class="stack">
            <span class="hint">Path data (SVG: M, L, Q, C, Z; y down, about −1 to 1)</span>
            <textarea rows="5" spellcheck="false" [class.invalid]="!!editor.path()?.error" [value]="editor.variant() ?? ''"
                      (input)="editor.setVariantText($any($event.target).value, 'path-text')" (blur)="editor.settle()"></textarea>
          </label>
          @if (editor.path()?.error; as error) { <p class="problem">{{ error }}</p> }
        </section>
      }

      @if (notes().length || editor.problems().length) {
        <section class="panel-section">
          <h3>Notes</h3>
          @for (p of editor.problems(); track $index) {
            <p class="note-error">{{ p.font }}{{ p.glyph !== undefined ? ' ' + p.glyph : '' }}: {{ p.message }}</p>
          }
          @for (note of notes(); track $index) { <p class="note-warning">{{ note }}</p> }
        </section>
      }
    }
  `,
  styles: `
    :host { display: block; overflow-y: auto; background: var(--surface); min-height: 0; }
    .name { font: 600 12px var(--mono); }
    .dirty { color: var(--accent); }
    p { margin: 0; }
    .row input[type='text'] { flex: 1; }
    .key { font-family: var(--mono); max-width: 9em; }
    .stack { display: flex; flex-direction: column; gap: 4px; }
    textarea { font-family: var(--mono); font-size: 11px; resize: vertical; }
    button.danger { color: var(--danger); align-self: flex-start; }
  `,
})
export class FontPanelComponent {
  protected readonly editor = inject(FontEditorService);
  protected readonly nameDraft = signal<string | null>(null);
  protected readonly newName = signal('');
  protected readonly fontError = signal<string | null>(null);
  protected readonly keyDraft = signal<string | null>(null);
  protected readonly glyphError = signal<string | null>(null);

  constructor() {
    // Another font or glyph shown: what was typed for the last one's name goes.
    effect(() => {
      this.editor.fontName();
      this.nameDraft.set(null);
      this.fontError.set(null);
    });
    effect(() => {
      this.editor.selection();
      this.keyDraft.set(null);
      this.glyphError.set(null);
    });
  }

  protected readonly fallback = computed(() => {
    const font = this.editor.font();
    return font ? fallbackOf(font) : undefined;
  });

  protected readonly fallbackChoices = computed(() => {
    const file = this.editor.file() ?? [];
    const name = this.editor.fontName();
    return file.filter((font) => font.name !== name).map((font) => {
      const tried = file.map((f) => (f.name === name ? withOption(f, 'fallback', JSON.stringify(font.name)) : f));
      return { name: font.name, loops: fallbackLoop(tried, name) !== null };
    });
  });

  /** The built-in fonts' names, to tell new and renamed fonts by. */
  private readonly builtinNames = computed(() => {
    const builtin = this.editor.builtin();
    try {
      return builtin ? parseFonts(builtin).map((font) => font.name) : [];
    } catch {
      return [];
    }
  });

  protected readonly notes = computed(() => {
    const notes: string[] = [];
    const file = this.editor.file() ?? [];
    const builtin = this.builtinNames();
    for (const name of builtin) {
      if (!file.some((font) => font.name === name)) notes.push(`There's no font '${name}' any more: scripts calling font("${name}") will fail.`);
    }
    const symbols = file.find((font) => font.name === FALLBACK_GLYPH.font);
    if (!symbols?.glyphs.some((glyph) => glyph.key === FALLBACK_GLYPH.key)) {
      notes.push(`Characters no font has are drawn with ${FALLBACK_GLYPH.font}'s ${FALLBACK_GLYPH.key}, which isn't there: the renderer will fail on them.`);
    }
    for (const font of file) {
      if (!builtin.includes(font.name) && font.glyphs.length >= 20 && !font.name.startsWith('debug')) {
        notes.push(`'${font.name}' is new, with ${font.glyphs.length} glyphs: the showcase needs a line of it (tests/showcase.rs checks).`);
      }
    }
    return notes;
  });

  protected numeric(key: string): string {
    const font = this.editor.font();
    const raw = font ? optionRaw(font, key) : undefined;
    return raw ?? '';
  }

  protected problem(option: string): string | null {
    return this.editor.problems().find((p) => p.font === this.editor.fontName() && p.option === option)?.message ?? null;
  }

  protected setNumber(key: string, value: string) {
    const text = value.trim();
    this.editor.setOption(key, text === '' ? undefined : /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(text) ? JSON.stringify(Number(text)) : text);
    this.editor.settle();
  }

  protected setFallback(name: string) {
    this.editor.setOption('fallback', name ? JSON.stringify(name) : undefined);
    this.editor.settle();
  }

  protected rename() {
    const name = this.nameDraft();
    if (name === null) return;
    const error = this.editor.renameFont(name.trim());
    this.fontError.set(error);
    if (!error) this.nameDraft.set(null);
  }

  protected add(copy: boolean) {
    const error = this.editor.addFont(this.newName().trim(), copy ? this.editor.font() : null);
    this.fontError.set(error);
    if (!error) this.newName.set('');
  }

  protected renameGlyph() {
    const key = this.keyDraft();
    if (key === null) return;
    const error = this.editor.renameGlyph(key);
    this.glyphError.set(error);
    if (!error) this.keyDraft.set(null);
  }

  protected revert() {
    if (this.editor.unchanged() || confirm('Throw away every edit to the fonts and start again from config/fonts.json?')) this.editor.revert();
  }
}
