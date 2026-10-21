//! A tree of stars cut into constellations, a page each: pieces of the
//! tree that hold together, neighbouring pieces sharing the star they meet
//! at, so laid over each other on their shared stars the pages make the
//! whole. Each piece is fitted to the page on its own, and every star gets
//! a spot for its label clear of the links, the other stars and the other
//! labels.
//!
//! The tree is cut leaves up: a star hands its parent the links below it
//! not yet in a piece, and once a star holds about a piece's worth of
//! links, they're a constellation round that star, which its parent's
//! piece shares. What reaches the root is the last piece (a crumb too
//! small for one joins the last piece round the root). The piece's worth
//! is whichever gives the number of pieces asked for, or nearest it.

use parry2d::math::Vec2;

use super::geom::V;
use super::starchart::to_segment;

/// Where each constellation goes and how much room its labels need.
pub struct ConstellationSettings {
    /// The middle of the box each constellation is fitted to.
    pub center: Vec2,
    pub width: f64,
    pub height: f64,
    /// A label's box, kept clear of links, stars and other labels.
    pub label_width: f64,
    pub label_height: f64,
    /// How far from its star's edge a label sits, at least.
    pub label_gap: f64,
    /// Stars whose constellation is split in two at them, its branches
    /// from the star divided by the way they go into two runs about as big.
    pub split_at: Vec<usize>,
    /// The most stars a constellation may have: more pieces than `count`
    /// if that's what it takes.
    pub most_stars: Option<usize>,
}

pub struct Constellation {
    /// The tree's stars in it, by index, in index order.
    pub stars: Vec<usize>,
    /// Its links, [parent, child] index pairs into `stars`.
    pub pairs: Vec<(usize, usize)>,
    /// Where each star of `stars` goes, fitted to the box.
    pub points: Vec<Vec2>,
    /// Where the middle of each star's label goes.
    pub labels: Vec<Vec2>,
    /// How much the tree's positions were scaled by to fit, across and down.
    pub scale: Vec2,
}

pub fn constellations(points: &[Vec2], radii: &[f64], edges: &[(usize, usize)], count: usize, s: &ConstellationSettings) -> Result<Vec<Constellation>, String> {
    let n = points.len();
    if n == 0 {
        return Err("there are no stars".into());
    }
    if radii.len() != n {
        return Err(format!("there are {} stars but {} radii", n, radii.len()));
    }
    if count == 0 {
        return Err("count must be at least 1".into());
    }
    if !(s.width > 0.0 && s.height > 0.0 && s.width.is_finite() && s.height.is_finite()) {
        return Err(format!("width and height must be positive numbers, got {} and {}", s.width, s.height));
    }
    if !(s.label_width >= 0.0 && s.label_height >= 0.0 && s.label_gap >= 0.0) {
        return Err("label_width, label_height and label_gap must be at least 0".into());
    }
    let (children, root) = tree(n, edges)?;
    if let Some(most) = s.most_stars {
        if most < 2 {
            return Err(format!("most_stars must be at least 2, got {}", most));
        }
    }
    let mut pieces = cut_best(&children, root, count, s.most_stars);
    let mut parent = vec![usize::MAX; n];
    for &(a, b) in edges {
        parent[b] = a;
    }
    for &star in &s.split_at {
        if star >= n {
            return Err(format!("split_at names star {}, past the last, {}", star, n - 1));
        }
        split(&mut pieces, star, &parent, points)?;
    }
    let settings_box = (V::from(s.center), s.width / 2.0, s.height / 2.0);
    Ok(pieces.iter().map(|piece| lay_out(piece, points, radii, edges_of(piece, &children, root), settings_box, s)).collect())
}

/// Each star's children and the root, checking the links make one tree.
fn tree(n: usize, edges: &[(usize, usize)]) -> Result<(Vec<Vec<usize>>, usize), String> {
    if edges.len() + 1 != n {
        return Err(format!("{} stars need {} links to make a tree, got {}", n, n - 1, edges.len()));
    }
    let mut parent = vec![None; n];
    let mut children = vec![Vec::new(); n];
    for &(a, b) in edges {
        if a >= n || b >= n {
            return Err(format!("the link [{}, {}] names a star past the last, {}", a, b, n - 1));
        }
        if parent[b].is_some() {
            return Err(format!("star {} has two parents", b));
        }
        parent[b] = Some(a);
        children[a].push(b);
    }
    let root = (0..n).find(|&i| parent[i].is_none()).ok_or("the links go round in a loop: no star is the root")?;
    // Every star must be reached from the root.
    let mut seen = 0;
    let mut stack = vec![root];
    while let Some(v) = stack.pop() {
        seen += 1;
        stack.extend(&children[v]);
        if seen > n {
            break;
        }
    }
    if seen != n {
        return Err("the links don't join every star into one tree".into());
    }
    Ok((children, root))
}

/// The stars below and including `root`, children before parents.
fn post_order(children: &[Vec<usize>], root: usize) -> Vec<usize> {
    let mut order = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((v, done)) = stack.pop() {
        if done {
            order.push(v);
        } else {
            stack.push((v, true));
            for &c in children[v].iter().rev() {
                stack.push((c, false));
            }
        }
    }
    order
}

/// The tree cut into pieces of about `worth` links each; a link is named
/// by its child.
fn cut(children: &[Vec<usize>], root: usize, worth: usize) -> Vec<Vec<usize>> {
    let n = children.len();
    let mut pieces: Vec<Vec<usize>> = Vec::new();
    let mut left: Vec<Vec<usize>> = vec![Vec::new(); n];
    for v in post_order(children, root) {
        let mut group = Vec::new();
        for &c in &children[v] {
            group.push(c);
            group.append(&mut left[c]);
            if group.len() >= worth {
                pieces.push(std::mem::take(&mut group));
            }
        }
        left[v] = group;
    }
    let rest = std::mem::take(&mut left[root]);
    if !rest.is_empty() {
        let touches_root = |piece: &Vec<usize>| piece.iter().any(|c| children[root].contains(c));
        match pieces.iter().rposition(touches_root) {
            Some(last) if rest.len() * 2 < worth => pieces[last].extend(rest),
            _ => pieces.push(rest),
        }
    }
    pieces
}

/// The cut nearest `count` pieces, the most even of those, of those whose
/// pieces have at most `most_stars` stars (a piece's stars are its links
/// and one more). Cutting a link to a piece always fits.
fn cut_best(children: &[Vec<usize>], root: usize, count: usize, most_stars: Option<usize>) -> Vec<Vec<usize>> {
    let links = children.len() - 1;
    if links == 0 {
        return vec![Vec::new()];
    }
    let fits = |pieces: &Vec<Vec<usize>>| most_stars.is_none_or(|most| pieces.iter().all(|p| p.len() + 1 <= most));
    (1..=links).map(|worth| cut(children, root, worth)).filter(fits)
        .min_by_key(|pieces| (pieces.len().abs_diff(count), pieces.iter().map(|p| p.len()).max().unwrap_or(0)))
        .unwrap()
}

/// The piece holding `star` with two or more branches from it split in
/// two, each keeping `star`: its branches (the piece's parts that meet only
/// at `star`) taken round `star` by the way they go, from the widest gap
/// between them, and cut into two runs with about as many links each.
fn split(pieces: &mut Vec<Vec<usize>>, star: usize, parent: &[usize], points: &[Vec2]) -> Result<(), String> {
    // A piece's branches from `star`: its links grouped by the part they're
    // in once `star` is taken out.
    let branches = |piece: &Vec<usize>| -> Vec<Vec<usize>> {
        let mut group: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let find = |x: usize, group: &mut std::collections::HashMap<usize, usize>| {
            let mut x = x;
            while let Some(&up) = group.get(&x) {
                if up == x { break; }
                x = up;
            }
            x
        };
        for &c in piece {
            for v in [c, parent[c]] {
                group.entry(v).or_insert(v);
            }
        }
        for &c in piece {
            if c != star && parent[c] != star {
                let (a, b) = (find(c, &mut group), find(parent[c], &mut group));
                group.insert(a, b);
            }
        }
        let mut by_root: Vec<(usize, Vec<usize>)> = Vec::new();
        for &c in piece {
            let other = if c == star { parent[c] } else { c };
            let r = find(other, &mut group);
            match by_root.iter_mut().find(|(k, _)| *k == r) {
                Some((_, links)) => links.push(c),
                None => by_root.push((r, vec![c])),
            }
        }
        by_root.into_iter().map(|(_, links)| links).collect()
    };
    let found = pieces.iter().position(|piece| piece.iter().any(|&c| c == star || parent[c] == star) && branches(piece).len() >= 2)
        .ok_or_else(|| format!("star {} has no constellation with two branches from it to split", star))?;
    let piece = pieces[found].clone();
    let at = V::from(points[star]);
    // Each branch's way: toward the middle of its stars.
    let mut ways: Vec<(f64, Vec<usize>)> = branches(&piece).into_iter().map(|links| {
        let stars: Vec<V> = links.iter().flat_map(|&c| [c, parent[c]]).filter(|&v| v != star).map(|v| V::from(points[v])).collect();
        let middle = stars.iter().fold(V::default(), |m, &p| m + p) * (1.0 / stars.len() as f64) - at;
        (middle.y.atan2(middle.x), links)
    }).collect();
    ways.sort_by(|a, b| a.0.total_cmp(&b.0));
    let m = ways.len();
    // Start after the widest gap, so each run goes one way.
    let gap = (0..m).max_by(|&i, &j| {
        let g = |i: usize| if i + 1 < m { ways[i + 1].0 - ways[i].0 } else { ways[0].0 + std::f64::consts::TAU - ways[i].0 };
        g(i).total_cmp(&g(j))
    }).unwrap();
    ways.rotate_left((gap + 1) % m);
    let total: usize = ways.iter().map(|w| w.1.len()).sum();
    let cut_after = (1..m).min_by_key(|&k| {
        let first: usize = ways[..k].iter().map(|w| w.1.len()).sum();
        first.abs_diff(total - first)
    }).unwrap();
    let first: Vec<usize> = ways[..cut_after].iter().flat_map(|w| w.1.clone()).collect();
    let second: Vec<usize> = ways[cut_after..].iter().flat_map(|w| w.1.clone()).collect();
    // Left, by the middle of its stars, first.
    let x = |links: &Vec<usize>| links.iter().map(|&c| points[c].x as f64 + points[parent[c]].x as f64).sum::<f64>() / links.len() as f64;
    let (left, right) = if x(&first) <= x(&second) { (first, second) } else { (second, first) };
    pieces.splice(found..=found, [left, right]);
    Ok(())
}

/// A piece's links as [parent, child] stars.
fn edges_of(piece: &[usize], children: &[Vec<usize>], root: usize) -> Vec<(usize, usize)> {
    if piece.is_empty() {
        return vec![(root, root)];
    }
    let mut parent = vec![usize::MAX; children.len()];
    for (v, cs) in children.iter().enumerate() {
        for &c in cs {
            parent[c] = v;
        }
    }
    let mut edges: Vec<(usize, usize)> = piece.iter().map(|&c| (parent[c], c)).collect();
    edges.sort_by_key(|&(_, c)| c);
    edges
}

/// How many times more one way may be scaled than the other.
const STRETCH: f64 = 4.0;

/// A piece fitted to the box, across and down, its labels placed.
fn lay_out(_piece: &[usize], points: &[Vec2], radii: &[f64], edges: Vec<(usize, usize)>, (center, a, b): (V, f64, f64), s: &ConstellationSettings) -> Constellation {
    let mut stars: Vec<usize> = edges.iter().flat_map(|&(p, c)| [p, c]).collect();
    stars.sort();
    stars.dedup();
    let local = |g: usize| stars.binary_search(&g).unwrap();
    let pairs: Vec<(usize, usize)> = edges.iter().filter(|(p, c)| p != c).map(|&(p, c)| (local(p), local(c))).collect();
    let at: Vec<V> = stars.iter().map(|&g| V::from(points[g])).collect();
    let r: Vec<f64> = stars.iter().map(|&g| radii[g]).collect();

    // Fit it, labels' room and all, to the box.
    let biggest = r.iter().copied().fold(0.0, f64::max);
    let margin = biggest + s.label_gap + s.label_width.max(s.label_height);
    let (lo, hi) = at.iter().fold((V::new(f64::MAX, f64::MAX), V::new(f64::MIN, f64::MIN)), |(lo, hi), p| {
        (V::new(lo.x.min(p.x), lo.y.min(p.y)), V::new(hi.x.max(p.x), hi.y.max(p.y)))
    });
    let room = |half: f64| (2.0 * half - 2.0 * margin).max(1e-6);
    let span = |l: f64, h: f64| if h - l > 1e-9 { h - l } else { 0.0 };
    // Across and down each scaled to fill the box, so a cluster gets the
    // room its way: an affine stretch, so links still don't cross. Neither
    // way is stretched more than STRETCH times the other.
    let fill = |half: f64, l: f64, h: f64| if span(l, h) > 0.0 { room(half) / span(l, h) } else { f64::MAX };
    let (mut sx, mut sy) = (fill(a, lo.x, hi.x), fill(b, lo.y, hi.y));
    if sx == f64::MAX && sy == f64::MAX {
        (sx, sy) = (1.0, 1.0);
    }
    sx = sx.min(sy * STRETCH);
    sy = sy.min(sx * STRETCH);
    let scale = V::new(sx, sy);
    let middle = (lo + hi) * 0.5;
    let fitted: Vec<V> = at.iter().map(|&p| center + V::new((p.x - middle.x) * scale.x, (p.y - middle.y) * scale.y)).collect();

    // Each label where it's clearest of links, stars and labels placed
    // before it, inside the box; a little toward the upper right, all else
    // equal.
    let (hw, hh) = (s.label_width / 2.0, s.label_height / 2.0);
    let mut labels: Vec<V> = Vec::new();
    for (k, &p) in fitted.iter().enumerate() {
        let mut best = (f64::MIN, p);
        for step in 0..32 {
            let angle = step as f64 / 32.0 * std::f64::consts::TAU;
            let dir = V::angle(angle);
            // Far enough out that the box clears the star whichever way.
            let reach = r[k] + s.label_gap + (dir.x.abs() * hw + dir.y.abs() * hh);
            let q = p + dir * reach;
            let mut clear = f64::MAX;
            for &(i, j) in &pairs {
                clear = clear.min(box_to_segment(q, hw, hh, fitted[i], fitted[j]));
            }
            for (m, &o) in fitted.iter().enumerate() {
                if m != k {
                    clear = clear.min(box_to_point(q, hw, hh, o) - r[m]);
                }
            }
            for &l in &labels {
                clear = clear.min(((l.x - q.x).abs() - 2.0 * hw).max((l.y - q.y).abs() - 2.0 * hh));
            }
            let outside = ((q.x - hw) - (center.x - a)).min((center.x + a) - (q.x + hw))
                .min((q.y - hh) - (center.y - b)).min((center.y + b) - (q.y + hh));
            let score = clear.min(4.0 * s.label_gap.max(1.0)) + outside.min(0.0) * 10.0 + 0.5 * (angle + std::f64::consts::FRAC_PI_4).cos();
            if score > best.0 {
                best = (score, q);
            }
        }
        labels.push(best.1);
    }
    let out = |v: &V| Vec2::new(v.x as f32, v.y as f32);
    Constellation { stars, pairs, points: fitted.iter().map(out).collect(), labels: labels.iter().map(out).collect(), scale: out(&scale) }
}

/// How far `p` is from the box of half-width `hw` and half-height `hh`
/// about `c` (0 inside).
fn box_to_point(c: V, hw: f64, hh: f64, p: V) -> f64 {
    let dx = ((p.x - c.x).abs() - hw).max(0.0);
    let dy = ((p.y - c.y).abs() - hh).max(0.0);
    dx.hypot(dy)
}

/// How far the segment ab is from the box about `c` (0 where they meet).
fn box_to_segment(c: V, hw: f64, hh: f64, a: V, b: V) -> f64 {
    let corners = [V::new(c.x - hw, c.y - hh), V::new(c.x + hw, c.y - hh), V::new(c.x + hw, c.y + hh), V::new(c.x - hw, c.y + hh)];
    let inside = |p: V| (p.x - c.x).abs() <= hw && (p.y - c.y).abs() <= hh;
    if inside(a) || inside(b) || (0..4).any(|i| super::starchart::segments_meet(a, b, corners[i], corners[(i + 1) % 4])) {
        return 0.0;
    }
    let to_corners = corners.iter().map(|&q| to_segment(q, a, b)).fold(f64::MAX, f64::min);
    to_corners.min(box_to_point(c, hw, hh, a)).min(box_to_point(c, hw, hh, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::art::{huffman_stars, StarChartSettings};

    fn chart(seed: u64) -> (Vec<Vec2>, Vec<f64>, Vec<(usize, usize)>) {
        let text: Vec<u32> = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG".chars().map(|c| if c == ' ' { 0 } else { c as u32 - 'A' as u32 + 1 }).collect();
        let s = StarChartSettings { center: Vec2::new(0.0, 0.0), width: 2000.0, height: 1500.0, round: false, smoothing: 1, variation: 0.6, iterations: 100, star_size: Some(20.0), gap: None, seed };
        let c = huffman_stars(&text, 27, 3, &s).unwrap();
        (c.stars.iter().map(|s| s.position).collect(), c.stars.iter().map(|s| s.radius).collect(), c.edges)
    }

    fn settings() -> ConstellationSettings {
        ConstellationSettings { center: Vec2::new(600.0, 600.0), width: 1000.0, height: 900.0, label_width: 30.0, label_height: 24.0, label_gap: 6.0, split_at: Vec::new(), most_stars: None }
    }

    #[test]
    fn pieces_cover_the_tree_once_and_hold_together() {
        let (points, radii, edges) = chart(3);
        for count in [1, 2, 4, 5] {
            let pieces = constellations(&points, &radii, &edges, count, &settings()).unwrap();
            assert!(pieces.len().abs_diff(count) <= 1, "asked {}, got {}", count, pieces.len());
            // Every link in exactly one piece.
            let mut links: Vec<(usize, usize)> = pieces.iter().flat_map(|p| p.pairs.iter().map(|&(a, b)| (p.stars[a], p.stars[b]))).collect();
            links.sort();
            let mut all = edges.clone();
            all.sort();
            assert_eq!(links, all);
            for p in &pieces {
                // A piece is connected: one star has no parent in it.
                let roots = (0..p.stars.len()).filter(|&i| !p.pairs.iter().any(|&(_, c)| c == i)).count();
                assert_eq!(roots, 1, "a piece should hold together");
                assert_eq!(p.points.len(), p.stars.len());
                assert_eq!(p.labels.len(), p.stars.len());
            }
            // Pieces meet at shared stars: with more than one, each shares one.
            if pieces.len() > 1 {
                for (i, p) in pieces.iter().enumerate() {
                    let shares = pieces.iter().enumerate().any(|(j, q)| j != i && p.stars.iter().any(|s| q.stars.contains(s)));
                    assert!(shares, "piece {} shares no star", i);
                }
            }
        }
    }

    #[test]
    fn pieces_fill_the_box_and_labels_keep_clear() {
        let (points, radii, edges) = chart(5);
        let s = settings();
        for p in constellations(&points, &radii, &edges, 4, &s).unwrap() {
            let xs: Vec<f32> = p.points.iter().map(|q| q.x).collect();
            let ys: Vec<f32> = p.points.iter().map(|q| q.y).collect();
            let w = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
            let h = ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min);
            // Fitted: it reaches across most of the box one way or the other.
            assert!(w > 700.0 || h > 600.0, "{} by {}", w, h);
            for (k, l) in p.labels.iter().enumerate() {
                let c = V::from(*l);
                for &(i, j) in &p.pairs {
                    let d = box_to_segment(c, 15.0, 12.0, V::from(p.points[i]), V::from(p.points[j]));
                    assert!(d > 0.0, "star {}'s label touches a link", k);
                }
            }
        }
    }

    #[test]
    fn a_constellation_splits_in_two_at_a_star() {
        let (points, radii, edges) = chart(3);
        let whole = constellations(&points, &radii, &edges, 4, &settings()).unwrap();
        let split = constellations(&points, &radii, &edges, 4, &ConstellationSettings { split_at: vec![0], ..settings() }).unwrap();
        assert_eq!(split.len(), whole.len() + 1);
        let holding: Vec<&Constellation> = split.iter().filter(|p| p.stars.contains(&0)).collect();
        let before = whole.iter().filter(|p| p.stars.contains(&0)).count();
        assert_eq!(holding.len(), before + 1, "the root's constellation should be two");
        // The two halves sit left and right of the root.
        let half = |p: &Constellation| p.stars.iter().map(|&s| points[s].x).sum::<f32>() / p.stars.len() as f32;
        let new: Vec<&&Constellation> = holding.iter().filter(|p| !whole.iter().any(|w| w.stars == p.stars)).collect();
        assert_eq!(new.len(), 2);
        assert!(half(new[0]) < half(new[1]));
        let err = constellations(&points, &radii, &edges, 4, &ConstellationSettings { split_at: vec![999], ..settings() }).err().unwrap();
        assert!(err.contains("past the last"), "{}", err);
        let leaf = (0..points.len()).find(|&i| !edges.iter().any(|&(a, _)| a == i)).unwrap();
        let err = constellations(&points, &radii, &edges, 4, &ConstellationSettings { split_at: vec![leaf], ..settings() }).err().unwrap();
        assert!(err.contains("no constellation with two branches"), "{}", err);
    }

    #[test]
    fn constellations_keep_under_most_stars() {
        let (points, radii, edges) = chart(4);
        for most in [3, 6, 9] {
            let pieces = constellations(&points, &radii, &edges, 1, &ConstellationSettings { most_stars: Some(most), ..settings() }).unwrap();
            assert!(pieces.iter().all(|p| p.stars.len() <= most), "most {}: {:?}", most, pieces.iter().map(|p| p.stars.len()).collect::<Vec<_>>());
            assert!(pieces.len() > 1);
        }
        let err = constellations(&points, &radii, &edges, 1, &ConstellationSettings { most_stars: Some(1), ..settings() }).err().unwrap();
        assert!(err.contains("most_stars must be at least 2"), "{}", err);
    }

    #[test]
    fn bad_trees_are_named() {
        let pts = vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)];
        let r = vec![1.0; 3];
        let err = |edges: &[(usize, usize)]| constellations(&pts, &r, edges, 2, &settings()).err().unwrap();
        assert!(err(&[(0, 1)]).contains("need 2 links"));
        assert!(err(&[(0, 1), (2, 1)]).contains("two parents"));
        assert!(err(&[(0, 5), (0, 1)]).contains("past the last"));
        assert!(err(&[(1, 2), (2, 1)]).contains("don't join every star"));
        let one = constellations(&[Vec2::ZERO], &[1.0], &[], 3, &settings()).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].stars, vec![0]);
    }
}
