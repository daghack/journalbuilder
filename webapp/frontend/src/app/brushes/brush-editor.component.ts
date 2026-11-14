import { Component, ElementRef, effect, inject, signal, untracked, type OnDestroy } from '@angular/core';
import { PaneTabDirective, ThreePaneComponent, type PaneTab } from '../layout/three-pane.component';
import { LibraryBannerComponent } from '../resources/library-banner.component';
import { RenderService, type RenderProgress } from '../services/render.service';
import { rhaiString } from '../shared/rhai-literal';
import { BrushEditorService } from './brush-editor.service';
import { BrushGraphComponent } from './brush-graph.component';
import { setAbout } from './brush-model';
import { BrushPaletteComponent } from './brush-palette.component';

/** How long changes pause before the preview's rendered again. */
const PREVIEW_DELAY_MS = 500;

/** The entry that previews brush `name`: waves at three sizes, pressure tapering from full to a tenth, and a line of writing. */
export function brushPreviewScript(name: string): string {
  const brush = rhaiString(name);
  return `let page = new_page(#{ size: vec2(720, 400) });
let canvas = new_canvas();
let hand = new_handwriting_style();
let y = 50.0;
for size in [2.0, 5.0, 10.0] {
	let brush = new_brush_style("#2b1d12", ${brush}, size, #{ min_pressure: 0.1, max_pressure: 1.0, pressure_profile: "taper" });
	canvas.add(write_line(size.to_int().to_string(), vec2(20, y + 6), #{ type: new_type_style(glyphset("plain"), 18) }));
	canvas.add(new_wave(vec2(80, y), vec2(690, y), #{ amplitude: 18, waves: 3 }), new_style(brush, hand));
	y += 90;
}
let brush = new_brush_style("#9b2318", ${brush}, 2.5);
canvas.add(write_line("The quick brown fox", vec2(80, y), #{ style: new_style(brush, hand), type: new_type_style(glyphset("plain"), 34) }));
page.add(canvas);
journal_entry.add(page);
`;
}

/** The brush editor: the node types (a tab beside the files), the brush's nodes wired together, and its name, a Darkly preview and its file. */
@Component({
  selector: 'app-brush-editor',
  imports: [ThreePaneComponent, PaneTabDirective, BrushPaletteComponent, BrushGraphComponent, LibraryBannerComponent],
  providers: [BrushEditorService],
  host: { '(window:keydown)': 'onKey($event)' },
  template: `
    <app-three-pane key="brushes" [tabs]="tabs" [rightFraction]="0.3" [rightMin]="340">
      <app-brush-palette appPaneTab="nodes" />
      <section center class="centre">
        <app-library-banner [path]="editor.path()" [own]="editor.own()" [builtin]="editor.builtin()" (copy)="editor.copyIn()" (revert)="editor.useLibrary()" />
        <app-brush-graph class="graph" />
      </section>
      <aside right class="side">
        @if (editor.brush(); as brush) {
          <label>Name
            <input type="text" [value]="brush.name ?? ''" [placeholder]="editor.fileName()" spellcheck="false" (change)="about('name', $any($event.target).value)" />
          </label>
          <p class="note">Styles draw with it as <code>new_brush_style(colour, {{ quoted() }}, size)</code>.</p>
          @if (editor.clash(); as clash) {
            <p class="warn">
              @if (clash.own) { The journal's {{ clash.file }} is called {{ editor.brushName() }} too: rename one. }
              @else { This stands in for the library's {{ clash.file }}, called {{ editor.brushName() }} too. }
            </p>
          }
          <label>Description
            <textarea rows="3" [value]="brush.description ?? ''" (change)="about('description', $any($event.target).value)"></textarea>
          </label>
          <label>Author
            <input type="text" [value]="brush.author ?? ''" (change)="about('author', $any($event.target).value)" />
          </label>
        }
        @if (editor.problem(); as problem) { <p class="error">It can't be drawn with yet: {{ problem }}</p> }
        <h3>Preview <span class="state">{{ progressText() }}</span></h3>
        <div class="preview">
          @if (preview(); as url) { <img [src]="url" alt="The brush drawn by Darkly: waves at three sizes and a line of writing" [class.stale]="stale()" /> }
          @if (previewError(); as error) { <p class="error">{{ error }}</p> }
        </div>
        <h3>File
          <button type="button" (click)="copy()" title="Copy the brush's file, to paste into Darkly">Copy</button>
          <button type="button" [disabled]="yaml() === editor.text()" (click)="apply()" title="Use the text as the brush">Apply</button>
        </h3>
        <textarea class="yaml" spellcheck="false" [value]="yaml()" (input)="yaml.set($any($event.target).value)"></textarea>
        @if (pasteError(); as error) { <p class="error">{{ error }}</p> }
        <p class="note">A Darkly brush: paste one copied from Darkly here and apply it.</p>
      </aside>
    </app-three-pane>
  `,
  styles: `
    :host { display: block; min-height: 0; }
    .centre { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
    .graph { flex: 1; min-height: 0; }
    .side { display: flex; flex-direction: column; gap: 8px; padding: 10px; overflow-y: auto; min-height: 0; font-size: 12px; }
    label { display: flex; flex-direction: column; gap: 3px; font-weight: 600; }
    label input, label textarea { font-weight: normal; }
    h3 { display: flex; align-items: center; gap: 6px; margin: 8px 0 0; font-size: 12px; }
    h3 button:first-of-type { margin-left: auto; }
    .state { font-weight: normal; color: var(--muted); }
    .note { margin: 0; color: var(--muted); }
    .warn { margin: 0; color: var(--warning); }
    .error { margin: 0; color: var(--danger); white-space: pre-wrap; }
    .preview { min-height: 120px; background: #fff; border: 1px solid var(--border); border-radius: 4px; }
    .preview img { display: block; width: 100%; }
    .preview img.stale { opacity: 0.5; }
    .yaml { min-height: 240px; flex: 1; font: 11px var(--mono); white-space: pre; }
  `,
})
export class BrushEditorComponent implements OnDestroy {
  protected readonly editor = inject(BrushEditorService);
  private readonly render = inject(RenderService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  protected readonly tabs: PaneTab[] = [{ id: 'nodes', label: 'Nodes', title: 'The node types brushes are made of, to drag into the brush' }];

  /** The file's text as the YAML box holds it: the file's, till it's edited there. */
  protected readonly yaml = signal('');
  protected readonly pasteError = signal<string | null>(null);
  protected readonly preview = signal<string | null>(null);
  protected readonly previewError = signal<string | null>(null);
  private readonly progress = signal<RenderProgress | null>(null);
  /** The text the preview shows. */
  private readonly previewed = signal<string | null>(null);
  protected readonly stale = () => this.previewed() !== this.editor.text();
  protected readonly progressText = () => {
    const p = this.progress();
    return p ? `Rendering with Darkly… ${Math.round(p.fraction * 100)}%` : '';
  };
  protected readonly quoted = () => rhaiString(this.editor.brushName());
  private timer: ReturnType<typeof setTimeout> | null = null;
  private rendering = false;

  constructor() {
    effect(() => {
      const text = this.editor.text();
      untracked(() => {
        this.yaml.set(text);
        this.pasteError.set(null);
      });
    });
    // Render the preview when the brush changes, once changes pause; one at a time.
    effect(() => {
      const text = this.editor.text();
      const problem = this.editor.problem() ?? this.editor.readError();
      if (!this.editor.showing() || !this.editor.path() || problem || untracked(this.previewed) === text) return;
      untracked(() => {
        if (this.timer) clearTimeout(this.timer);
        this.timer = setTimeout(() => this.renderPreview(), PREVIEW_DELAY_MS);
      });
    });
  }

  private renderPreview() {
    const path = this.editor.path();
    if (this.rendering || !path) return;
    const text = this.editor.text();
    this.rendering = true;
    this.previewError.set(null);
    this.progress.set({ page: 0, pages: 1, fraction: 0 });
    let png: Blob | null = null;
    this.render.renderPages(brushPreviewScript(this.editor.brushName()), null, [0], { scale: 1, supersample: 2, parchment: false },
      (_, image) => (png = image),
      (p) => this.progress.set(p),
      { [path]: text },
    ).then(
      () => {
        const old = this.preview();
        if (old) URL.revokeObjectURL(old);
        this.preview.set(png ? URL.createObjectURL(png) : null);
      },
      (err: Error) => this.previewError.set(err.message),
    ).finally(() => {
      this.previewed.set(text);
      this.rendering = false;
      this.progress.set(null);
      // It changed while it rendered: render it as it is now.
      if (this.editor.showing() && this.editor.text() !== text && !this.editor.problem()) this.renderPreview();
    });
  }

  protected about(key: 'name' | 'description' | 'author', value: string) {
    const brush = this.editor.brush();
    if (brush) this.editor.change(setAbout(brush, key, value));
  }

  protected async apply() {
    this.pasteError.set(await this.editor.paste(this.yaml()));
  }

  protected copy() {
    void navigator.clipboard?.writeText(this.yaml());
  }

  /** Undo and redo, when the brush editor shows and no text field has the keyboard. */
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

  ngOnDestroy() {
    const url = this.preview();
    if (url) URL.revokeObjectURL(url);
    if (this.timer) clearTimeout(this.timer);
  }
}
