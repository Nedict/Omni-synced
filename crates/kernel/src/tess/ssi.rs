//! Points on a surface–surface intersection edge, on our own surface evaluators.
//!
//! truck stores an intersection edge as a leader curve (a polyline or B-spline near the
//! intersection) and its two surfaces, and finds each point by projecting the leader point onto
//! each surface in turn from no starting guess: a nearest-point search per surface per point.
//! Here the parameters of both surfaces are found once at a set of leader parameters, then each
//! point is refined from the interpolated parameters by Newton's method on the four unknowns
//! (u1, v1, u2, v2): S1(u1, v1) = S2(u2, v2), held on the plane through the leader point normal
//! to the leader (Barnhill & Kersey, "A marching method for parametric surface/surface
//! intersection", CAGD 7, 1990; Patrikalakis & Maekawa, "Shape Interrogation for CAD/CAM",
//! 2002, ch. 5). Every point is independent once seeded. Where the surfaces are close to
//! tangent along the edge (the system becomes singular) or Newton does not converge, the caller
//! uses truck's evaluation.

use solidcraft_geom::Vec3;
use solidcraft_geom::tess::EvalSurface;
use truck_meshalgo::prelude::*;
use truck_modeling as mt;

use super::uv::{self, Domain};
use crate::body::from_p3;

/// Leader parameters at which both surfaces' parameters are found up front.
const SEEDS: usize = 32;
/// Newton iterations per point (two or three suffice from a seed).
const MAX_ITER: usize = 8;
/// Below this sine of the angle between the surface normals the intersection is treated as
/// near-tangent: points there are left to truck.
const MIN_SIN: f64 = 0.05;

/// An intersection edge ready for point evaluation.
pub(crate) struct Intersection<'a> {
    curve: &'a mt::Curve,
    surfaces: [EvalSurface; 2],
    domains: [Domain; 2],
    /// Leader parameters and both surfaces' parameters there (periodic directions unwrapped
    /// along the leader, so neighbours interpolate).
    seeds: Vec<(f64, [f64; 4])>,
    /// Convergence tolerance (mm).
    eps: f64,
}

impl<'a> Intersection<'a> {
    /// `None` unless `curve` is an intersection curve whose surfaces have evaluators and whose
    /// leader projects onto both.
    pub(crate) fn new(curve: &'a mt::Curve) -> Option<Self> {
        let mt::Curve::IntersectionCurve(ic) = curve else { return None };
        let surfaces = [super::surface::eval_surface(ic.surface0())?, super::surface::eval_surface(ic.surface1())?];
        let domains = [uv::domain(&surfaces[0]), uv::domain(&surfaces[1])];
        let (t0, t1) = curve.range_tuple();
        if !(t0.is_finite() && t1.is_finite() && t1 > t0) {
            return None;
        }
        let leader = |t: f64| match ic.leader() {
            mt::Leader::Polyline(l) => from_p3(l.subs(t)),
            mt::Leader::BSpline(l) => from_p3(l.subs(t)),
        };
        // The leader lies within the curve's tolerance of both surfaces.
        let accept = (ic.tolerance() * 100.0).max(1e-6);
        let mut seeds = Vec::with_capacity(SEEDS + 1);
        let mut prev: Option<[f64; 4]> = None;
        for k in 0..=SEEDS {
            let t = t0 + (t1 - t0) * k as f64 / SEEDS as f64;
            let p = leader(t);
            let mut q = [0.0; 4];
            for (i, (s, dom)) in surfaces.iter().zip(&domains).enumerate() {
                let hint = prev.map(|w| [w[2 * i], w[2 * i + 1]]);
                let mut uv = uv::project(s, dom, p, hint, accept)?;
                if let Some(h) = hint {
                    for d in 0..2 {
                        uv[d] = uv::unwrap(uv[d], h[d], dom.period[d]);
                    }
                }
                q[2 * i] = uv[0];
                q[2 * i + 1] = uv[1];
            }
            seeds.push((t, q));
            prev = Some(q);
        }
        let scale = seeds.iter().map(|(t, _)| leader(*t).len()).fold(1.0, f64::max);
        Some(Intersection { curve, surfaces, domains, seeds, eps: scale * 1e-10 })
    }

    /// The point at curve parameter `t`, or `None` (near-tangent, or Newton failed).
    pub(crate) fn eval(&self, t: f64) -> Option<Vec3> {
        let mt::Curve::IntersectionCurve(ic) = self.curve else { return None };
        let (l, dl) = match ic.leader() {
            mt::Leader::Polyline(c) => (from_p3(c.subs(t)), from_v3(c.der(t))),
            mt::Leader::BSpline(c) => (from_p3(c.subs(t)), from_v3(c.der(t))),
        };
        let dir = dl.normalized()?;
        let mut x = self.seed(t)?;
        for _ in 0..MAX_ITER {
            let (Some((p1, u1, v1)), Some((p2, u2, v2))) = (self.surfaces[0].derivs(x[0], x[1]), self.surfaces[1].derivs(x[2], x[3])) else {
                return None;
            };
            let gap = p1 - p2;
            let plane = dir.dot(p1 - l);
            let n = u1.cross(v1).normalized()?.cross(u2.cross(v2).normalized()?);
            if n.len() < MIN_SIN {
                return None;
            }
            if gap.len() <= self.eps && plane.abs() <= self.eps {
                return Some((p1 + p2) * 0.5);
            }
            // J · dx = −F, F = (S1 − S2, dir · (S1 − L)).
            let j = [[u1.x, v1.x, -u2.x, -v2.x], [u1.y, v1.y, -u2.y, -v2.y], [u1.z, v1.z, -u2.z, -v2.z], [dir.dot(u1), dir.dot(v1), 0.0, 0.0]];
            let dx = solve4(j, [-gap.x, -gap.y, -gap.z, -plane])?;
            for (xi, d) in x.iter_mut().zip(dx) {
                *xi += d;
            }
            for i in 0..2 {
                let (u, v) = self.domains[i].clamp([x[2 * i], x[2 * i + 1]]).into();
                x[2 * i] = u;
                x[2 * i + 1] = v;
            }
        }
        None
    }

    /// Both surfaces' parameters at `t`, interpolated between the seeds around it.
    fn seed(&self, t: f64) -> Option<[f64; 4]> {
        let k = self.seeds.partition_point(|(s, _)| *s <= t).clamp(1, self.seeds.len().saturating_sub(1).max(1));
        let ((ta, a), (tb, b)) = (self.seeds.get(k - 1)?, self.seeds.get(k)?);
        let f = if tb > ta { ((t - ta) / (tb - ta)).clamp(0.0, 1.0) } else { 0.0 };
        Some([0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * f))
    }
}

fn from_v3(v: mt::Vector3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// `a · x = b` for a 4 × 4 system by Gaussian elimination with partial pivoting; `None` when
/// singular.
fn solve4(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> Option<[f64; 4]> {
    let scale = a.iter().flatten().fold(0.0f64, |m, x| m.max(x.abs()));
    if !(scale > 0.0) {
        return None;
    }
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if !(a[p][c].abs() > 1e-12 * scale) {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        let pivot = a[c];
        for r in c + 1..4 {
            let f = a[r][c] / pivot[c];
            for (x, p) in a[r].iter_mut().zip(pivot).skip(c) {
                *x -= f * p;
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; 4];
    for c in (0..4).rev() {
        let s: f64 = (c + 1..4).map(|k| a[c][k] * x[k]).sum();
        x[c] = (b[c] - s) / a[c][c];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_four_by_four_system() {
        let a = [[2.0, 1.0, 0.0, 0.0], [1.0, 3.0, 1.0, 0.0], [0.0, 1.0, 4.0, 1.0], [0.0, 0.0, 1.0, 5.0]];
        let x = [1.0, -2.0, 0.5, 3.0];
        let b = [0, 1, 2, 3].map(|r| (0..4).map(|c| a[r][c] * x[c]).sum::<f64>());
        let Some(got) = solve4(a, b) else { panic!("solves") };
        for (g, w) in got.iter().zip(x) {
            assert!((g - w).abs() < 1e-12);
        }
        assert!(solve4([[1.0; 4]; 4], [1.0; 4]).is_none(), "singular");
    }

    /// Points on the intersection edges of a boolean (a cross hole: two cylinders) are on both
    /// surfaces and agree with truck's evaluation of the same curve parameter (to truck's own
    /// precision).
    #[test]
    fn intersection_points_lie_on_both_surfaces_and_match_truck() {
        let Ok(pin) = crate::cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 6.0) else { panic!("pin") };
        let Ok(cross) = crate::cylinder(Vec3::new(5.0, -1.0, 2.0), Vec3::Y, 1.0, 12.0) else { panic!("cross") };
        let Ok(Some(body)) = crate::boolean(&pin, &cross, crate::BoolOp::Cut) else { panic!("cut") };
        let mut checked = 0;
        for e in body.solid.edge_iter() {
            let curve = e.curve();
            let Some(ix) = Intersection::new(&curve) else { continue };
            let (t0, t1) = curve.range_tuple();
            for k in 0..=50 {
                let t = t0 + (t1 - t0) * f64::from(k) / 50.0;
                let Some(p) = ix.eval(t) else { continue };
                // truck stops its projections at its own tolerance (about 1e-6 mm); ours lies on
                // both surfaces far closer than that.
                let q = from_p3(curve.subs(t));
                assert!(p.dist(q) < 1e-5, "t {t}: {p:?} vs truck {q:?}");
                for (s, dom) in ix.surfaces.iter().zip(&ix.domains) {
                    assert!(uv::project(s, dom, p, None, 1e-8).is_some(), "t {t}: {p:?} off a surface");
                }
                checked += 1;
            }
        }
        assert!(checked > 100, "{checked} points checked");
    }
}
