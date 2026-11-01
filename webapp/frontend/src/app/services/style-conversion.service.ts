import { Injectable, effect, inject, signal, untracked } from '@angular/core';
import { OLD_STYLE, pathOf } from '../journal/journal-files';
import { readStyle, styleFile } from '../journal/style-builder';
import { JournalStore } from './journal-store.service';
import { ToolsService, failed } from './tools.service';

/** A style script that wasn't converted: why not, and the script. */
export interface OldStyle {
  path: string;
  name: string;
  source: string;
  /** The nearest a file can come, if the script runs. */
  file?: string;
  /** What that file wouldn't do that the script did, or why the script can't run. */
  problems: string[];
}

/**
 * Styles were scripts, styles/NAME.rhai; now they're JSON. A journal loaded
 * with scripts has them converted: a script a file can say all of is replaced
 * by the file, and the others are left as they are, and said.
 */
@Injectable({ providedIn: 'root' })
export class StyleConversionService {
  private readonly store = inject(JournalStore);
  private readonly tools = inject(ToolsService);

  /** The scripts left as they are, and why. */
  readonly left = signal<OldStyle[]>([]);
  /** The names of the styles last converted. */
  readonly converted = signal<string[]>([]);

  constructor() {
    effect(() => {
      const loads = this.store.loads();
      if (this.store.restored()) untracked(() => void this.convert(loads));
    });
  }

  private async convert(loads: number) {
    const left: OldStyle[] = [];
    const converted: string[] = [];
    for (const [path, source] of Object.entries(this.store.files())) {
      const name = OLD_STYLE.exec(path)?.[1];
      // A file of its name is the style already.
      if (!name || pathOf('styles', name) in this.store.files()) continue;
      const answer = await this.tools.convertStyle(name, source).catch((err: Error) => ({ error: err.message }));
      if (this.store.loads() !== loads) return;
      if (failed(answer)) {
        left.push({ path, name, source, problems: [answer.error] });
      } else if (answer.problems.length) {
        left.push({ path, name, source, file: answer.file, problems: answer.problems });
      } else {
        this.replace(path, name, answer.file);
        converted.push(name);
      }
    }
    this.left.set(left);
    if (converted.length) this.converted.set(converted);
  }

  private replace(path: string, name: string, file: string) {
    this.store.setFile(pathOf('styles', name), styleFile(readStyle(file)));
    this.store.removeFile(path);
  }

  /** Replace `style`'s script with the nearest a file can come. */
  convertAnyway(style: OldStyle) {
    if (style.file) this.replace(style.path, style.name, style.file);
    this.left.update((left) => left.filter((other) => other !== style));
  }

  /** Remove `style`'s script. */
  remove(style: OldStyle) {
    this.store.removeFile(style.path);
    this.left.update((left) => left.filter((other) => other !== style));
  }
}
