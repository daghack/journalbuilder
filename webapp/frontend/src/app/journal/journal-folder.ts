// A journal read from its directory on the user's disk, unzipped, as
// journalbuild reads it: one of the repository's journals/NAME, say.

import type { Directory } from '../services/file.service';
import { KIND_INFO, MANIFEST, journalName, readJournal, splitPath, type JournalData, type Kind } from './journal-files';

/**
 * The journal in `dir`: a journal's directory (it has a journal.json), or
 * one holding a single journal's directory, as a zip may. Library files its
 * manifest lists as bundled are dropped, as an import of a zip drops them.
 */
export async function readDirectory(dir: Directory): Promise<JournalData> {
  const listing = await dir.list();
  const manifest = listing.files.get(MANIFEST);
  if (manifest) return readFrom(dir, manifest, listing.dirs);
  const inner = (await Promise.all(listing.dirs.map(async (sub) => ({ sub, listing: await sub.list() }))))
    .filter(({ listing }) => listing.files.has(MANIFEST));
  if (!inner.length) throw new Error(`${dir.name} has no journal.json: is it a journal's directory?`);
  if (inner.length > 1) {
    const names = inner.map(({ sub }) => sub.name).sort().join(', ');
    throw new Error(`${dir.name} holds several journals (${names}): pick one of their directories`);
  }
  const [{ sub, listing: subListing }] = inner;
  return readFrom(sub, subListing.files.get(MANIFEST)!, subListing.dirs);
}

async function readFrom(dir: Directory, manifest: () => Promise<string>, dirs: Directory[]): Promise<JournalData> {
  const files: Record<string, string> = {};
  for (const sub of dirs) {
    if (KIND_INFO.has(sub.name as Kind)) await readTree(sub, `${sub.name}/`, files);
  }
  return readJournal(journalName(dir.name, 'imported'), await manifest(), files);
}

/** The files a journal has under `dir`, by path from the journal's directory, into `into`. */
async function readTree(dir: Directory, prefix: string, into: Record<string, string>) {
  const { files, dirs } = await dir.list();
  await Promise.all([...files]
    .filter(([name]) => !name.startsWith('.') && splitPath(prefix + name))
    .map(async ([name, read]) => { into[prefix + name] = await read(); }));
  for (const sub of dirs) {
    if (!sub.name.startsWith('.')) await readTree(sub, `${prefix}${sub.name}/`, into);
  }
}
