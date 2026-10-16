//! Exact topology for a contained cylindrical tool opening through a whole cap.
//! Eligibility is restricted to closed convex coaxial plane/cylinder/cone bodies.
//! Every side band and complete circular cap is checked; displaced, crossing,
//! hollow, partial-angle and non-axial bodies stay on the general Boolean path.
use solidcraft_geom::Vec3;

use crate::body::{Body, from_p3};
use crate::loopblend::{Seg, seg_of};
use crate::offset::{Surf, surf_of};
use crate::{Result, guard};

#[derive(Clone, Debug)]
struct Band {
    low: f64,
    high: f64,
    r0: f64,
    r1: f64,
    angle: f64,
    spans: Vec<(f64, f64)>,
}
impl Band {
    fn radius(&self, z: f64) -> f64 {
        self.r0 + (self.r1 - self.r0) * (z - self.low) / (self.high - self.low)
    }
}
#[derive(Debug)]
struct Axial {
    low: f64,
    high: f64,
    bands: Vec<Band>,
}

fn radial(point: Vec3, origin: Vec3, axis: Vec3) -> Vec3 {
    let d = point - origin;
    d - axis * d.dot(axis)
}

fn full_turn(spans: &[(f64, f64)]) -> bool {
    let tau = std::f64::consts::TAU;
    let mut pieces = Vec::new();
    for (start, sweep) in spans {
        if !start.is_finite() || !sweep.is_finite() || *sweep <= 0. || *sweep > tau + 1e-7 {
            return false;
        }
        let start = start.rem_euclid(tau);
        let end = start + sweep;
        pieces.push((start, end.min(tau)));
        if end > tau {
            pieces.push((0., end - tau));
        }
    }
    pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut end = 0.;
    for (low, high) in pieces {
        if (low - end).abs() > 1e-7 {
            return false;
        }
        end = high;
    }
    (end - tau).abs() <= 1e-7
}

/// A full body of revolution whose meridian is the lower envelope of its side
/// lines. Its occupied section is a disc at every height between the two caps.
fn axial(body: &Body, origin: Vec3, axis: Vec3, tol: f64) -> Option<Axial> {
    if body.solid.boundaries().len() != 1 || !(3..=64).contains(&body.face_count()) {
        return None;
    }
    let mut caps = [None, None];
    let mut bands: Vec<Band> = Vec::new();
    for face in body.solid.face_iter() {
        let surface = surf_of(face, tol)?;
        let boundaries = face.boundaries();
        let [wire] = boundaries.as_slice() else { return None };
        if wire.len() > 8 {
            return None;
        }
        let mut circles = Vec::new();
        let mut lines = 0;
        for edge in wire.edge_iter() {
            match seg_of(edge, tol)? {
                Seg::Arc { a, c, axis: normal, angle, .. } => {
                    if normal.dot(axis).abs() < 1. - 1e-9 || radial(c, origin, axis).len() > tol {
                        return None;
                    }
                    let u = axis.any_perp();
                    let v = axis.cross(u);
                    let radial = a - c;
                    let phase = radial.dot(v).atan2(radial.dot(u));
                    let start = if normal.dot(axis) < 0. { phase - angle } else { phase };
                    circles.push(((c - origin).dot(axis), a.dist(c), angle, start));
                }
                Seg::Line { a, b } => {
                    if radial(a, origin, axis).normalized()?.dot(radial(b, origin, axis).normalized()?) < 1. - 1e-9 {
                        return None;
                    }
                    lines += 1;
                }
            }
        }
        match surface {
            Surf::Plane { n, d } => {
                if n.dot(axis).abs() < 1. - 1e-9 || lines != 0 || circles.is_empty() {
                    return None;
                }
                let high = n.dot(axis) > 0.;
                let z = (d - n.dot(origin)) / n.dot(axis);
                let r = circles.first()?.1;
                if r <= tol
                    || circles.iter().any(|(h, radius, _, _)| (h - z).abs() > tol || (radius - r).abs() > tol)
                    || !full_turn(&circles.iter().map(|c| (c.3, c.2)).collect::<Vec<_>>())
                {
                    return None;
                }
                let slot = caps.get_mut(usize::from(high))?;
                if slot.replace((z, r)).is_some() {
                    return None;
                }
            }
            Surf::Cylinder { o, a, convex: true, .. } | Surf::Cone { v: o, a, convex: true, .. } => {
                if a.dot(axis).abs() < 1. - 1e-9 || radial(o, origin, axis).len() > tol || lines != 2 || circles.len() != 2 {
                    return None;
                }
                circles.sort_by(|a, b| a.0.total_cmp(&b.0));
                let [(low, r0, a0, start), (high, r1, a1, _)] = circles.as_slice() else { return None };
                if high - low <= tol || (a0 - a1).abs() > 1e-7 {
                    return None;
                }
                let radius = |z: f64| match surface {
                    Surf::Cylinder { r, .. } => r,
                    Surf::Cone { v, a, half, .. } => (origin + axis * z - v).dot(a) * half.tan(),
                    _ => f64::NAN,
                };
                if !radius(*low).is_finite() || !radius(*high).is_finite() || (radius(*low) - r0).abs() > tol || (radius(*high) - r1).abs() > tol {
                    return None;
                }
                let band = Band { low: *low, high: *high, r0: *r0, r1: *r1, angle: *a0, spans: vec![(*start, *a0)] };
                if let Some(old) = bands.iter_mut().find(|old| {
                    (old.low - low).abs() <= tol && (old.high - high).abs() <= tol && (old.r0 - r0).abs() <= tol && (old.r1 - r1).abs() <= tol
                }) {
                    old.angle += band.angle;
                    old.spans.extend(band.spans);
                } else {
                    bands.push(band);
                }
            }
            _ => return None,
        }
    }
    let [Some((low, low_r)), Some((high, high_r))] = caps else { return None };
    if high - low <= tol || bands.is_empty() {
        return None;
    }
    bands.sort_by(|a, b| a.low.total_cmp(&b.low));
    let mut z = low;
    for band in &bands {
        if (band.low - z).abs() > tol || (band.angle - std::f64::consts::TAU).abs() > 1e-7 || !full_turn(&band.spans) || band.r0.min(band.r1) <= tol {
            return None;
        }
        z = band.high;
        // All side patches must lie in every other convex supporting cylinder
        // or cone. Coaxial meridians are linear, so endpoints bound each patch.
        for other in &bands {
            if band.r0 > other.radius(band.low) + tol || band.r1 > other.radius(band.high) + tol {
                return None;
            }
        }
    }
    if (z - high).abs() > tol || (bands.first()?.r0 - low_r).abs() > tol || (bands.last()?.r1 - high_r).abs() > tol {
        return None;
    }
    // Check the whole boundary topology against those meridians, not a cached
    // sampled mesh. Circle edges were verified above; straight edges are convex.
    for vertex in body.solid.vertex_iter() {
        let point = from_p3(vertex.point());
        let z = (point - origin).dot(axis);
        let r = radial(point, origin, axis).len();
        if z < low - tol || z > high + tol || bands.iter().any(|band| r > band.radius(z) + tol) {
            return None;
        }
    }
    Some(Axial { low, high, bands })
}

pub(crate) fn contained_cap_cut(a: &Body, b: &Body) -> Option<Result<Body>> {
    guard("contained circular cap cut", || {
        if a.face_count() > 64 || b.face_count() > 64 {
            return Ok(None);
        }
        let tol = (a.size().max(b.size()) * 1e-7).max(1e-9) * 10.;
        let axis = b.solid.face_iter().find_map(|f| match surf_of(f, tol) {
            Some(Surf::Cylinder { o, a, r, convex: true }) => Some((o, a, r)),
            _ => None,
        });
        let Some((origin, axis, radius)) = axis else { return Ok(None) };
        let (Some(target), Some(tool)) = (axial(a, origin, axis, tol), axial(b, origin, axis, tol)) else { return Ok(None) };
        if tool.bands.iter().any(|band| (band.r0 - radius).abs() > tol || (band.r1 - radius).abs() > tol) {
            return Ok(None);
        }
        let low = tool.low <= target.low + tol;
        let high = tool.high >= target.high - tol;
        if low == high {
            return Ok(None);
        }
        let (opening, inside) = if low { (target.low, tool.high) } else { (target.high, tool.low) };
        if inside <= target.low + tol || inside >= target.high - tol {
            return Ok(None);
        }
        let cap_radius = target.bands.iter().map(|band| band.radius(opening)).fold(f64::INFINITY, f64::min);
        if (cap_radius - radius).abs() > tol
            || target.bands.iter().any(|band| band.radius(opening) < radius - tol || band.radius(inside) <= radius + tol)
        {
            return Ok(None);
        }
        // Cut extrusions deliberately extend beyond their starting face. That
        // portion is wholly beyond a target support plane, so removing it cannot
        // change the subtraction. Keep the inward end and compose the trim's
        // history back to the original tool before returning the stitched body.
        let delta = if low { tool.low - opening } else { opening - tool.high };
        if delta < -tol {
            let normal = if low { -axis } else { axis };
            let height = if low { tool.low } else { tool.high };
            let clipped = super::push_faces(b, normal, normal.dot(origin + axis * height), delta)?;
            let Some(result) = super::glue_faces(a, &clipped, true).transpose()? else { return Ok(None) };
            let history = crate::boolean_record::through(&result, 1, clipped.history())
                .ok_or_else(|| crate::KernelError::Failed("contained cap cut lost tool history".into()))?;
            return result.with_history(history, &[a, b]).map(Some);
        }
        super::glue_faces(a, b, true).transpose()
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chamfered(axis: Vec3, top: bool, flip: bool) -> Body {
        let body = crate::build::cylinder(Vec3::ZERO, axis, 20., 40.).unwrap();
        let mesh = body.tessellate(0.01).unwrap();
        let z = if top { 40. } else { 0. };
        let picks: Vec<_> =
            mesh.edges.iter().filter(|edge| edge.len() > 2 && edge.iter().all(|p| (p.dot(axis) - z).abs() < 1e-7)).map(|e| e[e.len() / 2]).collect();
        crate::chamfer_sides(&body, &picks, 2., crate::ChamferSide::Distance(3.), flip).unwrap()
    }

    #[test]
    fn contained_cap_cuts_keep_exact_surfaces_and_history() {
        for axis in [Vec3::Z, Vec3::Y, Vec3::new(1., 2., 3.).normalized().unwrap()] {
            for top in [false, true] {
                for flip in [false, true] {
                    let a = chamfered(axis, top, flip);
                    let (cap, wall) = if flip { (3., 2.) } else { (2., 3.) };
                    // Keep the source cap's seam partition, as a linked face
                    // extrusion does. Other partitions use the general Boolean.
                    let padding = if flip { 0.7 } else { 0. };
                    let tool = crate::build::cylinder(axis * if top { 35. } else { -padding }, axis, 20. - cap, 5. + padding).unwrap();
                    let signatures = tool
                        .face_sources()
                        .unwrap()
                        .iter()
                        .map(|source| matches!(source, Some(crate::FaceSource::ExtrudeCap { .. })).then(|| (vec!["circle".into()], vec![])))
                        .collect::<Vec<_>>();
                    let tool = tool.with_extrude_cap_certificates(88, 87, &signatures).unwrap();
                    let exact = contained_cap_cut(&a, &tool)
                        .unwrap_or_else(|| {
                            panic!(
                                "not eligible {axis:?}/{top}/{flip}: {:?} {:?}",
                                axial(&a, Vec3::ZERO, axis, 1e-4),
                                axial(&tool, Vec3::ZERO, axis, 1e-4)
                            )
                        })
                        .unwrap();
                    assert!(exact.validity().is_empty(), "{axis:?}/{top}/{flip}: {:?}", exact.validity());
                    let h = exact.history().unwrap();
                    assert_eq!(h.faces.len(), a.face_count() + tool.face_count() - 2);
                    assert_eq!(h.operands, 2);
                    h.check(&exact, &[&a, &tool]).unwrap();
                    let (result, report) = crate::capture_boolean_diagnostics(|| crate::boolean(&a, &tool, crate::BoolOp::Cut));
                    let result = result.unwrap().unwrap();
                    let caps = result
                        .face_sources()
                        .unwrap()
                        .iter()
                        .enumerate()
                        .filter_map(|(index, source)| {
                            if let Some(crate::FaceSource::CertifiedExtrudeCap { producer: 88, high, plane, .. }) = source {
                                Some((index, *high, *plane))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(caps.len(), 1, "the single exposed tool cap retains its identity");
                    assert_eq!(caps[0].1, !top);
                    let actual = result.planar_face(caps[0].0).unwrap();
                    assert!(actual.normal().dot(caps[0].2.normal()) > 1. - 1e-10);
                    assert!(report.events.iter().all(|event| !event.strategy.alters_result()));
                    let pi = std::f64::consts::PI;
                    let removed = pi * wall * (20. * cap - cap * cap / 3.);
                    let bore = pi * (20. - cap).powi(2) * 5.;
                    let expected = pi * 20. * 20. * 40. - removed - bore;
                    let measured = crate::measure(&result).unwrap();
                    assert!((measured.volume - expected).abs() < expected * 2e-4, "{axis:?}/{top}/{flip}: {} != {expected}", measured.volume);
                    assert!(result.validity().is_empty());
                    // Every surviving source surface keeps its own operand origin;
                    // the pocket walls/floor are reversed tool faces, never samples.
                    assert!(h.faces.iter().all(|face| !face.origins.is_empty()));
                }
            }
        }
    }

    #[test]
    fn circular_coverage_rejects_overlapping_or_missing_patches() {
        let pi = std::f64::consts::PI;
        assert!(full_turn(&[(0., pi), (pi, pi)]));
        assert!(full_turn(&[(0.3, pi), (0.3 + pi, pi)]));
        assert!(!full_turn(&[(0., pi), (0., pi)]));
        assert!(!full_turn(&[(0., pi), (pi + 0.01, pi - 0.01)]));
        assert!(!full_turn(&[(0., pi), (pi - 0.01, pi + 0.01)]));
    }

    #[test]
    fn cap_cut_rejects_crossing_displaced_and_other_contacts() {
        let a = chamfered(Vec3::Z, true, false);
        for (center, axis, radius, depth) in [
            (Vec3::new(0., 0., 40.), -Vec3::Z, 18., 40.),
            (Vec3::new(0., 0., 40.), -Vec3::Z, 18., 41.),
            (Vec3::new(0.1, 0., 40.), -Vec3::Z, 18., 5.),
            (Vec3::new(0., 0., 40.), -Vec3::Z, 19., 5.),
            (Vec3::new(0., 0., 40.), -Vec3::Z, 17., 5.),
            (Vec3::new(0., 0., 40.), Vec3::Z, 18., 5.),
            (Vec3::new(0., 0., 39.), -Vec3::Z, 18., 5.),
            (Vec3::new(0., 0., 40.), Vec3::new(0.01, 0., -1.).normalized().unwrap(), 18., 5.),
        ] {
            let b = crate::build::cylinder(center, axis, radius, depth).unwrap();
            assert!(contained_cap_cut(&a, &b).is_none(), "{center:?}/{axis:?}/{radius}/{depth}");
        }
        let plain = crate::build::cylinder(Vec3::ZERO, Vec3::Z, 20., 40.).unwrap();
        let tool = crate::build::cylinder(Vec3::new(0., 0., 40.), -Vec3::Z, 20., 5.).unwrap();
        assert!(contained_cap_cut(&plain, &tool).is_none(), "coincident cylindrical walls need the prism route");
    }
}
