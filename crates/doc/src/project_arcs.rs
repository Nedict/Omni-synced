//! Opt-in seam removal for live face projections. Existing links keep their
//! original curve decomposition and identifiers.
use crate::{DocError, Result};
use solidcraft_geom::{Plane, Vec2, Vec3};
use solidcraft_sketch::LinkGeom;
use std::f64::consts::TAU;

fn endpoints(curve: &LinkGeom) -> Option<(Vec2, Vec2)> {
    match curve {
        LinkGeom::Line(a, b) | LinkGeom::Arc { a, b, .. } | LinkGeom::Conic { a, b, .. } => Some((*a, *b)),
        LinkGeom::Spline { pts, periodic: false, .. } => pts.first().zip(pts.last()).map(|(a, b)| (*a, *b)),
        _ => None,
    }
}

fn sweep(c: Vec2, a: Vec2, b: Vec2) -> f64 {
    ((b - c).angle() - (a - c).angle()).rem_euclid(TAU)
}

/// Use the kernel curve's circular carrier and directed span. A short arc may
/// have only two display samples, which cannot distinguish it from a chord.
/// The cached samples must still agree with the carrier; tilted circles use the
/// existing general projection path.
pub(super) fn project_circle(plane: &Plane, circle: solidcraft_kernel::EdgeCircle, points: &[Vec3]) -> Option<LinkGeom> {
    let solidcraft_kernel::EdgeCircle { center, axis, radius, start, end, angle } = circle;
    if !center.is_finite()
        || !axis.is_finite()
        || !start.is_finite()
        || !end.is_finite()
        || !radius.is_finite()
        || radius <= 1e-9
        || !angle.is_finite()
        || !(1e-9..=TAU).contains(&angle)
    {
        return None;
    }
    let direction = axis.dot(plane.normal());
    if (direction.abs() - 1.).abs() > 1e-10 {
        return None;
    }
    let tolerance = (1. + radius) * 1e-7 + center.len() * f64::EPSILON * 512.;
    if !tolerance.is_finite()
        || points.len() < 2
        || points
            .iter()
            .chain([&start, &end])
            .any(|point| !point.is_finite() || (point.dist(center) - radius).abs() > tolerance || (*point - center).dot(axis).abs() > tolerance)
    {
        return None;
    }
    let (a, b) = (plane.to_local(start), plane.to_local(end));
    let c = plane.to_local(center);
    if angle == TAU {
        return (start.dist(end) <= tolerance).then_some(LinkGeom::Circle(c, radius));
    }
    let (a, b) = if direction >= 0. { (a, b) } else { (b, a) };
    (a.dist(b) > 1e-9 && (sweep(c, a, b) - angle).abs() <= 1e-7).then_some(LinkGeom::Arc { c, a, b })
}

/// Coalesce contiguous, consistently oriented circular arcs at degree-two
/// stations. Branches and overlaps stay separate; a complete circle retains
/// complementary arcs rather than becoming a degenerate start=end arc.
/// The result is recomputed from the face on every edit; no seam position is saved.
pub fn join_face_arcs(mut geometry: Vec<LinkGeom>) -> Result<Vec<LinkGeom>> {
    let invalid = |g: &LinkGeom| match g {
        LinkGeom::Arc { c, a, b } => !c.is_finite() || !a.is_finite() || !b.is_finite() || c.dist(*a) <= 1e-9 || a.dist(*b) <= 1e-9,
        _ => endpoints(g).is_some_and(|(a, b)| !a.is_finite() || !b.is_finite()),
    };
    if geometry.len() > 128 || geometry.iter().any(invalid) {
        return Err(DocError::Invalid("invalid face arc geometry or work limit exceeded".into()));
    }
    let mut work = 0_usize;
    loop {
        let mut pair = None;
        'search: for (i, first) in geometry.iter().enumerate() {
            let LinkGeom::Arc { c, a, b } = first else { continue };
            for (j, second) in geometry.iter().enumerate() {
                let LinkGeom::Arc { c: other, a: start, b: end } = second else { continue };
                let tolerance = (1.0 + c.dist(*a).max(other.dist(*start))) * 1e-7 + c.len().max(other.len()) * f64::EPSILON * 512.;
                if !tolerance.is_finite() {
                    return Err(DocError::Invalid("non-finite face arc tolerance".into()));
                }
                if i == j || b.dist(*start) > tolerance || c.dist(*other) > tolerance || (c.dist(*a) - other.dist(*start)).abs() > tolerance {
                    continue;
                }
                work = work.saturating_add(geometry.len());
                if work > 2_000_000 {
                    return Err(DocError::Invalid("face arc incidence work limit exceeded".into()));
                }
                let incidence = geometry.iter().filter_map(endpoints).filter(|(p, q)| p.dist(*b) <= tolerance || q.dist(*b) <= tolerance).count();
                let total = sweep(*c, *a, *b) + sweep(*other, *start, *end);
                if incidence != 2 || a.dist(*end) <= tolerance || total >= TAU - 1e-7 || (sweep(*c, *a, *end) - total).abs() > 1e-7 {
                    continue;
                }
                pair = Some((i, j, LinkGeom::Arc { c: *c, a: *a, b: *end }));
                break 'search;
            }
        }
        let Some((i, j, joined)) = pair else { return Ok(geometry) };
        if let Some(slot) = geometry.get_mut(i) {
            *slot = joined;
        }
        geometry.remove(j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(angle: f64) -> Vec2 {
        Vec2::new(angle.cos(), angle.sin()) * 8.0
    }
    fn arc(a: f64, b: f64) -> LinkGeom {
        LinkGeom::Arc { c: Vec2::ZERO, a: point(a), b: point(b) }
    }

    #[test]
    fn circular_edges_preserve_short_spans_direction_and_major_sweeps() {
        let plane = Plane::new(Vec3::ZERO, Vec3::X, Vec3::Y).unwrap();
        for (start, end) in [(0., 0.008), (2., 6.), (6., 2.)] {
            let points = [plane.to_world(point(start)), plane.to_world(point(end))];
            let circle = solidcraft_kernel::EdgeCircle {
                center: Vec3::ZERO,
                axis: if end > start { Vec3::Z } else { -Vec3::Z },
                radius: 8.,
                start: points[0],
                end: points[1],
                angle: (end - start).abs(),
            };
            let LinkGeom::Arc { c, a, b } = project_circle(&plane, circle, &points).unwrap() else { panic!("circular edge") };
            assert!(c.len() < 1e-9);
            let (lo, hi) = (start.min(end), start.max(end));
            assert!(a.dist(point(lo)) < 1e-9 && b.dist(point(hi)) < 1e-9);
            assert!((sweep(c, a, b) - (hi - lo)).abs() < 1e-9);
            let invalid = solidcraft_kernel::EdgeCircle { angle: TAU - circle.angle, ..circle };
            assert!(project_circle(&plane, invalid, &points).is_none(), "the opposite sweep is not interchangeable");
            assert!(project_circle(&plane, circle, &[points[0], points[1] + Vec3::X * 0.01]).is_none());
        }
        let closed: Vec<_> = (0..=16).map(|i| plane.to_world(point(TAU * f64::from(i) / 16.))).collect();
        let circle = solidcraft_kernel::EdgeCircle { center: Vec3::ZERO, axis: Vec3::Z, radius: 8., start: closed[0], end: closed[0], angle: TAU };
        assert!(matches!(project_circle(&plane, circle, &closed), Some(LinkGeom::Circle(c,r)) if c.len()<1e-9 && (r-8.).abs()<1e-9));
        let tilted = Plane::new(Vec3::ZERO, Vec3::X, Vec3::Z).unwrap();
        assert!(project_circle(&tilted, circle, &closed).is_none());
    }

    #[test]
    fn partial_circle_crosses_seams_without_switching_to_the_short_arc() {
        let pieces = [arc(2.0, 3.0), arc(3.0, 5.0), arc(5.0, 7.0)];
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let result = join_face_arcs(order.map(|i| pieces[i].clone()).to_vec()).unwrap();
            assert_eq!(result, [arc(2.0, 7.0)]);
        }
    }

    #[test]
    fn branches_different_circles_overlaps_and_closed_circles_are_not_joined() {
        for geometry in [
            vec![arc(0.0, 1.0), arc(1.0, 2.0), LinkGeom::Line(point(1.0), Vec2::ZERO)],
            vec![arc(0.0, 1.0), arc(0.5, 2.0)],
            vec![arc(0.0, 4.0), arc(4.0, 7.0)],
            vec![arc(0.0, std::f64::consts::PI), arc(std::f64::consts::PI, TAU)],
            vec![arc(0.0, 1.0), LinkGeom::Arc { c: Vec2::X, a: point(1.0), b: point(2.0) }],
        ] {
            assert_eq!(join_face_arcs(geometry.clone()).unwrap(), geometry);
        }
        assert!(join_face_arcs(vec![arc(0.0, 1.0); 129]).is_err());
        assert!(join_face_arcs(vec![LinkGeom::Arc { c: Vec2::ZERO, a: Vec2::ZERO, b: Vec2::X }]).is_err());
    }

    #[test]
    fn translation_does_not_loosen_circle_identity() {
        let origin = Vec2::new(1_000_000., 0.);
        let geometry = vec![
            LinkGeom::Arc { c: origin, a: origin + point(0.), b: origin + point(1.) },
            LinkGeom::Arc { c: origin + Vec2::X * 0.01, a: origin + point(1.), b: origin + point(2.) },
        ];
        assert_eq!(join_face_arcs(geometry.clone()).unwrap(), geometry);
    }
}
