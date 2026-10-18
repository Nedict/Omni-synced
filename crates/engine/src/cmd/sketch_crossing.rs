//! Where two sketch curves visibly cross: the shared finder behind the crossing snaps. It reuses
//! the Trim/Break pair finders, so a crossing here is exactly a point Trim would cut at.

use solidcraft_geom::{Seg2, Vec2};
use solidcraft_sketch::Sketch;

use super::sketch_freeform::{cuts_pair, pieces};
use super::sketch_modify::cut_pair;

/// Tangent directions closer than this (sin of 0.5°) touch rather than cross.
const PARALLEL: f64 = 0.008_726_6;

/// A visible crossing of two different curves of a sketch (indices into `sk.curves`, a < b).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub at: Vec2,
    pub a: usize,
    pub b: usize,
}

fn infinite(sk: &Sketch, ci: usize) -> bool {
    sk.curves.get(ci).is_some_and(|c| c.infinite)
}

/// Unit tangent of curve `ci` at `q`: exact for line/round, else the nearest polyline segment's.
fn tangent_at(sk: &Sketch, ci: usize, q: Vec2) -> Option<Vec2> {
    if let Some(sh) = sk.shape(ci) {
        return Some(sh.tangent(sh.param(q)));
    }
    let poly = sk.polyline(ci);
    let best = poly.windows(2).min_by(|x, y| seg_dist(x, q).total_cmp(&seg_dist(y, q)))?;
    (*best.get(1)? - *best.first()?).normalized()
}

fn seg_dist(w: &[Vec2], q: Vec2) -> f64 {
    match (w.first(), w.get(1)) {
        (Some(&a), Some(&b)) => {
            let s = Seg2::Line { a, b };
            s.point_at(s.closest_param(q)).dist(q)
        }
        _ => f64::INFINITY,
    }
}

/// Distance from `p` to curve `ci` as drawn (infinite lines run on forever).
fn dist_to(sk: &Sketch, ci: usize, p: Vec2) -> f64 {
    match sk.shape(ci) {
        Some(sh) if infinite(sk, ci) => sh.project(p).dist(p),
        Some(sh) => sh.dist(p),
        None => sk.polyline(ci).windows(2).map(|w| seg_dist(w, p)).fold(f64::INFINITY, f64::min),
    }
}

/// Visible, non-tangent crossings of curves `a` and `b` (empty when a == b or either is unknown).
pub fn crossings_of(sk: &Sketch, a: usize, b: usize) -> Vec<Vec2> {
    if a == b || a >= sk.curves.len() || b >= sk.curves.len() {
        return Vec::new();
    }
    let pts: Vec<Vec2> = match (sk.shape(a), sk.shape(b)) {
        (Some(sa), Some(sb)) => cut_pair(sk, &sa, infinite(sk, a), b, &sb, false).into_iter().map(|(_, q)| q).collect(),
        _ => {
            // Free-form pair: cut the free-form one (an infinite line runs on forever).
            let (ci, oi) = if sk.shape(a).is_none() { (a, b) } else { (b, a) };
            let segs = pieces(sk, ci);
            cuts_pair(sk, ci, &sk.polyline(ci), &segs, oi, false, true).into_iter().map(|(_, q)| q).collect()
        }
    };
    pts.into_iter()
        .filter(|&q| match (tangent_at(sk, a, q), tangent_at(sk, b, q)) {
            (Some(ta), Some(tb)) => ta.cross(tb).abs() >= PARALLEL,
            _ => false,
        })
        .collect()
}

/// The crossing nearest `at`, within `tol` (sketch units), among curves passing within `tol` of `at`.
pub fn crossing_near(sk: &Sketch, at: Vec2, tol: f64) -> Option<Crossing> {
    let near: Vec<usize> = (0..sk.curves.len()).filter(|&ci| dist_to(sk, ci, at) <= tol).collect();
    let mut best: Option<(f64, Crossing)> = None;
    for (n, &a) in near.iter().enumerate() {
        for &b in near.iter().skip(n + 1) {
            for q in crossings_of(sk, a, b) {
                let d = q.dist(at);
                if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, Crossing { at: q, a, b }));
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use solidcraft_sketch::CurveKind;

    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    fn line(sk: &mut Sketch, a: (f64, f64), b: (f64, f64)) -> usize {
        let pa = sk.add_point(v(a.0, a.1), None).unwrap();
        let pb = sk.add_point(v(b.0, b.1), None).unwrap();
        sk.add_line_pts(pa, pb, None).unwrap()
    }

    fn spline(sk: &mut Sketch, pts: &[(f64, f64)]) -> usize {
        let pts = pts.iter().map(|p| sk.add_point(v(p.0, p.1), None).unwrap()).collect();
        sk.add_curve(CurveKind::Spline { pts, control: false, degree: 3, periodic: false }, None).unwrap()
    }

    #[test]
    fn two_lines_cross_where_they_cross() {
        let mut sk = Sketch::new();
        line(&mut sk, (0.0, 0.0), (10.0, 10.0));
        line(&mut sk, (0.0, 10.0), (10.0, 0.0));
        let c = crossing_near(&sk, v(5.1, 5.0), 0.5).unwrap();
        assert!(c.at.dist(v(5.0, 5.0)) < 1e-9 && c.a < c.b);
    }

    #[test]
    fn extensions_do_not_cross() {
        let mut sk = Sketch::new();
        let a = line(&mut sk, (0.0, 0.0), (4.0, 4.0));
        let b = line(&mut sk, (0.0, 10.0), (10.0, 0.0));
        assert!(crossings_of(&sk, a, b).is_empty());
        assert!(crossing_near(&sk, v(5.0, 5.0), 1.0).is_none());
    }

    #[test]
    fn infinite_line_crosses_beyond_its_points() {
        let mut sk = Sketch::new();
        let a = line(&mut sk, (0.0, 0.0), (1.0, 0.0));
        sk.curves[a].infinite = true;
        let b = line(&mut sk, (5.0, -1.0), (5.0, 1.0));
        let x = crossings_of(&sk, a, b);
        assert_eq!(x.len(), 1);
        assert!(x[0].dist(v(5.0, 0.0)) < 1e-9);
        assert!(crossing_near(&sk, v(5.0, 0.1), 0.5).is_some());
    }

    #[test]
    fn nearest_of_two_close_crossings_wins() {
        let mut sk = Sketch::new();
        sk.add_circle(v(0.0, 0.0), 1.0, None, None).unwrap();
        line(&mut sk, (-3.0, 0.0), (3.0, 0.0));
        assert!(crossing_near(&sk, v(0.6, 0.1), 2.0).unwrap().at.dist(v(1.0, 0.0)) < 1e-9);
        assert!(crossing_near(&sk, v(-0.6, 0.1), 2.0).unwrap().at.dist(v(-1.0, 0.0)) < 1e-9);
    }

    #[test]
    fn arc_crossing_outside_its_sweep_is_ignored() {
        let mut sk = Sketch::new();
        sk.add_arc(v(0.0, 0.0), v(5.0, 0.0), v(0.0, 5.0), [None; 3], None).unwrap();
        // The line meets the circle at x = -4.97, outside the 0..90 degree sweep.
        line(&mut sk, (-10.0, 0.5), (0.0, 0.5));
        assert!(crossing_near(&sk, v(-4.97, 0.5), 1.0).is_none());
        // Inside the sweep it does cross.
        line(&mut sk, (0.0, 2.0), (10.0, 2.0));
        let c = crossing_near(&sk, v(4.5, 2.0), 1.0).unwrap();
        assert!(c.at.dist(v(21.0f64.sqrt(), 2.0)) < 1e-9);
    }

    #[test]
    fn tangent_touch_is_not_a_crossing() {
        let mut sk = Sketch::new();
        let a = sk.add_circle(v(0.0, 0.0), 5.0, None, None).unwrap();
        let b = line(&mut sk, (-10.0, 5.0), (10.0, 5.0));
        assert!(crossings_of(&sk, a, b).is_empty());
    }

    #[test]
    fn spline_crosses_a_line() {
        let mut sk = Sketch::new();
        let s = spline(&mut sk, &[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)]);
        let l = line(&mut sk, (0.0, 2.0), (10.0, 2.0));
        let x = crossings_of(&sk, s, l);
        assert_eq!(x.len(), 2);
        let segs = sk.exact_segs(s).unwrap();
        for q in x {
            assert!((q.y - 2.0).abs() < 1e-6);
            let d = segs.iter().map(|g| g.point_at(g.closest_param(q)).dist(q)).fold(f64::INFINITY, f64::min);
            assert!(d < 1e-6, "off the spline by {d}");
        }
    }

    #[test]
    fn infinite_line_crosses_a_spline_beyond_its_points() {
        let mut sk = Sketch::new();
        let s = spline(&mut sk, &[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)]);
        let l = line(&mut sk, (-3.0, 2.0), (-2.0, 2.0));
        sk.curves[l].infinite = true;
        assert_eq!(crossings_of(&sk, s, l).len(), 2);
        assert!(crossing_near(&sk, v(1.5, 2.0), 1.0).is_some());
    }

    #[test]
    fn overlap_offers_no_crossing_inside_it() {
        let mut sk = Sketch::new();
        let a = line(&mut sk, (0.0, 0.0), (10.0, 0.0));
        let b = line(&mut sk, (2.0, 0.0), (8.0, 0.0));
        assert!(crossings_of(&sk, a, b).is_empty());
        assert!(crossing_near(&sk, v(5.0, 0.0), 0.5).is_none());
    }

    #[test]
    fn a_curve_does_not_cross_itself() {
        let mut sk = Sketch::new();
        let s = spline(&mut sk, &[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]);
        assert!(crossings_of(&sk, s, s).is_empty());
        for i in 0..=10 {
            for j in 0..=10 {
                assert!(crossing_near(&sk, v(i as f64, j as f64), 3.0).is_none());
            }
        }
    }
}
