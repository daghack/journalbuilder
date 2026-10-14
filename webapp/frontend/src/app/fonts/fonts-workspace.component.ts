import { Component, inject, viewChild } from '@angular/core';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { ShellService } from '../shell/shell.service';
import { FontEditorService } from './font-editor.service';
import { FontPanelComponent } from './font-panel.component';
import { GlyphCanvasComponent } from './glyph-canvas.component';
import { GlyphListComponent } from './glyph-list.component';
import { GlyphToolbarComponent } from './glyph-toolbar.component';
import { TextPreviewComponent } from './text-preview.component';

/** The font editor: the font's glyphs, the glyph drawn with a preview of text in the font, and the font's options. */
@Component({
  selector: 'app-fonts-workspace',
  imports: [ThreePaneComponent, GlyphListComponent, GlyphToolbarComponent, GlyphCanvasComponent, TextPreviewComponent, FontPanelComponent],
  providers: [FontEditorService],
  host: { '(window:keydown)': 'onKey($event)' },
  template: `
    <app-three-pane key="fonts" [leftDefault]="230" [rightFraction]="0.28" [rightMin]="320">
      <app-glyph-list left />
      <section center class="centre" aria-label="Glyph">
        <app-glyph-toolbar [canvas]="canvas()" />
        <app-glyph-canvas class="canvas" />
        <app-text-preview />
      </section>
      <app-font-panel right />
    </app-three-pane>
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .centre { display: flex; flex-direction: column; min-width: 0; }
    .canvas { flex: 1; min-height: 200px; }
  `,
})
export class FontsWorkspaceComponent {
  private readonly editor = inject(FontEditorService);
  private readonly shell = inject(ShellService);
  protected readonly canvas = viewChild(GlyphCanvasComponent);

  /** Undo and redo, when the font editor shows and no text field has the keyboard. */
  protected onKey(event: KeyboardEvent) {
    if (this.shell.mode() !== 'fonts' || !(event.ctrlKey || event.metaKey)) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest('input, textarea, select')) return;
    const key = event.key.toLowerCase();
    if (key === 'z' && !event.shiftKey) {
      event.preventDefault();
      this.editor.undo();
    } else if ((key === 'z' && event.shiftKey) || key === 'y') {
      event.preventDefault();
      this.editor.redo();
    } else if (key === 's') {
      event.preventDefault();
      void this.editor.export();
    }
  }
}
