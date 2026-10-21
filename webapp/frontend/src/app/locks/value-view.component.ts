import { Component, computed, input } from '@angular/core';
import type { LockValue } from '../scribe';

/** A value from a lock run: text for symbols and strings, a value kind's sketch drawn (a maze, a star chart), anything else as JSON. */
@Component({
  selector: 'app-value-view',
  template: `
    @if (value(); as v) {
      @if (v.art; as art) {
        <svg class="art" [attr.viewBox]="art.viewBox.join(' ')" [style.aspect-ratio]="art.viewBox[2] + ' / ' + art.viewBox[3]">
          @for (part of art.parts; track part.name) {
            <path [attr.d]="part.d" [attr.class]="'part-' + part.name" [attr.stroke-width]="art.labelSize / 8" />
          }
          @for (label of art.labels; track $index) {
            <text [attr.x]="label.x" [attr.y]="label.y" [attr.font-size]="art.labelSize">{{ label.text }}</text>
          }
        </svg>
      } @else if (v.text !== undefined) {
        <code class="text">{{ v.text === '' ? '(empty)' : v.text }}</code>
      } @else {
        <code class="json">{{ json() }}</code>
      }
    }
  `,
  styles: `
    :host { display: block; min-width: 0; }
    .text, .json { font: 12px var(--mono); white-space: pre-wrap; word-break: break-all; }
    .art { width: 100%; max-width: 320px; display: block; background: var(--paper); border-radius: 4px; }
    .art path { fill: none; stroke: #2b1d12; stroke-linecap: round; }
    .art path:nth-of-type(even) { stroke: #9b2318; }
    .art text { fill: #9b2318; text-anchor: middle; dominant-baseline: central; font-family: var(--mono); }
  `,
})
export class ValueViewComponent {
  readonly value = input<LockValue | null>(null);
  protected readonly json = computed(() => JSON.stringify(this.value()?.json));
}
