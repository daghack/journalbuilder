/// <reference lib="webworker" />
// Renders pages with Darkly over WebGPU (see RenderService): page images
// for the preview and PNG exports, and PDFs. One render at a time; the app
// cancels one by ending the worker.

import init, { Renderer, lastPanic, setWorkspace } from '../../../wasm-pkg/scribe_wasm.js';
import type { DarklyReply, DarklyRequest, WorkspacePayload } from '../scribe';

const post = (reply: DarklyReply, transfer: Transferable[] = []) => postMessage(reply, transfer);

let renderer: Promise<Renderer> | null = null;
let wasmUrl = '';
/** The workspace to render with, and the one the renderer has. */
let workspace: WorkspacePayload | null = null;
let applied: WorkspacePayload | null = null;

function getRenderer(): Promise<Renderer> {
  if (!renderer) {
    if (!('gpu' in navigator)) {
      return Promise.reject(new Error("this browser has no WebGPU here, which Darkly renders need; try a current Chrome, Edge or Safari"));
    }
    renderer = init({ module_or_path: wasmUrl }).then(() => Renderer.create());
    renderer.catch(() => (renderer = null));
  }
  return renderer;
}

/** A copy of `bytes` in an ArrayBuffer of its own, to transfer. */
function buffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.slice().buffer as ArrayBuffer;
}

let queue: Promise<void> = Promise.resolve();
/** The render under way. */
let current: number | null = null;

// A panic inside a render surfaces here, not in its promise (which never
// settles): fail the render with what the panic said. The app then ends
// this worker, whose module can't be trusted after a panic.
const onCrash = (event: Event, err: unknown) => {
  event.preventDefault();
  const message = `the renderer crashed: ${safeLastPanic() ?? String(err)}`;
  if (current !== null) post({ type: 'error', id: current, message });
};
addEventListener('error', (event) => onCrash(event, (event as ErrorEvent).message));
addEventListener('unhandledrejection', (event) => onCrash(event, (event as PromiseRejectionEvent).reason));

function safeLastPanic(): string | undefined {
  try {
    return lastPanic() ?? undefined;
  } catch {
    return undefined;
  }
}

addEventListener('message', ({ data }: MessageEvent<DarklyRequest>) => {
  if (data.type === 'init') {
    wasmUrl = data.wasmUrl;
    return;
  }
  if (data.type === 'workspace') {
    // Taken up by the next render, not one under way.
    workspace = data.workspace;
    return;
  }
  // One at a time: the renderer has one GPU canvas.
  queue = queue.then(() => handle(data)).catch(() => {});
});

async function handle(request: Exclude<DarklyRequest, { type: 'init' | 'workspace' }>) {
  const { id } = request;
  current = id;
  try {
    const r = await getRenderer();
    if (workspace !== applied) {
      setWorkspace(workspace?.fonts ?? null, workspace?.locks ?? {});
      applied = workspace;
    }
    if (request.type === 'renderPages') {
      const pages = request.indices.length;
      for (const [n, index] of request.indices.entries()) {
        const progress = (fraction: number) => post({ type: 'progress', id, page: n, pages, fraction });
        const png: Uint8Array | undefined = await r.renderPage(request.source, index, request.options, progress);
        const copy = png ? buffer(png) : null;
        post({ type: 'page', id, index, png: copy }, copy ? [copy] : []);
      }
      post({ type: 'done', id });
    } else {
      const progress = (page: number, pages: number, fraction: number) => post({ type: 'progress', id, page, pages, fraction });
      const pdf: Uint8Array = await r.exportPdf(request.source, request.title, request.options, progress);
      const copy = buffer(pdf);
      post({ type: 'done', id, pdf: copy }, [copy]);
    }
  } catch (err) {
    post({ type: 'error', id, message: err instanceof Error ? err.message : String(err) });
  } finally {
    current = null;
  }
}
