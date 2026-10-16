//! Explicit extrusion face labels linked to complete sketch curves. Labels are
//! never inferred from registration order, coordinates, or cached B-rep faces.
use super::history::{finish, payload, require_class, zeros};
use super::*;

const LABELS: &str = "B7F34D7B-DAC4-41A5-B67D-D672537C3BFA";
const WRAPPER: &str = "335CC7BD-BBE5-40AA-9E8C-5EC99D427A3A";
const CURVE: &str = "2DEE34BE-FA70-4D54-979C-164D86FDA786";

pub(super) fn label_table(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64) -> Result<BTreeMap<u64, u64>> {
    if !classes.get(&feature).is_some_and(|c| {
        matches!(
            (c.id.as_str(), c.version),
            ("DD405BC2-D673-44F0-8833-5CB2A1C186C7", 8)
                | (super::fillets::CLASS, 2 | 3)
                | (super::chamfers::CLASS, 2 | 3)
                | (super::lofts::CLASS, 9)
                | (super::mirrors::CLASS, 0)
                | (super::circular_patterns::CLASS, 0)
        )
    }) {
        return Err(invalid("unsupported feature label-table owner"));
    }
    // Abstract inherited classes have no object IDs; consult the class table,
    // not the object-to-class index, when locating this explicit reference.
    let version = records
        .design
        .classes
        .iter()
        .find(|c| c.id == "A0A15D26-1F3B-4120-A3F1-9CDDA189AB74")
        .map(|c| c.version)
        .unwrap_or(if records.design.layout == 8 { 2 } else { 0 });
    let tail = match version {
        1 => 58,
        2 => 59,
        _ => return Err(invalid("unsupported side-role inherited feature version")),
    };
    let bytes = records.primary(feature)?;
    let mut r = payload(bytes, feature)?;
    r.at = bytes.len().checked_sub(tail).filter(|at| *at >= r.at).ok_or_else(|| invalid("truncated side-role feature tail"))?;
    let labels = r.reference()?;
    require_class(classes, labels, LABELS, 2)?;
    let mut r = payload(records.primary(labels)?, labels)?;
    zeros(&mut r, 6)?;
    let count = r.count(19)?;
    if count > 1024 {
        return Err(invalid("extrusion label table exceeds budget"));
    }
    let mut entries = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for _ in 0..count {
        let id = r.reference()?;
        let role = r.u64()?;
        let minimum = if classes.get(&feature).is_some_and(|c| {
            matches!(c.id.as_str(), super::mirrors::CLASS | super::circular_patterns::CLASS | super::fillets::CLASS | super::chamfers::CLASS)
        }) {
            1
        } else {
            3
        };
        if role < minimum || !classes.contains_key(&id) || !ids.insert(id) || entries.insert(role, id).is_some() {
            return Err(invalid("invalid or duplicate extrusion label"));
        }
    }
    let next = r.u64()?;
    if entries.keys().any(|role| *role >= next) {
        return Err(invalid("inconsistent extrusion label counter"));
    }
    finish(&r)?;
    let mut r = payload(records.secondary(labels)?, labels)?;
    zeros(&mut r, 5)?;
    finish(&r)?;
    Ok(entries)
}

pub(super) fn circle_tag(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, label: u64) -> Result<u64> {
    require_class(classes, feature, "DD405BC2-D673-44F0-8833-5CB2A1C186C7", 8)?;
    let entries = label_table(records, classes, feature)?;
    let id = *entries.get(&label).ok_or_else(|| invalid("missing circular side label"))?;
    require_class(classes, id, WRAPPER, 0)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 5)?;
    if r.u32()? != 1 {
        return Err(invalid("circular side requires one source"));
    }
    zeros(&mut r, 1)?;
    let curve = r.reference()?;
    zeros(&mut r, 1)?;
    finish(&r)?;
    require_class(classes, curve, CURVE, 2)?;
    let mut r = payload(records.primary(curve)?, curve)?;
    zeros(&mut r, 6)?;
    for value in [1, 0, 1, 3] {
        if r.u32()? != value {
            return Err(invalid("unsupported circular side source header"));
        }
    }
    let tag = r.u64()?;
    if tag == 0 || r.u64()? != 0 || r.u32()? != 1 || r.u32()? != 1 || !matches!(r.u32()?, 1 | 2) {
        return Err(invalid("unsupported circular side source span"));
    }
    zeros(&mut r, 8)?;
    let mut ends = Vec::new();
    for _ in 0..2 {
        let count = r.count(8)?;
        if !matches!(count, 0 | 2) {
            return Err(invalid("unsupported circular side incidence"));
        }
        let mut adjacent = BTreeSet::new();
        for _ in 0..count {
            let neighbor = r.u64()?;
            if neighbor == 0 || neighbor == tag || !adjacent.insert(neighbor) {
                return Err(invalid("invalid circular side incidence"));
            }
        }
        ends.push(adjacent);
    }
    if ends.first() != ends.get(1) {
        return Err(invalid("circular side is not a closed source carrier"));
    }
    finish(&r)?;
    // The caller must also prove the selected reconstructed profile is this
    // whole circle; a retained source span alone never selects a partial region.
    Ok(tag)
}

pub(super) fn curve_span(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, label: u64) -> Result<CurveSpan> {
    require_class(classes, feature, "DD405BC2-D673-44F0-8833-5CB2A1C186C7", 8)?;
    let entries = label_table(records, classes, feature)?;
    let id = *entries.get(&label).ok_or_else(|| invalid("missing extrusion side label"))?;
    require_class(classes, id, WRAPPER, 0)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 5)?;
    if r.u32()? != 1 {
        return Err(invalid("unsupported side-role source count"));
    }
    zeros(&mut r, 1)?;
    let curve = r.reference()?;
    zeros(&mut r, 1)?;
    finish(&r)?;
    require_class(classes, curve, CURVE, 2)?;
    let mut r = payload(records.primary(curve)?, curve)?;
    zeros(&mut r, 6)?;
    for value in [1, 0, 1, 3] {
        if r.u32()? != value {
            return Err(invalid("unsupported side-role curve header"));
        }
    }
    let profile_id = r.u64()?;
    if profile_id == 0 || r.u64()? != 0 {
        return Err(invalid("unsupported side-role curve piece"));
    }
    if r.u32()? != 1 {
        return Err(invalid("unsupported side-role curve direction"));
    }
    let piece = (r.u32()?, r.u32()?);
    if piece.0 == 0 || piece.0 > piece.1 || piece.1 > 512 {
        return Err(invalid("invalid side-role curve partition"));
    }
    zeros(&mut r, 8)?;
    let mut adjacent = [Vec::new(), Vec::new()];
    for neighbors in &mut adjacent {
        let count = r.count(8)?;
        if !(1..=2).contains(&count) {
            return Err(invalid("unsupported side-role adjacency count"));
        }
        for _ in 0..count {
            let id = r.u64()?;
            if id == 0 || id == profile_id || neighbors.contains(&id) {
                return Err(invalid("invalid side-role adjacency"));
            }
            neighbors.push(id);
        }
    }
    finish(&r)?;
    Ok(CurveSpan { tag: profile_id, piece, adjacent })
}

pub(super) struct CurveSpan {
    pub tag: u64,
    pub piece: (u32, u32),
    pub adjacent: [Vec<u64>; 2],
}

fn curve_identity(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, label: u64) -> Result<(u64, [u64; 2])> {
    let span = curve_span(records, classes, feature, label)?;
    let [a] = span.adjacent[0].as_slice() else { return Err(invalid("unsupported side-role adjacency count")) };
    let [b] = span.adjacent[1].as_slice() else { return Err(invalid("unsupported side-role adjacency count")) };
    if span.piece != (1, 1) || a == b {
        return Err(invalid("unsupported complete side-role curve"));
    }
    Ok((span.tag, [*a, *b]))
}

pub(super) fn curve(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    feature: u64,
    label: u64,
    profile: &SketchProfile,
    lines: &BTreeMap<u64, SketchLine>,
) -> Result<u64> {
    let (identity, neighbors) = curve_identity(records, classes, feature, label)?;
    let [loops] = profile.regions.as_slice() else { return Err(invalid("side-role requires one profile region")) };
    let [boundary] = loops.as_slice() else { return Err(invalid("side-role requires one profile boundary")) };
    let mut owned = BTreeMap::new();
    for id in &boundary.curves {
        let line = lines.get(id).filter(|l| l.sketch == profile.geometry).ok_or_else(|| invalid("missing side-role sketch line"))?;
        let Some((key, 0)) = line.profile_id else { return Err(invalid("unsupported side-role sketch curve piece")) };
        if owned.insert(key, line).is_some() {
            return Err(invalid("ambiguous side-role sketch curve"));
        }
    }
    let line = owned.get(&identity).ok_or_else(|| invalid("side-role curve outside selected profile"))?;
    let mut shared = BTreeSet::new();
    for neighbor in neighbors {
        let other = owned.get(&neighbor).ok_or_else(|| invalid("side-role neighbor outside selected profile"))?;
        // The explicit curve IDs already establish ownership and adjacency.
        // Separate sketch-point objects can represent one constrained vertex;
        // validate their positions instead of requiring object-ID equality.
        let common: Vec<_> = [line.start, line.end]
            .into_iter()
            .enumerate()
            .filter_map(|(slot, p)| [other.start, other.end].iter().any(|q| p.iter().zip(q).all(|(a, b)| (a - b).abs() < 1e-10)).then_some(slot))
            .collect();
        let [point] = common.as_slice() else { return Err(invalid("side-role curves are not incident")) };
        if !shared.insert(*point) {
            return Err(invalid("side-role neighbors share the same endpoint"));
        }
    }
    Ok(line.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::import_tests::Fixture;
    use crate::f3d::relationship_tests::{reference, u32, u64};

    // Independently authored label numbers and profile identities. The label
    // sequence intentionally differs from curve registration and loop order.
    fn authored() -> Fixture {
        let mut f = crate::f3d::polygon_tests::rectangle();
        let bytes = &mut f.0.get_mut(&9).unwrap().2;
        let at = bytes.len() - 59;
        let mut slot = Vec::new();
        reference(&mut slot, 600);
        bytes[at..at + 11].copy_from_slice(&slot);
        let mut labels = vec![0; 6];
        u32(&mut labels, 4);
        for (i, (label, profile, neighbors)) in
            [(23, 19, [18, 20]), (13, 17, [20, 18]), (47, 20, [19, 17]), (17, 18, [17, 19])].into_iter().enumerate()
        {
            let id = 701 + i as u64 * 2;
            reference(&mut labels, id);
            u64(&mut labels, label);
            let mut wrapper = vec![0; 5];
            u32(&mut wrapper, 1);
            wrapper.push(0);
            reference(&mut wrapper, id + 1);
            wrapper.push(0);
            f.put(id, WRAPPER, 0, wrapper, None);
            let mut curve = vec![0; 6];
            for n in [1, 0, 1, 3] {
                u32(&mut curve, n);
            }
            u64(&mut curve, profile);
            u64(&mut curve, 0);
            for n in [1, 1, 1, 0, 0] {
                u32(&mut curve, n);
            }
            for neighbor in neighbors {
                u32(&mut curve, 1);
                u64(&mut curve, neighbor);
            }
            f.put(id + 1, CURVE, 2, curve, None);
        }
        u64(&mut labels, 48);
        f.put(600, LABELS, 2, labels, Some(vec![0; 5]));
        f
    }

    #[test]
    fn fillet_output_tables_keep_explicit_labels_including_one_and_two() {
        for version in [2, 3] {
            let mut fixture = authored();
            let owner = fixture.0.get_mut(&9).unwrap();
            owner.0 = super::super::fillets::CLASS.into();
            owner.1 = version;
            let labels = &mut fixture.0.get_mut(&600).unwrap().2;
            labels[21..29].copy_from_slice(&1_u64.to_le_bytes());
            labels[40..48].copy_from_slice(&2_u64.to_le_bytes());
            let design = fixture.design();
            let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            let table = label_table(&Records::new(&design), &classes, 9).unwrap();
            assert_eq!(table, BTreeMap::from([(1, 701), (2, 703), (47, 705), (17, 707)]));
        }
    }

    fn resolve(f: Fixture, label: u64, reverse: bool) -> Result<u64> {
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let profile = &d.sketch_profiles()?[0];
        let mut lines: BTreeMap<_, _> = d
            .sketch_lines()?
            .into_iter()
            .map(|mut l| {
                l.profile_id = l.profile_id.map(|(id, _)| (id, 0));
                if reverse {
                    std::mem::swap(&mut l.start, &mut l.end);
                    std::mem::swap(&mut l.start_point, &mut l.end_point);
                }
                (l.id, l)
            })
            .collect();
        if label == 99 {
            lines.get_mut(&31).unwrap().start_point = 999;
            lines.get_mut(&31).unwrap().start = [999., 999., 0.];
        } else if label == 98 {
            lines.get_mut(&31).unwrap().start_point = 998;
            lines.get_mut(&31).unwrap().end_point = 999;
        }
        curve(&Records::new(&d), &classes, 9, if label >= 98 { 13 } else { label }, profile, &lines)
    }

    #[test]
    fn side_labels_follow_explicit_curve_and_endpoint_references() {
        for reverse in [false, true] {
            for (label, expected) in [(13, 31), (17, 32), (23, 33), (47, 34)] {
                assert_eq!(resolve(authored(), label, reverse).unwrap(), expected);
            }
        }
        assert_eq!(resolve(authored(), 98, false).unwrap(), 31, "distinct point objects may represent the same incident endpoints");
        assert!(resolve(authored(), 99, false).is_err(), "referenced neighbors must actually meet the selected curve");
        assert!(resolve(authored(), 3, false).is_err(), "labels are not ordinal curve positions");
    }

    #[test]
    fn legacy_tail_uses_abstract_class_version_for_explicit_label_reference() {
        let mut f = authored();
        let bytes = &mut f.0.get_mut(&9).unwrap().2;
        bytes.remove(bytes.len() - 15);
        let mut d = f.design();
        d.classes.push(Class {
            id: "A0A15D26-1F3B-4120-A3F1-9CDDA189AB74".into(),
            parent: String::new(),
            version: 1,
            group: String::new(),
            objects: Vec::new(),
        });
        let resolve = |d: &Design| {
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            curve_identity(&Records::new(d), &classes, 9, 23)
        };
        assert_eq!(resolve(&d).unwrap(), (19, [18, 20]));
        for version in [0, 2, 3] {
            d.classes.last_mut().unwrap().version = version;
            assert!(resolve(&d).is_err(), "wrong inherited version {version}");
        }
    }

    #[test]
    fn side_span_partitions_retain_every_coincident_endpoint_neighbor() {
        let make = || {
            let mut f = authored();
            let bytes = &mut f.0.get_mut(&702).unwrap().2;
            bytes[42..46].copy_from_slice(&2_u32.to_le_bytes());
            bytes[46..50].copy_from_slice(&2_u32.to_le_bytes());
            bytes[58..62].copy_from_slice(&2_u32.to_le_bytes());
            bytes.splice(70..70, 99_u64.to_le_bytes());
            f
        };
        let parse = |f: Fixture| {
            let d = f.design();
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            curve_span(&Records::new(&d), &classes, 9, 23)
        };
        let span = parse(make()).unwrap();
        assert_eq!(span.tag, 19);
        assert_eq!(span.piece, (2, 2));
        assert_eq!(span.adjacent, [vec![18, 99], vec![20]]);
        for keep in 0..make().0[&702].2.len() {
            let mut f = make();
            f.0.get_mut(&702).unwrap().2.truncate(keep);
            assert!(parse(f).is_err(), "truncated at {keep}");
        }
        for (at, bytes) in [
            (42, 0_u32.to_le_bytes().to_vec()),
            (42, 3_u32.to_le_bytes().to_vec()),
            (46, 513_u32.to_le_bytes().to_vec()),
            (58, u32::MAX.to_le_bytes().to_vec()),
            (70, 18_u64.to_le_bytes().to_vec()),
        ] {
            let mut f = make();
            f.0.get_mut(&702).unwrap().2[at..at + bytes.len()].copy_from_slice(&bytes);
            assert!(parse(f).is_err(), "invalid field {at}");
        }
    }

    #[test]
    fn malformed_side_role_records_fail_before_attachment() {
        for id in [600, 701, 702] {
            for keep in 0..authored().0[&id].2.len() {
                let mut f = authored();
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(resolve(f, 23, false).is_err(), "{id}/{keep}");
            }
        }
        for (id, at, bytes) in [
            (600, 6, u32::MAX.to_le_bytes().to_vec()),
            (600, 40, 23_u64.to_le_bytes().to_vec()),
            (701, 5, 2_u32.to_le_bytes().to_vec()),
            (702, 22, 999_u64.to_le_bytes().to_vec()),
            (702, 30, 1_u64.to_le_bytes().to_vec()),
            (702, 62, 999_u64.to_le_bytes().to_vec()),
        ] {
            let mut f = authored();
            f.0.get_mut(&id).unwrap().2[at..at + bytes.len()].copy_from_slice(&bytes);
            assert!(resolve(f, 23, false).is_err(), "{id}/{at}");
        }
    }
}
