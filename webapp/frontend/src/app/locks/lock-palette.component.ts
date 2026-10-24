import { Component, computed, inject, signal } from '@angular/core';
import type { LockType, RendererType } from '../scribe';
import { LockEditorService } from './lock-editor.service';

/** The left: the chains, to edit; every lock type and renderer type, to drag into the chain (or click to add). */
@Component({
  selector: 'app-lock-palette',
  template: `
    <div class="head">
      <input type="text" placeholder="Find a lock type or illustration" aria-label="Find a lock type" [value]="filter()" (input)="filter.set($any($event.target).value)" />
    </div>
    @if (editor.typesError(); as error) { <p class="error">{{ error }}</p> }
    <h3 class="renderers-head">Chains <button type="button" class="open" (click)="editor.newChain()" title="Start a new chain">New</button></h3>
    <p class="note">Click one to edit it: journals using chain("name") follow along.</p>
    <ul>
      @for (c of chains(); track c.name) {
        <li class="type chain" [class.current]="c.name === editor.model().name" [class.broken]="!!c.error" (click)="editor.openChain(c.name)" [title]="c.error ?? c.description ?? ''">
          <span class="name">{{ c.name }}</span>
          @if (c.workspace) { <span class="tag" title="Only in this browser, or edited here">browser</span> }
          @if (c.workspace) {
            <button type="button" class="remove" (click)="$event.stopPropagation(); editor.removeWorkspaceChain(c.name)" title="Remove this browser's copy of the chain">×</button>
          }
          <span class="about">{{ c.error ?? c.description }}</span>
        </li>
      }
    </ul>
    @for (group of groups(); track group.title) {
      @if (group.types.length) {
        <h3>{{ group.title }}</h3>
        <ul>
          @for (type of group.types; track type.name) {
            <li class="type" draggable="true" [class.broken]="!!type.error" (dragstart)="drag($event, type)"
                (click)="!type.error && add(type)" [title]="type.error ?? type.description ?? ''">
              <span class="name">{{ type.name }}</span>
              @if (type.composite) { <span class="tag" title="A composite lock file: other lock types wired together">composite</span> }
              @if (type.workspace) {
                <button type="button" class="remove" (click)="$event.stopPropagation(); editor.removeWorkspaceLock(type.name)" title="Remove this lock type from this browser">×</button>
              }
              <span class="about">{{ type.error ?? type.description }}</span>
            </li>
          }
        </ul>
      }
    }
    <h3 class="renderers-head">Illustrations <button type="button" class="open" (click)="editor.newRenderer()" title="Write a new illustration file">New</button></h3>
    <p class="note">Draw a lock's values as canvases: wire an output to one. Journals attach them with lock.attach.</p>
    <ul>
      @for (type of renderers(); track type.name) {
        <li class="type renderer" draggable="true" [class.broken]="!!type.error" (dragstart)="dragRenderer($event, type)"
            (click)="!type.error && addRenderer(type)" [title]="type.error ?? type.description ?? ''">
          <span class="name">{{ type.name }}</span>
          @if (type.workspace) { <span class="tag" title="Only in this browser">browser</span> }
          <button type="button" class="open" (click)="$event.stopPropagation(); editor.editRenderer(type.name)" title="Edit this illustration's file">Edit</button>
          @if (type.workspace) {
            <button type="button" class="remove" (click)="$event.stopPropagation(); editor.removeWorkspaceRenderer(type.name)" title="Remove this illustration from this browser">×</button>
          }
          <span class="about">{{ type.error ?? type.description }}</span>
        </li>
      }
    </ul>
  `,
  styles: `
    :host { display: flex; flex-direction: column; overflow-y: auto; background: var(--rail-bg); color: var(--rail-fg); min-height: 0; font-size: 12px; }
    .head { padding: 8px; display: flex; }
    .head input { flex: 1; }
    .error { margin: 0 8px; color: #f2a59c; }
    h3 { margin: 10px 10px 4px; font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--rail-selected); }
    ul { list-style: none; margin: 0; padding: 0 6px; display: flex; flex-direction: column; gap: 2px; }
    .type { display: grid; grid-template-columns: 1fr auto auto auto; gap: 2px 6px; align-items: center; padding: 6px 8px; border-radius: 6px; cursor: grab; border: 1px solid transparent; }
    .type:hover { background: var(--rail-hover); border-color: var(--rail-selected-bg); }
    .type.broken { cursor: default; opacity: 0.7; }
    .name { font: 600 12px var(--mono); overflow: hidden; text-overflow: ellipsis; }
    .tag { font-size: 10px; padding: 0 5px; border-radius: 8px; background: var(--rail-selected-bg); border: 1px solid var(--rail-selected); }
    .about { grid-column: 1 / -1; color: #a39b90; font-size: 11px; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
    .type.broken .about { color: #f2a59c; }
    .open, .remove { padding: 0 6px; font-size: 11px; background: transparent; color: var(--rail-fg); border-color: var(--rail-selected); }
    .renderers-head { display: flex; align-items: center; gap: 6px; margin-top: 14px; }
    .note { margin: 0 10px 4px; color: #a39b90; font-size: 11px; }
    .type.renderer { border-left: 2px dashed #8fc1e3; }
    .type.chain { cursor: pointer; }
    .type.chain.current { border-color: var(--rail-selected); }
  `,
})
export class LockPaletteComponent {
  protected readonly editor = inject(LockEditorService);
  protected readonly filter = signal('');

  protected readonly groups = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    const types = this.editor.types().filter((t) => !filter || t.name.includes(filter) || (t.description ?? '').toLowerCase().includes(filter));
    return [
      { title: 'Lock types in this browser', types: types.filter((t) => t.workspace) },
      { title: 'Lock types', types: types.filter((t) => !t.workspace) },
    ];
  });

  protected readonly chains = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    return this.editor.chainTypes().filter((c) => !filter || c.name.includes(filter) || (c.description ?? '').toLowerCase().includes(filter));
  });

  protected readonly renderers = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    return this.editor.rendererTypes().filter((t) => !filter || t.name.includes(filter) || (t.description ?? '').toLowerCase().includes(filter));
  });

  protected dragRenderer(event: DragEvent, type: RendererType) {
    if (type.error) return event.preventDefault();
    event.dataTransfer?.setData('application/x-renderer-type', type.name);
    event.dataTransfer!.effectAllowed = 'copy';
  }

  /** Clicked: attach it below the locks. */
  protected addRenderer(type: RendererType) {
    const model = this.editor.model();
    const placed = [...model.nodes, ...model.renderers];
    const x = placed.length ? Math.max(...placed.map((n) => n.x)) + 360 : 120;
    const y = placed.length ? Math.max(...placed.map((n) => n.y)) + 160 : 60;
    this.editor.addRenderer(type.name, x, y);
  }

  protected drag(event: DragEvent, type: LockType) {
    if (type.error) return event.preventDefault();
    event.dataTransfer?.setData('application/x-lock-type', type.name);
    event.dataTransfer!.effectAllowed = 'copy';
  }

  /** Clicked: add it beside the locks already there. */
  protected add(type: LockType) {
    const nodes = this.editor.model().nodes;
    const x = nodes.length ? Math.max(...nodes.map((n) => n.x)) + 360 : 120;
    const y = nodes.length ? nodes[nodes.length - 1].y : 60;
    this.editor.addNode(type.name, x, y);
  }
}
