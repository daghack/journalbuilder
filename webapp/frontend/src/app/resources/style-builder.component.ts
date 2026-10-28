import { Component, ElementRef, computed, effect, inject, output, signal, untracked, viewChild, type OnDestroy } from '@angular/core';
import { VALID_NAME, pathOf } from '../journal/journal-files';
import { DEFAULT_STYLE, SETTING_GROUPS, styleSource, stylePreviewScript, type SettingField, type SettingKey, type StyleSettings } from '../journal/style-builder';
import { JournalStore } from '../services/journal-store.service';
import { RenderService, type RenderProgress } from '../services/render.service';
import { ToolsService, failed } from '../services/tools.service';
import { ColourFieldComponent } from '../shared/colour-field.component';

/** How long changes pause before the preview is drawn again. */
const PREVIEW_DELAY_MS = 250;
/** How wide the preview writes, as the style editor's does. */
const PREVIEW_WIDTH = 1000;
/** What the preview is checked as when the name isn't one yet. */
const PLACEHOLDER_NAME = 'new_style';

/**
 * New style: a dialog of the ink, hand, type and paragraph a text style
 * is made of, previewed as they're set, that writes the style file.
 */
@Component({
  selector: 'app-style-builder',
  imports: [ColourFieldComponent],
  template: `
    <dialog #dialog (close)="isOpen.set(false)" aria-label="New style">
      <form method="dialog" (submit)="$event.preventDefault(); create()">
        <header>
          <h2>New style</h2>
          <label class="name">Name <input #nameInput type="text" spellcheck="false" placeholder="letters, digits and _" [value]="name()" (input)="name.set($any($event.target).value)" /></label>
          <label class="about">About <input type="text" placeholder="how a kind of text looks." [value]="settings().description" (input)="set('description', $any($event.target).value)" /></label>
        </header>
        <div class="body">
          <div class="settings">
            @for (group of groups; track group.label) {
              <fieldset>
                <legend>{{ group.label }}</legend>
                @for (field of group.fields; track field.key) {
                  <label class="row" [title]="field.title">
                    <span>{{ field.label }}</span>
                    @switch (field.kind) {
                      @case ('colour') {
                        <app-colour-field [value]="text(field.key)" [label]="field.label" (changed)="set(field.key, $event)" />
                      }
                      @case ('choice') {
                        <select (change)="set(field.key, $any($event.target).value)">
                          @for (choice of choicesOf(field); track choice) { <option [value]="choice" [selected]="choice === settings()[field.key]">{{ choice }}</option> }
                        </select>
                      }
                      @case ('switch') {
                        <input type="checkbox" [checked]="settings()[field.key]" (change)="set(field.key, $any($event.target).checked)" />
                      }
                      @default {
                        <span class="range">
                          <input type="range" [min]="field.min" [max]="field.max" [step]="field.step" [value]="settings()[field.key]" (input)="setNumber(field, $any($event.target).value)" />
                          <input type="number" [step]="field.step" [value]="settings()[field.key]" (change)="setNumber(field, $any($event.target).value)" [attr.aria-label]="field.label" />
                        </span>
                      }
                    }
                  </label>
                }
              </fieldset>
            }
          </div>
          <div class="preview">
            <div class="bar">
              <span class="modes" role="group" aria-label="Preview with">
                <button type="button" [class.on]="mode() === 'svg'" (click)="mode.set('svg')" title="Quick: strokes as plain lines">SVG</button>
                <button type="button" [class.on]="mode() === 'darkly'" (click)="mode.set('darkly')" title="The brush's texture, rendered on the GPU">Darkly</button>
              </span>
              @if (busy()) { <span class="muted">{{ progressText() }}</span> }
            </div>
            <div class="sheet">
              @if (shownUrl(); as url) { <img [src]="url" alt="The style's example text" [class.stale]="stale()" /> }
              @else if (!busy()) { <p class="muted">Nothing drawn.</p> }
            </div>
            @if (error(); as e) { <p class="error">{{ e }}</p> }
            <label class="example">Preview text
              <textarea rows="3" placeholder="Lorem ipsum, as content(&quot;lorem&quot;) gives it" [value]="settings().example" (input)="set('example', $any($event.target).value)"></textarea>
            </label>
            <details>
              <summary>Code</summary>
              <pre>{{ source() }}</pre>
            </details>
          </div>
        </div>
        <footer>
          <button type="button" (click)="reset()" title="Back to the defaults">Reset</button>
          @if (problem(); as p) { <p class="error">{{ p }}</p> } @else { <span class="spacer"></span> }
          <button type="button" (click)="dialog.close()">Cancel</button>
          <button type="submit" class="primary" [disabled]="!!problem()">Create</button>
        </footer>
      </form>
    </dialog>
  `,
  styles: `
    dialog { border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: var(--fg); padding: 0;
             width: min(1100px, calc(100vw - 32px)); height: min(780px, calc(100vh - 32px)); font-size: 12px; }
    dialog[open] { display: flex; }
    dialog::backdrop { background: rgb(0 0 0 / 0.45); }
    form { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; }
    header { display: flex; flex-wrap: wrap; align-items: end; gap: 12px; padding: 14px 16px 10px; border-bottom: 1px solid var(--border); }
    h2 { margin: 0 8px 4px 0; font-size: 16px; }
    header label { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--muted); }
    .name input { font-family: var(--mono); width: 180px; }
    .about { flex: 1; min-width: 200px; }
    .body { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(300px, 380px) 1fr; }
    .settings { overflow-y: auto; padding: 8px 12px; border-right: 1px solid var(--border); }
    fieldset { border: 0; padding: 0; margin: 0 0 10px; }
    legend { font-weight: 600; padding: 4px 0; }
    .row { display: grid; grid-template-columns: 120px 1fr; align-items: center; gap: 8px; min-height: 26px; }
    .row > span:first-child { color: var(--muted); }
    .row select { min-width: 0; }
    .row input[type='checkbox'] { justify-self: start; }
    .range { display: flex; gap: 6px; align-items: center; min-width: 0; }
    .range input[type='range'] { flex: 1; min-width: 0; accent-color: var(--accent); }
    .range input[type='number'] { width: 72px; font-family: var(--mono); }
    .preview { display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; min-width: 0; min-height: 0; overflow-y: auto; background: var(--stage-bg); }
    .preview > * { flex: none; }
    .bar { display: flex; align-items: center; gap: 10px; }
    .modes { display: inline-flex; border: 1px solid var(--border); border-radius: 5px; overflow: hidden; }
    .modes button { border: 0; border-radius: 0; padding: 3px 10px; background: var(--surface); color: var(--fg); cursor: pointer; }
    .modes button.on { background: var(--accent); color: var(--accent-fg); }
    .bar .muted { color: var(--stage-fg); }
    .sheet { background: var(--paper); border-radius: 4px; box-shadow: var(--page-shadow); min-height: 160px; display: grid; place-items: center; padding: 8px; }
    .sheet img { display: block; width: 100%; height: auto; }
    .sheet img.stale { opacity: 0.55; }
    .sheet .muted { color: var(--paper-muted); margin: 0; }
    .example { display: flex; flex-direction: column; gap: 3px; color: var(--stage-fg); }
    .example textarea { resize: vertical; font: inherit; }
    details { background: var(--surface); border-radius: 4px; padding: 4px 8px; }
    summary { cursor: pointer; }
    pre { margin: 6px 0 2px; font-family: var(--mono); font-size: 11px; white-space: pre-wrap; tab-size: 4; }
    .error { margin: 0; color: var(--danger); white-space: pre-wrap; }
    .preview .error { background: var(--danger-bg); padding: 4px 8px; border-radius: 4px; }
    footer { display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-top: 1px solid var(--border); }
    footer .error, .spacer { flex: 1; }
    .primary { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
    .primary:disabled { opacity: 0.5; }
  `,
})
export class StyleBuilderComponent implements OnDestroy {
  private readonly store = inject(JournalStore);
  private readonly tools = inject(ToolsService);
  private readonly render = inject(RenderService);
  private readonly dialogRef = viewChild.required<ElementRef<HTMLDialogElement>>('dialog');
  private readonly nameInput = viewChild.required<ElementRef<HTMLInputElement>>('nameInput');

  /** The style file to make, called name: styles/NAME.rhai. */
  readonly created = output<{ name: string; source: string }>();

  protected readonly groups = SETTING_GROUPS;
  protected readonly name = signal('');
  protected readonly settings = signal<StyleSettings>({ ...DEFAULT_STYLE });
  protected readonly isOpen = signal(false);
  protected readonly mode = signal<'svg' | 'darkly'>('svg');
  private readonly choices = signal<{ brushes: string[]; glyphsets: string[] }>({ brushes: [], glyphsets: [] });

  protected readonly source = computed(() => styleSource(this.name().trim() || PLACEHOLDER_NAME, this.settings()));

  protected readonly problem = computed(() => {
    const name = this.name().trim();
    if (!name) return 'Give it a name.';
    if (!VALID_NAME.test(name)) return 'A name is letters, digits and _ only.';
    if (pathOf('styles', name, 'rhai') in this.store.files()) return `There's a styles/${name}.rhai already.`;
    return null;
  });

  /** The SVG preview: what it shows, and for which source. */
  private readonly svg = signal<{ url: string | null; source: string } | null>(null);
  private readonly svgError = signal<string | null>(null);
  private readonly drawing = signal(false);
  /** The Darkly render: what it shows, and for which source. */
  private readonly darkly = signal<{ url: string | null; source: string } | null>(null);
  private readonly darklyError = signal<string | null>(null);
  private readonly progress = signal<RenderProgress | null>(null);
  private rendering = false;
  private svgTimer: ReturnType<typeof setTimeout> | null = null;
  private darklyTimer: ReturnType<typeof setTimeout> | null = null;
  private asked = 0;

  protected readonly shownUrl = computed(() => {
    const darkly = this.darkly();
    if (this.mode() === 'darkly' && darkly?.url) return darkly.url;
    return this.svg()?.url ?? null;
  });
  protected readonly stale = computed(() => {
    const source = this.source();
    const darkly = this.darkly();
    if (this.mode() === 'darkly') return darkly?.source !== source;
    return this.svg()?.source !== source;
  });
  protected readonly busy = computed(() => this.drawing() || (this.mode() === 'darkly' && this.progress() !== null));
  protected readonly progressText = computed(() => {
    const p = this.progress();
    if (this.mode() === 'darkly' && p) return `Rendering with Darkly… ${Math.round(p.fraction * 100)}%`;
    return 'Drawing…';
  });
  protected readonly error = computed(() => this.svgError() ?? (this.mode() === 'darkly' ? this.darklyError() : null));

  constructor() {
    // Draw the SVG preview when the style changes, once changes pause.
    effect(() => {
      const source = this.source();
      const name = this.name().trim();
      if (!this.isOpen()) return;
      untracked(() => this.schedule('svg', () => this.drawSvg(VALID_NAME.test(name) ? name : PLACEHOLDER_NAME, source)));
    });
    // In Darkly mode, render it too: one at a time, the newest waiting.
    effect(() => {
      const source = this.source();
      if (!this.isOpen() || this.mode() !== 'darkly') return;
      if (untracked(this.darkly)?.source === source) return;
      untracked(() => this.schedule('darkly', () => this.renderDarkly()));
    });
  }

  /** Show the builder, its name blank and its settings as last left. */
  open() {
    this.name.set('');
    this.isOpen.set(true);
    this.dialogRef().nativeElement.showModal();
    const name = this.nameInput().nativeElement;
    name.value = '';
    name.focus();
    if (!this.choices().brushes.length) {
      this.tools.styleChoices().then((choices) => this.choices.set(choices), (err: Error) => this.svgError.set(err.message));
    }
  }

  protected get dialog(): HTMLDialogElement {
    return this.dialogRef().nativeElement;
  }

  protected choicesOf(field: SettingField): string[] {
    const choices = field.choices ?? [];
    const list = choices === 'brushes' ? this.choices().brushes : choices === 'glyphsets' ? this.choices().glyphsets : choices;
    // The current choice stays shown while the engine's lists load.
    const current = String(this.settings()[field.key]);
    return list.includes(current) ? list : [current, ...list];
  }

  protected text(key: SettingKey): string {
    return String(this.settings()[key]);
  }

  protected set<K extends keyof StyleSettings>(key: K, value: StyleSettings[K]) {
    this.settings.update((s) => ({ ...s, [key]: value }));
  }

  /** A slider or number moved: min_pressure and max_pressure keep in order, the other giving way. */
  protected setNumber(field: SettingField, typed: string) {
    const n = Number(typed);
    if (!Number.isFinite(n)) return;
    this.settings.update((s) => {
      const next = { ...s, [field.key]: n };
      if (field.key === 'min_pressure' && n > s.max_pressure) next.max_pressure = n;
      if (field.key === 'max_pressure' && n < s.min_pressure) next.min_pressure = n;
      return next;
    });
  }

  protected reset() {
    this.settings.set({ ...DEFAULT_STYLE });
  }

  protected create() {
    if (this.problem()) return;
    this.created.emit({ name: this.name().trim(), source: this.source() });
    this.dialog.close();
  }

  private schedule(which: 'svg' | 'darkly', run: () => void) {
    const timer = which === 'svg' ? this.svgTimer : this.darklyTimer;
    if (timer) clearTimeout(timer);
    const next = setTimeout(run, PREVIEW_DELAY_MS);
    if (which === 'svg') this.svgTimer = next;
    else this.darklyTimer = next;
  }

  private async drawSvg(name: string, source: string) {
    const asked = ++this.asked;
    this.drawing.set(true);
    try {
      const preview = await this.tools.stylePreview(name, source, {}, PREVIEW_WIDTH);
      if (asked !== this.asked) return;
      if (failed(preview)) {
        this.svgError.set(preview.error);
        return;
      }
      const svg = preview.pages[0]?.svg;
      this.replace(this.svg, { url: svg ? URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })) : null, source });
      this.svgError.set(null);
    } catch (err) {
      if (asked === this.asked) this.svgError.set((err as Error).message);
    } finally {
      if (asked === this.asked) this.drawing.set(false);
    }
  }

  private renderDarkly() {
    if (this.rendering) return;
    const source = this.source();
    this.rendering = true;
    this.darklyError.set(null);
    this.progress.set({ page: 0, pages: 1, fraction: 0 });
    let png: Blob | null = null;
    this.render.renderPages(stylePreviewScript(source, PREVIEW_WIDTH), [0], { scale: 0.5, parchment: false },
      (_, image) => (png = image),
      (p) => this.progress.set(p),
    ).then(
      () => this.replace(this.darkly, { url: png ? URL.createObjectURL(png) : null, source }),
      (err: Error) => {
        this.darklyError.set(err.message);
        // Not tried again for this source till it changes.
        this.replace(this.darkly, { url: null, source });
      },
    ).finally(() => {
      this.rendering = false;
      this.progress.set(null);
      // The style changed while it rendered: render it as it is now.
      if (this.isOpen() && this.mode() === 'darkly' && this.darkly()?.source !== this.source()) this.renderDarkly();
    });
  }

  /** Show `next` in `shown`, letting go of the image it had. */
  private replace(shown: typeof this.svg, next: { url: string | null; source: string }) {
    const old = shown()?.url;
    if (old) URL.revokeObjectURL(old);
    shown.set(next);
  }

  ngOnDestroy() {
    for (const shown of [this.svg(), this.darkly()]) if (shown?.url) URL.revokeObjectURL(shown.url);
    if (this.svgTimer) clearTimeout(this.svgTimer);
    if (this.darklyTimer) clearTimeout(this.darklyTimer);
  }
}
