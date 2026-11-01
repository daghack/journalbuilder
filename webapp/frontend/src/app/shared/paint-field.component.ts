import { Component, computed, input, output } from '@angular/core';
import { ColourFieldComponent } from './colour-field.component';
import { LAYOUTS, between, gradientCss, isGradient, layoutOf, stopsOf, withLayout, withStops, type GradientData, type Layout, type Paint, type Stop } from './gradient';

/** What the other end of a new gradient is: the inks the journals are drawn in. */
const SECOND_COLOUR = ['#9b2318', '#2b1d12'];

/**
 * What strokes are drawn in: a colour, or a gradient of them. A gradient
 * shows its colours, each placed from 0 to 1, how it's laid over strokes,
 * and where on the canvas if it's laid across it. Not for inside a
 * `<label>`, which would press its first button when clicked.
 */
@Component({
  selector: 'app-paint-field',
  imports: [ColourFieldComponent],
  template: `
    <span class="modes" role="group" [attr.aria-label]="label() + ': a colour or a gradient'">
      <button type="button" [class.on]="!gradient()" (click)="toColour()" title="One colour">Colour</button>
      <button type="button" [class.on]="!!gradient()" (click)="toGradient()" title="Colours changing along strokes, or across the page">Gradient</button>
    </span>
    @if (gradient(); as g) {
      <div class="strip" aria-hidden="true"><span [style.background]="css()"></span></div>
      <select [attr.aria-label]="label() + ': how the gradient runs'" (change)="setLayout($any($event.target).value)">
        @for (l of layouts; track l.layout) { <option [value]="l.layout" [selected]="l.layout === layout()" [title]="l.title">{{ l.label }}</option> }
      </select>
      @for (stop of stops(); track $index) {
        <div class="stop">
          <input type="number" class="at" min="0" max="1" step="0.05" [value]="round(stop.at)" [attr.aria-label]="'Colour ' + ($index + 1) + ': where, from 0 to 1'" title="Where it is, from 0 to 1"
                 (change)="setAt($index, $any($event.target))" />
          <app-colour-field [value]="stop.colour" [label]="'Colour ' + ($index + 1)" (changed)="setColour($index, $event)" />
          <button type="button" class="remove" [disabled]="stops().length <= 2" (click)="remove($index)" [attr.aria-label]="'Remove colour ' + ($index + 1)" title="Remove this colour">×</button>
        </div>
      }
      <button type="button" class="add" (click)="add()" title="A colour halfway across the widest gap">Add a colour</button>
      @switch (layout()) {
        @case ('across') {
          <div class="point" title="Where it starts, in canvas units"><span>From</span>
            <input type="number" [value]="g.from?.[0] ?? 0" aria-label="From, across" (change)="setPoint('from', 0, $any($event.target).value)" />
            <input type="number" [value]="g.from?.[1] ?? 0" aria-label="From, down" (change)="setPoint('from', 1, $any($event.target).value)" />
          </div>
          <div class="point" title="Where it ends, in canvas units"><span>To</span>
            <input type="number" [value]="g.to?.[0] ?? 0" aria-label="To, across" (change)="setPoint('to', 0, $any($event.target).value)" />
            <input type="number" [value]="g.to?.[1] ?? 0" aria-label="To, down" (change)="setPoint('to', 1, $any($event.target).value)" />
          </div>
        }
        @case ('out') {
          <div class="point" title="Where it starts, in canvas units"><span>Centre</span>
            <input type="number" [value]="g.center?.[0] ?? 0" aria-label="Centre, across" (change)="setPoint('center', 0, $any($event.target).value)" />
            <input type="number" [value]="g.center?.[1] ?? 0" aria-label="Centre, down" (change)="setPoint('center', 1, $any($event.target).value)" />
          </div>
          <div class="point" title="How far it reaches, in canvas units"><span>Radius</span>
            <input type="number" min="0" [value]="g.radius ?? 0" aria-label="Radius" (change)="setRadius($any($event.target).value)" />
          </div>
        }
      }
    } @else {
      <app-colour-field [value]="colour()" [placeholder]="placeholder()" [label]="label()" (changed)="changed.emit($event)" />
    }
  `,
  styles: `
    :host { display: flex; flex-direction: column; gap: 4px; min-width: 0; flex: 1; }
    .modes { display: inline-flex; align-self: flex-start; border: 1px solid var(--border); border-radius: 5px; overflow: hidden; }
    .modes button { border: 0; border-radius: 0; padding: 2px 8px; background: var(--surface); color: var(--fg); cursor: pointer; font-size: 11px; }
    .modes button.on { background: var(--accent); color: var(--accent-fg); }
    .strip { height: 14px; border: 1px solid var(--border); border-radius: 4px; overflow: hidden;
      background: repeating-conic-gradient(#ccc 0 25%, #fff 0 50%) 0 0 / 8px 8px; }
    .strip > span { display: block; height: 100%; }
    select { min-width: 0; }
    .stop { display: flex; gap: 4px; align-items: center; min-width: 0; }
    .at { width: 54px; flex: none; font-family: var(--mono); }
    .remove { flex: none; padding: 0 6px; cursor: pointer; }
    .add { align-self: flex-start; cursor: pointer; }
    .point { display: flex; gap: 4px; align-items: center; min-width: 0; }
    .point > span { width: 44px; flex: none; color: var(--muted); }
    .point input { flex: 1; width: 0; min-width: 0; font-family: var(--mono); }
  `,
})
export class PaintFieldComponent {
  /** A colour or gradient; empty for none. */
  readonly value = input<Paint>('');
  /** The default shown when there's no value, which colours start from. */
  readonly placeholder = input<string>('');
  readonly label = input<string>('');
  readonly changed = output<Paint>();

  protected readonly layouts = LAYOUTS;
  protected readonly gradient = computed(() => { const v = this.value(); return isGradient(v) ? v : null; });
  protected readonly colour = computed(() => { const v = this.value(); return typeof v === 'string' ? v : ''; });
  protected readonly stops = computed(() => stopsOf(this.gradient() ?? { colors: [] }));
  protected readonly layout = computed(() => layoutOf(this.gradient() ?? { colors: [] }));
  protected readonly css = computed(() => gradientCss(this.gradient() ?? { colors: [] }));

  /** The gradient last shown, to go back to from a colour. */
  private kept: GradientData | null = null;

  protected toColour() {
    const g = this.gradient();
    if (!g) return;
    this.kept = g;
    this.changed.emit(this.stops()[0]?.colour ?? '');
  }

  /** A gradient from the colour to another, or the one it was until it was a colour. */
  protected toGradient() {
    if (this.gradient()) return;
    const colour = this.colour() || this.placeholder() || SECOND_COLOUR[1];
    this.changed.emit(this.kept ?? { colors: [colour, SECOND_COLOUR.find((c) => c !== colour)!] });
  }

  private setStops(stops: Stop[]) {
    this.changed.emit(withStops(this.gradient()!, stops));
  }

  protected setColour(index: number, colour: string) {
    this.setStops(this.stops().map((s, i) => (i === index ? { ...s, colour } : s)));
  }

  /** A place typed: kept from 0 to 1, what's shown put right if that changed it. */
  protected setAt(index: number, field: HTMLInputElement) {
    const typed = Number(field.value);
    const at = field.value.trim() === '' || !Number.isFinite(typed) ? this.stops()[index].at : Math.min(1, Math.max(0, typed));
    field.value = String(this.round(at));
    this.setStops(this.stops().map((s, i) => (i === index ? { ...s, at } : s)));
  }

  protected remove(index: number) {
    this.setStops(this.stops().filter((_, i) => i !== index));
  }

  /** A colour halfway across the widest gap between two, as the gradient has it there. */
  protected add() {
    const stops = this.stops();
    let widest = 0;
    for (let i = 1; i < stops.length - 1; i++) if (stops[i + 1].at - stops[i].at > stops[widest + 1].at - stops[widest].at) widest = i;
    const [a, b] = [stops[widest], stops[widest + 1]];
    this.setStops([...stops, { at: (a.at + b.at) / 2, colour: between(a.colour, b.colour) }]);
  }

  protected setLayout(layout: Layout) {
    this.changed.emit(withLayout(this.gradient()!, layout));
  }

  protected setPoint(key: 'from' | 'to' | 'center', axis: 0 | 1, typed: string) {
    const g = this.gradient()!;
    const n = Number(typed);
    if (typed.trim() === '' || !Number.isFinite(n)) return;
    const point: [number, number] = [...(g[key] ?? [0, 0])];
    point[axis] = n;
    this.changed.emit({ ...g, [key]: point });
  }

  protected setRadius(typed: string) {
    const n = Number(typed);
    if (typed.trim() !== '' && n > 0 && Number.isFinite(n)) this.changed.emit({ ...this.gradient()!, radius: n });
  }

  protected round(at: number): number {
    return +at.toFixed(3);
  }
}
