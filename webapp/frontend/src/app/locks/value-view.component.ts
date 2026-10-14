import { Component, computed, input } from '@angular/core';
import type { LockValue } from '../scribe';

/** A value from a lock run: text for symbols and strings, a maze drawn, anything else as JSON. */
@Component({
  selector: 'app-value-view',
  template: `
    @if (value(); as v) {
      @if (v.maze; as maze) {
        <svg class="maze" [attr.viewBox]="'-0.5 -0.5 ' + maze.width + ' ' + maze.height" [style.aspect-ratio]="maze.width + ' / ' + maze.height">
          <path [attr.d]="maze.path" />
          @for (cell of maze.cells; track cell.step) {
            <text [attr.x]="cell.x" [attr.y]="cell.y" font-size="0.55">{{ letter(cell.symbol) }}</text>
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
    .maze { width: 100%; max-width: 220px; display: block; background: var(--paper); border-radius: 4px; }
    .maze path { fill: none; stroke: #9b2318; stroke-width: 0.12; stroke-linecap: round; }
    .maze text { fill: #2b1d12; text-anchor: middle; dominant-baseline: central; font-family: var(--mono); }
  `,
})
export class ValueViewComponent {
  readonly value = input<LockValue | null>(null);
  protected readonly json = computed(() => JSON.stringify(this.value()?.json));

  protected letter(symbol: number): string {
    return symbol === 0 ? '·' : String.fromCharCode(64 + symbol);
  }
}
