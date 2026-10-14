//! Sampled model-space seam diagnostics. These are not whole-edge certificates.
//! The second fundamental form is lifted into world coordinates using the dual
//! tangent basis; comparing it avoids dependence on a surface's UV scale or axes.

use mt::{BoundedCurve, InnerSpace, ParametricCurve, ParametricSurface, SearchNearestParameter};
use serde::Serialize;
use solidcraft_geom::Vec3;
use truck_modeling as mt;

use crate::body::{Body, edge_order, from_p3, p3};
use crate::{KernelError, Result, guard};

const MAX_EDGES: usize = 10_000;
const MAX_SAMPLE_WORK: usize = 100_000;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ContinuityOptions {
    pub samples: usize,
    pub gap_tolerance_mm: f64,
    pub angle_tolerance_deg: f64,
    /// Frobenius norm of the world-space second fundamental form difference.
    pub curvature_tolerance_per_mm: f64,
}

impl Default for ContinuityOptions {
    fn default() -> Self {
        Self { samples: 33, gap_tolerance_mm: 1e-5, angle_tolerance_deg: 0.01, curvature_tolerance_per_mm: 1e-5 }
    }
}

impl ContinuityOptions {
    fn validate(self) -> Result<()> {
        if !(3..=257).contains(&self.samples)
            || ![self.gap_tolerance_mm, self.angle_tolerance_deg, self.curvature_tolerance_per_mm].iter().all(|v| v.is_finite() && *v >= 0.)
            || self.angle_tolerance_deg > 180.
        {
            return Err(KernelError::Invalid(
                "continuity: samples must be 3..257 and tolerances finite and non-negative (angle <= 180 degrees)".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ContinuityMaximum {
    pub value: f64,
    pub at: Vec3,
    pub edge_fraction: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EdgeContinuity {
    pub edge: usize,
    pub faces: Vec<usize>,
    /// complete, incomplete, boundary, or non_manifold.
    pub status: String,
    pub samples_evaluated: usize,
    pub samples_failed: usize,
    pub gap_mm: Option<ContinuityMaximum>,
    pub normal_angle_deg: Option<ContinuityMaximum>,
    pub curvature_jump_per_mm: Option<ContinuityMaximum>,
    /// Highest passing level at the sampled stations; never a global proof.
    pub sampled_continuity: Option<u8>,
    pub points: Vec<Vec3>,
}

struct Jet {
    point: Vec3,
    normal: Vec3,
    form: [[f64; 3]; 3],
}

fn xyz(p: Vec3) -> [f64; 3] {
    [p.x, p.y, p.z]
}

fn jet(surface: &mt::Surface, orientation: bool, point: Vec3) -> Option<Jet> {
    let (u, v) = surface.search_nearest_parameter(p3(point), None, 64)?;
    if !u.is_finite() || !v.is_finite() {
        return None;
    }
    let at = from_p3(surface.subs(u, v));
    let a = surface.uder(u, v);
    let b = surface.vder(u, v);
    let su = Vec3::new(a.x, a.y, a.z);
    let sv = Vec3::new(b.x, b.y, b.z);
    form_jet(at, su, sv, [surface.uuder(u, v), surface.uvder(u, v), surface.vvder(u, v)], orientation)
}

fn form_jet(at: Vec3, su: Vec3, sv: Vec3, [uu, uv, vv]: [mt::Vector3; 3], orientation: bool) -> Option<Jet> {
    let normal = su.cross(sv).normalized()? * if orientation { 1. } else { -1. };
    let (e, f, g) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let det = e * g - f * f;
    // Relative conditioning, so rescaling the parameter domains does not change validity.
    if !det.is_finite() || det <= e * g * 1e-12 || e <= 0. || g <= 0. {
        return None;
    }
    let (a, b) = (xyz((su * g - sv * f) / det), xyz((sv * e - su * f) / det));
    let n = mt::Vector3::new(normal.x, normal.y, normal.z);
    let (l, m, r) = (uu.dot(n), uv.dot(n), vv.dot(n));
    let form = std::array::from_fn(|i| std::array::from_fn(|j| l * a[i] * a[j] + m * (a[i] * b[j] + b[i] * a[j]) + r * b[i] * b[j]));
    (at.is_finite() && normal.is_finite() && form.iter().flatten().all(|v| v.is_finite())).then_some(Jet { point: at, normal, form })
}

fn jump(a: &Jet, b: &Jet) -> f64 {
    a.form.iter().flatten().zip(b.form.iter().flatten()).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt()
}

fn maximum(slot: &mut Option<ContinuityMaximum>, value: f64, at: Vec3, edge_fraction: f64) {
    if slot.as_ref().is_none_or(|old| value > old.value) {
        *slot = Some(ContinuityMaximum { value, at, edge_fraction });
    }
}

/// Diagnostics of B-rep edges in the same index order as `Body::edges`.
/// Empty `selected` means all edges. Sampling is uniform in edge parameter,
/// including endpoints. Singular/failed stations prevent a passing verdict.
/// Open boundaries and same-face periodic seams have no pair of distinct faces.
pub fn edge_continuity(body: &Body, selected: &[usize], options: ContinuityOptions) -> Result<Vec<EdgeContinuity>> {
    options.validate()?;
    if selected.len().saturating_mul(options.samples) > MAX_SAMPLE_WORK {
        return Err(KernelError::Invalid("continuity: too many selected sample stations".into()));
    }
    if body.is_mesh() {
        return Err(KernelError::Invalid("continuity: not supported for mesh bodies".into()));
    }
    guard("surface continuity", || {
        if body.solid.face_iter().take(MAX_EDGES + 1).count() > MAX_EDGES {
            return Err(KernelError::Invalid("continuity: too many faces".into()));
        }
        let faces: Vec<_> = body.solid.face_iter().collect();
        let mut edge_uses = 0usize;
        for face in &faces {
            edge_uses = edge_uses.saturating_add(face.edge_iter().take(MAX_EDGES * 4 + 1).count());
            if edge_uses > MAX_EDGES * 4 {
                return Err(KernelError::Invalid("continuity: too many face-edge uses".into()));
            }
        }
        let (edges, bounds) = edge_order(faces.iter().copied());
        if edges.len() > MAX_EDGES {
            return Err(KernelError::Invalid("continuity: too many edges".into()));
        }
        let indices: Vec<_> = if selected.is_empty() { (0..edges.len()).collect() } else { selected.to_vec() };
        if indices.len().saturating_mul(options.samples) > MAX_SAMPLE_WORK {
            return Err(KernelError::Invalid("continuity: sample work exceeds 100000 stations; select fewer edges or samples".into()));
        }
        let mut out = Vec::with_capacity(indices.len());
        for index in indices {
            let edge = edges.get(index).ok_or_else(|| KernelError::Invalid(format!("continuity: no edge {index}")))?;
            let adjacent = bounds.get(index).cloned().unwrap_or_default();
            let mut report = EdgeContinuity {
                edge: index,
                faces: adjacent.clone(),
                status: "complete".into(),
                samples_evaluated: 0,
                samples_failed: 0,
                gap_mm: None,
                normal_angle_deg: None,
                curvature_jump_per_mm: None,
                sampled_continuity: None,
                points: Vec::new(),
            };
            if adjacent.len() != 2 {
                report.status = if adjacent.len() < 2 { "boundary" } else { "non_manifold" }.into();
                out.push(report);
                continue;
            }
            let fa = adjacent.first().and_then(|i| faces.get(*i)).ok_or_else(|| KernelError::Failed("continuity: missing face".into()))?;
            let fb = adjacent.get(1).and_then(|i| faces.get(*i)).ok_or_else(|| KernelError::Failed("continuity: missing face".into()))?;
            let (sa, sb) = (fa.surface(), fb.surface());
            let curve = edge.oriented_curve();
            let (start, end) = curve.range_tuple();
            if !start.is_finite() || !end.is_finite() || end <= start {
                report.status = "incomplete".into();
                report.samples_failed = options.samples;
                out.push(report);
                continue;
            }
            for i in 0..options.samples {
                let fraction = i as f64 / (options.samples - 1) as f64;
                let point = from_p3(curve.subs(start * (1. - fraction) + end * fraction));
                if point.is_finite() {
                    report.points.push(point);
                }
                let pair = point.is_finite().then(|| jet(&sa, fa.orientation(), point).zip(jet(&sb, fb.orientation(), point))).flatten();
                let Some((a, b)) = pair else {
                    report.samples_failed += 1;
                    continue;
                };
                let gap = a.point.dist(b.point).max(a.point.dist(point)).max(b.point.dist(point));
                let angle = a.normal.cross(b.normal).len().atan2(a.normal.dot(b.normal)).to_degrees();
                let curvature = jump(&a, &b);
                if ![gap, angle, curvature].iter().all(|v| v.is_finite()) {
                    report.samples_failed += 1;
                    continue;
                }
                maximum(&mut report.gap_mm, gap, point, fraction);
                maximum(&mut report.normal_angle_deg, angle, point, fraction);
                maximum(&mut report.curvature_jump_per_mm, curvature, point, fraction);
                report.samples_evaluated += 1;
            }
            if report.samples_failed > 0 {
                report.status = "incomplete".into();
            } else if report.gap_mm.as_ref().is_some_and(|m| m.value <= options.gap_tolerance_mm) {
                report.sampled_continuity = Some(0);
                if report.normal_angle_deg.as_ref().is_some_and(|m| m.value <= options.angle_tolerance_deg) {
                    report.sampled_continuity = Some(1);
                    if report.curvature_jump_per_mm.as_ref().is_some_and(|m| m.value <= options.curvature_tolerance_per_mm) {
                        report.sampled_continuity = Some(2);
                    }
                }
            }
            out.push(report);
        }
        Ok(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_edges_are_ninety_degrees() {
        let b = crate::box_solid(Vec3::ZERO, Vec3::new(10., 20., 30.)).unwrap();
        let reports = edge_continuity(&b, &[], ContinuityOptions::default()).unwrap();
        assert_eq!(reports.len(), 12);
        for r in reports {
            assert_eq!(r.status, "complete");
            assert_eq!(r.samples_evaluated, 33);
            assert_eq!(r.sampled_continuity, Some(0));
            assert!((r.normal_angle_deg.unwrap().value - 90.).abs() < 1e-8);
            assert!(r.gap_mm.unwrap().value < 1e-8);
        }
    }

    #[test]
    fn tangent_cylinder_plane_jump_and_uv_scaling() {
        // Cylinder at (r,0,z): tangent basis y,z; normal x; curvature -1/r.
        let r = 4.;
        let point = Vec3::new(r, 0., 0.);
        let zero = mt::Vector3::new(0., 0., 0.);
        let cylinder = form_jet(point, Vec3::Y * r, Vec3::Z, [mt::Vector3::new(-r, 0., 0.), zero, zero], true).unwrap();
        let scaled = form_jet(point, Vec3::Z * 7., Vec3::Y * -2. * r, [zero, zero, mt::Vector3::new(-4. * r, 0., 0.)], true).unwrap();
        let plane = form_jet(point, Vec3::Y, Vec3::Z, [zero, zero, zero], true).unwrap();
        assert!(jump(&cylinder, &scaled) < 1e-12);
        assert!((jump(&cylinder, &plane) - 1. / r).abs() < 1e-12);
        assert!(cylinder.normal.dist(plane.normal) < 1e-12);
    }

    #[test]
    fn actual_round_seams_are_g1_and_quintic_seams_are_g2() {
        let b = crate::box_solid(Vec3::ZERO, Vec3::new(10., 10., 10.)).unwrap();
        let edge = Vec3::new(5., 0., 0.);
        let round = crate::fillet(&b, &[edge], 2.).unwrap();
        let g2 = crate::fillet_g2(&b, &[edge], 2.).unwrap();
        for (body, level) in [(&round, 1), (&g2, 2)] {
            let reports = edge_continuity(body, &[], ContinuityOptions::default()).unwrap();
            let seams: Vec<_> = reports.iter().filter(|r| r.normal_angle_deg.as_ref().is_some_and(|m| m.value < 0.01)).collect();
            assert_eq!(seams.len(), 2, "{reports:#?}");
            for seam in seams {
                assert_eq!(seam.status, "complete");
                assert_eq!(seam.sampled_continuity, Some(level), "{seam:#?}");
                if level == 1 {
                    assert!((seam.curvature_jump_per_mm.as_ref().unwrap().value - 0.5).abs() < 1e-6);
                } else {
                    assert!(seam.curvature_jump_per_mm.as_ref().unwrap().value < 1e-6);
                }
            }
        }
    }

    #[test]
    fn boundary_and_non_manifold_edges_are_not_passing_seams() {
        let b = crate::box_solid(Vec3::ZERO, Vec3::new(10., 10., 10.)).unwrap();
        let face = b.solid.face_iter().next().unwrap().clone();
        for (faces, status) in [(vec![face.clone()], "boundary"), (vec![face.clone(), face.clone(), face.inverse()], "non_manifold")] {
            let surface = crate::surfaces::surface_body(faces);
            let reports = edge_continuity(&surface, &[], ContinuityOptions::default()).unwrap();
            assert_eq!(reports.len(), 4);
            assert!(reports.iter().all(|r| r.status == status && r.sampled_continuity.is_none()));
        }
    }

    #[test]
    fn singular_charts_are_refused_and_curvature_directions_matter() {
        let zero = mt::Vector3::new(0., 0., 0.);
        assert!(form_jet(Vec3::ZERO, Vec3::ZERO, Vec3::Z, [zero; 3], true).is_none());
        assert!(form_jet(Vec3::ZERO, Vec3::Y, Vec3::Y, [zero; 3], true).is_none());
        let curvature = mt::Vector3::new(-0.25, 0., 0.);
        let a = form_jet(Vec3::ZERO, Vec3::Y, Vec3::Z, [curvature, zero, zero], true).unwrap();
        let b = form_jet(Vec3::ZERO, Vec3::Y, Vec3::Z, [zero, zero, curvature], true).unwrap();
        // Same principal eigenvalues, but different bending directions: not G2.
        assert!((jump(&a, &b) - 2f64.sqrt() * 0.25).abs() < 1e-12);
    }

    #[test]
    fn invalid_options_and_indices_return_errors() {
        let b = crate::box_solid(Vec3::ZERO, Vec3::new(10., 10., 10.)).unwrap();
        for samples in [0, 1, 2, 258, usize::MAX] {
            assert!(edge_continuity(&b, &[], ContinuityOptions { samples, ..Default::default() }).is_err());
        }
        for angle in [-1., f64::NAN, f64::INFINITY, 181.] {
            assert!(edge_continuity(&b, &[], ContinuityOptions { angle_tolerance_deg: angle, ..Default::default() }).is_err());
        }
        assert!(edge_continuity(&b, &[usize::MAX], ContinuityOptions::default()).is_err());
    }
}
