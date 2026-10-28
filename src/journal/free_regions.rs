//! Free regions: the stroke-free space inside an area, as closed
//! outlines text can be written into.
//!
//! The area is cut into a grid of square cells; a cell is free where its
//! centre is inside the area and clear of what's avoided. Free space
//! narrower than a least width is taken away (an opening: what's further
//! than half that width from anything blocked, grown back by as much), so
//! thin necks part regions. Free cells joined side to side are one
//! region; its outline runs through the middles of the sides it shares
//! with cells that aren't in it, each corner cut, and is thinned to the
//! points it needs.

use std::collections::HashMap;

use parry2d::math::Vec2;

use crate::journal::avoid::AvoidanceTree;
use crate::journal::canvas::textbox::Region;

pub struct FreeRegionOptions {
    /// The cells' size.
    pub resolution: f32,
    /// How far outlines keep from what's avoided.
    pub clearance: f32,
    /// Free space narrower than this is left out.
    pub min_width: f32,
    /// Regions smaller than this are left out.
    pub min_area: f32,
    /// How far a thinned outline may stray from the traced one.
    pub tolerance: f32,
}

/// A region of free space: its outline, then the outlines of its holes,
/// and how much room it has (its holes taken away).
#[derive(Debug, Clone)]
pub struct FreeRegion {
    pub rings: Vec<Vec<Vec2>>,
    pub area: f32,
}

/// The most cells a grid has, so a fine resolution over a large area
/// can't run away.
pub const MAX_CELLS: usize = 4_000_000;

/// The free regions inside `area` (closed outlines, one inside another a
/// hole), clear of `avoid`, largest first.
pub fn free_regions(area: &[Vec<Vec2>], avoid: &AvoidanceTree, o: &FreeRegionOptions) -> Result<Vec<FreeRegion>, String> {
    let points = area.iter().flatten();
    let (min, max) = points.fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    if !(min.x < max.x && min.y < max.y) {
        return Ok(Vec::new());
    }
    let res = o.resolution;
    let (nx, ny) = (((max.x - min.x) / res).ceil() as usize, ((max.y - min.y) / res).ceil() as usize);
    if nx.saturating_mul(ny) > MAX_CELLS {
        return Err(format!("a resolution of {} cuts the area into {} x {} cells, more than {}; make it coarser", res, nx, ny, MAX_CELLS));
    }
    let grid = Grid { min, res, nx, ny };
    let free = grid.free(area, avoid, o.clearance);
    let groups = if o.min_width > 0.0 { grid.wide_components(&free, o.min_width / 2.0) } else { grid.components(&free) };
    let mut regions: Vec<FreeRegion> = groups.into_iter()
        .filter_map(|cells| {
            let rings: Vec<Vec<Vec2>> = grid.outlines(&cells).into_iter()
                .map(|ring| thin(&ring, o.tolerance))
                .filter(|ring| ring.len() >= 3)
                .collect();
            // The outline winds one way round, holes the other.
            let area = rings.iter().map(|ring| signed_area(ring)).sum::<f32>().abs();
            (area >= o.min_area && !rings.is_empty()).then_some(FreeRegion { rings, area })
        })
        .collect();
    regions.sort_by(|a, b| b.area.total_cmp(&a.area));
    Ok(regions)
}

/// Twice the area a closed outline winds round, positive anticlockwise
/// (x right, y up).
fn signed_area(ring: &[Vec2]) -> f32 {
    ring.iter().enumerate().map(|(i, a)| a.perp_dot(ring[(i + 1) % ring.len()])).sum::<f32>() / 2.0
}

struct Grid {
    min: Vec2,
    res: f32,
    nx: usize,
    ny: usize,
}

impl Grid {
    fn center(&self, i: usize, j: usize) -> Vec2 {
        self.min + Vec2::new(i as f32 + 0.5, j as f32 + 0.5) * self.res
    }

    /// Whether each cell is free: its centre inside the area, half a cell
    /// in from its edge, and half a cell further than `clearance` from
    /// what's avoided, so outlines, which run half a cell from free
    /// centres, keep inside and clear.
    fn free(&self, area: &[Vec<Vec2>], avoid: &AvoidanceTree, clearance: f32) -> Vec<bool> {
        let region = Region::new(area.to_vec());
        let mut edge = AvoidanceTree::new();
        for ring in area {
            edge.add_polyline(ring, true, 0.0);
        }
        let half = self.res / 2.0;
        let mut free = vec![false; self.nx * self.ny];
        for j in 0..self.ny {
            for i in 0..self.nx {
                let p = self.center(i, j);
                // A hair short of half a cell, so a grid lined up with the area's
                // edge keeps its outermost cells.
                free[j * self.nx + i] = region.contains(p) && !edge.blocks(&[p], half * 0.999) && !avoid.blocks(&[p], clearance + half);
            }
        }
        free
    }

    /// The free cells in groups, free space narrower than twice `radius`
    /// left out and parting them: the cores further than `radius` from
    /// anything blocked, joined side to side, each grown back over the free
    /// cells within `radius` of a core, the nearer core (along the free
    /// cells) taking those two could reach.
    fn wide_components(&self, free: &[bool], radius: f32) -> Vec<Vec<bool>> {
        let r2 = (radius / self.res).powi(2);
        // Beyond the grid counts as blocked.
        let from_blocked = distances(free.iter().map(|&f| !f).collect(), self.nx, self.ny, true);
        let core: Vec<bool> = from_blocked.iter().map(|&d| d > r2).collect();
        let from_core = distances(core.clone(), self.nx, self.ny, false);
        let room: Vec<bool> = free.iter().zip(&from_core).map(|(&f, &d)| f && d <= r2).collect();
        let cores = self.components(&core);
        let mut label = vec![usize::MAX; free.len()];
        let mut queue = std::collections::VecDeque::new();
        for (n, cells) in cores.iter().enumerate() {
            for k in (0..cells.len()).filter(|&k| cells[k]) {
                label[k] = n;
                queue.push_back(k);
            }
        }
        while let Some(k) = queue.pop_front() {
            let (i, j) = (k % self.nx, k / self.nx);
            let mut reach = |m: usize| if room[m] && label[m] == usize::MAX { label[m] = label[k]; queue.push_back(m) };
            if i > 0 { reach(k - 1) }
            if i + 1 < self.nx { reach(k + 1) }
            if j > 0 { reach(k - self.nx) }
            if j + 1 < self.ny { reach(k + self.nx) }
        }
        (0..cores.len()).map(|n| label.iter().map(|&l| l == n).collect()).collect()
    }

    /// The free cells in groups joined side to side.
    fn components(&self, free: &[bool]) -> Vec<Vec<bool>> {
        let mut seen = vec![false; free.len()];
        let mut groups = Vec::new();
        for start in 0..free.len() {
            if !free[start] || seen[start] {
                continue;
            }
            let mut group = vec![false; free.len()];
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(k) = stack.pop() {
                group[k] = true;
                let (i, j) = (k % self.nx, k / self.nx);
                let mut visit = |n: usize| if free[n] && !seen[n] { seen[n] = true; stack.push(n) };
                if i > 0 { visit(k - 1) }
                if i + 1 < self.nx { visit(k + 1) }
                if j > 0 { visit(k - self.nx) }
                if j + 1 < self.ny { visit(k + self.nx) }
            }
            groups.push(group);
        }
        groups
    }

    /// The closed outlines of a group of cells, through the middles of the
    /// cell sides between it and what isn't in it: the group on each one's
    /// left (x right, y up), so the outer one winds anticlockwise and
    /// holes clockwise.
    fn outlines(&self, cells: &[bool]) -> Vec<Vec<Vec2>> {
        let inside = |i: i64, j: i64| i >= 0 && j >= 0 && (i as usize) < self.nx && (j as usize) < self.ny && cells[j as usize * self.nx + i as usize];
        // Sides, from corner to corner, by where they start.
        let mut sides: Vec<((i64, i64), (i64, i64))> = Vec::new();
        for j in 0..self.ny as i64 {
            for i in 0..self.nx as i64 {
                if !inside(i, j) {
                    continue;
                }
                if !inside(i, j - 1) { sides.push(((i, j), (i + 1, j))) }
                if !inside(i + 1, j) { sides.push(((i + 1, j), (i + 1, j + 1))) }
                if !inside(i, j + 1) { sides.push(((i + 1, j + 1), (i, j + 1))) }
                if !inside(i - 1, j) { sides.push(((i, j + 1), (i, j))) }
            }
        }
        let mut from: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (k, side) in sides.iter().enumerate() {
            from.entry(side.0).or_default().push(k);
        }
        let mut used = vec![false; sides.len()];
        let mut rings = Vec::new();
        for first in 0..sides.len() {
            if used[first] {
                continue;
            }
            let mut ring = Vec::new();
            let mut k = first;
            loop {
                used[k] = true;
                let (a, b) = sides[k];
                ring.push(self.min + Vec2::new((a.0 + b.0) as f32 / 2.0, (a.1 + b.1) as f32 / 2.0) * self.res);
                let heading = (b.0 - a.0, b.1 - a.1);
                // Where two sides go on from a corner, cells meeting only
                // there, turn left: round this cell, apart from the other.
                let next = from.get(&b).into_iter().flatten().copied()
                    .filter(|&n| !used[n] || n == first)
                    .max_by_key(|&n| {
                        let (c, d) = sides[n];
                        let turn = (d.0 - c.0, d.1 - c.1);
                        heading.0 * turn.1 - heading.1 * turn.0
                    });
                match next {
                    Some(n) if n != first => k = n,
                    _ => break,
                }
            }
            rings.push(ring);
        }
        rings
    }
}

/// Squared distances, in cells, from each cell to the nearest marked one
/// (exactly, by Felzenszwalb and Huttenlocher's two passes), the grid's
/// surroundings counted as marked if `edge_marked`.
fn distances(marked: Vec<bool>, nx: usize, ny: usize, edge_marked: bool) -> Vec<f32> {
    const FAR: f32 = 1e20;
    // Padded by a cell all round, marked or not as the surroundings are.
    let (w, h) = (nx + 2, ny + 2);
    let mut d = vec![if edge_marked { 0.0 } else { FAR }; w * h];
    for j in 0..ny {
        for i in 0..nx {
            d[(j + 1) * w + i + 1] = if marked[j * nx + i] { 0.0 } else { FAR };
        }
    }
    let mut line = Vec::new();
    for i in 0..w {
        line.clear();
        line.extend((0..h).map(|j| d[j * w + i]));
        for (j, v) in squared_distances_1d(&line).into_iter().enumerate() {
            d[j * w + i] = v;
        }
    }
    for j in 0..h {
        let row = squared_distances_1d(&d[j * w..(j + 1) * w]);
        d[j * w..(j + 1) * w].copy_from_slice(&row);
    }
    let mut out = vec![0.0; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            out[j * nx + i] = d[(j + 1) * w + i + 1];
        }
    }
    out
}

/// The lower envelope of the parabolas `f[q] + (p - q)^2`.
fn squared_distances_1d(f: &[f32]) -> Vec<f32> {
    let n = f.len();
    let mut out = vec![0.0; n];
    let (mut v, mut z) = (vec![0usize; n], vec![0f32; n + 1]);
    let mut k = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    let cross = |q: usize, p: usize| ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * q as f32 - 2.0 * p as f32);
    for q in 1..n {
        let mut s = cross(q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = cross(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, slot) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let dq = q as f32 - v[k] as f32;
        *slot = dq * dq + f[v[k]];
    }
    out
}

/// A closed outline thinned to the points it needs to keep within
/// `tolerance` of it (Douglas-Peucker, from its two furthest-apart
/// points).
pub fn thin(ring: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    if ring.len() < 4 || tolerance <= 0.0 {
        return ring.to_vec();
    }
    let far = (1..ring.len()).max_by(|&a, &b| ring[0].distance_squared(ring[a]).total_cmp(&ring[0].distance_squared(ring[b]))).unwrap_or(0);
    let mut keep = vec![false; ring.len()];
    keep[0] = true;
    keep[far] = true;
    let open: Vec<Vec2> = ring.iter().chain(ring.first()).copied().collect();
    thin_between(&open, 0, far, tolerance, &mut keep);
    let mut tail = vec![false; open.len()];
    thin_between(&open, far, ring.len(), tolerance, &mut tail);
    for (k, kept) in tail.into_iter().enumerate().take(ring.len()) {
        keep[k] |= kept;
    }
    ring.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

fn thin_between(points: &[Vec2], a: usize, b: usize, tolerance: f32, keep: &mut [bool]) {
    if b <= a + 1 {
        return;
    }
    let (p, q) = (points[a], points[b]);
    let off = |r: Vec2| {
        let pq = q - p;
        let t = if pq.length_squared() > 0.0 { ((r - p).dot(pq) / pq.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        r.distance(p + pq * t)
    };
    let (worst, distance) = (a + 1..b).map(|k| (k, off(points[k]))).fold((a, 0.0), |best, x| if x.1 > best.1 { x } else { best });
    if distance > tolerance {
        keep[worst] = true;
        thin_between(points, a, worst, tolerance, keep);
        thin_between(points, worst, b, tolerance, keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(c: Vec2, h: f32) -> Vec<Vec2> {
        vec![c + Vec2::new(-h, -h), c + Vec2::new(h, -h), c + Vec2::new(h, h), c + Vec2::new(-h, h)]
    }

    fn options(resolution: f32) -> FreeRegionOptions {
        FreeRegionOptions { resolution, clearance: 0.0, min_width: 0.0, min_area: 0.0, tolerance: resolution / 2.0 }
    }

    #[test]
    fn an_empty_area_is_one_region() {
        let regions = free_regions(&[square(Vec2::ZERO, 50.0)], &AvoidanceTree::new(), &options(2.0)).unwrap();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].rings.len(), 1);
        assert!(regions[0].area > 0.95 * 100.0 * 100.0 && regions[0].area <= 100.0 * 100.0, "{}", regions[0].area);
        let region = Region::new(regions[0].rings.clone());
        assert!(region.contains(Vec2::ZERO) && !region.contains(Vec2::new(51.0, 0.0)));
    }

    #[test]
    fn a_blot_in_the_middle_is_a_hole() {
        let mut avoid = AvoidanceTree::new();
        avoid.add_polygon(&square(Vec2::ZERO, 10.0), 0.0);
        let regions = free_regions(&[square(Vec2::ZERO, 50.0)], &avoid, &FreeRegionOptions { clearance: 3.0, ..options(1.0) }).unwrap();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].rings.len(), 2, "an outline and a hole");
        let region = Region::new(regions[0].rings.clone());
        assert!(!region.contains(Vec2::ZERO), "the blot is out");
        assert!(!region.contains(Vec2::new(12.0, 0.0)), "and its clearance");
        assert!(region.contains(Vec2::new(30.0, 0.0)));
        // Every outline point keeps the clearance.
        for p in regions[0].rings.iter().flatten() {
            assert!(!avoid.blocks(&[*p], 3.0 - 1e-3), "{:?} is too near the blot", p);
        }
    }

    #[test]
    fn a_wall_parts_two_rooms() {
        let mut avoid = AvoidanceTree::new();
        avoid.add_segment(Vec2::new(0.0, -50.0), Vec2::new(0.0, 50.0), 0.0);
        let regions = free_regions(&[square(Vec2::ZERO, 50.0)], &avoid, &FreeRegionOptions { clearance: 2.0, ..options(1.0) }).unwrap();
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn min_width_parts_rooms_at_a_neck() {
        // A wall with a gap 6 wide in its middle.
        let mut avoid = AvoidanceTree::new();
        avoid.add_segment(Vec2::new(0.0, -50.0), Vec2::new(0.0, -3.0), 0.0);
        avoid.add_segment(Vec2::new(0.0, 3.0), Vec2::new(0.0, 50.0), 0.0);
        let area = [square(Vec2::ZERO, 50.0)];
        let joined = free_regions(&area, &avoid, &options(0.5)).unwrap();
        assert_eq!(joined.len(), 1);
        let parted = free_regions(&area, &avoid, &FreeRegionOptions { min_width: 10.0, ..options(0.5) }).unwrap();
        assert_eq!(parted.len(), 2);
        // The rooms keep their corners: the opening only takes away necks.
        assert!(parted[0].area > 0.9 * 50.0 * 100.0, "{}", parted[0].area);
    }

    #[test]
    fn min_area_leaves_out_specks_and_regions_come_largest_first() {
        let mut avoid = AvoidanceTree::new();
        avoid.add_segment(Vec2::new(20.0, -50.0), Vec2::new(20.0, 50.0), 0.0);
        avoid.add_segment(Vec2::new(45.0, -50.0), Vec2::new(45.0, 50.0), 0.0);
        let area = [square(Vec2::ZERO, 50.0)];
        let all = free_regions(&area, &avoid, &options(1.0)).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all[0].area > all[1].area && all[1].area > all[2].area);
        let big = free_regions(&area, &avoid, &FreeRegionOptions { min_area: 1000.0, ..options(1.0) }).unwrap();
        assert_eq!(big.len(), 2);
    }

    #[test]
    fn diagonal_cells_are_apart() {
        let grid = Grid { min: Vec2::ZERO, res: 1.0, nx: 2, ny: 2 };
        let cells = vec![true, false, false, true];
        assert_eq!(grid.components(&cells).len(), 2);
        // Both cells of a group meeting at a corner get an outline each.
        assert_eq!(grid.outlines(&cells).len(), 2);
    }

    #[test]
    fn thinning_keeps_corners() {
        let mut ring = Vec::new();
        for k in 0..10 { ring.push(Vec2::new(k as f32, 0.0)) }
        for k in 0..10 { ring.push(Vec2::new(10.0, k as f32)) }
        for k in 0..10 { ring.push(Vec2::new(10.0 - k as f32, 10.0)) }
        for k in 0..10 { ring.push(Vec2::new(0.0, 10.0 - k as f32)) }
        let thinned = thin(&ring, 0.1);
        assert_eq!(thinned.len(), 4, "{:?}", thinned);
    }
}
