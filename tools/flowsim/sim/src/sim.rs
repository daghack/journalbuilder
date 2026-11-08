//! The simulator as the page drives it: the scene, the fluid, particles on
//! their way, and the paths they leave.
//!
//! While the flow **runs**, the fluid moves, stirrers push it, and seeds
//! with an emit rate release particles whose trails become recorded paths.
//! **Frozen**, the fluid stands still (solved fully divergence-free), every
//! seed's paths are traced through it at once, and they're retraced
//! whenever anything changes. Obstacles and sources moved while frozen
//! re-solve the frozen flow round them, from the flow as it froze.
//!
//! Moving sources and attractors (movers) move like particles, frozen or
//! running, but leave no paths. A moving source re-solves the frozen flow
//! as it goes.

use crate::geom::V2;
use crate::field::Field;
use crate::objects::{Attractors, Family, Kind, Mask, Object, Obstacles};
use crate::render::{View, render};
use crate::scene::{Edge, FlowSettings, ParticleSettings, RecordedPath, SCENE_VERSION, Scene, Settings};
use crate::solver::{Accuracy, Fluid, Push};
use crate::svg::{Line, SvgOptions, write_svg};
use crate::tracer::{End, Env, Particle, Path, Walls};
use serde::Serialize;
use std::collections::HashMap;
use std::rc::Rc;

/// Paths as flat arrays, for the page: every point's x and y in turn;
/// where each path starts, in points, with the total last; whether each
/// closes; and when each point was reached (NaN where that isn't known).
#[derive(Clone, Debug, Default)]
pub struct Flat {
    pub points: Vec<f32>,
    pub offsets: Vec<u32>,
    pub closed: Vec<u8>,
    pub times: Vec<f32>,
}

impl Flat {
    fn push(&mut self, points: impl Iterator<Item = V2>, times: &[f32], closed: bool) {
        let from = self.points.len() / 2;
        self.offsets.push(from as u32);
        for p in points {
            self.points.push(p.x);
            self.points.push(p.y);
        }
        let n = self.points.len() / 2 - from;
        if times.len() == n {
            self.times.extend_from_slice(times);
        } else {
            self.times.extend(std::iter::repeat_n(f32::NAN, n));
        }
        self.closed.push(closed as u8);
    }
    fn finish(mut self) -> Self {
        self.offsets.push((self.points.len() / 2) as u32);
        self
    }
}

#[derive(Serialize)]
struct SceneRef<'a> {
    version: u32,
    flow: &'a FlowSettings,
    particles: &'a ParticleSettings,
    objects: &'a [Object],
    paths: Vec<&'a RecordedPath>,
}

#[derive(Serialize)]
pub struct Stats {
    pub frozen: bool,
    pub grid: [usize; 2],
    pub particles: usize,
    pub movers: usize,
    pub live_paths: usize,
    pub recorded_paths: usize,
    pub recorded_points: usize,
    /// The largest divergence left in the frozen flow (0 while running).
    pub divergence: f32,
    pub ends: HashMap<&'static str, usize>,
}

/// How many points of trail a particle keeps while paths aren't recorded.
const TAIL_KEPT: usize = 400;

/// A moving source or attractor, and the particle that carries it.
struct Mover {
    id: u32,
    p: Particle,
}

/// The recorded paths at some moment, and whether the live ones had been
/// cleared.
type Snapshot = (Vec<Rc<RecordedPath>>, bool);

pub struct Sim {
    flow: FlowSettings,
    particle_settings: ParticleSettings,
    objects: Vec<Object>,
    paths: Vec<Rc<RecordedPath>>,
    snapshots: HashMap<u32, Snapshot>,
    next_snapshot: u32,
    fluid: Fluid,
    obstacles: Obstacles,
    /// An obstacle the host draws (a page's strokes), not part of the scene.
    mask: Option<Rc<Mask>>,
    attractors: Attractors,
    pushes: Vec<Push>,
    stirs: Vec<Push>,
    frozen: bool,
    /// The flow as it froze, before obstacles moved, and the cells solid
    /// then.
    base: Option<(Vec<f32>, Vec<f32>)>,
    base_solid: Vec<bool>,
    reproject: bool,
    /// Moving sources moved: re-solve the frozen flow roughly, and fully
    /// once they stop (`rough`).
    reproject_rough: bool,
    rough: bool,
    particles: Vec<Particle>,
    movers: Vec<Mover>,
    /// The mover being dragged, which stays put.
    held: Option<u32>,
    live: Vec<Path>,
    live_dirty: bool,
    /// The live paths were cleared, and stay so until something changes.
    live_cleared: bool,
    emit: HashMap<u32, f32>,
    divergence: f32,
    /// The live paths' version when they were last baked: those are
    /// recorded already, so the SVG leaves them out.
    baked_live: u32,
    pub field_version: u32,
    pub paths_version: u32,
    pub live_version: u32,
    /// Bumped when movers move or leave: the objects changed here.
    pub objects_version: u32,
    /// The colour stirring pours, as dye channels; none for plain dye.
    stir_tint: Option<[f32; 4]>,
    /// Whether particles' trails are kept as recorded paths when they end.
    recording: bool,
    rgba: Vec<u8>,
    scratch: Vec<f32>,
}

impl Sim {
    pub fn new(scene: Scene) -> Self {
        let (nx, ny, h) = scene.flow.grid_size();
        let mut sim = Sim {
            flow: scene.flow,
            particle_settings: scene.particles,
            objects: Vec::new(),
            paths: scene.paths.into_iter().map(Rc::new).collect(),
            snapshots: HashMap::new(),
            next_snapshot: 1,
            fluid: Fluid::new(nx, ny, h),
            obstacles: Obstacles::default(),
            mask: None,
            attractors: Attractors::default(),
            pushes: Vec::new(),
            stirs: Vec::new(),
            frozen: false,
            base: None,
            base_solid: Vec::new(),
            reproject: false,
            reproject_rough: false,
            rough: false,
            particles: Vec::new(),
            movers: Vec::new(),
            held: None,
            live: Vec::new(),
            live_dirty: false,
            live_cleared: false,
            emit: HashMap::new(),
            divergence: 0.0,
            baked_live: 0,
            field_version: 1,
            paths_version: 1,
            live_version: 1,
            objects_version: 1,
            stir_tint: None,
            recording: true,
            rgba: Vec::new(),
            scratch: Vec::new(),
        };
        sim.apply_objects(scene.objects, true);
        sim.settle();
        sim
    }

    /// Sets the edges and solves the flow fully: from a still fluid, the
    /// smooth flow the edges and obstacles alone make.
    fn settle(&mut self) {
        self.fluid.apply_boundaries(&self.flow);
        self.fluid.project(&self.flow, true);
    }

    pub fn scene_json(&self) -> String {
        serde_json::to_string(&SceneRef {
            version: SCENE_VERSION,
            flow: &self.flow,
            particles: &self.particle_settings,
            objects: &self.objects,
            paths: self.paths.iter().map(|p| p.as_ref()).collect(),
        })
        .unwrap_or_default()
    }

    /// Replaces everything, and starts the flow afresh, running. The mask,
    /// whether paths are recorded and stirring's colour stay.
    pub fn set_scene(&mut self, scene: Scene) {
        let mask = self.mask.take();
        let (recording, stir_tint) = (self.recording, self.stir_tint);
        *self = Sim::new(scene);
        self.recording = recording;
        self.stir_tint = stir_tint;
        if mask.is_some() {
            self.use_mask(mask);
        }
    }

    /// Makes a host's picture an obstacle (see `Mask::from_alpha`), or
    /// none. Frozen, the flow is solved round it again.
    pub fn set_mask(&mut self, mask: Option<Mask>) {
        self.use_mask(mask.map(Rc::new));
    }

    fn use_mask(&mut self, mask: Option<Rc<Mask>>) {
        self.mask = mask;
        self.obstacles = Obstacles::new(&self.objects).with_mask(self.mask.as_ref(), 0.0);
        self.fluid.set_solids(&self.objects, self.mask.as_ref());
        self.fluid.set_sources(&self.objects);
        if self.frozen {
            self.reproject = true;
        }
        self.field_version += 1;
        self.live_dirty = self.frozen;
    }

    pub fn has_mask(&self) -> bool {
        self.mask.is_some()
    }

    /// The fluid as it is now, frozen or running.
    pub fn field(&self) -> Field {
        Field::from_grid(&self.fluid.g)
    }

    /// Freezes the flow as `field` has it: its velocity, pressure and dye,
    /// read onto this grid, solved divergence-free again round this
    /// scene's obstacles.
    pub fn load_field(&mut self, field: &Field) {
        self.unfreeze();
        let g = &mut self.fluid.g;
        let same = g.nx == field.nx() && g.ny == field.ny() && (g.h - field.h()).abs() <= 1e-4 * g.h;
        if same {
            let f = field.grid();
            g.u.copy_from_slice(&f.u);
            g.v.copy_from_slice(&f.v);
            g.q.copy_from_slice(&f.q);
            g.dye.copy_from_slice(&f.dye);
        } else {
            // Another grid's: sampled where this one's faces and cells are,
            // stretched to fit.
            let scale = crate::geom::v2(field.size().x / g.width(), field.size().y / g.height());
            let at = |x: f32, y: f32| crate::geom::v2(x * scale.x, y * scale.y);
            let h = g.h;
            for j in 0..g.ny {
                for i in 0..=g.nx {
                    let k = g.ui(i, j);
                    g.u[k] = field.velocity(at(i as f32 * h, (j as f32 + 0.5) * h)).x / scale.x;
                }
            }
            for j in 0..=g.ny {
                for i in 0..g.nx {
                    let k = g.vi(i, j);
                    g.v[k] = field.velocity(at((i as f32 + 0.5) * h, j as f32 * h)).y / scale.y;
                }
            }
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let k = g.ci(i, j);
                    let p = at((i as f32 + 0.5) * h, (j as f32 + 0.5) * h);
                    g.q[k] = field.sample(crate::field::Channel::Pressure, p);
                    g.dye[k] = field.sample(crate::field::Channel::Dye, p);
                }
            }
        }
        for (k, solid) in g.solid.iter().enumerate() {
            if *solid {
                g.dye[k] = 0.0;
                g.q[k] = 0.0;
            }
        }
        self.fluid.zero_solid_faces();
        self.fluid.plain_tint();
        self.particles.clear();
        self.freeze();
    }

    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    pub fn set_objects(&mut self, objects: Vec<Object>) {
        self.apply_objects(objects, false);
    }

    /// Takes the objects; `reshape` rasterises obstacles and sources even if
    /// they haven't changed.
    fn apply_objects(&mut self, objects: Vec<Object>, reshape: bool) {
        // What changes the flow: obstacles and sources, less their colours.
        let shaping = |list: &[Object]| -> Vec<Object> {
            list.iter()
                .filter(|o| o.kind.family() == Family::Obstacle || matches!(o.kind, Kind::Source { .. }))
                .cloned()
                .map(|mut o| {
                    if let Kind::Source { color, .. } = &mut o.kind {
                        *color = None;
                    }
                    o
                })
                .collect()
        };
        let colours = |list: &[Object]| -> Vec<(u32, Option<String>)> {
            list.iter().filter_map(|o| o.kind.color().map(|c| (o.id, Some(c.to_string())))).collect()
        };
        let reshaped = reshape || shaping(&objects) != shaping(&self.objects);
        let recoloured = colours(&objects) != colours(&self.objects);
        self.objects = objects;
        self.obstacles = Obstacles::new(&self.objects).with_mask(self.mask.as_ref(), 0.0);
        self.attractors = Attractors::new(&self.objects);
        self.pushes = Push::from_objects(&self.objects);
        if reshaped {
            self.fluid.set_solids(&self.objects, self.mask.as_ref());
            self.fluid.set_sources(&self.objects);
            if self.frozen {
                self.reproject = true;
            }
            self.field_version += 1;
        } else if recoloured {
            self.fluid.set_sources(&self.objects);
        }
        let ids: Vec<u32> = self.objects.iter().map(|o| o.id).collect();
        self.emit.retain(|id, _| ids.contains(id));
        self.live_dirty = self.frozen;
        self.sync_movers();
    }

    /// Matches the movers to the moving objects. A new one starts where
    /// it's put, and one moved by hand starts afresh, still, where it's put.
    fn sync_movers(&mut self) {
        let mut old = std::mem::take(&mut self.movers);
        let inertia = self.particle_settings.inertia;
        for o in &self.objects {
            let (true, Some(pos)) = (o.kind.moving(), o.kind.pos()) else { continue };
            match old.iter().position(|m| m.id == o.id).map(|i| old.swap_remove(i)) {
                Some(m) if m.p.pos.dist(pos) <= 1e-3 => self.movers.push(m),
                _ => {
                    let vel = self.fluid.g.velocity(pos);
                    self.movers.push(Mover { id: o.id, p: Particle::mover(pos, vel, inertia) });
                }
            }
        }
    }

    /// Keeps a trail as a recorded path, if it's long enough to draw and
    /// paths are being recorded.
    fn keep_trail(&mut self, points: Vec<V2>, times: Vec<f32>, closed: bool) {
        if self.recording && points.len() >= 2 && crate::svg::length(&points) >= 2.0 * self.particle_settings.step {
            let times = if times.len() == points.len() { times } else { Vec::new() };
            self.paths.push(Rc::new(RecordedPath { points, closed, times }));
            self.paths_version += 1;
        }
    }

    /// Whether particles' trails become recorded paths when they end. Off,
    /// particles keep only their last stretch of trail, to draw.
    pub fn set_recording(&mut self, on: bool) {
        self.recording = on;
        if !on {
            for p in &mut self.particles {
                p.trim_trail(TAIL_KEPT);
            }
        }
    }

    pub fn recording(&self) -> bool {
        self.recording
    }

    /// Holds a mover still (while it's dragged), or none.
    pub fn hold(&mut self, id: Option<u32>) {
        self.held = id;
    }

    pub fn settings(&self) -> Settings {
        Settings { flow: self.flow.clone(), particles: self.particle_settings.clone() }
    }

    pub fn set_settings(&mut self, s: Settings) {
        let regrid = s.flow.grid_size() != self.flow.grid_size()
            || s.flow.width != self.flow.width
            || s.flow.height != self.flow.height;
        let reflow = s.flow.edges != self.flow.edges || s.flow.inflow_speed != self.flow.inflow_speed;
        self.flow = s.flow;
        self.particle_settings = s.particles;
        for m in &mut self.movers {
            m.p.inertia = self.particle_settings.inertia.max(0.0);
        }
        if regrid {
            let (nx, ny, h) = self.flow.grid_size();
            self.fluid = Fluid::new(nx, ny, h);
            self.fluid.set_solids(&self.objects, self.mask.as_ref());
            self.fluid.set_sources(&self.objects);
            self.settle();
            self.particles.clear();
            if self.frozen {
                let g = &self.fluid.g;
                self.base = Some((g.u.clone(), g.v.clone()));
                self.base_solid = g.solid.clone();
                self.reproject = true;
            }
            self.field_version += 1;
        } else if reflow && self.frozen {
            self.reproject = true;
        }
        self.live_dirty = self.frozen;
    }

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    /// Stops the flow, solved fully divergence-free.
    pub fn freeze(&mut self) {
        if self.frozen {
            return;
        }
        self.fluid.apply_boundaries(&self.flow);
        self.fluid.project(&self.flow, true);
        let g = &self.fluid.g;
        self.base = Some((g.u.clone(), g.v.clone()));
        self.base_solid = g.solid.clone();
        self.frozen = true;
        self.divergence = self.fluid.divergence_error();
        self.field_version += 1;
        self.live_dirty = true;
    }

    pub fn unfreeze(&mut self) {
        if !self.frozen {
            return;
        }
        self.frozen = false;
        self.base = None;
        self.reproject = false;
        self.reproject_rough = false;
        self.rough = false;
        self.live.clear();
        self.live_dirty = false;
        self.live_version += 1;
        self.divergence = 0.0;
    }

    /// Runs the flow one step, then freezes it again if it was frozen.
    pub fn step_once(&mut self, dt: f32) {
        let was = self.frozen;
        if was {
            // From the flow as it froze, without moved obstacles' fix-ups.
            if let Some((u, v)) = self.base.take() {
                self.fluid.g.u = u;
                self.fluid.g.v = v;
                self.fluid.zero_solid_faces();
            }
            self.frozen = false;
        }
        self.run_fluid(dt);
        if was {
            self.freeze();
        }
    }

    /// Runs the fluid `dt` seconds on, stirring and all.
    fn run_fluid(&mut self, dt: f32) {
        let mut pushes = self.pushes.clone();
        pushes.append(&mut self.stirs);
        self.fluid.step(dt, &self.flow, &pushes);
        self.field_version += 1;
    }

    /// Stills the fluid. Frozen, this leaves the flow the edges and
    /// obstacles alone make: smooth (potential) flow round everything.
    pub fn reset_fluid(&mut self) {
        self.fluid.reset();
        self.settle();
        self.particles.clear();
        if self.frozen {
            let g = &self.fluid.g;
            self.base = Some((g.u.clone(), g.v.clone()));
            self.base_solid = g.solid.clone();
            self.reproject = true;
        }
        self.field_version += 1;
    }

    /// Pushes the fluid at `pos` toward `velocity`, at the next tick. Frozen,
    /// this reshapes the frozen flow.
    pub fn stir(&mut self, pos: V2, velocity: V2, radius: f32) {
        self.stirs.push(Push::stir(pos, velocity, radius, self.stir_tint));
    }

    /// The colour stirring pours (`#rrggbb`), or none for plain dye.
    pub fn set_stir_color(&mut self, color: Option<&str>) {
        self.stir_tint = color.and_then(crate::colour::parse_hex).map(crate::colour::dye_channels);
    }

    /// Releases a particle at `pos`, if it isn't in an obstacle or out of
    /// the domain.
    pub fn drop_particle(&mut self, pos: V2) -> bool {
        let g = &self.fluid.g;
        if self.particles.len() >= self.particle_settings.max_particles as usize
            || !(0.0..=g.width()).contains(&pos.x)
            || !(0.0..=g.height()).contains(&pos.y)
            || self.obstacles.sdf(pos) < 0.0
        {
            return false;
        }
        let vel = g.velocity(pos) + self.attractors.drift(pos);
        self.particles.push(Particle::new(pos, vel, self.particle_settings.inertia));
        true
    }

    pub fn clear_particles(&mut self) {
        self.particles.clear();
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    pub fn mover_count(&self) -> usize {
        self.movers.len()
    }

    /// Each mover's id and where it is, in turn.
    pub fn movers(&self) -> Vec<f32> {
        self.movers.iter().flat_map(|m| [m.id as f32, m.p.pos.x, m.p.pos.y]).collect()
    }

    /// Moves everything `dt` seconds on.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        if self.frozen {
            if !self.stirs.is_empty() {
                if let Some((mut u, mut v)) = self.base.take() {
                    std::mem::swap(&mut self.fluid.g.u, &mut u);
                    std::mem::swap(&mut self.fluid.g.v, &mut v);
                    for s in self.stirs.drain(..) {
                        self.fluid.push(&s, dt);
                    }
                    std::mem::swap(&mut self.fluid.g.u, &mut u);
                    std::mem::swap(&mut self.fluid.g.v, &mut v);
                    self.base = Some((u, v));
                    self.reproject = true;
                }
                self.stirs.clear();
            }
            if self.reproject || (self.rough && !self.reproject_rough) {
                self.reproject_frozen(Accuracy::Full);
            } else if self.reproject_rough {
                self.reproject_frozen(Accuracy::Rough);
            }
            if self.live_dirty {
                self.retrace();
            }
        } else if dt > 0.0 {
            self.run_fluid(dt);
            self.emit_from_seeds(dt);
        }
        self.stirs.clear();
        if dt > 0.0 {
            self.advance_particles(dt);
        }
    }

    fn reproject_frozen(&mut self, accuracy: Accuracy) {
        self.reproject = false;
        self.reproject_rough = false;
        self.rough = accuracy != Accuracy::Full;
        if let Some((u, v)) = &self.base {
            self.fluid.g.u.clone_from(u);
            self.fluid.g.v.clone_from(v);
            self.fluid.fill_uncovered(&self.base_solid);
        }
        self.fluid.apply_boundaries(&self.flow);
        self.fluid.project_to(&self.flow, accuracy);
        self.divergence = self.fluid.divergence_error();
        self.field_version += 1;
        self.live_dirty = true;
    }

    fn retrace(&mut self) {
        self.live_dirty = false;
        self.live_cleared = false;
        let g = &self.fluid.g;
        let flow = |p: V2| g.velocity_smooth(p);
        let env = Env::new(&flow, &self.obstacles, &self.attractors, g.width(), g.height());
        let s = &self.particle_settings;
        self.live.clear();
        for o in &self.objects {
            for p in o.kind.seed_points() {
                let path = if s.backward { env.trace_both_ways(p, s) } else { env.trace(p, s, false) };
                if path.points.len() >= 2 {
                    self.live.push(path);
                }
            }
        }
        self.live_version += 1;
    }

    fn emit_from_seeds(&mut self, dt: f32) {
        let mut starts = Vec::new();
        for o in &self.objects {
            let rate = o.kind.emit_rate();
            if rate <= 0.0 {
                continue;
            }
            let acc = self.emit.entry(o.id).or_insert(0.0);
            *acc += rate * dt;
            let mut n = 0;
            while *acc >= 1.0 && n < 8 {
                *acc -= 1.0;
                n += 1;
                starts.extend(o.kind.seed_points());
            }
            *acc = acc.min(1.0);
        }
        for p in starts {
            self.drop_particle(p);
        }
    }

    fn walls(&self) -> Walls {
        let e = &self.flow.edges;
        Walls {
            left: e.left == Edge::Wall,
            right: e.right == Edge::Wall,
            top: e.top == Edge::Wall,
            bottom: e.bottom == Edge::Wall,
        }
    }

    fn advance_particles(&mut self, dt: f32) {
        if self.particles.is_empty() && self.movers.is_empty() {
            return;
        }
        let s = self.particle_settings.clone();
        let frozen = self.frozen;
        let walls = self.walls();
        let held = self.held;
        let mut finished = Vec::new();
        let mut gone = Vec::new();
        {
            let g = &self.fluid.g;
            let flow = |p: V2| if frozen { g.velocity_smooth(p) } else { g.velocity(p) };
            let mut env = Env { walls, ..Env::new(&flow, &self.obstacles, &self.attractors, g.width(), g.height()) };
            for m in &mut self.movers {
                if held == Some(m.id) {
                    continue;
                }
                env.skip = Some(m.id);
                if m.p.advance(&env, &s, dt).is_some() {
                    gone.push(m.id);
                }
            }
            env.skip = None;
            let recording = self.recording;
            self.particles.retain_mut(|p| match p.advance(&env, &s, dt) {
                Some(end) => {
                    finished.push((std::mem::take(&mut p.trail), std::mem::take(&mut p.times), end == End::Closed));
                    false
                }
                None => {
                    if !recording {
                        p.trim_trail(TAIL_KEPT);
                    }
                    true
                }
            });
        }
        if !self.movers.is_empty() || !gone.is_empty() {
            self.separate_sources(&gone);
            self.place_movers(&gone);
        }
        for (points, times, closed) in finished {
            self.keep_trail(points, times, closed);
        }
    }

    /// Keeps moving sources and sinks at least their two radii apart, from
    /// each other and from fixed ones: an overlapping pair is pushed apart
    /// along the line between them (a fixed or held one not at all), and
    /// loses the speed it had toward each other.
    fn separate_sources(&mut self, gone: &[u32]) {
        let radius = |id: u32| {
            self.objects.iter().find(|o| o.id == id).and_then(|o| match o.kind {
                Kind::Source { radius, moving: true, .. } => Some(radius.max(0.0)),
                _ => None,
            })
        };
        // Each moving source's mover, radius, and whether it can be pushed.
        let moving: Vec<(usize, f32, bool)> = self
            .movers
            .iter()
            .enumerate()
            .filter(|(_, m)| !gone.contains(&m.id))
            .filter_map(|(i, m)| radius(m.id).map(|r| (i, r, self.held != Some(m.id))))
            .collect();
        if moving.is_empty() {
            return;
        }
        let fixed: Vec<(V2, f32)> = self
            .objects
            .iter()
            .filter_map(|o| match o.kind {
                Kind::Source { pos, radius, moving: false, .. } => Some((pos, radius.max(0.0))),
                _ => None,
            })
            .collect();
        let (w, h) = (self.fluid.g.width(), self.fluid.g.height());
        let movers = &mut self.movers;
        // Pushing one pair apart can push another together: a few rounds.
        for _ in 0..8 {
            let mut overlapped = false;
            for a in 0..moving.len() {
                let (ia, ra, free_a) = moving[a];
                if free_a {
                    for &(c, rc) in &fixed {
                        let m = &mut movers[ia].p;
                        if let Some(n) = apart(c, m.pos, rc + ra) {
                            overlapped = true;
                            m.pos = c + n * (rc + ra);
                            m.vel -= n * m.vel.dot(n).min(0.0);
                        }
                    }
                }
                for &(ib, rb, free_b) in &moving[a + 1..] {
                    let (pa, pb) = (movers[ia].p.pos, movers[ib].p.pos);
                    let Some(n) = apart(pa, pb, ra + rb) else { continue };
                    let shares = (free_a as u8 as f32, free_b as u8 as f32);
                    let total = shares.0 + shares.1;
                    if total == 0.0 {
                        continue;
                    }
                    overlapped = true;
                    let overlap = ra + rb - pa.dist(pb);
                    let (sa, sb) = (shares.0 / total, shares.1 / total);
                    let closing = (movers[ib].p.vel - movers[ia].p.vel).dot(n).min(0.0);
                    movers[ia].p.pos = pa - n * (overlap * sa);
                    movers[ib].p.pos = pb + n * (overlap * sb);
                    movers[ia].p.vel += n * (closing * sa);
                    movers[ib].p.vel -= n * (closing * sb);
                }
            }
            if !overlapped {
                break;
            }
        }
        // Back inside the domain and out of obstacles, where pushing put them.
        for &(i, _, free) in &moving {
            if !free {
                continue;
            }
            let m = &mut movers[i].p;
            m.pos.x = m.pos.x.clamp(0.0, w);
            m.pos.y = m.pos.y.clamp(0.0, h);
            let d = self.obstacles.sdf(m.pos);
            if d < 0.0 {
                m.pos -= self.obstacles.normal(m.pos) * (d - 0.05);
            }
        }
    }

    /// Moves the moving objects to their movers, drops those that left, and
    /// updates what they act through.
    fn place_movers(&mut self, gone: &[u32]) {
        self.movers.retain(|m| !gone.contains(&m.id));
        let (mut sources, mut attractors) = (false, false);
        let mut changed = |kind: &Kind| match kind {
            Kind::Source { .. } => sources = true,
            _ => attractors = true,
        };
        for o in &mut self.objects {
            if !o.kind.moving() {
                continue;
            }
            if gone.contains(&o.id) {
                changed(&o.kind);
            } else if let Some(m) = self.movers.iter().find(|m| m.id == o.id)
                && o.kind.pos() != Some(m.p.pos)
            {
                o.kind.set_pos(m.p.pos);
                changed(&o.kind);
            }
        }
        if gone.is_empty() && !sources && !attractors {
            return;
        }
        self.objects.retain(|o| !gone.contains(&o.id));
        self.objects_version += 1;
        if attractors {
            self.attractors = Attractors::new(&self.objects);
            if self.frozen {
                self.live_dirty = true;
            }
        }
        if sources {
            self.fluid.set_sources(&self.objects);
            if self.frozen {
                self.reproject_rough = true;
            }
        }
    }

    /// Keeps the paths the seeds trace now. Gives how many.
    pub fn bake(&mut self) -> usize {
        let n = self.live.len();
        self.paths
            .extend(self.live.iter().map(|p| Rc::new(RecordedPath { points: p.points.clone(), closed: p.closed, times: Vec::new() })));
        if n > 0 {
            self.paths_version += 1;
            self.baked_live = self.live_version;
        }
        n
    }

    /// Clears every path: the recorded ones, the trails of particles on
    /// their way (which go on, starting new ones), and the live ones, until
    /// something changes and the seeds trace again.
    pub fn clear_paths(&mut self) {
        self.paths.clear();
        self.paths_version += 1;
        for p in &mut self.particles {
            p.restart_trail();
        }
        self.live.clear();
        self.live_cleared = true;
        self.live_dirty = false;
        self.live_version += 1;
    }

    pub fn delete_path(&mut self, index: usize) {
        if index < self.paths.len() {
            self.paths.remove(index);
            self.paths_version += 1;
        }
    }

    /// Remembers the recorded paths as they are, cheaply, for undo.
    pub fn snapshot_paths(&mut self) -> u32 {
        let id = self.next_snapshot;
        self.next_snapshot += 1;
        self.snapshots.insert(id, (self.paths.clone(), self.live_cleared));
        id
    }

    pub fn restore_paths(&mut self, id: u32) {
        let Some((paths, cleared)) = self.snapshots.get(&id) else { return };
        self.paths = paths.clone();
        self.paths_version += 1;
        if *cleared && !self.live_cleared {
            self.live.clear();
            self.live_cleared = true;
            self.live_version += 1;
        } else if !*cleared && self.live_cleared {
            self.live_dirty = self.frozen;
            self.live_cleared = false;
        }
    }

    pub fn forget_paths(&mut self, id: u32) {
        self.snapshots.remove(&id);
    }

    pub fn live_paths(&self) -> Flat {
        let mut f = Flat::default();
        for p in &self.live {
            f.push(p.points.iter().copied(), &[], p.closed);
        }
        f.finish()
    }

    pub fn recorded_paths(&self) -> Flat {
        let mut f = Flat::default();
        for p in &self.paths {
            f.push(p.points.iter().copied(), &p.times, p.closed);
        }
        f.finish()
    }

    /// The moving particles' trails, the last `tail` points of each, ending
    /// where each is now.
    pub fn particle_trails(&self, tail: usize) -> Flat {
        let mut f = Flat::default();
        for p in &self.particles {
            let all: Vec<V2> = p.trail_with_head().collect();
            let times: Vec<f32> = p.times_with_head().collect();
            let from = all.len().saturating_sub(tail.max(2));
            let times = if times.len() == all.len() { &times[from..] } else { &[] };
            f.push(all[from..].iter().copied(), times, false);
        }
        f.finish()
    }

    pub fn particle_positions(&self) -> Vec<f32> {
        self.particles.iter().flat_map(|p| [p.pos.x, p.pos.y]).collect()
    }

    pub fn grid_size(&self) -> (usize, usize) {
        (self.fluid.g.nx, self.fluid.g.ny)
    }

    /// The velocity at the centres of a `cols` × `rows` lattice, x and y in
    /// turn, row by row.
    pub fn velocities(&self, cols: usize, rows: usize) -> Vec<f32> {
        let g = &self.fluid.g;
        let (w, h) = (g.width(), g.height());
        let mut out = Vec::with_capacity(cols * rows * 2);
        for j in 0..rows {
            for i in 0..cols {
                let p = V2 { x: (i as f32 + 0.5) * w / cols as f32, y: (j as f32 + 0.5) * h / rows as f32 };
                let v = if self.obstacles.sdf(p) < 0.0 { V2::default() } else { g.velocity(p) };
                out.push(v.x);
                out.push(v.y);
            }
        }
        out
    }

    pub fn velocity_at(&self, p: V2) -> V2 {
        self.fluid.g.velocity_smooth(p)
    }

    /// The field as RGBA, a pixel a cell.
    pub fn render(&mut self, view: View) -> &[u8] {
        let reference = self.pushes.iter().map(|p| p.speed()).fold(self.flow.inflow_speed.abs(), f32::max).max(40.0);
        render(&self.fluid.g, self.fluid.tint(), view, reference, &mut self.scratch, &mut self.rgba);
        &self.rgba
    }

    pub fn stats(&self) -> Stats {
        let mut ends = HashMap::new();
        for p in &self.live {
            let name = match p.end {
                End::LeftDomain => "left_domain",
                End::HitObstacle => "hit_obstacle",
                End::Captured => "captured",
                End::Stalled => "stalled",
                End::Closed => "closed",
                End::MaxLength => "max_length",
            };
            *ends.entry(name).or_insert(0) += 1;
        }
        Stats {
            frozen: self.frozen,
            grid: [self.fluid.g.nx, self.fluid.g.ny],
            particles: self.particles.len(),
            movers: self.movers.len(),
            live_paths: self.live.len(),
            recorded_paths: self.paths.len(),
            recorded_points: self.paths.iter().map(|p| p.points.len()).sum(),
            divergence: self.divergence,
            ends,
        }
    }

    pub fn svg(&self, o: &SvgOptions) -> String {
        let mut lines = Vec::new();
        if o.recorded {
            lines.extend(self.paths.iter().map(|p| Line { points: &p.points, closed: p.closed }));
        }
        if o.live && !(o.recorded && self.baked_live == self.live_version) {
            lines.extend(self.live.iter().map(|p| Line { points: &p.points, closed: p.closed }));
        }
        write_svg(self.flow.width, self.flow.height, &self.objects, &lines, o)
    }

    /// For tests: the fluid as it is.
    pub fn fluid(&self) -> &Fluid {
        &self.fluid
    }

    pub fn live(&self) -> &[Path] {
        &self.live
    }
}

/// The direction from `a` to `b` when they're closer than `least`; when
/// they're at the same point, any direction.
fn apart(a: V2, b: V2, least: f32) -> Option<V2> {
    let d = b - a;
    let gap = d.len();
    if gap >= least - 1e-4 {
        return None;
    }
    Some(if gap > 1e-6 { d / gap } else { V2 { x: 1.0, y: 0.0 } })
}
