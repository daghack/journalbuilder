import { Component, inject, signal, viewChild } from '@angular/core';
import { DiagnosticsComponent } from './diagnostics/diagnostics.component';
import { EditorComponent } from './editor/editor.component';
import { ExportBarComponent } from './export/export-bar.component';
import { PageListComponent } from './pages/page-list.component';
import { PagePreviewComponent } from './preview/page-preview.component';
import { JournalService } from './services/journal.service';
import { TopBarComponent, loadExample } from './toolbar/top-bar.component';

/** The script a first visit starts with. */
const FIRST_EXAMPLE = 'showcase';

type Splitter = 'pages' | 'editor';

/** Three columns: the pages, the picked page, and the script with its exports. */
@Component({
  selector: 'app-root',
  imports: [PageListComponent, PagePreviewComponent, EditorComponent, TopBarComponent, DiagnosticsComponent, ExportBarComponent],
  host: {
    '[style.--pages-width.px]': 'pagesWidth()',
    '[style.--editor-width.px]': 'editorWidth()',
    '(window:pointermove)': 'drag($event)',
    '(window:pointerup)': 'dragging = null',
  },
  template: `
    <app-page-list class="pages" />
    <div class="split" role="separator" aria-label="Resize the pages" (pointerdown)="startDrag($event, 'pages')"></div>
    <app-page-preview class="preview" />
    <div class="split" role="separator" aria-label="Resize the editor" (pointerdown)="startDrag($event, 'editor')"></div>
    <section class="code" aria-label="Script">
      <app-top-bar />
      <app-editor class="editor" />
      <app-diagnostics (goTo)="editor()?.goTo($event)" />
      <app-export-bar />
    </section>
  `,
  styles: `
    :host {
      display: grid; height: 100dvh;
      grid-template-columns: var(--pages-width) 4px minmax(240px, 1fr) 4px var(--editor-width);
    }
    .pages, .preview, .code { min-height: 0; }
    .code { display: flex; flex-direction: column; min-width: 0; background: var(--editor-bg); }
    .editor { flex: 1; }
    .split { cursor: col-resize; background: var(--border); }
    .split:hover { background: var(--accent); }
    @media (max-width: 760px) {
      :host { grid-template-columns: 1fr; grid-template-rows: 120px 45vh auto; height: auto; min-height: 100dvh; }
      .split { display: none; }
      .pages { overflow-x: auto; }
      .code { min-height: 70vh; }
    }
  `,
})
export class App {
  private readonly journal = inject(JournalService);
  protected readonly editor = viewChild(EditorComponent);
  protected readonly pagesWidth = signal(190);
  protected readonly editorWidth = signal(Math.round(Math.max(420, window.innerWidth * 0.4)));
  protected dragging: { which: Splitter; x: number; width: number } | null = null;

  constructor() {
    void this.journal.restore(async () => ({
      name: `${FIRST_EXAMPLE}.rhai`,
      source: (await loadExample(FIRST_EXAMPLE)) ?? '// Write a journal script here.\n',
    }));
  }

  protected startDrag(event: PointerEvent, which: Splitter) {
    event.preventDefault();
    this.dragging = { which, x: event.clientX, width: which === 'pages' ? this.pagesWidth() : this.editorWidth() };
  }

  protected drag(event: PointerEvent) {
    if (!this.dragging) return;
    const { which, x, width } = this.dragging;
    const dx = event.clientX - x;
    if (which === 'pages') this.pagesWidth.set(Math.min(400, Math.max(110, width + dx)));
    else this.editorWidth.set(Math.min(window.innerWidth - 400, Math.max(300, width - dx)));
  }
}
