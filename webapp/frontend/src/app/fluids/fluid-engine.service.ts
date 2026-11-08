import { Injectable, signal, type OnDestroy } from '@angular/core';
import type { Box, FluidPrepared, FluidReply, FluidRequest, FluidResult, JournalPayload, RawField } from '../scribe';

const wasmUrl = () => new URL('wasm/scribe_wasm_bg.wasm', document.baseURI).href;

interface Pending {
  resolve(reply: FluidReply): void;
  reject(error: Error): void;
}

/**
 * The engine, for the Fluid tab: a worker of its own that keeps the page
 * the tab works on between calls. If it crashes, the calls under way fail,
 * a fresh worker starts, and `restarts` counts it, for the tab to prepare
 * its page again.
 */
@Injectable()
export class FluidEngine implements OnDestroy {
  readonly restarts = signal(0);
  /** Why the engine couldn't load, if it couldn't. */
  readonly loadError = signal<string | null>(null);

  private worker: Worker | null = null;
  private nextId = 0;
  private pending = new Map<number, Pending>();
  private sent: JournalPayload | null = null;

  /** Run the entry `source` with the journal's `files` and keep its page `page` (counted from 0), for the fluid `name`. */
  async prepare(source: string, page: number, name: string, files: JournalPayload): Promise<FluidPrepared> {
    const worker = this.start();
    if (files !== this.sent) {
      worker.postMessage({ type: 'journal', files } satisfies FluidRequest);
      this.sent = files;
    }
    const reply = await this.call({ type: 'prepare', id: 0, source, page, name });
    return reply.type === 'prepared' ? reply.prepared : { pages: [], warnings: [], error: { message: 'no page came back' } };
  }

  /** The ink of the kept page's `canvases` over `box`, as `nx` × `ny` alpha. */
  async mask(canvases: string[], box: Box, nx: number, ny: number): Promise<Uint8Array> {
    const reply = await this.call({ type: 'mask', id: 0, canvases, box, nx, ny });
    if (reply.type !== 'mask') throw new Error('no mask came back');
    return reply.alpha;
  }

  /** The kept page distorted by `field`, with `settings` (a fluid file's text), added to `target` if the entry doesn't add it; `live` draws it quickly, coarser, for a running flow. */
  async apply(field: RawField, settings: string, target: string | null, live: boolean): Promise<FluidResult> {
    const reply = await this.call({ type: 'apply', id: 0, field, settings, target, live });
    return reply.type === 'applied' ? reply.result : { svg: null, view: null, error: { message: 'no page came back' } };
  }

  ngOnDestroy() {
    this.worker?.terminate();
    this.worker = null;
    this.failAll('the Fluid tab closed');
  }

  private start(): Worker {
    if (!this.worker) {
      const worker = new Worker(new URL('../workers/fluid.worker', import.meta.url), { type: 'module' });
      worker.onmessage = ({ data }: MessageEvent<FluidReply>) => this.onReply(data);
      worker.onerror = (event) => this.loadError.set(event.message || 'the fluid worker failed to start');
      worker.postMessage({ type: 'init', wasmUrl: wasmUrl() } satisfies FluidRequest);
      this.worker = worker;
      this.sent = null;
    }
    return this.worker;
  }

  private call(request: Extract<FluidRequest, { id: number }>): Promise<FluidReply> {
    const worker = this.start();
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      worker.postMessage({ ...request, id } satisfies FluidRequest);
    });
  }

  private onReply(reply: FluidReply) {
    switch (reply.type) {
      case 'ready':
        this.loadError.set(null);
        break;
      case 'failed':
        this.loadError.set(reply.message);
        break;
      case 'crashed':
        this.worker?.terminate();
        this.worker = null;
        this.failAll(reply.message);
        this.restarts.update((n) => n + 1);
        break;
      case 'error': {
        const pending = this.pending.get(reply.id);
        this.pending.delete(reply.id);
        pending?.reject(new Error(reply.message));
        break;
      }
      default: {
        const pending = this.pending.get(reply.id);
        this.pending.delete(reply.id);
        pending?.resolve(reply);
      }
    }
  }

  private failAll(message: string) {
    const pending = [...this.pending.values()];
    this.pending.clear();
    for (const p of pending) p.reject(new Error(message));
  }
}
