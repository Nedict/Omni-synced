//! Reuse intersection support grids only while meshing one shell.
//! The grid and ordered seed scan are shared with curve_sampler; see its attribution.
use crate::curve_sampler::Grid;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use truck_meshalgo::tessellation::TrySubs;
use truck_modeling::{self as mt, ParametricCurve, ParametricSurface, ParametricSurface3D};

// At most 32,378,880 bytes of grid data and 4 MiB of key words per shell, plus headers.
// Each key construction temporarily needs at most 8 KiB. No state survives this shell.
// Only unhinted spline support searches consume a slot. Exhaustion uses the original search.
pub(super) const MAX_GRIDS: usize = 512;
// Keys retain every geometry bit, including signed zero, weights and NaN payloads.
// At most 1,024 words (8 KiB) per support key; larger supports keep the original search.
const MAX_KEY_WORDS: usize = 1024;
#[derive(Debug, PartialEq, Eq, Hash)]
struct SurfaceKey(Vec<u64>);
impl SurfaceKey {
    fn new(surface: &mt::Surface) -> Option<Self> {
        fn push(out: &mut Vec<u64>, word: u64) -> Option<()> {
            if out.len() >= MAX_KEY_WORDS {
                return None;
            }
            out.push(word);
            Some(())
        }
        fn knots(out: &mut Vec<u64>, values: &mt::KnotVec) -> Option<()> {
            push(out, values.len() as u64)?;
            for &v in values.iter() {
                push(out, v.to_bits())?;
            }
            Some(())
        }
        let mut out = Vec::new();
        match surface {
            mt::Surface::BSplineSurface(s) => {
                push(&mut out, 0)?;
                knots(&mut out, s.uknot_vec())?;
                knots(&mut out, s.vknot_vec())?;
                push(&mut out, s.control_points().len() as u64)?;
                for row in s.control_points() {
                    push(&mut out, row.len() as u64)?;
                    for p in row {
                        for value in [p.x, p.y, p.z] {
                            push(&mut out, value.to_bits())?;
                        }
                    }
                }
            }
            mt::Surface::NurbsSurface(s) => {
                push(&mut out, 1)?;
                knots(&mut out, s.uknot_vec())?;
                knots(&mut out, s.vknot_vec())?;
                push(&mut out, s.control_points().len() as u64)?;
                for row in s.control_points() {
                    push(&mut out, row.len() as u64)?;
                    for p in row {
                        for value in [p.x, p.y, p.z, p.w] {
                            push(&mut out, value.to_bits())?;
                        }
                    }
                }
            }
            _ => return None,
        }
        Some(Self(out))
    }
}
type SharedGrid = Arc<OnceLock<Option<Grid>>>;
type LocalGrid = OnceLock<Option<SharedGrid>>;
pub(super) struct MeshBudget {
    maximum: usize,
    cells: Mutex<HashMap<SurfaceKey, SharedGrid>>,
}
impl MeshBudget {
    pub(super) fn new(grids: usize) -> Self {
        Self { maximum: grids, cells: Mutex::new(HashMap::new()) }
    }
    fn cell(&self, surface: &mt::Surface) -> Option<SharedGrid> {
        let key = SurfaceKey::new(surface)?;
        let mut cells = self.cells.lock().ok()?;
        if let Some(cell) = cells.get(&key) {
            return Some(cell.clone());
        }
        if cells.len() >= self.maximum {
            return None;
        }
        let cell = Arc::new(OnceLock::new());
        cells.insert(key, cell.clone());
        Some(cell)
    }
}
struct PreparedSurface<'a> {
    original: &'a mt::Surface,
    grid: &'a LocalGrid,
    budget: &'a MeshBudget,
}
impl Clone for PreparedSurface<'_> {
    fn clone(&self) -> Self {
        Self { original: self.original, grid: self.grid, budget: self.budget }
    }
}
impl<'a> PreparedSurface<'a> {
    fn seed(&self, point: mt::Point3) -> Option<(f64, f64)> {
        if !matches!(self.original, mt::Surface::BSplineSurface(_) | mt::Surface::NurbsSurface(_)) {
            return None;
        }
        // The map lock is released before evaluating the immutable bare support.
        let cell = self.grid.get_or_init(|| self.budget.cell(self.original)).as_ref()?;
        cell.get_or_init(|| Grid::prepare(self.original)).as_ref().map(|grid| grid.seed(point))
    }
}
impl ParametricSurface for PreparedSurface<'_> {
    type Point = mt::Point3;
    type Vector = mt::Vector3;
    fn subs(&self, u: f64, v: f64) -> mt::Point3 {
        self.original.subs(u, v)
    }
    fn uder(&self, u: f64, v: f64) -> mt::Vector3 {
        self.original.uder(u, v)
    }
    fn vder(&self, u: f64, v: f64) -> mt::Vector3 {
        self.original.vder(u, v)
    }
    fn uuder(&self, u: f64, v: f64) -> mt::Vector3 {
        self.original.uuder(u, v)
    }
    fn uvder(&self, u: f64, v: f64) -> mt::Vector3 {
        self.original.uvder(u, v)
    }
    fn vvder(&self, u: f64, v: f64) -> mt::Vector3 {
        self.original.vvder(u, v)
    }
    fn parameter_range(&self) -> (mt::ParameterRange, mt::ParameterRange) {
        self.original.parameter_range()
    }
    fn u_period(&self) -> Option<f64> {
        self.original.u_period()
    }
    fn v_period(&self) -> Option<f64> {
        self.original.v_period()
    }
}
impl ParametricSurface3D for PreparedSurface<'_> {
    fn normal(&self, u: f64, v: f64) -> mt::Vector3 {
        self.original.normal(u, v)
    }
}
impl mt::SearchNearestParameter<mt::D2> for PreparedSurface<'_> {
    type Point = mt::Point3;
    fn search_nearest_parameter<H: Into<mt::SPHint2D>>(&self, point: mt::Point3, hint: H, trials: usize) -> Option<(f64, f64)> {
        let hint = hint.into();
        let effective = match hint {
            mt::SPHint2D::None => self.seed(point).map_or(hint, |(u, v)| mt::SPHint2D::Parameter(u, v)),
            _ => hint,
        };
        self.original.search_nearest_parameter(point, effective, trials)
    }
}

#[derive(Clone)]
pub(super) struct MeshCurve {
    original: Arc<mt::Curve>,
    grids: Arc<(LocalGrid, LocalGrid)>,
    budget: Arc<MeshBudget>,
}
impl MeshCurve {
    pub(super) fn new(original: mt::Curve, budget: Arc<MeshBudget>) -> Self {
        Self { original: Arc::new(original), grids: Arc::new((OnceLock::new(), OnceLock::new())), budget }
    }
}
impl ParametricCurve for MeshCurve {
    type Point = mt::Point3;
    type Vector = mt::Vector3;
    fn subs(&self, t: f64) -> mt::Point3 {
        self.original.subs(t)
    }
    fn der(&self, t: f64) -> mt::Vector3 {
        self.original.der(t)
    }
    fn der2(&self, t: f64) -> mt::Vector3 {
        self.original.der2(t)
    }
    fn parameter_range(&self) -> mt::ParameterRange {
        self.original.parameter_range()
    }
    fn period(&self) -> Option<f64> {
        self.original.period()
    }
}
impl mt::BoundedCurve for MeshCurve {}
impl mt::ParameterDivision1D for MeshCurve {
    type Point = mt::Point3;
    fn parameter_division(&self, range: (f64, f64), tol: f64) -> (Vec<f64>, Vec<mt::Point3>) {
        self.original.parameter_division(range, tol)
    }
}
impl TrySubs for MeshCurve {
    fn try_subs(&self, t: f64) -> Option<mt::Point3> {
        match self.original.as_ref() {
            mt::Curve::IntersectionCurve(c) => {
                let a = PreparedSurface { original: c.surface0(), grid: &self.grids.0, budget: &self.budget };
                let b = PreparedSurface { original: c.surface1(), grid: &self.grids.1, budget: &self.budget };
                // Same entry point, leader, initial hints, iteration limit and rejection behavior.
                mt::double_projection(&a, None, &b, None, c.leader().subs(t), c.leader().der(t), 100).map(|triple| triple.0)
            }
            _ => self.original.try_subs(t),
        }
    }
    fn approx_subs(&self, t: f64) -> mt::Point3 {
        self.original.approx_subs(t)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn curve(offset: f64) -> mt::Curve {
        let p = mt::Point3::new;
        let plane = mt::Surface::Plane(mt::Plane::new(p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)));
        let spline = mt::Surface::BSplineSurface(mt::BSplineSurface::new(
            (mt::KnotVec::bezier_knot(2), mt::KnotVec::bezier_knot(1)),
            vec![
                vec![p(offset, 0., -1.), p(offset, 0., 1.)],
                vec![p(offset + 0.5, 0., -1.), p(offset + 0.5, 0., 1.)],
                vec![p(offset + 1., 1., -1.), p(offset + 1., 1., 1.)],
            ],
        ));
        let leader = mt::Leader::BSpline(mt::BSplineCurve::new(
            mt::KnotVec::bezier_knot(2),
            vec![p(offset, 0., 0.), p(offset + 0.5, 0., 0.), p(offset + 1., 1., 0.)],
        ));
        mt::Curve::IntersectionCurve(mt::IntersectionCurve::new_unchecked(Box::new(plane), Box::new(spline), leader, 1e-8))
    }
    fn bits(p: Option<mt::Point3>) -> Option<[u64; 3]> {
        p.map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
    }
    #[test]
    fn samples_and_exhausted_budget_match_original() {
        for cap in [0, 1, 512] {
            let raw = curve(0.);
            let adapted = MeshCurve::new(raw.clone(), Arc::new(MeshBudget::new(cap)));
            for i in 0..=64 {
                let t = i as f64 / 64.;
                assert_eq!(bits(raw.try_subs(t)), bits(adapted.try_subs(t)));
                assert_eq!(bits(Some(raw.approx_subs(t))), bits(Some(adapted.approx_subs(t))));
            }
        }
    }
    #[test]
    fn distinct_supports_keep_distinct_grids() {
        let budget = Arc::new(MeshBudget::new(2));
        for offset in [0., 10.] {
            let raw = curve(offset);
            let adapted = MeshCurve::new(raw.clone(), budget.clone());
            for i in 0..=32 {
                let t = i as f64 / 32.;
                assert_eq!(bits(raw.try_subs(t)), bits(adapted.try_subs(t)));
            }
        }
        assert_eq!(budget.cells.lock().unwrap().len(), 2);
    }
    #[test]
    fn concurrent_clones_initialize_once_and_drop() {
        let budget = Arc::new(MeshBudget::new(1));
        let adapted = MeshCurve::new(curve(0.), budget.clone());
        let weak = Arc::downgrade(&adapted.grids);
        let expected = bits(adapted.original.try_subs(0.4));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let c = adapted.clone();
                scope.spawn(move || assert_eq!(bits(c.try_subs(0.4)), expected));
            }
        });
        assert_eq!(budget.cells.lock().unwrap().len(), 1);
        drop(adapted);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn nonintersection_curve_keeps_original_contract() {
        let raw = mt::Curve::Line(mt::Line(mt::Point3::new(0., 0., 0.), mt::Point3::new(1., 2., 3.)));
        let adapted = MeshCurve::new(raw.clone(), Arc::new(MeshBudget::new(1)));
        for t in [-1., 0., 0.5, 1., 2.] {
            assert_eq!(bits(raw.try_subs(t)), bits(adapted.try_subs(t)));
            assert_eq!(raw.der(t), adapted.der(t));
            assert_eq!(raw.der2(t), adapted.der2(t));
        }
    }
    use mt::Transformed;
    fn rational(weights: [[f64; 2]; 2]) -> mt::Surface {
        let controls = (0..2)
            .map(|i| {
                (0..2)
                    .map(|j| {
                        let w = weights[i][j];
                        mt::Vector4::new(i as f64 * w, j as f64 * w, (i * j) as f64 * w, w)
                    })
                    .collect()
            })
            .collect();
        mt::Surface::NurbsSurface(mt::NurbsSurface::new(mt::BSplineSurface::new(
            (mt::KnotVec::bezier_knot(1), mt::KnotVec::bezier_knot(1)),
            controls,
        )))
    }
    #[test]
    fn weighted_transformed_intersections_match_all_33_projection_results() {
        let p = mt::Point3::new;
        let plane = mt::Surface::Plane(mt::Plane::new(p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)));
        let leader = mt::Leader::BSpline(mt::BSplineCurve::new(mt::KnotVec::bezier_knot(1), vec![p(0.1, 0.03, 0.02), p(0.9, 0.03, 0.02)]));
        let curve = mt::Curve::IntersectionCurve(mt::IntersectionCurve::new_unchecked(
            Box::new(plane),
            Box::new(rational([[1., 2.], [0.5, 3.]])),
            leader,
            1e-8,
        ));
        for transform in [
            mt::Matrix4::from_scale(1.),
            mt::Matrix4::from_translation(mt::Vector3::new(3., -2., 1.)),
            mt::Matrix4::from_nonuniform_scale(-2., 3., 0.5),
            mt::Matrix4::from_angle_z(mt::Rad(0.37)),
        ] {
            let transformed = curve.transformed(transform);
            let sampler = MeshCurve::new(transformed.clone(), Arc::new(MeshBudget::new(512)));
            for k in 0..=32 {
                let t = k as f64 / 32.;
                let expected = transformed.try_subs(t);
                assert!(expected.is_some(), "valid weighted fixture sample {k}");
                assert_eq!(bits(sampler.try_subs(t)), bits(expected));
            }
        }
    }
    #[test]
    fn equal_independent_supports_share_one_grid_and_operation_drop_releases_it() {
        let budget = Arc::new(MeshBudget::new(1));
        let a = MeshCurve::new(curve(0.), budget.clone());
        let b = MeshCurve::new(curve(0.), budget.clone());
        assert_eq!(bits(a.try_subs(0.3)), bits(b.try_subs(0.3)));
        assert_eq!(budget.cells.lock().unwrap().len(), 1);
        let ga = a.grids.1.get().unwrap().as_ref().unwrap();
        let gb = b.grids.1.get().unwrap().as_ref().unwrap();
        assert!(Arc::ptr_eq(ga, gb));
        let weak = Arc::downgrade(ga);
        drop(a);
        drop(b);
        assert!(weak.upgrade().is_some());
        drop(budget);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn independent_concurrent_curves_admit_one_exact_support() {
        let budget = Arc::new(MeshBudget::new(1));
        let expected = bits(curve(0.).try_subs(0.4));
        std::thread::scope(|scope| {
            for _ in 0..12 {
                let b = budget.clone();
                scope.spawn(move || assert_eq!(bits(MeshCurve::new(curve(0.), b).try_subs(0.4)), expected));
            }
        });
        assert_eq!(budget.cells.lock().unwrap().len(), 1);
    }
    #[test]
    fn key_retains_signed_zero_nan_payload_and_surface_kind() {
        let make = |x: f64| {
            mt::Surface::BSplineSurface(mt::BSplineSurface::new(
                (mt::KnotVec::bezier_knot(1), mt::KnotVec::bezier_knot(1)),
                vec![vec![mt::Point3::new(x, 0., 0.), mt::Point3::new(0., 1., 0.)], vec![mt::Point3::new(1., 0., 0.), mt::Point3::new(1., 1., 0.)]],
            ))
        };
        assert_ne!(SurfaceKey::new(&make(0.)), SurfaceKey::new(&make(-0.)));
        assert_ne!(SurfaceKey::new(&make(f64::from_bits(0x7ff8000000000001))), SurfaceKey::new(&make(f64::from_bits(0x7ff8000000000002))));
        let rational = rational([[1., 1.], [1., 1.]]);
        assert_ne!(SurfaceKey::new(&make(0.)), SurfaceKey::new(&rational));
    }
    #[test]
    fn oversized_key_preserves_original_fallback() {
        let surface = mt::Surface::BSplineSurface(mt::BSplineSurface::new(
            (mt::KnotVec::bezier_knot(199), mt::KnotVec::bezier_knot(1)),
            (0..200).map(|i| vec![mt::Point3::new(i as f64, 0., 0.), mt::Point3::new(i as f64, 1., 0.)]).collect(),
        ));
        assert!(SurfaceKey::new(&surface).is_none());
        let budget = MeshBudget::new(1);
        assert!(budget.cell(&surface).is_none());
        assert!(budget.cells.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
#[path = "mesh_curve_sampler_mesh_tests.rs"]
mod mesh_tests;
