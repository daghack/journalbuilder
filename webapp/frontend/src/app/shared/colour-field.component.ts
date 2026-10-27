import { Component, computed, input, output, signal } from '@angular/core';
import { formatHsl, parseColour, type Hsla } from './colour';

let nextId = 0;

/**
 * A colour option: a swatch and its text. The swatch opens hue,
 * saturation, lightness and alpha sliders, which give HSL; text typed is
 * given as it's typed.
 */
@Component({
  selector: 'app-colour-field',
  template: `
    <button #swatch type="button" class="swatch" [attr.popovertarget]="id" [attr.aria-label]="'Pick ' + (label() || 'a colour')" title="Pick a colour">
      <span [style.background]="shown()"></span>
    </button>
    <input type="text" class="mono" [class.invalid]="!valid()" [value]="text()" [placeholder]="placeholder()" [attr.aria-label]="label() || null"
           (change)="changed.emit($any($event.target).value)" spellcheck="false" />
    <div #pop [id]="id" popover class="picker" (click)="$event.stopPropagation()" (beforetoggle)="toggled($event, swatch, pop)">
      <div class="preview"><span [style.background]="css()"></span></div>
      <div class="slider"><span>Hue</span>
        <input type="range" min="0" max="360" step="1" [value]="hsla().h" [style.background]="tracks().h" (input)="drag('h', $any($event.target).value)" (change)="commit()" />
        <span class="mono">{{ round(hsla().h) }}</span></div>
      <div class="slider"><span>Saturation</span>
        <input type="range" min="0" max="100" step="1" [value]="hsla().s * 100" [style.background]="tracks().s" (input)="drag('s', $any($event.target).value / 100)" (change)="commit()" />
        <span class="mono">{{ round(hsla().s * 100) }}%</span></div>
      <div class="slider"><span>Lightness</span>
        <input type="range" min="0" max="100" step="1" [value]="hsla().l * 100" [style.background]="tracks().l" (input)="drag('l', $any($event.target).value / 100)" (change)="commit()" />
        <span class="mono">{{ round(hsla().l * 100) }}%</span></div>
      <div class="slider"><span>Alpha</span>
        <input type="range" min="0" max="1" step="0.01" [value]="hsla().a"[style.background]="tracks().a" (input)="drag('a', +$any($event.target).value)" (change)="commit()" />
        <span class="mono">{{ hsla().a.toFixed(2) }}</span></div>
      <p class="mono">{{ css() }}</p>
    </div>
  `,
  styles: `
    :host { display: flex; flex: 1; min-width: 0; gap: 4px; align-items: center; }
    .mono { font-family: var(--mono); }
    input[type='text'] { flex: 1; min-width: 0; }
    input.invalid { border-color: var(--danger); }
    .swatch, .preview { flex: none; padding: 0; border: 1px solid var(--border); border-radius: 4px; cursor: pointer; overflow: hidden;
      background: repeating-conic-gradient(#ccc 0 25%, #fff 0 50%) 0 0 / 8px 8px; }
    .swatch { width: 26px; height: 22px; }
    .swatch > span, .preview > span { display: block; width: 100%; height: 100%; }
    .picker { position: fixed; inset: auto; margin: 0; width: 240px; padding: 8px; box-sizing: border-box;
      background: var(--surface); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; box-shadow: 0 6px 18px rgb(0 0 0 / 0.25); font-size: 12px; }
    .picker:popover-open { display: flex; flex-direction: column; gap: 6px; }
    .preview { height: 32px; cursor: default; }
    .picker .slider { display: grid; grid-template-columns: 68px 1fr 36px; align-items: center; gap: 6px; }
    .picker .slider > .mono { text-align: right; }
    .picker p { margin: 0; text-align: center; user-select: all; }
    input[type='range'] { appearance: none; -webkit-appearance: none; width: 100%; min-width: 0; height: 12px; margin: 0; border-radius: 6px; border: 1px solid var(--border); }
    input[type='range']::-webkit-slider-thumb { -webkit-appearance: none; width: 8px; height: 16px; border-radius: 3px; background: #fff; border: 1px solid #333; box-shadow: 0 0 2px rgb(0 0 0 / 0.4); }
    input[type='range']::-moz-range-thumb { width: 6px; height: 14px; border-radius: 3px; background: #fff; border: 1px solid #333; }
  `,
})
export class ColourFieldComponent {
  /** The colour given; empty for none. */
  readonly value = input<string>('');
  /** Shown when there's no value: the default, which the sliders start from. */
  readonly placeholder = input<string>('');
  readonly label = input<string>('');
  /** Text typed, as typed; or the sliders' colour, as HSL. */
  readonly changed = output<string>();

  protected readonly id = `colour-picker-${nextId++}`;
  /** The sliders' colour while they're dragged; null when it's the value's. */
  private readonly draft = signal<Hsla | null>(null);

  protected readonly hsla = computed(() => this.draft() ?? parseColour(this.value() || this.placeholder()) ?? { h: 0, s: 0, l: 0, a: 1 });
  protected readonly css = computed(() => formatHsl(this.hsla()));
  protected readonly text = computed(() => (this.draft() ? this.css() : this.value()));
  /** What the swatch shows: the text, if the browser can draw it (named colours too). */
  protected readonly shown = computed(() => {
    const text = this.text() || this.placeholder();
    return this.cssColour(text) ? text : 'transparent';
  });
  protected readonly valid = computed(() => !this.value() || this.cssColour(this.value()));

  /** Each slider's track: the colour as that slider moves across it. */
  protected readonly tracks = computed(() => {
    const { h, s, l } = this.hsla();
    const at = (h: number, s: number, l: number, a = 1) => formatHsl({ h, s, l, a });
    const hues = [0, 60, 120, 180, 240, 300, 359.4].map((x) => at(x, s, l)).join(', ');
    return {
      h: `linear-gradient(to right, ${hues})`,
      s: `linear-gradient(to right, ${at(h, 0, l)}, ${at(h, 1, l)})`,
      l: `linear-gradient(to right, ${at(h, s, 0)}, ${at(h, s, 0.5)}, ${at(h, s, 1)})`,
      a: `linear-gradient(to right, ${at(h, s, l, 0)}, ${at(h, s, l)}), repeating-conic-gradient(#ccc 0 25%, #fff 0 50%) 0 0 / 8px 8px`,
    };
  });

  protected drag(part: keyof Hsla, value: number) {
    this.draft.set({ ...this.hsla(), [part]: Number(value) });
  }

  protected commit() {
    const draft = this.draft();
    if (draft) this.changed.emit(formatHsl(draft));
  }

  /** Opening: below the swatch, or above it if there's no room; closing: the value's colour again. */
  protected toggled(event: Event, swatch: HTMLElement, pop: HTMLElement) {
    if ((event as ToggleEvent).newState !== 'open') {
      this.draft.set(null);
      return;
    }
    const r = swatch.getBoundingClientRect();
    const height = 220;
    pop.style.left = `${Math.max(8, Math.min(r.left, window.innerWidth - 248))}px`;
    pop.style.top = `${r.bottom + 4 + height > window.innerHeight ? Math.max(8, r.top - 4 - height) : r.bottom + 4}px`;
  }

  protected round(v: number): number {
    return Math.round(v);
  }

  private cssColour(text: string): boolean {
    return typeof CSS !== 'undefined' && CSS.supports('color', text);
  }
}
