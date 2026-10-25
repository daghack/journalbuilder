import type { Directory } from '../services/file.service';
import { manifestText } from './journal-files';
import { readDirectory } from './journal-folder';

interface Tree { [name: string]: string | Tree }

/** A directory on disk, as the file service gives it. */
function directory(name: string, tree: Tree): Directory {
  return {
    name,
    list: async () => ({
      files: new Map(Object.entries(tree).filter(([, v]) => typeof v === 'string').map(([n, text]) => [n, async () => text as string])),
      dirs: Object.entries(tree).filter(([, v]) => typeof v !== 'string').map(([n, sub]) => directory(n, sub as Tree)),
    }),
  };
}

const letters: Tree = {
  'journal.json': manifestText({ version: 1, title: 'Letters', entries: ['letter_1'], bundled: ['locks/huffman.rhai'] }),
  'README.md': 'not the journal\'s',
  entries: { 'letter_1.rhai': 'journal_entry.add(new_page());\n', '.DS_Store': '' },
  locks: { 'swap.rhai': '// swap\n', 'huffman.rhai': '// the library\'s, bundled' },
  lib: { deep: { 'helpers.rhai': 'fn f() {}' } },
  letters_output: { 'page_0.png': '' },
};

describe('a journal read from its directory', () => {
  it('has its own files, by path, and not those it bundled', async () => {
    expect(await readDirectory(directory('letters', letters))).toEqual({
      name: 'letters',
      manifest: { version: 1, title: 'Letters', entries: ['letter_1'] },
      files: {
        'entries/letter_1.rhai': 'journal_entry.add(new_page());\n',
        'locks/swap.rhai': '// swap\n',
        'lib/deep/helpers.rhai': 'fn f() {}',
      },
    });
  });

  it('is found in a directory holding it, but not among several', async () => {
    expect((await readDirectory(directory('picked', { letters, notes: {} }))).name).toBe('letters');
    await expect(readDirectory(directory('journals', { letters, other: letters }))).rejects.toThrow(/several journals \(letters, other\)/);
    await expect(readDirectory(directory('empty', { notes: {} }))).rejects.toThrow(/no journal.json/);
  });

  it('says what its manifest lacks', async () => {
    const missing = { ...letters, entries: {} };
    await expect(readDirectory(directory('letters', missing))).rejects.toThrow(/entry 'letter_1' has no file/);
  });
});
