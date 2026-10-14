import { Component, viewChild } from '@angular/core';
import { DiagnosticsComponent } from '../diagnostics/diagnostics.component';
import { EditorComponent } from '../editor/editor.component';
import { ExportBarComponent } from '../export/export-bar.component';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { PageListComponent } from '../pages/page-list.component';
import { PagePreviewComponent } from '../preview/page-preview.component';
import { TopBarComponent } from '../toolbar/top-bar.component';

/** The journal editor: the pages, the picked page, and the script with its exports. */
@Component({
  selector: 'app-journal-workspace',
  imports: [ThreePaneComponent, PageListComponent, PagePreviewComponent, EditorComponent, TopBarComponent, DiagnosticsComponent, ExportBarComponent],
  template: `
    <app-three-pane key="journal">
      <app-page-list left />
      <app-page-preview center />
      <section right class="code" aria-label="Script">
        <app-top-bar />
        <app-editor class="editor" />
        <app-diagnostics (goTo)="editor()?.goTo($event)" />
        <app-export-bar />
      </section>
    </app-three-pane>
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .code { display: flex; flex-direction: column; min-width: 0; background: var(--editor-bg); }
    .editor { flex: 1; }
    @media (max-width: 760px) {
      .code { min-height: 70vh; }
    }
  `,
})
export class JournalWorkspaceComponent {
  protected readonly editor = viewChild(EditorComponent);
}
