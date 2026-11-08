/// <reference lib="webworker" />
// Runs the Fluid tab's engine calls (see FluidEngine): the page it works
// on, its canvases' ink as a mask for the flow, and the page distorted by
// the live fluid. A worker of its own, which keeps the page between calls,
// so the editor's other runs neither wait on it nor disturb it.

import init, { fluidApply, fluidMask, fluidPrepare, lastPanic, setJournal } from '../../../wasm-pkg/scribe_wasm.js';
import type { FluidReply, FluidRequest } from '../scribe';

const post = (reply: FluidReply, transfer: Transferable[] = []) => postMessage(reply, transfer);

let ready: Promise<void> = Promise.reject(new Error('not started'));
ready.catch(() => {});

addEventListener('message', ({ data }: MessageEvent<FluidRequest>) => {
  if (data.type === 'init') {
    ready = init({ module_or_path: data.wasmUrl }).then(
      () => post({ type: 'ready' }),
      (err) => {
        post({ type: 'failed', message: String(err) });
        throw err;
      },
    );
    return;
  }
  void ready.then(() => guard(() => handle(data)), (err) => {
    if ('id' in data) post({ type: 'error', id: data.id, message: `the engine didn't load: ${String(err)}` });
  });
});

function handle(data: Exclude<FluidRequest, { type: 'init' }>) {
  switch (data.type) {
    case 'journal':
      setJournal(data.files);
      break;
    case 'prepare':
      post({ type: 'prepared', id: data.id, prepared: JSON.parse(fluidPrepare(data.source, data.page, data.name)) });
      break;
    case 'mask': {
      const [x, y, width, height] = data.box;
      try {
        const alpha = fluidMask(data.canvases, x, y, width, height, data.nx, data.ny);
        post({ type: 'mask', id: data.id, alpha }, [alpha.buffer]);
      } catch (err) {
        if (lastPanic()) throw err;
        post({ type: 'error', id: data.id, message: err instanceof Error ? err.message : String(err) });
      }
      break;
    }
    case 'apply': {
      const f = data.field;
      post({ type: 'applied', id: data.id, result: JSON.parse(fluidApply(f.nx, f.ny, f.h, f.u, f.v, f.pressure, f.dye, data.settings, data.target ?? undefined, data.live)) });
      break;
    }
  }
}

/** Run `f`; if the renderer panics, say why and stop: the app starts another worker. */
function guard(f: () => void) {
  try {
    f();
  } catch (err) {
    post({ type: 'crashed', message: `the engine crashed: ${lastPanic() ?? String(err)}` });
    close();
  }
}
