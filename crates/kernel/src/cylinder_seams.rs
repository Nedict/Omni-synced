//! Remove coincident cylindrical subdivisions at a selected blend's junction.
//! Keep real corners, deliberate splits and a meridian of a full periodic wall.
use std::collections::{BTreeMap, HashMap};

use solidcraft_geom::Vec3;
use truck_modeling as mt;

use crate::body::{Body, Solid, p3};
use crate::history::{Change, FaceOrigin};
use crate::offset::Surf;
use crate::{Result, record};

pub(crate) fn joined(body: &Body, selected: &[mt::EdgeID]) -> Result<Option<Body>> {
    // A bounded fallback for local modifiers, not a general surface simplifier.
    if body.face_count() > 512 || body.edge_count() > 4096 || !body.split_keep().is_empty() || body.paint().is_some_and(|p| !p.faces.is_empty()) {
        return Ok(None);
    }
    crate::guard("join cylindrical wall subdivisions", || {
        let tol = (body.size() * 1e-7).max(1e-9);
        if !tol.is_finite() {
            return Ok(None);
        }
        let mut shells = Vec::new();
        let mut provenance = Vec::new();
        let mut offset = 0;
        let mut changed = false;
        for shell in body.solid.boundaries() {
            let faces: Vec<_> = shell.face_iter().collect();
            let surfaces: Vec<_> = faces.iter().map(|f| crate::offset::surf_of(f, tol)).collect();
            let junction = shell.edge_iter().filter(|e| selected.contains(&e.id())).any(|edge| {
                [edge.front(), edge.back()].iter().any(|v| {
                    let at: Vec<_> = faces.iter().enumerate().filter(|(_, f)| f.vertex_iter().any(|q| q == **v)).map(|(i, _)| i).collect();
                    at.len() > 3 && at.iter().filter(|i| matches!(surfaces.get(**i), Some(Some(Surf::Cylinder { .. })))).count() > 1
                })
            });
            if !junction {
                shells.push(shell.clone());
                provenance.extend((0..faces.len()).map(|i| record::from_face(0, offset + i, Change::Kept)));
                offset += faces.len();
                continue;
            }
            let mut parents: Vec<_> = (0..faces.len()).collect();
            let mut adjacent: HashMap<_, Vec<usize>> = HashMap::new();
            for (i, face) in faces.iter().enumerate() {
                for edge in face.edge_iter() {
                    adjacent.entry(edge.id()).or_default().push(i);
                }
            }
            for edge in shell.edge_iter() {
                let Some(pair) = adjacent.get(&edge.id()) else { continue };
                let [i, j] = pair.as_slice() else { continue };
                if i == j {
                    continue;
                }
                let (Some(Some(Surf::Cylinder { o, a, r, convex })), Some(Some(Surf::Cylinder { o: q, a: b, r: s, convex: c }))) =
                    (surfaces.get(*i), surfaces.get(*j))
                else {
                    continue;
                };
                let off = *q - *o;
                if convex != c || a.dot(*b).abs() < 1. - 1e-9 || (r - s).abs() > tol || (off - *a * off.dot(*a)).len() > tol {
                    continue;
                }
                let (x, y) = (crate::heal::find(&mut parents, *i), crate::heal::find(&mut parents, *j));
                if let Some(slot) = parents.get_mut(x.max(y)) {
                    *slot = x.min(y);
                }
            }
            let mut groups = BTreeMap::<usize, Vec<usize>>::new();
            for i in 0..faces.len() {
                groups.entry(crate::heal::find(&mut parents, i)).or_default().push(i);
            }
            let mut output = Vec::new();
            for members in groups.values() {
                let merged = merge(&faces, &surfaces, members, tol);
                if let Some(face) = merged {
                    output.push(face);
                    provenance.push(record::region(members.iter().map(|i| FaceOrigin::Face { operand: 0, face: offset + i }).collect(), false));
                    changed = true;
                } else {
                    for i in members {
                        if let Some(face) = faces.get(*i) {
                            output.push((*face).clone());
                            provenance.push(record::from_face(0, offset + i, Change::Kept));
                        }
                    }
                }
            }
            shells.push(output.into());
            offset += faces.len();
        }
        if !changed {
            return Ok(None);
        }
        let solid = Solid::try_new(shells).map_err(|e| crate::KernelError::Failed(format!("joined cylindrical wall: {e}")))?;
        let edges = record::edge_origins(&body.solid, 0);
        Ok(Some(record::build(solid, &record::counts(&[body]), Some(provenance), Some(&edges))?))
    })
}

fn merge(faces: &[&mt::Face], surfaces: &[Option<Surf>], members: &[usize], tol: f64) -> Option<mt::Face> {
    if members.len() < 2 {
        return None;
    }
    let Surf::Cylinder { o, a, r, convex } = (*surfaces.get(*members.first()?)?)? else { return None };
    let mut counts = HashMap::new();
    for i in members {
        for edge in faces.get(*i)?.edge_iter() {
            *counts.entry(edge.id()).or_insert(0_usize) += 1;
        }
    }
    let mut rest = Vec::new();
    for i in members {
        for edge in faces.get(*i)?.boundary_iters().into_iter().flatten() {
            if counts.get(&edge.id()) == Some(&1) {
                rest.push(edge);
            }
        }
    }
    let mut loops = Vec::new();
    while let Some(first) = rest.pop() {
        let (start, mut end) = (first.front().clone(), first.back().clone());
        let mut chain = vec![first];
        while end != start {
            let index = rest.iter().position(|e| e.front() == &end)?;
            let next = rest.remove(index);
            end = next.back().clone();
            chain.push(next);
        }
        if !mt::Wire::from(chain.clone()).is_simple() {
            return None;
        }
        loops.push(chain);
    }
    let radial = |p: Vec3| p - o - a * (p - o).dot(a);
    let (chain, seam) = match loops.len() {
        1 => (loops.pop()?, None),
        2 => {
            let mut first = loops.pop()?;
            let mut second = loops.pop()?;
            let winding = |edges: &[mt::Edge]| -> f64 {
                edges
                    .iter()
                    .map(|edge| {
                        crate::heal::edge_points(edge)
                            .windows(2)
                            .filter_map(|pair| {
                                let [p, q] = pair else { return None };
                                let (p, q) = (radial(*p), radial(*q));
                                Some(a.dot(p.cross(q)).atan2(p.dot(q)))
                            })
                            .sum::<f64>()
                            * if edge.orientation() { 1. } else { -1. }
                    })
                    .sum()
            };
            let (wa, wb) = (winding(&first), winding(&second));
            if (wa.abs() - std::f64::consts::TAU).abs() > 1e-6 || (wa + wb).abs() > 1e-6 {
                return None;
            }
            // A Boolean may divide one meridian at several height stations.
            // Follow its existing internal edges; only intermediate vertices
            // owned entirely by this cylindrical family may be removed.
            let internal: Vec<_> =
                members.iter().filter_map(|i| faces.get(*i)).flat_map(|f| f.edge_iter()).filter(|e| counts.get(&e.id()) == Some(&2)).collect();
            let outside = |v: &mt::Vertex| faces.iter().enumerate().filter(|(j, f)| !members.contains(j) && f.vertex_iter().any(|q| q == *v)).count();
            let mut bridge = None;
            'found: for (i, start_edge) in first.iter().enumerate() {
                let start = start_edge.front();
                if outside(start) != 1 {
                    continue;
                }
                let p = crate::body::from_p3(start.point());
                for initial in internal.iter().filter(|e| e.front() == start || e.back() == start) {
                    let initial = if initial.front() == start { initial.clone() } else { initial.inverse() };
                    let direction = (crate::body::from_p3(initial.back().point()) - p).dot(a).signum();
                    let mut path = vec![initial];
                    let mut used = std::collections::HashSet::new();
                    loop {
                        let last = path.last()?;
                        if !used.insert(last.id())
                            || crate::heal::edge_points(last).iter().any(|q| {
                                let d = *q - p;
                                (d - a * d.dot(a)).len() > tol
                            })
                        {
                            break;
                        }
                        let end = last.back();
                        if let Some(j) = second.iter().position(|e| e.front() == end) {
                            if outside(end) != 1 {
                                break;
                            }
                            let edge = if path.len() == 1 { last.clone() } else { mt::builder::line(start, end) };
                            first.rotate_left(i);
                            second.rotate_left(j);
                            bridge = Some(edge);
                            break 'found;
                        }
                        if outside(end) != 0 {
                            break;
                        }
                        let q = crate::body::from_p3(end.point());
                        let next = internal.iter().filter(|e| !used.contains(&e.id())).find_map(|e| {
                            let e = if e.front() == end {
                                e.clone()
                            } else if e.back() == end {
                                e.inverse()
                            } else {
                                return None;
                            };
                            let delta = crate::body::from_p3(e.back().point()) - q;
                            (delta.dot(a) * direction > 1e-9 && (delta - a * delta.dot(a)).len() <= tol).then_some(e)
                        });
                        let Some(next) = next else { break };
                        path.push(next);
                    }
                }
            }
            let bridge = bridge?;
            let seam = radial(crate::body::from_p3(bridge.front().point())).normalized()?;
            let mut chain = vec![bridge.clone()];
            chain.extend(second);
            chain.push(bridge.inverse());
            chain.extend(first);
            (chain, Some(seam))
        }
        _ => return None,
    };
    let points: Vec<_> = chain.iter().flat_map(crate::heal::edge_points).collect();
    if points.iter().any(|p| !p.is_finite() || (radial(*p).len() - r).abs() > tol) {
        return None;
    }
    let x = a.any_perp();
    let y = a.cross(x);
    let mut angles: Vec<_> = points.iter().map(|p| radial(*p).dot(y).atan2(radial(*p).dot(x))).collect();
    angles.sort_by(f64::total_cmp);
    let mut gap = (0., 0.);
    for (i, angle) in angles.iter().enumerate() {
        let next = angles.get(i + 1).copied().unwrap_or(*angles.first()? + std::f64::consts::TAU);
        if next - angle > gap.0 {
            gap = (next - angle, next);
        }
    }
    let span = std::f64::consts::TAU - gap.0;
    if seam.is_none() && span > std::f64::consts::TAU - 1e-6 {
        return None;
    }
    let angle = gap.1 + span * 0.5;
    let mid = x * angle.cos() + y * angle.sin();
    let (lo, hi) = points.iter().map(|p| (*p - o).dot(a)).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| (lo.min(h), hi.max(h)));
    if !lo.is_finite() || !hi.is_finite() || hi - lo <= tol {
        return None;
    }
    let base = o + seam.unwrap_or(-mid) * r;
    let (low, high) = (p3(base + a * lo), p3(base + a * hi));
    let line = mt::Curve::Line(if convex { mt::Line(high, low) } else { mt::Line(low, high) });
    let surface = mt::Surface::RevolutedCurve(mt::Processor::new(mt::RevolutedCurve::by_revolution(line, p3(o), mt::Vector3::new(a.x, a.y, a.z))));
    // A full periodic wall uses its one retained meridian twice, in opposite
    // directions. Truck's simple-wire constructor rejects that valid seam.
    let result =
        if seam.is_some() { Ok(mt::Face::new_unchecked(vec![chain.into()], surface)) } else { mt::Face::try_new(vec![chain.into()], surface) };
    result.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transverse_cylinder_subdivisions_do_not_stop_vertical_grip_rounds() {
        let base = crate::cylinder(Vec3::ZERO, Vec3::Z, 10., 8.).unwrap();
        let top = crate::cylinder(Vec3::Z * 8., Vec3::Z, 10., 12.).unwrap();
        let strip = crate::box_solid(Vec3::new(-3., -20., 7.), Vec3::new(3., 20., 21.)).unwrap();
        let top = crate::boolean(&top, &strip, crate::BoolOp::Intersect).unwrap().unwrap();
        let body = crate::boolean(&base, &top, crate::BoolOp::Union).unwrap().unwrap();
        let y = 91_f64.sqrt();
        let picks = [Vec3::new(-3., -y, 14.), Vec3::new(-3., y, 14.), Vec3::new(3., -y, 14.), Vec3::new(3., y, 14.)];
        let all = Body::unique_edges(&body.solid);
        let ids: Vec<_> = picks.iter().map(|p| all[body.nearest_edge(*p, 1e-5).unwrap().unwrap().0].id()).collect();
        let healed = joined(&body, &ids).unwrap().expect("transverse subdivisions");
        assert!(healed.face_count() < body.face_count());
        assert!(healed.history().is_some());
        assert!(healed.validity().is_empty(), "{:?}", healed.validity());
        let volume = |b: &Body| b.tessellate_with(1e-5, true).unwrap().measure().volume;
        assert!((volume(&body) - volume(&healed)).abs() < 0.01);
        let wall = healed.solid.face_iter().find(|f| f.absolute_boundaries().iter().any(|w| !w.is_simple())).unwrap();
        let wires = wall.absolute_boundaries().clone();
        assert!(crate::heal::absolute_face(wall, wires.clone()).is_some());
        let mut broken = wires;
        let edges: Vec<_> = broken[0].edge_iter().cloned().collect();
        let start = edges[0].front();
        let spur = mt::builder::line(start, &mt::builder::vertex(start.point() + mt::Vector3::new(1., 2., 3.)));
        broken[0] = std::iter::once(spur.clone()).chain(std::iter::once(spur.inverse())).chain(edges).collect();
        assert!(crate::heal::absolute_face(wall, broken).is_none(), "a retained seam cannot authorize another repeated edge");
        // Integrate the circle segment minus the tangent round's segment for
        // each corner, independently of the B-rep and its tessellation.
        let integral = |r: f64, x: f64| 0.5 * (x * (r * r - x * x).max(0.).sqrt() + r * r * (x / r).asin());
        let stock = std::f64::consts::PI * 100. * 8. + 4. * integral(10., 3.) * 12.;
        assert!((volume(&body) - stock).abs() < 0.01, "stock {} != {stock}", volume(&body));
        for radius in [0.5_f64, 1., 1.5] {
            let cx = 3. - radius;
            let cy = ((10. - radius).powi(2) - cx * cx).sqrt();
            let tx = 10. * cx / (10. - radius);
            let removed = 4. * 12. * (integral(10., 3.) - integral(10., tx) - (3. - tx) * cy - integral(radius, radius) + integral(radius, tx - cx));
            let rounded = crate::fillet(&body, &picks, radius).unwrap_or_else(|e| {
                panic!("fallback failed at radius {radius}: {e}; direct: {:?}", crate::fillet(&healed, &picks, radius).err());
            });
            assert!(rounded.validity().is_empty(), "{:?}", rounded.validity());
            assert_eq!(rounded.crossed_face_loops().unwrap(), 0);
            assert!(rounded.history().is_some());
            let got = volume(&rounded);
            assert!((got - (stock - removed)).abs() < 0.02, "radius={radius}: {got} != {}", stock - removed);
        }
        // Rotate the cylinder axis as well as the grip and translate away from
        // the origin; neither seam selection nor shoulder sense is world-Z based.
        let matrix = [[0., 1., 0., 0.], [0., 0., 1., 0.], [1., 0., 0., 0.], [23., -17., 9., 1.]];
        let placed = crate::transform_matrix(&body, matrix).unwrap();
        let picks = picks.map(|p| Vec3::new(p.z + 23., p.x - 17., p.y + 9.));
        let rounded = crate::fillet(&placed, &picks, 1.).unwrap();
        assert!(rounded.validity().is_empty(), "{:?}", rounded.validity());
        assert_eq!(rounded.crossed_face_loops().unwrap(), 0);
        assert!((volume(&rounded) - 3925.8150032126127).abs() < 0.02);
        assert!(joined(&base, &[]).unwrap().is_none());
        let kept = body.clone().with_split_lines(Some(std::sync::Arc::new(vec![vec![Vec3::ZERO, Vec3::Z]])));
        assert!(joined(&kept, &ids).unwrap().is_none());
    }
}
