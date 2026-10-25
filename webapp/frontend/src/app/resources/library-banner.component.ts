import { Component, input, output } from '@angular/core';

/**
 * Above a file's editor, when the library is involved: the library's file
 * (changing it makes the journal a copy that stands in for it), or the
 * journal's copy standing in for the library's (removable, to use the
 * library's again).
 */
@Component({
  selector: 'app-library-banner',
  template: `
    @if (path(); as path) {
      @if (!own() && builtin()) {
        <p class="banner library">
          <span>The library's <code>{{ path }}</code>. Changing it gives the journal a copy, which stands in for it.</span>
          <button type="button" (click)="copy.emit()">Copy into the journal</button>
        </p>
      } @else if (own() && builtin()) {
        <p class="banner override">
          <span><b>⚠</b> The journal's <code>{{ path }}</code> stands in for the library's file of that name.</span>
          <button type="button" (click)="revert.emit()" title="Remove the journal's copy">Use the library's</button>
        </p>
      }
    }
  `,
  styles: `
    :host { display: block; }
    .banner { display: flex; align-items: center; gap: 10px; margin: 0; padding: 6px 10px; font-size: 12px; border-bottom: 1px solid var(--border); }
    .banner span { flex: 1; }
    .library { background: var(--surface-2); color: var(--fg); }
    .override { background: var(--warning-bg); color: var(--fg); }
    .override b { color: var(--warning); }
    code { font-family: var(--mono); }
  `,
})
export class LibraryBannerComponent {
  readonly path = input<string | null>(null);
  readonly own = input(false);
  readonly builtin = input(false);
  readonly copy = output<void>();
  readonly revert = output<void>();
}
