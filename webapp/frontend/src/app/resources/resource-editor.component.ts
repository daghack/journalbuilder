import { Component, computed, inject } from '@angular/core';
import { EditorComponent } from '../editor/editor.component';
import { EDITOR_DOCUMENT } from '../editor/editor-document';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { RenderPagesComponent } from '../locks/render-pages.component';
import { failed } from '../services/tools.service';
import { LibraryBannerComponent } from './library-banner.component';
import { OptionsFormComponent } from './options-form.component';
import { ResourceEditorService } from './resource-editor.service';

/** What each kind's file defines, for the hint under its preview. */
const DEFINES: Record<string, string> = {
  locks: 'A lock file defines options() (option types: integer(3), one_of([...], default)…), inputs(o) and outputs(o) (name → type), lock(o, inputs), unlock(o, outputs), and optionally check(o) and examples(), which are run here.',
  illustrations: 'An illustration file defines inputs(o) (the values it draws, name → type) and render(o, i) (a canvas, or an array of them), and optionally options() (defaults) and example() (inputs to preview with). It runs in the journal\'s engine, so it can draw.',
  layouts: 'A layout file defines page(n, info) (page n\'s tree of boxes; info.options are its options), and optionally options() (defaults), page_size(o) and example() (what to typeset to preview it).',
  styles: 'A text style file defines style(o) (#{ style, type, paragraph, spans }), and optionally options() (defaults) and example() (the text to preview it with). Entries use it as text_style("name", options).',
  lib: 'A helper file defines functions any script imports: import "lib/name" as name; then name::f(x). A helper runs in the engine of the script importing it.',
};

/**
 * A lock, illustration, layout, style or helper file: its options on the
 * left, given values to preview with; its preview in the middle; its code
 * on the right. Helpers have only their code.
 */
@Component({
  selector: 'app-resource-editor',
  imports: [ThreePaneComponent, EditorComponent, RenderPagesComponent, OptionsFormComponent, LibraryBannerComponent],
  providers: [ResourceEditorService, { provide: EDITOR_DOCUMENT, useFactory: () => inject(ResourceEditorService).document }],
  template: `
    @if (editor.kind() === 'lib') {
      <div class="lib">
        <app-library-banner [path]="editor.path()" [own]="editor.own()" [builtin]="editor.builtin()" (copy)="editor.copyIn()" (revert)="editor.revertToLibrary()" />
        <app-editor class="code" />
        <p class="hint">{{ defines() }}</p>
      </div>
    } @else {
      <app-three-pane key="resource" [leftDefault]="220" [rightFraction]="0.42" [rightMin]="380">
        <aside left class="options" aria-label="Options">
          <h3>Options</h3>
          @if (editor.info()?.description; as description) { <p class="description">{{ description }}</p> }
          <app-options-form [fields]="editor.info()?.fields ?? []" [values]="editor.values()" (changed)="editor.setValue($event.name, $event.value)" />
          <p class="hint">Values here are for the preview; journals give their own.</p>
        </aside>
        <section center class="preview" aria-label="Preview">
          <app-library-banner [path]="editor.path()" [own]="editor.own()" [builtin]="editor.builtin()" (copy)="editor.copyIn()" (revert)="editor.revertToLibrary()" />
          @if (editor.kind() === 'locks') {
            @if (editor.lockCheck(); as check) {
              @if (examples(); as e) {
                <p [class.ok]="e.ok" [class.error]="!e.ok">{{ e.text }}</p>
              }
              @if (ports(); as p) {
                @if (p.error) { <p class="error">{{ p.error }}</p> } @else {
                  <div class="ports">
                    <div><h4>Inputs</h4>@for (port of p.inputs; track port[0]) { <div class="port"><span>{{ port[0] }}</span><em>{{ port[1] }}</em></div> } @empty { <p class="hint">none</p> }</div>
                    <div><h4>Outputs</h4>@for (port of p.outputs; track port[0]) { <div class="port"><span>{{ port[0] }}</span><em>{{ port[1] }}</em></div> } @empty { <p class="hint">none</p> }</div>
                  </div>
                }
              }
            } @else { <p class="hint">Checking…</p> }
          } @else {
            @if (editor.preview(); as preview) { <app-render-pages class="pages" [preview]="preview" [large]="true" /> } @else { <p class="hint">Drawing…</p> }
          }
          @if (editor.previewing()) { <p class="hint">Updating…</p> }
          <p class="hint">{{ defines() }}</p>
        </section>
        <app-editor right class="code" />
      </app-three-pane>
    }
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .options { overflow-y: auto; padding: 10px; background: var(--surface); display: flex; flex-direction: column; gap: 8px; }
    h3 { margin: 0; font-size: 12px; }
    h4 { margin: 0 0 4px; font-size: 12px; }
    .description { margin: 0; font-size: 12px; color: var(--muted); }
    .preview { overflow-y: auto; padding: 10px; background: var(--stage-bg); display: flex; flex-direction: column; gap: 8px; }
    .pages { display: block; }
    .code { min-height: 0; }
    .lib { display: grid; grid-template-rows: auto 1fr auto; height: 100%; min-height: 0; }
    .lib .hint { padding: 6px 10px; margin: 0; background: var(--surface); }
    .ports { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
    .port { display: flex; justify-content: space-between; gap: 8px; font-family: var(--mono); font-size: 12px; padding: 2px 0; border-bottom: 1px solid var(--border); }
    .port em { color: var(--muted); font-style: normal; }
    .ok { color: var(--ok); margin: 0; }
    .error { color: var(--danger); margin: 0; white-space: pre-wrap; font-size: 12px; }
  `,
})
export class ResourceEditorComponent {
  protected readonly editor = inject(ResourceEditorService);
  protected readonly defines = computed(() => DEFINES[this.editor.kind() ?? ''] ?? '');

  protected readonly examples = computed(() => {
    const examples = this.editor.lockCheck()?.examples;
    if (!examples) return null;
    if (failed(examples)) return { ok: false, text: examples.error };
    const n = examples.examples;
    return { ok: n > 0, text: n ? `${n} example${n === 1 ? '' : 's'} lock and unlock as they should.` : 'No examples(): give it some, and they\'re checked here and by cargo test.' };
  });

  protected readonly ports = computed(() => {
    const ports = this.editor.lockCheck()?.ports;
    if (!ports) return null;
    return failed(ports) ? { error: ports.error, inputs: [], outputs: [] } : { error: null, inputs: ports.inputs, outputs: ports.outputs };
  });
}
