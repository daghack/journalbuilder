import { Injectable, inject, signal, untracked } from '@angular/core';
import type {
  ChainFile, ChainRun, ChainType, IllustrationType, JournalPayload, LayoutType, LockPorts, LockType, Preview, RenderPreview, ToolError, ToolFunction, ToolsReply, ToolsRequest, ValueKind,
} from '../scribe';
import { JournalStore } from './journal-store.service';

/** How long a call may take before the worker is taken to be stuck. */
const CALL_TIMEOUT_MS = 10_000;

const wasmUrl = () => new URL('wasm/scribe_wasm_bg.wasm', document.baseURI).href;

interface Pending {
  resolve(value: unknown): void;
  reject(error: Error): void;
  timer: ReturnType<typeof setTimeout>;
}

/** An answer that's either what was asked for or `{ error }`. */
export type Answer<T> = T | ToolError;

export function failed<T>(answer: Answer<T>): answer is ToolError {
  return typeof answer === 'object' && answer !== null && 'error' in answer && typeof (answer as ToolError).error === 'string';
}

/**
 * The engine, for the editors: a worker of its own, started when first
 * needed, with the journal's files. A call that crashes the engine or runs
 * too long fails, and the next starts a fresh worker.
 */
@Injectable({ providedIn: 'root' })
export class ToolsService {
  private readonly store = inject(JournalStore);
  /** Why the engine couldn't load, if it couldn't. */
  readonly loadError = signal<string | null>(null);

  private worker: Worker | null = null;
  private nextId = 0;
  private pending = new Map<number, Pending>();
  /** The journal's files the worker was last sent; null for a new worker. */
  private sent: JournalPayload | null = null;

  /**
   * `source` (an entry script) drawn: its first page as SVG. `files`
   * (path -> text) stand in for the journal's, as an editor shows them
   * before they're kept.
   */
  async preview(source: string, files: Record<string, string> | null = null): Promise<Preview> {
    return JSON.parse(await this.call<string>('previewSvg', source, files === null ? null : JSON.stringify(files)));
  }

  /** Every file of the library, by path. */
  async libraryFiles(): Promise<Record<string, string>> {
    return JSON.parse(await this.call<string>('libraryFiles'));
  }

  /** The library files `entries` (scripts) use, run with the journal's files: what an export bundles. */
  async usedLibrary(entries: string[]): Promise<Answer<{ paths: string[] }>> {
    return JSON.parse(await this.callWith<string>(60_000, 'usedLibrary', JSON.stringify(entries)));
  }

  async lockTypes(): Promise<Answer<LockType[]>> {
    return JSON.parse(await this.call<string>('lockTypes'));
  }

  async lockPorts(type: string, options: Record<string, unknown>): Promise<Answer<LockPorts>> {
    return JSON.parse(await this.call<string>('lockPorts', type, JSON.stringify(options)));
  }

  /** Run lock `type`'s examples, `source` standing in for its file if given: how many there are, or the first failure. */
  async lockExamples(type: string, source: string | null): Promise<Answer<{ examples: number }>> {
    return JSON.parse(await this.call<string>('lockExamples', type, source));
  }

  /** Every illustration type: the journal's illustrations/*.rhai over the library's. */
  async illustrationTypes(): Promise<IllustrationType[]> {
    return JSON.parse(await this.call<string>('illustrationTypes'));
  }

  /** Illustration `type`'s inputs with `options`. */
  async illustrationPorts(type: string, options: Record<string, unknown>): Promise<Answer<{ inputs: [string, string][]; fallbacks: string[] }>> {
    return JSON.parse(await this.call<string>('illustrationPorts', type, JSON.stringify(options)));
  }

  /**
   * What illustration `type` draws of `inputs` (name -> value, as a run
   * shows it), or of its own example() if null; `source` stands in for its
   * file if given.
   */
  async illustrationPreview(type: string, source: string | null, options: Record<string, unknown>, inputs: Record<string, unknown> | null): Promise<Answer<RenderPreview>> {
    return JSON.parse(await this.call<string>('illustrationPreview', type, source, JSON.stringify(options), inputs === null ? null : JSON.stringify(inputs)));
  }

  /** Every layout: the journal's layouts/*.rhai over the library's. */
  async layoutTypes(): Promise<LayoutType[]> {
    return JSON.parse(await this.call<string>('layoutTypes'));
  }

  /** Layout `name` typesetting its example(), `source` standing in for its file if given. */
  async layoutPreview(name: string, source: string | null, options: Record<string, unknown>): Promise<Answer<RenderPreview>> {
    return JSON.parse(await this.call<string>('layoutPreview', name, source, JSON.stringify(options)));
  }

  /** Every text style: the journal's styles/*.rhai over the library's. */
  async styleTypes(): Promise<LayoutType[]> {
    return JSON.parse(await this.call<string>('styleTypes'));
  }

  /** Text style `name` writing its example in a block `width` wide, `source` standing in for its file if given. */
  async stylePreview(name: string, source: string | null, options: Record<string, unknown>, width: number): Promise<Answer<RenderPreview>> {
    return JSON.parse(await this.call<string>('stylePreview', name, source, JSON.stringify(options), width));
  }

  /** Every value kind, and rhai that makes one. */
  async valueKinds(): Promise<ValueKind[]> {
    return JSON.parse(await this.call<string>('valueKinds'));
  }

  /** Every chain file: the journal's chains/*.rhai over the library's. */
  async chainTypes(): Promise<Answer<ChainType[]>> {
    return JSON.parse(await this.call<string>('chainTypes'));
  }

  /** Run the chain file `source` with `inputs` (port -> `{ text }`, `{ json }` or `{ rhai }`). */
  async runChain(source: string, options: Record<string, unknown>, inputs: Record<string, unknown>): Promise<Answer<ChainRun>> {
    return JSON.parse(await this.call<string>('runChain', source, JSON.stringify(options), JSON.stringify(inputs)));
  }

  /** The chain file `name` read back, from `source` if given. */
  async readChain(name: string, source: string | null = null): Promise<Answer<ChainFile>> {
    return JSON.parse(await this.call<string>('readChain', name, source));
  }

  private call<T>(fn: ToolFunction, ...args: unknown[]): Promise<T> {
    return this.callWith<T>(CALL_TIMEOUT_MS, fn, ...args);
  }

  private callWith<T>(timeout: number, fn: ToolFunction, ...args: unknown[]): Promise<T> {
    const worker = this.start();
    // The journal's files first, if they've changed, so the call is sure to use them.
    const files = untracked(() => this.store.resources());
    if (files !== this.sent) {
      worker.postMessage({ type: 'journal', files } satisfies ToolsRequest);
      this.sent = files;
    }
    const id = ++this.nextId;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => this.restart(`it took over ${timeout / 1000} s and was stopped`), timeout);
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
      worker.postMessage({ type: 'call', id, fn, args } satisfies ToolsRequest);
    });
  }

  private start(): Worker {
    if (this.worker) return this.worker;
    const worker = new Worker(new URL('../workers/tools.worker', import.meta.url), { type: 'module' });
    worker.onmessage = ({ data }: MessageEvent<ToolsReply>) => this.onReply(data);
    worker.onerror = (event) => {
      event.preventDefault();
      this.restart(event.message || 'the engine crashed');
    };
    worker.postMessage({ type: 'init', wasmUrl: wasmUrl() } satisfies ToolsRequest);
    this.worker = worker;
    this.sent = null;
    return worker;
  }

  /** End the worker, failing what it was doing with `reason`; the next call starts another. */
  private restart(reason: string) {
    this.worker?.terminate();
    this.worker = null;
    const pending = [...this.pending.values()];
    this.pending.clear();
    for (const call of pending) {
      clearTimeout(call.timer);
      call.reject(new Error(reason));
    }
  }

  private onReply(reply: ToolsReply) {
    switch (reply.type) {
      case 'ready':
        this.loadError.set(null);
        break;
      case 'failed':
        this.loadError.set(reply.message);
        break;
      case 'crashed':
        this.restart(reply.message);
        break;
      case 'result':
      case 'error': {
        const call = this.pending.get(reply.id);
        if (!call) return;
        this.pending.delete(reply.id);
        clearTimeout(call.timer);
        if (reply.type === 'result') call.resolve(reply.value);
        else call.reject(new Error(reply.message));
        break;
      }
    }
  }
}
