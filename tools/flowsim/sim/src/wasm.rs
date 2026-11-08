//! The engine for the page: one `FlowSim`, driven a frame at a time.
//! Scenes, objects, settings and SVG options go in and out as JSON; fields
//! and paths come out as typed arrays.

use crate::field::{EncodedField, Field};
use crate::geom::v2;
use crate::objects::Mask;
use crate::objects::Object;
use crate::render::View;
use crate::scene::{Scene, Settings};
use crate::sim::{Flat, Sim};
use crate::svg::SvgOptions;
use wasm_bindgen::prelude::*;

fn parse<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> Result<T, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&format!("bad {what}: {e}")))
}

/// Paths as flat arrays: `points` is x, y, x, y, ...; path i's points are
/// `offsets[i]` to `offsets[i + 1]`; `closed[i]` is 1 if it closes.
#[wasm_bindgen]
pub struct PathSet {
    flat: Flat,
}

#[wasm_bindgen]
impl PathSet {
    #[wasm_bindgen(getter)]
    pub fn points(&self) -> Vec<f32> {
        self.flat.points.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn offsets(&self) -> Vec<u32> {
        self.flat.offsets.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn closed(&self) -> Vec<u8> {
        self.flat.closed.clone()
    }
}

/// The fluid at one moment, raw: `u` on the (nx+1) × ny vertical faces,
/// `v` on the nx × (ny+1) horizontal ones, `pressure` and `dye` per cell,
/// cells `h` across.
#[wasm_bindgen]
pub struct FieldSnapshot {
    field: Field,
}

#[wasm_bindgen]
impl FieldSnapshot {
    #[wasm_bindgen(getter)]
    pub fn nx(&self) -> u32 {
        self.field.nx() as u32
    }
    #[wasm_bindgen(getter)]
    pub fn ny(&self) -> u32 {
        self.field.ny() as u32
    }
    #[wasm_bindgen(getter)]
    pub fn h(&self) -> f32 {
        self.field.h()
    }
    #[wasm_bindgen(getter)]
    pub fn u(&self) -> Vec<f32> {
        self.field.grid().u.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn v(&self) -> Vec<f32> {
        self.field.grid().v.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn pressure(&self) -> Vec<f32> {
        self.field.grid().q.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn dye(&self) -> Vec<f32> {
        self.field.grid().dye.clone()
    }
}

#[wasm_bindgen]
pub struct FlowSim {
    sim: Sim,
}

#[wasm_bindgen]
impl FlowSim {
    /// A simulator for this scene (JSON); an empty string gives the default
    /// scene.
    #[wasm_bindgen(constructor)]
    pub fn new(scene_json: &str) -> Result<FlowSim, JsError> {
        console_error_panic_hook::set_once();
        let scene: Scene = if scene_json.trim().is_empty() { Scene::default() } else { parse(scene_json, "scene")? };
        Ok(FlowSim { sim: Sim::new(scene) })
    }

    pub fn default_scene() -> String {
        serde_json::to_string(&Scene::default()).unwrap_or_default()
    }

    pub fn default_svg_options() -> String {
        serde_json::to_string(&SvgOptions::default()).unwrap_or_default()
    }

    /// Replaces everything, and restarts the flow, running.
    pub fn set_scene(&mut self, json: &str) -> Result<(), JsError> {
        self.sim.set_scene(parse(json, "scene")?);
        Ok(())
    }

    pub fn scene(&self) -> String {
        self.sim.scene_json()
    }

    pub fn set_objects(&mut self, json: &str) -> Result<(), JsError> {
        let objects: Vec<Object> = parse(json, "objects")?;
        self.sim.set_objects(objects);
        Ok(())
    }

    /// `{flow, particles}`.
    pub fn set_settings(&mut self, json: &str) -> Result<(), JsError> {
        let settings: Settings = parse(json, "settings")?;
        self.sim.set_settings(settings);
        Ok(())
    }

    pub fn tick(&mut self, dt: f32) {
        self.sim.tick(dt);
    }

    pub fn step_once(&mut self, dt: f32) {
        self.sim.step_once(dt);
    }

    pub fn freeze(&mut self) {
        self.sim.freeze();
    }

    pub fn unfreeze(&mut self) {
        self.sim.unfreeze();
    }

    pub fn frozen(&self) -> bool {
        self.sim.frozen()
    }

    pub fn reset_fluid(&mut self) {
        self.sim.reset_fluid();
    }

    pub fn stir(&mut self, x: f32, y: f32, vx: f32, vy: f32, radius: f32) {
        self.sim.stir(v2(x, y), v2(vx, vy), radius);
    }

    pub fn drop_particle(&mut self, x: f32, y: f32) -> bool {
        self.sim.drop_particle(v2(x, y))
    }

    pub fn clear_particles(&mut self) {
        self.sim.clear_particles();
    }

    pub fn particle_count(&self) -> u32 {
        self.sim.particle_count() as u32
    }

    /// How many moving sources and attractors there are.
    pub fn mover_count(&self) -> u32 {
        self.sim.mover_count() as u32
    }

    /// Each moving source and attractor's id, x and y, in turn.
    pub fn movers(&self) -> Vec<f32> {
        self.sim.movers()
    }

    /// Holds the mover with this id still while it's dragged; none with
    /// `undefined`.
    pub fn hold(&mut self, id: Option<u32>) {
        self.sim.hold(id);
    }

    pub fn particle_positions(&self) -> Vec<f32> {
        self.sim.particle_positions()
    }

    pub fn particle_trails(&self, tail: u32) -> PathSet {
        PathSet { flat: self.sim.particle_trails(tail as usize) }
    }

    pub fn live_paths(&self) -> PathSet {
        PathSet { flat: self.sim.live_paths() }
    }

    pub fn recorded_paths(&self) -> PathSet {
        PathSet { flat: self.sim.recorded_paths() }
    }

    pub fn bake(&mut self) -> u32 {
        self.sim.bake() as u32
    }

    pub fn clear_paths(&mut self) {
        self.sim.clear_paths();
    }

    pub fn delete_path(&mut self, index: u32) {
        self.sim.delete_path(index as usize);
    }

    pub fn snapshot_paths(&mut self) -> u32 {
        self.sim.snapshot_paths()
    }

    pub fn restore_paths(&mut self, id: u32) {
        self.sim.restore_paths(id);
    }

    pub fn forget_paths(&mut self, id: u32) {
        self.sim.forget_paths(id);
    }

    /// The field as RGBA, `grid_width` × `grid_height`: `dye`, `speed`,
    /// `vorticity`, `pressure` or `plain`.
    pub fn render(&mut self, view: &str) -> Vec<u8> {
        self.sim.render(View::parse(view)).to_vec()
    }

    pub fn grid_width(&self) -> u32 {
        self.sim.grid_size().0 as u32
    }

    pub fn grid_height(&self) -> u32 {
        self.sim.grid_size().1 as u32
    }

    pub fn velocities(&self, cols: u32, rows: u32) -> Vec<f32> {
        self.sim.velocities(cols as usize, rows as usize)
    }

    pub fn velocity_at(&self, x: f32, y: f32) -> Vec<f32> {
        let v = self.sim.velocity_at(v2(x, y));
        vec![v.x, v.y]
    }

    pub fn field_version(&self) -> u32 {
        self.sim.field_version
    }

    pub fn paths_version(&self) -> u32 {
        self.sim.paths_version
    }

    pub fn live_version(&self) -> u32 {
        self.sim.live_version
    }

    /// Bumped when movers move or leave the domain.
    pub fn objects_version(&self) -> u32 {
        self.sim.objects_version
    }

    pub fn stats(&self) -> String {
        serde_json::to_string(&self.sim.stats()).unwrap_or_default()
    }

    /// The fluid now, raw, for a host to distort with.
    pub fn field_raw(&self) -> FieldSnapshot {
        FieldSnapshot { field: self.sim.field() }
    }

    /// The fluid now, encoded as JSON, to keep (see `field::EncodedField`).
    pub fn field_json(&self) -> String {
        serde_json::to_string(&self.sim.field().encode()).unwrap_or_default()
    }

    /// Freezes the flow as a kept field (`field_json`'s) has it.
    pub fn load_field(&mut self, json: &str) -> Result<(), JsError> {
        let encoded: EncodedField = parse(json, "field")?;
        let field = Field::decode(&encoded).map_err(|e| JsError::new(&e))?;
        self.sim.load_field(&field);
        Ok(())
    }

    /// Makes an alpha raster an obstacle: `nx` × `ny` square cells `cell`
    /// across from the domain's corner, row by row, solid where at least
    /// half opaque, grown by `grow`. Whether any of it is solid.
    pub fn set_mask(&mut self, nx: u32, ny: u32, cell: f32, alpha: &[u8], grow: f32) -> bool {
        let mask = Mask::from_alpha(nx as usize, ny as usize, cell, alpha, grow);
        let any = mask.is_some();
        self.sim.set_mask(mask);
        any
    }

    pub fn clear_mask(&mut self) {
        self.sim.set_mask(None);
    }

    pub fn svg(&self, options_json: &str) -> Result<String, JsError> {
        let options: SvgOptions =
            if options_json.trim().is_empty() { SvgOptions::default() } else { parse(options_json, "SVG options")? };
        Ok(self.sim.svg(&options))
    }
}
