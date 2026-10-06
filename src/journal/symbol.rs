use super::paths::{
    CurveSample, CurveStepper,
    StrokePath, SubstrokePath, SubstrokeStepper,
};
use parry2d::math::Vec2;
use kdtree::KdTree;
use kdtree::distance::squared_euclidean;

use std::rc::Rc;
use std::iter::Peekable;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum SymbolType {
    Blank(f32, f32),
    Pathed(StrokePath),
}

#[derive(Debug, Clone)]
pub struct SymbolBase {
    symbol_type: SymbolType,
    hull: parry2d::shape::ConvexPolygon,
    /// The hull's bounding box, which text layout reads for every glyph.
    hull_aabb: parry2d::bounding_volume::Aabb,
}

impl SymbolBase {
    pub fn new(symbol_type: SymbolType) -> SymbolBase {
        let hull = SymbolBase::generate_hull(&symbol_type);
        SymbolBase {
            hull_aabb: hull.local_aabb(),
            hull,
            symbol_type,
        }
    }
    pub fn is_blank(&self) -> bool {
        matches!(self.symbol_type, SymbolType::Blank(_, _))
    }

    fn generate_hull(symbol_type: &SymbolType ) -> parry2d::shape::ConvexPolygon {
        let points =
        match &symbol_type {
            SymbolType::Blank(w, h) => {
                let hw = w / 2.0;
                let hh = h / 2.0;
                vec![
                    Vec2::new(-hw, -hh),
                    Vec2::new(hw, -hh),
                    Vec2::new(hw, hh),
                    Vec2::new(-hw, hh),
                ]
            }
            SymbolType::Pathed(path) => {
                path.get_hull_points()
            }
        };
        if points.len() >= 3
            && let Some(hull) = parry2d::shape::ConvexPolygon::from_convex_hull(&gift_wrap(&points)) {
            return hull;
        }
        // A straight line (or a point, or nothing) has no area to wrap, so
        // use its bounding box, padded so layout still sees a little width
        // and height.
        const PAD: f32 = 0.02;
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for p in &points {
            min = min.min(*p);
            max = max.max(*p);
        }
        if points.is_empty() {
            (min, max) = (Vec2::ZERO, Vec2::ZERO);
        }
        let (min, max) = (min - Vec2::splat(PAD), max + Vec2::splat(PAD));
        parry2d::shape::ConvexPolygon::from_convex_hull(&[min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)])
            .expect("a padded box has area")
    }
    pub fn get_hull(&self) -> &parry2d::shape::ConvexPolygon {
        &self.hull
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    base: Rc<SymbolBase>,
    scale: f32,
    position: Vec2,
    rotation_rad: f32,
    color: String,
}

impl Symbol {
    pub fn new(path: StrokePath, scale: f32, position: Vec2, rotation_rad: f32) -> Symbol {
        let base = Rc::new(SymbolBase::new(SymbolType::Pathed(path)));
        Symbol::new_from_base_rc(base, scale, position, rotation_rad)
    }
    pub fn new_default_from_base_rc(base: Rc<SymbolBase>) -> Symbol {
        Symbol::new_from_base_rc(base, 1.0, Vec2::new(0.0, 0.0), 0.0)
    }
    pub fn new_from_base_rc(base: Rc<SymbolBase>, scale: f32, position: Vec2, rotation_rad: f32) -> Symbol {
        Symbol {
            base,
            scale,
            position,
            rotation_rad,
            color: "black".to_string(),
        }
    }
    pub fn symbol_type(&self) -> &SymbolType {
        &self.base.symbol_type
    }
    pub fn color(&self) -> &str {
        &self.color
    }
    pub fn set_color(&mut self, color: String) {
        self.color = color;
    }
    pub fn position(&self) -> Vec2 {
        self.position
    }
    pub fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }
    pub fn translate_by(&mut self, delta: Vec2) {
        self.position += delta;
    }
    pub fn scale_by(&mut self, scale: f32) {
        self.scale *= scale;
    }
    pub fn rotate_rad_by(&mut self, delta: f32) {
        self.rotation_rad += delta;
    }
    pub fn new_blank(width: f32, height: f32, scale: f32, position: Vec2, rotation_rad: f32) -> Symbol {
        let base = Rc::new(SymbolBase::new(SymbolType::Blank(width, height)));
        Symbol::new_from_base_rc(base, scale, position, rotation_rad)
    }
    fn placed_path(&self, path: &StrokePath) -> StrokePath {
        path.clone()
            .rotated_by(self.rotation_rad)
            .scaled_by(self.scale / 2.0)
            .translated_by(self.position)
    }
    pub fn get_strokepath(&self) -> StrokePath {
        let symbol_base = self.base.as_ref();
        match &symbol_base.symbol_type {
            SymbolType::Blank(_, _) => StrokePath::new(),
            SymbolType::Pathed(path) => path.clone(),
        }
    }
    pub fn get_substrokepaths(&self) -> Vec<SubstrokePath> {
        self.get_strokepath().to_substrokes()

    }
    pub fn placed_strokepath(&self) -> StrokePath {
        let symbol_base = self.base.as_ref();
        match &symbol_base.symbol_type {
            SymbolType::Blank(_, _) => StrokePath::new(),
            SymbolType::Pathed(path) => self.placed_path(path),
        }
    }
    pub fn placed_substrokepaths(&self) -> Vec<SubstrokePath> {
        self.placed_strokepath().to_substrokes()
    }

    pub fn sample_points_along(&self, step: f32) -> Vec<CurveSample> {
        let path = self.placed_strokepath();
        path.sample_points_along(step)
    }
    
    /// The bounding box of the unplaced glyph's hull (its path spans about
    /// -1..1), without copying the hull.
    pub fn hull_aabb(&self) -> parry2d::bounding_volume::Aabb {
        self.base.hull_aabb
    }

    pub fn rotation_rad(&self) -> f32 {
        self.rotation_rad
    }
    pub fn set_rotation_rad(&mut self, rotation: f32) {
        self.rotation_rad = rotation;
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }
    pub fn is_blank(&self) -> bool {
        self.base.is_blank()
    }
}

#[derive(Debug, Clone)]
pub struct SymbolPlacementArgs<'kd> {
    pub avoid_kdtree: Option<&'kd AvoidanceKdTree>,
    pub symbol_scale: f32,
}

// A wrapper around a k-d tree for avoidance purposes
#[derive(Debug, Clone)]
pub struct AvoidanceKdTree {
    pub kdtree: kdtree::KdTree<f32, (f32, f32), [f32; 2]>,
    pub scale: f32,
}
impl AvoidanceKdTree {
    pub fn new(kdtree: kdtree::KdTree<f32, (f32, f32), [f32; 2]>, scale: f32) -> Self {
        Self { kdtree, scale }
    }
    pub fn empty(scale: f32) -> Self {
        Self { kdtree: KdTree::new(2), scale }
    }
    pub fn add_point(&mut self, point: Vec2) {
        if point.x.is_nan() || point.y.is_nan() {
            return
        }
        self.kdtree.add([point.x, point.y], (point.x, point.y)).unwrap();
    }
    pub fn contained_within(&self, point: Vec2) -> bool {
        let query_point = [point.x, point.y];
        let scale = self.scale;
        self.kdtree
            .nearest(&query_point, 1, &squared_euclidean)
            .is_ok_and(|results| {
                if let Some((dist, _)) = results.first() {
                    *dist < scale * scale
                } else {
                    false
                }
            })
    }
}

impl From<Symbol> for StrokePath {
    fn from(val: Symbol) -> Self {
        val.placed_strokepath()
    }
}

impl From<&Symbol> for StrokePath {
    fn from(val: &Symbol) -> Self {
        val.placed_strokepath()
    }
}

impl From<Vec<Symbol>> for StrokePath {
    fn from(val: Vec<Symbol>) -> Self {
        let mut combined_path = StrokePath::new();
        for symbol in val {
            let placed_path = symbol.placed_strokepath();
            combined_path = combined_path.merge_path(&placed_path);
        }
        combined_path
    }
}

fn orientation(p: Vec2, q: Vec2, r: Vec2) -> i32 {
    let val = (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);
    if val == 0.0 {
        0 // collinear
    } else if val > 0.0 {
        1 // clockwise
    } else {
        -1 // counterclockwise
    }
}

// the algorithm which generates the convex hull efficiently from a set of points
// is called the "gift wrapping" algorithm, which is O(n^2) in the worst case.
fn gift_wrap(points: &[Vec2]) -> Vec<Vec2> {
    let mut hull = Vec::new();
    let mut leftmost = 0;
    for i in 1..points.len() {
        if points[i].x < points[leftmost].x {
            leftmost = i;
        }
    }
    let mut p = leftmost;
    loop {
        hull.push(points[p]);
        let mut q = (p + 1) % points.len();
        for i in 0..points.len() {
            if orientation(points[p], points[q], points[i]) == -1 {
                q = i;
            }
        }
        p = q;
        if p == leftmost {
            break;
        }
    }
    hull
}

pub fn place_symbols_along_stepper<'a, SymIter, Stepper>(stepper: &mut Stepper, symbols: &mut Peekable<SymIter>, symbol_avoidtree: &mut AvoidanceKdTree, external_avoidtree: Option<&AvoidanceKdTree>, update_symbols: bool) -> usize where
    SymIter: Iterator<Item=&'a mut Symbol>,
    Stepper: CurveStepper,
{
    let scale = symbol_avoidtree.scale * 2.0;
    let mut placed_count = 0;
    while let Some(current_symbol) = symbols.peek_mut() {
        let aabb = current_symbol.hull_aabb();
        let width = aabb.extents().x * scale / 2.0;
        let step_size = width / 3.0;
        if let (Some(l), Some(r)) = (stepper.prospective_step(0.0), stepper.prospective_step(width)) {
            let center = l.point + (r.point - l.point) * 0.5;
            let mut skip = symbol_avoidtree.contained_within(center);
            skip |= match external_avoidtree {
                Some(tree) => tree.contained_within(center),
                None => false,
            };
            if skip {
                stepper.step(step_size);
            } else {
                let current_symbol = symbols.next().unwrap();
                if update_symbols {
                    current_symbol.set_position(center);
                    current_symbol.rotate_rad_by(l.normal.y.atan2(l.normal.x) - std::f32::consts::FRAC_PI_2);
                    current_symbol.scale_by(scale);
                }
                symbol_avoidtree.add_point(center);
                placed_count += 1;
                stepper.step(width);
            }
        } else {
            break;
        }
    }
    placed_count
}

pub fn place_symbols_along<'a, SymIter: Iterator<Item=&'a mut Symbol>>(substroke: &SubstrokePath, symbols: &mut Peekable<SymIter>, symbol_avoidtree: &mut AvoidanceKdTree, external_avoidtree: Option<&AvoidanceKdTree>) -> usize {
    let mut stepper = SubstrokeStepper::new(substroke, 20);
    place_symbols_along_stepper(&mut stepper, symbols, symbol_avoidtree, external_avoidtree, true)
}

pub fn place_along_path<'kd>(mut symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs<'kd>) -> (Vec<Symbol>, Vec<Symbol>) {
    let substrokes = path.to_substrokes();
    let SymbolPlacementArgs {
        avoid_kdtree: external_avoidtree,
        symbol_scale,
    } = args;
    let mut symbol_avoidtree = AvoidanceKdTree::empty(*symbol_scale / 2.0);
    let mut placed_symbols = 0;
    let mut sym_iter = symbols.iter_mut().peekable();
    for substroke in &substrokes {
        placed_symbols += place_symbols_along(substroke, &mut sym_iter, &mut symbol_avoidtree, *external_avoidtree);
    }
    // copy the last n placed symbols into a new vec,
    // then return the original vector, now truncated, and the new vec
    let unplaced_symbols = symbols.split_off(placed_symbols);
    (symbols, unplaced_symbols)
}

pub fn place_words_along_path<'kd>(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs<'kd>) -> (Vec<Symbol>, Vec<Symbol>) {
    let substrokes = path.to_substrokes();
    let SymbolPlacementArgs {
        avoid_kdtree: external_avoidtree,
        symbol_scale,
    } = args;

    let mut symbol_avoidtree = AvoidanceKdTree::empty(*symbol_scale / 2.0);
    let sym_iter = symbols.into_iter();
    let mut words: Vec<VecDeque<Symbol>> = Vec::new();
    let mut current_word: VecDeque<Symbol> = VecDeque::new();
    for symbol in sym_iter {
        let symbol_blank = symbol.is_blank();
        current_word.push_back(symbol);
        if symbol_blank
            && !current_word.is_empty() {
                words.push(current_word);
                current_word = VecDeque::new();
            }
    }
    if !current_word.is_empty() {
        words.push(current_word);
    }

    let mut stepper_iter = substrokes.iter().map(|s| SubstrokeStepper::new(s, 20));
    let mut words_iter = words.into_iter();

    let mut current_word = None;
    let mut current_stepper : Option<SubstrokeStepper> = None;

    let mut placed_symbols = Vec::new();
    let mut unplaced_symbols = Vec::new();

    loop {
        let mut new_stepper = false;
        current_word = current_word.or_else(|| words_iter.next());
        current_stepper = current_stepper.or_else(|| {new_stepper = true; stepper_iter.next()});

        if current_word.is_none() || current_stepper.is_none() {
            break;
        }

        let word = current_word.as_mut().unwrap();
        let stepper = current_stepper.as_mut().unwrap();

        let mut child_stepper = stepper.new_child();
        let mut word_sym_iter = word.iter_mut().peekable();
        let mut avoidtree_clone = symbol_avoidtree.clone();
        let placed = place_symbols_along_stepper(
            &mut child_stepper, &mut word_sym_iter,
            &mut avoidtree_clone, *external_avoidtree,
            false,
        );

        if word_sym_iter.peek().is_some() {
            if new_stepper {
                // We just started a new line, so if we can't place
                // the word here, it means it won't fit anywhere.
                // So, in this case, we allow a partial placement of
                // the word, and then remove the first N symbols
                // from the word vector.
                let mut current = current_word.take().unwrap();
                let next = current.split_off(placed);
                if !next.is_empty() && !next[0].is_blank() {
                    current_word = Some(next);
                }
                let mut word_sym_iter = current.iter_mut().peekable();
                place_symbols_along_stepper(
                    stepper, &mut word_sym_iter,
                    &mut symbol_avoidtree, *external_avoidtree,
                    true,
                );
                for symbol in current {
                    placed_symbols.push(symbol);
                }
            } else {
                // word did not fully fit, so we need discard our
                // stepper and substroke and try again on with the
                // next stepper.
                current_stepper = None;
            }
        } else {
            // the word was able to fully be placed, so we can move on
            // to the next word, and keep our progress on this substroke
            // by re-inserting the cloned stepper.
            let mut word_sym_iter = word.iter_mut().peekable();
            place_symbols_along_stepper(
                stepper, &mut word_sym_iter,
                &mut symbol_avoidtree, *external_avoidtree,
                true,
            );
            let placed_word = current_word.take().unwrap();
            for symbol in placed_word {
                placed_symbols.push(symbol);
            }
        }
    }
    for symbol in current_word.into_iter().flatten() {
        unplaced_symbols.push(symbol);
    }
    for word in words_iter {
        for symbol in word {
            unplaced_symbols.push(symbol);
        }
    }
    (placed_symbols, unplaced_symbols)
}
