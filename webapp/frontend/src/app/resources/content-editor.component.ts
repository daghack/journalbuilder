import { Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { splitPath } from '../journal/journal-files';
import { readSymbols, symbolHelp, symbolsOf, textOf, writeSymbols } from '../journal/symbols';
import type { Preview } from '../scribe';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ToolsService } from '../services/tools.service';
import { ShellService } from '../shell/shell.service';
import { LibraryBannerComponent } from './library-banner.component';

/** How long typing pauses before the preview is drawn again. */
const PREVIEW_DELAY_MS = 350;

/** An entry script writing content `name` (marked up, if `markdown`) in the library's plain hand. */
function previewScript(name: string, markdown: boolean): string {
  const text = markdown ? `markdown(content(${JSON.stringify(name)}))` : `content(${JSON.stringify(name)})`;
  return `journal_entry.add(new_layout("content", |n, info| column([flow("text", #{ flex: #{ grow: 1 } })], #{ flex: #{ padding: 70 } })).typeset(#{
  width: 1100, height: 1500, max_pages: 500,
  flows: #{ text: ${text} },
  roles: #{ text: text_style("plain_hand") },
}));`;
}

/**
 * A content file: plain or marked-up text, typed, and drawn as an entry
 * would write it; or symbols, typed as text in their base. Nobody edits
 * the JSON a symbols file is kept as.
 */
@Component({
  selector: 'app-content-editor',
  imports: [LibraryBannerComponent],
  template: `
    <app-library-banner [path]="path()" [own]="own()" [builtin]="builtin()" (copy)="copyIn()" (revert)="revert()" />
    @if (ext() === 'json') {
      <div class="symbols">
        <label class="row">Base
          <select (change)="setBase(+$any($event.target).value)">
            @for (b of bases; track b) { <option [value]="b" [selected]="b === symbols()?.base">{{ b }}</option> }
          </select>
          <span class="hint">{{ symbolHelp(symbols()?.base ?? 27) }}</span>
        </label>
        <label class="column">The symbols, as text
          <textarea class="mono" rows="6" spellcheck="false" [value]="typed() ?? symbolText()" (input)="setSymbolText($any($event.target).value)"></textarea>
        </label>
        @if (problem(); as p) { <p class="error">{{ p }}</p> }
        @if (symbols(); as s) {
          <p class="hint">{{ s.symbols.length }} symbol{{ s.symbols.length === 1 ? '' : 's' }} in base {{ s.base }}: <code>{{ s.symbols.join(' ') }}</code></p>
          <p class="hint">An entry reads it with content("{{ name() }}"): symbols, as symbols({{ s.base }}, text) gives them, for a lock.</p>
        }
      </div>
    } @else {
      <div class="split">
        <section class="preview" aria-label="Preview">
          @if (previewUrl(); as url) { <img [src]="url" alt="The content, written" /> }
          @if (preview()?.error; as error) { <p class="error">{{ error.message }}</p> }
          <p class="hint">Written in the library's plain_hand. An entry reads it with content("{{ name() }}"){{ ext() === 'md' ? ', and sets it marked up with markdown(content("' + name() + '"))' : '' }}.</p>
        </section>
        <textarea class="text" spellcheck="true" aria-label="The text" [value]="text()" (input)="setText($any($event.target).value)"></textarea>
      </div>
    }
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-height: 0; height: 100%; }
    .split { flex: 1; display: grid; grid-template-columns: minmax(0, 1fr) minmax(320px, 1fr); min-height: 0; }
    .preview { overflow: auto; padding: 12px; background: var(--stage-bg); display: flex; flex-direction: column; gap: 8px; }
    img { width: 100%; max-width: 700px; background: var(--paper); border-radius: 4px; box-shadow: 0 1px 6px rgb(0 0 0 / 0.25); }
    .text { resize: none; border: 0; border-left: 1px solid var(--border); padding: 12px; font: 14px/1.6 var(--font); background: var(--editor-bg); color: var(--editor-fg); }
    .symbols { padding: 14px; display: flex; flex-direction: column; gap: 10px; max-width: 760px; overflow: auto; }
    .row { display: flex; align-items: center; gap: 8px; }
    .column { display: flex; flex-direction: column; gap: 4px; }
    .mono, code { font-family: var(--mono); }
    .error { color: var(--danger); margin: 0; white-space: pre-wrap; font-size: 12px; }
  `,
})
export class ContentEditorComponent {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly tools = inject(ToolsService);
  private readonly shell = inject(ShellService);

  protected readonly bases = Array.from({ length: 26 }, (_, i) => i + 2);
  protected readonly symbolHelp = symbolHelp;
  protected readonly path = computed(() => {
    const path = this.shell.selected();
    return path && splitPath(path)?.kind === 'content' ? path : null;
  });
  protected readonly name = computed(() => splitPath(this.path() ?? '')?.name ?? '');
  protected readonly ext = computed(() => splitPath(this.path() ?? '')?.ext ?? 'txt');
  protected readonly own = computed(() => (this.path() ?? '') in this.store.files());
  protected readonly builtin = computed(() => this.library.has(this.path() ?? ''));
  protected readonly text = computed(() => {
    const path = this.path();
    return path ? (this.store.files()[path] ?? this.library.files()[path] ?? '') : '';
  });

  // ---- symbols ----

  protected readonly symbols = computed(() => {
    try {
      return readSymbols(this.text());
    } catch {
      return null;
    }
  });
  protected readonly symbolText = computed(() => {
    const s = this.symbols();
    return s ? textOf(s.base, s.symbols) : '';
  });
  /** Text typed that isn't symbols in the base, kept as typed until it is. */
  protected readonly typed = signal<string | null>(null);
  protected readonly problem = signal<string | null>(null);

  // ---- text ----

  protected readonly preview = signal<Preview | null>(null);
  protected readonly previewUrl = computed(() => {
    const svg = this.preview()?.svg;
    return svg ? URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })) : null;
  });
  private timer: ReturnType<typeof setTimeout> | null = null;
  private asked = 0;
  private shownUrl: string | null = null;

  constructor() {
    effect(() => {
      this.path();
      untracked(() => {
        this.typed.set(null);
        this.problem.set(this.text() && !this.symbols() && this.ext() === 'json' ? "the file isn't symbols: retype them below" : null);
      });
    });
    effect(() => {
      const path = this.path();
      const text = this.text();
      const ext = this.ext();
      if (this.timer) clearTimeout(this.timer);
      if (!path || ext === 'json') return;
      this.timer = setTimeout(() => void this.draw(path, text, ext === 'md'), PREVIEW_DELAY_MS);
    });
    effect(() => {
      const url = this.previewUrl();
      if (this.shownUrl && this.shownUrl !== url) URL.revokeObjectURL(this.shownUrl);
      this.shownUrl = url;
    });
  }

  private async draw(path: string, text: string, markdown: boolean) {
    const asked = ++this.asked;
    const preview = await this.tools.preview(previewScript(this.name(), markdown), { [path]: text })
      .catch((err: Error) => ({ svg: null, view: null, warnings: [], missingGlyphs: null, error: { message: err.message } }));
    if (asked === this.asked) this.preview.set(preview);
  }

  private write(text: string) {
    const path = this.path();
    if (path && text !== this.text()) this.store.setFile(path, text);
  }

  protected setText(text: string) {
    this.write(text);
  }

  protected setSymbolText(text: string) {
    const base = this.symbols()?.base ?? 27;
    try {
      this.write(writeSymbols({ base, symbols: symbolsOf(base, text) }));
      this.typed.set(null);
      this.problem.set(null);
    } catch (err) {
      this.typed.set(text);
      this.problem.set((err as Error).message);
    }
  }

  protected setBase(base: number) {
    const text = this.typed() ?? this.symbolText();
    try {
      this.write(writeSymbols({ base, symbols: symbolsOf(base, text) }));
      this.typed.set(null);
      this.problem.set(null);
    } catch (err) {
      // The symbols don't fit the new base: keep them as typed, to fix.
      this.write(writeSymbols({ base, symbols: [] }));
      this.typed.set(text);
      this.problem.set((err as Error).message);
    }
  }

  protected copyIn() {
    const path = this.path();
    if (path && !this.own()) this.store.setFile(path, this.text());
  }

  protected revert() {
    const path = this.path();
    if (path && this.own() && this.builtin()) this.store.removeFile(path);
  }
}
