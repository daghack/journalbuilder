//! A scene: the flow's settings, the particles' settings, the objects, and
//! the paths recorded so far. It is what the page saves and loads, as JSON;
//! every field has a default, so a partial scene loads too.

use crate::geom::V2;
use crate::objects::Object;
use serde::{Deserialize, Serialize};

pub const SCENE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// Nothing crosses it; the fluid slides along it.
    Wall,
    /// Fluid comes in across it at the inflow speed.
    Inflow,
    /// Fluid leaves (or enters) freely.
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edges {
    pub left: Edge,
    pub right: Edge,
    pub top: Edge,
    pub bottom: Edge,
}

impl Default for Edges {
    /// A closed tank.
    fn default() -> Self {
        Edges::tank()
    }
}

impl Edges {
    /// Walls all round: only stirrers and stirring move the fluid.
    pub fn tank() -> Self {
        Edges { left: Edge::Wall, right: Edge::Wall, top: Edge::Wall, bottom: Edge::Wall }
    }

    /// A wind tunnel: in on the left, out on the right.
    pub fn tunnel() -> Self {
        Edges { left: Edge::Inflow, right: Edge::Open, top: Edge::Wall, bottom: Edge::Wall }
    }

    pub fn any_open(&self) -> bool {
        [self.left, self.right, self.top, self.bottom].contains(&Edge::Open)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DyeInflow {
    /// Bands of dye come in with the inflow, to show the flow.
    Stripes,
    /// Only sources and stirring add dye.
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlowSettings {
    /// The domain's size, in its own units (the SVG's too).
    pub width: f32,
    pub height: f32,
    /// Cells across the width.
    pub resolution: u32,
    /// Kinematic viscosity, in units² a second (0: only the solver's own).
    pub viscosity: f32,
    /// Vorticity confinement: keeps swirls from blurring away.
    pub vorticity: f32,
    pub inflow_speed: f32,
    pub edges: Edges,
    /// Pressure iterations a step while running. Freezing solves fully.
    pub pressure_iterations: u32,
    /// How much the inflow sways, as a share of its speed, so wakes start
    /// shedding.
    pub perturbation: f32,
    pub dye: DyeInflow,
    /// How fast dye fades, a share a second.
    pub dye_fade: f32,
    /// Simulated seconds per second shown.
    pub speed: f32,
}

impl Default for FlowSettings {
    fn default() -> Self {
        FlowSettings {
            width: 1600.0,
            height: 900.0,
            resolution: 256,
            viscosity: 0.0,
            vorticity: 0.25,
            inflow_speed: 120.0,
            edges: Edges::default(),
            pressure_iterations: 40,
            perturbation: 0.03,
            dye: DyeInflow::None,
            dye_fade: 0.05,
            speed: 1.0,
        }
    }
}

impl FlowSettings {
    /// The grid this needs: cells across and down, and their size.
    pub fn grid_size(&self) -> (usize, usize, f32) {
        let width = self.width.max(1.0);
        let height = self.height.max(1.0);
        let nx = (self.resolution.clamp(16, 1024)) as usize;
        let h = width / nx as f32;
        let ny = ((height / h).round() as usize).max(4);
        (nx, ny, h)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtObstacle {
    /// The path ends where it meets an obstacle.
    Stop,
    /// Heavy particles bounce off; weightless ones slide round.
    Bounce,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleSettings {
    /// Seconds a particle takes to catch up with the fluid: 0 follows it
    /// exactly, more overshoots and swings wide.
    pub inertia: f32,
    /// How far apart a path's points are, in domain units.
    pub step: f32,
    /// The longest a path gets, in domain units.
    pub max_length: f32,
    /// Seeds in a frozen flow trace back upstream too, giving the whole
    /// streamline through them (weightless particles only).
    pub backward: bool,
    pub at_obstacle: AtObstacle,
    /// A path that comes back round to where it started is closed and ends.
    pub close_loops: bool,
    /// The most particles moving at once.
    pub max_particles: u32,
}

impl Default for ParticleSettings {
    fn default() -> Self {
        ParticleSettings {
            inertia: 0.0,
            step: 3.0,
            max_length: 8000.0,
            backward: false,
            at_obstacle: AtObstacle::Stop,
            close_loops: true,
            max_particles: 4000,
        }
    }
}

/// A path kept: points in domain units, whether it closes, and how old
/// its particle was at each point (seconds since it was released), if it
/// was a particle's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedPath {
    #[serde(serialize_with = "rounded_points")]
    pub points: Vec<V2>,
    #[serde(default)]
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "rounded_times")]
    pub times: Vec<f32>,
}

fn rounded_times<S: serde::Serializer>(times: &[f32], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(times.len()))?;
    for t in times {
        seq.serialize_element(&((t * 100.0).round() / 100.0))?;
    }
    seq.end()
}

fn known_objects<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Object>, D::Error> {
    let values: Vec<serde_json::Value> = Deserialize::deserialize(d)?;
    Ok(values.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
}

fn rounded_points<S: serde::Serializer>(points: &[V2], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(points.len()))?;
    for p in points {
        seq.serialize_element(&p.rounded())?;
    }
    seq.end()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scene {
    pub version: u32,
    pub flow: FlowSettings,
    pub particles: ParticleSettings,
    /// Objects of kinds this doesn't know are left out.
    #[serde(deserialize_with = "known_objects")]
    pub objects: Vec<Object>,
    pub paths: Vec<RecordedPath>,
}

impl Default for Scene {
    /// An empty tank of still fluid.
    fn default() -> Self {
        Scene {
            version: SCENE_VERSION,
            flow: FlowSettings::default(),
            particles: ParticleSettings::default(),
            objects: Vec::new(),
            paths: Vec::new(),
        }
    }
}

/// The settings alone, as the page sends them when they change.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub flow: FlowSettings,
    pub particles: ParticleSettings,
}
