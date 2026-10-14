import { Injectable, effect, signal } from '@angular/core';

/** The editors the app has: journals, fonts and lock chains. */
export type Mode = 'journal' | 'fonts' | 'locks';

const MODES: Mode[] = ['journal', 'fonts', 'locks'];

function fromHash(): Mode {
  const mode = location.hash.slice(1) as Mode;
  return MODES.includes(mode) ? mode : 'journal';
}

/** Which editor shows, kept in the address's hash (#fonts, #locks) so a reload keeps it. */
@Injectable({ providedIn: 'root' })
export class ShellService {
  readonly mode = signal<Mode>(fromHash());

  constructor() {
    addEventListener('hashchange', () => this.mode.set(fromHash()));
    effect(() => {
      const mode = this.mode();
      const hash = mode === 'journal' ? '' : `#${mode}`;
      if (location.hash !== hash) history.replaceState(null, '', hash || location.pathname + location.search);
    });
  }
}
