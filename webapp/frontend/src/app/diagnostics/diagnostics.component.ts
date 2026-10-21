import { Component, computed, inject, output, signal } from '@angular/core';
import { JournalService } from '../services/journal.service';
import { RenderService } from '../services/render.service';

/** Under the editor: the last run's error, warnings, missing glyphs and what the script printed. */
@Component({
  selector: 'app-diagnostics',
  template: `
    <button type="button" class="summary" (click)="open.set(!open())" [attr.aria-expanded]="open()">
      @if (render.loadError(); as error) {
        <span class="error">The renderer didn't load: {{ error }}</span>
      } @else if (journal.error()) {
        <span class="error">Error{{ journal.error()!.line ? ' on line ' + journal.error()!.line : '' }}</span>
      } @else if (evaluation(); as e) {
        <span class="ok">{{ e.pages.length }} page{{ e.pages.length === 1 ? '' : 's' }} in {{ e.millis.toFixed(0) }} ms</span>
      } @else {
        <span class="muted">Loading the renderer…</span>
      }
      @if (count(); as n) { <span class="count">{{ n }} message{{ n === 1 ? '' : 's' }}</span> }
      <span class="chevron">{{ open() ? '▾' : '▸' }}</span>
    </button>
    @if (open()) {
      <div class="messages">
        @if (journal.error(); as error) {
          <pre class="error" (click)="line(error.line)">{{ error.message }}</pre>
        }
        @if (workspaceNote(); as note) { <pre class="print">{{ note }}</pre> }
        @if (evaluation()?.missingGlyphs; as missing) { <pre class="warning">{{ missing }}</pre> }
        @for (warning of evaluation()?.warnings ?? []; track $index) {
          <pre class="warning" (click)="line(lineOf(warning))">{{ warning }}</pre>
        }
        @for (text of evaluation()?.prints ?? []; track $index) { <pre class="print">{{ text }}</pre> }
        @if (!count() && !workspaceNote()) { <p class="muted">Nothing to report.</p> }
      </div>
    }
  `,
  styles: `
    :host { display: block; border-top: 1px solid var(--border); background: var(--surface); font-size: 12px; }
    .summary { display: flex; width: 100%; gap: 10px; align-items: center; border: none; border-radius: 0; background: transparent; padding: 6px 10px; text-align: left; }
    .count { color: var(--muted); }
    .chevron { margin-left: auto; color: var(--muted); }
    .messages { max-height: 30vh; overflow: auto; padding: 0 10px 8px; }
    pre { margin: 4px 0; white-space: pre-wrap; font-family: var(--mono); font-size: 12px; cursor: default; }
    pre[class] { padding: 4px 6px; border-radius: 4px; }
    .error { color: var(--danger); }
    pre.error { background: var(--danger-bg); cursor: pointer; }
    .warning { color: var(--warning); }
    pre.warning { background: var(--warning-bg); cursor: pointer; }
    .print { color: var(--fg); background: var(--surface-2); }
    .ok { color: var(--ok); }
    .muted { color: var(--muted); }
  `,
})
export class DiagnosticsComponent {
  protected readonly journal = inject(JournalService);
  protected readonly render = inject(RenderService);
  protected readonly evaluation = this.journal.evaluation;
  protected readonly open = signal(false);
  /** Asked to show a line of the script. */
  readonly goTo = output<number>();

  /** What the workspace changes from the repository's files, which only this browser has. */
  protected readonly workspaceNote = computed(() => {
    const workspace = this.evaluation()?.workspace;
    const parts = [
      workspace?.editedFonts ? 'the fonts as edited in the font editor' : '',
      workspace?.locks.length ? `the workspace's lock files (${workspace.locks.join(', ')})` : '',
      workspace?.renderers?.length ? `its renderers (${workspace.renderers.join(', ')})` : '',
      workspace?.chains?.length ? `its chains (${workspace.chains.join(', ')})` : '',
    ].filter(Boolean);
    return parts.length ? `Ran with ${parts.join(' and ')}, which only this browser has.` : null;
  });

  protected readonly count = computed(() => {
    const e = this.evaluation();
    return (this.journal.error() ? 1 : 0) + (e?.missingGlyphs ? 1 : 0) + (e?.warnings.length ?? 0) + (e?.prints.length ?? 0);
  });

  protected lineOf(message: string): number | null {
    const match = message.match(/\(line (\d+)/);
    return match ? Number(match[1]) : null;
  }

  protected line(line: number | null) {
    if (line) this.goTo.emit(line);
  }
}
