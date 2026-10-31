import { signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { unzipSync, strFromU8 } from 'fflate';
import { describe, expect, it, vi } from 'vitest';
import { unpackZip, type JournalData } from '../journal/journal-files';
import { FileService } from './file.service';
import { JournalIoService } from './journal-io.service';
import { JournalStore } from './journal-store.service';
import { LibraryService } from './library.service';
import { ToolsService } from './tools.service';

const journal: JournalData = {
  name: 'partly_broken',
  manifest: { version: 1, title: 'Partly Broken', entries: ['first', 'throws', 'crashes', 'last'] },
  files: {
    'entries/first.rhai': 'first',
    'entries/throws.rhai': 'throws',
    'entries/crashes.rhai': 'crashes',
    'entries/last.rhai': 'last',
    'locks/own.rhai': '// its own lock',
  },
};

const library: Record<string, string> = {
  'locks/first.rhai': '// first',
  'locks/before_throwing.rhai': '// read before it threw',
  'locks/last.rhai': '// last',
  'locks/unused.rhai': '// no one uses this',
};

/** The journal exported, each script answering as `used` says: what's saved, and what the export returned. */
async function exported(used: (source: string) => Promise<{ paths: string[]; error: { message: string; line: number | null; column: number | null } | null }>) {
  let saved: Blob | null = null;
  const markSaved = vi.fn();
  TestBed.configureTestingModule({
    providers: [
      { provide: JournalStore, useValue: { journal: () => journal, markSaved } },
      { provide: LibraryService, useValue: { files: signal(library) } },
      { provide: ToolsService, useValue: { usedLibrary: used } },
      { provide: FileService, useValue: { saveTarget: async (name: string) => ({ name, write: async (blob: Blob) => void (saved = blob) }) } },
    ],
  });
  const result = await TestBed.inject(JournalIoService).exportZip();
  return { result, zip: new Uint8Array(await saved!.arrayBuffer()), markSaved };
}

describe('exporting a journal as a zip', () => {
  it('saves everything, however its scripts do, and says which failed and why', async () => {
    const { result, zip, markSaved } = await exported(async (source) => {
      if (source === 'first') return { paths: ['locks/first.rhai', 'locks/own.rhai'], error: null };
      if (source === 'throws') return { paths: ['locks/before_throwing.rhai'], error: { message: 'Runtime error: boom (line 3, position 1)', line: 3, column: 1 } };
      if (source === 'crashes') throw new Error('the engine crashed: oops');
      return { paths: ['locks/last.rhai'], error: null };
    });

    expect(result).toEqual({
      saved: 'partly_broken.zip',
      bundled: ['locks/first.rhai', 'locks/before_throwing.rhai', 'locks/last.rhai'],
      failures: [
        { entry: 'throws', message: 'Runtime error: boom (line 3, position 1)' },
        { entry: 'crashes', message: 'the engine crashed: oops' },
      ],
    });
    // Every file of the journal, the failing scripts' too.
    expect(unpackZip(zip, 'fallback')).toEqual(journal);
    // The library files of the scripts that ran, and those a failing one read first.
    const files = unzipSync(zip);
    expect(strFromU8(files['partly_broken/locks/before_throwing.rhai'])).toBe('// read before it threw');
    expect(strFromU8(files['partly_broken/locks/last.rhai'])).toBe('// last');
    expect(files['partly_broken/locks/unused.rhai']).toBeUndefined();
    expect(markSaved).toHaveBeenCalled();
  });

  it('lists no failures when every script runs', async () => {
    const { result } = await exported(async () => ({ paths: [], error: null }));
    expect(result?.failures).toEqual([]);
  });
});
