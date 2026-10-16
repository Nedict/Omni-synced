//! Finite, convex planar face coverage through a union. Infinite support
//! equality alone never grants construction lineage to the resulting face.
use super::from_p3;
use super::planar_sources::{actual_plane, certified_plane, descriptor_cost, same_support};
use crate::{Body, FaceSource, KernelError, Result, guard};
use solidcraft_geom::{Plane, Vec2};
use std::collections::BTreeMap;
use std::sync::Arc;
use truck_modeling as mt;

const MAX_VERTICES: usize = 128;
const MAX_PIECES: usize = 256;

pub(super) fn is_leaf(source: &FaceSource) -> bool {
    matches!(
        source,
        FaceSource::Box { producer: Some(_), .. }
            | FaceSource::CylinderCap { .. }
            | FaceSource::CertifiedExtrudeSide { plane: Some(_), .. }
            | FaceSource::CertifiedExtrudeCap { .. }
    )
}

fn key(source: &FaceSource) -> Option<String> {
    descriptor_cost(source)?;
    match source {
        FaceSource::Box { producer: Some(p), axis, high, .. } => Some(format!("{p}:box:{axis}:{high}")),
        FaceSource::CylinderCap { producer, high, .. } => Some(format!("{producer}:cylinder:{high}")),
        FaceSource::CertifiedExtrudeSide { producer, sketch, curve, plane: Some(_), .. } => {
            Some(format!("{producer}:side:{sketch}:{}:{curve}", curve.len()))
        }
        FaceSource::CertifiedExtrudeCap { producer, sketch, high, outer, holes, .. } => {
            let mut key = format!("{producer}:cap:{sketch}:{high}:");
            for ids in std::iter::once(outer).chain(holes) {
                key.push_str(&format!("{}:", ids.len()));
                for id in ids {
                    key.push_str(&format!("{}:{id}", id.len()));
                }
            }
            Some(key)
        }
        _ => None,
    }
}

fn charge(work: &mut usize, n: usize) -> Result<()> {
    *work = work.checked_sub(n).ok_or_else(|| KernelError::Invalid("finite planar lineage work budget exceeded".into()))?;
    Ok(())
}

fn error(a: Vec2, b: Vec2, p: Vec2) -> f64 {
    let d = b - a;
    let q = p - a;
    4096. * f64::EPSILON * (1. + (d.x * q.y).abs() + (d.y * q.x).abs())
}

fn side(a: Vec2, b: Vec2, p: Vec2) -> f64 {
    let d = (b - a).cross(p - a);
    if d.abs() <= error(a, b, p) { 0. } else { d }
}

fn area(poly: &[Vec2]) -> f64 {
    let Some(origin) = poly.first() else { return 0. };
    poly.iter().zip(poly.iter().cycle().skip(1)).take(poly.len()).map(|(a, b)| (*a - *origin).cross(*b - *origin)).sum::<f64>() * 0.5
}

fn polygon(face: &mt::Face, plane: &Plane, work: &mut usize) -> Result<Option<Vec<Vec2>>> {
    if face.boundaries().len() != 1 {
        return Ok(None);
    }
    let mut result = Vec::new();
    let mut last = None;
    for edge in face.boundary_iters().into_iter().flatten() {
        charge(work, 1)?;
        if result.len() == MAX_VERTICES {
            return Ok(None);
        }
        let mt::Curve::Line(line) = edge.curve() else { return Ok(None) };
        let (a, b) = (from_p3(line.0), from_p3(line.1));
        if !a.is_finite() || !b.is_finite() || a.len().max(b.len()) > 1e9 {
            return Ok(None);
        }
        let tolerance = 4096. * f64::EPSILON * (1. + a.len() + b.len() + plane.origin.len());
        if a.dist(from_p3(edge.absolute_front().point())) > tolerance
            || b.dist(from_p3(edge.absolute_back().point())) > tolerance
            || plane.height(a).abs() > tolerance
            || plane.height(b).abs() > tolerance
        {
            return Ok(None);
        }
        let (a, b) = if edge.orientation() { (a, b) } else { (b, a) };
        if last.is_some_and(|previous: solidcraft_geom::Vec3| previous.dist(a) > tolerance) {
            return Ok(None);
        }
        result.push(plane.to_local(a));
        last = Some(b);
    }
    if result.len() < 3 || result.iter().any(|p| !p.is_finite()) {
        return Ok(None);
    }
    let Some(first) = result.first().copied() else { return Ok(None) };
    let Some(last) = last.map(|p| plane.to_local(p)) else { return Ok(None) };
    if first.dist(last) > 4096. * f64::EPSILON * (1. + first.len() + last.len()) {
        return Ok(None);
    }
    let a = area(&result);
    if !a.is_finite() || a.abs() <= result.iter().map(|p| error(first, *p, *p)).sum::<f64>() {
        return Ok(None);
    }
    if a < 0. {
        result.reverse();
    }
    // Every point must lie in every edge half-space. This rejects concavity,
    // self-intersections and uncertain tiny loops, including holes.
    for (a, b) in result.iter().zip(result.iter().cycle().skip(1)).take(result.len()) {
        for p in &result {
            charge(work, 1)?;
            if side(*a, *b, *p) < 0. {
                return Ok(None);
            }
        }
    }
    Ok(Some(result))
}

fn clip(poly: &[Vec2], a: Vec2, b: Vec2, inside: bool, work: &mut usize) -> Result<Vec<Vec2>> {
    let mut result = Vec::new();
    for (p, q) in poly.iter().zip(poly.iter().cycle().skip(1)).take(poly.len()) {
        charge(work, 1)?;
        let sign = if inside { 1. } else { -1. };
        let (dp, dq) = (side(a, b, *p) * sign, side(a, b, *q) * sign);
        if dp >= 0. {
            result.push(*p);
        }
        if (dp < 0. && dq > 0.) || (dp > 0. && dq < 0.) {
            let t = dp / (dp - dq);
            let point = *p + (*q - *p) * t;
            if !point.is_finite() {
                return Err(KernelError::Invalid("non-finite planar coverage intersection".into()));
            }
            result.push(point);
        }
    }
    if result.len() > MAX_VERTICES {
        return Err(KernelError::Invalid("finite planar lineage vertex budget exceeded".into()));
    }
    if result.len() < 3 || area(&result) <= 0. {
        result.clear();
    }
    Ok(result)
}

fn intersection(mut poly: Vec<Vec2>, cutter: &[Vec2], work: &mut usize) -> Result<Vec<Vec2>> {
    for (a, b) in cutter.iter().zip(cutter.iter().cycle().skip(1)).take(cutter.len()) {
        poly = clip(&poly, *a, *b, true, work)?;
        if poly.is_empty() {
            break;
        }
    }
    Ok(poly)
}

fn subtract(mut poly: Vec<Vec2>, cutter: &[Vec2], work: &mut usize) -> Result<Vec<Vec<Vec2>>> {
    let mut outside = Vec::new();
    for (a, b) in cutter.iter().zip(cutter.iter().cycle().skip(1)).take(cutter.len()) {
        let piece = clip(&poly, *a, *b, false, work)?;
        if !piece.is_empty() {
            outside.push(piece);
        }
        poly = clip(&poly, *a, *b, true, work)?;
        if poly.is_empty() {
            break;
        }
    }
    Ok(outside)
}

fn leaves(source: &FaceSource) -> Option<Vec<&FaceSource>> {
    if is_leaf(source) {
        return Some(vec![source]);
    }
    match source {
        FaceSource::PlanarUnion { sources, plane, .. }
            if (2..=64).contains(&sources.len())
                && sources.iter().all(|s| is_leaf(s) && certified_plane(s).is_some_and(|p| same_support(&p, plane))) =>
        {
            Some(sources.iter().collect())
        }
        _ => None,
    }
}

fn retain_other(source: Option<&FaceSource>) -> Option<FaceSource> {
    source.filter(|s| !is_leaf(s) && !matches!(s, FaceSource::PlanarUnion { .. }) && descriptor_cost(s).is_some()).cloned()
}

fn preserved_unique_support(
    plane: &Plane,
    operands: &[(&mt::Face, Option<&FaceSource>)],
    preserved: Option<&FaceSource>,
    work: &mut usize,
) -> Result<Option<FaceSource>> {
    let Some(preserved) = preserved.filter(|s| is_leaf(s)) else { return Ok(None) };
    let mut matching = None;
    for (face, source) in operands {
        charge(work, 1)?;
        let Some(actual) = actual_plane(face) else { continue };
        let reversed = Plane { y: -actual.y, ..actual };
        if !same_support(&actual, plane) && !same_support(&reversed, plane) {
            continue;
        }
        // A second support, even unknown or oppositely oriented, prevents
        // carrying one operand's identity through this unsupported boundary.
        if matching.is_some() || !same_support(&actual, plane) || *source != Some(preserved) {
            return Ok(None);
        }
        matching = Some(preserved.clone());
    }
    Ok(matching)
}

fn lineage(
    face: &mt::Face,
    operands: &[(&mt::Face, Option<&FaceSource>)],
    preserved: Option<&FaceSource>,
    work: &mut usize,
) -> Result<Option<FaceSource>> {
    let Some(plane) = actual_plane(face) else { return Ok(None) };
    let Some(output) = polygon(face, &plane, work)? else {
        // The boolean's support mapper can already certify an unchanged face
        // with a hole or concave boundary. Do not erase that proof merely
        // because convex polygon coverage cannot represent its boundary.
        return preserved_unique_support(&plane, operands, preserved, work);
    };
    let mut uncovered = vec![output.clone()];
    let mut contributions = BTreeMap::new();
    let mut first_source = None;
    let mut identical = true;
    for (input, source) in operands {
        charge(work, 1)?;
        let Some(actual) = actual_plane(input) else { continue };
        let reversed = Plane { y: -actual.y, ..actual };
        if !same_support(&actual, &plane) && !same_support(&reversed, &plane) {
            continue;
        }
        // An unbounded/curved boundary on the same support cannot be excluded.
        let Some(boundary) = polygon(input, &plane, work)? else { return Ok(None) };
        if intersection(output.clone(), &boundary, work)?.is_empty() {
            continue;
        }
        let Some(source) = source.filter(|source| certified_plane(source).is_some_and(|p| same_support(&p, &plane))) else { return Ok(None) };
        if !same_support(&actual, &plane) {
            return Ok(None);
        }
        let Some(parts) = leaves(source) else { return Ok(None) };
        if let Some(previous) = first_source {
            identical &= previous == source;
        } else {
            first_source = Some(source);
        }
        for part in parts {
            let Some(key) = key(part) else { return Ok(None) };
            // One role on two different canonical frames is ambiguous, even
            // when both finite patches happen to overlap this result.
            if let Some(previous) = contributions.insert(key, part)
                && previous != part
            {
                return Ok(None);
            }
            if contributions.len() > 64 {
                return Ok(None);
            }
        }
        let mut next = Vec::new();
        for piece in uncovered {
            next.extend(subtract(piece, &boundary, work)?);
            if next.len() > MAX_PIECES {
                return Err(KernelError::Invalid("finite planar lineage fragment budget exceeded".into()));
            }
        }
        uncovered = next;
    }
    if !uncovered.is_empty() || contributions.is_empty() {
        return Ok(None);
    }
    if identical {
        return Ok(first_source.cloned());
    }
    let sources: Vec<_> = contributions.into_values().cloned().collect();
    let Some(first) = sources.first() else { return Ok(None) };
    // Keep the first operand's construction frame. Sorting by producer IDs
    // would change this frame when a component copy remaps those IDs.
    let Some(canonical) = first_source.and_then(certified_plane) else { return Ok(None) };
    if sources.len() == 1 {
        return Ok(Some(first.clone()));
    }
    Ok(Some(FaceSource::PlanarUnion { producer: None, sources, plane: canonical }))
}

impl Body {
    pub(crate) fn with_union_planar_sources(mut self, a: &Body, b: &Body) -> Result<Self> {
        if ![a, b].iter().any(|body| body.face_sources().is_some_and(|s| s.iter().flatten().any(|s| leaves(s).is_some()))) {
            return Ok(self);
        }
        if a.solid.face_iter().count() + b.solid.face_iter().count() + self.solid.face_iter().count() > 16_384 {
            self.face_sources = self.face_sources().map(|sources| Arc::new(sources.iter().map(|s| retain_other(s.as_ref())).collect()));
            return Ok(self);
        }
        let mapped = guard("finite planar union lineage", || {
            let operands: Vec<_> = [a, b]
                .into_iter()
                .flat_map(|body| {
                    body.solid.face_iter().enumerate().map(move |(i, f)| (f, body.face_sources().and_then(|s| s.get(i)).and_then(Option::as_ref)))
                })
                .collect();
            let mut work = 2_000_000;
            let mut bytes = 16_000_000usize;
            let mut result = Vec::new();
            for (i, face) in self.solid.face_iter().enumerate() {
                let old = self.face_sources().and_then(|s| s.get(i)).and_then(Option::as_ref);
                // Provenance is optional; exhausting its budget revokes it,
                // without turning a valid geometric union into a failed edit.
                let mapped = if work == 0 {
                    None
                } else {
                    match lineage(face, &operands, old, &mut work) {
                        Ok(source) => source,
                        Err(_) => {
                            work = 0;
                            None
                        }
                    }
                };
                let source = mapped.or_else(|| retain_other(old)).filter(|s| {
                    if let Some(remaining) = descriptor_cost(s).and_then(|cost| bytes.checked_sub(cost)) {
                        bytes = remaining;
                        true
                    } else {
                        false
                    }
                });
                result.push(source);
            }
            Ok(result)
        })?;
        self.face_sources = Some(Arc::new(mapped));
        self.regularize_certified_planar_faces()
    }

    /// Stamp only freshly combined faces; unchanged existing joins keep their identity.
    pub fn with_union_certificate(mut self, producer: u64) -> Result<Self> {
        if producer == 0 {
            return Err(KernelError::Invalid("invalid planar union producer".into()));
        }
        if let Some(sources) = &self.face_sources {
            let mut sources = sources.as_ref().clone();
            for source in sources.iter_mut().flatten() {
                if let FaceSource::PlanarUnion { producer: p, .. } = source
                    && p.is_none()
                {
                    *p = Some(producer);
                }
            }
            self.face_sources = Some(Arc::new(sources));
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoolOp, boolean, box_solid, measure};
    use solidcraft_geom::Vec3;

    fn block(x: f64, z: f64, width: f64, height: f64, producer: u64) -> Body {
        box_solid(Vec3::new(x, 0., z), Vec3::new(x + width, 20., z + height)).unwrap().with_box_certificate(producer).unwrap()
    }

    fn contributions(body: &Body, normal: Vec3) -> Vec<u64> {
        let sources: Vec<_> =
            body.face_sources().unwrap().iter().flatten().filter(|s| certified_plane(s).is_some_and(|p| p.normal().dot(normal) > 0.999)).collect();
        assert_eq!(sources.len(), 1, "{sources:?}");
        leaves(sources[0])
            .unwrap()
            .into_iter()
            .map(|s| match s {
                FaceSource::Box { producer: Some(p), .. } => *p,
                _ => panic!("box leaf"),
            })
            .collect()
    }

    #[test]
    fn cylinder_cap_identity_requires_a_unique_coplanar_support() {
        let cylinder = crate::cylinder(Vec3::ZERO, Vec3::Z, 10., 40.).unwrap().with_cylinder_certificate(1).unwrap();
        let other = block(100., 10., 20., 10., 2);
        let preserved = cylinder.clone().with_union_planar_sources(&cylinder, &other).unwrap();
        let caps = |body: &Body| body.face_sources().unwrap().iter().flatten().filter(|s| matches!(s, FaceSource::CylinderCap { .. })).count();
        assert_eq!(caps(&preserved), 2);
        let competing = crate::cylinder(Vec3::new(5., 0., 0.), Vec3::Z, 10., 40.).unwrap().with_cylinder_certificate(3).unwrap();
        // Curved coplanar boundaries are outside finite polygon coverage. A
        // previously mapped cap alone cannot prove the output's construction.
        let revoked = cylinder.clone().with_union_planar_sources(&cylinder, &competing).unwrap();
        assert_eq!(caps(&revoked), 0);
    }

    #[test]
    fn coincident_join_faces_retain_all_finite_contributions() {
        for reverse in [false, true] {
            let a = block(0., 0., 30., 10., 1);
            let b = block(0., 0., 30., 20., 2);
            let joined = if reverse { boolean(&b, &a, BoolOp::Union) } else { boolean(&a, &b, BoolOp::Union) }
                .unwrap()
                .unwrap()
                .with_union_certificate(3)
                .unwrap();
            assert_eq!(contributions(&joined, Vec3::X), [1, 2]);
            assert_eq!(contributions(&joined, -Vec3::Z), [1, 2]);
            assert_eq!(contributions(&joined, Vec3::Z), [2]);
            assert!((measure(&joined).unwrap().volume - 12_000.).abs() < 1e-5);
        }
    }

    #[test]
    fn unrelated_coplanar_faces_and_uncovered_gaps_never_gain_lineage() {
        let a = block(0., 0., 10., 10., 1);
        let b = block(20., 0., 10., 10., 2);
        let impossible = block(0., 0., 30., 10., 9).with_union_planar_sources(&a, &b).unwrap();
        assert!(!impossible.face_sources().unwrap().iter().flatten().any(|s| certified_plane(s).is_some_and(|p| p.normal().dot(Vec3::Z) > 0.999)));
        // A nearby coplanar but disjoint operand must neither block nor be
        // included in the first body's finite source identity.
        let unchanged = a.clone().with_union_planar_sources(&a, &b).unwrap();
        assert_eq!(contributions(&unchanged, Vec3::Z), [1]);
        let unknown = block(0., 0., 10., 10., 2).with_face_sources(None);
        let revoked = a.clone().with_union_planar_sources(&a, &unknown).unwrap();
        assert!(revoked.face_sources().unwrap().iter().all(Option::is_none));
    }

    #[test]
    fn repeated_joins_flatten_and_transform_contributions() {
        let a = block(0., 0., 30., 10., 1);
        let b = block(0., 0., 30., 20., 2);
        let first = boolean(&a, &b, BoolOp::Union).unwrap().unwrap().with_union_certificate(3).unwrap();
        let c = block(0., 0., 30., 30., 4);
        let joined = boolean(&first, &c, BoolOp::Union).unwrap().unwrap().with_union_certificate(5).unwrap();
        assert_eq!(contributions(&joined, Vec3::X), [1, 2, 4]);
        let moved = crate::transform_matrix(&joined, [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [13., 17., 19., 1.]]).unwrap();
        assert_eq!(contributions(&moved, Vec3::X), [1, 2, 4]);
        for source in moved.face_sources().unwrap().iter().flatten() {
            if let FaceSource::PlanarUnion { plane, sources, producer } = source {
                assert_eq!(*producer, Some(5));
                assert!(sources.iter().all(|s| same_support(&certified_plane(s).unwrap(), plane)));
            }
        }
    }
}
