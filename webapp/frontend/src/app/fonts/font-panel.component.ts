import { Component, computed, effect, inject, signal } from '@angular/core';
import { VALID_NAME, bytesToDataUrl, journalName } from '../journal/journal-files';
import { LibraryBannerComponent } from '../resources/library-banner.component';
import { FontEditorService } from './font-editor.service';
import { FALLBACK_GLYPH, fallbackLoop, fallbackOf, optionRaw, withOption } from './font-file';
import { NEW_REFERENCE_OPACITY, linkedTo, replacePlacement, type Placement } from './glyph-link';
import { PlacementFieldsComponent } from './placement-fields.component';

/** The right: the glyphset shown, its options, and the glyph drawn. */
@Component({
  selector: 'app-font-panel',
  imports: [LibraryBannerComponent, PlacementFieldsComponent],
  template: `
    <app-library-banner [path]="editor.fontPath()" [own]="editor.fontOwn()" [builtin]="!!editor.builtinFont()" (copy)="copyIn()" (revert)="editor.useLibraryFont()" />
    <section class="panel-section">
      @if (editor.loadError(); as error) { <p class="note-error">{{ error }}</p> }
      <p class="hint">
        @if (editor.problems().length) {
          The journal keeps its last glyphsets without problems until these are fixed.
        } @else {
          Entries use the glyphsets as you edit them: each that differs from the library's is a file of the journal's, glyphsets/NAME.json.
        }
      </p>
    </section>

    @if (editor.file(); as file) {
      <section class="panel-section">
        <h3>Glyphset</h3>
        <select aria-label="Glyphset" (change)="editor.openFont($any($event.target).value)">
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
        <div class="row">
          <button type="button" class="danger" (click)="editor.deleteFont()" [disabled]="file.length < 2 || !!editor.builtinFont()"
                  [title]="deleteTitle()">Delete {{ editor.fontName() }}</button>
        </div>
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
            <span class="hint">variant {{ (editor.selection()?.variant ?? 0) + 1 }} of {{ editor.variantCount() }}</span>
          </div>
          @if (glyphError(); as error) { <p class="problem">{{ error }}</p> }
          <div class="row">
            <input type="text" class="key" aria-label="The copy's key" placeholder="copy's key" [value]="copyKey()" (input)="copyKey.set($any($event.target).value)"
                   (keydown.enter)="duplicate(true)" />
            <button type="button" (click)="duplicate(false)" [disabled]="!copyKey()" title="A copy of this glyph under a new key, to change on its own">Duplicate</button>
            <button type="button" (click)="duplicate(true)" [disabled]="!copyKey()"
                    title="A glyph drawn from this one and kept in step with it: move, turn, mirror or scale it, and give it strokes of its own">Linked copy</button>
          </div>
          @if (children().length) {
            <p class="hint">Drawn from it, and changed with it: {{ children().join(', ') }}.</p>
          }
          <div class="row">
            <button type="button" (click)="editor.duplicateVariant()" [disabled]="!!glyph.link" [title]="glyph.link ? 'A linked glyph has its parent’s variants' : 'Add a copy of this variant to draw differently'">Duplicate variant</button>
            <button type="button" (click)="editor.deleteVariant()" [disabled]="!!glyph.link || glyph.variants.length < 2">Delete variant</button>
            <button type="button" class="danger" (click)="editor.deleteGlyph()" [title]="children().length ? 'Delete it: the glyphs drawn from it keep what they draw, as paths of their own' : 'Delete it'">Delete glyph</button>
          </div>
          <label class="stack">
            <span class="hint">{{ glyph.link ? 'Its own strokes, drawn over its parent’s (path data)' : 'Path data (SVG: M, L, Q, C, Z; y down, about −1 to 1)' }}</span>
            <textarea rows="5" spellcheck="false" [class.invalid]="!!editor.path()?.error" [value]="editor.variant() ?? ''"
                      (input)="editor.setVariantText($any($event.target).value, 'path-text')" (blur)="editor.settle()"></textarea>
          </label>
          @if (editor.path()?.error; as error) { <p class="problem">{{ error }}</p> }
        </section>

        @if (glyph.link; as link) {
          <section class="panel-section">
            <h3>Linked</h3>
            <p class="hint">Drawn from another glyph as that is now, placed as here, about the origin, with its own strokes over it. The Parent tool (P) moves, turns and scales it on the canvas.</p>
            <label class="field">
              <span>from</span>
              <select aria-label="The glyph it's drawn from" (change)="relink($any($event.target).value)">
                @for (key of parentChoices(); track key) {
                  <option [value]="key" [selected]="key === link.from">{{ key }}</option>
                }
                @if (!hasParent()) { <option [value]="link.from" selected disabled>{{ link.from }} (gone)</option> }
              </select>
            </label>
            @if (linkError(); as error) { <p class="problem">{{ error }}</p> }
            @if (editor.drawError(); as error) { <p class="problem">{{ error }}</p> }
            <app-placement-fields [placement]="link" (changed)="placeLink($event)" />
            <div class="row">
              <button type="button" (click)="editor.detachGlyph()" [disabled]="!!editor.drawError()"
                      [title]="'Draw it with paths of its own, as it is now: it no longer follows ' + link.from">Detach</button>
            </div>
          </section>
        }

        <section class="panel-section">
          <h3>Reference picture</h3>
          @if (glyph.reference; as ref) {
            <label class="field">
              <span>picture</span>
              <select aria-label="Reference picture" (change)="useImage($any($event.target).value)">
                @for (name of editor.images(); track name) { <option [value]="name" [selected]="name === ref.image">{{ name }}</option> }
                @if (!editor.referenceUrl()) { <option [value]="ref.image" selected disabled>{{ ref.image }} (missing)</option> }
              </select>
            </label>
            @if (!editor.referenceUrl()) { <p class="problem">There's no picture images/{{ ref.image }}.png or .jpg.</p> }
            <label class="field">
              <span>opacity</span>
              <input type="range" min="0.05" max="1" step="0.05" [value]="ref.opacity ?? 1" (input)="setOpacity($any($event.target).value)" (change)="editor.settle()" />
            </label>
            <app-placement-fields [placement]="ref" (changed)="placeReference($event)" />
            <div class="row">
              <button type="button" (click)="importReference()" title="Use a PNG or JPEG of yours instead, added to the journal's images">Import another…</button>
              <button type="button" class="danger" (click)="editor.setReference(undefined)" title="Take the picture away from this glyph (the picture stays in the journal's images)">Remove</button>
            </div>
          } @else {
            <p class="hint">A picture behind the glyph to draw over: fitted in −1 to 1, then moved, turned and scaled with the Reference tool (R). It's kept in the glyphset file; the drawing ignores it.</p>
            <div class="row">
              <button type="button" (click)="importReference()" title="A PNG or JPEG of yours, added to the journal's images">Import picture…</button>
              @if (editor.images().length) {
                <select aria-label="A picture of the journal's" (change)="useImage($any($event.target).value); $any($event.target).value = ''">
                  <option value="" selected>or a picture of the journal's…</option>
                  @for (name of editor.images(); track name) { <option [value]="name">{{ name }}</option> }
                </select>
              }
            </div>
            @if (editor.lastReference(); as last) {
              @if (last.key !== glyph.key) {
                <div class="row">
                  <button type="button" (click)="editor.setReference(last.reference)" [title]="'The picture ' + last.key + ' has, where it has it'">Same as {{ last.key }}'s</button>
                </div>
              }
            }
          }
          @if (referenceError(); as error) { <p class="problem">{{ error }}</p> }
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
  protected readonly copyKey = signal('');
  protected readonly linkError = signal<string | null>(null);
  protected readonly referenceError = signal<string | null>(null);

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
      this.linkError.set(null);
      this.referenceError.set(null);
    });
  }

  /** The glyphs drawn straight from the one shown. */
  protected readonly children = computed(() => {
    const key = this.editor.glyph()?.key;
    return (this.editor.font()?.glyphs ?? []).filter((g) => g.link?.from === key).map((g) => g.key);
  });

  /** The glyphs the one shown could be drawn from: any not drawn from it. */
  protected readonly parentChoices = computed(() => {
    const font = this.editor.font();
    const glyph = this.editor.glyph();
    if (!font || !glyph) return [];
    const own = linkedTo(font, glyph.key);
    return font.glyphs.filter((g) => !own.has(g.key)).map((g) => g.key);
  });
  protected readonly hasParent = computed(() => this.parentChoices().includes(this.editor.link()?.from ?? ''));

  protected duplicate(linked: boolean) {
    const error = this.editor.duplicateGlyph(this.copyKey(), linked);
    this.glyphError.set(error);
    if (!error) this.copyKey.set('');
  }

  protected relink(from: string) {
    this.linkError.set(this.editor.relinkGlyph(from));
  }

  protected placeLink(p: Placement) {
    this.editor.setLink((link) => replacePlacement(link, p));
  }

  protected placeReference(p: Placement) {
    this.editor.updateReference((reference) => replacePlacement(reference, p));
  }

  protected setOpacity(value: string) {
    const opacity = Math.min(1, Math.max(0.05, Number(value)));
    this.editor.updateReference((reference) => ({ ...reference, opacity }), 'reference-opacity');
  }

  /** Picture `name` behind the glyph: in place of the one it has, or fitted in -1..1, faint. */
  protected useImage(name: string) {
    if (!name) return;
    const reference = this.editor.glyph()?.reference;
    this.editor.setReference(reference ? { ...reference, image: name } : { image: name, opacity: NEW_REFERENCE_OPACITY });
    this.editor.settle();
  }

  /** A PNG or JPEG of the user's, added to the journal's images under a name they give, behind the glyph. */
  protected importReference() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/png,image/jpeg';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const ext = file.type === 'image/png' ? 'png' : file.type === 'image/jpeg' ? 'jpg' : null;
      if (!ext) return this.referenceError.set('A reference picture is a PNG or a JPEG.');
      const taken = new Set(this.editor.images());
      const base = journalName(file.name.replace(/\.[^.]*$/, ''), 'reference');
      let suggested = base;
      for (let i = 2; taken.has(suggested); i++) suggested = `${base}_${i}`;
      const typed = prompt("The picture's name, in the journal's images:", suggested)?.trim();
      if (!typed) return;
      if (!VALID_NAME.test(typed)) return this.referenceError.set('A name is letters, digits and _ only.');
      if (taken.has(typed)) return this.referenceError.set(`There's a picture called ${typed} already: pick it from the list, or give this one another name.`);
      this.referenceError.set(null);
      this.editor.addImage(typed, ext, bytesToDataUrl(new Uint8Array(await file.arrayBuffer()), ext));
      this.useImage(typed);
    };
    input.click();
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

  protected readonly notes = computed(() => {
    const notes: string[] = [];
    const file = this.editor.file() ?? [];
    const builtin = this.editor.builtinNames();
    for (const name of builtin) {
      if (!file.some((font) => font.name === name)) notes.push(`There's no glyphset '${name}' any more: scripts calling glyphset("${name}") will fail.`);
    }
    const symbols = file.find((font) => font.name === FALLBACK_GLYPH.font);
    if (!symbols?.glyphs.some((glyph) => glyph.key === FALLBACK_GLYPH.key)) {
      notes.push(`Characters no glyphset has are drawn with ${FALLBACK_GLYPH.font}'s ${FALLBACK_GLYPH.key}, which isn't there: the engine will fail on them.`);
    }
    for (const font of file) {
      if (!builtin.includes(font.name) && font.glyphs.length >= 20 && !font.name.startsWith('debug')) {
        notes.push(`'${font.name}' is new, with ${font.glyphs.length} glyphs: put in the library, the showcase needs a line of it (tests/showcase.rs checks).`);
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

  protected readonly deleteTitle = computed(() => this.editor.builtinFont()
    ? `The library has ${this.editor.fontName()}, so it can't be deleted`
    : `Delete the journal's glyphsets/${this.editor.fontName()}.json`);

  /** Give the journal a copy of the library's glyphset shown, to change. */
  protected copyIn() {
    this.editor.copyFont();
  }
}
