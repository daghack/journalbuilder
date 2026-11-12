import { Component, DestroyRef, ElementRef, effect, inject, signal, viewChild } from '@angular/core';
import { entryPath } from '../journal/journal-files';
import { keep, stored } from '../layout/pane-state';
import { PageListComponent } from '../pages/page-list.component';
import { JournalService } from '../services/journal.service';
import { VisualEditService } from '../services/visual-edit.service';
import { ShellService } from '../shell/shell.service';
import { CanvasToolbarComponent } from './canvas-toolbar.component';
import { PagePreviewComponent } from './page-preview.component';

/**
 * The last entry opened, run with the journal's files as they are, over
 * whatever file is open: Ctrl/⌘+P (or the top bar's Preview) opens it and
 * shuts it, as does Esc. It's the visual editor too: the canvases the
 * entry commits to it are listed and boxed on the SVG, to place by hand,
 * with a Darkly render of the page beside it.
 */
@Component({
  selector: 'app-entry-preview-dialog',
  imports: [PagePreviewComponent, PageListComponent, CanvasToolbarComponent],
  host: {
    '(window:pointermove)': 'drag($event)',
    '(window:pointerup)': 'endDrag()',
  },
  template: `
    <dialog #dialog aria-label="Entry preview" (close)="shell.previewOpen.set(false)" (cancel)="onCancel($event)"
            (click)="onClick($event)" (keydown)="onKey($event)">
      <div class="head">
        <h2>entries/{{ journal.entry() ?? '—' }}.rhai</h2>
        @if (journal.error()) { <span class="problem" title="The pages are the last good run's">The script has an error.</span> }
        @if (journal.evaluating()) { <span class="busy" title="Running the entry">running…</span> }
        <span class="spacer"></span>
        <button type="button" [class.on]="darklyShown()" (click)="toggleDarkly()" title="Show or hide the page rendered with Darkly, beside the editor">Darkly</button>
        <button type="button" (click)="openEntry()" [disabled]="journal.entry() === null" title="Edit the entry">Open entry</button>
        <button type="button" (click)="shell.previewOpen.set(false)" title="Close (Esc)">Close</button>
      </div>
      <div class="body" [class.solo]="!darklyShown()" [style.--darkly-width.px]="darklyWidth()">
        <app-canvas-toolbar class="tools" />
        <app-page-preview class="preview" placement="dialog" fixed="svg" [editing]="true" />
        @if (darklyShown()) {
          <div class="split" role="separator" aria-label="Resize the Darkly render" (pointerdown)="startDrag($event)"></div>
          <app-page-preview class="preview" placement="dialog" fixed="darkly" />
        }
      </div>
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
    .head button.on { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
    .body { flex: 1; min-height: 0; display: grid; grid-template-columns: 248px minmax(240px, 1fr) 4px var(--darkly-width); }
    .body.solo { grid-template-columns: 248px minmax(240px, 1fr); }
    .body > * { min-width: 0; min-height: 0; }
    .split { cursor: col-resize; background: var(--border); }
    .split:hover { background: var(--accent); }
    .pages { flex: none; max-height: 168px; overflow-x: auto; overflow-y: hidden; border-top: 1px solid var(--border); }
    .pages ::ng-deep .pages { flex-direction: row; padding: 10px 12px; gap: 10px; }
    .pages ::ng-deep .page { flex: none; width: 96px; }
    @media (max-width: 760px) {
      dialog { width: calc(100vw - 16px); height: calc(100dvh - 16px); }
      .body, .body.solo { grid-template-columns: 1fr; grid-template-rows: auto minmax(0, 1fr); }
      .tools { max-height: 30vh; }
      .split, .body > app-page-preview + .split + app-page-preview { display: none; }
    }
  `,
})
export class EntryPreviewDialogComponent {
  protected readonly journal = inject(JournalService);
  protected readonly shell = inject(ShellService);
  private readonly edit = inject(VisualEditService);
  private readonly dialog = viewChild.required<ElementRef<HTMLDialogElement>>('dialog');

  protected readonly darklyShown = signal(stored('preview-darkly', true));
  protected readonly darklyWidth = signal(stored('preview-darkly-width', 0) || Math.round(Math.max(320, window.innerWidth * 0.3)));
  private dragging: { x: number; width: number } | null = null;

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

  /** Esc lets go of the selection before it closes. */
  protected onCancel(event: Event) {
    if (this.edit.selection().length) {
      event.preventDefault();
      this.edit.selection.set([]);
    }
  }

  /** The editor's keys: undo and redo, nudging, and the tools. */
  protected onKey(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.closest('input, textarea, select, [contenteditable]')) return;
    const mod = event.ctrlKey || event.metaKey;
    const key = event.key.toLowerCase();
    if (mod && key === 'z') {
      event.preventDefault();
      if (event.shiftKey) this.edit.redo();
      else this.edit.undo();
    } else if (mod && key === 'y') {
      event.preventDefault();
      this.edit.redo();
    } else if (!mod && !event.altKey && key.startsWith('arrow') && this.edit.selection().length) {
      event.preventDefault();
      const step = event.shiftKey ? 10 : 1;
      const by: [number, number] = key === 'arrowleft' ? [-step, 0] : key === 'arrowright' ? [step, 0] : key === 'arrowup' ? [0, -step] : [0, step];
      this.edit.nudge(this.edit.selection().filter((n) => !this.edit.edit(n).locked), by);
    } else if (!mod && !event.altKey && key === 'v') {
      this.edit.tool.set('select');
    } else if (!mod && !event.altKey && key === 'w') {
      this.edit.tool.set('warp');
    }
  }

  protected toggleDarkly() {
    this.darklyShown.update((shown) => !shown);
    keep('preview-darkly', this.darklyShown());
  }

  protected startDrag(event: PointerEvent) {
    event.preventDefault();
    this.dragging = { x: event.clientX, width: this.darklyWidth() };
  }

  protected drag(event: PointerEvent) {
    if (!this.dragging) return;
    const width = this.dragging.width - (event.clientX - this.dragging.x);
    this.darklyWidth.set(Math.round(Math.min(window.innerWidth - 600, Math.max(240, width))));
  }

  protected endDrag() {
    if (!this.dragging) return;
    this.dragging = null;
    keep('preview-darkly-width', this.darklyWidth());
  }

  protected openEntry() {
    const entry = this.journal.entry();
    if (entry === null) return;
    this.shell.open(entryPath(entry));
    this.shell.previewOpen.set(false);
  }
}
