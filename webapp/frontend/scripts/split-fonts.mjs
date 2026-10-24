// Splits a fonts.json (as the font editor exports it) into the library's
// glyphsets, webapp/library/glyphsets/: a NAME.json for each glyphset, laid
// out as it was. Glyphsets no longer in the file are left alone; delete
// their files by hand.
//   npm run split-fonts -- exported.json

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const input = process.argv[2];
if (!input) {
  console.error('usage: npm run split-fonts -- FONTS.json');
  process.exit(1);
}
const out = join(dirname(fileURLToPath(import.meta.url)), '../../library/glyphsets');
mkdirSync(out, { recursive: true });
let name = null;
let lines = [];
let count = 0;
for (const line of readFileSync(input, 'utf8').split('\n')) {
  const start = line.match(/^\t("(?:[^"\\]|\\.)*"): \{$/);
  if (!name && start) {
    name = JSON.parse(start[1]);
    lines = ['{'];
  } else if (name && /^\t\},?$/.test(line)) {
    writeFileSync(join(out, `${name}.json`), [...lines, '}'].join('\n') + '\n');
    count++;
    name = null;
  } else if (name) {
    lines.push(line.replace(/^\t/, ''));
  }
}
console.log(`wrote ${count} font file(s) to ${out}`);
