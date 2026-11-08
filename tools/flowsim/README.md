# Flow paths

A 2-D liquid flow simulator for making paths to draw with. Run the flow,
freeze it, drop particles in and keep the paths they trace, then download
them as an SVG, whose paths use only `M`, `L`, `C` and `Z`, which the
journal's path parser (`StrokePath::from_str`) reads. Journals use it too:
the journal editor's Fluid tab runs it over a page, and the frozen field
distorts what the page draws (see [In journals](#in-journals)).

- `sim/`: the `flowsim` crate, the simulation in Rust, built to wasm for
  the page (with its `bindings` feature; without it, as the journal crate
  depends on it, it's only the simulation and the field format).
- `web/`: the page, in plain TypeScript with Vite.

## Running it

It needs Rust with the `wasm32-unknown-unknown` target, [wasm-pack] and
Node 24 (`web/.nvmrc`).

```bash
cd tools/flowsim/web && npm install
```

```bash
npm run dev
```

Then open http://localhost:5180. `npm run dev` builds the wasm first;
`npm run serve` skips that after changing only the page. `npm run build`
puts a static site in `web/dist`, which any static file server can host
at any path. The repository's `Dockerfile` serves it at `/flow/`, beside
the journal editor.

`cargo test --release -p flowsim` runs the simulation's tests, and
`scripts/smoke.sh` runs them too, and builds the page.

[wasm-pack]: https://rustwasm.github.io/wasm-pack/installer/

## The flow

The whole plane is fluid, incompressible and seen from above. The domain
is 1600 × 900 units by default, and objects, paths and the SVG are in
those units. The grid it is simulated on is a setting of its own (64 to
512 cells across; 64 is the fastest), so changing it moves nothing.

Each edge of the domain is a wall, an inflow or open. A new scene (**New**)
is a blank slate: an empty **Closed tank**, every edge a wall, so only
stirrers and the mouse move the fluid. The **Wind tunnel** preset pours
fluid in on the left, lets it out on the right and has walls top and
bottom.

**Run** steps the flow. **Freeze** stops it, after making it exactly
divergence-free, so paths traced through it don't bunch up. While frozen:

- Seeds trace their paths live, retraced whenever anything changes.
  Moving an obstacle, a source or a sink solves the flow round it again,
  from the field as it was frozen. Jets and vortices only act while the
  flow runs.
- Stirring reshapes the frozen flow.
- **Bake** keeps the seeds' live paths as recorded paths, so you can run
  the flow on, freeze it again and add another layer.

## The objects

| family | objects | what they do |
|---|---|---|
| Obstacles | pebble, block, wall | The fluid flows round them. Particles stop at them, or bounce (a particle setting). |
| Stirrers | jet, vortex, source, sink | Push the fluid: a jet along its direction, a vortex round its centre, a source out of it, a sink into it. A source's flow runs from −500 (it draws in) to 500, and it can pour dye of a colour of its own (**Dye**). |
| Forces | attractor, repeller | Pull or push particles, never the fluid. Each has a strength, radius, falloff (inverse-square or Gaussian), an optional spin that sends particles round it, and a capture radius where paths end. |
| Seeds | seed, rake, ring | Where particles start: a point, a line of them, or a ring. Frozen, they trace live paths; running, they emit particles at their emit rate. |
| Moving | attract, repel, source, sink | Sources, sinks, attractors and repellers that move like particles (or tick **Moves like a particle** on one placed already). |

Moving forces are carried by the flow, frozen or running, and pulled by
attractors, the moving ones included, so moving attractors pull each other
together. They leave no paths of their own, bounce off obstacles and
wall edges, and are gone when they leave through an open or inflow
edge. A moving source or sink re-solves
the frozen flow round it as it goes, and keeps at least its radius plus
the other's from every other source or sink, moving or fixed: a sink can't
swallow a source. Dragging one holds it still.

A path ends when it leaves the domain, hits an obstacle (if particles
stop), is captured, stalls, comes back round to its start (it is then
closed) or reaches its maximum length.

Particles with no inertia follow the fluid exactly. With inertia (a time,
in seconds) they lag behind it, overshoot and spiral. **Trace seeds back
upstream too** traces each seed's whole streamline rather than only what
lies downstream.

## Controls

| | |
|---|---|
| Space | run or freeze |
| V | select: pick an object or a recorded path, drag to move it, drag its handles to shape it |
| S | stir: drag to push the fluid |
| P | particle: click to drop one, drag to drop a trail of them; each leaves a recorded path |
| B | bake the seeds' live paths |
| Delete, Backspace | remove the selection |
| Escape | back to select |
| Ctrl/Cmd+Z, Shift+Ctrl/Cmd+Z, Ctrl+Y | undo, redo |

The other tools are on the left. Obstacles, stirrers, forces, rakes and
rings are dragged out; walls are drawn freehand. The panel on the right
sets the selected object, what's shown (dye, speed, vorticity, pressure,
velocity arrows), the particles and the flow.

Dye with no colour of its own is the dye view's sea-green. Colours poured
by sources, or by the stir brush (its **Dye** row), mix as light does
rather than paint: two meeting keep the hue between them and stay bright,
so red and yellow make orange, not brown.

**Record paths** in the top bar keeps particles' paths as they go. Untick
it to let a run go on without filling memory: particles keep only a short
tail and nothing new is recorded; what's recorded already stays.

**Colour paths by time** (View) draws every particle's path in a rainbow
of its own: red where the particle was released, the hue running round
once every **Rainbow** seconds of its life. Paths recorded before paths
kept their times are drawn plainly.

**Clear paths** clears every path on the stage: the recorded ones, the
trails of particles on their way (which go on, starting new ones), and the
seeds' live paths, until something changes and the seeds trace again.

Undo covers objects, settings and recorded paths, not the fluid itself.
The scene saves itself in the browser as you go. **Save scene** and
**Open** keep it in a JSON file.

## Export

**Export SVG** opens a dialog with a preview. It sets the width (in mm or
px; the height follows the domain's shape), the stroke's colour and
width, how far simplified paths may stray from the traced ones, whether
paths are smoothed into curves, the shortest path kept, whether recorded
and live paths go in, and optional outlines of the objects, each family in
its own group. The paths are in `<g id="paths">`.

## In journals

A journal's `fluids/NAME.json` keeps a frozen field, which scripts read
with `fluid(name)` to distort strokes by it (see the API reference's
Fluids section), and the scene it was made from, for the Fluid tab to
carry on from.

- **The field** (`sim/src/field.rs`) is the grid's velocity, pressure and
  dye, each quantised to 16 bits over its range, deflated and put in
  base64: `{ nx, ny, h, channels, data }`, 150 to 250 KB for a page.
  `Field` decodes it and samples it smoothly (Catmull-Rom) for the
  journal crate, which depends on `flowsim` for this alone. In wasm,
  `field_json()` encodes it, `field_raw()` gives the arrays the Fluid tab
  redraws from live, and `load_field(json)` takes a saved one back,
  frozen.
- **Strokes as obstacles**: `set_mask(nx, ny, cell, alpha, grow)` takes
  an alpha raster over the domain, `cell` units to a pixel, as solid,
  grown by `grow` units, and a frozen flow is worked out again round it.
  It isn't part of the scene.
- **The view** (`web/src/simview.ts`): `FlowSimView` builds the bar, the
  tool rail, the stage, the settings panel and the status line into
  containers its host gives it, with the styles scoped under `.flowsim`
  and its keys listening on the host, not the window. The page here
  (`main.ts`) is one host; the Fluid tab is another, which also lays the
  page beneath the flow (`setBackdrop`), sets the domain to the page's
  box (`setDomain`), and hears of each change to the field
  (`onFieldChange`).

The showcase's fluid, `webapp/journals/showcase/fluids/ripples.json`, is
made by a scene run the same every time, so it can be made again:

```bash
cargo run --release -p flowsim --example showcase_flow
```
