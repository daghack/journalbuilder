import { Component, computed, effect, inject, signal } from '@angular/core';
import type { LockOption } from '../scribe';
import { failed } from '../services/tools.service';
import { symbolBase, testText, textSymbols, type TestValue } from './chain-model';
import { optionDeclaration } from './chain-source';
import { LockEditorService } from './lock-editor.service';
import { RenderPagesComponent } from './render-pages.component';
import { ValueViewComponent } from './value-view.component';

const typed = testText;

/**
 * What's typed as a value for a port of `type`, and what's wrong with it:
 * text for symbols and strings; otherwise JSON, or else rhai that makes
 * the value (checked when the chain runs).
 */
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
    return text.trim() ? { value: { rhai: text }, problem: null } : { value: { text }, problem: 'give JSON, or rhai that makes it' };
  }
}

/** The right: the chain being made, the picked lock's options, the chain's ports, and the test. */
@Component({
  selector: 'app-lock-panel',
  imports: [ValueViewComponent, RenderPagesComponent],
  template: `
    <section class="panel-section">
      <h3>Chain</h3>
      <label class="field">
        <span>Name</span>
        <input type="text" [value]="editor.model().name" (input)="editor.setName($any($event.target).value)" (blur)="editor.settle()" spellcheck="false" />
        <span class="hint">Its file is webapp/library/chains/{{ editor.model().name }}.rhai.</span>
      </label>
      <label class="stack">
        <span class="hint">What it does (the file's opening comment)</span>
        <textarea rows="2" [value]="editor.model().description" (input)="editor.setDescription($any($event.target).value)" (blur)="editor.settle()"></textarea>
      </label>
      <div class="row">
        <button type="button" class="primary" (click)="editor.save()" title="Save the chain file, to put in webapp/library/chains/">Save file</button>
        <button type="button" (click)="editor.openFile()" title="Open a chain file">Open…</button>
        <button type="button" (click)="editor.newChain()">New</button>
        <button type="button" (click)="revert()" [disabled]="!editor.canRevert()"
                [title]="editor.builtin() ? 'Throw away the edits made in this browser: put the chain back as it is in webapp/library/chains/' : 'Throw away the edits since the chain was opened'">Revert</button>
      </div>
      <div class="row">
        <button type="button" (click)="copy('chain')" title="A journal script using the chain: chain(name)">Copy journal snippet</button>
        <button type="button" (click)="copy('script')" title="A journal script building the chain lock by lock with new_lock: no chain file needed">Copy as script</button>
      </div>
      @if (copied(); as c) { <p class="hint">{{ c }}</p> }
      <p class="hint">As you edit it, the chain is in this browser: journals using chain("{{ editor.model().name }}") follow along.</p>
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

    @if (editor.renderer(); as renderer) {
      <section class="panel-section">
        <h3>Illustration</h3>
        <div class="row">
          <input type="text" class="mono" aria-label="Illustration name" [value]="nodeName() ?? renderer.id" (input)="nodeName.set($any($event.target).value)" (keydown.enter)="rename(renderer.id)" />
          <button type="button" (click)="rename(renderer.id)" [disabled]="nodeName() === null || nodeName() === renderer.id">Rename</button>
          <button type="button" class="danger" (click)="editor.removeNode(renderer.id)">Remove</button>
        </div>
        @if (renameError(); as error) { <p class="problem">{{ error }}</p> }
        <p class="hint"><b>{{ renderer.type }}</b>: {{ editor.rendererByName().get(renderer.type)?.description }}</p>
        @if (editor.rendererErrors()[renderer.id]; as error) { <p class="note-error">{{ error }}</p> }
        @for (option of rendererOptions(); track option.name) {
          <div class="field">
            <span class="mono">{{ option.name }}</span>
            @if (option.promoted) {
              <span class="row"><span class="hint">the chain's <b>{{ option.promoted }}</b></span>
                <button type="button" (click)="editor.demote(renderer.id, option.name)" title="Give this option a value of its own again">Own value</button></span>
            } @else {
              <span class="row">
                @if (option.kind === 'bool') {
                  <input type="checkbox" [checked]="option.value === true" (change)="editor.setRendererOption(renderer.id, option.name, $any($event.target).checked)" />
                } @else {
                  <input [type]="option.kind === 'number' ? 'number' : 'text'" class="mono value" step="any" [value]="show(option.value)" [placeholder]="show(option.default)"
                         (change)="editor.setRendererOption(renderer.id, option.name, readRendererOption($any($event.target).value, option.kind))" />
                }
                <button type="button" (click)="editor.promote(renderer.id, option.name)" title="Make this one of the chain's own options, set where the chain is used">Chain's</button>
              </span>
            }
          </div>
        }
        <div class="row">
          <button type="button" (click)="editor.editRenderer(renderer.type)" title="Edit this illustration's file">Edit {{ renderer.type }}.rhai</button>
        </div>
        <h4>What it draws of the run</h4>
        @if (editor.previews()[renderer.id]; as preview) {
          <app-render-pages [preview]="preview" />
        } @else {
          <p class="hint">Wire each of its inputs from a lock's output, and give the chain a test that runs.</p>
        }
      </section>
    }

    <section class="panel-section">
      <h3>Inputs</h3>
      @for (input of editor.inputs(); track input.port) {
        <div class="port">
          <div class="row">
            <span class="mono">{{ input.port }}</span><span class="hint">{{ input.type }}</span>
          </div>
          <div class="row">
            <input type="text" class="mono value" [class.invalid]="!!problemOf(input.port, input.type)" [placeholder]="placeholder(input.type)" [value]="test(input.port)"
                   [attr.aria-label]="input.port + ' test value'" (input)="setTest(input.port, input.type, $any($event.target).value)" (blur)="editor.settle()" />
            <button type="button" (click)="fix(input.port)" title="Give this input its value in the chain itself, not where it's used">Fix</button>
          </div>
          @if (problemOf(input.port, input.type); as p) { <span class="problem">{{ p }}</span> }
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
            <span class="mono">{{ output.port }}</span><span class="hint">{{ output.type }}</span>
          </div>
          @if (outputValue(output.port); as value) { <app-value-view [value]="value" /> }
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

  /** What to type for a port of `type`: a value kind's example, if it has one. */
  protected placeholder(type: string): string {
    if (symbolBase(type) !== null) return 'test value';
    if (type === 'String') return 'text';
    const example = this.editor.kinds().get(type);
    return example ? `e.g. ${example}` : 'JSON, or rhai that makes it';
  }

  /** The picked renderer's options: its defaults, and what it's given. */
  protected readonly rendererOptions = computed(() => {
    const renderer = this.editor.renderer();
    if (!renderer) return [];
    const defaults = this.editor.rendererByName().get(renderer.type)?.options ?? {};
    return Object.entries(defaults).map(([name, value]) => ({
      name, default: value, value: renderer.options[name], promoted: renderer.promoted[name] ?? null,
      kind: typeof value === 'number' ? 'number' : typeof value === 'boolean' ? 'bool' : 'text',
    }));
  });

  /** A renderer option as typed: a number for a number's, else text; empty for its default. */
  protected readRendererOption(text: string, kind: string): unknown {
    if (text === '') return undefined;
    return kind === 'number' ? Number(text) : text;
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
  protected fix(port: string) {
    this.editor.setFixed(port, this.editor.model().test[port] ?? { text: '' });
    this.editor.settle();
  }

  protected revert() {
    const name = this.editor.model().name;
    if (confirm(this.editor.builtin() ? `Throw away this browser's edits to ${name} and use webapp/library/chains/${name}.rhai again?` : `Throw away the edits to ${name} since it was opened?`)) void this.editor.revert();
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
    const model = this.editor.model();
    return [...model.nodes, ...model.renderers].flatMap((n) => Object.entries(n.promoted).filter(([, v]) => v === name).map(([k]) => `${n.id}.${k}`)).join(', ') || 'nothing';
  }

  protected async copy(kind: 'chain' | 'script') {
    const text = this.editor.snippet(kind);
    try {
      await navigator.clipboard.writeText(text);
      this.copied.set('Copied to the clipboard.');
    } catch {
      this.copied.set(text);
    }
  }
}
