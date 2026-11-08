// The flow simulator's page: a view (simview.ts) of an engine, the scene
// kept in the browser as it changes.

import init, { FlowSim } from '../wasm-pkg/flowsim.js';
import { FlowSimView } from './simview';
import { loadText } from './store';

const SCENE_KEY = 'flowsim.scene';

function part(id: string): HTMLElement {
  return document.getElementById(id)!;
}

async function start(): Promise<void> {
  await init();
  const saved = loadText(SCENE_KEY);
  let sim: FlowSim;
  try {
    sim = new FlowSim(saved ?? '');
  } catch (e) {
    console.warn('The saved scene would not load; starting afresh.', e);
    sim = new FlowSim('');
  }
  const parts = { bar: part('bar'), rail: part('tools'), stage: part('stage'), panel: part('panel'), status: part('status') };
  const view = new FlowSimView(sim, parts, { sceneKey: SCENE_KEY, files: true });
  if (import.meta.env.DEV) Object.assign(window, { flowsim: view });
}

start().catch((e) => {
  part('status').textContent = `The engine failed to load: ${e}`;
  console.error(e);
});
