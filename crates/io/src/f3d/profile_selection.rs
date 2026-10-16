//! Translate native boundary-scoped partitions into persistent full-sketch pieces.
//! Counts and directions must match a regenerated graph exactly. Geometry alone
//! never selects a region: it certifies the translation of a named source boundary.
use std::collections::{BTreeMap, BTreeSet};

use solidcraft_doc::sketch::{Profile, ProfilePiece, find_profiles, merge_regions};
use solidcraft_doc::{PieceProfileBoundary, ProfileSel, SolvedSketch, profile_selection_indices};
use solidcraft_geom::{Loop2, Region2, Seg2};

use super::{Result, invalid};

const TOL: f64 = 1e-7; // mm; tighter than sketch graph vertex merging.

pub(super) fn resolve(selection: ProfileSel, solved: &SolvedSketch) -> Result<ProfileSel> {
    let ProfileSel::PieceBoundaries { profiles, circle_seams } = &selection else { return Ok(selection) };
    let (normalized, aliases) = solidcraft_doc::normalize_coincident_profile_boundaries(&solved.sketch, profiles, circle_seams)?;
    if aliases.is_empty() {
        return resolve_distinct(selection, solved);
    }
    let mut seams = circle_seams.clone();
    seams.retain(|id, _| !aliases.contains_key(id));
    let mut translated = resolve_distinct(ProfileSel::PieceBoundaries { profiles: normalized, circle_seams: seams }, solved)?;
    let ProfileSel::PieceBoundaries { profiles, circle_seams } = &mut translated else { return Err(invalid("missing normalized profile pieces")) };
    // Preserve both source dependencies in the full-graph partition, so a later
    // edit separating the coincident circles cannot silently change selection.
    let mut by_source: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (alias, canonical) in &aliases {
        by_source.entry(canonical).or_default().push(alias);
    }
    let mut total: usize = profiles.iter().flat_map(|p| std::iter::once(&p.outer).chain(&p.holes)).map(Vec::len).sum();
    for profile in profiles.iter_mut() {
        for boundary in std::iter::once(&mut profile.outer).chain(&mut profile.holes) {
            let mut references = Vec::new();
            for piece in boundary.iter() {
                for alias in by_source.get(piece.curve.as_str()).into_iter().flatten() {
                    total += 1;
                    if total > 20_000 || boundary.len() + references.len() >= 512 {
                        return Err(invalid("excessive coincident profile references"));
                    }
                    references.push(ProfilePiece { curve: (*alias).into(), ..piece.clone() });
                    if let Some(seam) = circle_seams.get(&piece.curve).copied() {
                        circle_seams.insert((*alias).into(), seam);
                    }
                }
            }
            boundary.extend(references);
            boundary.sort();
        }
        profile.holes.sort();
    }
    solidcraft_doc::piece_profile_selection_from_value(&serde_json::to_value(&translated).map_err(|e| invalid(&e.to_string()))?)?;
    profile_selection_indices(solved, &translated)?;
    Ok(translated)
}

fn resolve_distinct(selection: ProfileSel, solved: &SolvedSketch) -> Result<ProfileSel> {
    let ProfileSel::PieceBoundaries { profiles, circle_seams } = &selection else { return Ok(selection) };
    // Validate before deriving a second graph, including input budgets and seams.
    solidcraft_doc::piece_profile_selection_from_value(&serde_json::to_value(&selection).map_err(|e| invalid(&e.to_string()))?)?;
    if profile_selection_indices(solved, &selection).is_ok() {
        return Ok(selection);
    }
    let sources: BTreeSet<_> = profiles.iter().flat_map(|p| std::iter::once(&p.outer).chain(&p.holes)).flatten().map(|p| &p.curve).collect();
    let mut scoped = solved.clone();
    for curve in &mut scoped.sketch.curves {
        curve.construction |= !sources.contains(&curve.id);
    }
    scoped.profiles = find_profiles(&scoped.sketch);
    let selected = profile_selection_indices(&scoped, &selection)?;
    let mut translated = Vec::with_capacity(selected.len());
    let mut used = BTreeSet::new();
    let mut comparisons = 0usize;
    for index in selected {
        let native = scoped.profiles.get(index).ok_or_else(|| invalid("missing scoped profile"))?;
        for index in refined_region(native, solved, &mut comparisons)? {
            if !used.insert(index) {
                return Err(invalid("ambiguous selected boundary translation"));
            }
            let candidate = solved.profiles.get(index).ok_or_else(|| invalid("missing refined profile"))?;
            let (outer, holes) = candidate.piece_boundaries_with_seams(circle_seams).ok_or_else(|| invalid("uncertified selected profile pieces"))?;
            translated.push(PieceProfileBoundary { outer, holes });
        }
    }
    let translated = ProfileSel::PieceBoundaries { profiles: translated, circle_seams: circle_seams.clone() };
    solidcraft_doc::piece_profile_selection_from_value(&serde_json::to_value(&translated).map_err(|e| invalid(&e.to_string()))?)?;
    profile_selection_indices(solved, &translated)?;
    Ok(translated)
}

fn refined_region(native: &Profile, solved: &SolvedSketch, comparisons: &mut usize) -> Result<Vec<usize>> {
    // Preserve the inexpensive complete-region translation, including sketches
    // with unrelated free-form faces that have no analytic piece certificate.
    let mut matches = solved.profiles.iter().enumerate().filter(|(_, candidate)| same_boundary(native, candidate));
    if let Some((index, _)) = matches.next() {
        return if matches.next().is_none() { Ok(vec![index]) } else { Err(invalid("ambiguous selected boundary translation")) };
    }
    let mut graph: BTreeMap<String, BTreeMap<ProfilePiece, Seg2>> = BTreeMap::new();
    let mut incidences = 0usize;
    for profile in &solved.profiles {
        let (outer, holes) = profile.piece_boundaries().ok_or_else(|| invalid("uncertified refined profile graph"))?;
        for mut piece in std::iter::once(outer).chain(holes).flatten() {
            incidences += 1;
            if incidences > 100_000 {
                return Err(invalid("excessive refined profile graph"));
            }
            piece.reversed = false;
            let segment = profile.piece_geometry(&piece).ok_or_else(|| invalid("missing analytic graph carrier"))?;
            graph.entry(piece.curve.clone()).or_default().insert(piece, segment);
        }
    }
    let (outer, holes) = native.piece_boundaries().ok_or_else(|| invalid("uncertified source boundary"))?;
    let mut boundary = Vec::new();
    for piece in std::iter::once(outer).chain(holes).flatten() {
        let mut source = native.piece_geometry(&piece).ok_or_else(|| invalid("missing source boundary carrier"))?;
        let candidates = graph.get(&piece.curve).ok_or_else(|| invalid("missing refined boundary source"))?;
        // A full circle has no geometric seam. Start at a real refined cut so
        // none of its intervals wraps across the coverage check's origin.
        if let Seg2::Arc { start, sweep, .. } = &mut source
            && sweep.abs() == std::f64::consts::TAU
            && let Some(segment) = candidates.values().next()
            && let Seg2::Arc { start: cut, .. } = if piece.reversed { segment.reversed() } else { *segment }
        {
            *start = cut;
        }
        let mut spans = Vec::new();
        for (key, segment) in candidates {
            *comparisons += 1;
            if *comparisons > 2_000_000 {
                return Err(invalid("excessive profile refinement work"));
            }
            let segment = if piece.reversed { segment.reversed() } else { *segment };
            if let Some((start, end, length)) = contained_span(&source, &segment) {
                spans.push((start, end, length, ProfilePiece { reversed: piece.reversed, ..key.clone() }));
            }
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut end = 0.;
        for (start, next, length, key) in &spans {
            if (start - end).abs() * length > TOL {
                return Err(invalid("gap or overlap in refined boundary"));
            }
            end = *next;
            boundary.push(key.clone());
        }
        if spans.last().is_none_or(|(_, _, length, _)| (end - 1.).abs() * length > TOL) {
            return Err(invalid("incomplete refined boundary"));
        }
    }
    let indices = super::profile_groups::enclosed(solved, &boundary, &BTreeMap::new())?;
    let regions: Vec<_> = indices.iter().filter_map(|index| solved.profiles.get(*index).map(|p| p.region.clone())).collect();
    let merged = merge_regions(&regions);
    if !matches!(merged.as_slice(), [region] if same_region(region, &native.region)) {
        return Err(invalid("refined regions disagree with source boundary"));
    }
    Ok(indices)
}

fn same_boundary(a: &Profile, b: &Profile) -> bool {
    fn lineage(pieces: &[ProfilePiece]) -> BTreeSet<(&str, bool)> {
        pieces.iter().map(|p| (p.curve.as_str(), p.reversed)).collect()
    }
    let (Some((ao, ah)), Some((bo, bh))) = (a.piece_boundaries(), b.piece_boundaries()) else { return false };
    let mut ah: Vec<_> = ah.iter().map(|h| lineage(h)).collect();
    let mut bh: Vec<_> = bh.iter().map(|h| lineage(h)).collect();
    ah.sort();
    bh.sort();
    lineage(&ao) == lineage(&bo) && ah == bh && same_region(&a.region, &b.region)
}

/// Directed analytic containment, followed by a complete interval-coverage check.
/// Source identity is checked separately; nearby unrelated geometry cannot match.
fn contained_span(source: &Seg2, part: &Seg2) -> Option<(f64, f64, f64)> {
    if !same_carrier(source, part) {
        return None;
    }
    let (start, end, length) = match (source, part) {
        (Seg2::Line { a, b }, Seg2::Line { a: c, b: d }) => {
            let axis = *b - *a;
            let length = axis.len();
            (((*c - *a).dot(axis) / (length * length)), ((*d - *a).dot(axis) / (length * length)), length)
        }
        (Seg2::Arc { radius, start, sweep, .. }, Seg2::Arc { start: p, sweep: s, .. }) => {
            let mut delta = ((p - start) * sweep.signum()).rem_euclid(std::f64::consts::TAU);
            if (std::f64::consts::TAU - delta) * radius <= TOL {
                delta = 0.;
            }
            (delta / sweep.abs(), (delta + s.abs()) / sweep.abs(), sweep.abs() * radius)
        }
        _ => return None,
    };
    (start.is_finite()
        && end.is_finite()
        && length.is_finite()
        && length > TOL
        && end > start
        && start * length >= -TOL
        && (end - 1.) * length <= TOL)
        .then_some((start, end, length))
}

pub(super) fn same_region(a: &Region2, b: &Region2) -> bool {
    if !same_loop(&a.outer, &b.outer) || a.holes.len() != b.holes.len() {
        return false;
    }
    let mut used = BTreeSet::new();
    for hole in &a.holes {
        let mut matches = b.holes.iter().enumerate().filter(|(_, candidate)| same_loop(hole, candidate));
        let Some((index, _)) = matches.next() else { return false };
        if matches.next().is_some() || !used.insert(index) {
            return false;
        }
    }
    true
}

fn same_loop(a: &Loop2, b: &Loop2) -> bool {
    let (Some(a), Some(b)) = (canonical_loop(a), canonical_loop(b)) else { return false };
    let n = a.len();
    // Tangent contacts can split a boundary without changing its directed path.
    // Union callers first cancel shared graph edges; holes remain explicit.
    n != 0 && n == b.len() && (0..n).any(|offset| a.iter().zip(b.iter().cycle().skip(offset)).all(|(a, b)| same_segment(a, b)))
}

fn canonical_loop(boundary: &Loop2) -> Option<Vec<Seg2>> {
    let mut runs: Vec<(Seg2, Vec<Seg2>)> = Vec::new();
    for segment in &boundary.segs {
        if !matches!(segment, Seg2::Line { .. } | Seg2::Arc { .. }) {
            return None;
        }
        if let Some((previous, parts)) = runs.last_mut()
            && let Some(joined) = join_segments(previous, segment)
        {
            *previous = joined;
            parts.push(*segment);
        } else {
            runs.push((*segment, vec![*segment]));
        }
    }
    if runs.len() > 1 {
        let joined = join_segments(&runs.last()?.0, &runs.first()?.0);
        if let Some(joined) = joined {
            let (_, mut last) = runs.pop()?;
            let first = runs.first_mut()?;
            last.append(&mut first.1);
            *first = (joined, last);
        }
    }
    let mut result = Vec::with_capacity(runs.len());
    for (mut segment, parts) in runs {
        // Recheck every original piece against the final carrier, so tolerances
        // cannot accumulate through a long chain of nearly collinear segments.
        if !parts.iter().all(|part| same_carrier(&segment, part)) {
            return None;
        }
        if let Seg2::Arc { radius, start, sweep, .. } = &mut segment
            && (sweep.abs() - std::f64::consts::TAU).abs() * *radius <= TOL
        {
            *start = 0.;
            *sweep = sweep.signum() * std::f64::consts::TAU;
        }
        result.push(segment);
    }
    Some(result)
}

fn same_carrier(a: &Seg2, b: &Seg2) -> bool {
    match (a, b) {
        (Seg2::Line { a, b }, Seg2::Line { a: c, b: d }) => {
            let axis = *b - *a;
            let length = axis.len();
            length.is_finite()
                && length > 0.
                && axis.dot(*d - *c) > 0.
                && (*c - *a).cross(axis).abs() / length <= TOL
                && (*d - *a).cross(axis).abs() / length <= TOL
        }
        (Seg2::Arc { center, radius, sweep, .. }, Seg2::Arc { center: c, radius: r, sweep: s, .. }) => {
            // Graph partitions of one source circle share the exact carrier.
            // Nearby but distinct circles must not become one circular run.
            center == c && radius == r && radius.is_finite() && *radius > 0. && sweep.is_finite() && s.is_finite() && sweep * s > 0.
        }
        _ => false,
    }
}

fn join_segments(a: &Seg2, b: &Seg2) -> Option<Seg2> {
    if !same_carrier(a, b) || a.end().dist(b.start()) > TOL {
        return None;
    }
    let joined = match (a, b) {
        (Seg2::Line { a, .. }, Seg2::Line { b, .. }) => Seg2::Line { a: *a, b: *b },
        (Seg2::Arc { center, radius, start, sweep }, Seg2::Arc { sweep: s, .. }) => {
            if ((sweep + s).abs() - std::f64::consts::TAU) * radius > TOL {
                return None;
            }
            Seg2::Arc { center: *center, radius: *radius, start: *start, sweep: sweep + s }
        }
        _ => return None,
    };
    (joined.end().dist(b.end()) <= TOL).then_some(joined)
}

fn same_segment(a: &Seg2, b: &Seg2) -> bool {
    match (a, b) {
        (Seg2::Line { a, b }, Seg2::Line { a: c, b: d }) => a.dist(*c) <= TOL && b.dist(*d) <= TOL,
        (Seg2::Arc { center, radius, sweep, .. }, Seg2::Arc { center: c, radius: r, sweep: s, .. }) => {
            center.dist(*c) + (radius - r).abs() <= TOL
                && sweep.signum() == s.signum()
                && (sweep - s).abs() * radius.abs().max(r.abs()) <= TOL
                && a.start().dist(b.start()) <= TOL
                && a.end().dist(b.end()) <= TOL
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_geom::Vec2;
    use std::f64::consts::{PI, TAU};

    #[test]
    fn subdivision_comparison_preserves_major_arcs_cyclic_lines_and_holes() {
        let arc = Seg2::Arc { center: Vec2::new(3., 2.), radius: 9., start: 0.7, sweep: 4.1 };
        let chord = Seg2::Line { a: arc.end(), b: arc.start() };
        let whole = Loop2 { segs: vec![arc, chord] };
        let split = Loop2 {
            segs: vec![
                Seg2::Arc { center: Vec2::new(3., 2.), radius: 9., start: 2.7, sweep: 2.1 },
                chord,
                Seg2::Arc { center: Vec2::new(3., 2.), radius: 9., start: 0.7, sweep: 2. },
            ],
        };
        assert!(same_loop(&whole, &split));
        assert!(!same_loop(&whole, &split.reversed()));
        let outer = Loop2::polygon(&[Vec2::new(-20., -20.), Vec2::new(20., -20.), Vec2::new(20., 20.), Vec2::new(-20., 20.)]);
        let divided = Loop2::polygon(&[Vec2::new(0., -20.), Vec2::new(20., -20.), Vec2::new(20., 20.), Vec2::new(-20., 20.), Vec2::new(-20., -20.)]);
        let a = Region2 { outer, holes: vec![Loop2::circle(Vec2::ZERO, 3.).reversed(), Loop2::circle(Vec2::new(10., 0.), 2.).reversed()] };
        let b = Region2 {
            outer: divided,
            holes: vec![Loop2::circle_from(Vec2::new(10., 0.), 2., PI / 3.).reversed(), Loop2::circle_from(Vec2::ZERO, 3., PI / 7.).reversed()],
        };
        assert!(same_region(&a, &b));
        let mut wrong = b.clone();
        wrong.holes[0] = wrong.holes[0].reversed();
        assert!(!same_region(&a, &wrong));
        wrong.holes[0] = Loop2::circle(Vec2::new(10.01, 0.), 2.).reversed();
        assert!(!same_region(&a, &wrong));
        let complementary = Loop2 { segs: vec![Seg2::Arc { center: Vec2::new(3., 2.), radius: 9., start: 0.7, sweep: 4.1 - TAU }, chord] };
        assert!(!same_loop(&whole, &complementary));
    }
}
