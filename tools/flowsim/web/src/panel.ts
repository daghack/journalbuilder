// The side panel: the selection's properties, then the view, particle and
// flow settings.

import { type Edit, check, colour, el, numberBox, select, slider } from './form';
import { type Edge, type FlowSettings, type Obj, type ParticleSettings, label } from './model';
import { TOOLS, type ToolName } from './tools';

export type ViewMode = 'dye' | 'speed' | 'vorticity' | 'pressure' | 'plain';

export interface ViewSettings {
  mode: ViewMode;
  arrows: boolean;
  recorded: boolean;
  live: boolean;
  particles: boolean;
  objects: boolean;
  brush: number;
  /** Whether particles' trails become recorded paths when they end. */
  record: boolean;
  /** Paths coloured by how old their particle was there, the hue round once a `rainbowPeriod` seconds. */
  rainbow: boolean;
  rainbowPeriod: number;
  /** The dye stirring pours; none for the plain dye. */
  stirColor?: string;
}

export interface PanelHost {
  tool: ToolName;
  selected(): Obj | null;
  selectedPath: number | null;
  pathPoints(index: number): number;
  flow: FlowSettings;
  particles: ParticleSettings;
  view: ViewSettings;
  objectEdit: Edit;
  settingsEdit: Edit;
  viewEdit: Edit;
  deleteSelection(): void;
  duplicateSelection(): void;
  preset(name: 'tunnel' | 'tank'): void;
}

const EDGES: [Edge, string][] = [
  ['wall', 'Wall'],
  ['inflow', 'Inflow'],
  ['open', 'Open'],
];

export function buildPanel(root: HTMLElement, host: PanelHost, open: Record<string, boolean>): void {
  const scrolled = root.scrollTop;
  root.replaceChildren(selectionSection(host), viewSection(host, open), particleSection(host, open), flowSection(host, open));
  root.scrollTop = scrolled;
}

function details(name: string, title: string, open: Record<string, boolean>, defaultOpen: boolean): HTMLDetailsElement {
  const d = el('details');
  d.open = open[name] ?? defaultOpen;
  d.addEventListener('toggle', () => (open[name] = d.open));
  d.append(el('summary', undefined, title));
  return d;
}

function selectionSection(host: PanelHost): HTMLElement {
  const s = el('section');
  const o = host.selected();
  const e = host.objectEdit;
  if (o) {
    s.append(el('h2', undefined, label(o)));
    s.append(...objectRows(o, e));
    const buttons = el('div', 'buttons');
    const dup = el('button', undefined, 'Duplicate');
    dup.type = 'button';
    dup.addEventListener('click', () => host.duplicateSelection());
    const del = el('button', undefined, 'Delete');
    del.type = 'button';
    del.addEventListener('click', () => host.deleteSelection());
    buttons.append(dup, del);
    s.append(buttons);
    return s;
  }
  if (host.selectedPath !== null) {
    s.append(el('h2', undefined, 'Recorded path'));
    s.append(el('p', 'hint', `${host.pathPoints(host.selectedPath)} points. Delete removes it.`));
    const del = el('button', undefined, 'Delete path');
    del.type = 'button';
    del.addEventListener('click', () => host.deleteSelection());
    const buttons = el('div', 'buttons');
    buttons.append(del);
    s.append(buttons);
    return s;
  }
  const tool = TOOLS.find((t) => t.name === host.tool);
  s.append(el('h2', undefined, tool?.label ?? 'Select'));
  s.append(el('p', 'hint', tool?.hint ?? ''));
  if (host.tool === 'stir') {
    s.append(
      slider('Brush', () => host.view.brush, (v) => (host.view.brush = v), { min: 5, max: 300, step: 1 }, host.viewEdit),
      colour('Dye', () => host.view.stirColor, (v) => (host.view.stirColor = v), host.viewEdit, 'The colour of the dye stirring pours; colours mix where they meet'),
    );
  }
  return s;
}

function objectRows(o: Obj, e: Edit): HTMLElement[] {
  const radius = (get: () => number, set: (v: number) => void, name = 'Radius', max = 400) =>
    slider(name, get, set, { min: 1, max, step: 1 }, e);
  const angle = (get: () => number, set: (v: number) => void) =>
    slider('Angle', get, set, { min: -180, max: 180, step: 1, unit: '°' }, e);
  const emit = (get: () => number, set: (v: number) => void) =>
    slider('Emit', get, set, { min: 0, max: 10, step: 0.1, unit: '/s', title: 'Particles a second from each seed while the flow runs' }, e);
  const count = (get: () => number, set: (v: number) => void) =>
    slider('Seeds', get, (v) => set(Math.round(v)), { min: 1, max: 200, step: 1 }, e);
  switch (o.kind) {
    case 'pebble':
      return [radius(() => o.radius, (v) => (o.radius = v))];
    case 'rect':
      return [
        slider('Width', () => o.size[0], (v) => (o.size[0] = v), { min: 2, max: 1200, step: 1 }, e),
        slider('Height', () => o.size[1], (v) => (o.size[1] = v), { min: 2, max: 1200, step: 1 }, e),
        angle(() => o.angle, (v) => (o.angle = v)),
      ];
    case 'wall':
      return [
        slider('Thickness', () => o.thickness, (v) => (o.thickness = v), { min: 1, max: 120, step: 1 }, e),
        el('p', 'hint', `${o.points.length} points; drag one to move it.`),
      ];
    case 'jet':
      return [
        angle(() => o.angle, (v) => (o.angle = v)),
        slider('Speed', () => o.speed, (v) => (o.speed = v), { min: 0, max: 800, step: 1 }, e),
        radius(() => o.radius, (v) => (o.radius = v), 'Radius', 300),
      ];
    case 'vortex':
      return [
        slider('Spin', () => o.spin, (v) => (o.spin = v), { min: -600, max: 600, step: 1, title: 'Speed at its radius; clockwise when positive' }, e),
        radius(() => o.radius, (v) => (o.radius = v), 'Radius', 400),
      ];
    case 'source':
      return [
        slider('Flow', () => o.speed, (v) => (o.speed = v), { min: -500, max: 500, step: 1, title: 'How fast fluid crosses its radius: out when positive, in when negative (a sink)' }, e),
        radius(() => o.radius, (v) => (o.radius = v), 'Radius', 300),
        dye(() => o.color, (v) => (o.color = v), e),
        moves(() => !!o.moving, (v) => (o.moving = v), e),
      ];
    case 'attractor':
      return [
        slider('Strength', () => o.strength, (v) => (o.strength = v), { min: -400, max: 400, step: 1, title: 'Pull on particles (push when negative), as a speed' }, e),
        radius(() => o.radius, (v) => (o.radius = v), 'Reach', 1000),
        select('Falloff', () => o.falloff, (v) => (o.falloff = v), [
          ['inverse_square', 'Inverse square'],
          ['gaussian', 'Gaussian'],
        ], e),
        slider('Spin', () => o.spin, (v) => (o.spin = v), { min: -400, max: 400, step: 1, title: 'Sideways pull that makes particles orbit; clockwise when positive' }, e),
        slider('Capture', () => o.capture, (v) => (o.capture = v), { min: 0, max: 100, step: 0.5, title: 'Particles this close are caught and their paths end' }, e),
        moves(() => !!o.moving, (v) => (o.moving = v), e),
      ];
    case 'seed_point':
      return [emit(() => o.emit_rate, (v) => (o.emit_rate = v))];
    case 'seed_line':
      return [count(() => o.count, (v) => (o.count = v)), emit(() => o.emit_rate, (v) => (o.emit_rate = v))];
    case 'seed_ring':
      return [
        count(() => o.count, (v) => (o.count = v)),
        radius(() => o.radius, (v) => (o.radius = v)),
        emit(() => o.emit_rate, (v) => (o.emit_rate = v)),
      ];
  }
}

function dye(get: () => string | undefined, set: (v: string | undefined) => void, e: Edit): HTMLElement {
  return colour('Dye', get, (v) => {
    if (v === undefined) set(undefined);
    else set(v);
  }, e, 'The colour of the dye it pours; colours mix where they meet');
}

function moves(get: () => boolean, set: (v: boolean) => void, e: Edit): HTMLElement {
  return check('Moves like a particle', get, set, e, 'Carried by the flow and pulled by attractors (moving ones too), leaving no path; it bounces off walls and obstacles, and goes when it leaves through an open edge');
}

function viewSection(host: PanelHost, open: Record<string, boolean>): HTMLElement {
  const d = details('view', 'View', open, true);
  const v = host.view;
  const e = host.viewEdit;
  d.append(
    select<ViewMode>('Field', () => v.mode, (m) => (v.mode = m), [
      ['dye', 'Dye'],
      ['speed', 'Speed'],
      ['vorticity', 'Vorticity'],
      ['pressure', 'Pressure'],
      ['plain', 'Plain'],
    ], e),
    check('Velocity arrows', () => v.arrows, (x) => (v.arrows = x), e),
    check('Recorded paths', () => v.recorded, (x) => (v.recorded = x), e),
    check('Colour paths by time', () => v.rainbow, (x) => (v.rainbow = x), e, 'Each particle draws a rainbow of its own, starting red where it was released, the hue going round as it ages'),
    slider('Rainbow', () => v.rainbowPeriod, (x) => (v.rainbowPeriod = x), { min: 0.5, max: 60, step: 0.5, unit: 's', title: 'Seconds for the hue to go round once' }, e),
    check('Live paths (frozen seeds)', () => v.live, (x) => (v.live = x), e),
    check('Moving particles', () => v.particles, (x) => (v.particles = x), e),
    check('Objects', () => v.objects, (x) => (v.objects = x), e),
  );
  return d;
}

function particleSection(host: PanelHost, open: Record<string, boolean>): HTMLElement {
  const d = details('particles', 'Particles', open, true);
  const p = host.particles;
  const e = host.settingsEdit;
  d.append(
    slider('Inertia', () => p.inertia, (v) => (p.inertia = v), { min: 0, max: 3, step: 0.01, unit: 's', title: 'How long a particle takes to catch up with the fluid: 0 follows it exactly' }, e),
    slider('Point gap', () => p.step, (v) => (p.step = v), { min: 0.5, max: 20, step: 0.1, title: 'Distance between a path’s points' }, e),
    slider('Max length', () => p.max_length, (v) => (p.max_length = v), { min: 100, max: 50000, step: 100 }, e),
    select('At obstacles', () => p.at_obstacle, (v) => (p.at_obstacle = v), [
      ['stop', 'Stop'],
      ['bounce', 'Bounce / slide'],
    ], e),
    check('Trace seeds back upstream too', () => p.backward, (x) => (p.backward = x), e, 'Frozen, weightless seeds trace the whole streamline through them'),
    check('Close loops', () => p.close_loops, (x) => (p.close_loops = x), e, 'A path that comes back to its start closes and ends'),
    numberBox('Max particles', () => p.max_particles, (v) => (p.max_particles = Math.round(v)), { min: 1, max: 50000, step: 100 }, e),
  );
  return d;
}

function flowSection(host: PanelHost, open: Record<string, boolean>): HTMLElement {
  const d = details('flow', 'Flow', open, true);
  const f = host.flow;
  const e = host.settingsEdit;
  const presets = el('div', 'buttons');
  const tunnel = el('button', undefined, 'Wind tunnel');
  tunnel.type = 'button';
  tunnel.addEventListener('click', () => host.preset('tunnel'));
  const tank = el('button', undefined, 'Closed tank');
  tank.type = 'button';
  tank.addEventListener('click', () => host.preset('tank'));
  presets.append(tunnel, tank);
  const edge = (name: string, key: keyof FlowSettings['edges']) =>
    select<Edge>(name, () => f.edges[key], (v) => (f.edges[key] = v), EDGES, e);
  d.append(
    presets,
    slider('Inflow', () => f.inflow_speed, (v) => (f.inflow_speed = v), { min: 0, max: 500, step: 1, title: 'Speed across inflow edges' }, e),
    slider('Swirl keep', () => f.vorticity, (v) => (f.vorticity = v), { min: 0, max: 2, step: 0.01, title: 'Vorticity confinement: keeps swirls from blurring away' }, e),
    slider('Viscosity', () => f.viscosity, (v) => (f.viscosity = v), { min: 0, max: 300, step: 1, title: 'Thicker fluid, smoother flow' }, e),
    slider('Sway', () => f.perturbation, (v) => (f.perturbation = v), { min: 0, max: 0.3, step: 0.005, title: 'How much the inflow wavers, so wakes start shedding' }, e),
    slider('Time', () => f.speed, (v) => (f.speed = v), { min: 0.1, max: 4, step: 0.05, unit: '×', title: 'Simulated seconds per second' }, e),
    slider('Pressure', () => f.pressure_iterations, (v) => (f.pressure_iterations = Math.round(v)), { min: 5, max: 200, step: 1, title: 'Solver iterations a step while running (freezing solves fully)' }, e),
    select('Dye', () => f.dye, (v) => (f.dye = v), [
      ['stripes', 'Stripes from inflow'],
      ['none', 'Sources and stirring only'],
    ], e),
    slider('Dye fade', () => f.dye_fade, (v) => (f.dye_fade = v), { min: 0, max: 1, step: 0.01, unit: '/s' }, e),
    el('div', 'sub', 'Edges'),
    edge('Left', 'left'),
    edge('Right', 'right'),
    edge('Top', 'top'),
    edge('Bottom', 'bottom'),
    el('div', 'sub', 'Domain'),
    select('Resolution', () => f.resolution, (v) => (f.resolution = v), [
      [64, '64 across (fastest)'],
      [96, '96 across'],
      [128, '128 across (fast)'],
      [192, '192 across'],
      [256, '256 across'],
      [384, '384 across'],
      [512, '512 across (slow)'],
    ], e, 'Cells across; changing it restarts the fluid'),
    numberBox('Width', () => f.width, (v) => (f.width = v), { min: 100, max: 20000, step: 10, title: 'In the SVG’s units; changing it restarts the fluid' }, e),
    numberBox('Height', () => f.height, (v) => (f.height = v), { min: 100, max: 20000, step: 10 }, e),
  );
  return d;
}
