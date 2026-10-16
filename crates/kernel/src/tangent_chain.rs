//! Smooth edge continuations from exact topology and endpoint derivatives.
use std::collections::{HashMap, HashSet, VecDeque};

use solidcraft_geom::Vec3;
use truck_modeling::{self as mt, BoundedCurve, ParametricCurve};

use crate::{Body, KernelError, Result, guard};

/// Resolve selected seed edges and their tangent continuations in the current body.
/// The returned interior points are transient: documents retain only seed references.
/// Separate coincident vertices are not welded, and ambiguous branches are refused.
pub fn tangent_edge_points(body: &Body, seeds: &[Vec3]) -> Result<Vec<Vec3>> {
    guard("tangent chain", || resolve(body, seeds))
}

fn invalid(message: &str) -> KernelError {
    KernelError::Invalid(message.into())
}

fn resolve(body: &Body, seeds: &[Vec3]) -> Result<Vec<Vec3>> {
    if body.mesh.is_some() || seeds.is_empty() || seeds.len() > 1000 {
        return Err(invalid("tangent chains require 1…1000 solid edge selections"));
    }
    let edges = Body::unique_edges(&body.solid);
    if edges.len() > 100_000 {
        return Err(invalid("too many edges for tangent-chain selection"));
    }
    let mut incident = HashMap::<mt::VertexID, Vec<(usize, Vec3)>>::new();
    for (index, edge) in edges.iter().enumerate() {
        let curve = edge.oriented_curve();
        let (a, b) = curve.range_tuple();
        for (vertex, parameter, sign) in [(edge.front().id(), a, 1.), (edge.back().id(), b, -1.)] {
            let d = curve.der(parameter);
            let direction = Vec3::new(d.x, d.y, d.z) * sign;
            if let Some(direction) = direction.normalized() {
                incident.entry(vertex).or_default().push((index, direction));
            }
        }
    }
    let tolerance = (body.size() * 2e-3).max(1e-3);
    let polylines = body.edges(tolerance)?;
    let mut selected = HashSet::new();
    let mut todo = VecDeque::new();
    let mut smooth = HashMap::new();
    for point in seeds {
        if ![point.x, point.y, point.z].iter().all(|x| x.is_finite()) {
            return Err(invalid("invalid tangent-chain seed"));
        }
        let (index, distance) = polylines
            .iter()
            .flat_map(|edge| edge.points.windows(2).filter_map(move |pair| Some((edge.index, point.dist_to_segment(*pair.first()?, *pair.get(1)?)))))
            .filter(|(_, distance)| distance.is_finite())
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .ok_or_else(|| invalid("missing tangent-chain seed"))?;
        if distance > tolerance * 2. {
            return Err(invalid("no edge at the tangent-chain seed"));
        }
        if selected.insert(index) {
            todo.push_back(index);
        }
    }
    while let Some(index) = todo.pop_front() {
        let edge = edges.get(index).ok_or_else(|| invalid("missing tangent-chain edge"))?;
        for vertex in [edge.front().id(), edge.back().id()] {
            let ends = incident.get(&vertex).ok_or_else(|| invalid("degenerate tangent-chain endpoint"))?;
            if !ends.iter().any(|(i, _)| *i == index) {
                return Err(invalid("degenerate tangent-chain endpoint"));
            }
            for (_, direction) in ends.iter().filter(|(i, _)| *i == index) {
                // A curve can continue smoothly into an internal surface seam.
                // Only corners belong to a blend chain. Keep explicit seeds so
                // selecting a smooth edge still produces the ordinary refusal.
                let mut continuations = ends.iter().filter(|(i, d)| {
                    *i != index
                        && direction.dot(*d) < -1. + 1e-8
                        && !*smooth.entry(*i).or_insert_with(|| edges.get(*i).is_some_and(|e| crate::blend::tangent_faces(&body.solid, e)))
                });
                let next = continuations.next();
                if continuations.next().is_some() {
                    return Err(invalid("ambiguous tangent-chain branch"));
                }
                if let Some((next, _)) = next
                    && selected.insert(*next)
                {
                    if selected.len() > 1000 {
                        return Err(invalid("tangent chain exceeds 1000 edges"));
                    }
                    todo.push_back(*next);
                }
            }
        }
    }
    edges
        .iter()
        .enumerate()
        .filter(|(index, _)| selected.contains(index))
        .map(|(_, edge)| {
            let curve = edge.oriented_curve();
            let (a, b) = curve.range_tuple();
            let p = curve.subs((a + b) * 0.5);
            if ![p.x, p.y, p.z].iter().all(|x| x.is_finite()) {
                return Err(invalid("invalid tangent-chain midpoint"));
            }
            Ok(Vec3::new(p.x, p.y, p.z))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_tangents_follow_both_semicircles_without_vertical_seams() {
        let body = crate::cylinder(Vec3::ZERO, Vec3::Z, 10., 40.).unwrap();
        let chain = tangent_edge_points(&body, &[Vec3::new(0., 10., 40.)]).unwrap();
        assert_eq!(chain.len(), 2);
        assert!(chain.iter().all(|p| (p.z - 40.).abs() < 1e-9 && (p.x.hypot(p.y) - 10.).abs() < 1e-9));
        assert_eq!(tangent_edge_points(&body, &chain).unwrap(), chain, "several seeds on one chain are deduplicated");
        let box_body = crate::box_solid(Vec3::ZERO, Vec3::new(20., 30., 40.)).unwrap();
        assert_eq!(tangent_edge_points(&box_body, &[Vec3::new(0., 0., 20.)]).unwrap(), [Vec3::new(0., 0., 20.)]);
    }

    #[test]
    fn invalid_or_unbounded_chain_selections_are_errors() {
        let body = crate::cylinder(Vec3::ZERO, Vec3::Z, 10., 40.).unwrap();
        for seeds in [vec![], vec![Vec3::new(f64::NAN, 0., 0.)], vec![Vec3::new(100., 100., 100.)], vec![Vec3::ZERO; 1001]] {
            assert!(tangent_edge_points(&body, &seeds).is_err());
        }
    }

    #[test]
    fn an_explicit_smooth_seed_is_not_silently_removed() {
        let body = crate::cylinder(Vec3::ZERO, Vec3::Z, 10., 40.).unwrap();
        let edges = Body::unique_edges(&body.solid);
        let seam = edges.iter().find(|e| (e.front().point().z - e.back().point().z).abs() > 39.).unwrap();
        let curve = seam.oriented_curve();
        let (a, b) = curve.range_tuple();
        let p = crate::body::from_p3(curve.subs((a + b) * 0.5));
        let chain = tangent_edge_points(&body, &[p]).unwrap();
        assert_eq!(chain, [p]);
        assert!(crate::fillet(&body, &chain, 1.).is_err());
    }
}
