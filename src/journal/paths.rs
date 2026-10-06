use parry2d::math::{Vec2, Rot2};
use parry2d::query::RayCast;
use std::ops::Add;
use std::cell::Cell;
use rand::{Rng, RngCore, SeedableRng};
use rand::rngs::StdRng;

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

fn roughen_point(point: Vec2, r: f32, rng: &mut dyn RngCore) -> Vec2 {
    let random_theta = rng.random::<f32>() * std::f32::consts::PI * 2.0;
    let random_magnitude = (rng.random::<f32>() - 0.5) * 2.0 * r;
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
    pub fn points_within(&self) -> Vec<Vec2> {
        match self {
            PathCmd::MoveTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::LineTo(x, y) => vec![Vec2::new(*x, *y)],
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => vec![Vec2::new(*x1, *y1), Vec2::new(*x2, *y2), Vec2::new(*x, *y)],
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => vec![Vec2::new(*x1, *y1), Vec2::new(*x, *y)],
            PathCmd::ClosePath => vec![],
        }
    }
    /// Nudge the end point and control points by random offsets of up to
    /// `max_magnitude`.
    pub fn pos_roughen_by(&self, max_magnitude: f32, rng: &mut dyn RngCore) -> Self {
        let mut new_roughen = || {
            let random_theta = rng.random::<f32>() * std::f32::consts::PI * 2.0;
            let random_magnitude = rng.random::<f32>() * max_magnitude;
            Vec2::new(random_magnitude * random_theta.cos(), random_magnitude * random_theta.sin())
        };
        let pos_roughen = new_roughen();
        match self {
            PathCmd::MoveTo(x, y) => {
                PathCmd::MoveTo(x + pos_roughen.x, y + pos_roughen.y)
            }
            PathCmd::LineTo(x, y) => {
                PathCmd::LineTo(x + pos_roughen.x, y + pos_roughen.y)
            }
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => {
                let (c1, c2) = (new_roughen(), new_roughen());
                PathCmd::CubicCurveTo(x1 + c1.x, y1 + c1.y,
                                      x2 + c2.x, y2 + c2.y,
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
                let rotated_point = rotate_around_point(Vec2::new(*x, *y) - point, angle);
                PathCmd::MoveTo(rotated_point.x + point.x, rotated_point.y + point.y)
            }
            PathCmd::LineTo(x, y) => {
                let rotated_point = rotate_around_point(Vec2::new(*x, *y) - point, angle);
                PathCmd::LineTo(rotated_point.x + point.x, rotated_point.y + point.y)
            }
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => {
                let rotated_point1 = rotate_around_point(Vec2::new(*x1, *y1) - point, angle);
                let rotated_point2 = rotate_around_point(Vec2::new(*x2, *y2) - point, angle);
                let rotated_point3 = rotate_around_point(Vec2::new(*x, *y) - point, angle);
                PathCmd::CubicCurveTo(rotated_point1.x + point.x, rotated_point1.y + point.y,
                                      rotated_point2.x + point.x, rotated_point2.y + point.y,
                                      rotated_point3.x + point.x, rotated_point3.y + point.y)
            }
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => {
                let rotated_point1 = rotate_around_point(Vec2::new(*x1, *y1) - point, angle);
                let rotated_point2 = rotate_around_point(Vec2::new(*x, *y) - point, angle);
                PathCmd::QuadraticCurveTo(rotated_point1.x + point.x, rotated_point1.y + point.y,
                                          rotated_point2.x + point.x, rotated_point2.y + point.y)
            }
            PathCmd::ClosePath => PathCmd::ClosePath,
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
        min_vec.y += avoid_scale / 2.0;
        let sw_vec = match expected_stroke_width {
            Some(sw) => Vec2::new(sw, sw),
            None => Vec2::new(0.0, 0.0),
        };
        let mut avoid_tree: kdtree::KdTree<f32, (f32, f32), [f32; 2]> = kdtree::KdTree::new(2);
        let sample_points =
            self.sample_points_along(avoid_scale * 1.0)
            .into_iter().map(|sample| sample.point - min_vec);
        // The k-d tree rejects non-finite points; they can't be avoided
        // anyway.
        for point in sample_points.filter(|p| p.is_finite()) {
            for side in [point - sw_vec, point + sw_vec] {
                avoid_tree.add([side.x, side.y], (side.x, side.y)).expect("finite points are accepted");
            }
        }
        avoid_tree
    }
    pub fn points_within(&self) -> Vec<Vec2> {
        let mut points = Vec::new();
        for cmd in &self.commands {
            points.extend(cmd.points_within());
        }
        points
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
    /// The same path with every line and quadratic segment written as a
    /// cubic, which the SVG output's roughening works on.
    pub fn to_cubics(&self) -> StrokePath {
        let mut current = Vec2::ZERO;
        let mut subpath_start = Vec2::ZERO;
        let commands = self.commands.iter().map(|cmd| {
            let cubic = match *cmd {
                PathCmd::MoveTo(x, y) => {
                    subpath_start = Vec2::new(x, y);
                    PathCmd::MoveTo(x, y)
                }
                // Control points on the end point keep it straight.
                PathCmd::LineTo(x, y) => PathCmd::CubicCurveTo(x, y, x, y, x, y),
                PathCmd::CubicCurveTo(..) => *cmd,
                PathCmd::QuadraticCurveTo(qx, qy, x, y) => {
                    // The exact cubic: each control point is 2/3 of the way
                    // from an end point to the quadratic's control point.
                    let (q, end) = (Vec2::new(qx, qy), Vec2::new(x, y));
                    let c1 = current + (q - current) * (2.0 / 3.0);
                    let c2 = end + (q - end) * (2.0 / 3.0);
                    PathCmd::CubicCurveTo(c1.x, c1.y, c2.x, c2.y, x, y)
                }
                PathCmd::ClosePath => PathCmd::ClosePath,
            };
            current = cmd.ending_point().unwrap_or(subpath_start);
            cubic
        }).collect();
        StrokePath { commands }
    }
    pub fn adjust_by(&mut self, mut f: impl FnMut(&PathCmd) -> PathCmd) {
        for cmd in &mut self.commands {
            *cmd = f(cmd);
        }
    }
    pub fn rotated_by(self, angle: f32) -> Self {
        self.rotated_around(Vec2::new(0.0, 0.0), angle)
    }
    pub fn to_substrokes(&self) -> Vec<SubstrokePath> {
        let mut substrokes = Vec::new();
        let mut current_substroke = SubstrokePath::new(Vec::new());
        for cmd in &self.commands {
            if let PathCmd::MoveTo(_, _) = cmd
                && !current_substroke.commands.is_empty() {
                    substrokes.push(current_substroke.clone());
                    current_substroke.commands.clear();
                }
            current_substroke.commands.push(*cmd);
        }
        if !current_substroke.commands.is_empty() {
            substrokes.push(current_substroke);
        }
        substrokes
    }
    pub fn new() -> Self {
        StrokePath {
            commands: Vec::new(),
        }
    }
    pub fn circle(center: Vec2, radius: f32) -> Self {
        let mut path = StrokePath::new();
        let kappa = 0.552_284_8; // Approximation constant
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
    /// A circular arc from `start_rad` to `end_rad` (either direction),
    /// built from cubic segments of at most 90 degrees each.
    pub fn arc(center: Vec2, radius: f32, start_rad: f32, end_rad: f32) -> Self {
        let sweep = end_rad - start_rad;
        let segments = ((sweep.abs() / std::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
        let step = sweep / segments as f32;
        // Control-point distance for a cubic approximating a circular arc.
        let k = 4.0 / 3.0 * (step / 4.0).tan() * radius;
        let point = |a: f32| center + Vec2::new(a.cos(), a.sin()) * radius;
        let tangent = |a: f32| Vec2::new(-a.sin(), a.cos()) * k;
        let start = point(start_rad);
        let mut path = StrokePath::new().move_to(start.x, start.y);
        for i in 0..segments {
            let a0 = start_rad + step * i as f32;
            let a1 = a0 + step;
            let (p0, p3) = (point(a0), point(a1));
            let c1 = p0 + tangent(a0);
            let c2 = p3 - tangent(a1);
            path = path.cubic_curve_to(c1.x, c1.y, c2.x, c2.y, p3.x, p3.y);
        }
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
    pub fn get_hull_points(&self) -> Vec<Vec2> {
        let substrokes = &self.to_substrokes();
        // The jitter keeps the hull from degenerating on straight runs; a
        // fixed seed keeps hulls, and so symbol placement, the same every run.
        let mut rng = StdRng::seed_from_u64(0);
        let rand_offset_amount = 0.01;
        let mut points = Vec::new();
        for substroke in substrokes {
            for sample in substroke.get_point_samples(0.01) {
                let rand_offset = rng.random::<f32>() * rand_offset_amount - (rand_offset_amount / 2.0);
                points.push(sample.point + Vec2::new(rand_offset, rand_offset));
            }
        }
        points
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

impl Default for StrokePath {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses SVG path data: every command except arcs, absolute or relative,
/// with numbers separated by whitespace, commas, or nothing where that's
/// unambiguous (`1-2`, `.5.5`), and repeated argument groups after a
/// command (extra pairs after a moveto are linetos).
impl std::str::FromStr for StrokePath {
    type Err = String;
    fn from_str(data: &str) -> Result<Self, String> {
        PathParser { data: data.as_bytes(), pos: 0 }.parse()
    }
}

struct PathParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl PathParser<'_> {
    fn skip_separators(&mut self) {
        while self.pos < self.data.len() && (self.data[self.pos].is_ascii_whitespace() || self.data[self.pos] == b',') {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_separators();
        self.data.get(self.pos).copied()
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self, cmd: u8) -> Result<f32, String> {
        self.skip_separators();
        let start = self.pos;
        if matches!(self.data.get(self.pos), Some(b'+' | b'-')) {
            self.pos += 1;
        }
        let mut digits = self.digits();
        if self.data.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            digits += self.digits();
        }
        if digits == 0 {
            self.pos = start;
            return Err(format!("'{}' expects more numbers at byte {}", cmd as char, start));
        }
        if matches!(self.data.get(self.pos), Some(b'e' | b'E')) {
            let mantissa_end = self.pos;
            self.pos += 1;
            if matches!(self.data.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                self.pos = mantissa_end;
            }
        }
        let text = std::str::from_utf8(&self.data[start..self.pos]).expect("numbers are ASCII");
        text.parse().map_err(|_| format!("invalid number '{}' at byte {}", text, start))
    }

    fn point(&mut self, cmd: u8, origin: Vec2) -> Result<Vec2, String> {
        let x = self.number(cmd)?;
        let y = self.number(cmd)?;
        Ok(origin + Vec2::new(x, y))
    }

    fn parse(mut self) -> Result<StrokePath, String> {
        let mut path = StrokePath::new();
        let mut current = Vec2::ZERO;
        let mut subpath_start = Vec2::ZERO;
        // The previous segment's last control point, for S and T to reflect.
        let mut last_cubic_control: Option<Vec2> = None;
        let mut last_quad_control: Option<Vec2> = None;
        let mut cmd: Option<u8> = None;
        while let Some(next) = self.peek() {
            if next.is_ascii_alphabetic() {
                self.pos += 1;
                if cmd.is_none() && !matches!(next, b'M' | b'm') {
                    return Err(format!("path data must start with a moveto, not '{}'", next as char));
                }
                cmd = Some(next);
                if matches!(next, b'Z' | b'z') {
                    path = path.close_path();
                    current = subpath_start;
                    last_cubic_control = None;
                    last_quad_control = None;
                    continue;
                }
            } else if !matches!(next, b'0'..=b'9' | b'.' | b'+' | b'-') {
                return Err(format!("unexpected '{}' at byte {}", next as char, self.pos));
            }
            let c = cmd.ok_or("path data must start with a moveto")?;
            let origin = if c.is_ascii_lowercase() { current } else { Vec2::ZERO };
            let (mut cubic_control, mut quad_control) = (None, None);
            match c.to_ascii_uppercase() {
                b'M' => {
                    current = self.point(c, origin)?;
                    subpath_start = current;
                    path = path.move_to(current.x, current.y);
                    // Further pairs are implicit linetos.
                    cmd = Some(if c == b'm' { b'l' } else { b'L' });
                }
                b'L' => {
                    current = self.point(c, origin)?;
                    path = path.line_to(current.x, current.y);
                }
                b'H' => {
                    current.x = origin.x + self.number(c)?;
                    path = path.line_to(current.x, current.y);
                }
                b'V' => {
                    current.y = origin.y + self.number(c)?;
                    path = path.line_to(current.x, current.y);
                }
                b'C' | b'S' => {
                    let c1 = if c.eq_ignore_ascii_case(&b'C') {
                        self.point(c, origin)?
                    } else {
                        last_cubic_control.map_or(current, |p| 2.0 * current - p)
                    };
                    let c2 = self.point(c, origin)?;
                    current = self.point(c, origin)?;
                    path = path.cubic_curve_to(c1.x, c1.y, c2.x, c2.y, current.x, current.y);
                    cubic_control = Some(c2);
                }
                b'Q' | b'T' => {
                    let control = if c.eq_ignore_ascii_case(&b'Q') {
                        self.point(c, origin)?
                    } else {
                        last_quad_control.map_or(current, |p| 2.0 * current - p)
                    };
                    current = self.point(c, origin)?;
                    path = path.quadratic_curve_to(control.x, control.y, current.x, current.y);
                    quad_control = Some(control);
                }
                b'Z' => return Err(format!("unexpected number after '{}' at byte {}", c as char, self.pos)),
                b'A' => return Err("arcs ('A'/'a') aren't supported".to_string()),
                _ => return Err(format!("unknown path command '{}'", c as char)),
            }
            last_cubic_control = cubic_control;
            last_quad_control = quad_control;
        }
        Ok(path)
    }
}

/// SVG path data, one command at a time.
impl std::fmt::Display for PathCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathCmd::MoveTo(x, y) => write!(f, "M {} {}", x, y),
            PathCmd::LineTo(x, y) => write!(f, "L {} {}", x, y),
            PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) => write!(f, "C {} {} {} {} {} {}", x1, y1, x2, y2, x, y),
            PathCmd::QuadraticCurveTo(x1, y1, x, y) => write!(f, "Q {} {} {} {}", x1, y1, x, y),
            PathCmd::ClosePath => write!(f, "Z"),
        }
    }
}

/// SVG path data, with a space after each command.
impl std::fmt::Display for StrokePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for cmd in &self.commands {
            write!(f, "{} ", cmd)?;
        }
        Ok(())
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

    fn forward_seek(starting_indices: Option<(CurveIndex, SampleIndex)>, target_distance: f32, curves: &[AugCurveSampleInfo]) -> Option<(CurveIndex, SampleIndex, CurveSample)> {

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
        self.prospective_step(distance).inspect(|_sample| {
            self.current_along += distance;
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
    pub fn get_point_samples(&self, distance_interval: f32) -> Vec<CurveSample> {
        let mut stepper = SubstrokeStepper::new(self, 50);
        stepper.generate_equidistant_samples(distance_interval)
    }
    pub fn ray_stroke_fill(&self, step_distance: f32, stroke_angle: f32, roughness: f32, rng: &mut dyn RngCore) -> StrokePath {
        let samples = self
            .get_point_samples(step_distance)
            .into_iter()
            .map(|sample| { sample.point })
            .collect::<Vec<Vec2>>();
        let ray_direction = Vec2::new(stroke_angle.cos(), stroke_angle.sin());
        let ray_normal = Vec2::new(-ray_direction.y, ray_direction.x).normalize();
        // find the point farthest along the ray normal in one direction
        let mut min_projection = f32::MAX;
        let mut min_point = Vec2::new(0.0, 0.0);
        let mut max_projection = f32::MIN;
        for point in &samples {
            // project point onto ray normal
            let projection = point.dot(ray_normal);
            if projection < min_projection {
                min_point = *point;
                min_projection = projection;
            }
            if projection > max_projection {
                max_projection = projection;
            }
        }
        let mut line_segments : Vec<parry2d::shape::Segment> = samples.windows(2).map(|w| {
            parry2d::shape::Segment::new(w[0], w[1])
        }).collect();
        line_segments.push(parry2d::shape::Segment::new(samples[samples.len() - 1], samples[0]));
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
                    &transform, &ray, f32::MAX, true,
                );
                if let Some(intersection) = intersection {
                    let int_point = ray.origin + ray.dir * intersection;
                    let dist_along_ray = ray_origin.distance(int_point);
                    intersections.push((int_point, dist_along_ray));
                }
                //sort by distance along ray, from nearest to farthest
            }
            intersections.sort_by(|a, b| a.1.total_cmp(&b.1));
            let mut kept_intersections = Vec::new();
            let mut last_dist = f32::MIN;
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
                let entry_point = roughen_point(kept_intersections[i].0, roughness, rng);
                let exit_point = roughen_point(kept_intersections[i + 1].0, roughness, rng);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cubic_ends(path: &StrokePath) -> Vec<Vec2> {
        path.commands.iter().filter_map(|cmd| match cmd {
            PathCmd::MoveTo(x, y) | PathCmd::CubicCurveTo(_, _, _, _, x, y) => Some(Vec2::new(*x, *y)),
            _ => None,
        }).collect()
    }

    fn close(a: Vec2, b: Vec2) -> bool {
        (a - b).length() < 1e-4
    }

    fn parsed(data: &str) -> String {
        data.parse::<StrokePath>().unwrap_or_else(|e| panic!("{:?}: {}", data, e)).to_string().trim_end().to_string()
    }

    #[test]
    fn parses_absolute_commands() {
        assert_eq!(parsed("M 0 0 L 1 1 C 1 2 3 4 5 6 Q 7 8 9 10 Z"), "M 0 0 L 1 1 C 1 2 3 4 5 6 Q 7 8 9 10 Z");
    }

    #[test]
    fn parses_relative_and_axis_commands() {
        assert_eq!(parsed("m 1 1 l 2 0 h 3 v 4 z l 1 1"), "M 1 1 L 3 1 L 6 1 L 6 5 Z L 2 2");
        assert_eq!(parsed("M 1 1 H 5 V 7"), "M 1 1 L 5 1 L 5 7");
    }

    #[test]
    fn parses_compact_numbers_and_repeated_arguments() {
        // Extra pairs after a moveto are linetos; commas and signs separate.
        assert_eq!(parsed("M0,0 1,1 2,2"), "M 0 0 L 1 1 L 2 2");
        assert_eq!(parsed("M.5.5-1-1"), "M 0.5 0.5 L -1 -1");
        assert_eq!(parsed("M1e1,2E-1 L+3-4"), "M 10 0.2 L 3 -4");
        assert_eq!(parsed("M0 0 L1 1 2 2"), "M 0 0 L 1 1 L 2 2");
    }

    #[test]
    fn smooth_curves_reflect_the_previous_control_point() {
        assert_eq!(parsed("M0 0 C 1 1 2 1 3 0 S 5 -1 6 0"), "M 0 0 C 1 1 2 1 3 0 C 4 -1 5 -1 6 0");
        assert_eq!(parsed("M0 0 Q 1 1 2 0 T 4 0"), "M 0 0 Q 1 1 2 0 Q 3 -1 4 0");
        // Without a curve before them, the control point is the current point.
        assert_eq!(parsed("M0 0 L 1 0 S 2 1 3 0"), "M 0 0 L 1 0 C 1 0 2 1 3 0");
    }

    #[test]
    fn quadratics_convert_to_the_same_curve() {
        let quad = StrokePath::new().move_to(0.0, 0.0).quadratic_curve_to(1.0, 2.0, 2.0, 0.0)
            .move_to(5.0, 5.0).line_to(6.0, 5.0).quadratic_curve_to(7.0, 7.0, 8.0, 5.0);
        let cubic = quad.to_cubics();
        let PathCmd::CubicCurveTo(x1, y1, x2, y2, x, y) = cubic.commands[1] else { panic!("{}", cubic) };
        for (got, want) in [(x1, 2.0 / 3.0), (y1, 4.0 / 3.0), (x2, 4.0 / 3.0), (y2, 4.0 / 3.0), (x, 2.0), (y, 0.0)] {
            assert!((got - want).abs() < 1e-5, "{}", cubic);
        }
        assert!(cubic.commands.iter().all(|c| matches!(c, PathCmd::MoveTo(..) | PathCmd::CubicCurveTo(..))), "{}", cubic);
        // Same points along the way, not just the same ends. (Only the
        // quadratic: a line written as a cubic has the same shape but is
        // sampled slightly differently.)
        let quad = StrokePath::new().move_to(0.0, 0.0).quadratic_curve_to(1.0, 2.0, 2.0, 0.0);
        let (a, b) = (quad.sample_points_along(0.05), quad.to_cubics().sample_points_along(0.05));
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(&b) {
            assert!((p.point - q.point).length() < 1e-3, "{:?} vs {:?}", p.point, q.point);
        }
    }

    #[test]
    fn rejects_bad_path_data() {
        for (data, want) in [
            ("L 1 1", "must start with a moveto"),
            ("M 1", "'M' expects more numbers"),
            ("M 0 0 C 1 2 3", "'C' expects more numbers"),
            ("M 0 0 A 1 1 0 0 1 2 2", "arcs"),
            ("M 0 0 X 1 1", "unknown path command 'X'"),
            ("M 0 0 # 1", "unexpected '#'"),
            ("M 0 0 Z 1", "unexpected number after 'Z'"),
        ] {
            let err = data.parse::<StrokePath>().err().unwrap_or_else(|| panic!("{:?} should fail", data));
            assert!(err.contains(want), "{:?}: {}", data, err);
        }
    }

    #[test]
    fn arc_splits_into_quarter_segments_and_hits_endpoints() {
        let center = Vec2::new(1.0, 2.0);
        // 270 degrees: three segments, ending at angle 3pi/2.
        let path = StrokePath::arc(center, 2.0, 0.0, 1.5 * std::f32::consts::PI);
        let ends = cubic_ends(&path);
        assert_eq!(ends.len(), 4); // move_to + 3 cubics
        assert!(close(ends[0], Vec2::new(3.0, 2.0)));
        assert!(close(ends[1], Vec2::new(1.0, 4.0)));
        assert!(close(ends[3], Vec2::new(1.0, 0.0)));
    }

    #[test]
    fn arc_runs_backwards_for_negative_sweep() {
        let path = StrokePath::arc(Vec2::new(0.0, 0.0), 1.0, 0.0, -std::f32::consts::FRAC_PI_2);
        let ends = cubic_ends(&path);
        assert_eq!(ends.len(), 2);
        assert!(close(ends[1], Vec2::new(0.0, -1.0)));
    }
}
