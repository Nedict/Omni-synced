//! Selected atomic regions and their independently stored combined boundaries.
//! Resolve every member by source identity, then certify its union against the
//! combined boundary. No centroid, area ranking, or nearest-curve selection.
use std::collections::{BTreeMap, BTreeSet};

use solidcraft_doc::sketch::{ProfilePiece, find_profiles, merge_regions};
use solidcraft_doc::{PieceProfileBoundary, ProfileSel, SolvedSketch, profile_selection_indices};
use solidcraft_geom::Region2;

use super::{Result, SketchCircle, SketchProfile, invalid};

pub(super) fn resolve(native: &SketchProfile, circles: &BTreeMap<u64, SketchCircle>, solved: &SolvedSketch) -> Result<ProfileSel> {
    if native.regions.len() != native.region_members.len() {
        return Err(invalid("inconsistent profile member groups"));
    }
    let mut used = BTreeSet::new();
    let mut profiles = Vec::new();
    let mut circle_seams = BTreeMap::new();
    for (region_index, (parent, members)) in native.regions.iter().zip(&native.region_members).enumerate() {
        let members = if members.is_empty() { std::slice::from_ref(parent) } else { members.as_slice() };
        let mut indices = Vec::new();
        for (member_index, member) in members.iter().enumerate() {
            let selection = super::import::selection_for_regions(std::slice::from_ref(member), circles, true)?;
            let selection = super::profile_selection::resolve(selection, solved)
                .map_err(|error| invalid(&format!("profile {} region {} member {}: {error}", native.id, region_index + 1, member_index + 1)))?;
            let selected = profile_selection_indices(solved, &selection)?;
            for index in selected {
                if !used.insert(index) {
                    return Err(invalid("duplicate selected profile member"));
                }
                indices.push(index);
            }
            let ProfileSel::PieceBoundaries { profiles: member_profiles, circle_seams: seams } = selection else {
                return Err(invalid("uncertified profile member identity"));
            };
            profiles.extend(member_profiles);
            circle_seams.extend(seams);
        }
        let combined = merged(solved, &indices)?;
        let [combined] = combined.as_slice() else { return Err(invalid("profile members do not form one connected region")) };
        let selection = super::import::selection_for_regions(std::slice::from_ref(parent), circles, true)?;
        let expected = declared_region(&selection, solved)?;
        if !super::profile_selection::same_region(combined, &expected) {
            return Err(invalid("profile members disagree with their combined boundary"));
        }
    }
    let selection = ProfileSel::PieceBoundaries { profiles, circle_seams };
    solidcraft_doc::piece_profile_selection_from_value(&serde_json::to_value(&selection).map_err(|e| invalid(&e.to_string()))?)?;
    profile_selection_indices(solved, &selection)?;
    Ok(selection)
}

fn merged(solved: &SolvedSketch, indices: &[usize]) -> Result<Vec<Region2>> {
    let regions = indices
        .iter()
        .map(|index| solved.profiles.get(*index).map(|p| p.region.clone()).ok_or_else(|| invalid("missing selected group region")))
        .collect::<Result<Vec<_>>>()?;
    Ok(merge_regions(&regions))
}

pub(super) fn declared_region(selection: &ProfileSel, solved: &SolvedSketch) -> Result<Region2> {
    let ProfileSel::PieceBoundaries { profiles, circle_seams } = selection else { return Err(invalid("missing directed group boundary")) };
    solidcraft_doc::piece_profile_selection_from_value(&serde_json::to_value(selection).map_err(|e| invalid(&e.to_string()))?)?;
    let (profiles, _) = solidcraft_doc::normalize_coincident_profile_boundaries(&solved.sketch, profiles, circle_seams)?;
    let [boundary] = profiles.as_slice() else { return Err(invalid("group requires one combined boundary")) };
    if let Ok(region) = boundary_region(boundary, circle_seams, solved) {
        return Ok(region);
    }
    // Native partitions may be local to the curves in this boundary. This graph
    // is only a certificate: the persisted selection still uses the full sketch.
    let sources: BTreeSet<_> = std::iter::once(&boundary.outer).chain(&boundary.holes).flatten().map(|p| &p.curve).collect();
    let mut scoped = solved.clone();
    for curve in &mut scoped.sketch.curves {
        curve.construction |= !sources.contains(&curve.id);
    }
    scoped.profiles = find_profiles(&scoped.sketch);
    boundary_region(boundary, circle_seams, &scoped)
}

fn boundary_region(boundary: &PieceProfileBoundary, seams: &BTreeMap<String, f64>, solved: &SolvedSketch) -> Result<Region2> {
    let fill = |pieces: &[ProfilePiece]| -> Result<_> {
        let indices = enclosed(solved, pieces, seams)?;
        let regions = merged(solved, &indices)?;
        let [region] = regions.as_slice() else { return Err(invalid("combined profile loop is disconnected")) };
        if !region.holes.is_empty() {
            return Err(invalid("combined profile loop does not enclose a filled region"));
        }
        Ok(region.outer.clone())
    };
    let outer = fill(&boundary.outer)?;
    let mut holes = Vec::new();
    for hole in &boundary.holes {
        let reversed: Vec<_> = hole.iter().map(|p| ProfilePiece { reversed: !p.reversed, ..p.clone() }).collect();
        holes.push(fill(&reversed)?.reversed());
    }
    Ok(Region2 { outer, holes })
}

/// Label graph faces inside/outside an explicitly directed loop. Across a named
/// boundary the two faces have opposite membership; across every other shared
/// edge they have equal membership. Unshared unnamed edges border the exterior.
/// Thus internal edges cancel without enumerating subsets or guessing points.
pub(super) fn enclosed(solved: &SolvedSketch, boundary: &[ProfilePiece], seams: &BTreeMap<String, f64>) -> Result<Vec<usize>> {
    let key = |piece: &ProfilePiece| ProfilePiece { reversed: false, ..piece.clone() };
    let mut wanted = BTreeMap::new();
    for piece in boundary {
        if wanted.insert(key(piece), piece.reversed).is_some() {
            return Err(invalid("repeated combined profile piece"));
        }
    }
    let mut edges: BTreeMap<_, Vec<(usize, bool)>> = BTreeMap::new();
    let mut total = 0usize;
    for (index, profile) in solved.profiles.iter().enumerate() {
        let (outer, holes) = profile.piece_boundaries_with_seams(seams).ok_or_else(|| invalid("uncertified combined profile graph"))?;
        let mut face_edges: BTreeMap<_, Vec<bool>> = BTreeMap::new();
        for piece in std::iter::once(&outer).chain(&holes).flatten() {
            total += 1;
            if total > 100_000 {
                return Err(invalid("excessive combined profile graph"));
            }
            face_edges.entry(key(piece)).or_default().push(piece.reversed);
        }
        for (key, directions) in face_edges {
            match directions.as_slice() {
                [direction] => edges.entry(key).or_default().push((index, *direction)),
                // Nested faces can express a compound hole as adjacent loops.
                // Their opposite shared spans are internal to this same face.
                [a, b] if a != b => {}
                _ => return Err(invalid("ambiguous combined profile face boundary")),
            }
        }
    }
    if wanted.is_empty() || wanted.keys().any(|key| !edges.contains_key(key)) {
        return Err(invalid("missing combined profile piece"));
    }
    let mut inside = vec![None; solved.profiles.len()];
    let mut adjacent = vec![Vec::new(); solved.profiles.len()];
    for (key, faces) in edges {
        if !matches!(faces.as_slice(), [_] | [_, _]) || matches!(faces.as_slice(), [(a, ar), (b, br)] if a == b || ar == br) {
            return Err(invalid("ambiguous combined profile graph"));
        }
        if let Some(direction) = wanted.get(&key) {
            if !faces.iter().any(|(_, reversed)| reversed == direction) {
                return Err(invalid("reversed combined profile boundary"));
            }
            for (face, reversed) in faces {
                assign(&mut inside, face, reversed == *direction)?;
            }
        } else {
            match faces.as_slice() {
                [(face, _)] => assign(&mut inside, *face, false)?,
                [(a, _), (b, _)] => {
                    adjacent.get_mut(*a).ok_or_else(|| invalid("missing group face"))?.push(*b);
                    adjacent.get_mut(*b).ok_or_else(|| invalid("missing group face"))?.push(*a);
                }
                _ => return Err(invalid("invalid combined profile graph")),
            }
        }
    }
    let mut queue: Vec<_> = inside.iter().enumerate().filter_map(|(index, state)| state.map(|value| (index, value))).collect();
    while let Some((index, value)) = queue.pop() {
        for neighbor in adjacent.get(index).ok_or_else(|| invalid("missing group adjacency"))? {
            let unknown = inside.get(*neighbor).is_some_and(Option::is_none);
            assign(&mut inside, *neighbor, value)?;
            if unknown {
                queue.push((*neighbor, value));
            }
        }
    }
    if inside.iter().any(Option::is_none) {
        return Err(invalid("ambiguous combined profile membership"));
    }
    let selected: Vec<_> = inside.iter().enumerate().filter_map(|(index, state)| (*state == Some(true)).then_some(index)).collect();
    if selected.is_empty() {
        return Err(invalid("empty combined profile region"));
    }
    Ok(selected)
}

fn assign(states: &mut [Option<bool>], index: usize, value: bool) -> Result<()> {
    let slot = states.get_mut(index).ok_or_else(|| invalid("missing combined profile face"))?;
    if slot.is_some_and(|old| old != value) {
        return Err(invalid("inconsistent combined profile boundary"));
    }
    *slot = Some(value);
    Ok(())
}
