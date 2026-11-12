//! Affine maps of the plane, as SVG's `matrix(a b c d e f)` holds them:
//! a point (x, y) goes to (a x + c y + e, b x + d y + f). The visual
//! editor's edits are these, and so are the paints moved with them.

use parry2d::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [f32; 6]);

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn translate(v: Vec2) -> Affine {
        Affine([1.0, 0.0, 0.0, 1.0, v.x, v.y])
    }
    /// The linear map with columns `x` and `y` (where (1, 0) and (0, 1) go).
    pub fn linear(x: Vec2, y: Vec2) -> Affine {
        Affine([x.x, x.y, y.x, y.y, 0.0, 0.0])
    }
    /// Turned `degrees` about the origin, clockwise on a page (y down).
    pub fn rotate(degrees: f32) -> Affine {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Affine([cos, sin, -sin, cos, 0.0, 0.0])
    }
    pub fn scale(x: f32, y: f32) -> Affine {
        Affine([x, 0.0, 0.0, y, 0.0, 0.0])
    }
    /// Skewed sideways by `degrees`: points further down go further right.
    pub fn skew(degrees: f32) -> Affine {
        Affine([1.0, 0.0, degrees.to_radians().tan(), 1.0, 0.0, 0.0])
    }
    pub fn apply(&self, p: Vec2) -> Vec2 {
        let [a, b, c, d, e, f] = self.0;
        Vec2::new(a * p.x + c * p.y + e, b * p.x + d * p.y + f)
    }
    /// This map, then `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        let [a, b, c, d, e, f] = next.0;
        let [p, q, r, s, t, u] = self.0;
        Affine([
            a * p + c * q, b * p + d * q,
            a * r + c * s, b * r + d * s,
            a * t + c * u + e, b * t + d * u + f,
        ])
    }
    /// `self` about `centre`: moved there, mapped, and moved back.
    pub fn about(&self, centre: Vec2) -> Affine {
        Affine::translate(-centre).then(self).then(&Affine::translate(centre))
    }
    pub fn determinant(&self) -> f32 {
        let [a, b, c, d, _, _] = self.0;
        a * d - b * c
    }
    /// The map back; `None` if it flattens the plane.
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.determinant();
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        let [a, b, c, d, e, f] = self.0;
        Some(Affine([
            d / det, -b / det,
            -c / det, a / det,
            (c * f - d * e) / det, (b * e - a * f) / det,
        ]))
    }
    pub fn is_identity(&self) -> bool {
        *self == Affine::IDENTITY
    }
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }
    /// As an SVG transform.
    pub fn svg(&self) -> String {
        let [a, b, c, d, e, f] = self.0;
        format!("matrix({} {} {} {} {} {})", a, b, c, d, e, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec2, b: Vec2) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn maps_compose_in_order() {
        let p = Vec2::new(2.0, 1.0);
        let m = Affine::scale(2.0, 3.0).then(&Affine::translate(Vec2::new(10.0, 0.0)));
        assert!(close(m.apply(p), Vec2::new(14.0, 3.0)));
        // A quarter turn clockwise on a page takes right to down.
        assert!(close(Affine::rotate(90.0).apply(Vec2::new(1.0, 0.0)), Vec2::new(0.0, 1.0)));
        assert!(close(Affine::rotate(90.0).about(Vec2::new(1.0, 1.0)).apply(Vec2::new(2.0, 1.0)), Vec2::new(1.0, 2.0)));
        assert!(close(Affine::skew(45.0).apply(Vec2::new(0.0, 2.0)), Vec2::new(2.0, 2.0)));
    }

    #[test]
    fn the_inverse_undoes_it() {
        let m = Affine::rotate(30.0).then(&Affine::skew(10.0)).then(&Affine::scale(2.0, -0.5)).then(&Affine::translate(Vec2::new(3.0, -7.0)));
        let back = m.inverse().expect("invertible");
        let p = Vec2::new(5.0, 9.0);
        assert!(close(back.apply(m.apply(p)), p));
        assert!(Affine::scale(0.0, 1.0).inverse().is_none());
    }
}
