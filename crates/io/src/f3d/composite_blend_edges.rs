//! Select a unique span or complete circle through logical face incidence.
//! Kernel seam subdivisions are internal to a native face family, not extra
//! native endpoints. Coordinates only certify the already selected topology.
use super::*;

fn circle_coverage(mut intervals: Vec<(f64, f64)>) -> Result<()> {
    // Fitted carrier axes can place a seam a few ulps below zero. Splitting
    // that wrap produces a zero-length interval at the opposite end; it is not
    // an overlapping half circle. Real overlaps and gaps remain errors.
    intervals.retain(|(start, end)| end - start > 1e-7);
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut cursor: f64 = 0.;
    for (start, end) in intervals {
        if !start.is_finite() || !end.is_finite() || (start - cursor).abs() > 1e-7 {
            return Err(invalid("circular blend pieces overlap or leave a gap"));
        }
        cursor = end;
    }
    if (cursor - std::f64::consts::TAU).abs() > 1e-7 {
        return Err(invalid("circular blend pieces leave a gap"));
    }
    Ok(())
}

pub(super) fn resolve(body: &ModelBody, edge: &recipe_incidence::Edge, indices: &BTreeMap<Term, BTreeSet<usize>>) -> Result<Vec<Selection>> {
    resolve_selected(body, edge, indices, None)
}

fn resolve_selected(
    body: &ModelBody,
    edge: &recipe_incidence::Edge,
    indices: &BTreeMap<Term, BTreeSet<usize>>,
    only: Option<&BTreeSet<usize>>,
) -> Result<Vec<Selection>> {
    let mut owners = BTreeMap::new();
    for (term, faces) in indices {
        for face in faces {
            if owners.insert(*face, term).is_some() {
                return Err(invalid("distinct blend face identities collapse"));
            }
        }
    }
    let mesh = body.mesh();
    if mesh.edges.len() > 16_384 || mesh.edges.len() != mesh.edge_faces.len() {
        return Err(invalid("blend topology exceeds its work budget"));
    }
    let incident: BTreeSet<_> = edge.faces.iter().collect();
    if incident.len() != 2 || incident.iter().any(|t| !indices.contains_key(*t)) {
        return Err(invalid("blend lacks two distinct named incident faces"));
    }
    let selected = mesh
        .edge_faces
        .iter()
        .enumerate()
        .filter_map(|(index, faces)| {
            if only.is_some_and(|allowed| !allowed.contains(&index)) {
                return None;
            }
            let mapped = faces.iter().map(|i| owners.get(&(*i as usize)).copied()).collect::<Option<BTreeSet<_>>>()?;
            (mapped == incident).then_some(index)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() || selected.len() > 128 {
        return Err(invalid("blend edge incidence is missing or excessive"));
    }
    let tolerance = 1e-7 * (1.0 + body.body.size());
    if !tolerance.is_finite() || tolerance <= 0. {
        return Err(invalid("invalid blend scale"));
    }
    let at = |point: Vec3| -> Result<BTreeSet<Term>> {
        let mut adjacent = BTreeSet::new();
        for (samples, faces) in mesh.edges.iter().zip(&mesh.edge_faces) {
            if samples.first().is_some_and(|p| p.dist(point) <= tolerance) || samples.last().is_some_and(|p| p.dist(point) <= tolerance) {
                for face in faces {
                    adjacent.insert(
                        (*owners.get(&(*face as usize)).ok_or_else(|| {
                            invalid(&format!(
                                "blend endpoint touches an unnamed face {face}: {}",
                                solidcraft_doc::naming::face_names(body).get(*face as usize).map(String::as_str).unwrap_or("unknown")
                            ))
                        })?)
                        .clone(),
                    );
                }
            }
        }
        Ok(adjacent)
    };
    match edge.ends.as_slice() {
        [endpoint] => {
            if endpoint.faces != edge.faces.iter().cloned().collect() {
                return Err(invalid("closed blend has another endpoint face"));
            }
            let mut circles = Vec::new();
            for index in &selected {
                let circle = solidcraft_kernel::edge_circle(&body.body, *index).ok_or_else(|| invalid("closed blend carrier is not circular"))?;
                if at(circle.start)? != endpoint.faces || at(circle.end)? != endpoint.faces {
                    return Err(invalid("closed blend circle has extra incident faces"));
                }
                circles.push(circle);
            }
            let first = circles.first().ok_or_else(|| invalid("missing circular blend carrier"))?;
            let mut angle = 0.;
            for circle in &circles {
                if circle.center.dist(first.center) > tolerance
                    || (circle.radius - first.radius).abs() > tolerance
                    || circle.axis.cross(first.axis).len() > 1e-7
                    || !circle.angle.is_finite()
                    || circle.angle <= 0.
                {
                    return Err(invalid("closed blend incidence identifies different circles"));
                }
                angle += circle.angle;
            }
            if (angle - std::f64::consts::TAU).abs() > 1e-7 {
                return Err(invalid("closed blend incidence does not cover exactly one circle"));
            }
            let tau = std::f64::consts::TAU;
            let x = (first.start - first.center) * (1. / first.radius);
            let y = first.axis.cross(x);
            let mut intervals = Vec::new();
            for circle in &circles {
                let radial = circle.start - first.center;
                let mut start = radial.dot(y).atan2(radial.dot(x));
                if circle.axis.dot(first.axis) < 0. {
                    start -= circle.angle;
                }
                start = start.rem_euclid(tau);
                if (circle.angle - tau).abs() <= 1e-7 {
                    intervals.push((0., tau));
                } else if start + circle.angle <= tau {
                    intervals.push((start, start + circle.angle));
                } else {
                    intervals.push((start, tau));
                    intervals.push((0., start + circle.angle - tau));
                }
            }
            circle_coverage(intervals)?;
            // Each split must have precisely one neighboring split at either
            // end. Duplicate arcs, gaps and branching are not a circular chain.
            for (i, circle) in circles.iter().enumerate() {
                for point in [circle.start, circle.end] {
                    let count = circles
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, other)| usize::from(other.start.dist(point) <= tolerance) + usize::from(other.end.dist(point) <= tolerance))
                        .sum::<usize>();
                    if (circles.len() == 1 && circle.start.dist(circle.end) > tolerance) || (circles.len() > 1 && count != 1) {
                        return Err(invalid("circular blend seam subdivision is not closed"));
                    }
                }
            }
        }
        [a, b] => {
            // A native finite edge can span kernel parameter seams, including
            // spline intersections of earlier rounds. Require one unbranched,
            // connected chain; internal nodes have only its two logical faces.
            let spans = selected
                .iter()
                .map(|index| {
                    let points = mesh.edges.get(*index).ok_or_else(|| invalid("missing finite blend edge samples"))?;
                    let (Some(start), Some(end)) = (points.first(), points.last()) else { return Err(invalid("empty finite blend edge")) };
                    let length = start.dist(*end);
                    if points.len() > 4096 || points.iter().any(|p| !p.is_finite()) || !length.is_finite() || length <= tolerance {
                        return Err(invalid("invalid finite blend edge span"));
                    }
                    Ok([*start, *end])
                })
                .collect::<Result<Vec<_>>>()?;
            let (ends, internal) = finite_chain(&spans, tolerance)?;
            let expected_internal: BTreeSet<_> = edge.faces.iter().cloned().collect();
            for point in internal {
                if at(point)? != expected_internal {
                    return Err(invalid("finite blend chain has another incident face"));
                }
            }
            let mut actual = [at(ends[0])?, at(ends[1])?];
            let mut expected = [a.faces.clone(), b.faces.clone()];
            actual.sort();
            expected.sort();
            // A chord can meet the same cylinder at both ends. The complete
            // native vertex identities must differ, and the resolved face pair
            // must still select exactly one connected, unbranched finite chain.
            if a == b || expected.iter().any(|faces| faces.len() < 3) || actual != expected {
                return Err(invalid("blend endpoints disagree with named face incidence"));
            }
        }
        _ => return Err(invalid("unsupported blend endpoint count")),
    }
    let names = solidcraft_doc::naming::edge_names(body);
    selected
        .into_iter()
        .map(|index| {
            let points = mesh.edges.get(index).ok_or_else(|| invalid("missing blend edge samples"))?;
            let point = if let [a, b] = points.as_slice() {
                (*a + *b) * 0.5
            } else {
                *points.get(points.len() / 2).ok_or_else(|| invalid("empty blend edge samples"))?
            };
            let name = names.get(index).cloned().ok_or_else(|| invalid("missing persistent blend edge name"))?;
            Ok(Selection { body: body.name.clone(), name, point })
        })
        .collect()
}

#[derive(Clone)]
pub(super) struct Candidate {
    pub spans: BTreeSet<usize>,
    pub ends: [Vec3; 2],
    pub end_faces: [BTreeSet<Term>; 2],
    pub selection: Vec<Selection>,
}

/// Enumerate complete connected chains; do not pick between symmetric ones.
/// The caller must certify the entire requested graph before using candidates.
pub(super) fn candidates(body: &ModelBody, edge: &recipe_incidence::Edge, indices: &BTreeMap<Term, BTreeSet<usize>>) -> Result<Vec<Candidate>> {
    if edge.ends.len() != 2 {
        return Err(invalid("group incidence requires finite edges"));
    }
    let mut owners = BTreeMap::new();
    for (term, faces) in indices {
        for face in faces {
            if owners.insert(*face, term).is_some() {
                return Err(invalid("distinct group face identities collapse"));
            }
        }
    }
    let mesh = body.mesh();
    if mesh.edges.len() > 16_384 || mesh.edges.len() != mesh.edge_faces.len() {
        return Err(invalid("group topology exceeds its work budget"));
    }
    let tolerance = 1e-7 * (1. + body.body.size());
    if !tolerance.is_finite() || tolerance <= 0. {
        return Err(invalid("invalid group blend scale"));
    }
    let incident: BTreeSet<_> = edge.faces.iter().collect();
    if incident.len() != 2 {
        return Err(invalid("group edge lacks distinct incident faces"));
    }
    let mut spans = BTreeMap::new();
    for (index, faces) in mesh.edge_faces.iter().enumerate() {
        let mapped = faces.iter().map(|f| owners.get(&(*f as usize)).copied()).collect::<Option<BTreeSet<_>>>();
        if mapped != Some(incident.clone()) {
            continue;
        }
        let points = mesh.edges.get(index).ok_or_else(|| invalid("missing group edge"))?;
        let (Some(a), Some(b)) = (points.first(), points.last()) else {
            return Err(invalid("empty group edge"));
        };
        if !a.is_finite() || !b.is_finite() || a.dist(*b) <= tolerance {
            return Err(invalid("invalid finite group span"));
        }
        spans.insert(index, [*a, *b]);
        if spans.len() > 128 {
            return Err(invalid("excessive group edge candidates"));
        }
    }
    let at = |point: Vec3| -> Option<BTreeSet<Term>> {
        let mut adjacent = BTreeSet::new();
        for (samples, faces) in mesh.edges.iter().zip(&mesh.edge_faces) {
            if samples.first().is_some_and(|p| p.dist(point) <= tolerance) || samples.last().is_some_and(|p| p.dist(point) <= tolerance) {
                for face in faces {
                    adjacent.insert((*owners.get(&(*face as usize))?).clone());
                }
            }
        }
        Some(adjacent)
    };
    let mut pending: BTreeSet<_> = spans.keys().copied().collect();
    let mut out = Vec::new();
    let mut first_error = None;
    while let Some(first) = pending.pop_first() {
        let mut component = BTreeSet::from([first]);
        let mut queue = vec![first];
        while let Some(index) = queue.pop() {
            let pair = spans.get(&index).ok_or_else(|| invalid("missing group chain span"))?;
            let neighbors: Vec<_> = pending
                .iter()
                .copied()
                .filter(|i| spans.get(i).is_some_and(|other| pair.iter().any(|a| other.iter().any(|b| a.dist(*b) <= tolerance))))
                .collect();
            for index in neighbors {
                pending.remove(&index);
                component.insert(index);
                queue.push(index);
            }
        }
        let selection = match resolve_selected(body, edge, indices, Some(&component)) {
            Ok(selection) => selection,
            Err(error) => {
                first_error.get_or_insert_with(|| error.to_string());
                continue;
            }
        };
        let points =
            component.iter().map(|i| spans.get(i).copied().ok_or_else(|| invalid("missing group chain geometry"))).collect::<Result<Vec<_>>>()?;
        let (ends, _) = finite_chain(&points, tolerance)?;
        let (Some(a), Some(b)) = (at(ends[0]), at(ends[1])) else {
            continue;
        };
        out.push(Candidate { spans: component, ends, end_faces: [a, b], selection });
    }
    if out.is_empty() {
        return Err(invalid(&format!("group incidence has no complete finite chain: {}", first_error.as_deref().unwrap_or("no incident edges"))));
    }
    Ok(out)
}

fn finite_chain(spans: &[[Vec3; 2]], tolerance: f64) -> Result<([Vec3; 2], Vec<Vec3>)> {
    let mut nodes: Vec<(Vec3, Vec<usize>)> = Vec::new();
    let mut edges = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        let mut pair = [0; 2];
        for (slot, point) in pair.iter_mut().zip(span) {
            let found = nodes.iter().enumerate().filter(|(_, (p, _))| p.dist(*point) <= tolerance).map(|(i, _)| i).collect::<Vec<_>>();
            *slot = match found.as_slice() {
                [] => {
                    nodes.push((*point, Vec::new()));
                    nodes.len() - 1
                }
                [i] => *i,
                _ => return Err(invalid("ambiguous finite blend seam location")),
            };
            nodes.get_mut(*slot).ok_or_else(|| invalid("missing finite blend seam node"))?.1.push(index);
        }
        if pair.first() == pair.get(1) {
            return Err(invalid("closed span in finite blend chain"));
        }
        edges.push(pair);
    }
    let ends = nodes.iter().filter(|(_, links)| links.len() == 1).map(|(p, _)| *p).collect::<Vec<_>>();
    let [a, b] = ends.as_slice() else { return Err(invalid("finite blend chain needs two endpoints")) };
    if nodes.iter().any(|(_, links)| links.len() > 2) {
        return Err(invalid("finite blend chain branches"));
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![0];
    while let Some(index) = pending.pop() {
        if visited.insert(index) {
            for edge in &nodes.get(index).ok_or_else(|| invalid("missing finite blend node"))?.1 {
                pending.extend(edges.get(*edge).ok_or_else(|| invalid("missing finite blend span"))?);
            }
        }
    }
    if visited.len() != nodes.len() {
        return Err(invalid("finite blend chain is disconnected"));
    }
    Ok(([*a, *b], nodes.into_iter().filter(|(_, links)| links.len() == 2).map(|(p, _)| p).collect()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn finite_seams_require_one_connected_unbranched_chain() {
        use super::{Vec3, finite_chain};
        let p = |x, y| Vec3::new(x, y, 0.);
        let chain = [[p(0., 0.), p(1., 0.)], [p(2., 0.), p(1., 0.)], [p(3., 0.), p(2., 0.)]];
        let (ends, seams) = finite_chain(&chain, 1e-7).unwrap();
        assert_eq!(ends, [p(0., 0.), p(3., 0.)]);
        assert_eq!(seams.len(), 2);
        assert!(finite_chain(&[], 1e-7).is_err());
        assert!(finite_chain(&[[p(0., 0.), p(0., 0.)]], 1e-7).is_err());
        let mut bad = chain.to_vec();
        bad.push(chain[0]);
        assert!(finite_chain(&bad, 1e-7).is_err(), "duplicate span");
        let mut bad = chain.to_vec();
        bad.push([p(1., 0.), p(1., 1.)]);
        assert!(finite_chain(&bad, 1e-7).is_err(), "branch");
        assert!(finite_chain(&[chain[0], chain[2]], 1e-7).is_err(), "gap");
        let mut bad = chain.to_vec();
        bad.extend([[p(0., 2.), p(1., 2.)], [p(1., 2.), p(1., 3.)], [p(1., 3.), p(0., 2.)]]);
        assert!(finite_chain(&bad, 1e-7).is_err(), "disconnected cycle beside a valid chain");
    }

    #[test]
    fn circular_coverage_ignores_roundoff_wraps_but_rejects_duplicate_halves_and_gaps() {
        use super::circle_coverage;
        use std::f64::consts::{PI, TAU};
        assert!(circle_coverage(vec![(0., 1e-15), (0., PI), (PI, TAU)]).is_ok());
        assert!(circle_coverage(vec![(TAU - 1e-15, TAU), (0., PI), (PI, TAU - 1e-15)]).is_ok());
        assert!(circle_coverage(vec![(0., PI), (0., PI)]).is_err());
        assert!(circle_coverage(vec![(0., PI - 0.01), (PI, TAU)]).is_err());
        assert!(circle_coverage(vec![(0., PI + 0.01), (PI, TAU)]).is_err());
        assert!(circle_coverage(vec![(0., PI)]).is_err());
    }
}
