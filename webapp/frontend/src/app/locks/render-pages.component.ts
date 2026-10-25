import { Component, computed, effect, input, type OnDestroy } from '@angular/core';
import type { RenderPreview } from '../scribe';

/** What an illustration (or a layout or style) draws: each canvas it gives, as a page. */
@Component({
  selector: 'app-render-pages',
  template: `
    @if (preview(); as p) {
      @if (error(); as e) {
        <p class="error">{{ e }}</p>
      } @else {
        <div class="pages" [class.large]="large()">
          @for (url of urls(); track $index) {
            @if (url) { <img [src]="url" [alt]="'Canvas ' + ($index + 1)" /> } @else { <span class="blank">nothing drawn</span> }
          }
        </div>
        <p class="hint">{{ urls().length }} canvas{{ urls().length === 1 ? '' : 'es' }}</p>
      }
    }
  `,
  styles: `
    :host { display: block; }
    .pages { display: grid; grid-template-columns: repeat(auto-fill, minmax(110px, 1fr)); gap: 6px; }
    .pages.large { grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 12px; }
    img { width: 100%; box-sizing: border-box; padding: 5px; background: var(--paper); border-radius: 4px; box-shadow: 0 1px 4px rgb(0 0 0 / 0.25); }
    .blank { font-size: 11px; color: var(--muted); }
    .error { margin: 0; color: var(--danger); font-size: 12px; white-space: pre-wrap; }
    .hint { margin: 4px 0 0; }
  `,
})
export class RenderPagesComponent implements OnDestroy {
  readonly preview = input<RenderPreview | { error: string } | null>(null);
  /** Pages shown large, a file's preview, rather than as thumbnails. */
  readonly large = input(false);
  protected readonly error = computed(() => {
    const p = this.preview();
    return p && 'error' in p ? p.error : null;
  });
  protected readonly urls = computed(() => {
    const p = this.preview();
    if (!p || 'error' in p) return [];
    return p.pages.map((page) => (page.svg ? URL.createObjectURL(new Blob([page.svg], { type: 'image/svg+xml' })) : null));
  });
  private shown: (string | null)[] = [];

  constructor() {
    // Let go of the last pages' images once new ones show.
    effect(() => {
      const urls = this.urls();
      for (const url of this.shown) if (url && !urls.includes(url)) URL.revokeObjectURL(url);
      this.shown = urls;
    });
  }

  ngOnDestroy() {
    for (const url of this.shown) if (url) URL.revokeObjectURL(url);
  }
}
