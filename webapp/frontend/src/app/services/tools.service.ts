import { Injectable, inject, signal, untracked } from '@angular/core';
import type { ChainFile, ChainRun, LockPorts, LockType, Preview, ToolError, ToolFunction, ToolsReply, ToolsRequest, WorkspacePayload } from '../scribe';
import { WorkspaceService } from './workspace.service';

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
 * The renderer, for the font and lock editors: a worker of its own, started
 * when first needed, with the workspace. A call that crashes the renderer
 * or runs too long fails, and the next starts a fresh worker.
 */
@Injectable({ providedIn: 'root' })
export class ToolsService {
  private readonly workspace = inject(WorkspaceService);
  /** Why the renderer couldn't load, if it couldn't. */
  readonly loadError = signal<string | null>(null);

  private worker: Worker | null = null;
  private nextId = 0;
  private pending = new Map<number, Pending>();
  /** The workspace the worker was last sent; null for a new worker. */
  private sent: WorkspacePayload | null = null;

  /** `text` (a journal script) drawn: its first page as SVG. Runs with `fonts` if given. */
  async preview(source: string, fonts: string | null): Promise<Preview> {
    return JSON.parse(await this.call<string>('previewSvg', source, fonts));
  }

  builtinFonts(): Promise<string> {
    return this.call<string>('builtinFonts');
  }

  async lockTypes(): Promise<Answer<LockType[]>> {
    return JSON.parse(await this.call<string>('lockTypes'));
  }

  async lockPorts(type: string, options: Record<string, unknown>): Promise<Answer<LockPorts>> {
    return JSON.parse(await this.call<string>('lockPorts', type, JSON.stringify(options)));
  }

  /** Run the composite lock file `source` with `inputs` (name -> `{ text }` or `{ json }`). */
  async runChain(source: string, options: Record<string, unknown>, inputs: Record<string, unknown>): Promise<Answer<ChainRun>> {
    return JSON.parse(await this.call<string>('runChain', source, JSON.stringify(options), JSON.stringify(inputs)));
  }

  /** The composite lock type `type` read back, from `source` if given. */
  async readChain(type: string, source: string | null = null): Promise<Answer<ChainFile>> {
    return JSON.parse(await this.call<string>('readChain', type, source));
  }

  /** Run the examples of lock file `source` as type `name`. */
  async checkLock(name: string, source: string): Promise<Answer<{ examples: number }>> {
    return JSON.parse(await this.call<string>('checkLock', name, source));
  }

  private call<T>(fn: ToolFunction, ...args: unknown[]): Promise<T> {
    const worker = this.start();
    // The workspace first, if it's changed, so the call is sure to use it.
    const workspace = untracked(() => this.workspace.payload());
    if (workspace !== this.sent) {
      worker.postMessage({ type: 'workspace', workspace } satisfies ToolsRequest);
      this.sent = workspace;
    }
    const id = ++this.nextId;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => this.restart(`it took over ${CALL_TIMEOUT_MS / 1000} s and was stopped`), CALL_TIMEOUT_MS);
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
      this.restart(event.message || 'the renderer crashed');
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
