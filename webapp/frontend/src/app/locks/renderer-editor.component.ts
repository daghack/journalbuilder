import { Component, computed, inject } from '@angular/core';
import { EditorComponent } from '../editor/editor.component';
import { EDITOR_DOCUMENT, problemAt, type EditorDocument } from '../editor/editor-document';
import { LockEditorService } from './lock-editor.service';
import { RenderPagesComponent } from './render-pages.component';

/** The renderer file being written, as the script editor's document. */
function rendererDocument(): EditorDocument {
  const editor = inject(LockEditorService);
  return {
    source: computed(() => editor.editing()?.source ?? ''),
    setSource: (source: string) => editor.setRendererSource(source),
    error: computed(() => {
      const preview = editor.editingPreview();
      return preview && 'error' in preview ? problemAt(preview.error) : null;
    }),
    warnings: computed(() => []),
  };
}

/**
 * The middle, while a renderer file is written: its source, and what it
 * draws of its example() as it's typed. While it draws, it's in this
 * browser's workspace, so chains and journals using it follow along.
 */
@Component({
  selector: 'app-renderer-editor',
  imports: [EditorComponent, RenderPagesComponent],
  providers: [{ provide: EDITOR_DOCUMENT, useFactory: rendererDocument }],
  template: `
    <div class="toolbar">
      <span class="label">Renderer</span>
      <input type="text" class="mono" aria-label="Renderer name" [value]="editor.editing()?.name ?? ''"
             (change)="editor.setRendererName($any($event.target).value)" spellcheck="false" />
      <span class="hint">renderers/{{ editor.editing()?.name }}.rhai</span>
      <span class="spacer"></span>
      <button type="button" class="primary" (click)="editor.saveRenderer()" title="Check it draws its example, add it to this browser's renderers, and save its file">Save renderer</button>
      <button type="button" (click)="editor.closeRenderer()" title="Back to the chain">Close</button>
    </div>
    <div class="body">
      <app-editor class="code" />
      <div class="preview">
        <h3>Its example, drawn</h3>
        @if (editor.editingPreview(); as preview) { <app-render-pages [preview]="preview" /> } @else { <p class="hint">Drawing…</p> }
        @if (editor.editingSaved(); as saved) { <p class="hint saved">{{ saved }}</p> }
        <p class="hint">A renderer file defines inputs(o) (the values it draws, name → type), render(o, i) (a canvas, or an array of them),
          and optionally options() (defaults) and example() (inputs to preview with). It runs in the journal's engine, so it can draw.</p>
      </div>
    </div>
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-height: 0; min-width: 0; background: var(--stage-bg); }
    .toolbar { align-items: center; }
    .label { font-weight: 600; }
    .mono { font-family: var(--mono); }
    .spacer { flex: 1; }
    .body { flex: 1; display: grid; grid-template-columns: minmax(0, 3fr) minmax(220px, 2fr); min-height: 0; }
    .code { min-height: 0; border-right: 1px solid var(--border); }
    .preview { overflow-y: auto; padding: 10px; background: var(--surface); color: var(--fg); display: flex; flex-direction: column; gap: 8px; }
    h3 { margin: 0; font-size: 12px; }
    .saved { color: var(--fg); }
  `,
})
export class RendererEditorComponent {
  protected readonly editor = inject(LockEditorService);
}
