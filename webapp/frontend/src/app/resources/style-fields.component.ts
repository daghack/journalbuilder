import { Component, computed, forwardRef, input, output } from '@angular/core';
import {
  RULE_COUNT_FIELD, RULE_KIND_FIELD, RULE_SPACING_FIELD, STYLE_FIELDS, numberOf, pairOf, readRule, ruleValue, startingValue,
  type Rule, type SettingField,
} from '../journal/style-builder';
import { PaintFieldComponent } from '../shared/paint-field.component';

/** What a style's brush and glyphset can be picked from: the engine's lists. */
export interface StyleChoices {
  brushes: string[];
  glyphsets: string[];
}

/** One setting of a text style, as its field says: changed as it's set. */
@Component({
  selector: 'app-setting-field',
  imports: [PaintFieldComponent, forwardRef(() => SettingOverridesComponent)],
  template: `
    @let f = field();
    @switch (f.kind) {
      @case ('colour') {
        <div class="row paint" [title]="f.title">
          <span>{{ f.label }}</span>
          <app-paint-field [value]="$any(value())" [label]="f.label" (changed)="changed.emit($event)" />
        </div>
      }
      @case ('choice') {
        <label class="row" [title]="f.title">
          <span>{{ f.label }}</span>
          <select (change)="changed.emit($any($event.target).value || null)">
            @if (f.unset; as unset) { <option value="" [selected]="value() === null">{{ unset }}</option> }
            @for (choice of list(); track choice) { <option [value]="choice" [selected]="choice === value()">{{ choice }}</option> }
          </select>
        </label>
      }
      @case ('switch') {
        <label class="row" [title]="f.title">
          <span>{{ f.label }}</span>
          @if (f.unset; as unset) {
            <select (change)="pickSwitch($any($event.target).value)">
              <option value="" [selected]="value() === null">{{ unset }}</option>
              <option value="on" [selected]="value() === true">on</option>
              <option value="off" [selected]="value() === false">off</option>
            </select>
          } @else {
            <input type="checkbox" [checked]="value()" (change)="changed.emit($any($event.target).checked)" />
          }
        </label>
      }
      @case ('pair') {
        <div class="row" [title]="f.title">
          <span>{{ f.label }}</span>
          <div class="pair">
            @for (way of ways; track way; let i = $index) {
              <span class="range">
                <em>{{ way }}</em>
                <input type="range" [min]="f.min" [max]="f.max" [step]="f.step" [value]="pair()[i]" (input)="setPair(i, $any($event.target).value)" [attr.aria-label]="f.label + ', ' + way" />
                <input type="number" [step]="f.step" [value]="pair()[i]" (change)="setPair(i, $any($event.target).value)" [attr.aria-label]="f.label + ', ' + way" />
              </span>
            }
          </div>
        </div>
      }
      @case ('rule') {
        <div class="row" [title]="f.title">
          <span>{{ f.label }}</span>
          <span class="range">
            <input type="range" [min]="count.min" [max]="count.max" [step]="count.step" [value]="rule().count" (input)="setRule({ count: whole($any($event.target).value) })" [attr.aria-label]="f.label + ', how many lines'" />
            <input type="number" [min]="count.min" [max]="count.max" [step]="count.step" [value]="rule().count" (change)="setRule({ count: whole($any($event.target).value) })" [attr.aria-label]="f.label + ', how many lines'" />
          </span>
        </div>
        @if (rule().count) {
          <div class="under">
            @if (f.key === 'strike') {
              <app-setting-field [field]="kind" [value]="rule().kind" [choices]="choices()" (changed)="setRule({ kind: $any($event) ?? 'lines' })" />
            }
            <app-setting-field [field]="spacing" [value]="rule().spacing" [choices]="choices()" (changed)="setRule({ spacing: $any($event) })" />
            <app-setting-overrides [fields]="ruleStyleFields" [values]="rule().style" [choices]="choices()" adding="Draw the lines differently…" (changed)="setRule({ style: $event })" />
          </div>
        }
      }
      @default {
        <div class="row" [title]="f.title">
          <span>{{ f.label }}</span>
          <span class="range" [class.unset]="number() === null">
            <input type="range" [min]="f.min" [max]="f.max" [step]="f.step" [value]="number() ?? f.min" [disabled]="disabled()" (input)="setNumber($any($event.target).value)" [attr.aria-label]="f.label" />
            <input type="number" [step]="f.step" [value]="number()" [placeholder]="f.unset ?? ''" [disabled]="disabled()" (change)="setNumber($any($event.target).value)" [attr.aria-label]="f.label" />
            @if (f.unset && number() !== null) {
              <button type="button" class="clear" (click)="changed.emit(unsetValue())" [title]="'Back to ' + f.unset" [attr.aria-label]="f.label + ': back to ' + f.unset">×</button>
            }
          </span>
        </div>
      }
    }
  `,
  styles: `
    :host { display: block; }
    .row { display: grid; grid-template-columns: 120px 1fr; align-items: center; gap: 8px; min-height: 26px; }
    .row > span:first-child { color: var(--muted); }
    .row.paint { grid-template-columns: 52px 1fr; align-items: start; padding: 3px 0; }
    select { min-width: 0; }
    input[type='checkbox'] { justify-self: start; }
    .pair { display: flex; flex-direction: column; min-width: 0; }
    .range { display: flex; gap: 6px; align-items: center; min-width: 0; min-height: 26px; }
    .range em { font-style: normal; color: var(--muted); width: 40px; flex: none; }
    .range input[type='range'] { flex: 1; min-width: 0; accent-color: var(--accent); }
    .range input[type='number'] { width: 72px; font-family: var(--mono); }
    .range.unset input[type='range'] { opacity: 0.4; }
    .clear { border: 0; background: transparent; color: var(--muted); cursor: pointer; padding: 0 2px; }
    .under { margin: 0 0 6px 8px; padding-left: 8px; border-left: 2px solid var(--border); }
  `,
})
export class SettingFieldComponent {
  readonly field = input.required<SettingField>();
  readonly value = input<unknown>();
  readonly choices = input<StyleChoices>({ brushes: [], glyphsets: [] });
  readonly disabled = input(false);
  readonly changed = output<unknown>();

  protected readonly ways = ['across', 'down'];
  protected readonly count = RULE_COUNT_FIELD;
  protected readonly spacing = RULE_SPACING_FIELD;
  protected readonly kind = RULE_KIND_FIELD;
  protected readonly ruleStyleFields = STYLE_FIELDS.map((field) => ({ ...field, preview: undefined }));

  protected readonly number = computed(() => numberOf(this.value()));
  protected readonly pair = computed(() => pairOf(this.value()));
  protected readonly rule = computed(() => readRule(this.value()));

  protected readonly list = computed(() => {
    const choices = this.field().choices ?? [];
    const list = choices === 'brushes' ? this.choices().brushes : choices === 'glyphsets' ? this.choices().glyphsets : choices;
    // The current choice stays shown while the engine's lists load.
    const current = this.value();
    return typeof current !== 'string' || list.includes(current) ? list : [current, ...list];
  });

  protected unsetValue() {
    return this.field().unsetValue ?? null;
  }

  protected pickSwitch(picked: string) {
    this.changed.emit(picked === '' ? null : picked === 'on');
  }

  protected whole(typed: string): number {
    return Math.max(0, Math.round(Number(typed)) || 0);
  }

  /** A slider or number moved; a number emptied is unset, if it can be. */
  protected setNumber(typed: string) {
    if (!typed.trim()) {
      if (this.field().unset) this.changed.emit(this.unsetValue());
      return;
    }
    const n = Number(typed);
    if (Number.isFinite(n)) this.changed.emit(this.field().whole ? Math.round(n) : n);
  }

  protected setPair(way: number, typed: string) {
    const n = Number(typed);
    if (!Number.isFinite(n)) return;
    const pair = this.pair();
    this.changed.emit(way === 0 ? [n, pair[1]] : [pair[0], n]);
  }

  protected setRule(part: Partial<Rule>) {
    this.changed.emit(ruleValue({ ...this.rule(), ...part }));
  }
}

/**
 * Some settings, set over what's under them (the text's), the rest left to
 * it: an underline's ink, or a span's. Each can be taken off again.
 */
@Component({
  selector: 'app-setting-overrides',
  imports: [forwardRef(() => SettingFieldComponent)],
  template: `
    @for (field of set(); track field.key) {
      <div class="set">
        <app-setting-field [field]="field" [value]="values()[field.key]" [choices]="choices()" (changed)="change(field.key, $event)" />
        <button type="button" class="remove" (click)="remove(field.key)" [title]="'Take ' + field.label + ' off: as the text has it'" [attr.aria-label]="'Take ' + field.label + ' off'">×</button>
      </div>
    }
    <select class="add" [attr.aria-label]="adding()" (change)="add($any($event.target))">
      <option value="">{{ adding() }}</option>
      @for (field of left(); track field.key) { <option [value]="field.key" [title]="field.title">{{ field.label }}</option> }
    </select>
  `,
  styles: `
    :host { display: block; }
    .set { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: start; gap: 4px; }
    .remove { border: 0; background: transparent; color: var(--muted); cursor: pointer; padding: 4px 2px; }
    .add { margin: 3px 0; max-width: 100%; }
  `,
})
export class SettingOverridesComponent {
  readonly fields = input.required<readonly SettingField[]>();
  readonly values = input.required<Record<string, unknown>>();
  readonly choices = input<StyleChoices>({ brushes: [], glyphsets: [] });
  /** What the list of settings to add says. */
  readonly adding = input('Add a setting…');
  readonly changed = output<Record<string, unknown>>();

  protected readonly set = computed(() => this.fields().filter((field) => field.key in this.values()));
  protected readonly left = computed(() => this.fields().filter((field) => !(field.key in this.values())));

  protected change(key: string, value: unknown) {
    this.changed.emit({ ...this.values(), [key]: value });
  }

  protected remove(key: string) {
    const { [key]: _, ...rest } = this.values();
    this.changed.emit(rest);
  }

  protected add(picker: HTMLSelectElement) {
    const field = this.fields().find((f) => f.key === picker.value);
    picker.value = '';
    if (field) this.change(field.key, startingValue(field));
  }
}
