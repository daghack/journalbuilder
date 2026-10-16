import { Component, computed, effect, inject, signal, type OnDestroy } from '@angular/core';
import { ToolsService } from '../services/tools.service';
import { rhaiFloat, rhaiString } from '../shared/rhai-literal';
import type { Preview } from '../scribe';
import { FontEditorService } from './font-editor.service';
import { isTernary } from './font-file';

const TEXT_KEY = 'journal-editor.preview-text';

/**
 * A journal script that sets `text` in `font`, a line of the text to a
 * line of the page, as journals do (fill_with_text): the font editor's
 * preview.
 */
export function previewScript(font: string, ternary: boolean, text: string, size: number, seed: number): string {
  const lines = [
    `seed(${Math.round(seed)});`,
    'let style = new_style(1.0, "#2b1d12", new_brush("Ink Pen", 0.4, 0.8, 2.2));',
    'let page = new_page();',
    `let f = ${ternary ? 'ternary_font' : 'font'}(${rhaiString(font)});`,
  ];
  text.split('\n').forEach((line, i) => {
    if (!line.trim()) return;
    // Long enough for the widest glyphs: a short path fails to fit the text.
    const length = (line.length + 2) * size * 2;
    const y = rhaiFloat(i * size * 2.4);
    lines.push(`for part in new_line(vec2(0.0, ${y}), vec2(${rhaiFloat(length)}, ${y})).fill_with_text(${rhaiString(line)}, f, ${rhaiFloat(size)}, style) { page.add_canvas(part); }`);
  });
  lines.push('journal_entry.add_page(page);');
  return lines.join('\n') + '\n';
}

/** Under the glyph canvas: text typed, set in the font as edited, as a journal would set it. */
@Component({
  selector: 'app-text-preview',
  template: `
    <div class="bar">
      <textarea rows="2" aria-label="Text to preview" placeholder="Type to see it in the font" [value]="text()" (input)="setText($any($event.target).value)"></textarea>
      <label>Size <input type="number" min="4" max="400" step="2" [value]="size()" (change)="size.set(+$any($event.target).value || 60)" /></label>
      <label title="Which variants are picked: a journal's seed(n)">Seed <input type="number" step="1" [value]="seed()" (change)="seed.set(+$any($event.target).value || 0)" /></label>
      <button type="button" (click)="seed.set(seed() + 1)" title="Pick other variants">Reroll</button>
    </div>
    <div class="sheet">
      @if (url(); as url) {
        <img [src]="url" alt="The text in the font" [class.stale]="busy()" />
      } @else if (!busy()) {
        <p class="muted">{{ text().trim() ? 'Nothing drawn.' : 'Type some text to see it set in the font.' }}</p>
      }
    </div>
    @if (preview()?.error; as error) { <p class="note-error">{{ error.message }}</p> }
    @if (preview()?.missingGlyphs; as missing) { <p class="note-warning">{{ missing }}</p> }
  `,
  styles: `
    :host { display: flex; flex-direction: column; gap: 6px; padding: 8px 10px; background: var(--surface); border-top: 1px solid var(--border); font-size: 12px; min-height: 0; }
    .bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    textarea { flex: 1; min-width: 200px; resize: vertical; font-family: var(--mono); }
    label { display: inline-flex; gap: 4px; align-items: center; }
    input[type='number'] { width: 64px; }
    .sheet { background: var(--paper); border-radius: 4px; min-height: 80px; max-height: 30vh; overflow: auto; display: grid; place-items: center; padding: 10px; box-shadow: inset 0 0 0 1px var(--border); }
    img { max-width: 100%; max-height: calc(30vh - 20px); display: block; }
    img.stale { opacity: 0.6; }
    .muted { color: var(--paper-muted); margin: 0; }
    p { margin: 0; }
  `,
})
export class TextPreviewComponent implements OnDestroy {
  private readonly editor = inject(FontEditorService);
  private readonly tools = inject(ToolsService);

  protected readonly text = signal(readText());
  protected readonly size = signal(60);
  protected readonly seed = signal(0);
  protected readonly preview = signal<Preview | null>(null);
  protected readonly url = signal<string | null>(null);
  protected readonly busy = signal(false);
  private timer: ReturnType<typeof setTimeout> | null = null;
  /** Only the newest preview asked for is shown. */
  private asked = 0;

  private readonly script = computed(() => {
    const font = this.editor.font();
    if (!font) return null;
    return previewScript(font.name, isTernary(font), this.text(), this.size(), this.seed());
  });

  constructor() {
    effect(() => {
      const script = this.script();
      const fonts = this.editor.text();
      if (script === null || fonts === null) return;
      if (this.timer) clearTimeout(this.timer);
      this.timer = setTimeout(() => this.draw(script, fonts), 250);
    });
  }

  protected setText(text: string) {
    this.text.set(text);
    try {
      localStorage.setItem(TEXT_KEY, text);
    } catch {
      // Not kept, then.
    }
  }

  private async draw(script: string, fonts: string) {
    const asked = ++this.asked;
    this.busy.set(true);
    let preview: Preview;
    try {
      preview = await this.tools.preview(script, fonts);
    } catch (err) {
      preview = { svg: null, view: null, warnings: [], missingGlyphs: null, error: { message: (err as Error).message } };
    }
    if (asked !== this.asked) return;
    this.busy.set(false);
    this.preview.set(preview);
    if (preview.error) return;
    const old = this.url();
    this.url.set(preview.svg ? URL.createObjectURL(new Blob([preview.svg], { type: 'image/svg+xml' })) : null);
    if (old) URL.revokeObjectURL(old);
  }

  ngOnDestroy() {
    const url = this.url();
    if (url) URL.revokeObjectURL(url);
  }
}

function readText(): string {
  try {
    return localStorage.getItem(TEXT_KEY) ?? 'The quick brown fox\njumps over the lazy dog 0123456789';
  } catch {
    return 'The quick brown fox';
  }
}
