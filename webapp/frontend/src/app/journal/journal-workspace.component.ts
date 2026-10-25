import { Component, ElementRef, effect, inject, signal, viewChild } from '@angular/core';
import { DiagnosticsComponent } from '../diagnostics/diagnostics.component';
import { EditorComponent } from '../editor/editor.component';
import { ExportBarComponent } from '../export/export-bar.component';
import { PaneTabDirective, ThreePaneComponent, type PaneTab } from '../layout/three-pane.component';
import { PageListComponent } from '../pages/page-list.component';
import { PagePreviewComponent } from '../preview/page-preview.component';
import { JournalStore } from '../services/journal-store.service';
import { JournalService } from '../services/journal.service';
import { ShellService } from '../shell/shell.service';
import { entryPath } from './journal-files';

/** The entry editor: the entry's pages (a tab beside the files), the picked page, and its script with its exports. */
@Component({
  selector: 'app-journal-workspace',
  imports: [ThreePaneComponent, PaneTabDirective, PageListComponent, PagePreviewComponent, EditorComponent, DiagnosticsComponent, ExportBarComponent],
  template: `
    <app-three-pane key="journal" [tabs]="tabs">
      <app-page-list appPaneTab="pages" />
      <app-page-preview center />
      <section right class="code" aria-label="Script">
        <div class="head">
          @if (renaming(); as entry) {
            <span class="name">entries/<input #nameInput class="name-edit" [value]="entry" aria-label="The entry's name"
              (keydown.enter)="commitName(nameInput.value)" (keydown.escape)="renaming.set(null)" (blur)="commitName(nameInput.value)" />.rhai</span>
          } @else {
            <button type="button" class="name" (click)="renaming.set(journal.entry())" [disabled]="journal.entry() === null"
                    title="Rename the entry (its file follows)">entries/{{ journal.entry() }}.rhai</button>
          }
          @if (problem(); as p) { <span class="problem">{{ p }}</span> }
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
    .name { flex: 1; min-width: 0; font-weight: 600; font-family: var(--mono); font-size: inherit; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    button.name { background: transparent; border: 0; color: inherit; padding: 0; text-align: left; cursor: text; }
    button.name:hover:not(:disabled) { text-decoration: underline dotted; }
    .name-edit { font: inherit; padding: 0 3px; width: 20ch; }
    .problem { color: var(--danger); }
    .busy { color: var(--muted); }
    .editor { flex: 1; }
    @media (max-width: 760px) {
      .code { min-height: 70vh; }
    }
  `,
})
export class JournalWorkspaceComponent {
  protected readonly journal = inject(JournalService);
  private readonly store = inject(JournalStore);
  private readonly shell = inject(ShellService);
  protected readonly editor = viewChild(EditorComponent);
  protected readonly tabs: PaneTab[] = [{ id: 'pages', label: 'Pages', title: "The entry's pages: pick one to see it" }];
  /** The entry being renamed, in place. */
  protected readonly renaming = signal<string | null>(null);
  /** Why the last rename didn't happen. */
  protected readonly problem = signal<string | null>(null);
  private readonly nameInput = viewChild<ElementRef<HTMLInputElement>>('nameInput');

  constructor() {
    effect(() => {
      const input = this.nameInput()?.nativeElement;
      if (input) {
        input.focus();
        input.select();
      }
    });
  }

  /** Rename the entry: its file, and its place in the manifest. */
  protected commitName(value: string) {
    const from = this.renaming();
    if (from === null) return;
    this.renaming.set(null);
    const to = value.trim();
    this.problem.set(null);
    if (!to || to === from) return;
    const problem = this.store.renameFile(entryPath(from), entryPath(to));
    if (problem) return this.problem.set(problem);
    // The same entry, renamed: its pages stay.
    this.journal.entry.set(to);
    this.shell.open(entryPath(to));
  }
}
