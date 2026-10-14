import { Component, computed, inject } from '@angular/core';
import { WorkspaceService } from '../services/workspace.service';
import { ShellService, type Mode } from './shell.service';

/** Across the top: which editor shows, and what the workspace changes from the repository's files. */
@Component({
  selector: 'app-mode-tabs',
  template: `
    <nav class="tabs" aria-label="Editors">
      @for (tab of tabs; track tab.mode) {
        <button type="button" [class.on]="shell.mode() === tab.mode" [attr.aria-current]="shell.mode() === tab.mode ? 'page' : null"
                (click)="shell.mode.set(tab.mode)" [title]="tab.title">{{ tab.label }}</button>
      }
    </nav>
    <span class="spacer"></span>
    @if (workspace.fonts() !== null) {
      <span class="chip" title="Journals use the fonts as edited in the font editor, not config/fonts.json">edited fonts</span>
    }
    @if (lockNames().length) {
      <span class="chip" [title]="'Lock files only in this browser: ' + lockNames().join(', ')">
        {{ lockNames().length }} workspace lock{{ lockNames().length === 1 ? '' : 's' }}
      </span>
    }
  `,
  styles: `
    :host { display: flex; align-items: center; gap: 8px; padding: 4px 10px; background: var(--rail-bg); color: var(--rail-fg); font-size: 12px; }
    .tabs { display: inline-flex; gap: 2px; }
    .tabs button { background: transparent; border: 1px solid transparent; color: var(--rail-fg); padding: 3px 12px; }
    .tabs button:hover { background: var(--rail-hover); border-color: transparent; }
    .tabs button.on { background: var(--rail-selected-bg); border-color: var(--rail-selected); font-weight: 600; }
    .spacer { flex: 1; }
    .chip { padding: 2px 8px; border-radius: 10px; background: var(--rail-selected-bg); border: 1px solid var(--rail-selected); }
  `,
})
export class ModeTabsComponent {
  protected readonly shell = inject(ShellService);
  protected readonly workspace = inject(WorkspaceService);
  protected readonly lockNames = computed(() => Object.keys(this.workspace.locks()));
  protected readonly tabs: { mode: Mode; label: string; title: string }[] = [
    { mode: 'journal', label: 'Journal', title: 'Write journal scripts and see their pages' },
    { mode: 'fonts', label: 'Fonts', title: 'Draw glyphs and set font options' },
    { mode: 'locks', label: 'Locks', title: 'Wire lock types together into new ones' },
  ];
}
