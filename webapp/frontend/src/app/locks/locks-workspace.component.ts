import { Component, inject } from '@angular/core';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { ShellService } from '../shell/shell.service';
import { LockEditorService } from './lock-editor.service';
import { LockGraphComponent } from './lock-graph.component';
import { LockPaletteComponent } from './lock-palette.component';
import { LockPanelComponent } from './lock-panel.component';
import { RendererEditorComponent } from './renderer-editor.component';

/** The lock editor: the lock and renderer types, the chain wired from them (or a renderer file being written), and its options, ports and test. */
@Component({
  selector: 'app-locks-workspace',
  imports: [ThreePaneComponent, LockPaletteComponent, LockGraphComponent, LockPanelComponent, RendererEditorComponent],
  providers: [LockEditorService],
  host: { '(window:keydown)': 'onKey($event)' },
  template: `
    <app-three-pane key="locks" [leftDefault]="230" [rightFraction]="0.3" [rightMin]="340">
      <app-lock-palette left />
      @if (editor.editing()) { <app-renderer-editor center /> } @else { <app-lock-graph center /> }
      <app-lock-panel right />
    </app-three-pane>
  `,
  styles: `:host { display: block; min-height: 0; }`,
})
export class LocksWorkspaceComponent {
  protected readonly editor = inject(LockEditorService);
  private readonly shell = inject(ShellService);

  /** Undo and redo, when the lock editor shows and no text field has the keyboard. */
  protected onKey(event: KeyboardEvent) {
    if (this.shell.mode() !== 'locks' || !(event.ctrlKey || event.metaKey)) return;
    if ((event.target as HTMLElement | null)?.closest('input, textarea, select')) return;
    const key = event.key.toLowerCase();
    if (key === 'z' && !event.shiftKey) {
      event.preventDefault();
      this.editor.undo();
    } else if ((key === 'z' && event.shiftKey) || key === 'y') {
      event.preventDefault();
      this.editor.redo();
    } else if (key === 's') {
      event.preventDefault();
      void this.editor.save();
    }
  }
}
