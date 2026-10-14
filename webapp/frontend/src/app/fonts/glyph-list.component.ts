import { Component, ElementRef, computed, effect, inject, signal, viewChildren } from '@angular/core';
import { FontEditorService } from './font-editor.service';
import { bounds, parsePath } from './glyph-path';

/** The left: every glyph of the font, each with its variants drawn small; pick one to draw. */
@Component({
  selector: 'app-glyph-list',
  template: `
    <div class="head">
      <input type="text" placeholder="Find or add a glyph" aria-label="Find or add a glyph" [value]="filter()"
             (input)="filter.set($any($event.target).value)" (keydown.enter)="canAdd() && add()" />
      <button type="button" [disabled]="!canAdd()" (click)="add()" [title]="canAdd() ? 'Add the glyph ' + filter() : 'Type a key that is new to the font to add it'">Add</button>
    </div>
    @if (addError(); as error) { <p class="error">{{ error }}</p> }
    <ol class="glyphs" role="listbox" aria-label="Glyphs">
      @for (row of rows(); track row.index) {
        <li #row class="glyph" [class.selected]="row.index === editor.selection()?.glyph">
          <span class="key" [title]="row.key">{{ row.key }}</span>
          <span class="variants">
            @for (d of row.variants; track $index) {
              <button type="button" class="variant" role="option" [attr.aria-selected]="row.index === editor.selection()?.glyph && $index === editor.selection()?.variant"
                      [class.on]="row.index === editor.selection()?.glyph && $index === editor.selection()?.variant"
                      (click)="editor.select(row.index, $index)" [title]="row.key + ', variant ' + ($index + 1)">
                <svg [attr.viewBox]="frame()"><path [attr.d]="d" /></svg>
              </button>
            }
          </span>
        </li>
      } @empty {
        <li class="none">{{ editor.font() ? 'No glyphs' + (filter() ? ' match' : ' yet') : 'Loading the fonts…' }}</li>
      }
    </ol>
  `,
  styles: `
    :host { display: flex; flex-direction: column; background: var(--rail-bg); color: var(--rail-fg); min-height: 0; }
    .head { display: flex; gap: 6px; padding: 8px; }
    .head input { flex: 1; }
    .error { margin: 0 8px 6px; font-size: 11px; color: #f2a59c; }
    .glyphs { list-style: none; margin: 0; padding: 0 8px 12px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
    .glyph { display: flex; gap: 6px; align-items: center; padding: 4px; border-radius: 6px; border: 1px solid transparent; }
    .glyph.selected { background: var(--rail-selected-bg); border-color: var(--rail-selected); }
    .key { width: 2.6em; flex: none; font: 12px var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: center; }
    .variants { display: flex; flex-wrap: wrap; gap: 4px; }
    .variant { padding: 0; width: 38px; height: 38px; background: var(--paper); border: 2px solid transparent; border-radius: 4px; }
    .variant.on { border-color: var(--accent); }
    .variant svg { width: 100%; height: 100%; display: block; }
    .variant path { fill: none; stroke: #2b1d12; stroke-width: 0.08; stroke-linecap: round; stroke-linejoin: round; }
    .none { font-size: 12px; color: var(--muted); padding: 8px; }
  `,
})
export class GlyphListComponent {
  protected readonly editor = inject(FontEditorService);
  protected readonly filter = signal('');
  protected readonly addError = signal<string | null>(null);
  private readonly rowElements = viewChildren<ElementRef<HTMLElement>>('row');

  protected readonly rows = computed(() => {
    const font = this.editor.font();
    const filter = this.filter().trim();
    if (!font) return [];
    return font.glyphs
      .map((glyph, index) => ({ index, key: glyph.key, variants: glyph.variants }))
      .filter((row) => !filter || row.key.includes(filter));
  });

  /** One frame for every thumbnail of the font, so glyphs keep their sizes against each other. */
  protected readonly frame = computed(() => {
    const font = this.editor.font();
    let box = { minX: -1, minY: -1, maxX: 1, maxY: 1 };
    for (const glyph of font?.glyphs ?? []) {
      try {
        const b = bounds(parsePath(glyph.variants[0]));
        if (b) box = { minX: Math.min(box.minX, b.minX), minY: Math.min(box.minY, b.minY), maxX: Math.max(box.maxX, b.maxX), maxY: Math.max(box.maxY, b.maxY) };
      } catch {
        // Unreadable: the panel says so.
      }
    }
    const pad = 0.1;
    const size = Math.max(box.maxX - box.minX, box.maxY - box.minY) + 2 * pad;
    const cx = (box.minX + box.maxX) / 2, cy = (box.minY + box.maxY) / 2;
    return `${cx - size / 2} ${cy - size / 2} ${size} ${size}`;
  });

  constructor() {
    // Keep the glyph being drawn in view.
    effect(() => {
      const glyph = this.editor.selection()?.glyph;
      const index = this.rows().findIndex((row) => row.index === glyph);
      this.rowElements()[index]?.nativeElement.scrollIntoView({ block: 'nearest' });
    });
  }

  /** Whether what's typed is a key the font hasn't got. */
  protected readonly canAdd = computed(() => {
    const key = this.filter();
    return key !== '' && !this.editor.font()?.glyphs.some((glyph) => glyph.key === key);
  });

  protected add() {
    const error = this.editor.addGlyph(this.filter());
    this.addError.set(error);
    if (!error) this.filter.set('');
  }
}
