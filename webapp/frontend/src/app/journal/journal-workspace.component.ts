import { Component, inject, viewChild } from '@angular/core';
import { DiagnosticsComponent } from '../diagnostics/diagnostics.component';
import { EditorComponent } from '../editor/editor.component';
import { ExportBarComponent } from '../export/export-bar.component';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { PageListComponent } from '../pages/page-list.component';
import { PagePreviewComponent } from '../preview/page-preview.component';
import { JournalService } from '../services/journal.service';

/** The entry editor: the entry's pages, the picked page, and its script with its exports. */
@Component({
  selector: 'app-journal-workspace',
  imports: [ThreePaneComponent, PageListComponent, PagePreviewComponent, EditorComponent, DiagnosticsComponent, ExportBarComponent],
  template: `
    <app-three-pane key="journal">
      <app-page-list left />
      <app-page-preview center />
      <section right class="code" aria-label="Script">
        <div class="head">
          <span class="name">entries/{{ journal.entry() }}.rhai</span>
          @if (journal.evaluating()) { <span class="busy" title="Running the entry">running…</span> }
        </div>
        <app-editor class="editor" />
        <app-diagnostics (goTo)="editor()?.goTo($event)" />
        <app-export-bar />
      </section>
    </app-three-pane>
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .code { display: flex; flex-direction: column; min-width: 0; background: var(--editor-bg); }
    .head { display: flex; align-items: center; gap: 8px; padding: 6px 10px; background: var(--surface); border-bottom: 1px solid var(--border); font-size: 12px; }
    .name { flex: 1; font-weight: 600; font-family: var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .busy { color: var(--muted); }
    .editor { flex: 1; }
    @media (max-width: 760px) {
      .code { min-height: 70vh; }
    }
  `,
})
export class JournalWorkspaceComponent {
  protected readonly journal = inject(JournalService);
  protected readonly editor = viewChild(EditorComponent);
}
