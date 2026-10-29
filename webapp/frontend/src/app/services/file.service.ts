import { Injectable } from '@angular/core';

// The File System Access API (Chromium only), when the browser has it.
interface FileHandle {
  kind: 'file';
  name: string;
  getFile(): Promise<File>;
  createWritable(): Promise<{ write(data: string | Blob): Promise<void>; close(): Promise<void> }>;
}
interface DirectoryHandle {
  kind: 'directory';
  name: string;
  values(): AsyncIterable<FileHandle | DirectoryHandle>;
}
type FilePickerWindow = Window & {
  showOpenFilePicker?: (options?: object) => Promise<FileHandle[]>;
  showSaveFilePicker?: (options?: object) => Promise<FileHandle>;
  showDirectoryPicker?: (options?: object) => Promise<DirectoryHandle>;
};

/** A directory on the user's disk, read as it's walked. */
export interface Directory {
  name: string;
  /** What's directly in it: its files by name, to read, and its subdirectories. */
  list(): Promise<{ files: Map<string, () => Promise<string>>; dirs: Directory[] }>;
}

/** What a file picker offers: a description and the file types. */
export interface FileKind {
  types: { description: string; accept: Record<string, string[]> }[];
  /** The same, for a plain file input. */
  accept: string;
  mime: string;
}

export const RHAI_FILES: FileKind = {
  types: [{ description: 'Rhai script', accept: { 'text/plain': ['.rhai'] } }],
  accept: '.rhai,text/plain',
  mime: 'text/plain',
};

export const JSON_FILES: FileKind = {
  types: [{ description: 'JSON', accept: { 'application/json': ['.json'] } }],
  accept: '.json,application/json',
  mime: 'application/json',
};

export const ZIP_FILES: FileKind = {
  types: [{ description: 'Zipped journal', accept: { 'application/zip': ['.zip'] } }],
  accept: '.zip,application/zip',
  mime: 'application/zip',
};

/** A script opened or saved: its name, and where to save it back, if the browser can. */
export interface OpenedFile {
  name: string;
  text: string;
  handle: FileHandle | null;
}

/** Where the user picked to save a file: the name it's saved as, and how to write it there. */
export interface SaveTarget {
  name: string;
  write(blob: Blob): Promise<void>;
}

/** Reading and writing local files: scripts, journals' directories, and exports to download. */
@Injectable({ providedIn: 'root' })
export class FileService {
  private get picker(): FilePickerWindow {
    return window as FilePickerWindow;
  }

  /** Let the user pick a script; null if they don't. */
  async open(kind: FileKind = RHAI_FILES): Promise<OpenedFile | null> {
    if (this.picker.showOpenFilePicker) {
      try {
        const [handle] = await this.picker.showOpenFilePicker({ types: kind.types, multiple: false });
        const file = await handle.getFile();
        return { name: file.name, text: await file.text(), handle };
      } catch (err) {
        if (isAbort(err)) return null;
        throw err;
      }
    }
    const file = await pickWithInput(kind.accept);
    return file ? { name: file.name, text: await file.text(), handle: null } : null;
  }

  /** Let the user pick a file, read as bytes; null if they don't pick one. */
  async openBytes(kind: FileKind): Promise<{ name: string; bytes: Uint8Array } | null> {
    let file: File | null;
    if (this.picker.showOpenFilePicker) {
      try {
        const [handle] = await this.picker.showOpenFilePicker({ types: kind.types, multiple: false });
        file = await handle.getFile();
      } catch (err) {
        if (isAbort(err)) return null;
        throw err;
      }
    } else {
      file = await pickWithInput(kind.accept);
    }
    return file ? { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) } : null;
  }

  /** Let the user pick a directory; null if they don't. */
  async openDirectory(): Promise<Directory | null> {
    if (this.picker.showDirectoryPicker) {
      try {
        return handleDirectory(await this.picker.showDirectoryPicker({ mode: 'read' }));
      } catch (err) {
        if (isAbort(err)) return null;
        throw err;
      }
    }
    const files = await pickDirectoryWithInput();
    return files ? filesDirectory(files) : null;
  }

  /**
   * Save `text` to `handle`, or where the user picks (named `name`) if
   * there's no handle. Returns where it went, or null if the user didn't
   * pick anywhere. Without the File System Access API it downloads.
   */
  async save(text: string, name: string, handle: FileHandle | null, kind: FileKind = RHAI_FILES): Promise<OpenedFile | null> {
    if (!handle && this.picker.showSaveFilePicker) {
      try {
        handle = await this.picker.showSaveFilePicker({ suggestedName: name, types: kind.types });
      } catch (err) {
        if (isAbort(err)) return null;
        throw err;
      }
    }
    if (handle) {
      const writable = await handle.createWritable();
      await writable.write(text);
      await writable.close();
      return { name: handle.name, text, handle };
    }
    // A download can't tell us what the user saves it as, so ask for the name.
    const chosen = prompt('Save as', name)?.trim();
    if (!chosen) return null;
    const extension = /\.[^.]*$/.exec(name)?.[0] ?? '';
    const saveAs = /\.[^.]*$/.test(chosen) ? chosen : chosen + extension;
    this.download(new Blob([text], { type: kind.mime }), saveAs);
    return { name: saveAs, text, handle: null };
  }

  /**
   * Let the user pick where to save a file, named `name` to begin with, to
   * write to later; null if they don't pick anywhere. Ask before working
   * out what to save: the browser shows the picker only while the click
   * that asked for it is recent. Without the File System Access API,
   * writing downloads it, as `name`.
   */
  async saveTarget(name: string, kind: FileKind): Promise<SaveTarget | null> {
    if (!this.picker.showSaveFilePicker) return { name, write: async (blob) => this.download(blob, name) };
    let handle: FileHandle;
    try {
      handle = await this.picker.showSaveFilePicker({ suggestedName: name, types: kind.types });
    } catch (err) {
      if (isAbort(err)) return null;
      throw err;
    }
    return {
      name: handle.name,
      async write(blob) {
        const writable = await handle.createWritable();
        await writable.write(blob);
        await writable.close();
      },
    };
  }

  /** Hand `blob` to the browser to save as `name`. */
  download(blob: Blob, name: string) {
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
  }
}

function isAbort(err: unknown): boolean {
  return err instanceof DOMException && err.name === 'AbortError';
}

/** The directory `handle` names, read as it's listed. */
function handleDirectory(handle: DirectoryHandle): Directory {
  return {
    name: handle.name,
    async list() {
      const files = new Map<string, () => Promise<string>>();
      const dirs: Directory[] = [];
      for await (const entry of handle.values()) {
        if (entry.kind === 'directory') dirs.push(handleDirectory(entry));
        else files.set(entry.name, async () => (await entry.getFile()).text());
      }
      return { files, dirs };
    },
  };
}

/** The directory a plain file input picked, from its files' paths. */
function filesDirectory(picked: File[]): Directory {
  interface Node { files: Map<string, File>; dirs: Map<string, Node> }
  const root: Node = { files: new Map(), dirs: new Map() };
  let name = 'folder';
  for (const file of picked) {
    const [top, ...path] = file.webkitRelativePath.split('/');
    name = top;
    let node = root;
    for (const dir of path.slice(0, -1)) {
      let next = node.dirs.get(dir);
      if (!next) node.dirs.set(dir, (next = { files: new Map(), dirs: new Map() }));
      node = next;
    }
    node.files.set(path[path.length - 1], file);
  }
  const directory = (name: string, node: Node): Directory => ({
    name,
    list: async () => ({
      files: new Map([...node.files].map(([name, file]) => [name, () => file.text()])),
      dirs: [...node.dirs].map(([name, dir]) => directory(name, dir)),
    }),
  });
  return directory(name, root);
}

/** The files under a directory picked with a plain file input; null if none is. */
function pickDirectoryWithInput(): Promise<File[] | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.webkitdirectory = true;
    input.onchange = () => resolve(input.files ? [...input.files] : null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

/** A file picked with a plain file input; null if none is. */
function pickWithInput(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}
