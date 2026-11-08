//! Makes the showcase's fluid, `webapp/journals/showcase/fluids/ripples.json`:
//! a closed tank the showcase page's size, turned by two vortices and a jet
//! and stirred through with dye, run for four seconds and frozen. It runs
//! the same every time, so the file can be made again:
//!
//! ```bash
//! cargo run --release -p flowsim --example showcase_flow
//! ```
//!
//! (or give another path to write to). The Fluid tab opens it to shape it
//! further, from the scene kept in it.

use flowsim::objects::{Kind, Object};
use flowsim::scene::{DyeInflow, Edges, FlowSettings, Scene};
use flowsim::{Sim, v2};
use serde_json::{Value, json};

/// The showcase's pages, in canvas units.
const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 1600.0;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "webapp/journals/showcase/fluids/ripples.json".to_string());
    let objects = vec![
        Object { id: 1, kind: Kind::Vortex { pos: v2(380.0, 520.0), spin: 90.0, radius: 240.0 } },
        Object { id: 2, kind: Kind::Vortex { pos: v2(820.0, 1120.0), spin: -100.0, radius: 280.0 } },
        Object { id: 3, kind: Kind::Jet { pos: v2(140.0, 1420.0), angle: -0.7, speed: 150.0, radius: 40.0 } },
    ];
    let flow = FlowSettings {
        width: WIDTH,
        height: HEIGHT,
        resolution: 128,
        edges: Edges::tank(),
        dye: DyeInflow::None,
        dye_fade: 0.0,
        ..FlowSettings::default()
    };
    let mut sim = Sim::new(Scene { flow, objects, ..Scene::default() });
    let dt = 1.0 / 60.0;
    for frame in 0..240 {
        // Dye stirred in along three strokes over the first second.
        if frame < 60 {
            let t = frame as f32 / 60.0;
            sim.stir(v2(200.0 + 800.0 * t, 300.0), v2(800.0, 0.0), 60.0);
            sim.stir(v2(1000.0 - 700.0 * t, 820.0 + 120.0 * t), v2(-700.0, 120.0), 50.0);
            sim.stir(v2(600.0, 1500.0 - 500.0 * t), v2(0.0, -500.0), 70.0);
        }
        sim.tick(dt);
    }
    sim.freeze();
    let field = serde_json::to_value(sim.field().encode()).expect("a field encodes");
    let scene: Value = serde_json::from_str(&sim.scene_json()).expect("the scene reads back");
    let lines: Vec<(&str, Value)> = vec![
        ("version", json!(1)),
        ("at", json!([0, 0])),
        ("size", json!([WIDTH, HEIGHT])),
        ("obstacles", json!({ "canvases": [], "grow": 2 })),
        ("preview", json!({ "entry": "showcase", "page": 0, "target": null })),
        ("field", field),
        ("scene", scene),
    ];
    // As the Fluid tab writes it: a line for each setting.
    let body: Vec<String> = lines.iter().map(|(key, value)| format!("\t{}: {}", json!(key), value)).collect();
    let text = format!("{{\n{}\n}}\n", body.join(",\n"));
    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("couldn't make {}: {}", dir.display(), e));
    }
    std::fs::write(&out, &text).unwrap_or_else(|e| panic!("couldn't write {}: {}", out, e));
    println!("wrote {} ({} KB)", out, text.len() / 1024);
}
