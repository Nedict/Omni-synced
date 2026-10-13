//! Edge nodes: every edge of a body sampled once, so the faces on either side share the very
//! same boundary points (watertight by construction).

use solidcraft_geom::Vec3;
use truck_meshalgo::prelude::*;
use truck_modeling as mt;

use crate::body::from_p3;

/// Most nodes on one edge.
pub(crate) const MAX_EDGE_NODES: usize = 20_000;
/// Deepest bisection of one edge segment.
const MAX_DEPTH: u32 = 24;

/// Nodes of `edge` from its absolute front to its absolute back, every chord within `tol` of
/// the curve (checked at the quarter points, so an S-bend in one segment is not missed). The
/// end nodes are the vertices' own points, so edges meeting at a vertex meet exactly.
pub(crate) fn sample_edge(edge: &mt::Edge, tol: f64) -> Vec<Vec3> {
    let front = from_p3(edge.absolute_front().point());
    let back = from_p3(edge.absolute_back().point());
    let curve = edge.curve();
    if matches!(curve, mt::Curve::Line(_)) {
        return vec![front, back];
    }
    let (t0, t1) = curve.range_tuple();
    if !(t0.is_finite() && t1.is_finite() && t1 > t0) {
        return vec![front, back];
    }
    // Our evaluator where the curve has a form it knows (no allocation per point); truck's
    // for intersection curves.
    let fast = super::surface::eval_curve(&curve);
    let at = |t: f64| {
        match &fast {
            Some(c) => c.eval(t).map(|(p, _)| p),
            None => Some(from_p3(curve.subs(t))),
        }
        .filter(|p| p.is_finite())
    };
    // A closed curve's chord between its ends is empty: start from a few segments.
    let initial: u32 = 4;
    let t_at = |i: u32| t0 + (t1 - t0) * f64::from(i) / f64::from(initial);
    let mut ends: Vec<Vec3> = Vec::with_capacity(initial as usize + 1);
    for i in 0..=initial {
        let Some(p) = at(t_at(i)) else { return vec![front, back] };
        ends.push(p);
    }
    // Depth-first bisection, segment by segment, keeping the output ordered. Each segment
    // carries its end points and middle, so every parameter is evaluated once (an
    // intersection curve's point is a Newton solve).
    struct Seg {
        a: f64,
        b: f64,
        pa: Vec3,
        pb: Vec3,
        mid: Option<Vec3>,
        depth: u32,
    }
    let mut out: Vec<Vec3> = vec![front];
    let mut stack: Vec<Seg> = Vec::with_capacity(64);
    for i in (0..initial).rev() {
        let (Some(pa), Some(pb)) = (<[Vec3]>::get(&ends, i as usize).copied(), <[Vec3]>::get(&ends, i as usize + 1).copied()) else {
            return vec![front, back];
        };
        stack.push(Seg { a: t_at(i), b: t_at(i + 1), pa, pb, mid: None, depth: 0 });
    }
    while let Some(sg) = stack.pop() {
        let (a, b, pa, pb) = (sg.a, sg.b, sg.pa, sg.pb);
        let m = 0.5 * (a + b);
        let pm = match sg.mid {
            Some(p) => Some(p),
            None => at(m),
        };
        let (q1, q3) = (at(a + 0.25 * (b - a)), at(a + 0.75 * (b - a)));
        let done = sg.depth >= MAX_DEPTH || out.len() + stack.len() >= MAX_EDGE_NODES;
        let fine = || [q1, pm, q3].iter().all(|q| q.is_none_or(|q| q.dist_to_segment(pa, pb) <= tol));
        match pm {
            Some(pm) if !done && !fine() => {
                stack.push(Seg { a: m, b, pa: pm, pb, mid: q3, depth: sg.depth + 1 });
                stack.push(Seg { a, b: m, pa, pb: pm, mid: q1, depth: sg.depth + 1 });
            }
            _ => out.push(pb),
        }
    }
    if out.len() < 2 {
        return vec![front, back];
    }
    if let Some(p) = out.last_mut() {
        *p = back;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_edges_are_within_tolerance_and_end_on_vertices() {
        let Ok(cyl) = crate::cylinder(Vec3::ZERO, Vec3::Z, 10.0, 5.0) else { panic!("cylinder") };
        for tol in [0.1, 0.01, 0.001] {
            for e in cyl.solid.edge_iter() {
                let pts = sample_edge(&e, tol);
                assert!(pts.len() >= 2);
                assert_eq!(pts.first().copied(), Some(from_p3(e.absolute_front().point())));
                assert_eq!(pts.last().copied(), Some(from_p3(e.absolute_back().point())));
                if matches!(e.curve(), mt::Curve::Line(_)) {
                    assert_eq!(pts.len(), 2);
                    continue;
                }
                // Circle of radius 10: every chord's middle sags at most `tol` (plus rounding).
                for w in pts.windows(2) {
                    let m = (w[0] + w[1]) * 0.5;
                    let r = (m.x * m.x + m.y * m.y).sqrt();
                    assert!(10.0 - r <= tol * 1.0001, "sag {} at tol {tol}", 10.0 - r);
                }
            }
        }
    }
}
