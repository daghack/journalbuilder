import { Component, computed, effect, inject, input, model, signal, untracked, type OnDestroy } from '@angular/core';
import { pathOf } from '../journal/journal-files';
import {
  SETTING_GROUPS, SPAN_FIELDS, isTypeSetting, spanNameProblem, styleFile, stylePreviewScript, withSetting,
  type PreviewAs, type SettingField, type SettingKey, type SpanSettings, type StyleSettings,
} from '../journal/style-builder';
import { RenderService, type RenderProgress } from '../services/render.service';
import { ToolsService } from '../services/tools.service';
import { SettingFieldComponent, SettingOverridesComponent, type StyleChoices } from './style-fields.component';

/** How long changes pause before the preview is drawn again. */
const PREVIEW_DELAY_MS = 250;
/** How wide the preview writes. */
const PREVIEW_WIDTH = 1000;

/**
 * Every setting of a text style's ink, hand, type and paragraph, and its
 * spans, as sliders and choices, beside a preview drawn as they're set: what
 * New style shows, and what a style file opens in.
 */
@Component({
  selector: 'app-style-designer',
  imports: [SettingFieldComponent, SettingOverridesComponent],
  template: `
    <div class="settings">
      @for (group of groups; track group.label) {
        <fieldset>
          <legend>{{ group.label }}</legend>
          @for (field of group.fields; track field.key) {
            <app-setting-field [field]="field" [value]="valueOf(field)" [choices]="choices()" [disabled]="!!field.needs && !settings()[field.needs]" (changed)="setField(field, $event)" />
          }
        </fieldset>
      }
      <fieldset>
        <legend>Spans</legend>
        <p class="hint">How marked-up text changes when it's typeset: *text* is the span em, **text** is strong, and [text]{{ '{' }}name{{ '}' }} any named here. Each setting is over the text's.</p>
        @for (name of spanNames(); track name) {
          <div class="span">
            <div class="span-name">
              <code>{{ name }}</code>
              <button type="button" class="remove" (click)="removeSpan(name)" [attr.aria-label]="'Remove the span ' + name" [title]="'Remove the span ' + name">×</button>
            </div>
            <app-setting-overrides [fields]="spanFields" [values]="spanValues(name)" [choices]="choices()" (changed)="setSpan(name, $event)" />
          </div>
        }
        <div class="add-span">
          <input type="text" spellcheck="false" list="style-span-names" placeholder="A span's name: em, strong…" aria-label="A new span's name" [value]="newSpan()"
                 (input)="newSpan.set($any($event.target).value.trim())" (keydown.enter)="$event.preventDefault(); addSpan()" />
          <datalist id="style-span-names"><option value="em"></option><option value="strong"></option></datalist>
          <button type="button" [disabled]="!newSpan() || !!spanProblem()" (click)="addSpan()">Add</button>
        </div>
        @if (spanProblem(); as p) { <p class="error">{{ p }}</p> }
      </fieldset>
    </div>
    <div class="preview">
      <div class="bar">
        <span class="modes" role="group" aria-label="Preview with">
          <button type="button" [class.on]="mode() === 'svg'" (click)="mode.set('svg')" title="Quick: strokes as plain lines">SVG</button>
          <button type="button" [class.on]="mode() === 'darkly'" (click)="mode.set('darkly')" title="The brush's texture, rendered on the GPU">Darkly</button>
        </span>
        <span class="modes" role="group" aria-label="Preview as">
          <button type="button" [class.on]="previewAs() === 'block'" (click)="previewAs.set('block')" title="As write_block writes it: shows baseline wander, not what's only for typeset text">Block</button>
          <button type="button" [class.on]="previewAs() === 'typeset'" (click)="previewAs.set('typeset')" title="As a layout typesets it, as markdown: shows indent, paragraph gap, vertical align, fit, underline, strike and spans">Typeset</button>
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
        <summary>File</summary>
        <pre>{{ text() }}</pre>
      </details>
    </div>
  `,
  styles: `
    :host { display: grid; grid-template-columns: minmax(300px, 380px) minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); flex: 1; min-height: 0; min-width: 0; font-size: 12px; }
    .settings { overflow-y: auto; padding: 8px 12px; border-right: 1px solid var(--border); background: var(--surface); }
    fieldset { border: 0; padding: 0; margin: 0 0 10px; }
    legend { font-weight: 600; padding: 4px 0; }
    .hint { margin: 0 0 6px; color: var(--muted); }
    .span { margin-bottom: 8px; padding: 4px 8px; border: 1px solid var(--border); border-radius: 5px; }
    .span-name { display: flex; justify-content: space-between; align-items: center; }
    .span-name code { font-family: var(--mono); }
    .remove { border: 0; background: transparent; color: var(--muted); cursor: pointer; padding: 2px; }
    .add-span { display: flex; gap: 6px; }
    .add-span input { flex: 1; min-width: 0; font-family: var(--mono); }
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
    pre { margin: 6px 0 2px; font-family: var(--mono); font-size: 11px; white-space: pre-wrap; }
    .error { margin: 0; color: var(--danger); background: var(--danger-bg); padding: 4px 8px; border-radius: 4px; white-space: pre-wrap; }
    @media (max-width: 760px) {
      :host { grid-template-columns: minmax(0, 1fr); grid-template-rows: auto auto; overflow-y: auto; }
      .settings { border-right: 0; border-bottom: 1px solid var(--border); overflow-y: visible; }
      .preview { overflow-y: visible; }
    }
  `,
})
export class StyleDesignerComponent implements OnDestroy {
  private readonly tools = inject(ToolsService);
  private readonly render = inject(RenderService);

  /** The name to preview it as. */
  readonly name = input.required<string>();
  readonly settings = model.required<StyleSettings>();
  /** Whether it shows: it's only drawn then. */
  readonly showing = input(true);

  protected readonly groups = SETTING_GROUPS;
  protected readonly spanFields = SPAN_FIELDS;
  protected readonly mode = signal<'svg' | 'darkly'>('svg');
  protected readonly previewAs = signal<PreviewAs>('block');
  protected readonly choices = signal<StyleChoices>({ brushes: [], glyphsets: [] });
  protected readonly newSpan = signal('');

  protected readonly spanNames = computed(() => Object.keys(this.settings().spans));
  protected readonly spanProblem = computed(() => spanNameProblem(this.newSpan(), this.spanNames()));

  protected readonly text = computed(() => styleFile(this.settings()));
  /** What's drawn, for telling a drawing that's out of date. */
  protected readonly file = computed(() => `${this.previewAs()}\n${this.text()}`);

  /** The SVG preview: what it shows, and of what. */
  private readonly svg = signal<{ url: string | null; file: string } | null>(null);
  private readonly svgError = signal<string | null>(null);
  private readonly drawing = signal(false);
  /** The Darkly render: what it shows, and of what. */
  private readonly darkly = signal<{ url: string | null; file: string } | null>(null);
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
    const file = this.file();
    const darkly = this.darkly();
    if (this.mode() === 'darkly') return darkly?.file !== file;
    return this.svg()?.file !== file;
  });
  protected readonly busy = computed(() => this.drawing() || (this.mode() === 'darkly' && this.progress() !== null));
  protected readonly progressText = computed(() => {
    const p = this.progress();
    if (this.mode() === 'darkly' && p) return `Rendering with Darkly… ${Math.round(p.fraction * 100)}%`;
    return 'Drawing…';
  });
  protected readonly error = computed(() => this.svgError() ?? (this.mode() === 'darkly' ? this.darklyError() : null));

  constructor() {
    // What there is to pick from, each time it shows: the journal's own may have changed.
    effect(() => {
      if (!this.showing()) return;
      this.tools.styleChoices().then((choices) => this.choices.set(choices), (err: Error) => this.svgError.set(err.message));
    });
    // Draw the SVG preview when the style changes, once changes pause.
    effect(() => {
      this.file();
      this.name();
      if (!this.showing()) return;
      untracked(() => this.schedule('svg', () => this.drawSvg()));
    });
    // In Darkly mode, render it too: one at a time, the newest waiting.
    effect(() => {
      const file = this.file();
      if (!this.showing() || this.mode() !== 'darkly') return;
      if (untracked(this.darkly)?.file === file) return;
      untracked(() => this.schedule('darkly', () => this.renderDarkly()));
    });
  }

  protected set<K extends keyof StyleSettings>(key: K, value: StyleSettings[K]) {
    this.settings.update((s) => ({ ...s, [key]: value }));
  }

  protected valueOf(field: SettingField): unknown {
    return this.settings()[field.key as SettingKey];
  }

  /** A field set: the preview changes to one that shows it, if only one does. */
  protected setField(field: SettingField, value: unknown) {
    this.settings.update((s) => withSetting(s, field.key as SettingKey, value));
    if (field.preview) this.previewAs.set(field.preview);
  }

  /** A span's settings together. */
  protected spanValues(name: string): Record<string, unknown> {
    const span = this.settings().spans[name];
    return { ...span?.style, ...span?.type };
  }

  protected setSpan(name: string, values: Record<string, unknown>) {
    const parts = Object.entries(values);
    const style = parts.filter(([key]) => !isTypeSetting(key));
    const type = parts.filter(([key]) => isTypeSetting(key));
    const span: SpanSettings = { ...(style.length ? { style: Object.fromEntries(style) } : {}), ...(type.length ? { type: Object.fromEntries(type) } : {}) };
    this.set('spans', { ...this.settings().spans, [name]: span });
    this.previewAs.set('typeset');
  }

  protected addSpan() {
    const name = this.newSpan();
    if (!name || this.spanProblem()) return;
    this.setSpan(name, {});
    this.newSpan.set('');
  }

  protected removeSpan(name: string) {
    const { [name]: _, ...rest } = this.settings().spans;
    this.set('spans', rest);
  }

  private schedule(which: 'svg' | 'darkly', run: () => void) {
    const timer = which === 'svg' ? this.svgTimer : this.darklyTimer;
    if (timer) clearTimeout(timer);
    const next = setTimeout(run, PREVIEW_DELAY_MS);
    if (which === 'svg') this.svgTimer = next;
    else this.darklyTimer = next;
  }

  /** The entry that previews the style, and its file to run it with. */
  private previewOf() {
    const name = this.name();
    return {
      script: stylePreviewScript(name, this.settings(), PREVIEW_WIDTH, this.previewAs()),
      files: { [pathOf('styles', name)]: this.text() },
    };
  }

  private async drawSvg() {
    const asked = ++this.asked;
    const file = this.file();
    const { script, files } = this.previewOf();
    this.drawing.set(true);
    try {
      const preview = await this.tools.preview(script, files);
      if (asked !== this.asked) return;
      if (preview.error) {
        this.svgError.set(preview.error.message);
        return;
      }
      this.replace(this.svg, { url: preview.svg ? URL.createObjectURL(new Blob([preview.svg], { type: 'image/svg+xml' })) : null, file });
      this.svgError.set(null);
    } catch (err) {
      if (asked === this.asked) this.svgError.set((err as Error).message);
    } finally {
      if (asked === this.asked) this.drawing.set(false);
    }
  }

  private renderDarkly() {
    if (this.rendering) return;
    const file = this.file();
    const { script, files } = this.previewOf();
    this.rendering = true;
    this.darklyError.set(null);
    this.progress.set({ page: 0, pages: 1, fraction: 0 });
    let png: Blob | null = null;
    this.render.renderPages(script, null, [0], { scale: 0.5, parchment: false },
      (_, image) => (png = image),
      (p) => this.progress.set(p),
      files,
    ).then(
      () => this.replace(this.darkly, { url: png ? URL.createObjectURL(png) : null, file }),
      (err: Error) => {
        this.darklyError.set(err.message);
        // Not tried again for this file till it changes.
        this.replace(this.darkly, { url: null, file });
      },
    ).finally(() => {
      this.rendering = false;
      this.progress.set(null);
      // The style changed while it rendered: render it as it is now.
      if (this.showing() && this.mode() === 'darkly' && this.darkly()?.file !== this.file()) this.renderDarkly();
    });
  }

  /** Show `next` in `shown`, letting go of the image it had. */
  private replace(shown: typeof this.svg, next: { url: string | null; file: string }) {
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
