// Content files brought in from the user's disk: imported, a copy kept in
// the journal, or linked, read from disk before each run (see the
// repository's Resources::with_linked_content, which journalbuild reads
// journal.json's linked_content with).

import { CONTENT_TYPES, VALID_NAME, journalName } from './journal-files';
import { readSymbols, symbolHelp, symbolsIn, textOf, writeSymbols, type SymbolsFile } from './symbols';

/** The extensions a content file on disk may have, and the type each is kept as. */
const CONTENT_EXTS: Record<string, string> = {
  ...Object.fromEntries(CONTENT_TYPES.map((t) => [t.ext, t.ext])),
  markdown: 'md',
};

/** What a file input accepts as content. */
export const CONTENT_ACCEPT = Object.keys(CONTENT_EXTS).map((ext) => `.${ext}`).join(',');

/** The content type a file called `fileName` is (its extension's: .markdown is md), or null if it isn't content. */
export function contentExt(fileName: string): string | null {
  const match = /\.([A-Za-z]+)$/.exec(fileName);
  return match ? CONTENT_EXTS[match[1].toLowerCase()] ?? null : null;
}

/** An imported file's name in the journal, to start with: its stem, made a name. */
export function importedName(fileName: string): string {
  return journalName(fileName.replace(/\.[^.]*$/, ''), 'content');
}

/** Whether a link's path names one content file (by its extension) rather than a folder of them. */
export function linksAFile(link: string): boolean {
  const match = /\.([a-z]+)$/.exec(link);
  return !!match && CONTENT_TYPES.some((t) => t.ext === match[1]);
}

/**
 * The content a linked file on disk gives, by its path in the journal
 * (content/NAME.ext), as journalbuild reads it: of a content extension
 * (.markdown isn't one), named letters, digits and _. Null for any other
 * file, which a linked folder leaves out.
 */
export function linkedPath(fileName: string): string | null {
  const match = /^(.+)\.([a-z]+)$/.exec(fileName);
  if (!match || !VALID_NAME.test(match[1]) || !CONTENT_TYPES.some((t) => t.ext === match[2])) return null;
  return `content/${match[1]}.${match[2]}`;
}

/** Every link's files as one set, each link's in front of those after it. */
export function linkedFiles(links: Record<string, string>[]): Record<string, string> {
  return Object.assign({}, ...[...links].reverse());
}

/** The journal's files with its linked content in front of its own. */
export function withLinked(own: Record<string, string>, linked: Record<string, string>): Record<string, string> {
  return { ...own, ...linked };
}

/** How an imported file is kept: as content of type `ext`; symbols in `base`, strict or made to fit. */
export interface ImportAs {
  ext: string;
  base: number;
  strict: boolean;
}

/** What an imported file is kept as: its text, and for symbols what they read as; or why it can't be. */
export type Imported = { text: string; reads?: string; count?: number; note?: string } | { error: string };

/**
 * A file's text (from a file ending `fromExt`) as content of the type
 * `as` says. Text, Markdown and graphs keep it as it is. Symbols keep a
 * symbols file (.json) that's one already as it is; any other text is
 * written in `as.base`, its last newline left off: strict, it must be
 * written in the base already; otherwise it's made to fit as it's read,
 * in capitals, what the base can't write dropped.
 */
export function importedText(text: string, fromExt: string, as: ImportAs): Imported {
  if (as.ext !== 'json') return { text };
  if (fromExt === 'json') {
    try {
      const file = readSymbols(text);
      const symbols = symbolsIn(file);
      return { text, reads: textOf(file.base, symbols), count: symbols.length, note: `A symbols file already, in base ${file.base}: kept as it is.` };
    } catch {
      // Not a symbols file: its text is written in the base, below.
    }
  }
  const body = text.replace(/\r?\n$/, '');
  const file: SymbolsFile = as.strict ? { base: as.base, text: body, strict: true } : { base: as.base, text: body };
  let symbols: number[];
  try {
    symbols = symbolsIn(file);
  } catch (err) {
    return { error: `Strict: ${(err as Error).message}. Import it permissively to drop what doesn't fit.` };
  }
  if (!symbols.length) return { error: `Nothing in it is a base-${as.base} symbol (${symbolHelp(as.base)}).` };
  return { text: writeSymbols(file), reads: textOf(as.base, symbols), count: symbols.length };
}
