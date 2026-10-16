//! Certified line and circular side spans in extrusion profiles.
use super::*;
use solidcraft_doc::{
    SolvedSketch,
    sketch::{CurveKind, ProfilePiece, find_profiles},
};
use solidcraft_geom::{Seg2, Vec2};

pub(super) struct Source<'a> {
    pub profile: &'a SketchProfile,
    pub lines: &'a BTreeMap<u64, SketchLine>,
    pub circles: &'a BTreeMap<u64, SketchCircle>,
    pub solved: &'a SolvedSketch,
}

pub(super) fn circle(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, label: u64, source: Source<'_>) -> Result<u64> {
    let tag = super::side_roles::circle_tag(records, classes, feature, label)?;
    let [loops] = source.profile.regions.as_slice() else {
        return Err(invalid("circular blend requires one profile region"));
    };
    let [boundary] = loops.as_slice() else {
        return Err(invalid("circular blend requires one boundary"));
    };
    let [id] = boundary.curves.as_slice() else {
        return Err(invalid("circular blend requires one source circle"));
    };
    if !boundary.outer || boundary.pieces.as_slice() != [(1, 1)] {
        return Err(invalid("circular blend requires a complete circle"));
    }
    let circle = source
        .circles
        .get(id)
        .filter(|c| c.sketch == source.profile.geometry && !c.construction && c.profile_id == Some((tag, 0)))
        .ok_or_else(|| invalid("circular blend side disagrees with its source circle"))?;
    let index = source.solved.sketch.curve_index(&format!("c{}", circle.id)).ok_or_else(|| invalid("missing solved blend circle"))?;
    if !source.solved.sketch.curves.get(index).is_some_and(|c| matches!(c.kind, CurveKind::Circle { .. })) {
        return Err(invalid("circular blend carrier is not a solved circle"));
    }
    Ok(circle.id)
}

pub(super) fn curve(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, label: u64, source: Source<'_>) -> Result<u64> {
    if let Ok(id) = super::side_roles::curve(records, classes, feature, label, source.profile, source.lines) {
        return Ok(id);
    }
    let needs_arcs =
        source.profile.regions.iter().flatten().flat_map(|b| &b.curves).any(|id| !source.lines.contains_key(id) && !source.circles.contains_key(id));
    let arcs = if needs_arcs { records.design.sketch_arcs()?.into_iter().map(|arc| (arc.circle.id, arc)).collect() } else { BTreeMap::new() };
    resolve_with_arcs(&super::side_roles::curve_span(records, classes, feature, label)?, source, &arcs)
}

#[cfg(test)]
fn resolve(span: &super::side_roles::CurveSpan, source: Source<'_>) -> Result<u64> {
    resolve_with_arcs(span, source, &BTreeMap::new())
}

fn resolve_with_arcs(span: &super::side_roles::CurveSpan, source: Source<'_>, arcs: &BTreeMap<u64, SketchArc>) -> Result<u64> {
    let Source { profile, lines, circles, solved } = source;
    if profile.regions.len() > 1 {
        if profile.regions.len() > 64 || profile.regions.iter().flatten().map(|b| b.curves.len()).sum::<usize>() > 4096 {
            return Err(invalid("blend profile regions exceed the work budget"));
        }
        // Validate every selected region and its atomic members before matching
        // a local interval. The same numeric partition can occur in different
        // region namespaces, so exactly one complete certificate must match.
        super::profile_groups::resolve(profile, circles, solved)?;
        let mut found = None;
        for (region, members) in profile.regions.iter().zip(&profile.region_members) {
            let scoped = SketchProfile {
                id: profile.id,
                feature: profile.feature,
                geometry: profile.geometry,
                regions: vec![region.clone()],
                region_members: vec![members.clone()],
            };
            if let Ok(id) = resolve_with_arcs(span, Source { profile: &scoped, lines, circles, solved }, arcs)
                && found.replace(id).is_some()
            {
                return Err(invalid("blend interval is ambiguous across selected regions"));
            }
        }
        return found.ok_or_else(|| invalid("blend interval has no certified selected region"));
    }
    let [loops] = profile.regions.as_slice() else { return Err(invalid("blend side requires one profile region")) };
    let [boundary] = loops.as_slice() else { return Err(invalid("blend side requires one profile boundary")) };
    if !boundary.outer || boundary.curves.len() > 512 || boundary.curves.len() != boundary.pieces.len() {
        return Err(invalid("invalid blend side boundary"));
    }
    let mut tags = BTreeMap::<u64, (u64, BTreeSet<(u32, u32)>)>::new();
    let mut sources = BTreeSet::new();
    for (id, piece) in boundary.curves.iter().zip(&boundary.pieces) {
        let tag = if let Some(line) = lines.get(id).filter(|l| l.sketch == profile.geometry && !l.construction) {
            line.profile_id
        } else if let Some(circle) = circles.get(id).filter(|c| c.sketch == profile.geometry && !c.construction) {
            circle.profile_id
        } else if let Some(arc) = arcs.get(id).filter(|a| a.circle.sketch == profile.geometry && !a.circle.construction) {
            arc.circle.profile_id
        } else {
            return Err(invalid("unsupported blend boundary carrier"));
        };
        let Some((tag, 0)) = tag else { return Err(invalid("unsupported blend boundary identity")) };
        let (owned, pieces) = tags.entry(tag).or_insert_with(|| (*id, BTreeSet::new()));
        if tag == 0 || *owned != *id || pieces.iter().any(|(_, count)| *count != piece.1) || !pieces.insert(*piece) {
            return Err(invalid("ambiguous blend boundary carrier"));
        }
        sources.insert(format!("c{id}"));
    }
    let (id, _) = tags
        .get(&span.tag)
        .filter(|(_, pieces)| pieces.contains(&span.piece))
        .ok_or_else(|| invalid("blend side span is outside selected boundary"))?;
    let (id, piece) = (*id, span.piece);
    // Rebuild the source boundary's partition namespace. Certify the same region
    // as the full sketch before interpreting a locally numbered line interval.
    let selection = super::import::selection_for_regions(&profile.regions, circles, true)?;
    let expected = super::profile_groups::declared_region(&selection, solved)?;
    let mut scoped = solved.clone();
    for curve in &mut scoped.sketch.curves {
        curve.construction |= !sources.contains(&curve.id);
    }
    scoped.profiles = find_profiles(&scoped.sketch);
    let region = super::profile_groups::declared_region(&selection, &scoped)?;
    if !super::profile_selection::same_region(&expected, &region) {
        return Err(invalid("blend side partition changes selected region"));
    }
    let key = ProfilePiece { curve: format!("c{id}"), piece: piece.0, count: piece.1, reversed: false };
    let Some(segment) = scoped.profiles.iter().find_map(|p| p.piece_geometry(&key)) else {
        return Err(invalid("missing certified blend curve interval"));
    };
    if !matches!(segment, Seg2::Line { .. } if lines.contains_key(&id))
        && !matches!(segment, Seg2::Arc { .. } if circles.contains_key(&id) || arcs.contains_key(&id))
    {
        return Err(invalid("blend curve interval has a different carrier kind"));
    }
    let (a, b) = (segment.start(), segment.end());
    if a.dist(b) <= 1e-7 {
        return Err(invalid("degenerate blend curve interval"));
    }
    let carriers: BTreeMap<_, _> =
        scoped.sketch.curves.iter().filter(|c| !c.construction && sources.contains(&c.id)).map(|c| (c.id.as_str(), &c.kind)).collect();
    let mut actual = Vec::new();
    for point in [a, b] {
        let mut neighbors = BTreeSet::new();
        for (tag, (other, _)) in &tags {
            if *other == id {
                continue;
            }
            let name = format!("c{other}");
            let kind = carriers.get(name.as_str()).ok_or_else(|| invalid("missing solved blend neighbor"))?;
            if on_carrier(&scoped.sketch, kind, point)? {
                neighbors.insert(*tag);
            }
        }
        actual.push(neighbors);
    }
    let mut declared: Vec<BTreeSet<_>> = span.adjacent.iter().map(|a| a.iter().copied().collect()).collect();
    actual.sort();
    declared.sort();
    if actual != declared || actual.iter().any(BTreeSet::is_empty) {
        return Err(invalid("blend side endpoints disagree with declared neighbors"));
    }
    Ok(id)
}

fn on_carrier(sketch: &solidcraft_doc::sketch::Sketch, kind: &CurveKind, point: Vec2) -> Result<bool> {
    let get = |id| sketch.point(id).ok_or_else(|| invalid("missing blend neighbor point"));
    Ok(match kind {
        CurveKind::Line { a, b } => {
            let (a, b) = (get(*a)?, get(*b)?);
            let axis = b - a;
            let length = axis.len();
            let along = (point - a).dot(axis);
            length > 1e-7 && (point - a).cross(axis).abs() <= 1e-7 * length && along >= -1e-7 * length && along <= length * length + 1e-7 * length
        }
        CurveKind::Circle { c, r } => (point.dist(get(*c)?) - *r).abs() <= 1e-7,
        CurveKind::Arc { c, a, b } => {
            let (center, start, end) = (get(*c)?, get(*a)?, get(*b)?);
            let (a, b, p) = (start - center, end - center, point - center);
            let tau = std::f64::consts::TAU;
            let angle = |v: Vec2| a.cross(v).atan2(a.dot(v)).rem_euclid(tau);
            let sweep = angle(b);
            a.len() > 1e-7 && (p.len() - a.len()).abs() <= 1e-7 && ((point.dist(start).min(point.dist(end)) <= 1e-7) || angle(p) <= sweep)
        }
        _ => return Err(invalid("unsupported solved blend neighbor")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_doc::sketch::{Sketch, solve};
    use solidcraft_geom::Plane;

    fn authored(alias: bool) -> (SketchProfile, BTreeMap<u64, SketchLine>, BTreeMap<u64, SketchCircle>, SolvedSketch) {
        authored_rectangle(alias, [Vec2::new(-5., -35.), Vec2::new(5., -35.), Vec2::new(5., -25.), Vec2::new(-5., -25.)])
    }

    fn authored_rectangle(alias: bool, points: [Vec2; 4]) -> (SketchProfile, BTreeMap<u64, SketchLine>, BTreeMap<u64, SketchCircle>, SolvedSketch) {
        let mut sketch = Sketch::new();
        let mut lines = BTreeMap::new();
        for index in 0..4 {
            let id = 31 + index as u64;
            let (a, b) = (points[index], points[(index + 1) % 4]);
            sketch.add_line(a, b, None, None, Some(&format!("c{id}"))).unwrap();
            lines.insert(
                id,
                SketchLine {
                    id,
                    sketch: 70,
                    start_point: 0,
                    end_point: 0,
                    start: [a.x / 10., a.y / 10., 0.],
                    end: [b.x / 10., b.y / 10., 0.],
                    fixed: true,
                    construction: false,
                    centerline: false,
                    reference: false,
                    metadata: Default::default(),
                    profile_id: Some((1000 + id, 0)),
                },
            );
        }
        let mut circles = BTreeMap::new();
        for id in 40..=if alias { 41 } else { 40 } {
            sketch.add_circle(Vec2::ZERO, 30., None, Some(&format!("c{id}"))).unwrap();
            circles.insert(
                id,
                SketchCircle {
                    id,
                    sketch: 70,
                    center_point: 0,
                    center: [0.; 3],
                    normal: [0., 0., 1.],
                    x_axis: [1., 0., 0.],
                    radius: 3.,
                    construction: false,
                    fixed: true,
                    reference: false,
                    metadata: Default::default(),
                    profile_id: Some((1000 + id, 0)),
                },
            );
        }
        let mut boundary =
            ProfileLoop { curves: vec![32, 33, 34, 40], reversed: vec![false; 4], pieces: vec![(2, 2), (1, 1), (1, 2), (2, 2)], outer: true };
        if alias {
            boundary.curves.push(41);
            boundary.reversed.push(false);
            boundary.pieces.push((2, 2));
        }
        let profile = SketchProfile { id: 80, feature: 9, geometry: 70, regions: vec![vec![boundary]], region_members: vec![vec![]] };
        let report = solve(&mut sketch);
        let profiles = find_profiles(&sketch);
        let solved =
            SolvedSketch { feature: 1, name: "Original mixed boundary".into(), plane: Plane::named("XY").unwrap(), sketch, report, profiles };
        (profile, lines, circles, solved)
    }

    #[test]
    fn two_arcs_of_one_circle_keep_distinct_partition_and_neighbor_certificates() {
        let (mut profile, lines, circles, solved) =
            authored_rectangle(false, [Vec2::new(-5., -35.), Vec2::new(5., -35.), Vec2::new(5., 35.), Vec2::new(-5., 35.)]);
        let region = solved.profiles.iter().find(|p| p.region.contains(Vec2::ZERO)).unwrap();
        let (pieces, holes) = region.piece_boundaries().unwrap();
        assert!(holes.is_empty());
        assert_eq!(pieces.len(), 4);
        profile.regions[0][0] = ProfileLoop {
            curves: pieces.iter().map(|p| p.curve.trim_start_matches('c').parse().unwrap()).collect(),
            reversed: pieces.iter().map(|p| p.reversed).collect(),
            pieces: pieces.iter().map(|p| (p.piece, p.count)).collect(),
            outer: true,
        };
        let source = |profile| Source { profile, lines: &lines, circles: &circles, solved: &solved };
        let arcs: Vec<_> = pieces.iter().filter(|p| p.curve == "c40").collect();
        assert_eq!(arcs.len(), 2);
        assert_ne!(arcs[0].piece, arcs[1].piece);
        for piece in &pieces {
            let id: u64 = piece.curve.trim_start_matches('c').parse().unwrap();
            let neighbors = if id == 40 { [vec![1032], vec![1034]] } else { [vec![1040], vec![1040]] };
            let mut span = super::super::side_roles::CurveSpan { tag: 1000 + id, piece: (piece.piece, piece.count), adjacent: neighbors };
            assert_eq!(resolve(&span, source(&profile)).unwrap(), id);
            span.adjacent[0] = vec![1031];
            assert!(resolve(&span, source(&profile)).is_err(), "outside line cannot certify an endpoint");
        }
        let arc = arcs[0];
        let span = super::super::side_roles::CurveSpan { tag: 1040, piece: (arc.piece, arc.count), adjacent: [vec![1032], vec![1034]] };
        for change in 0..3 {
            let mut changed = SketchProfile {
                id: profile.id,
                feature: profile.feature,
                geometry: profile.geometry,
                regions: profile.regions.clone(),
                region_members: profile.region_members.clone(),
            };
            let b = &mut changed.regions[0][0];
            let indices: Vec<_> = b.curves.iter().enumerate().filter_map(|(i, id)| (*id == 40).then_some(i)).collect();
            match change {
                0 => b.pieces[indices[1]] = b.pieces[indices[0]],
                1 => b.pieces[indices[1]].1 += 1,
                _ => b.curves[indices[1]] = 999,
            }
            assert!(
                resolve(&span, Source { profile: &changed, lines: &lines, circles: &circles, solved: &solved }).is_err(),
                "duplicate, inconsistent, or foreign arc {change}"
            );
        }
    }

    #[test]
    fn disconnected_crescents_keep_projected_arc_and_circle_partitions() {
        let mut sketch = Sketch::new();
        let h = 91_f64.sqrt();
        let mut arcs = BTreeMap::new();
        let circle = |id, radius| SketchCircle {
            id,
            sketch: 70,
            center_point: 0,
            center: [0.; 3],
            normal: [0., 0., 1.],
            x_axis: [1., 0., 0.],
            radius: radius / 10.,
            construction: false,
            fixed: true,
            reference: false,
            metadata: Default::default(),
            profile_id: Some((1000 + id, 0)),
        };
        for (id, a, b) in [(100, Vec2::new(3., h), Vec2::new(-3., h)), (101, Vec2::new(-3., -h), Vec2::new(3., -h))] {
            sketch.add_arc(Vec2::ZERO, a, b, [None; 3], Some(&format!("c{id}"))).unwrap();
            arcs.insert(
                id,
                SketchArc { circle: circle(id, 10.), start_point: 0, end_point: 0, start_angle: a.y.atan2(a.x), end_angle: b.y.atan2(b.x) },
            );
        }
        let mut lines = BTreeMap::new();
        for (id, a, b) in [(102, Vec2::new(-3., h), Vec2::new(-3., -h)), (103, Vec2::new(3., -h), Vec2::new(3., h))] {
            sketch.add_line(a, b, None, None, Some(&format!("c{id}"))).unwrap();
            lines.insert(
                id,
                SketchLine {
                    id,
                    sketch: 70,
                    start_point: 0,
                    end_point: 0,
                    start: [a.x / 10., a.y / 10., 0.],
                    end: [b.x / 10., b.y / 10., 0.],
                    fixed: true,
                    construction: false,
                    centerline: false,
                    reference: false,
                    metadata: Default::default(),
                    profile_id: Some((1000 + id, 0)),
                },
            );
        }
        sketch.add_circle(Vec2::ZERO, 8., None, Some("c104")).unwrap();
        let circles = BTreeMap::from([(104, circle(104, 8.))]);
        let report = solve(&mut sketch);
        let profiles = find_profiles(&sketch);
        let mut regions = Vec::new();
        let mut selected = Vec::new();
        for y in [-9., 9.] {
            let region = profiles.iter().find(|p| p.region.contains(Vec2::new(0., y))).unwrap();
            let (pieces, holes) = region.piece_boundaries().unwrap();
            assert!(holes.is_empty());
            selected.push(pieces.iter().find(|p| p.curve == "c104").unwrap().clone());
            regions.push(vec![ProfileLoop {
                curves: pieces.iter().map(|p| p.curve.trim_start_matches('c').parse().unwrap()).collect(),
                reversed: pieces.iter().map(|p| p.reversed).collect(),
                pieces: pieces.iter().map(|p| (p.piece, p.count)).collect(),
                outer: true,
            }]);
        }
        let profile = SketchProfile { id: 80, feature: 9, geometry: 70, regions, region_members: vec![vec![], vec![]] };
        let solved =
            SolvedSketch { feature: 1, name: "Original projected crescents".into(), plane: Plane::named("XY").unwrap(), sketch, report, profiles };
        for piece in selected {
            let span = super::super::side_roles::CurveSpan { tag: 1104, piece: (piece.piece, piece.count), adjacent: [vec![1102], vec![1103]] };
            let source = || Source { profile: &profile, lines: &lines, circles: &circles, solved: &solved };
            assert_eq!(resolve_with_arcs(&span, source(), &arcs).unwrap(), 104);
            assert!(resolve_with_arcs(&span, source(), &BTreeMap::new()).is_err(), "projected arc ownership is required");
        }
        let arc = &solved.sketch.curves[solved.sketch.curve_index("c100").unwrap()].kind;
        assert!(on_carrier(&solved.sketch, arc, Vec2::new(0., 10.)).unwrap());
        assert!(!on_carrier(&solved.sketch, arc, Vec2::new(0., -10.)).unwrap(), "same circle outside the finite arc");
    }

    #[test]
    fn multiple_selected_regions_require_one_complete_interval_certificate() {
        let mut sketch = Sketch::new();
        let mut lines = BTreeMap::new();
        let mut regions = Vec::new();
        for (region, x) in [(0_u64, -20.), (1, 20.)] {
            let points = [Vec2::new(x, 0.), Vec2::new(x + 10., 0.), Vec2::new(x + 10., 8.), Vec2::new(x, 8.)];
            let mut ids = Vec::new();
            for i in 0..4 {
                let id = 31 + region * 10 + i as u64;
                let (a, b) = (points[i], points[(i + 1) % 4]);
                sketch.add_line(a, b, None, None, Some(&format!("c{id}"))).unwrap();
                ids.push(id);
                lines.insert(
                    id,
                    SketchLine {
                        id,
                        sketch: 70,
                        start_point: 0,
                        end_point: 0,
                        start: [a.x / 10., a.y / 10., 0.],
                        end: [b.x / 10., b.y / 10., 0.],
                        fixed: true,
                        construction: false,
                        centerline: false,
                        reference: false,
                        metadata: Default::default(),
                        profile_id: Some((1000 + id, 0)),
                    },
                );
            }
            regions.push(vec![ProfileLoop { curves: ids, reversed: vec![false; 4], pieces: vec![(1, 1); 4], outer: true }]);
        }
        let mut profile = SketchProfile { id: 80, feature: 9, geometry: 70, regions, region_members: vec![vec![], vec![]] };
        let report = solve(&mut sketch);
        let profiles = find_profiles(&sketch);
        let solved =
            SolvedSketch { feature: 1, name: "Disconnected rectangles".into(), plane: Plane::named("XY").unwrap(), sketch, report, profiles };
        let circles = BTreeMap::new();
        for id in [31, 41] {
            let mut span = super::super::side_roles::CurveSpan { tag: 1000 + id, piece: (1, 1), adjacent: [vec![1001 + id], vec![1003 + id]] };
            assert_eq!(resolve(&span, Source { profile: &profile, lines: &lines, circles: &circles, solved: &solved }).unwrap(), id);
            span.adjacent[1] = vec![if id == 31 { 1044 } else { 1034 }];
            assert!(
                resolve(&span, Source { profile: &profile, lines: &lines, circles: &circles, solved: &solved }).is_err(),
                "another region cannot supply an endpoint neighbor"
            );
        }
        let span = super::super::side_roles::CurveSpan { tag: 1031, piece: (1, 1), adjacent: [vec![1032], vec![1034]] };
        let second = profile.regions[1].clone();
        profile.regions[1] = profile.regions[0].clone();
        assert!(
            resolve(&span, Source { profile: &profile, lines: &lines, circles: &circles, solved: &solved }).is_err(),
            "duplicate selected region"
        );
        profile.regions[1] = second;
        profile.regions[1][0].curves[0] = 999;
        assert!(
            resolve(&span, Source { profile: &profile, lines: &lines, circles: &circles, solved: &solved }).is_err(),
            "a malformed other region cannot be ignored"
        );
    }

    #[test]
    fn mixed_line_spans_require_partition_ownership_and_all_endpoint_neighbors() {
        for alias in [false, true] {
            let (profile, lines, circles, solved) = authored(alias);
            let source = |profile| Source { profile, lines: &lines, circles: &circles, solved: &solved };
            let ends = if alias { vec![1040, 1041] } else { vec![1040] };
            let mut span = super::super::side_roles::CurveSpan { tag: 1032, piece: (2, 2), adjacent: [ends.clone(), vec![1033]] };
            assert_eq!(resolve(&span, source(&profile)).unwrap(), 32);
            span.piece = (1, 2);
            assert!(resolve(&span, source(&profile)).is_err(), "other half of the same named line");
            span.piece = (2, 2);
            span.adjacent[1] = vec![1034];
            assert!(resolve(&span, source(&profile)).is_err(), "nonincident named neighbor");
            span.adjacent = [ends, vec![1033]];
            if alias {
                span.adjacent[0].pop();
                assert!(resolve(&span, source(&profile)).is_err(), "coincident neighbor cannot be silently dropped");
                span.adjacent[0].push(1041);
            }
            span.tag = 1031;
            assert!(resolve(&span, source(&profile)).is_err(), "line outside selected boundary");
            span.tag = 1032;
            let mut changed = SketchProfile {
                id: profile.id,
                feature: profile.feature,
                geometry: profile.geometry,
                regions: profile.regions.clone(),
                region_members: profile.region_members.clone(),
            };
            changed.regions[0][0].pieces[0] = (1, 2);
            assert!(resolve(&span, source(&changed)).is_err(), "selected region changed");
        }
    }
}
