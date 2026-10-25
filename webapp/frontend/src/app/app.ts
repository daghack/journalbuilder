import { Component, computed, inject } from '@angular/core';
import { FontsWorkspaceComponent } from './fonts/fonts-workspace.component';
import { splitPath } from './journal/journal-files';
import { JournalWorkspaceComponent } from './journal/journal-workspace.component';
import { LocksWorkspaceComponent } from './locks/locks-workspace.component';
import { ContentEditorComponent } from './resources/content-editor.component';
import { ResourceEditorComponent } from './resources/resource-editor.component';
import { JournalStore } from './services/journal-store.service';
import { FileSidebarComponent } from './shell/file-sidebar.component';
import { JournalBarComponent } from './shell/journal-bar.component';
import { ShellService } from './shell/shell.service';

/**
 * The editor: the journal across the top, its files down the left, and the
 * file open beside them in the editor for its kind. The entry, glyphset and
 * chain editors stay as they were while another file shows; the glyphset
 * and chain editors load when first opened.
 */
@Component({
  selector: 'app-root',
  imports: [JournalBarComponent, FileSidebarComponent, JournalWorkspaceComponent, FontsWorkspaceComponent, LocksWorkspaceComponent,
    ResourceEditorComponent, ContentEditorComponent],
  template: `
    <app-journal-bar />
    <div class="body">
      <app-file-sidebar class="sidebar" />
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
            @case (null) { <p class="hint waiting">Pick a file on the left, or add one.</p> }
            @default { <app-resource-editor class="workspace" /> }
          }
        }
      </main>
    </div>
  `,
  styles: `
    :host { display: flex; flex-direction: column; height: 100dvh; }
    .body { flex: 1; display: grid; grid-template-columns: 220px minmax(0, 1fr); min-height: 0; }
    .sidebar { min-height: 0; }
    .editor { position: relative; min-height: 0; min-width: 0; display: flex; flex-direction: column; }
    .workspace { flex: 1; min-height: 0; }
    .workspace[hidden] { display: none; }
    .waiting { margin: 20px; }
    @media (max-width: 760px) {
      :host { height: auto; min-height: 100dvh; }
      .body { grid-template-columns: 1fr; }
      .sidebar { max-height: 40vh; }
    }
  `,
})
export class App {
  protected readonly store = inject(JournalStore);
  private readonly shell = inject(ShellService);
  protected readonly kind = computed(() => splitPath(this.shell.selected() ?? '')?.kind ?? null);
}
