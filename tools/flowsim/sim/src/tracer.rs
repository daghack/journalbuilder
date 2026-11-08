//! Particles carried by the flow, and the paths they trace.
//!
//! A particle's target velocity is the fluid's plus the attractors' drift.
//! With no inertia it moves at that velocity exactly; with inertia τ it
//! catches up over τ seconds (dv/dt = (target − v)/τ), so it overshoots
//! bends and swings round attractors.
//!
//! A **mover** is a moving source or attractor carried as a particle: it
//! leaves no trail, never ends but by leaving the domain through an edge
//! that isn't a wall, bounces off walls and obstacles, and isn't caught by
//! attractors.

use crate::geom::V2;
use crate::objects::{Attractors, Obstacles};
use crate::scene::{AtObstacle, ParticleSettings};
use serde::Serialize;

/// Below this speed (units a second) a particle counts as stopped.
const STALL_SPEED: f32 = 0.5;
/// Particles stopped this long (seconds) end.
const STALL_TIME: f32 = 1.5;
/// How much of its speed into an obstacle a bouncing particle keeps.
const RESTITUTION: f32 = 0.7;
/// The most steps a single trace takes.
const MAX_STEPS: usize = 200_000;

/// Something that gives a velocity anywhere in the domain.
pub trait Flow {
    fn velocity(&self, p: V2) -> V2;
}

impl<F: Fn(V2) -> V2> Flow for F {
    fn velocity(&self, p: V2) -> V2 {
        self(p)
    }
}

/// Why a path ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    LeftDomain,
    HitObstacle,
    Captured,
    Stalled,
    Closed,
    MaxLength,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub points: Vec<V2>,
    pub closed: bool,
    pub end: End,
}

impl Path {
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|w| w[0].dist(w[1])).sum()
    }
}

/// Which edges of the domain are walls, which movers bounce off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Walls {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

/// What particles move through.
pub struct Env<'a, F: Flow> {
    pub flow: &'a F,
    pub obstacles: &'a Obstacles,
    pub attractors: &'a Attractors,
    pub width: f32,
    pub height: f32,
    /// The attractor that doesn't pull: the mover moving.
    pub skip: Option<u32>,
    pub walls: Walls,
}

impl<'a, F: Flow> Env<'a, F> {
    pub fn new(flow: &'a F, obstacles: &'a Obstacles, attractors: &'a Attractors, width: f32, height: f32) -> Self {
        Env { flow, obstacles, attractors, width, height, skip: None, walls: Walls::default() }
    }

    fn target(&self, p: V2) -> V2 {
        self.flow.velocity(p) + self.attractors.drift_except(p, self.skip)
    }

    /// Back inside off the wall edges `p` has crossed, with the velocity
    /// turned back off them; `None` if it left through an edge that isn't a
    /// wall.
    fn bounce_off_walls(&self, mut p: V2, mut vel: V2) -> Option<(V2, V2)> {
        let inset = 0.01;
        if p.x < 0.0 {
            if !self.walls.left {
                return None;
            }
            p.x = (-p.x).min(self.width).max(inset);
            vel.x = vel.x.abs() * RESTITUTION;
        } else if p.x > self.width {
            if !self.walls.right {
                return None;
            }
            p.x = (2.0 * self.width - p.x).max(0.0).min(self.width - inset);
            vel.x = -vel.x.abs() * RESTITUTION;
        }
        if p.y < 0.0 {
            if !self.walls.top {
                return None;
            }
            p.y = (-p.y).min(self.height).max(inset);
            vel.y = vel.y.abs() * RESTITUTION;
        } else if p.y > self.height {
            if !self.walls.bottom {
                return None;
            }
            p.y = (2.0 * self.height - p.y).max(0.0).min(self.height - inset);
            vel.y = -vel.y.abs() * RESTITUTION;
        }
        Some((p, vel))
    }

    fn inside(&self, p: V2) -> bool {
        p.x >= 0.0 && p.y >= 0.0 && p.x <= self.width && p.y <= self.height
    }

    /// Where the segment from `a` (inside) to `b` (outside) leaves the domain.
    fn exit_point(&self, a: V2, b: V2) -> V2 {
        let d = b - a;
        let mut t = 1.0f32;
        if d.x < 0.0 {
            t = t.min(-a.x / d.x);
        }
        if d.x > 0.0 {
            t = t.min((self.width - a.x) / d.x);
        }
        if d.y < 0.0 {
            t = t.min(-a.y / d.y);
        }
        if d.y > 0.0 {
            t = t.min((self.height - a.y) / d.y);
        }
        a + d * t.clamp(0.0, 1.0)
    }

    /// Where the segment from `a` (outside obstacles) to `b` (inside one)
    /// meets its surface, by bisection.
    fn surface_point(&self, a: V2, b: V2) -> V2 {
        let (mut lo, mut hi) = (a, b);
        for _ in 0..16 {
            let mid = lo.lerp(hi, 0.5);
            if self.obstacles.sdf(mid) < 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        lo
    }

    /// Out of any obstacle `p` is in, to just past its surface.
    fn push_out(&self, p: V2) -> (V2, V2) {
        let d = self.obstacles.sdf(p);
        let n = self.obstacles.normal(p);
        (p - n * (d - 0.05), n)
    }

    /// The path from `start` through a flow that doesn't change, `backward`
    /// against it for weightless particles.
    pub fn trace(&self, start: V2, s: &ParticleSettings, backward: bool) -> Path {
        if s.inertia > 0.0 && !backward {
            self.trace_heavy(start, s)
        } else {
            self.trace_weightless(start, s, if backward { -1.0 } else { 1.0 })
        }
    }

    /// The whole streamline through `start`: back upstream, then down.
    pub fn trace_both_ways(&self, start: V2, s: &ParticleSettings) -> Path {
        if s.inertia > 0.0 {
            return self.trace(start, s, false);
        }
        let ahead = self.trace(start, s, false);
        if ahead.closed {
            return ahead;
        }
        let behind = self.trace(start, s, true);
        let mut points: Vec<V2> = behind.points.into_iter().rev().collect();
        points.pop();
        points.extend(ahead.points);
        Path { points, closed: false, end: ahead.end }
    }

    fn trace_weightless(&self, start: V2, s: &ParticleSettings, sign: f32) -> Path {
        let step = s.step.max(0.05);
        let f = |p: V2| self.target(p) * sign;
        let mut points = vec![start];
        let end = |points: Vec<V2>, end: End| Path { points, closed: false, end };
        if !self.inside(start) {
            return end(points, End::LeftDomain);
        }
        if self.obstacles.sdf(start) < 0.0 {
            return end(points, End::HitObstacle);
        }
        let first = f(start);
        if first.len() < STALL_SPEED {
            return end(points, End::Stalled);
        }
        let start_dir = first.normalized();
        let mut p = start;
        let mut length = 0.0;
        let mut checkpoint = start;
        for n in 1..=MAX_STEPS {
            if length >= s.max_length {
                return end(points, End::MaxLength);
            }
            let k1 = f(p);
            let speed = k1.len();
            if speed < STALL_SPEED {
                return end(points, End::Stalled);
            }
            let dt = step / speed;
            let k2 = f(p + k1 * (0.5 * dt));
            let k3 = f(p + k2 * (0.5 * dt));
            let k4 = f(p + k3 * dt);
            let mut delta = (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0);
            // RK4 can fling a point far where the flow changes fast.
            let dl = delta.len();
            if dl > 2.0 * step {
                delta = delta * (2.0 * step / dl);
            }
            let mut next = p + delta;
            if !next.is_finite() {
                return end(points, End::Stalled);
            }
            if !self.inside(next) {
                points.push(self.exit_point(p, next));
                return end(points, End::LeftDomain);
            }
            if self.obstacles.sdf(next) < 0.0 {
                match s.at_obstacle {
                    AtObstacle::Stop => {
                        points.push(self.surface_point(p, next));
                        return end(points, End::HitObstacle);
                    }
                    AtObstacle::Bounce => {
                        // Weightless: slide round the surface.
                        next = self.push_out(next).0;
                        if self.obstacles.sdf(next) < 0.0 {
                            return end(points, End::HitObstacle);
                        }
                    }
                }
            }
            length += p.dist(next);
            p = next;
            points.push(p);
            if self.attractors.captures(p) {
                return end(points, End::Captured);
            }
            if s.close_loops && length > 8.0 * step && p.dist(start) < step * 1.2 && f(p).normalized().dot(start_dir) > 0.5
            {
                points.push(start);
                return Path { points, closed: true, end: End::Closed };
            }
            if n % 64 == 0 {
                if p.dist(checkpoint) < 2.0 * step {
                    return end(points, End::Stalled);
                }
                checkpoint = p;
            }
        }
        end(points, End::MaxLength)
    }

    fn trace_heavy(&self, start: V2, s: &ParticleSettings) -> Path {
        let mut particle = Particle::new(start, self.target(start), s.inertia);
        let step = s.step.max(0.05);
        for _ in 0..MAX_STEPS {
            let speed = particle.vel.len().max(self.target(particle.pos).len()).max(STALL_SPEED);
            let dt = (step / speed).min(0.25);
            if let Some(end) = particle.advance(self, s, dt) {
                return particle.into_path(end);
            }
        }
        particle.into_path(End::MaxLength)
    }
}

/// A particle on its way, with the trail it has left.
#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: V2,
    pub vel: V2,
    pub inertia: f32,
    pub trail: Vec<V2>,
    /// How old it was at each of its trail's points: seconds since it was
    /// released.
    pub times: Vec<f32>,
    /// Seconds since it was released.
    pub age: f32,
    /// A moving source or attractor, which goes on until it leaves.
    pub mover: bool,
    /// How far it has gone since its trail began.
    length: f32,
    stalled_for: f32,
    start: V2,
    start_dir: V2,
}

impl Particle {
    pub fn new(pos: V2, vel: V2, inertia: f32) -> Self {
        Particle {
            pos,
            vel,
            inertia: inertia.max(0.0),
            trail: vec![pos],
            times: vec![0.0],
            age: 0.0,
            mover: false,
            length: 0.0,
            stalled_for: 0.0,
            start: pos,
            start_dir: vel.normalized(),
        }
    }

    /// A moving source or attractor's particle.
    pub fn mover(pos: V2, vel: V2, inertia: f32) -> Self {
        Particle { mover: true, ..Particle::new(pos, vel, inertia) }
    }

    /// Starts its trail afresh where it is.
    pub fn restart_trail(&mut self) {
        self.length = 0.0;
        self.start = self.pos;
        self.start_dir = self.vel.normalized();
        self.trail = vec![self.pos];
        self.times = vec![self.age];
    }

    /// Keeps only about the last `keep` points of its trail.
    pub fn trim_trail(&mut self, keep: usize) {
        let keep = keep.max(2);
        if self.trail.len() > 2 * keep {
            let from = self.trail.len() - keep;
            self.trail.drain(..from);
            let from = self.times.len().saturating_sub(keep);
            self.times.drain(..from);
        }
    }

    pub fn into_path(self, end: End) -> Path {
        Path { closed: end == End::Closed, points: self.trail, end }
    }

    /// Moves it `dt` seconds on, in steps no longer than a point apart.
    /// Gives why it ended, if it did.
    pub fn advance<F: Flow>(&mut self, env: &Env<F>, s: &ParticleSettings, dt: f32) -> Option<End> {
        let step = s.step.max(0.05);
        let fast = self.vel.len().max(env.target(self.pos).len());
        let n = ((fast * dt / step).ceil() as usize).clamp(1, 64);
        let sub = dt / n as f32;
        for _ in 0..n {
            if let Some(end) = self.substep(env, s, sub) {
                return Some(end);
            }
        }
        None
    }

    fn substep<F: Flow>(&mut self, env: &Env<F>, s: &ParticleSettings, dt: f32) -> Option<End> {
        self.age += dt;
        let step = s.step.max(0.05);
        let p = self.pos;
        let target = |q: V2| env.target(q);
        let mut next = if self.inertia <= 0.0 {
            // The midpoint rule.
            let mid = p + target(p) * (0.5 * dt);
            self.vel = target(mid);
            p + self.vel * dt
        } else {
            // Semi-implicit, so a small inertia can't blow up.
            let k = dt / self.inertia;
            self.vel = (self.vel + target(p) * k) / (1.0 + k);
            p + self.vel * dt
        };
        if !next.is_finite() {
            if self.mover {
                self.vel = V2::default();
                return None;
            }
            return Some(End::Stalled);
        }
        if !env.inside(next) {
            match self.mover.then(|| env.bounce_off_walls(next, self.vel)).flatten() {
                Some((back, vel)) => {
                    next = back;
                    self.vel = vel;
                }
                None => {
                    self.finish_at(env.exit_point(p, next));
                    return Some(End::LeftDomain);
                }
            }
        }
        if env.obstacles.sdf(next) < 0.0 {
            let at_obstacle = if self.mover { AtObstacle::Bounce } else { s.at_obstacle };
            match at_obstacle {
                AtObstacle::Stop => {
                    self.finish_at(env.surface_point(p, next));
                    return Some(End::HitObstacle);
                }
                AtObstacle::Bounce => {
                    let (out, n) = env.push_out(next);
                    next = out;
                    let vn = self.vel.dot(n);
                    if vn < 0.0 {
                        self.vel -= n * (vn * (1.0 + RESTITUTION));
                    }
                    if env.obstacles.sdf(next) < 0.0 {
                        if self.mover {
                            // Wedged: wait for the flow to turn.
                            self.vel = V2::default();
                            return None;
                        }
                        self.finish_at(p);
                        return Some(End::HitObstacle);
                    }
                }
            }
        }
        self.pos = next;
        if self.mover {
            return None;
        }
        self.length += p.dist(next);
        let last = *self.trail.last().unwrap_or(&p);
        if last.dist(next) >= step {
            self.trail.push(next);
            self.times.push(self.age);
        }
        if env.attractors.captures(next) {
            self.finish_at(next);
            return Some(End::Captured);
        }
        if self.vel.len() < STALL_SPEED {
            self.stalled_for += dt;
            if self.stalled_for > STALL_TIME {
                self.finish_at(next);
                return Some(End::Stalled);
            }
        } else {
            self.stalled_for = 0.0;
        }
        if s.close_loops
            && self.length > 8.0 * step
            && next.dist(self.start) < step * 1.2
            && self.vel.normalized().dot(self.start_dir) > 0.5
        {
            self.finish_at(self.start);
            return Some(End::Closed);
        }
        if self.length >= s.max_length {
            self.finish_at(next);
            return Some(End::MaxLength);
        }
        None
    }

    fn finish_at(&mut self, p: V2) {
        if self.trail.last().is_none_or(|l| l.dist(p) > 1e-3) {
            self.trail.push(p);
            self.times.push(self.age);
        }
        self.pos = p;
    }

    /// Where it has been and where it is, for drawing.
    pub fn trail_with_head(&self) -> impl Iterator<Item = V2> + '_ {
        let head = (self.trail.last() != Some(&self.pos)).then_some(self.pos);
        self.trail.iter().copied().chain(head)
    }

    /// When it was at each point `trail_with_head` gives.
    pub fn times_with_head(&self) -> impl Iterator<Item = f32> + '_ {
        let head = (self.trail.last() != Some(&self.pos)).then_some(self.age);
        self.times.iter().copied().chain(head)
    }
}

/// A uniform flow, for tests and empty scenes.
pub fn uniform(velocity: V2) -> impl Fn(V2) -> V2 {
    move |_| velocity
}

/// Solid-body rotation about `centre`, `omega` radians a second clockwise
/// on screen.
pub fn rotation(centre: V2, omega: f32) -> impl Fn(V2) -> V2 {
    move |p| (p - centre).perp() * omega
}

