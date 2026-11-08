//! The flow simulator's engine: the solver keeps the flow divergence-free
//! and out of obstacles, tracers follow known flows, and what it writes
//! reads back.

use flowsim::objects::{Attractors, Falloff, Kind, Object, Obstacles};
use flowsim::scene::{AtObstacle, Edge, Edges, FlowSettings, ParticleSettings, RecordedPath, Scene, Settings};
use flowsim::svg::{SvgOptions, path_data, simplify};
use flowsim::tracer::{End, Env, rotation, uniform};
use flowsim::{Sim, V2, v2};

/// A small, quick wind tunnel.
fn small_flow() -> FlowSettings {
    FlowSettings { resolution: 80, width: 800.0, height: 400.0, edges: Edges::tunnel(), ..FlowSettings::default() }
}

/// A wind tunnel with a pebble in it and a rake of 24 seeds upstream, on a
/// coarse grid.
fn pebble_and_rake() -> Scene {
    let rake = Object { id: 2, kind: Kind::SeedLine { a: v2(80.0, 240.0), b: v2(80.0, 660.0), count: 24, emit_rate: 0.0 } };
    Scene {
        flow: FlowSettings { resolution: 96, edges: Edges::tunnel(), ..FlowSettings::default() },
        objects: vec![pebble(1, 420.0, 450.0, 60.0), rake],
        ..Scene::default()
    }
}

fn scene_with(objects: Vec<Object>, flow: FlowSettings) -> Scene {
    Scene { flow, objects, ..Scene::default() }
}

fn pebble(id: u32, x: f32, y: f32, r: f32) -> Object {
    Object { id, kind: Kind::Pebble { pos: v2(x, y), radius: r } }
}

/// A small closed tank of still fluid.
fn tank() -> FlowSettings {
    FlowSettings { edges: Edges::tank(), perturbation: 0.0, ..small_flow() }
}

fn moving_attractor(id: u32, x: f32, y: f32, strength: f32) -> Object {
    Object {
        id,
        kind: Kind::Attractor {
            pos: v2(x, y),
            strength,
            radius: 80.0,
            falloff: Falloff::InverseSquare,
            spin: 0.0,
            capture: 0.0,
            moving: true,
        },
    }
}

fn position(sim: &Sim, id: u32) -> V2 {
    sim.objects().iter().find(|o| o.id == id).and_then(|o| o.kind.pos()).unwrap()
}

#[test]
fn freezing_leaves_the_flow_divergence_free() {
    let mut sim = Sim::new(scene_with(vec![pebble(1, 200.0, 200.0, 40.0)], small_flow()));
    for _ in 0..40 {
        sim.tick(1.0 / 60.0);
    }
    let running = sim.fluid().divergence_error();
    sim.freeze();
    let frozen = sim.stats().divergence;
    assert!(frozen < 1e-2, "divergence {frozen} after freezing (it was {running} running)");
    assert!(frozen <= running, "freezing made it worse: {frozen} from {running}");
}

#[test]
fn obstacles_carry_no_flow_across_their_faces() {
    let objects = vec![
        pebble(1, 200.0, 200.0, 40.0),
        Object { id: 2, kind: Kind::Rect { pos: v2(450.0, 150.0), size: v2(80.0, 30.0), angle: 30.0 } },
        Object { id: 3, kind: Kind::Wall { points: vec![v2(600.0, 250.0), v2(700.0, 320.0)], thickness: 4.0 } },
    ];
    let mut sim = Sim::new(scene_with(objects, small_flow()));
    for _ in 0..30 {
        sim.tick(1.0 / 60.0);
    }
    sim.freeze();
    let g = &sim.fluid().g;
    let solid_cells = g.solid.iter().filter(|s| **s).count();
    assert!(solid_cells > 50, "only {solid_cells} solid cells");
    for j in 0..g.ny {
        for i in 0..=g.nx {
            if g.u_solid(i, j) {
                assert_eq!(g.u[g.ui(i, j)], 0.0, "u face {i},{j}");
            }
        }
    }
    for j in 0..=g.ny {
        for i in 0..g.nx {
            if g.v_solid(i, j) {
                assert_eq!(g.v[g.vi(i, j)], 0.0, "v face {i},{j}");
            }
        }
    }
    // A wall thinner than a cell still blocks.
    let wall_cells = (0..g.nx * g.ny)
        .filter(|&c| {
            let (i, j) = (c % g.nx, c / g.nx);
            g.solid[c] && (i as f32 + 0.5) * g.h > 590.0 && (j as f32 + 0.5) * g.h > 240.0
        })
        .count();
    assert!(wall_cells >= 10, "the thin wall covers {wall_cells} cells");
}

#[test]
fn an_empty_channel_stays_uniform() {
    let flow = FlowSettings { perturbation: 0.0, vorticity: 0.0, ..small_flow() };
    let speed = flow.inflow_speed;
    let mut sim = Sim::new(scene_with(Vec::new(), flow));
    for _ in 0..90 {
        sim.tick(1.0 / 60.0);
    }
    let g = &sim.fluid().g;
    for (k, u) in g.u.iter().enumerate() {
        assert!((u - speed).abs() < speed * 0.01, "u[{k}] = {u}");
    }
    for (k, v) in g.v.iter().enumerate() {
        assert!(v.abs() < speed * 0.01, "v[{k}] = {v}");
    }
}

#[test]
fn a_tracer_in_a_rotation_closes_its_circle() {
    let centre = v2(500.0, 500.0);
    let flow = rotation(centre, 0.5);
    let (obstacles, attractors) = (Obstacles::default(), Attractors::default());
    let env = Env::new(&flow, &obstacles, &attractors, 1000.0, 1000.0);
    let s = ParticleSettings { step: 2.0, ..ParticleSettings::default() };
    let path = env.trace(centre + v2(100.0, 0.0), &s, false);
    assert_eq!(path.end, End::Closed);
    assert!(path.closed);
    let around = path.length();
    assert!((around - std::f32::consts::TAU * 100.0).abs() < 8.0, "went {around} round");
    for p in &path.points {
        assert!((p.dist(centre) - 100.0).abs() < 0.5, "strayed to {}", p.dist(centre));
    }
    // Clockwise on screen: from the right of the centre, it heads down.
    assert!(path.points[1].y > path.points[0].y);
}

#[test]
fn an_attractor_captures_a_particle() {
    let flow = uniform(V2::default());
    let objects = vec![Object {
        id: 1,
        kind: Kind::Attractor {
            pos: v2(500.0, 500.0),
            strength: 60.0,
            radius: 100.0,
            falloff: Falloff::InverseSquare,
            spin: 10.0,
            capture: 6.0,
            moving: false,
        },
    }];
    let (obstacles, attractors) = (Obstacles::new(&objects), Attractors::new(&objects));
    let env = Env::new(&flow, &obstacles, &attractors, 1000.0, 1000.0);
    for inertia in [0.0, 0.5] {
        let s = ParticleSettings { inertia, ..ParticleSettings::default() };
        let path = env.trace(v2(300.0, 500.0), &s, false);
        assert_eq!(path.end, End::Captured, "inertia {inertia}");
        let last = *path.points.last().unwrap();
        assert!(last.dist(v2(500.0, 500.0)) <= 6.0 + 1e-3, "ended {} away", last.dist(v2(500.0, 500.0)));
    }
}

#[test]
fn particles_stop_at_or_bounce_off_obstacles() {
    let flow = uniform(v2(100.0, 0.0));
    let objects = vec![pebble(1, 500.0, 500.0, 50.0)];
    let (obstacles, attractors) = (Obstacles::new(&objects), Attractors::new(&objects));
    let env = Env::new(&flow, &obstacles, &attractors, 1000.0, 1000.0);
    let stop = ParticleSettings { at_obstacle: AtObstacle::Stop, ..ParticleSettings::default() };
    let path = env.trace(v2(100.0, 510.0), &stop, false);
    assert_eq!(path.end, End::HitObstacle);
    assert!(obstacles.sdf(*path.points.last().unwrap()).abs() < 0.1);

    let bounce = ParticleSettings { at_obstacle: AtObstacle::Bounce, inertia: 0.3, ..ParticleSettings::default() };
    let path = env.trace(v2(100.0, 510.0), &bounce, false);
    assert_eq!(path.end, End::LeftDomain);
    assert!(path.points.iter().all(|p| obstacles.sdf(*p) > -0.5));
}

#[test]
fn frozen_seeds_trace_live_paths() {
    let mut sim = Sim::new(pebble_and_rake());
    for _ in 0..20 {
        sim.tick(1.0 / 60.0);
    }
    assert!(sim.live().is_empty());
    sim.freeze();
    sim.tick(1.0 / 60.0);
    assert_eq!(sim.live().len(), 24, "one path a seed of the default rake");
    // Moving the pebble while frozen re-solves round it and retraces.
    let version = sim.live_version;
    let mut objects = sim.objects().to_vec();
    if let Kind::Pebble { pos, .. } = &mut objects[0].kind {
        pos.y += 30.0;
    }
    sim.set_objects(objects);
    sim.tick(1.0 / 60.0);
    assert!(sim.live_version > version);
    assert!(sim.stats().divergence < 1e-2);
    assert_eq!(sim.bake(), 24);
    assert_eq!(sim.recorded_paths().offsets.len(), 25);
    // Clearing clears the live paths too, until something changes; undo
    // by snapshot brings them all back.
    let snap = sim.snapshot_paths();
    sim.clear_paths();
    assert_eq!(sim.stats().recorded_paths, 0);
    assert!(sim.live().is_empty(), "the live paths are cleared too");
    sim.tick(1.0 / 60.0);
    assert!(sim.live().is_empty(), "and stay cleared");
    sim.restore_paths(snap);
    sim.tick(1.0 / 60.0);
    assert_eq!(sim.stats().recorded_paths, 24);
    assert_eq!(sim.live().len(), 24);
}

#[test]
fn moving_attractors_pull_each_other_together() {
    let objects = vec![moving_attractor(1, 300.0, 200.0, 60.0), moving_attractor(2, 500.0, 200.0, 60.0)];
    let mut sim = Sim::new(scene_with(objects, tank()));
    let version = sim.objects_version;
    for _ in 0..60 {
        sim.tick(1.0 / 60.0);
    }
    let gap = position(&sim, 1).dist(position(&sim, 2));
    assert!(gap < 190.0, "still {gap} apart");
    assert!(sim.objects_version > version);
    assert_eq!(sim.mover_count(), 2);
    // Held, one stays put while the other comes on.
    sim.hold(Some(1));
    let (held, other) = (position(&sim, 1), position(&sim, 2));
    for _ in 0..30 {
        sim.tick(1.0 / 60.0);
    }
    assert_eq!(position(&sim, 1), held);
    assert!(position(&sim, 2).dist(held) < other.dist(held));
}

#[test]
fn a_moving_source_rides_the_flow_and_leaves_no_path() {
    let source = Object { id: 1, kind: Kind::Source { pos: v2(150.0, 200.0), speed: 30.0, radius: 20.0, color: None, moving: true } };
    let mut sim = Sim::new(scene_with(vec![source], FlowSettings { perturbation: 0.0, ..small_flow() }));
    for _ in 0..1200 {
        sim.tick(1.0 / 60.0);
        if sim.mover_count() == 0 {
            break;
        }
    }
    assert_eq!(sim.mover_count(), 0, "it left through the open edge");
    assert!(sim.objects().is_empty());
    assert_eq!(sim.stats().recorded_paths, 0, "movers leave no paths");
}

#[test]
fn moving_sources_and_sinks_keep_their_radii_apart() {
    let source = |id, x, y, speed, moving| Object { id, kind: Kind::Source { pos: v2(x, y), speed, radius: 20.0, color: None, moving } };
    let objects = vec![
        // A moving sink draws a moving source in...
        source(1, 170.0, 200.0, 40.0, true),
        source(2, 230.0, 200.0, -80.0, true),
        // ...and a fixed sink draws in a moving source.
        source(3, 600.0, 140.0, -80.0, false),
        source(4, 600.0, 200.0, 40.0, true),
    ];
    let mut sim = Sim::new(scene_with(objects, tank()));
    let (mut closest_pair, mut closest_fixed) = (f32::INFINITY, f32::INFINITY);
    for _ in 0..300 {
        sim.tick(1.0 / 60.0);
        closest_pair = closest_pair.min(position(&sim, 1).dist(position(&sim, 2)));
        closest_fixed = closest_fixed.min(position(&sim, 3).dist(position(&sim, 4)));
    }
    assert!(closest_pair < 55.0 && closest_fixed < 55.0, "they never came together: {closest_pair}, {closest_fixed}");
    assert!(closest_pair >= 40.0 - 0.01, "the moving pair came {closest_pair} apart");
    assert!(closest_fixed >= 40.0 - 0.01, "the source came {closest_fixed} from the fixed sink");
    assert_eq!(position(&sim, 3), v2(600.0, 140.0), "the fixed sink stays put");
}

#[test]
fn dropped_particles_leave_recorded_paths() {
    let mut sim = Sim::new(scene_with(Vec::new(), FlowSettings { perturbation: 0.0, ..small_flow() }));
    for _ in 0..30 {
        sim.tick(1.0 / 60.0);
    }
    assert!(sim.drop_particle(v2(100.0, 200.0)));
    for _ in 0..600 {
        sim.tick(1.0 / 60.0);
        if sim.particle_count() == 0 {
            break;
        }
    }
    assert_eq!(sim.particle_count(), 0, "the particle should have left the tunnel");
    let paths = sim.recorded_paths();
    assert_eq!(paths.offsets.len(), 2);
    let last_x = paths.points[paths.points.len() - 2];
    assert!((last_x - 800.0).abs() < 1.0, "ended at x {last_x}");
}

#[test]
fn seeds_emit_while_running() {
    let objects =
        vec![Object { id: 1, kind: Kind::SeedLine { a: v2(50.0, 100.0), b: v2(50.0, 300.0), count: 5, emit_rate: 2.0 } }];
    let mut sim = Sim::new(scene_with(objects, small_flow()));
    for _ in 0..60 {
        sim.tick(1.0 / 60.0);
    }
    assert!(sim.particle_count() >= 5, "{} particles", sim.particle_count());
}

#[test]
fn closed_tanks_and_sources_stay_bounded() {
    let mut flow = small_flow();
    flow.edges.left = Edge::Wall;
    flow.edges.right = Edge::Wall;
    let objects = vec![
        Object { id: 1, kind: Kind::Source { pos: v2(200.0, 200.0), speed: 60.0, radius: 20.0, color: None, moving: false } },
        Object { id: 2, kind: Kind::Source { pos: v2(600.0, 200.0), speed: -60.0, radius: 20.0, color: None, moving: false } },
        Object { id: 3, kind: Kind::Vortex { pos: v2(400.0, 200.0), spin: 80.0, radius: 50.0 } },
        Object { id: 4, kind: Kind::Jet { pos: v2(400.0, 100.0), angle: 90.0, speed: 100.0, radius: 20.0 } },
    ];
    let mut sim = Sim::new(scene_with(objects, flow));
    for _ in 0..200 {
        sim.tick(1.0 / 60.0);
    }
    let fastest = sim.fluid().g.max_speed();
    assert!(fastest.is_finite() && fastest < 1000.0, "fastest {fastest}");
    sim.freeze();
    assert!(sim.stats().divergence < 1e-2, "divergence {}", sim.stats().divergence);
}

#[test]
fn simplifying_stays_within_its_tolerance() {
    let points: Vec<V2> = (0..400).map(|i| v2(i as f32, (i as f32 * 0.05).sin() * 40.0)).collect();
    let tolerance = 0.5;
    let kept = simplify(&points, tolerance);
    assert!(kept.len() < points.len() / 4, "kept {} of {}", kept.len(), points.len());
    assert_eq!(kept.first(), points.first());
    assert_eq!(kept.last(), points.last());
    for p in &points {
        let d = kept.windows(2).map(|w| flowsim::geom::segment_distance(*p, w[0], w[1])).fold(f32::INFINITY, f32::min);
        assert!(d <= tolerance + 1e-4, "{p:?} is {d} away");
    }
}

#[test]
fn svg_path_data_is_what_journals_read() {
    let mut sim = Sim::new(pebble_and_rake());
    for _ in 0..20 {
        sim.tick(1.0 / 60.0);
    }
    sim.freeze();
    sim.tick(1.0 / 60.0);
    for smooth in [true, false] {
        let options = SvgOptions { smooth, obstacles: true, seeds: true, ..SvgOptions::default() };
        let svg = sim.svg(&options);
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"297mm\""));
        let paths: Vec<&str> = svg.split(" d=\"").skip(1).map(|rest| &rest[..rest.find('"').unwrap()]).collect();
        assert!(paths.len() > 20, "{} paths", paths.len());
        for d in paths {
            assert!(d.chars().all(|c| "MLCZ -.0123456789".contains(c)), "unexpected command in {d}");
            assert!(smooth || !d.contains('C'));
            let parsed: Result<scribe::journal::paths::StrokePath, String> = d.parse();
            assert!(parsed.is_ok(), "journals can't read {d}: {:?}", parsed.err());
        }
    }
    let closed = path_data(&[v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0), v2(0.0, 0.0)], true, 0.0, true);
    assert!(closed.ends_with(" Z"));
    assert_eq!(closed.matches(" C ").count(), 3);
}

#[test]
fn scenes_survive_a_json_round_trip() {
    let objects = vec![
        pebble(1, 10.0, 20.0, 5.0),
        Object { id: 2, kind: Kind::Rect { pos: v2(1.0, 2.0), size: v2(3.0, 4.0), angle: 15.0 } },
        Object { id: 3, kind: Kind::Wall { points: vec![v2(1.0, 1.0), v2(2.5, 3.25)], thickness: 6.0 } },
        Object { id: 4, kind: Kind::Jet { pos: v2(5.0, 5.0), angle: -45.0, speed: 100.0, radius: 20.0 } },
        Object { id: 5, kind: Kind::Vortex { pos: v2(5.0, 5.0), spin: -50.0, radius: 20.0 } },
        Object { id: 6, kind: Kind::Source { pos: v2(5.0, 5.0), speed: -10.0, radius: 20.0, color: None, moving: false } },
        Object {
            id: 7,
            kind: Kind::Attractor {
                pos: v2(5.0, 5.0),
                strength: -40.0,
                radius: 20.0,
                falloff: Falloff::Gaussian,
                spin: 3.0,
                capture: 2.0,
                moving: false,
            },
        },
        Object { id: 8, kind: Kind::SeedPoint { pos: v2(1.0, 1.0), emit_rate: 0.5 } },
        Object { id: 9, kind: Kind::SeedLine { a: v2(1.0, 1.0), b: v2(2.0, 2.0), count: 3, emit_rate: 0.0 } },
        Object { id: 10, kind: Kind::SeedRing { pos: v2(1.0, 1.0), radius: 9.0, count: 7, emit_rate: 1.0 } },
    ];
    let scene = Scene {
        objects,
        paths: vec![RecordedPath { points: vec![v2(1.25, 2.5), v2(3.0, 4.75)], closed: true, times: Vec::new() }],
        ..Scene::default()
    };
    let json = Sim::new(scene.clone()).scene_json();
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);

    // The page writes objects like this, and a partial scene fills in the rest.
    let partial: Scene = serde_json::from_str(
        r#"{"objects":[{"id":1,"kind":"seed_line","a":[0,0],"b":[10,10],"count":4}],"flow":{"resolution":128}}"#,
    )
    .unwrap();
    assert_eq!(partial.flow.resolution, 128);
    assert_eq!(partial.flow.width, 1600.0);
    assert!(matches!(partial.objects[0].kind, Kind::SeedLine { count: 4, emit_rate: 0.0, .. }));
    let settings: Settings = serde_json::from_str(r#"{"particles":{"inertia":0.2}}"#).unwrap();
    assert_eq!(settings.particles.inertia, 0.2);
    // A new scene is a blank slate: an empty tank.
    let blank = Scene::default();
    assert!(blank.objects.is_empty() && blank.paths.is_empty());
    assert_eq!(blank.flow.edges, Edges::tank());
}

/// A disc 60 across at (400, 200), drawn as a mask of 4-unit cells over
/// the 800 × 400 tunnel.
fn disc_mask() -> flowsim::objects::Mask {
    let (nx, ny, cell) = (200, 100, 4.0);
    let alpha: Vec<u8> = (0..ny)
        .flat_map(|j| (0..nx).map(move |i| (i, j)))
        .map(|(i, j)| {
            let c = v2((i as f32 + 0.5) * cell, (j as f32 + 0.5) * cell);
            if c.dist(v2(400.0, 200.0)) < 30.0 { 255 } else { 0 }
        })
        .collect();
    flowsim::objects::Mask::from_alpha(nx, ny, cell, &alpha, 0.0).expect("the disc is drawn")
}

#[test]
fn a_mask_is_an_obstacle_the_flow_goes_round() {
    let mask = disc_mask();
    assert!(mask.sdf(v2(400.0, 200.0)) < -25.0, "deep inside: {}", mask.sdf(v2(400.0, 200.0)));
    assert!((mask.sdf(v2(460.0, 200.0)) - 30.0).abs() < 3.0, "30 out: {}", mask.sdf(v2(460.0, 200.0)));
    assert!(flowsim::objects::Mask::from_alpha(4, 4, 1.0, &[0; 16], 0.0).is_none(), "nothing solid, no mask");

    let mut sim = Sim::new(scene_with(Vec::new(), small_flow()));
    sim.set_mask(Some(mask));
    sim.freeze();
    let g = &sim.fluid().g;
    let centre = g.ci(40, 20);
    assert!(g.solid[centre], "the disc's cells are solid");
    let still = sim.velocity_at(v2(400.0, 200.0)).len();
    let beside = sim.velocity_at(v2(400.0, 250.0)).len();
    assert!(still < 1e-3, "nothing moves inside: {still}");
    assert!(beside > sim.velocity_at(v2(100.0, 200.0)).len(), "the flow speeds up round it");
    // Particles stop at it too.
    let path = Env::new(&|p| g.velocity_smooth(p), &Obstacles::new(&[]).with_mask(Some(&std::rc::Rc::new(disc_mask())), 0.0), &Attractors::default(), 800.0, 400.0)
        .trace(v2(100.0, 200.0), &ParticleSettings::default(), false);
    assert_eq!(path.end, End::HitObstacle);
    // A new scene keeps it.
    sim.set_scene(scene_with(Vec::new(), small_flow()));
    assert!(sim.has_mask());
}

#[test]
fn a_field_kept_and_loaded_freezes_the_same_flow() {
    let mut sim = Sim::new(pebble_and_rake());
    for _ in 0..40 {
        sim.tick(1.0 / 30.0);
    }
    sim.freeze();
    let kept = sim.field().encode();
    let field = flowsim::field::Field::decode(&kept).unwrap();
    let mut again = Sim::new(pebble_and_rake());
    again.load_field(&field);
    assert!(again.frozen());
    for p in [v2(300.0, 300.0), v2(600.0, 450.0), v2(900.0, 700.0)] {
        let (a, b) = (sim.velocity_at(p), again.velocity_at(p));
        assert!((a - b).len() < 0.02 * sim.fluid().g.max_speed().max(1.0), "{a:?} vs {b:?} at {p:?}");
    }
}
