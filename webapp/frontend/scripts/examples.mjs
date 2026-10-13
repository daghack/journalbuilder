// Copies every journal in the repository (journals/<name>/journal_entry.rhai)
// to generated/examples/<name>.rhai, with an index.json listing them, for
// the editor's Examples menu.

import { mkdirSync, readdirSync, copyFileSync, existsSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const journals = join(here, '../../../journals');
const out = join(here, '../generated/examples');

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const names = readdirSync(journals, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && existsSync(join(journals, entry.name, 'journal_entry.rhai')))
  .map((entry) => entry.name)
  .sort();
for (const name of names) {
  copyFileSync(join(journals, name, 'journal_entry.rhai'), join(out, `${name}.rhai`));
}
writeFileSync(join(out, 'index.json'), JSON.stringify(names, null, 2) + '\n');
console.log(`examples: ${names.join(', ')}`);
