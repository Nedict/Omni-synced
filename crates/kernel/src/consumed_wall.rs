//! A chamfer may consume a complete cylindrical wall and meet its opposite cap.
//! Reuse that cap's edge only after certifying the whole contact curve, including
//! its angular traversal; shared endpoints or a sampled coincidence are not enough.
use std::collections::{HashMap, HashSet};

use solidcraft_geom::{Plane, Seg2};
use truck_modeling as mt;

use crate::body::from_p3;

fn same_arc(a: &[Seg2], b: &[Seg2], tolerance: f64) -> bool {
    let [Seg2::Arc { center: ca, radius: ra, start: sa, sweep: wa }] = a else { return false };
    let [Seg2::Arc { center: cb, radius: rb, start: sb, sweep: wb }] = b else { return false };
    if wa.signum() != wb.signum() {
        return false;
    }
    let angle = (sa - sb).sin().atan2((sa - sb).cos()).abs();
    let separation = ca.dist(*cb) + (ra - rb).abs() + ra.max(*rb) * (angle + (wa - wb).abs());
    separation.is_finite() && separation <= tolerance
}

pub(super) fn contact(
    wall: &mt::Face,
    selected: &mt::Edge,
    replacement: &mt::Edge,
    consumed: &HashMap<mt::EdgeID, mt::Vertex>,
    faces: &[mt::Face],
    tolerance: f64,
    work: &mut usize,
) -> Option<mt::Edge> {
    if !tolerance.is_finite() || tolerance <= 0. || faces.len() > 4096 {
        return None;
    }
    let boundaries = wall.absolute_boundaries();
    let [boundary] = boundaries.as_slice() else { return None };
    let edges: Vec<_> = boundary.edge_iter().take(5).collect();
    if edges.len() != 4
        || edges.iter().map(|e| e.id()).collect::<HashSet<_>>().len() != 4
        || edges.iter().filter(|e| e.id() == selected.id()).count() != 1
    {
        return None;
    }
    let opposite: Vec<_> = edges.iter().copied().filter(|e| e.id() != selected.id() && !consumed.contains_key(&e.id())).collect();
    let [opposite] = opposite.as_slice() else { return None };
    let far = if opposite.front() == replacement.front() && opposite.back() == replacement.back() {
        (*opposite).clone()
    } else if opposite.front() == replacement.back() && opposite.back() == replacement.front() {
        opposite.inverse()
    } else {
        return None;
    };
    for seam in edges.iter().filter(|e| e.id() != selected.id() && e.id() != far.id()) {
        let at = consumed.get(&seam.id())?;
        if ![(selected.front(), far.front()), (selected.back(), far.back())]
            .iter()
            .any(|(old, end)| *end == at && ((seam.front() == *old && seam.back() == *end) || (seam.back() == *old && seam.front() == *end)))
        {
            return None;
        }
    }
    let mut cap = None;
    for face in faces.iter().filter(|f| f.id() != wall.id()) {
        let mut incident = false;
        for edge in face.edge_iter() {
            *work = work.checked_sub(1)?;
            incident |= edge.id() == far.id();
        }
        if incident {
            if cap.is_some() {
                return None;
            }
            cap = Some(face);
        }
    }
    let normal = crate::blend::plane_normal(cap?)?;
    let origin = from_p3(far.front().point());
    let chord = from_p3(far.back().point()) - origin;
    let x = (chord - normal * chord.dot(normal)).normalized()?;
    let plane = Plane { origin, x, y: normal.cross(x) };
    if !crate::prism::wall_along(&wall.surface(), normal, tolerance * 0.25, work)
        || !crate::prism_curve::at_height(&replacement.oriented_curve(), &plane, 0., tolerance * 0.25, work)
        || plane.height(from_p3(selected.front().point())).abs() <= tolerance
    {
        return None;
    }
    let old = crate::prism_curve::segment(&far, &plane, tolerance * 0.25, work)?;
    let new = crate::prism_curve::segment(replacement, &plane, tolerance * 0.25, work)?;
    let before = crate::prism_curve::segment(selected, &plane, tolerance * 0.25, work)?;
    (same_arc(&old, &new, tolerance * 0.5) && same_arc(&old, &before, tolerance * 0.5)).then_some(far)
}
