import { Component, input, output } from '@angular/core';
import { chain, rotate, scale, type Affine } from '../journal/canvas-edits';
import { cleanPlacement, placementAffine, withPlacement, type Placement } from './glyph-link';
import { num } from './glyph-path';

type Part = 'move-x' | 'move-y' | 'turn' | 'skew' | 'scale-x' | 'scale-y';

/**
 * A placement's numbers to type (move, turn, skew and scale, about the
 * glyph's origin), and quick turns and mirrors; each change gives the
 * placement whole.
 */
@Component({
  selector: 'app-placement-fields',
  template: `
    <div class="parts">
      @for (part of parts; track part.part) {
        <label [title]="part.title">
          <span>{{ part.label }}</span>
          <input type="number" [step]="part.step" [value]="value(part.part)" (change)="set(part.part, $any($event.target).value)" />
        </label>
      }
    </div>
    <div class="row">
      <button type="button" (click)="apply(mirrorX)" title="Mirror it left to right, across the glyph's middle line (x = 0)">Mirror ↔</button>
      <button type="button" (click)="apply(mirrorY)" title="Mirror it top to bottom, across the glyph's middle line (y = 0)">Mirror ↕</button>
      <button type="button" (click)="apply(quarter)" title="Turn it a quarter turn clockwise about the glyph's middle">Turn 90°</button>
      <button type="button" (click)="changed.emit({})" title="Put it back where it started: not moved, turned, scaled or skewed">Reset</button>
    </div>
  `,
  styles: `
    :host { display: flex; flex-direction: column; gap: 6px; }
    .parts { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px 10px; }
    label { display: grid; grid-template-columns: 4.6em 1fr; align-items: center; gap: 4px; }
    label span { color: var(--muted); }
    input { min-width: 0; }
  `,
})
export class PlacementFieldsComponent {
  readonly placement = input.required<Placement>();
  /** The placement as changed: its move, turn, scale and skew, those that do anything. */
  readonly changed = output<Placement>();

  protected readonly parts: { part: Part; label: string; step: number; title: string }[] = [
    { part: 'move-x', label: 'move x', step: 0.05, title: 'How far right it goes, in glyph units' },
    { part: 'move-y', label: 'move y', step: 0.05, title: 'How far down it goes, in glyph units' },
    { part: 'scale-x', label: 'scale x', step: 0.05, title: 'How much wider it is (negative mirrors it)' },
    { part: 'scale-y', label: 'scale y', step: 0.05, title: 'How much taller it is (negative mirrors it)' },
    { part: 'turn', label: 'turn °', step: 5, title: 'How far it turns clockwise, in degrees' },
    { part: 'skew', label: 'skew °', step: 5, title: 'How far it leans, in degrees: further down goes further right' },
  ];
  protected readonly mirrorX = scale(-1, 1);
  protected readonly mirrorY = scale(1, -1);
  protected readonly quarter = rotate(90);

  protected value(part: Part): string {
    const p = this.placement();
    const v = { 'move-x': p.move?.[0] ?? 0, 'move-y': p.move?.[1] ?? 0, turn: p.turn ?? 0, skew: p.skew ?? 0, 'scale-x': p.scale?.[0] ?? 1, 'scale-y': p.scale?.[1] ?? 1 }[part];
    return num(v);
  }

  protected set(part: Part, text: string) {
    const v = Number(text);
    if (text.trim() === '' || !Number.isFinite(v)) return;
    const p = parts(this.placement());
    const move: [number, number] = [...(p.move ?? [0, 0])];
    const scaled: [number, number] = [...(p.scale ?? [1, 1])];
    if (part === 'move-x') move[0] = v;
    else if (part === 'move-y') move[1] = v;
    else if (part === 'scale-x') scaled[0] = v || 0.0001;
    else if (part === 'scale-y') scaled[1] = v || 0.0001;
    else p[part] = v;
    this.changed.emit(cleanPlacement({ ...p, move, scale: scaled }));
  }

  /** It put through `t` after where it is. */
  protected apply(t: Affine) {
    const p = parts(this.placement());
    this.changed.emit(withPlacement(p, chain(placementAffine(p), t)));
  }
}

/** Only `p`'s placement. */
function parts(p: Placement): Placement {
  return { move: p.move, turn: p.turn, scale: p.scale, skew: p.skew };
}
