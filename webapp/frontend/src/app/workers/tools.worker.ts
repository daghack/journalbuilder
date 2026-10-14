/// <reference lib="webworker" />
// Runs what the font and lock editors ask of the renderer (see
// ToolsService): text previews, and lock types, ports and test runs. Kept
// apart from the script worker so neither waits on, or crashes, the other.

import init, { builtinFonts, checkLock, lastPanic, lockPorts, lockTypes, previewSvg, readChain, runChain, setWorkspace } from '../../../wasm-pkg/scribe_wasm.js';
import type { ToolFunction, ToolsReply, ToolsRequest } from '../scribe';

const post = (reply: ToolsReply) => postMessage(reply);

const tools: Record<ToolFunction, (...args: never[]) => unknown> = {
  previewSvg, builtinFonts, lockTypes, lockPorts, runChain, readChain, checkLock,
};

let ready: Promise<void> = Promise.reject(new Error('not started'));
ready.catch(() => {});

addEventListener('message', ({ data }: MessageEvent<ToolsRequest>) => {
  switch (data.type) {
    case 'init':
      ready = init({ module_or_path: data.wasmUrl }).then(
        () => post({ type: 'ready' }),
        (err) => {
          post({ type: 'failed', message: String(err) });
          throw err;
        },
      );
      break;
    case 'workspace':
      void ready.then(() => guard(() => setWorkspace(data.workspace.fonts, data.workspace.locks)), () => {});
      break;
    case 'call':
      void ready.then(
        () => guard(() => {
          try {
            post({ type: 'result', id: data.id, value: (tools[data.fn] as (...args: unknown[]) => unknown)(...data.args) });
          } catch (err) {
            if (lastPanic()) throw err;
            post({ type: 'error', id: data.id, message: err instanceof Error ? err.message : String(err) });
          }
        }),
        (err) => post({ type: 'error', id: data.id, message: `the renderer didn't load: ${String(err)}` }),
      );
      break;
  }
});

/** Run `f`; if the renderer panics, say why and stop: the app starts another worker. */
function guard(f: () => void) {
  try {
    f();
  } catch (err) {
    post({ type: 'crashed', message: `the renderer crashed: ${lastPanic() ?? String(err)}` });
    close();
  }
}
