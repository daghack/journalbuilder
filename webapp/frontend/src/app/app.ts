import { Component, inject } from '@angular/core';
import { FontsWorkspaceComponent } from './fonts/fonts-workspace.component';
import { JournalWorkspaceComponent } from './journal/journal-workspace.component';
import { LocksWorkspaceComponent } from './locks/locks-workspace.component';
import { JournalService } from './services/journal.service';
import { ModeTabsComponent } from './shell/mode-tabs.component';
import { ShellService } from './shell/shell.service';
import { loadExample } from './toolbar/top-bar.component';

/** The script a first visit starts with. */
const FIRST_EXAMPLE = 'showcase';

/**
 * The editors, one showing at a time: journals, fonts and lock chains.
 * Each stays as it was while another shows; the font and lock editors load
 * when first opened.
 */
@Component({
  selector: 'app-root',
  imports: [ModeTabsComponent, JournalWorkspaceComponent, FontsWorkspaceComponent, LocksWorkspaceComponent],
  template: `
    <app-mode-tabs />
    <app-journal-workspace class="workspace" [hidden]="shell.mode() !== 'journal'" />
    @defer (when shell.mode() === 'fonts') {
      <app-fonts-workspace class="workspace" [hidden]="shell.mode() !== 'fonts'" />
    }
    @defer (when shell.mode() === 'locks') {
      <app-locks-workspace class="workspace" [hidden]="shell.mode() !== 'locks'" />
    }
  `,
  styles: `
    :host { display: flex; flex-direction: column; height: 100dvh; }
    .workspace { flex: 1; min-height: 0; }
    @media (max-width: 760px) {
      :host { height: auto; min-height: 100dvh; }
    }
  `,
})
export class App {
  private readonly journal = inject(JournalService);
  protected readonly shell = inject(ShellService);

  constructor() {
    void this.journal.restore(async () => ({
      name: `${FIRST_EXAMPLE}.rhai`,
      source: (await loadExample(FIRST_EXAMPLE)) ?? '// Write a journal script here.\n',
    }));
  }
}
