/// <reference lib="webworker" />
// Runs what the editors ask of the engine (see ToolsService): previews,
// the library's files, and lock types, ports and test runs. Kept apart from
// the script worker so neither waits on, or crashes, the other.

import init, {
  chainTypes, illustrationPorts, illustrationPreview, illustrationTypes, lastPanic, layoutPreview, layoutTypes, libraryFiles, lockExamples, lockPorts, lockTypes,
  previewSvg, readChain, runChain, setJournal, styleChoices, stylePreview, styleTypes, usedLibrary, valueKinds,
} from '../../../wasm-pkg/scribe_wasm.js';
import type { ToolFunction, ToolsReply, ToolsRequest } from '../scribe';

const post = (reply: ToolsReply) => postMessage(reply);

const tools: Record<ToolFunction, (...args: never[]) => unknown> = {
  previewSvg, libraryFiles, usedLibrary, lockTypes, lockPorts, lockExamples, chainTypes, runChain, readChain, valueKinds,
  illustrationTypes, illustrationPorts, illustrationPreview, layoutTypes, layoutPreview, styleTypes, stylePreview, styleChoices,
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
    case 'journal':
      void ready.then(() => guard(() => setJournal(data.files)), () => {});
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
        (err) => post({ type: 'error', id: data.id, message: `the engine didn't load: ${String(err)}` }),
      );
      break;
  }
});

/** Run `f`; if the renderer panics, say why and stop: the app starts another worker. */
function guard(f: () => void) {
  try {
    f();
  } catch (err) {
    post({ type: 'crashed', message: `the engine crashed: ${lastPanic() ?? String(err)}` });
    close();
  }
}
