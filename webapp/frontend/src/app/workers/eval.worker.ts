/// <reference lib="webworker" />
// Runs journal scripts (see RenderService.evaluate): evaluates one, replies
// with its pages, then sends each page's SVG, the page the editor shows
// first. A newer script stops the SVGs of an older one.

import init, { apiNames, evaluate, lastPanic, pageSvg, setJournal } from '../../../wasm-pkg/scribe_wasm.js';
import type { EvalReply, EvalRequest, Evaluation } from '../scribe';

const post = (reply: EvalReply) => postMessage(reply);

let ready: Promise<void> | null = null;
/** The newest script asked for; older ones' SVGs are dropped. */
let generation = 0;
/** Pages of the newest script still to send, in order. */
let queue: number[] = [];

addEventListener('message', ({ data }: MessageEvent<EvalRequest>) => {
  switch (data.type) {
    case 'init':
      ready = init({ module_or_path: data.wasmUrl }).then(
        () => post({ type: 'ready', apiNames: apiNames() }),
        (err) => post({ type: 'failed', message: String(err) }),
      );
      break;
    case 'journal':
      // The app sends it just before the script that needs it: both wait
      // on `ready`, and run in the order they came.
      void ready?.then(() => {
        try {
          setJournal(data.files);
        } catch (err) {
          crashed(err);
        }
      });
      break;
    case 'evaluate':
      generation = data.generation;
      void run(data.generation, data.source, data.first);
      break;
    case 'focus':
      if (data.generation === generation && queue.includes(data.index)) {
        queue = [data.index, ...queue.filter((i) => i !== data.index)];
      }
      break;
  }
});

/** The renderer panicked: say why, and stop; the app starts another worker. */
function crashed(err: unknown) {
  generation = -1;
  post({ type: 'crashed', message: `the engine crashed: ${lastPanic() ?? String(err)}` });
  close();
}

/** Let messages in: a newer script, or a page to send first. */
const pause = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

async function run(gen: number, source: string, first: number) {
  await ready;
  if (gen !== generation) return;
  let evaluation: Evaluation;
  try {
    evaluation = evaluate(source) as Evaluation;
  } catch (err) {
    crashed(err);
    return;
  }
  post({ type: 'evaluated', generation: gen, evaluation });
  const drawn = evaluation.pages.filter((p) => p.view).map((p) => p.index);
  queue = drawn.includes(first) ? [first, ...drawn.filter((i) => i !== first)] : drawn;
  while (queue.length) {
    await pause();
    if (gen !== generation) return;
    const index = queue.shift()!;
    let svg: string | null = null;
    try {
      svg = pageSvg(index) ?? null;
    } catch (err) {
      crashed(err);
      return;
    }
    post({ type: 'svg', generation: gen, index, svg });
  }
}
