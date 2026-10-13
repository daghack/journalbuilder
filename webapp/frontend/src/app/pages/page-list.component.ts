import { Component, ElementRef, effect, inject, viewChildren } from '@angular/core';
import { JournalService } from '../services/journal.service';

/** The left column: a thumbnail of each page; the one picked shows in the middle. */
@Component({
  selector: 'app-page-list',
  template: `
    <ol class="pages" role="listbox" aria-label="Pages" tabindex="0" (keydown)="onKey($event)">
      @for (page of journal.pages(); track page.index) {
        <li #item role="option" class="page" [class.selected]="page.index === journal.selectedIndex()"
            [attr.aria-selected]="page.index === journal.selectedIndex()" (click)="journal.select(page.index)">
          <div class="sheet" [style.aspect-ratio]="page.view ? page.view[2] + ' / ' + page.view[3] : '3 / 4'">
            @if (page.svgUrl) {
              <img [src]="page.svgUrl" [class.stale]="page.stale" [alt]="page.label" loading="lazy" decoding="async" />
            } @else if (page.view) {
              <span class="note">drawing…</span>
            } @else {
              <span class="note">empty</span>
            }
          </div>
          <span class="label">{{ page.index + 1 }} · {{ page.label }}</span>
        </li>
      } @empty {
        <li class="none">No pages yet</li>
      }
    </ol>
  `,
  styles: `
    :host { display: block; overflow-y: auto; background: var(--rail-bg); color: var(--rail-fg); }
    .pages { list-style: none; margin: 0; padding: 14px 12px; display: flex; flex-direction: column; gap: 14px; outline: none; }
    .page { padding: 6px; border-radius: 6px; cursor: pointer; border: 2px solid transparent; }
    .page:hover { background: var(--rail-hover); }
    .page.selected { border-color: var(--rail-selected); background: var(--rail-selected-bg); }
    .pages:focus-visible .page.selected { outline: 2px solid var(--accent); outline-offset: 2px; }
    .sheet { position: relative; background: var(--paper); border-radius: 2px; display: grid; place-items: center; overflow: hidden; }
    .sheet img { width: 100%; height: 100%; object-fit: contain; display: block; }
    .sheet img.stale { opacity: 0.55; }
    .note { font-size: 11px; color: var(--paper-muted); }
    .label { display: block; margin-top: 6px; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .none { font-size: 12px; color: var(--muted); padding: 8px; }
    @media (max-width: 760px) {
      .pages { flex-direction: row; padding: 8px; gap: 8px; }
      .page { flex: none; width: 76px; }
    }
  `,
})
export class PageListComponent {
  protected readonly journal = inject(JournalService);
  private readonly items = viewChildren<ElementRef<HTMLElement>>('item');

  constructor() {
    // Keep the picked page in view.
    effect(() => {
      const item = this.items()[this.journal.selectedIndex()];
      item?.nativeElement.scrollIntoView({ block: 'nearest' });
    });
  }

  protected onKey(event: KeyboardEvent) {
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[event.key];
    if (step) {
      event.preventDefault();
      this.journal.select(this.journal.selectedIndex() + step);
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      this.journal.select(event.key === 'Home' ? 0 : this.journal.pages().length - 1);
    }
  }
}
