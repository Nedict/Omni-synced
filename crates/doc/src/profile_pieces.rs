//! Bounded, persistent references to directed analytic profile pieces.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{DocError, PieceProfileBoundary, ProfileSel, Result, SolvedSketch};

fn invalid() -> DocError {
    DocError::Invalid("invalid analytic profile piece reference".into())
}

fn keys(value: &Value, expected: &[&str]) -> Result<()> {
    let fields = value.as_object().ok_or_else(invalid)?;
    if fields.len() != expected.len() || fields.keys().any(|key| !expected.contains(&key.as_str())) {
        return Err(invalid());
    }
    Ok(())
}

/// Check raw budgets before copying into typed document structures.
pub(crate) fn validate_raw(value: &Value) -> Result<()> {
    if let Some(seams) = value.get("circle_seams") {
        keys(value, &["type", "profiles", "circle_seams"])?;
        let seams = seams.as_object().ok_or_else(invalid)?;
        if seams.len() > 20_000
            || seams.iter().any(|(id, angle)| {
                id.is_empty()
                    || id.len() > 512
                    || id.chars().any(char::is_control)
                    || angle.as_f64().is_none_or(|angle| !angle.is_finite() || !(0.0..std::f64::consts::TAU).contains(&angle))
            })
        {
            return Err(invalid());
        }
    } else {
        keys(value, &["type", "profiles"])?;
    }
    if value.get("type").and_then(Value::as_str) != Some("piece_boundaries") {
        return Err(invalid());
    }
    let profiles = value.get("profiles").and_then(Value::as_array).ok_or_else(invalid)?;
    if profiles.is_empty() || profiles.len() > 128 {
        return Err(invalid());
    }
    let mut total = 0usize;
    for profile in profiles {
        keys(profile, &["outer", "holes"])?;
        let holes = profile.get("holes").and_then(Value::as_array).ok_or_else(invalid)?;
        if holes.len() > 128 {
            return Err(invalid());
        }
        for boundary in std::iter::once(profile.get("outer").ok_or_else(invalid)?).chain(holes) {
            let pieces = boundary.as_array().ok_or_else(invalid)?;
            total += pieces.len();
            if pieces.is_empty() || pieces.len() > 512 || total > 20_000 {
                return Err(invalid());
            }
            for piece in pieces {
                keys(piece, &["curve", "piece", "count", "reversed"])?;
                let id = piece.get("curve").and_then(Value::as_str).ok_or_else(invalid)?;
                let index = piece.get("piece").and_then(Value::as_u64).ok_or_else(invalid)?;
                let count = piece.get("count").and_then(Value::as_u64).ok_or_else(invalid)?;
                if id.is_empty()
                    || id.len() > 512
                    || id.chars().any(char::is_control)
                    || index == 0
                    || index > count
                    || count > 50_000
                    || piece.get("reversed").and_then(Value::as_bool).is_none()
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

pub fn piece_profile_selection_from_value(value: &Value) -> Result<ProfileSel> {
    validate_raw(value)?;
    let selection: ProfileSel = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if let ProfileSel::PieceBoundaries { profiles, circle_seams } = &selection {
        validate(profiles)?;
        validate_seams(profiles, circle_seams)?;
    }
    Ok(selection)
}

pub(crate) fn validate(profiles: &[PieceProfileBoundary]) -> Result<()> {
    if profiles.is_empty() || profiles.len() > 128 {
        return Err(invalid());
    }
    let mut total = 0usize;
    let mut identities = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for profile in profiles {
        if profile.holes.len() > 128
            || !identities.insert((&profile.outer, &profile.holes))
            || profile.holes.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid());
        }
        let mut seen = BTreeMap::new();
        for (boundary_index, boundary) in std::iter::once(&profile.outer).chain(&profile.holes).enumerate() {
            total += boundary.len();
            if boundary.is_empty() || boundary.len() > 512 || total > 20_000 || boundary.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid());
            }
            for part in boundary {
                if part.curve.is_empty()
                    || part.curve.len() > 512
                    || part.curve.chars().any(char::is_control)
                    || part.piece == 0
                    || part.piece > part.count
                    || part.count > 50_000
                    || counts.insert(&part.curve, part.count).is_some_and(|old| old != part.count)
                {
                    return Err(invalid());
                }
                // A nested graph component can give adjacent hole loops. An
                // edge shared by two such loops occurs once in each direction
                // and cancels from the compound hole. No other repetition is
                // valid; resolution still requires an exact regenerated face.
                let prior = seen.entry((&part.curve, part.piece)).or_insert((boundary_index, part.reversed, 0usize));
                if prior.2 > 0 && (prior.2 != 1 || prior.0 == 0 || boundary_index == 0 || prior.0 == boundary_index || prior.1 == part.reversed) {
                    return Err(invalid());
                }
                prior.2 += 1;
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_seams(profiles: &[PieceProfileBoundary], seams: &BTreeMap<String, f64>) -> Result<()> {
    if seams.len() > 20_000 {
        return Err(invalid());
    }
    let sources = profiles
        .iter()
        .flat_map(|profile| std::iter::once(&profile.outer).chain(&profile.holes))
        .flatten()
        .map(|part| &part.curve)
        .collect::<BTreeSet<_>>();
    for (id, angle) in seams {
        if !sources.contains(id) || !angle.is_finite() || !(0.0..std::f64::consts::TAU).contains(angle) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Collapse only explicit duplicate references to the same partition of
/// coincident full circles. Keep the original references in the document: this
/// certificate must be re-established after every edit and never comes from a
/// serialized profile cache. A reference to an alias alone is not sufficient.
pub fn normalize_coincident_profile_boundaries(
    sketch: &crate::sketch::Sketch,
    profiles: &[PieceProfileBoundary],
    seams: &BTreeMap<String, f64>,
) -> Result<(Vec<PieceProfileBoundary>, BTreeMap<String, String>)> {
    validate(profiles)?;
    validate_seams(profiles, seams)?;
    let sources: BTreeSet<_> = profiles.iter().flat_map(|p| std::iter::once(&p.outer).chain(&p.holes)).flatten().map(|p| p.curve.as_str()).collect();
    let mut carriers: Vec<(&str, solidcraft_geom::Vec2, f64, f64)> = Vec::new();
    let mut aliases = BTreeMap::new();
    let mut comparisons = 0usize;
    for curve in &sketch.curves {
        if curve.construction || !sources.contains(curve.id.as_str()) {
            continue;
        }
        let crate::sketch::CurveKind::Circle { c, r } = curve.kind else { continue };
        let center = sketch.point(c).ok_or_else(invalid)?;
        if !center.is_finite() || !r.is_finite() || r <= 0. {
            return Err(invalid());
        }
        let seam = seams.get(&curve.id).copied().unwrap_or(0.);
        let mut canonical = None;
        for (id, point, radius, phase) in &carriers {
            comparisons += 1;
            if comparisons > 2_000_000 {
                return Err(invalid());
            }
            // This is stricter than graph vertex merging. Compare every alias
            // with its representative, not with a chain of nearby circles.
            if center.dist(*point) + (r - radius).abs() <= 1e-9 && (seam - phase).abs() <= 1e-12 {
                canonical = Some(*id);
                break;
            }
        }
        if let Some(id) = canonical {
            aliases.insert(curve.id.clone(), id.to_owned());
        } else {
            carriers.push((&curve.id, center, r, seam));
        }
    }
    let mut occurrences: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    let mut references = BTreeSet::new();
    for (index, boundary) in profiles.iter().flat_map(|p| std::iter::once(&p.outer).chain(&p.holes)).enumerate() {
        for piece in boundary {
            occurrences.entry(&piece.curve).or_default().push((index, piece.piece, piece.count, piece.reversed));
            references.insert((index, piece.curve.as_str(), piece.piece, piece.count, piece.reversed));
        }
    }
    aliases.retain(|alias, canonical| {
        occurrences.get(alias.as_str()).is_some_and(|parts| {
            parts.iter().all(|(index, piece, count, reversed)| references.contains(&(*index, canonical.as_str(), *piece, *count, *reversed)))
        })
    });
    let mut normalized = profiles.to_vec();
    for profile in &mut normalized {
        for boundary in std::iter::once(&mut profile.outer).chain(&mut profile.holes) {
            for piece in boundary.iter_mut() {
                if let Some(id) = aliases.get(&piece.curve) {
                    piece.curve.clone_from(id);
                }
            }
            boundary.sort();
            boundary.dedup();
        }
        profile.holes.sort();
    }
    validate(&normalized)?;
    Ok((normalized, aliases))
}

pub(crate) fn indices(ss: &SolvedSketch, profiles: &[PieceProfileBoundary], seams: &BTreeMap<String, f64>) -> Result<Vec<usize>> {
    validate(profiles)?;
    validate_seams(profiles, seams)?;
    if !seams.is_empty() {
        let circles: BTreeSet<_> =
            ss.sketch.curves.iter().filter(|curve| matches!(curve.kind, crate::sketch::CurveKind::Circle { .. })).map(|curve| &curve.id).collect();
        if seams.keys().any(|id| !circles.contains(id)) {
            return Err(invalid());
        }
    }
    let current = ss
        .profiles
        .iter()
        .map(|profile| profile.piece_boundaries_with_seams(seams).map(|(outer, holes)| PieceProfileBoundary { outer, holes }))
        .collect::<Vec<_>>();
    let locate = |profiles: &[PieceProfileBoundary]| {
        profiles
            .iter()
            .map(|identity| {
                let mut matches = current.iter().enumerate().filter(|(_, candidate)| candidate.as_ref() == Some(identity));
                let (index, _) =
                    matches.next().ok_or_else(|| DocError::Unknown(format!("analytic profile pieces changed or missing in {}", ss.name)))?;
                if matches.next().is_some() {
                    return Err(DocError::Invalid(format!("ambiguous analytic profile pieces in {}", ss.name)));
                }
                Ok(index)
            })
            .collect::<Result<Vec<_>>>()
    };
    let original = locate(profiles);
    if original.is_ok() {
        return original;
    }
    let (normalized, aliases) = normalize_coincident_profile_boundaries(&ss.sketch, profiles, seams)?;
    if aliases.is_empty() { original } else { locate(&normalized) }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    use serde_json::json;
    use solidcraft_geom::Vec2;

    #[test]
    fn piece_references_are_bounded_canonical_and_disjoint() {
        let part = json!({"curve":"circle","piece":1,"count":2,"reversed":false});
        let good = json!({"type":"piece_boundaries","profiles":[{"outer":[part],"holes":[]}]});
        assert!(piece_profile_selection_from_value(&good).is_ok());
        for (key, bad) in [
            ("piece", json!(0)),
            ("piece", json!(3)),
            ("count", json!(50_001)),
            ("count", json!(-1)),
            ("reversed", json!(0)),
            ("curve", json!("x".repeat(513))),
            ("curve", json!("x\ny")),
        ] {
            let mut value = good.clone();
            value["profiles"][0]["outer"][0][key] = bad;
            assert!(piece_profile_selection_from_value(&value).is_err(), "{key}");
        }
        for bad in [
            json!({"outer":[part,part],"holes":[]}),
            json!({"outer":[part],"holes":[[part]]}),
            json!({"outer":[],"holes":[]}),
            json!({"outer":[part],"holes":[],"extra":true}),
        ] {
            let value = json!({"type":"piece_boundaries","profiles":[bad]});
            assert!(piece_profile_selection_from_value(&value).is_err());
        }
        let mut bad = good.clone();
        bad["profiles"] = json!(vec![good["profiles"][0].clone(); 129]);
        assert!(piece_profile_selection_from_value(&bad).is_err());
        let mut bad = good.clone();
        bad["profiles"][0]["outer"] = json!(vec![part; 513]);
        assert!(piece_profile_selection_from_value(&bad).is_err());
    }

    #[test]
    fn saved_circle_seams_are_optional_bounded_and_referenced() {
        let good = json!({"type":"piece_boundaries","profiles":[{"outer":[{"curve":"circle","piece":1,"count":4,"reversed":false}],"holes":[]}]});
        let legacy = piece_profile_selection_from_value(&good).unwrap();
        assert_eq!(serde_json::to_value(legacy).unwrap(), good);
        for seams in [json!({"circle":0.0}), json!({"circle":std::f64::consts::PI})] {
            let mut value = good.clone();
            value["circle_seams"] = seams;
            let parsed = piece_profile_selection_from_value(&value).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        }
        for seams in [json!([]), json!({"missing":1.0}), json!({"circle":-0.1}), json!({"circle":std::f64::consts::TAU}), json!({"circle":"1"})] {
            let mut value = good.clone();
            value["circle_seams"] = seams;
            assert!(piece_profile_selection_from_value(&value).is_err(), "{value}");
        }
        let ProfileSel::PieceBoundaries { profiles, .. } = piece_profile_selection_from_value(&good).unwrap() else { panic!("selection") };
        for angle in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(validate_seams(&profiles, &BTreeMap::from([("circle".into(), angle)])).is_err());
        }
    }

    #[test]
    fn coincident_circle_aliases_require_duplicate_identity_and_live_geometry() {
        let mut sketch = crate::sketch::Sketch::new();
        sketch.add_circle(Vec2::ZERO, 30., None, Some("left")).unwrap();
        sketch.add_circle(Vec2::new(40., 0.), 30., None, Some("right")).unwrap();
        sketch.add_circle(Vec2::new(1e-14, 0.), 30., None, Some("alias")).unwrap();
        let piece = |curve: &str, part| crate::sketch::ProfilePiece { curve: curve.into(), piece: part, count: 2, reversed: false };
        let expected = PieceProfileBoundary { outer: vec![piece("left", 1), piece("right", 2)], holes: vec![] };
        let mut selected = expected.clone();
        selected.outer.insert(0, piece("alias", 1));
        let empty = BTreeMap::new();
        let (normalized, aliases) = normalize_coincident_profile_boundaries(&sketch, &[selected.clone()], &empty).unwrap();
        assert_eq!(normalized, [expected]);
        assert_eq!(aliases, BTreeMap::from([("alias".into(), "left".into())]));
        let solved = |mut sketch: crate::sketch::Sketch| {
            let report = crate::sketch::solve(&mut sketch);
            let profiles = crate::sketch::find_profiles(&sketch);
            SolvedSketch { feature: 1, name: "Aliases".into(), plane: solidcraft_geom::Plane::named("XY").unwrap(), sketch, report, profiles }
        };
        assert!(indices(&solved(sketch.clone()), &[selected.clone()], &empty).is_ok());
        // Every damaged reference below must fail, rather than dropping the alias.
        for case in 0..7 {
            let mut changed = sketch.clone();
            let mut wanted = selected.clone();
            let mut seams = BTreeMap::new();
            match case {
                0 => {
                    let crate::sketch::CurveKind::Circle { r, .. } = &mut changed.curves[2].kind else { panic!("circle") };
                    *r += 0.1;
                }
                1 => changed.curves[2].construction = true,
                2 => wanted.outer[0].reversed = true,
                3 => wanted.outer[0].count = 3,
                4 => wanted.outer[0].piece = 2,
                5 => {
                    wanted.outer.remove(1);
                }
                6 => {
                    seams.insert("alias".into(), std::f64::consts::PI);
                }
                _ => panic!("case"),
            }
            assert!(indices(&solved(changed), &[wanted], &seams).is_err(), "case {case}");
        }
    }

    #[test]
    fn analytic_piece_identity_includes_holes_and_never_trusts_cached_profiles() {
        let mut sketch = crate::sketch::Sketch::new();
        sketch.add_circle(Vec2::ZERO, 30., None, Some("left")).unwrap();
        sketch.add_circle(Vec2::new(40., 0.), 30., None, Some("right")).unwrap();
        sketch.add_circle(Vec2::new(20., 0.), 2., None, Some("hole")).unwrap();
        let profiles = crate::sketch::find_profiles(&sketch);
        let ring = profiles.iter().find(|p| !p.hole_curves.is_empty()).unwrap();
        let (outer, holes) = ring.piece_boundaries().unwrap();
        assert_eq!(
            outer.iter().map(|p| (p.curve.as_str(), p.piece, p.count, p.reversed)).collect::<Vec<_>>(),
            [("left", 1, 2, false), ("right", 2, 2, false)]
        );
        assert_eq!(holes.len(), 1);
        assert!(holes[0][0].reversed);
        let copy: crate::sketch::Profile = serde_json::from_str(&serde_json::to_string(ring).unwrap()).unwrap();
        assert!(copy.piece_boundaries().is_none());
        let identity = PieceProfileBoundary { outer, holes };
        validate(std::slice::from_ref(&identity)).unwrap();
        sketch.add_circle(Vec2::new(20., 5.), 1., None, Some("new-hole")).unwrap();
        assert!(
            !crate::sketch::find_profiles(&sketch).iter().any(|p| p.piece_boundaries() == Some((identity.outer.clone(), identity.holes.clone())))
        );
    }

    #[test]
    fn compound_holes_allow_only_opposite_shared_spans_in_distinct_loops() {
        let mut sketch = crate::sketch::Sketch::new();
        sketch.add_circle(Vec2::ZERO, 30., None, Some("outer")).unwrap();
        sketch.add_circle(Vec2::ZERO, 10., None, Some("inner")).unwrap();
        sketch.add_circle(Vec2::new(10., 0.), 8., None, Some("divider")).unwrap();
        let profiles = crate::sketch::find_profiles(&sketch);
        let ring = profiles.iter().find(|p| p.hole_curves.len() > 1).unwrap();
        let (outer, holes) = ring.piece_boundaries().unwrap();
        let identity = PieceProfileBoundary { outer, holes };
        validate(std::slice::from_ref(&identity)).unwrap();
        let mut occurrences: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (h, hole) in identity.holes.iter().enumerate() {
            for (p, piece) in hole.iter().enumerate() {
                occurrences.entry((&piece.curve, piece.piece)).or_default().push((h, p));
            }
        }
        let shared = occurrences.values().find(|v| v.len() == 2).unwrap();
        let [(h, p), (j, q)] = shared.as_slice() else { panic!("shared span") };
        let mut wrong = identity.clone();
        wrong.holes[*j][*q].reversed = wrong.holes[*h][*p].reversed;
        wrong.holes[*j].sort();
        wrong.holes.sort();
        assert!(validate(&[wrong]).is_err());
        let mut wrong = identity.clone();
        wrong.outer.push(wrong.holes[*h][*p].clone());
        wrong.outer.sort();
        assert!(validate(&[wrong]).is_err());
        let mut wrong = identity.clone();
        wrong.holes.push(vec![wrong.holes[*h][*p].clone()]);
        wrong.holes.sort();
        assert!(validate(&[wrong]).is_err());
    }
}
