use regex::Regex;
use parry2d::math::{Vec2, Rot2};
use parry2d::query::RayCast;
use std::ops::Add;
use std::cell::Cell;

fn rotate_around_point(point: Vec2, angle: f32) -> Vec2 {
    let rot = Rot2::new(angle);
    rot * point
}

#[derive(Debug, Clone, Copy)]
pub enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    CubicCurveTo(f32, f32, f32, f32, f32, f32),
    QuadraticCurveTo(f32, f32, f32, f32),
    ClosePath,
}

fn roughen_point(point: Vec2, r: f32) -> Vec2 {
    let random_theta = rand::random::<f32>() * std::f32::consts::PI * 2.0;
    let random_magnitude = (rand::random::<f32>() - 0.5) * 2.0 * r;
    point + Vec2::new(random_magnitude * random_theta.cos(), random_magnitude * random_theta.sin())
}

impl PathCmd {
    pub fn ending_point(&self) -> Option<Vec2> {
        match self {
            PathCmd::MoveTo(x, y) => Some(Vec2::new(*x, *y)),
            PathCmd::LineTo(x, y) => Some(Vec2::new(*x, *y)),
            PathCmd::CubicCurveTo(_, _, _, _, x, y) => Some(Vec2::new(*x, *y)),
            PathCmd::QuadraticCurveTo(_, _, x, y) => Some(Vec2::new(*x, *y)),
            PathCmd::ClosePath => None,
        }
    }
    pub fn interpolate(&self, first: Vec2, current: Vec2, t: f32) -> Option<Vec2> {
        match self {
            PathCmd::LineTo(x, y) => {
                let p0 = current;
                let p1 = Vec2::new(*x, *y);
                Some(l_interp(p0, p1, t))
            }
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => {
                let p0 = current;
                let p1 = Vec2::new(*x1, *y1);
                let p2 = Vec2::new(*x2, *y2);
                let p3 = Vec2::new(*x, *y);
                Some(bc_interp(p0, p1, p2, p3, t))
            }
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => {
                let p0 = current;
                let p1 = Vec2::new(*x1, *y1);
                let p2 = Vec2::new(*x, *y);
                Some(bq_interp(p0, p1, p2, t))
            }
            PathCmd::ClosePath => {
                let p0 = current;
                let p1 = first;
                Some(l_interp(p0, p1, t))
            }
            _ => None
        }
    }
    pub fn positions(&self) -> Vec<Vec2> {
        match self {
            PathCmd::MoveTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::LineTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::CubicCurveTo(_, _, _, _, x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::QuadraticCurveTo(_, _, x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::ClosePath => vec![],
        }
    }
    pub fn points_within(&self) -> Vec<Vec2> {
        match self {
            PathCmd::MoveTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::LineTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => vec![Vec2::new(*x1, *y1), Vec2::new(*x2, *y2), Vec2::new(*x, *y)],
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => vec![Vec2::new(*x1, *y1), Vec2::new(*x, *y)],
            PathCmd::ClosePath => vec![],
        }
    }
    pub fn controls_within(&self) -> Vec<Vec2> {
        match self {
            PathCmd::MoveTo(_, _) => vec![],
            PathCmd::LineTo(_, _) => vec![],
            PathCmd::CubicCurveTo(x1, y1, x2, y2, _, _) => vec![Vec2::new(*x1, *y1), Vec2::new(*x2, *y2)],
            PathCmd::QuadraticCurveTo(x1, y1, _, _) => vec![Vec2::new(*x1, *y1)],
            PathCmd::ClosePath => vec![],
        }
    }
    pub fn positions_within(&self) -> Vec<Vec2> {
        let mut pts = self.points_within();
        let mut ctrls = self.controls_within();
        pts.append(&mut ctrls);
        pts
    }
    pub fn to_string(&self) -> String {
        match self {
            PathCmd::MoveTo(x, y) => format!("M {} {}", x, y),
            PathCmd::LineTo(x, y) => format!("L {} {}", x, y),
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => format!("C {} {} {} {} {} {}", x1, y1, x2, y2, x, y),
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => format!("Q {} {} {} {}", x1, y1, x, y),
            PathCmd::ClosePath => "Z".to_string(),
        }
    }
    pub fn to_cubic_curve(&self) -> Self {
        match self {
            PathCmd::MoveTo(x, y) => PathCmd::MoveTo(*x, *y),
            PathCmd::LineTo(x, y) => PathCmd::CubicCurveTo(*x, *y, *x, *y, *x, *y),
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => PathCmd::CubicCurveTo(*x1, *y1, *x2, *y2, *x, *y),
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => PathCmd::CubicCurveTo(*x1, *y1, *x1 + (*x - x1) / 3.0,
                *y1 + (*y - y1) / 3.0,
                *x + (*x - x1) / 3.0,
                *y + (*y - y1) / 3.0),
            PathCmd::ClosePath => PathCmd::ClosePath,
        }
    }
    pub fn pos_roughen_by(&self, max_magnitude: f32) -> Self {
        let new_roughen = || {
            let random_theta = rand::random::<f32>() * std::f32::consts::PI * 2.0;
            let random_magnitude = rand::random::<f32>() * max_magnitude;
            Vec2::new(random_magnitude * random_theta.cos(), random_magnitude * random_theta.sin())
        };
        let n_roughens = |c| { let mut roughens = vec![]; for _ in 0..c { roughens.push(new_roughen()); } roughens };
        let pos_roughen = new_roughen();
        match self {
            PathCmd::MoveTo(x, y) => {
                PathCmd::MoveTo(x + pos_roughen.x, y + pos_roughen.y)
            }
            PathCmd::LineTo(x, y) => {
                PathCmd::LineTo(x + pos_roughen.x, y + pos_roughen.y)
            }
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => {
                let control_roughens = n_roughens(2);
                PathCmd::CubicCurveTo(x1 + control_roughens[0].x, y1 + control_roughens[0].y,
                                      x2 + control_roughens[1].x, y2 + control_roughens[1].y,
                                      x + pos_roughen.x, y + pos_roughen.y)
            }
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => {
                let control_roughen = new_roughen();
                PathCmd::QuadraticCurveTo(x1 + control_roughen.x, y1 + control_roughen.y,
                                          x + pos_roughen.x, y + pos_roughen.y)
            }
            PathCmd::ClosePath => PathCmd::ClosePath,
        }
    }
    pub fn translate_by(&self, v: Vec2) -> Self {
        match self {
            PathCmd::MoveTo(x, y) => PathCmd::MoveTo(x + v.x, y + v.y),
            PathCmd::LineTo(x, y) => PathCmd::LineTo(x + v.x, y + v.y),
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => PathCmd::CubicCurveTo(x1 + v.x, y1 + v.y, x2 + v.x, y2 + v.y, x + v.x, y + v.y),
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => PathCmd::QuadraticCurveTo(x1 + v.x, y1 + v.y, x + v.x, y + v.y),
            PathCmd::ClosePath => PathCmd::ClosePath,
        }
    }
    pub fn scale_by(&self, s: f32) -> Self {
        match self {
            PathCmd::MoveTo(x, y) => PathCmd::MoveTo(x * s, y * s),
            PathCmd::LineTo(x, y) => PathCmd::LineTo(x * s, y * s),
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => PathCmd::CubicCurveTo(x1 * s, y1 * s, x2 * s, y2 * s, x * s, y * s),
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => PathCmd::QuadraticCurveTo(x1 * s, y1 * s, x * s, y * s),
            PathCmd::ClosePath => PathCmd::ClosePath,
        }
    }
    pub fn rotate_around(&self, point: Vec2, angle: f32) -> Self {
        // angle is expressed in radians from the y-axis, with positive angles being clockwise
        match self {
            PathCmd::MoveTo(x, y) => {
                let rotated_point = rotate_around_point((Vec2::new(*x, *y) - point).into(), angle);
                PathCmd::MoveTo(rotated_point.x + point.x, rotated_point.y + point.y)
            }
            PathCmd::LineTo(x, y) => {
                let rotated_point = rotate_around_point((Vec2::new(*x, *y) - point).into(), angle);
                PathCmd::LineTo(rotated_point.x + point.x, rotated_point.y + point.y)
            }
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => {
                let rotated_point1 = rotate_around_point((Vec2::new(*x1, *y1) - point).into(), angle);
                let rotated_point2 = rotate_around_point((Vec2::new(*x2, *y2) - point).into(), angle);
                let rotated_point3 = rotate_around_point((Vec2::new(*x, *y) - point).into(), angle);
                PathCmd::CubicCurveTo(rotated_point1.x + point.x, rotated_point1.y + point.y,
                                      rotated_point2.x + point.x, rotated_point2.y + point.y,
                                      rotated_point3.x + point.x, rotated_point3.y + point.y)
            }
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => {
                let rotated_point1 = rotate_around_point((Vec2::new(*x1, *y1) - point).into(), angle);
                let rotated_point2 = rotate_around_point((Vec2::new(*x, *y) - point).into(), angle);
                PathCmd::QuadraticCurveTo(rotated_point1.x + point.x, rotated_point1.y + point.y,
                                          rotated_point2.x + point.x, rotated_point2.y + point.y)
            }
            PathCmd::ClosePath => PathCmd::ClosePath,
        }
    }
    pub fn points(&self) -> Vec<Vec2> {
        match self {
            PathCmd::MoveTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::LineTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => vec![Vec2::new(*x1, *y1), Vec2::new(*x2, *y2), Vec2::new(*x, *y)],
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => vec![Vec2::new(*x1, *y1), Vec2::new(*x, *y)],
            PathCmd::ClosePath => vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrokePath {
    commands: Vec<PathCmd>,
}

impl StrokePath {
    pub fn build_kdtree(&self, avoid_scale: f32, expected_stroke_width: Option<f32>) -> kdtree::KdTree<f32, (f32, f32), [f32; 2]> {
        let mut min_vec = Vec2::new(0.0, 0.0);
        min_vec.y = min_vec.y + avoid_scale / 2.0;
        let sw_vec = match expected_stroke_width {
            Some(sw) => Vec2::new(sw, sw),
            None => Vec2::new(0.0, 0.0),
        };
        let mut avoid_tree: kdtree::KdTree<f32, (f32, f32), [f32; 2]> = kdtree::KdTree::new(2);
        let sample_points =
            self.sample_points_along(avoid_scale * 1.0)
            .into_iter().map(|sample| sample.point - min_vec);
        for point in sample_points {
            let lhpoint = point - sw_vec;
            avoid_tree.add([lhpoint.x, lhpoint.y], (lhpoint.x, lhpoint.y)).unwrap();
            let rhpoint = point + sw_vec;
            avoid_tree.add([rhpoint.x, rhpoint.y], (rhpoint.x, rhpoint.y)).unwrap();
        }
        avoid_tree
    }
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        for cmd in &self.commands {
            result.push_str(&cmd.to_string());
            result.push(' ');
        }
        result
    }
    pub fn from_str(stroke_str: &str) -> Self {
        let mut path = StrokePath::new();
        let re = Regex::new(r"([MLCQZ])\s*([^MLCQZ]*)").unwrap();
        for cap in re.captures_iter(stroke_str) {
            let cmd = &cap[1];
            let args = &cap[2];
            let args: Vec<f32> = args.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            match cmd {
                "M" => {
                    path = path.move_to(args[0], args[1]);
                }
                "L" => {
                    path = path.line_to(args[0], args[1]);
                }
                "C" => {
                    path = path.cubic_curve_to(args[0], args[1], args[2], args[3], args[4], args[5]);
                }
                "Q" => {
                    path = path.quadratic_curve_to(args[0], args[1], args[2], args[3]);
                }
                "Z" => {
                    path = path.close_path();
                }
                _ => {}
            }
        }
        path
    }
    pub fn points_within(&self) -> Vec<Vec2> {
        let mut points = Vec::new();
        for cmd in &self.commands {
            points.extend(cmd.points_within());
        }
        points
    }
    pub fn transformed_by(mut self, position: Vec2, scale: f32, rotation: f32) -> Self {
        for cmd in &mut self.commands {
            *cmd = cmd.scale_by(scale).rotate_around(position, rotation).translate_by(position);
        }
        self
    }
    pub fn scaled_by(mut self, s: f32) -> Self {
        for cmd in &mut self.commands {
            *cmd = cmd.scale_by(s);
        }
        self
    }
    pub fn scale_by(&mut self, s: f32) {
        for cmd in &mut self.commands {
            *cmd = cmd.scale_by(s);
        }
    }
    pub fn translated_by(mut self, v: Vec2) -> Self {
        for cmd in &mut self.commands {
            *cmd = cmd.translate_by(v);
        }
        self
    }
    pub fn translate_by(&mut self, v: Vec2) {
        for cmd in &mut self.commands {
            *cmd = cmd.translate_by(v);
        }
    }
    pub fn rotated_around(mut self, point: Vec2, angle: f32) -> Self {
        for cmd in &mut self.commands {
            *cmd = cmd.rotate_around(point, angle);
        }
        self
    }
    pub fn rotate_around(&mut self, point: Vec2, angle: f32) {
        for cmd in &mut self.commands {
            *cmd = cmd.rotate_around(point, angle);
        }
    }
    pub fn adjusted_by(mut self, f: impl Fn(&PathCmd) -> PathCmd) -> Self {
        for cmd in &mut self.commands {
            *cmd = f(cmd);
        }
        self
    }
    pub fn adjust_by(&mut self, f: impl Fn(&PathCmd) -> PathCmd) {
        for cmd in &mut self.commands {
            *cmd = f(cmd);
        }
    }
    pub fn rotated_by(self, angle: f32) -> Self {
        self.rotated_around(Vec2::new(0.0, 0.0), angle)
    }
    pub fn rotate_by(&mut self, angle: f32) {
        self.rotate_around(Vec2::new(0.0, 0.0), angle);
    }
    pub fn points(&self) -> Vec<Vec2> {
        let mut points = Vec::new();
        for cmd in &self.commands {
            points.extend(cmd.points());
        }
        points
    }
    pub fn to_substrokes(&self) -> Vec<SubstrokePath> {
        let mut substrokes = Vec::new();
        let mut current_substroke = SubstrokePath::new(Vec::new());
        for cmd in &self.commands {
            match cmd {
                PathCmd::MoveTo(_, _) => {
                    if !current_substroke.commands.is_empty() {
                        substrokes.push(current_substroke.clone());
                        current_substroke.commands.clear();
                    }
                }
                _ => {}
            }
            current_substroke.commands.push(*cmd);
        }
        if !current_substroke.commands.is_empty() {
            substrokes.push(current_substroke);
        }
        substrokes
    }
    pub fn roughened_by(mut self, max_magnitude: f32) -> Self {
        for cmd in &mut self.commands {
            *cmd = cmd.pos_roughen_by(max_magnitude);
        }
        self
    }
    pub fn new() -> Self {
        StrokePath {
            commands: Vec::new(),
        }
    }
    pub fn circle(center: Vec2, radius: f32) -> Self {
        let mut path = StrokePath::new();
        let kappa = 0.552284749831; // Approximation constant
        path = path.move_to(center.x + radius, center.y);
        path = path.cubic_curve_to(
            center.x + radius, center.y + kappa * radius,
            center.x + kappa * radius, center.y + radius,
            center.x, center.y + radius,
        );
        path = path.cubic_curve_to(
            center.x - kappa * radius, center.y + radius,
            center.x - radius, center.y + kappa * radius,
            center.x - radius, center.y,
        );
        path = path.cubic_curve_to(
            center.x - radius, center.y - kappa * radius,
            center.x - kappa * radius, center.y - radius,
            center.x, center.y - radius,
        );
        path = path.cubic_curve_to(
            center.x + kappa * radius, center.y - radius,
            center.x + radius, center.y - kappa * radius,
            center.x + radius, center.y,
        );
        // over draw to get a cleaner join
        let theta_overdraw_rad = std::f32::consts::PI / 32.0;
        let resolution = 32;
        for i in 1..(resolution + 1) {
            let next_theta = ((i as f32) / (resolution as f32)) * std::f32::consts::TAU + theta_overdraw_rad;
            let next_point = Vec2::new(
                center.x + radius * next_theta.cos(),
                center.y + radius * next_theta.sin(),
            );
            path = path.line_to(next_point.x, next_point.y);
        }
        path = path.close_path();
        path
    }
    pub fn rectangle(center: Vec2, width: f32, height: f32) -> Self {
        let half_w = width / 2.0;
        let half_h = height / 2.0;
        let mut path = StrokePath::new();
        path = path.move_to(center.x - half_w, center.y - half_h);
        path = path.line_to(center.x + half_w, center.y - half_h);
        path = path.line_to(center.x + half_w, center.y + half_h);
        path = path.line_to(center.x - half_w, center.y + half_h);
        path = path.close_path();
        path
    }
    pub fn line(start: Vec2, end: Vec2) -> Self {
        let mut path = StrokePath::new();
        path = path.move_to(start.x, start.y);
        path = path.line_to(end.x, end.y);
        path
    }
    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCmd::MoveTo(x, y));
        self
    }
    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCmd::LineTo(x, y));
        self
    }
    pub fn cubic_curve_to(mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) -> Self {
        self.commands.push(PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y));
        self
    }
    pub fn quadratic_curve_to(mut self, x1: f32, y1: f32, x: f32, y: f32) -> Self {
        self.commands.push(PathCmd::QuadraticCurveTo(x1, y1, x, y));
        self
    }
    pub fn close_path(mut self) -> Self {
        self.commands.push(PathCmd::ClosePath);
        self
    }
    pub fn append(&mut self, other: StrokePath) {
        self.commands.extend(other.commands);
    }
    pub fn append_clone(&mut self, other: &StrokePath) {
        self.commands.extend(other.commands.clone());
    }
    pub fn get_hull_points(&self) -> Vec<Vec2> {
        let substrokes = &self.to_substrokes();
        substrokes.into_iter().flat_map(|substroke| {
            substroke.get_point_samples(0.01).into_iter().map(|sample| {
                let rand_offset_amount = 0.01;
                let rand_offset = rand::random::<f32>() * rand_offset_amount - (rand_offset_amount / 2.0);
                sample.point + Vec2::new(rand_offset, rand_offset)
            })
        }).collect()
    }
    pub fn commands(&self) -> &Vec<PathCmd> {
        &self.commands
    }
    pub fn sample_points_along(&self, step: f32) -> Vec<CurveSample> {
        let substrokes = self.to_substrokes();
        let mut samples = Vec::new();
        for substroke in substrokes {
            samples.extend(substroke.get_point_samples(step));
        }
        samples
    }
    pub fn merge_path(mut self, other: &StrokePath) -> Self {
        self.commands.extend(other.commands.clone());
        self
    }
}

impl Add for StrokePath {
    type Output = StrokePath;

    fn add(mut self, other: Self) -> Self {
        self.commands.extend(other.commands.clone());
        self
    }
}

impl From<parry2d::bounding_volume::Aabb> for StrokePath {
    fn from(aabb: parry2d::bounding_volume::Aabb) -> Self {
        let center = aabb.center();
        let width = aabb.extents().x;
        let height = aabb.extents().y;
        StrokePath::rectangle(center, width, height)
    }
}

#[derive(Debug, Clone)]
pub struct CurveSample {
    pub point: Vec2,
    pub normal: Vec2,
    pub distance: f32,
    pub t: f32,
}

pub type CurveLength = f32;
pub type PriorDistance = f32;
pub type CurveSampleInfo = (Vec<CurveSample>, CurveLength);
pub fn curve_samples(cmd: &PathCmd, start: Vec2, current: Vec2, sample_count: usize) -> CurveSampleInfo {
    let mut samples = Vec::new();
    let delta_t = 1.0 / (sample_count + 2) as f32;
    let mut distance = 0.0;
    let curve = |t| cmd.interpolate(start, current, t).unwrap();
    for i in 1..(sample_count+2) {
        let t = i as f32 * delta_t;
        let point = curve(t);
        let left_point = curve(t - delta_t);
        let right_point = curve(t + delta_t);
        let tangent = (right_point - left_point).normalize();
        let normal = if tangent.x.is_nan() || tangent.y.is_nan() {
            Vec2::new(0.0, 1.0)
        } else {
            Vec2::new(-tangent.y, tangent.x)
        };
        let added_distance = left_point.distance(point);
        if !added_distance.is_nan() || !added_distance.is_infinite() {
            distance += added_distance;
        }
        samples.push(CurveSample {
            point,
            normal,
            distance,
            t,
        });
    }
    // add the sample for 0 and 1, extrapolating the normals from the first and second sample for 0 and the last and second to last for 1
    let first_sample = CurveSample {
        point: curve(0.0),
        normal: samples[0].normal - (samples[1].normal - samples[0].normal),
        distance: 0.0,
        t: 0.0,
    };
    let sample_count = samples.len();
    let last_sample = CurveSample {
        point: curve(1.0),
        normal: samples[sample_count - 1].normal + (samples[sample_count - 1].normal - samples[sample_count - 2].normal),
        distance: samples[sample_count - 1].distance + (samples[sample_count - 1].distance - samples[sample_count - 2].distance),
        t: 1.0,
    };
    samples.insert(0, first_sample);
    samples.push(last_sample);
    (samples, distance)
}

pub trait CurveStepper {
    fn step(&mut self, distance: f32) -> Option<CurveSample>;
    fn prospective_step(&self, distance: f32) -> Option<CurveSample>;
}

type CurveIndex = usize;
type SampleIndex = usize;
type AugCurveSampleInfo = (Vec<CurveSample>, CurveLength, PriorDistance);

#[derive(Clone)]
pub struct SubstrokeStepper {
    current_along: f32,
    last_index: Cell<(CurveIndex, SampleIndex)>,
    samples: Vec<AugCurveSampleInfo>,
}

impl CurveStepper for SubstrokeStepper {
    fn step(&mut self, distance: f32) -> Option<CurveSample> {
        if distance < 0.0 {
            return None;
        }

        let target_distance = self.current_along + distance;
        let last_indices = self.last_index.get();
        SubstrokeStepper::forward_seek(Some(last_indices), target_distance, &self.samples)
            .map(|(c, s, sample)| {
                self.current_along += distance;
                self.last_index.set((c, s));
                sample
            })
    }
    fn prospective_step(&self, distance: f32) -> Option<CurveSample> {
        if distance < 0.0 {
            return None;
        }

        let target_distance = self.current_along + distance;
        SubstrokeStepper::forward_seek(None, target_distance, &self.samples)
            .map(|(_, _, sample)| sample)
    }
}

impl SubstrokeStepper {
    pub fn new_child(&self) -> ChildStepper<'_> {
        ChildStepper {
            current_along: 0.0,
            parent_samples: &self.samples,
        }
    }
    pub fn new(substroke: &SubstrokePath, sample_count: usize) -> Self {
        let mut samples = Vec::new();
        let mut current_point = Vec2::new(0.0, 0.0);
        let mut start_point = None;
        let mut covered_distance = 0.0;
        for cmd in &substroke.commands {
            if let PathCmd::MoveTo(x, y) = cmd {
                current_point = Vec2::new(*x, *y);
                start_point = Some(current_point);
                continue;
            }
            let start = start_point.unwrap_or(current_point);
            let curve_sampling = curve_samples(cmd, start, current_point, sample_count);
            let aug_samples_info = (curve_sampling.0, curve_sampling.1,  covered_distance);
            covered_distance += aug_samples_info.1;
            samples.push(aug_samples_info);
            current_point = cmd.ending_point().unwrap_or(current_point)
        }
        SubstrokeStepper {
            current_along: 0.0,
            last_index: Cell::new((0, 0)),
            samples,
        }
    }
    pub fn generate_equidistant_samples(&mut self, distance: f32) -> Vec<CurveSample> {
        let mut samples = Vec::new();
        while let Some(sample) = self.step(distance) {
            samples.push(sample);
        }
        samples
    }

    fn forward_seek(starting_indices: Option<(CurveIndex, SampleIndex)>, target_distance: f32, curves: &Vec<AugCurveSampleInfo>) -> Option<(CurveIndex, SampleIndex, CurveSample)> {

        let starting_curve_index = starting_indices.map(|(c, _)| c).unwrap_or(0);
        let mut selected_curve_info = None;
        let mut curve_index = None;
        let curves_to_search = curves[starting_curve_index..].iter().enumerate();
        for (index_offset, info) in curves_to_search {
            let current_index = starting_curve_index + index_offset;
            if info.1 + info.2 >= target_distance {
                selected_curve_info.replace(info);
                curve_index.replace(current_index);
                break;
            }
        }

        let selected_curve = selected_curve_info?;
        let curve_index = curve_index?;
        let samples = &selected_curve.0;
        let prior_distance = selected_curve.2;

        let mut selected_left_sample_index = None;
        let starting_sample_index = 0; //starting_indices.map(|(_, s)| s).unwrap_or(0);
        let samples_to_search = samples[starting_sample_index..].iter().enumerate();

        for (index_offset, left_sample) in samples_to_search {
            let current_index = starting_sample_index + index_offset;
            let right_sample = &samples.get(current_index + 1)?;
            let left_distance = left_sample.distance + prior_distance;
            let right_distance = right_sample.distance + prior_distance;
            if left_distance <= target_distance && right_distance >= target_distance {
                selected_left_sample_index.replace(current_index);
                break;
            }
        }
        let left_sample_index = selected_left_sample_index?;
        let left_sample = &samples[left_sample_index];
        let right_sample = &samples[left_sample_index + 1];
        let distance_num = target_distance - prior_distance - left_sample.distance;
        let distance_den = right_sample.distance - left_sample.distance;

        let t_in = distance_num / distance_den;
        let point = left_sample.point.lerp(right_sample.point, t_in);
        let normal = left_sample.normal.lerp(right_sample.normal, t_in).normalize();
        let t = left_sample.t + (right_sample.t - left_sample.t) * t_in;
        Some((curve_index, left_sample_index, CurveSample {
            point,
            normal,
            distance: target_distance,
            t,
        }))
    }
}


pub struct ChildStepper<'parent>{
    current_along: f32,
    parent_samples: &'parent Vec<AugCurveSampleInfo>,
}

impl<'parent> CurveStepper for ChildStepper<'parent> {
    fn step(&mut self, distance: f32) -> Option<CurveSample> {
        self.prospective_step(distance).map(|sample| {
            self.current_along += distance;
            sample
        })
    }
    fn prospective_step(&self, distance: f32) -> Option<CurveSample> {
        // Right now this steps along the whole substroke path every
        // single step, which is horribly inefficient, but simpler to implement.
        if distance < 0.0 {
            return None;
        }

        let target_distance = self.current_along + distance;
        SubstrokeStepper::forward_seek(None, target_distance, self.parent_samples)
            .map(|(_, _, sample)| sample)
    }
}
#[derive(Debug, Clone)]
pub struct SubstrokePath {
    commands: Vec<PathCmd>,
}

impl SubstrokePath {
    pub fn new(commands: Vec<PathCmd>) -> Self {
        SubstrokePath {
            commands,
        }
    }
    pub fn path_points(&self) -> Vec<Vec2> {
        let mut points = Vec::new();
        let mut first = None;
        for cmd in &self.commands {
            first = first.or(Some(cmd.clone()));
            match cmd {
                PathCmd::MoveTo(x, y) => {
                    points.push(Vec2::new(*x, *y));
                }
                PathCmd::LineTo(x, y) => {
                    points.push(Vec2::new(*x, *y));
                }
                PathCmd::CubicCurveTo(_, _, _, _, x, y) => {
                    points.push(Vec2::new(*x, *y));
                }
                PathCmd::QuadraticCurveTo(_, _, x, y) => {
                    points.push(Vec2::new(*x, *y));
                }
                _ => {}
            }
        }
        points
    }
    pub fn get_point_samples(&self, distance_interval: f32) -> Vec<CurveSample> {
        let mut stepper = SubstrokeStepper::new(self, 50);
        stepper.generate_equidistant_samples(distance_interval)
    }
    pub fn ray_stroke_fill(&self, step_distance: f32, stroke_angle: f32, roughness: f32) -> StrokePath {
        let samples = self
            .get_point_samples(step_distance)
            .into_iter()
            .map(|sample| { sample.point })
            .collect::<Vec<Vec2>>();
        let ray_direction = Vec2::new(stroke_angle.cos(), stroke_angle.sin());
        let ray_normal = Vec2::new(-ray_direction.y, ray_direction.x).normalize();
        // find the point farthest along the ray normal in one direction
        let mut min_projection = std::f32::MAX;
        let mut min_point = Vec2::new(0.0, 0.0);
        let mut max_projection = std::f32::MIN;
        for point in &samples {
            // project point onto ray normal
            let projection = point.dot(ray_normal);
            if projection < min_projection {
                min_point = point.clone();
                min_projection = projection;
            }
            if projection > max_projection {
                max_projection = projection;
            }
        }
        let mut line_segments : Vec<parry2d::shape::Segment> = samples.windows(2).map(|w| {
            parry2d::shape::Segment::new(w[0].into(), w[1].into())
        }).collect();
        line_segments.push(parry2d::shape::Segment::new(samples[samples.len() - 1].into(), samples[0].into()));
        let distance_to_walk = max_projection - min_projection;
        let mut current_distance_along_norm = 0.0;
        let mut fill_path = StrokePath::new().move_to(min_point.x, min_point.y);
        while current_distance_along_norm < distance_to_walk {
            let current_point_on_norm =
                min_point + ray_normal * current_distance_along_norm;
            let ray_origin = current_point_on_norm - ray_direction * 1000.0;
            let ray = parry2d::query::Ray::new(ray_origin, ray_direction);
            let transform = parry2d::math::Pose2::IDENTITY;
            let mut intersections: Vec<(Vec2, f32)> = Vec::new();
            for segment in &line_segments {
                let intersection = segment.cast_ray(
                    &transform, &ray, std::f32::MAX, true,
                );
                if let Some(intersection) = intersection {
                    let int_point = ray.origin + ray.dir * intersection;
                    let dist_along_ray = ray_origin.distance(int_point);
                    intersections.push((int_point, dist_along_ray));
                }
                //sort by distance along ray, from nearest to farthest
            }
            intersections.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let mut kept_intersections = Vec::new();
            let mut last_dist = std::f32::MIN;
            for inter in intersections {
                if inter.1 - last_dist > step_distance {
                    kept_intersections.push(inter);
                    last_dist = inter.1;
                }
            }
            if kept_intersections.len() % 2 != 0 {
                kept_intersections.pop();
            }
            for i in (0..kept_intersections.len()).step_by(2) {
                let entry_point = roughen_point(kept_intersections[i].0, roughness);
                let exit_point = roughen_point(kept_intersections[i + 1].0, roughness);
                fill_path = fill_path
                    .line_to(entry_point.x, entry_point.y)
                    .line_to(exit_point.x, exit_point.y);
            }
            current_distance_along_norm += step_distance;
        }
        fill_path
    }
}

fn bc_interp(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

fn bq_interp(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    u * u * p0 + 2.0 * u * t * p1 + t * t * p2
}

fn l_interp(p0: Vec2, p1: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    u * p0 + t * p1
}
