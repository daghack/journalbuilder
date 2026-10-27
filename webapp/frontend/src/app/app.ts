import { Component, computed, inject } from '@angular/core';
import { FontsWorkspaceComponent } from './fonts/fonts-workspace.component';
import { splitPath } from './journal/journal-files';
import { JournalWorkspaceComponent } from './journal/journal-workspace.component';
import { ThreePaneComponent } from './layout/three-pane.component';
import { LocksWorkspaceComponent } from './locks/locks-workspace.component';
import { EntryPreviewDialogComponent } from './preview/entry-preview-dialog.component';
import { ContentEditorComponent } from './resources/content-editor.component';
import { ResourceEditorComponent } from './resources/resource-editor.component';
import { JournalStore } from './services/journal-store.service';
import { JournalBarComponent } from './shell/journal-bar.component';
import { ShellService } from './shell/shell.service';

/**
 * The editor: the journal across the top, and below it the file open in
 * the editor for its kind, each with the journal's files as a tab on its left. The entry, glyphset and
 * chain editors stay as they were while another file shows; the glyphset
 * and chain editors load when first opened. The last entry's preview opens
 * over any of them.
 */
@Component({
  selector: 'app-root',
  imports: [JournalBarComponent, ThreePaneComponent, JournalWorkspaceComponent, FontsWorkspaceComponent, LocksWorkspaceComponent,
    ResourceEditorComponent, ContentEditorComponent, EntryPreviewDialogComponent],
  template: `
    <app-journal-bar />
    <main class="editor">
      @if (!store.restored()) {
        <p class="hint waiting">Opening the journal…</p>
      } @else {
        <app-journal-workspace class="workspace" [hidden]="kind() !== 'entries'" />
        @defer (when kind() === 'glyphsets') {
          <app-fonts-workspace class="workspace" [hidden]="kind() !== 'glyphsets'" />
        }
        @defer (when kind() === 'chains') {
          <app-locks-workspace class="workspace" [hidden]="kind() !== 'chains'" />
        }
        @switch (kind()) {
          @case ('content') { <app-content-editor class="workspace" /> }
          @case ('entries') {}
          @case ('glyphsets') {}
          @case ('chains') {}
          @case (null) {
            <app-three-pane class="workspace" key="none" [right]="false"><p center class="hint waiting">Pick a file in the Files tab, or add one.</p></app-three-pane>
          }
          @default { <app-resource-editor class="workspace" /> }
        }
      }
    </main>
    <app-entry-preview-dialog />
  `,
  styles: `
    :host { display: flex; flex-direction: column; height: 100dvh; }
    .editor { flex: 1; position: relative; min-height: 0; min-width: 0; display: flex; flex-direction: column; }
    .workspace { flex: 1; min-height: 0; }
    .workspace[hidden] { display: none; }
    .waiting { margin: 20px; }
    @media (max-width: 760px) {
      :host { height: auto; min-height: 100dvh; }
    }
  `,
})
export class App {
  protected readonly store = inject(JournalStore);
  private readonly shell = inject(ShellService);
  protected readonly kind = computed(() => splitPath(this.shell.selected() ?? '')?.kind ?? null);
}
