import { Component, DestroyRef, ElementRef, effect, inject, viewChild } from '@angular/core';
import { entryPath } from '../journal/journal-files';
import { PageListComponent } from '../pages/page-list.component';
import { JournalService } from '../services/journal.service';
import { ShellService } from '../shell/shell.service';
import { PagePreviewComponent } from './page-preview.component';

/**
 * The last entry opened, run with the journal's files as they are, over
 * whatever file is open: Ctrl/⌘+P (or the top bar's Preview) opens it and
 * shuts it, as does Esc.
 */
@Component({
  selector: 'app-entry-preview-dialog',
  imports: [PagePreviewComponent, PageListComponent],
  template: `
    <dialog #dialog aria-label="Entry preview" (close)="shell.previewOpen.set(false)" (click)="onClick($event)">
      <div class="head">
        <h2>entries/{{ journal.entry() ?? '—' }}.rhai</h2>
        @if (journal.error()) { <span class="problem" title="The pages are the last good run's">The script has an error.</span> }
        @if (journal.evaluating()) { <span class="busy" title="Running the entry">running…</span> }
        <span class="spacer"></span>
        <button type="button" (click)="openEntry()" [disabled]="journal.entry() === null" title="Edit the entry">Open entry</button>
        <button type="button" (click)="shell.previewOpen.set(false)" title="Close (Esc)">Close</button>
      </div>
      <app-page-preview class="preview" placement="dialog" />
      <app-page-list class="pages" />
    </dialog>
  `,
  styles: `
    dialog { border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: var(--fg); padding: 0; overflow: hidden;
             width: calc(100vw - 64px); height: calc(100dvh - 64px); max-width: none; max-height: none; }
    dialog[open] { display: flex; flex-direction: column; }
    dialog::backdrop { background: rgb(0 0 0 / 0.45); }
    .head { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--border); font-size: 12px; }
    h2 { margin: 0; font-size: 14px; font-family: var(--mono); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .problem { color: var(--danger); }
    .busy { color: var(--muted); }
    .spacer { flex: 1; }
    .preview { flex: 1; }
    .pages { flex: none; max-height: 168px; overflow-x: auto; overflow-y: hidden; border-top: 1px solid var(--border); }
    .pages ::ng-deep .pages { flex-direction: row; padding: 10px 12px; gap: 10px; }
    .pages ::ng-deep .page { flex: none; width: 96px; }
    @media (max-width: 760px) {
      dialog { width: calc(100vw - 16px); height: calc(100dvh - 16px); }
    }
  `,
})
export class EntryPreviewDialogComponent {
  protected readonly journal = inject(JournalService);
  protected readonly shell = inject(ShellService);
  private readonly dialog = viewChild.required<ElementRef<HTMLDialogElement>>('dialog');

  constructor() {
    effect(() => {
      const dialog = this.dialog().nativeElement;
      if (this.shell.previewOpen() && !dialog.open) dialog.showModal();
      else if (!this.shell.previewOpen() && dialog.open) dialog.close();
    });
    // Ctrl/⌘+P anywhere, before the code editor (or the browser's Print) gets it.
    const onKey = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.key.toLowerCase() !== 'p') return;
      event.preventDefault();
      event.stopPropagation();
      this.shell.previewOpen.update((open) => !open);
    };
    document.addEventListener('keydown', onKey, { capture: true });
    inject(DestroyRef).onDestroy(() => document.removeEventListener('keydown', onKey, { capture: true }));
  }

  /** A click on the backdrop (outside the dialog's box) closes it. */
  protected onClick(event: MouseEvent) {
    const box = this.dialog().nativeElement.getBoundingClientRect();
    if (event.target === this.dialog().nativeElement
        && (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom)) {
      this.shell.previewOpen.set(false);
    }
  }

  protected openEntry() {
    const entry = this.journal.entry();
    if (entry === null) return;
    this.shell.open(entryPath(entry));
    this.shell.previewOpen.set(false);
  }
}
