import { Component, computed, inject, signal } from '@angular/core';
import type { LockType } from '../scribe';
import { LockEditorService } from './lock-editor.service';

/** The left: every lock type, to drag into the chain (or click to add). */
@Component({
  selector: 'app-lock-palette',
  template: `
    <div class="head">
      <input type="text" placeholder="Find a lock type" aria-label="Find a lock type" [value]="filter()" (input)="filter.set($any($event.target).value)" />
    </div>
    @if (editor.typesError(); as error) { <p class="error">{{ error }}</p> }
    @for (group of groups(); track group.title) {
      @if (group.types.length) {
        <h3>{{ group.title }}</h3>
        <ul>
          @for (type of group.types; track type.name) {
            <li class="type" draggable="true" [class.broken]="!!type.error" (dragstart)="drag($event, type)"
                (click)="!type.error && add(type)" [title]="type.error ?? type.description ?? ''">
              <span class="name">{{ type.name }}</span>
              @if (type.composite) { <span class="tag" title="A composite lock file: other lock types wired together">chain</span> }
              @if (type.composite) {
                <button type="button" class="open" (click)="$event.stopPropagation(); editor.openType(type.name)" title="Open this chain to edit it">Open</button>
              }
              @if (type.workspace) {
                <button type="button" class="remove" (click)="$event.stopPropagation(); editor.removeWorkspaceLock(type.name)" title="Remove this lock type from this browser">×</button>
              }
              <span class="about">{{ type.error ?? type.description }}</span>
            </li>
          }
        </ul>
      }
    }
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
  `,
})
export class LockPaletteComponent {
  protected readonly editor = inject(LockEditorService);
  protected readonly filter = signal('');

  protected readonly groups = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    const types = this.editor.types().filter((t) => !filter || t.name.includes(filter) || (t.description ?? '').toLowerCase().includes(filter));
    return [
      { title: 'In this browser', types: types.filter((t) => t.workspace) },
      { title: 'Lock files', types: types.filter((t) => !t.workspace && !t.composite) },
      { title: 'Chains', types: types.filter((t) => !t.workspace && t.composite) },
    ];
  });

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
