import { Component, input, output } from '@angular/core';
import { ColourFieldComponent } from '../shared/colour-field.component';
import { isColour } from '../shared/colour';

/** An option a file declares, as the form shows it. */
export interface OptionField {
  name: string;
  kind: 'integer' | 'number' | 'string' | 'colour' | 'bool' | 'array' | 'one_of' | 'json';
  choices?: unknown[];
  /** What it is when left alone; undefined if it must be given. */
  default?: unknown;
}

/** Fields for a file's `options()` defaults, `#{ NAME: default }`, each typed as its default is (a colour if it's written as one). */
export function fieldsOf(defaults: Record<string, unknown> | undefined): OptionField[] {
  return Object.entries(defaults ?? {}).map(([name, value]) => ({
    name,
    default: value,
    kind: typeof value === 'number' ? (Number.isInteger(value) ? 'integer' : 'number')
      : typeof value === 'boolean' ? 'bool' : isColour(value) ? 'colour' : typeof value === 'string' ? 'string' : Array.isArray(value) ? 'array' : 'json',
  }));
}

/** The options a file takes: a field each, its value or its default. */
@Component({
  selector: 'app-options-form',
  imports: [ColourFieldComponent],
  template: `
    @for (field of fields(); track field.name) {
      <label class="field">
        <span class="name">{{ field.name }}
          @if (field.name in values()) { <button type="button" class="reset" (click)="changed.emit({ name: field.name, value: undefined })" title="Back to its default">↺</button> }
        </span>
        @switch (kindOf(field)) {
          @case ('bool') {
            <input type="checkbox" [checked]="value(field) === true" (change)="set(field, $any($event.target).checked)" />
          }
          @case ('one_of') {
            <select (change)="set(field, field.choices?.[$any($event.target).selectedIndex])">
              @for (choice of field.choices ?? []; track $index) { <option [selected]="choice === value(field)">{{ show(choice) }}</option> }
            </select>
          }
          @case ('integer') {
            <input type="number" step="1" [value]="value(field) ?? ''" (change)="set(field, number($any($event.target).value, true))" />
          }
          @case ('number') {
            <input type="number" step="any" [value]="value(field) ?? ''" (change)="set(field, number($any($event.target).value, false))" />
          }
          @case ('colour') {
            <app-colour-field [value]="show(value(field) ?? '')" [label]="field.name" (changed)="set(field, $event)" />
          }
          @case ('string') {
            <input type="text" [value]="value(field) ?? ''" (change)="set(field, $any($event.target).value)" spellcheck="false" />
          }
          @default {
            <input type="text" class="mono" [value]="json(value(field))" (change)="set(field, parse($any($event.target).value))" spellcheck="false" />
          }
        }
      </label>
    } @empty {
      <p class="hint">No options.</p>
    }
  `,
  styles: `
    :host { display: flex; flex-direction: column; gap: 8px; }
    .field { display: flex; flex-direction: column; align-items: stretch; gap: 2px; font-size: 12px; text-align: left; }
    .name { font-family: var(--mono); display: flex; align-items: center; gap: 4px; }
    .reset { background: transparent; border: 0; padding: 0 2px; color: var(--muted); cursor: pointer; }
    .mono { font-family: var(--mono); }
    input[type='checkbox'] { align-self: flex-start; }
  `,
})
export class OptionsFormComponent {
  readonly fields = input<OptionField[]>([]);
  /** The values given, by name; others are their defaults. */
  readonly values = input<Record<string, unknown>>({});
  readonly changed = output<{ name: string; value: unknown }>();

  /** A string option whose default is a colour gets a colour's field. */
  protected kindOf(field: OptionField): OptionField['kind'] {
    return field.kind === 'string' && isColour(field.default) ? 'colour' : field.kind;
  }

  protected value(field: OptionField): unknown {
    return field.name in this.values() ? this.values()[field.name] : field.default;
  }

  protected set(field: OptionField, value: unknown) {
    if (value === undefined) return;
    this.changed.emit({ name: field.name, value });
  }

  protected number(text: string, integer: boolean): number | undefined {
    const n = Number(text);
    if (text.trim() === '' || !Number.isFinite(n)) return undefined;
    return integer ? Math.round(n) : n;
  }

  protected show(value: unknown): string {
    return typeof value === 'string' ? value : JSON.stringify(value);
  }

  protected json(value: unknown): string {
    return value === undefined ? '' : JSON.stringify(value);
  }

  protected parse(text: string): unknown {
    try {
      return JSON.parse(text);
    } catch {
      return undefined;
    }
  }
}
