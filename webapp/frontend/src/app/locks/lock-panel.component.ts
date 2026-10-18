import { Component, computed, effect, inject, signal } from '@angular/core';
import type { LockOption } from '../scribe';
import { failed } from '../services/tools.service';
import { symbolBase, textSymbols, type TestValue } from './chain-model';
import { optionDeclaration } from './chain-source';
import { LockEditorService } from './lock-editor.service';
import { ValueViewComponent } from './value-view.component';

/** A test or fixed value as typed: text for symbols and strings, else JSON. */
function typed(value: TestValue | undefined): string {
  if (!value) return '';
  return 'text' in value ? value.text : JSON.stringify(value.json);
}

/** What's typed as a value for a port of `type`, and what's wrong with it. */
function parseValue(text: string, type: string): { value: TestValue; problem: string | null } {
  const base = symbolBase(type);
  if (base !== null && base <= 27) {
    try {
      textSymbols(base, text);
      return { value: { text }, problem: null };
    } catch (err) {
      return { value: { text }, problem: (err as Error).message };
    }
  }
  if (type === 'String') return { value: { text }, problem: null };
  try {
    return { value: { json: JSON.parse(text) }, problem: null };
  } catch {
    return { value: { text }, problem: 'not JSON' };
  }
}

/** The right: the lock type being made, the picked lock's options, the chain's ports, and the test. */
@Component({
  selector: 'app-lock-panel',
  imports: [ValueViewComponent],
  template: `
    <section class="panel-section">
      <h3>Lock type</h3>
      <label class="field">
        <span>Name</span>
        <input type="text" [value]="editor.model().name" (input)="editor.setName($any($event.target).value)" (blur)="editor.settle()" spellcheck="false" />
        <span class="hint">Its file is webapp/locks/{{ editor.model().name }}.rhai.</span>
      </label>
      <label class="stack">
        <span class="hint">What it does (the file's opening comment)</span>
        <textarea rows="2" [value]="editor.model().description" (input)="editor.setDescription($any($event.target).value)" (blur)="editor.settle()"></textarea>
      </label>
      <div class="row">
        <button type="button" class="primary" (click)="editor.save()" title="Check the chain's example, add it to this browser's lock types, and save its lock file">Save as lock type</button>
        <button type="button" (click)="editor.openFile()" title="Open a composite lock file">Open…</button>
        <button type="button" (click)="editor.newChain()">New</button>
      </div>
      <div class="row">
        <button type="button" (click)="copy('type')" title="A journal script using the saved lock type">Copy journal snippet</button>
        <button type="button" (click)="copy('chain')" title="A journal script building the chain lock by lock: no lock file needed">Copy as chain</button>
      </div>
      @if (copied(); as c) { <p class="hint">{{ c }}</p> }
      @if (editor.note(); as note) { <p [class]="note.kind === 'error' ? 'note-error' : note.kind === 'note' ? 'note-warning' : 'hint'">{{ note.text }}</p> }
    </section>

    @if (editor.model().options.length) {
      <section class="panel-section">
        <h3>The chain's options</h3>
        @for (option of editor.model().options; track option.name) {
          <div class="field">
            <input type="text" class="mono" [value]="option.name" aria-label="Option name" (change)="editor.setChainOption(option.name, { name: $any($event.target).value })" />
            <input type="text" class="mono" [value]="show(option.default)" aria-label="Its default"
                   (change)="editor.setChainOption(option.name, { default: read($any($event.target).value, option.kind) })" />
            <span class="hint">{{ declaration(option) }}; used by {{ usedBy(option.name) }}</span>
          </div>
        }
      </section>
    }

    @if (editor.node(); as node) {
      <section class="panel-section">
        <h3>Lock</h3>
        <div class="row">
          <input type="text" class="mono" aria-label="Lock name" [value]="nodeName() ?? node.id" (input)="nodeName.set($any($event.target).value)" (keydown.enter)="rename(node.id)" />
          <button type="button" (click)="rename(node.id)" [disabled]="nodeName() === null || nodeName() === node.id">Rename</button>
          <button type="button" class="danger" (click)="editor.removeNode(node.id)">Remove</button>
        </div>
        @if (renameError(); as error) { <p class="problem">{{ error }}</p> }
        <p class="hint"><b>{{ node.type }}</b>: {{ editor.typeByName().get(node.type)?.description }}</p>
        @if (editor.nodeErrors()[node.id]; as error) { <p class="note-error">{{ error }}</p> }
        @for (option of nodeOptions(); track option.name) {
          <div class="field">
            <span class="mono">{{ option.name }}</span>
            @if (option.promoted) {
              <span class="row"><span class="hint">the chain's <b>{{ option.promoted }}</b></span>
                <button type="button" (click)="editor.demote(node.id, option.name)" title="Give this option a value of its own again">Own value</button></span>
            } @else {
              <span class="row">
                @switch (option.kind) {
                  @case ('bool') {
                    <input type="checkbox" [checked]="option.value === true" (change)="editor.setOption(node.id, option.name, $any($event.target).checked)" />
                  }
                  @case ('one_of') {
                    <select (change)="editor.setOption(node.id, option.name, choice(option, $any($event.target).value))">
                      @if (option.default !== undefined) { <option value="" [selected]="option.value === undefined">default ({{ show(option.default) }})</option> }
                      @for (c of option.choices ?? []; track $index) { <option [value]="$index" [selected]="option.value === c">{{ show(c) }}</option> }
                    </select>
                  }
                  @default {
                    <input [type]="option.kind === 'integer' || option.kind === 'number' ? 'number' : 'text'" class="mono value"
                           [step]="option.kind === 'integer' ? 1 : 'any'" [value]="show(option.value)"
                           [placeholder]="option.default !== undefined ? show(option.default) : 'required'"
                           (change)="editor.setOption(node.id, option.name, read($any($event.target).value, option.kind))" />
                  }
                }
                <button type="button" (click)="editor.promote(node.id, option.name)" title="Make this one of the chain's own options, set where the chain is used">Chain's</button>
              </span>
            }
          </div>
        }
      </section>
    }

    <section class="panel-section">
      <h3>Inputs</h3>
      @for (input of editor.inputs(); track input.port) {
        <div class="port">
          <div class="row">
            <input type="text" class="mono name" [value]="input.name" aria-label="Input name" (change)="editor.setPortName('inputNames', input.port, $any($event.target).value)" />
            <span class="hint">{{ input.port }} · {{ input.type }}</span>
          </div>
          <div class="row">
            <input type="text" class="mono value" [class.invalid]="!!problemOf(input.name, input.type)" placeholder="test value" [value]="test(input.name)"
                   (input)="setTest(input.name, input.type, $any($event.target).value)" (blur)="editor.settle()" />
            <button type="button" (click)="fix(input.port, input.name)" title="Give this input its value in the chain itself, not where it's used">Fix</button>
          </div>
          @if (problemOf(input.name, input.type); as p) { <span class="problem">{{ p }}</span> }
        </div>
      } @empty {
        <p class="hint">No open inputs.</p>
      }
      @for (fixed of fixedPorts(); track fixed.port) {
        <div class="port">
          <div class="row">
            <span class="mono">{{ fixed.port }}</span><span class="hint">fixed · {{ fixed.type }}</span>
          </div>
          <div class="row">
            <input type="text" class="mono value" [value]="fixed.text" (input)="setFixed(fixed.port, fixed.type, $any($event.target).value)" (blur)="editor.settle()" />
            <button type="button" (click)="editor.setFixed(fixed.port, undefined)" title="Leave this input open, to be given where the chain is used">Unfix</button>
          </div>
        </div>
      }
    </section>

    <section class="panel-section">
      <h3>Outputs</h3>
      @for (output of editor.outputs(); track output.port) {
        <div class="port">
          <div class="row">
            <input type="text" class="mono name" [value]="output.name" aria-label="Output name" (change)="editor.setPortName('outputNames', output.port, $any($event.target).value)" />
            <span class="hint">{{ output.port }} · {{ output.type }}</span>
          </div>
          @if (outputValue(output.name); as value) { <app-value-view [value]="value" /> }
        </div>
      } @empty {
        <p class="hint">No open outputs.</p>
      }
      @if (runError(); as error) { <p class="note-error">{{ error }}</p> }
      @if (roundTrip(); as rt) { <p [class]="rt.ok ? 'hint' : 'note-error'">{{ rt.text }}</p> }
    </section>
  `,
  styles: `
    :host { display: block; overflow-y: auto; background: var(--surface); min-height: 0; }
    p { margin: 0; }
    .stack { display: flex; flex-direction: column; gap: 4px; }
    textarea { resize: vertical; }
    .mono { font-family: var(--mono); }
    .row > input { flex: 1; }
    .name { max-width: 12em; }
    .value { min-width: 0; }
    .port { display: flex; flex-direction: column; gap: 4px; padding: 4px 0; border-top: 1px dashed var(--border); }
    .port:first-of-type { border-top: none; }
    button.danger { color: var(--danger); }
  `,
})
export class LockPanelComponent {
  protected readonly editor = inject(LockEditorService);
  protected readonly nodeName = signal<string | null>(null);
  protected readonly renameError = signal<string | null>(null);
  protected readonly copied = signal<string | null>(null);

  constructor() {
    // Another lock picked: what was typed for the last one's name goes.
    effect(() => {
      this.editor.selected();
      this.nodeName.set(null);
      this.renameError.set(null);
    });
  }

  protected readonly nodeOptions = computed(() => {
    const node = this.editor.node();
    if (!node) return [];
    const specs = this.editor.typeByName().get(node.type)?.options ?? [];
    return specs.map((spec) => ({ ...spec, value: node.options[spec.name], promoted: node.promoted[spec.name] ?? null }));
  });

  protected readonly fixedPorts = computed(() => {
    const ports = this.editor.ports();
    return Object.entries(this.editor.model().fixed).map(([port, value]) => {
      const [id, name] = port.split('.');
      const p = ports[id];
      const type = p && !('error' in p) ? (p.inputs.find(([n]) => n === name)?.[1] ?? '?') : '?';
      return { port, type, text: typed(value) };
    });
  });

  private readonly run = computed(() => {
    const result = this.editor.result();
    return result && !failed(result) ? result : null;
  });

  protected readonly runError = computed(() => {
    const result = this.editor.result();
    return result && failed(result) ? result.error : null;
  });

  protected readonly roundTrip = computed(() => {
    const run = this.run();
    if (!run) return null;
    if (run.unlockError) return { ok: false, text: `Unlocking fails: ${run.unlockError}` };
    return run.roundTrip ? { ok: true, text: 'Unlocking gives the test inputs back ✓' } : { ok: false, text: "Unlocking doesn't give the test inputs back ✗" };
  });

  protected outputValue(name: string) {
    return this.run()?.outputs.find(([n]) => n === name)?.[1] ?? null;
  }

  protected test(name: string): string {
    return typed(this.editor.model().test[name]);
  }

  protected problemOf(name: string, type: string): string | null {
    const value = this.editor.model().test[name];
    if (!value) return null;
    return parseValue(typed(value), type).problem;
  }

  protected setTest(name: string, type: string, text: string) {
    this.editor.setTest(name, text === '' && symbolBase(type) === null ? undefined : parseValue(text, type).value);
  }

  protected setFixed(port: string, type: string, text: string) {
    this.editor.setFixed(port, parseValue(text, type).value);
  }

  /** Fix an input to its test value (or nothing yet). */
  protected fix(port: string, name: string) {
    this.editor.setFixed(port, this.editor.model().test[name] ?? { text: '' });
    this.editor.settle();
  }

  protected rename(id: string) {
    const name = this.nodeName();
    if (name === null) return;
    const error = this.editor.renameNode(id, name.trim());
    this.renameError.set(error);
    if (!error) this.nodeName.set(null);
  }

  protected show(value: unknown): string {
    return value === undefined ? '' : typeof value === 'string' ? value : JSON.stringify(value);
  }

  /** What's typed for an option of `kind`. */
  protected read(text: string, kind: LockOption['kind']): unknown {
    if (text === '') return undefined;
    if (kind === 'integer') return Number.isInteger(Number(text)) ? Number(text) : text;
    if (kind === 'number') return Number.isFinite(Number(text)) ? Number(text) : text;
    if (kind === 'string') return text;
    try {
      return JSON.parse(text);
    } catch {
      return text;
    }
  }

  protected choice(option: LockOption, index: string): unknown {
    return index === '' ? undefined : option.choices?.[Number(index)];
  }

  protected declaration(option: Parameters<typeof optionDeclaration>[0]): string {
    return optionDeclaration(option);
  }

  protected usedBy(name: string): string {
    return this.editor.model().nodes.flatMap((n) => Object.entries(n.promoted).filter(([, v]) => v === name).map(([k]) => `${n.id}.${k}`)).join(', ') || 'nothing';
  }

  protected async copy(kind: 'type' | 'chain') {
    const text = this.editor.snippet(kind);
    try {
      await navigator.clipboard.writeText(text);
      this.copied.set('Copied to the clipboard.');
    } catch {
      this.copied.set(text);
    }
  }
}
