//! Centres derived from the selected model geometry, independent of any source sketch.
use crate::{Sel, Session};
use serde_json::{Value, json};
use solidcraft_geom::Vec3;
use solidcraft_kernel::FaceSurface;
use solidcraft_sketch::CurveKind;

pub(super) struct Center {
    pub position: Vec3,
    pub axis: Option<Vec3>,
    pub radius: f64,
}

impl Center {
    pub fn json(&self) -> Value {
        json!({"position": self.position, "axis": self.axis, "radius_mm": self.radius})
    }
}

/// A circle or open circular arc. Check every sample, so lines, ellipses and non-planar
/// curves cannot acquire a spurious centre. Farthest points also handle closed polylines.
pub(super) fn circle(points: &[Vec3]) -> Option<(Vec3, Vec3, f64)> {
    let a = *points.first()?;
    if !a.is_finite() || points.len() < 3 {
        return None;
    }
    let b = *points.iter().max_by(|p, q| p.dist(a).total_cmp(&q.dist(a)))?;
    let u = b - a;
    let c = *points.iter().max_by(|p, q| u.cross(**p - a).len2().total_cmp(&u.cross(**q - a).len2()))?;
    let v = c - a;
    let n = u.cross(v);
    if !(n.len2() > u.len2() * v.len2() * 1e-12) {
        return None;
    }
    let center = a + (v.cross(n) * u.len2() + n.cross(u) * v.len2()) / (2.0 * n.len2());
    let normal = n.normalized()?;
    let radius = a.dist(center);
    if !center.is_finite() || !radius.is_finite() || radius <= 1e-12 {
        return None;
    }
    let tol = (radius * 1e-6).max(1e-9);
    points
        .iter()
        .all(|p| p.is_finite() && (p.dist(center) - radius).abs() <= tol && (*p - center).dot(normal).abs() <= tol)
        .then_some((center, normal, radius))
}

pub(super) fn of(s: &Session, sel: &Sel) -> Option<Center> {
    let st = s.world_state();
    match sel {
        Sel::Edge { body, index, .. } => {
            let mesh = st.body(body)?.mesh();
            let (position, _, radius) = circle(mesh.edges.get(*index)?)?;
            Some(Center { position, axis: None, radius })
        }
        Sel::Face { body, index, .. } => {
            let b = st.body(body)?;
            let surface = solidcraft_kernel::guard("measure centre", || Ok(solidcraft_kernel::face_surface(&b.body, *index))).ok()??;
            let mesh = b.mesh();
            let patch = mesh.surface_patch(*index);
            match surface {
                FaceSurface::Cylinder { origin, axis, radius } => {
                    let axis = axis.normalized()?;
                    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                    for (tri, face) in mesh.triangles.iter().zip(&mesh.tri_face) {
                        if patch.contains(&(*face as usize)) {
                            for p in mesh.tri(tri).into_iter().flatten() {
                                let h = (p - origin).dot(axis);
                                lo = lo.min(h);
                                hi = hi.max(h);
                            }
                        }
                    }
                    let position = origin + axis * ((lo + hi) * 0.5);
                    (position.is_finite() && radius.is_finite() && radius > 0.0).then_some(Center { position, axis: Some(axis), radius })
                }
                FaceSurface::Plane { .. } => {
                    let mut points = Vec::new();
                    for (i, faces) in mesh.edge_faces.iter().enumerate() {
                        if faces.iter().filter(|f| patch.contains(&(**f as usize))).count() == 1 {
                            let edge = mesh.edges.get(i)?;
                            // A polygon's vertices may be concyclic; its straight boundaries are not circular.
                            circle(edge)?;
                            points.extend_from_slice(edge);
                        }
                    }
                    let (position, _, radius) = circle(&points)?;
                    Some(Center { position, axis: None, radius })
                }
                _ => None,
            }
        }
        Sel::SketchCurve { id } => {
            let (sketch, id) = match id.split_once(':') {
                Some((sketch, id)) => (sketch.parse().ok()?, id),
                None => (s.active_sketch?, id.as_str()),
            };
            sketch_center(s, sketch, id)
        }
        Sel::ExternalSketchCurve { sketch, id } => sketch_center(s, *sketch, id),
        _ => None,
    }
}

fn sketch_center(s: &Session, sketch: u64, id: &str) -> Option<Center> {
    let st = s.world_state();
    let ss = st.sketch(sketch)?;
    let curve = ss.sketch.curves.get(ss.sketch.curve_index(id)?)?;
    let (c, radius) = match curve.kind {
        CurveKind::Circle { c, r } => (c, r),
        CurveKind::Arc { c, a, .. } => (c, ss.sketch.point(c)?.dist(ss.sketch.point(a)?)),
        _ => return None,
    };
    Some(Center { position: ss.plane.to_world(ss.sketch.point(c)?), axis: None, radius })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measurement_circle_centres_reject_lines_ellipses_and_nonplanar_curves() {
        for (sweep, closed) in [(std::f64::consts::PI, false), (std::f64::consts::TAU, true)] {
            let points: Vec<_> = (0..=24)
                .map(|i| {
                    let t = i as f64 * sweep / 24.0;
                    Vec3::new(12.0 + 4.0 * t.cos(), 8.0, 9.0 + 4.0 * t.sin())
                })
                .collect();
            let (p, _, r) = circle(&points).unwrap();
            assert!(p.dist(Vec3::new(12.0, 8.0, 9.0)) < 1e-10, "closed={closed}");
            assert!((r - 4.0).abs() < 1e-10);
            let ellipse: Vec<_> = points.iter().map(|p| Vec3::new(p.x * 2.0, p.y, p.z)).collect();
            assert!(circle(&ellipse).is_none());
            let mut bent = points;
            bent[7].y += 0.1;
            assert!(circle(&bent).is_none());
        }
        assert!(circle(&[Vec3::ZERO, Vec3::X, Vec3::X * 2.0]).is_none());
        assert!(circle(&[Vec3::ZERO; 4]).is_none());
    }
}
