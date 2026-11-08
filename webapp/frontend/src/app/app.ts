import { Component, computed, inject } from '@angular/core';
import { FluidWorkspaceComponent } from './fluids/fluid-workspace.component';
import { FontsWorkspaceComponent } from './fonts/fonts-workspace.component';
import { RESOURCE_KINDS, splitPath } from './journal/journal-files';
import { JournalWorkspaceComponent } from './journal/journal-workspace.component';
import { ThreePaneComponent } from './layout/three-pane.component';
import { LocksWorkspaceComponent } from './locks/locks-workspace.component';
import { EntryPreviewDialogComponent } from './preview/entry-preview-dialog.component';
import { ContentEditorComponent } from './resources/content-editor.component';
import { ImageViewerComponent } from './resources/image-viewer.component';
import { ResourceEditorComponent } from './resources/resource-editor.component';
import { StyleEditorComponent } from './resources/style-editor.component';
import { JournalStore } from './services/journal-store.service';
import { JournalBarComponent } from './shell/journal-bar.component';
import { ShellService } from './shell/shell.service';

/**
 * The editor: the journal across the top, and below it the file open in
 * the editor for its kind, each with the journal's files as a tab on its left. The editors stay
 * as they were while another file shows; all but the entry editor load
 * when first opened. The last entry's preview opens over any of them.
 */
@Component({
  selector: 'app-root',
  imports: [JournalBarComponent, ThreePaneComponent, JournalWorkspaceComponent, FontsWorkspaceComponent, LocksWorkspaceComponent,
    ResourceEditorComponent, StyleEditorComponent, ContentEditorComponent, ImageViewerComponent, FluidWorkspaceComponent, EntryPreviewDialogComponent],
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
        @defer (when kind() === 'content') {
          <app-content-editor class="workspace" [hidden]="kind() !== 'content'" />
        }
        @defer (when resource()) {
          <app-resource-editor class="workspace" [hidden]="!resource()" />
        }
        @defer (when kind() === 'images') {
          <app-image-viewer class="workspace" [hidden]="kind() !== 'images'" />
        }
        @defer (when kind() === 'styles') {
          <app-style-editor class="workspace" [hidden]="kind() !== 'styles'" />
        }
        @defer (when kind() === 'fluids') {
          <app-fluid-workspace class="workspace" [hidden]="kind() !== 'fluids'" />
        }
        @if (kind() === null) {
          <app-three-pane class="workspace" key="none" [right]="false"><p center class="hint waiting">Pick a file in the Files tab, or add one.</p></app-three-pane>
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
  /** Whether the file shown is one the resource editor edits. */
  protected readonly resource = computed(() => {
    const kind = this.kind();
    return kind !== null && RESOURCE_KINDS.includes(kind);
  });
}
