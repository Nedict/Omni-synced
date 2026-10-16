//! Body and feature mirrors observed with source, plane, suppression and mode controls.
//! Source recipes and tracked planes select geometry; evaluation caches do not.
use super::history::{empty, finish, payload, references, require_class, zeros};
use super::*;

pub(super) const CLASS: &str = "D1728651-3640-4CCF-8083-AF7703013978";
const HELPER: &str = "C444E6AD-33DC-4877-B037-A3A5518856B1";
const SOURCE: &str = "8DB90DAE-3FF5-4E01-BACC-00AAA3EBB3EF";
const INSTANCE: &str = "AD719C1A-11AF-4537-BB41-030A9091D36B";

pub(super) fn inputs(
    r: &mut Reader<'_>,
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    owner: u64,
    body_version: u32,
) -> Result<HistoryKind> {
    empty(r)?;
    zeros(r, 1)?;
    if !super::attributes::read(r)?.is_empty() {
        return Err(invalid("unsupported mirror attributes"));
    }
    let helper = r.reference()?;
    require_class(classes, helper, HELPER, 1)?;
    let sources = r.reference()?;
    require_class(classes, sources, SOURCE, 1)?;
    let roles = super::history::body_roles(r, classes, body_version)?;
    let outputs = roles.get(1).ok_or_else(|| invalid("missing mirror outputs"))?;
    if outputs.len() > 64 || roles.iter().enumerate().any(|(i, ids)| i != 1 && !ids.is_empty()) {
        return Err(invalid("unsupported mirror body roles"));
    }
    let mut unique = BTreeSet::new();
    for output in outputs {
        if !unique.insert(*output) {
            return Err(invalid("duplicate mirror output"));
        }
        require_class(classes, *output, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2)?;
        let mut o = payload(records.primary(*output)?, *output)?;
        zeros(&mut o, 6)?;
        if o.wide_string()?.is_empty() {
            return Err(invalid("unsupported mirror output identity"));
        }
        // A later feature can consume an output without suppressing its producer.
        // Suppression comes from the inherited timeline feature record.
        if !matches!(o.take(2)?, [0, 1] | [1, 1]) {
            return Err(invalid("unsupported mirror output state"));
        }
        let body = o.reference()?;
        if !classes.get(&body).is_some_and(|c| c.id == "D3937028-C20C-4E65-B010-94AD418A5C20" && matches!(c.version, 18 | 19))
            || o.reference()? != owner
        {
            return Err(invalid("mirror output belongs to another feature"));
        }
        finish(&o)?;
    }
    let mut h = payload(records.primary(helper)?, helper)?;
    zeros(&mut h, 6)?;
    if h.take(1)? != [0] {
        return Err(invalid("combined mirror history is not supported yet"));
    }
    require_class(classes, h.reference()?, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0)?;
    let plane_container = h.reference()?;
    let handles = references(&mut h)?;
    if handles.len() > 64 || handles.iter().collect::<BTreeSet<_>>().len() != handles.len() {
        return Err(invalid("invalid mirror target count"));
    }
    let targets = handles.iter().map(|id| super::loft_faces::producer(records, classes, *id)).collect::<Result<Vec<_>>>()?;
    let pattern_version = records
        .design
        .classes
        .iter()
        .find(|c| c.id == "3EF91258-FE47-45A5-957D-1295C3679496")
        .map(|c| c.version)
        .unwrap_or(if records.design.layout == 8 { 9 } else { 0 });
    zeros(
        &mut h,
        match pattern_version {
            6 => 0,
            8 => 4,
            9 => 8,
            _ => return Err(invalid("unsupported inherited mirror pattern schema")),
        },
    )?;
    let compute = h.u32()?;
    let selection = h.u32()?;
    if compute != 2 || !matches!(selection, 1 | 2) || (pattern_version >= 8 && h.u32()? != u32::MAX) || h.take(5)? != [1, 1, 0, 0, 0] || h.u32()? != 2
    {
        return Err(invalid("unsupported mirror selection or computation mode"));
    }
    zeros(&mut h, 4)?;
    if h.u64()? == 0 {
        return Err(invalid("empty mirror evaluation operation"));
    }
    let instances = references(&mut h)?;
    if instances.len() != 2 {
        return Err(invalid("unsupported mirror instance count"));
    }
    for (i, instance) in instances.iter().enumerate() {
        require_class(classes, *instance, INSTANCE, 0)?;
        let mut instance = payload(records.primary(*instance)?, *instance)?;
        zeros(&mut instance, 6)?;
        if instance.u32()? != i as u32 + 1 || instance.reference()? != helper {
            return Err(invalid("inconsistent mirror instance ownership"));
        }
        finish(&instance)?;
    }
    let cache = h.reference()?;
    require_class(classes, cache, "A085449A-5144-4B2B-B455-7F7035A40559", 1)?;
    let bytes = records.primary(cache)?;
    let mut cache = payload(bytes, cache)?;
    zeros(&mut cache, 4)?;
    if cache.take(5)? != [1, 1, 0, 0, 0] || cache.reference()? != owner {
        return Err(invalid("inconsistent mirror cache owner"));
    }
    owner_tail(&mut cache, owner)?;
    if h.reference()? != owner {
        return Err(invalid("inconsistent mirror helper owner"));
    }
    let source_container = h.reference()?;
    zeros(&mut h, 4)?;
    finish(&h)?;
    let objects = super::loft_faces::objects(records, classes, source_container, owner)?;
    if !(1..=64).contains(&objects.len()) || objects.iter().collect::<BTreeSet<_>>().len() != objects.len() {
        return Err(invalid("inconsistent mirror source count"));
    }
    if selection == 2 && (objects.len() != outputs.len() || !targets.is_empty()) {
        return Err(invalid("unsupported body mirror target or output count"));
    }
    let mut s = payload(records.primary(sources)?, sources)?;
    zeros(&mut s, 6)?;
    for cached in references(&mut s)? {
        require_class(classes, cached, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1)?;
    }
    if s.count(15)? != objects.len() {
        return Err(invalid("inconsistent mirror source relationship count"));
    }
    let mut features = Vec::new();
    let mut new_bodies = 0;
    for object in objects {
        if selection == 2 {
            require_class(classes, object, "9716F783-676D-42E0-93F9-EBF273E7C035", 1)?;
        } else {
            let (feature, bodies) = feature_source(records, classes, object, owner)?;
            if features.contains(&feature) {
                return Err(invalid("duplicate mirrored feature"));
            }
            let mut input = payload(records.primary(feature)?, feature)?;
            let input = super::history::extrude_inputs(&mut input, classes)?;
            if !matches!(input.limit, super::history::ExtrudeLimit::Distance(_)) {
                return Err(invalid("To Object mirrored feature computation is not supported yet"));
            }
            match input.operation {
                solidcraft_doc::Operation::NewBody if bodies == [feature] => new_bodies += 1,
                solidcraft_doc::Operation::Join | solidcraft_doc::Operation::Cut if !targets.is_empty() && bodies == targets => {}
                _ => return Err(invalid("mirrored feature operation disagrees with its target bodies")),
            }
            features.push(feature);
        }
        if s.reference()? != object {
            return Err(invalid("mirror source relationships disagree"));
        }
        for cached in references(&mut s)? {
            require_class(classes, cached, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1)?;
        }
    }
    if s.reference()? != owner {
        return Err(invalid("inconsistent mirror source owner"));
    }
    owner_tail(&mut s, owner)?;
    let planes = super::loft_faces::objects(records, classes, plane_container, owner)?;
    let [plane] = planes.as_slice() else { return Err(invalid("mirror requires one tracked plane")) };
    let plane = super::planes::tracked_plane(records, classes, *plane, owner)?;
    let kind = if selection == 2 {
        HistoryKind::Mirror { plane, source_container, output_count: outputs.len() }
    } else {
        if outputs.len() != new_bodies || (new_bodies == features.len() && !targets.is_empty()) {
            return Err(invalid("mirrored feature outputs disagree with source operations"));
        }
        HistoryKind::MirrorFeatures { plane, sources: features, targets, output_count: outputs.len() }
    };
    Ok(kind)
}

// A mirrored body's label groups name its original body, the mirror operation,
// and the transformed instance. All three must belong to this exact mirror.
pub(super) fn output_operations(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    owners: &BTreeMap<u64, u64>,
    feature: u64,
    handle: u64,
    operation: u64,
) -> Result<BTreeSet<u64>> {
    require_class(classes, feature, CLASS, 0)?;
    let version = records
        .design
        .classes
        .iter()
        .find(|c| c.id == "2EDC2536-D95B-4469-8ACA-DC07124D9C43")
        .map(|c| c.version)
        .unwrap_or(if records.design.layout == 8 { 2 } else { 0 });
    let mut r = payload(records.primary(feature)?, feature)?;
    let HistoryKind::Mirror { source_container, output_count: 1, .. } = inputs(&mut r, records, classes, feature, version)? else {
        return Err(invalid("Combine requires an unambiguous single body-mirror output"));
    };
    let mut r = payload(records.primary(feature)?, feature)?;
    empty(&mut r)?;
    zeros(&mut r, 1)?;
    super::attributes::read(&mut r)?;
    let helper = r.reference()?;
    r.reference()?;
    let roles = super::history::body_roles(&mut r, classes, version)?;
    if roles.get(1).is_none_or(|ids| ids.as_slice() != [handle]) || owners.get(&operation) != Some(&feature) {
        return Err(invalid("mirror body label disagrees with output ownership"));
    }
    let source = super::loft_faces::target(records, classes, source_container, feature)?;
    let [source_operation] = source.operations.as_slice() else { return Err(invalid("unsupported composite mirror source")) };
    if owners.get(&source_operation.operation) != Some(&source.feature) {
        return Err(invalid("mirror body source operation belongs to another feature"));
    }
    let mut groups = BTreeSet::from([source_operation.operation, operation]);
    for (tag, owner) in owners {
        if !classes.get(owner).is_some_and(|c| c.id == INSTANCE && c.version == 0) {
            continue;
        }
        let mut r = payload(records.primary(*owner)?, *owner)?;
        zeros(&mut r, 6)?;
        let index = r.u32()?;
        let parent = r.reference()?;
        finish(&r)?;
        if parent == helper && (index != 2 || !groups.insert(*tag)) {
            return Err(invalid("unsupported mirror body instance operation"));
        }
    }
    if groups.len() != 3 {
        return Err(invalid("missing mirror body instance operation"));
    }
    Ok(groups)
}

pub(super) fn feature_source(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, owner: u64) -> Result<(u64, Vec<u64>)> {
    require_class(classes, id, "5A1BF548-241F-46FD-9FB5-E4B05126EB9D", 1)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    let tracker = r.reference()?;
    require_class(classes, tracker, "90055C05-546C-4EE7-B3C9-3DD922AD0C9C", 0)?;
    if r.u32()? != 1 {
        return Err(invalid("unsupported mirror feature tracker"));
    }
    r.wide_string()?;
    r.wide_string()?;
    if r.u32()? != 2 {
        return Err(invalid("unsupported mirror feature selection"));
    }
    zeros(&mut r, 8)?;
    let handles = references(&mut r)?;
    if !(1..=64).contains(&handles.len()) || handles.iter().collect::<BTreeSet<_>>().len() != handles.len() {
        return Err(invalid("invalid mirrored feature body handles"));
    }
    let inherited_version = records
        .design
        .classes
        .iter()
        .find(|c| c.id == "A0A15D26-1F3B-4120-A3F1-9CDDA189AB74")
        .map(|c| c.version)
        .unwrap_or(if records.design.layout == 8 { 2 } else { 0 });
    r.take(match inherited_version {
        1 => 55,
        2 => 56,
        _ => return Err(invalid("unsupported inherited mirror selector schema")),
    })?; // inherited evaluation state; never selects a feature or body
    if r.reference()? != owner {
        return Err(invalid("mirror feature selector belongs to another owner"));
    }
    finish(&r)?;
    let bodies = handles.into_iter().map(|id| super::loft_faces::producer(records, classes, id)).collect::<Result<Vec<_>>>()?;
    let mut r = payload(records.primary(tracker)?, tracker)?;
    zeros(&mut r, 6)?;
    let feature = r.u64()?;
    finish(&r)?;
    require_class(classes, feature, "DD405BC2-D673-44F0-8833-5CB2A1C186C7", 8)?;
    Ok((feature, bodies))
}

pub(super) fn owner_tail(r: &mut Reader<'_>, owner: u64) -> Result<()> {
    r.at = r.bytes.len().checked_sub(11).filter(|at| *at >= r.at).ok_or_else(|| invalid("truncated mirror evaluation data"))?;
    if r.reference()? != owner {
        return Err(invalid("inconsistent mirror evaluation back-reference"));
    }
    finish(r)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, common, fixture};
    use crate::f3d::relationship_tests::{reference, refs, string, u32, u64, wide};

    // Original, field-authored relationships with arbitrary IDs and identities.
    // Actual reconstruction is independently checked against local API controls.
    fn authored(suppressed: bool) -> Fixture {
        let mut f = fixture(false);
        let mut timeline = vec![0; 6];
        reference(&mut timeline, 4);
        refs(&mut timeline, &[8, 9, 19]);
        f.0.get_mut(&5).unwrap().2 = timeline;
        let mut b = vec![0; 6];
        reference(&mut b, 910);
        reference(&mut b, 930);
        for outputs in [&[][..], &[950][..], &[], &[]] {
            refs(&mut b, outputs);
        }
        common(&mut b, "Mirror", suppressed);
        f.put(19, CLASS, 0, b, None);
        let mut b = vec![0; 6];
        wide(&mut b, "original-mirror-output");
        b.extend([u8::from(suppressed), 1]);
        reference(&mut b, 951);
        reference(&mut b, 19);
        f.put(950, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2, b, None);
        f.put(951, "D3937028-C20C-4E65-B010-94AD418A5C20", 19, vec![], None);
        let mut b = vec![0; 7];
        reference(&mut b, 914);
        reference(&mut b, 903);
        b.extend([0; 12]);
        for n in [2, 2, u32::MAX] {
            u32(&mut b, n);
        }
        b.push(1);
        u32(&mut b, 1);
        u32(&mut b, 2);
        u32(&mut b, 0);
        u64(&mut b, 8007);
        refs(&mut b, &[911, 912]);
        reference(&mut b, 913);
        reference(&mut b, 19);
        reference(&mut b, 921);
        u32(&mut b, 0);
        f.put(910, HELPER, 1, b, None);
        f.put(914, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0, vec![], None);
        for (id, role) in [(911, 1), (912, 2)] {
            let mut b = vec![0; 6];
            u32(&mut b, role);
            reference(&mut b, 910);
            f.put(id, INSTANCE, 0, b, None);
        }
        let mut b = vec![0; 4];
        b.extend([1, 1, 0, 0, 0]);
        reference(&mut b, 19);
        b.extend([0; 17]);
        reference(&mut b, 19);
        f.put(913, "A085449A-5144-4B2B-B455-7F7035A40559", 1, b, None);
        for (id, object) in [(921, 920), (903, 901)] {
            let mut b = vec![0; 6];
            refs(&mut b, &[object]);
            reference(&mut b, 19);
            f.put(id, "2CA5A1CD-C99B-4C9B-91AF-57989148E841", 1, b, None);
        }
        f.put(920, "9716F783-676D-42E0-93F9-EBF273E7C035", 1, vec![], None);
        let mut b = vec![0; 10];
        u32(&mut b, 1);
        reference(&mut b, 920);
        refs(&mut b, &[]);
        reference(&mut b, 19);
        b.extend([0; 17]);
        reference(&mut b, 19);
        f.put(930, SOURCE, 1, b, None);
        let mut b = vec![0; 6];
        reference(&mut b, 902);
        u32(&mut b, 1);
        wide(&mut b, "original-plane");
        wide(&mut b, "original-plane-owner");
        u32(&mut b, 2);
        b.extend([0; 4]);
        refs(&mut b, &[]);
        b.extend([0; 60]);
        reference(&mut b, 19);
        f.put(901, "5A1BF548-241F-46FD-9FB5-E4B05126EB9D", 1, b, None);
        let mut b = vec![0; 6];
        u64(&mut b, 900);
        f.put(902, "90055C05-546C-4EE7-B3C9-3DD922AD0C9C", 0, b, None);
        f.put(900, super::super::planes::GEOMETRY, 1, vec![], None);
        f
    }

    #[test]
    fn mirror_history_retains_plane_source_and_suppression() {
        for suppressed in [false, true] {
            let history = authored(suppressed).design().history().unwrap();
            let mirror = history.last().unwrap();
            assert_eq!(mirror.name, "Mirror1");
            assert_eq!(mirror.suppressed, suppressed);
            assert_eq!(mirror.kind, HistoryKind::Mirror { plane: 900, source_container: 921, output_count: 1 });
        }
        let mut consumed = authored(false);
        let output = &mut consumed.0.get_mut(&950).unwrap().2;
        let at = output.len() - 24;
        output[at] = 1;
        assert!(!consumed.design().history().unwrap().last().unwrap().suppressed);
        let mut consumed = authored(false);
        consumed.0.get_mut(&950).unwrap().2[at] = 2;
        assert!(consumed.design().history().is_err());
    }

    pub(in crate::f3d) fn authored_feature(suppressed: bool) -> Fixture {
        let mut f = authored(suppressed);
        f.0.get_mut(&910).unwrap().2[45] = 1;
        let mut outputs = Vec::new();
        refs(&mut outputs, &[80]);
        f.0.get_mut(&9).unwrap().2.splice(151..155, outputs);
        let mut b = vec![0; 6];
        wide(&mut b, "original-source-body");
        b.extend([0, 1]);
        reference(&mut b, 81);
        reference(&mut b, 9);
        let mut secondary = vec![0; 4];
        reference(&mut secondary, 4);
        f.put(80, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2, b, Some(secondary));
        f.put(81, "D3937028-C20C-4E65-B010-94AD418A5C20", 19, vec![], None);
        let mut b = vec![0; 6];
        reference(&mut b, 940);
        u32(&mut b, 1);
        wide(&mut b, "original-feature-selection");
        wide(&mut b, "original-feature-owner");
        u32(&mut b, 2);
        b.extend([0; 8]);
        refs(&mut b, &[80]);
        b.extend([0; 56]);
        reference(&mut b, 19);
        f.put(920, "5A1BF548-241F-46FD-9FB5-E4B05126EB9D", 1, b, None);
        let mut b = vec![0; 6];
        u64(&mut b, 9);
        f.put(940, "90055C05-546C-4EE7-B3C9-3DD922AD0C9C", 0, b, None);
        f
    }

    #[test]
    fn feature_mirror_uses_explicit_feature_and_body_links() {
        for suppressed in [false, true] {
            let history = authored_feature(suppressed).design().history().unwrap();
            assert_eq!(history.last().unwrap().kind, HistoryKind::MirrorFeatures { plane: 900, sources: vec![9], targets: vec![], output_count: 1 });
            assert_eq!(history.last().unwrap().suppressed, suppressed);
        }
        for id in [80, 920, 940] {
            for keep in 0..authored_feature(false).0[&id].2.len() {
                let mut f = authored_feature(false);
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(f.design().history().is_err(), "{id}/{keep}");
            }
        }
        for id in [8_u64, 19, 900, 9999] {
            let mut f = authored_feature(false);
            f.0.get_mut(&940).unwrap().2[6..14].copy_from_slice(&id.to_le_bytes());
            assert!(f.design().history().is_err(), "wrong feature {id}");
        }
        // Changing the source's operation without adding its target-body links
        // must not be accepted as a new-body mirror.
        for operation in [1, 2, 3] {
            let mut f = authored_feature(false);
            f.0.get_mut(&9).unwrap().2[9] = operation;
            assert!(f.design().history().is_err(), "operation {operation}");
        }
    }

    pub(in crate::f3d) fn authored_output() -> Fixture {
        let mut f = authored_feature(false);
        f.0.get_mut(&910).unwrap().2[45] = 2; // body selection
        let mut b = vec![0; 6];
        for n in [1, 3, 1, 1] {
            u32(&mut b, n);
        }
        string(&mut b, "8001");
        u32(&mut b, 7);
        b.extend([0; 12]);
        string(&mut b, "body_recipe_data");
        u32(&mut b, 0);
        u32(&mut b, u32::MAX);
        f.put(941, "7ACC2A03-0261-4879-A14A-A93D661A5BDC", 3, b, None);
        let mut b = vec![0; 6];
        u32(&mut b, 1);
        u64(&mut b, 8001);
        u32(&mut b, 33);
        reference(&mut b, 941);
        u32(&mut b, 1);
        wide(&mut b, "original-selector");
        wide(&mut b, "original-source-body");
        u32(&mut b, 2);
        b.extend([0; 8]);
        refs(&mut b, &[80]);
        b.extend([0; 56]);
        reference(&mut b, 19);
        f.put(920, "9716F783-676D-42E0-93F9-EBF273E7C035", 1, b, None);
        f
    }

    #[test]
    fn mirrored_body_groups_require_the_exact_source_output_and_instance() {
        let resolve = |f: Fixture, owners: &BTreeMap<u64, u64>, handle| {
            let d = f.design();
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            output_operations(&Records::new(&d), &classes, owners, 19, handle, 8007)
        };
        let owners = BTreeMap::from([(8001, 9), (8007, 19), (8008, 912)]);
        assert_eq!(resolve(authored_output(), &owners, 950).unwrap(), BTreeSet::from([8001, 8007, 8008]));
        assert!(resolve(authored_output(), &owners, 80).is_err());
        for (tag, owner) in [(8001, 19), (8007, 9), (8008, 911), (8008, 900)] {
            let mut wrong = owners.clone();
            wrong.insert(tag, owner);
            assert!(resolve(authored_output(), &wrong, 950).is_err(), "{tag}/{owner}");
        }
        for id in [950, 912, 920, 941] {
            for keep in 0..authored_output().0[&id].2.len() {
                let mut f = authored_output();
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(resolve(f, &owners, 950).is_err(), "{id}/{keep}");
            }
        }
        let mut wrong = authored_output();
        wrong.0.get_mut(&912).unwrap().2[13..21].copy_from_slice(&999_u64.to_le_bytes());
        assert!(resolve(wrong, &owners, 950).is_err());
    }

    #[test]
    fn versioned_pattern_headers_preserve_feature_mirror_inputs() {
        for version in [6, 8, 9] {
            let mut f = authored_feature(false);
            let h = &mut f.0.get_mut(&910).unwrap().2;
            if version == 6 {
                h.drain(49..53);
                h.drain(33..41);
            } else if version == 8 {
                h.drain(33..37);
            }
            let mut d = f.design();
            d.classes.push(Class {
                id: "3EF91258-FE47-45A5-957D-1295C3679496".into(),
                version,
                parent: String::new(),
                group: String::new(),
                objects: vec![],
            });
            assert!(d.history().is_ok(), "version {version}");
            d.classes.last_mut().unwrap().version = 7;
            assert!(d.history().is_err());
        }
    }

    #[test]
    fn mirror_modes_and_broken_ownership_fail_without_cached_geometry_fallback() {
        for (id, offset, value) in [
            (910, 6, 1),   // combine
            (910, 41, 0),  // compute mode
            (910, 45, 1),  // feature rather than body
            (910, 78, 0),  // broken instance reference
            (910, 112, 9), // helper owner
            (911, 6, 2),   // instance role
            (912, 11, 9),  // instance owner
            (930, 15, 9),  // source disagreement
            (921, 22, 9),  // source container owner
            (913, 10, 9),  // cache owner
            (902, 6, 9),   // plane geometry identity
        ] {
            let mut f = authored(false);
            f.0.get_mut(&id).unwrap().2[offset] = value;
            assert!(f.design().history().is_err(), "{id}/{offset}");
        }
        for id in [19, 901, 902, 903, 910, 911, 912, 913, 921, 930, 950] {
            for keep in 0..authored(false).0[&id].2.len() {
                let mut f = authored(false);
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(f.design().history().is_err(), "{id}/{keep}");
            }
        }
    }
}
