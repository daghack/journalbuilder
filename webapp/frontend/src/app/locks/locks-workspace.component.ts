import { Component, ElementRef, inject } from '@angular/core';
import { PaneTabDirective, ThreePaneComponent, type PaneTab } from '../layout/three-pane.component';
import { LibraryBannerComponent } from '../resources/library-banner.component';
import { LockEditorService } from './lock-editor.service';
import { LockGraphComponent } from './lock-graph.component';
import { LockPaletteComponent } from './lock-palette.component';
import { LockPanelComponent } from './lock-panel.component';

/** The chain editor: the lock types and illustrations (a tab beside the files), the chain wired from them, and its options, ports and test. */
@Component({
  selector: 'app-locks-workspace',
  imports: [ThreePaneComponent, PaneTabDirective, LockPaletteComponent, LockGraphComponent, LockPanelComponent, LibraryBannerComponent],
  providers: [LockEditorService],
  host: { '(window:keydown)': 'onKey($event)' },
  template: `
    <app-three-pane key="locks" [tabs]="tabs" [rightFraction]="0.3" [rightMin]="340">
      <app-lock-palette appPaneTab="library" />
      <section center class="centre">
        <app-library-banner [path]="editor.path()" [own]="editor.own()" [builtin]="editor.builtin()" (copy)="editor.copyIn()" (revert)="editor.useLibrary()" />
        <app-lock-graph class="graph" />
      </section>
      <app-lock-panel right />
    </app-three-pane>
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .centre { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
    .graph { flex: 1; min-height: 0; }
  `,
})
export class LocksWorkspaceComponent {
  protected readonly editor = inject(LockEditorService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  protected readonly tabs: PaneTab[] = [{ id: 'library', label: 'Library', title: 'The lock types and illustrations to wire into the chain' }];

  /** Undo and redo, when the chain editor shows and no text field has the keyboard. */
  protected onKey(event: KeyboardEvent) {
    if (this.host.nativeElement.hidden || !(event.ctrlKey || event.metaKey)) return;
    if ((event.target as HTMLElement | null)?.closest('input, textarea, select')) return;
    const key = event.key.toLowerCase();
    if (key === 'z' && !event.shiftKey) {
      event.preventDefault();
      this.editor.undo();
    } else if ((key === 'z' && event.shiftKey) || key === 'y') {
      event.preventDefault();
      this.editor.redo();
    }
  }
}
