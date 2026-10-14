import { Injectable } from '@angular/core';

// The File System Access API (Chromium only), when the browser has it.
interface FileHandle {
  name: string;
  getFile(): Promise<File>;
  createWritable(): Promise<{ write(data: string | Blob): Promise<void>; close(): Promise<void> }>;
}
type FilePickerWindow = Window & {
  showOpenFilePicker?: (options?: object) => Promise<FileHandle[]>;
  showSaveFilePicker?: (options?: object) => Promise<FileHandle>;
};

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

/** A script opened or saved: its name, and where to save it back, if the browser can. */
export interface OpenedFile {
  name: string;
  text: string;
  handle: FileHandle | null;
}

/** Reading and writing local files: scripts, and exports to download. */
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
    this.download(new Blob([text], { type: kind.mime }), name);
    return { name, text, handle: null };
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
