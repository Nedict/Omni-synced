//! Whole-body labels are explicit feature-owned objects, not profile sizes.
use super::history::{finish, payload, require_class, zeros};
use super::*;

const BODY: &str = "ABA79BE7-D8D2-4C17-B59B-C2035E111F77";

pub(super) fn validate(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, operation: u64, label: u64) -> Result<()> {
    if operation == 0 {
        return Err(invalid("empty whole-body operation"));
    }
    validate_groups(records, classes, feature, label, None, &BTreeSet::from([operation]))
}

pub(super) fn validate_mirror(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    feature: u64,
    label: u64,
    handle: u64,
    operations: &BTreeSet<u64>,
) -> Result<()> {
    require_class(classes, feature, super::mirrors::CLASS, 0)?;
    require_class(classes, handle, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2)?;
    let mut r = payload(records.primary(handle)?, handle)?;
    zeros(&mut r, 6)?;
    let identity = r.wide_string()?;
    if operations.len() != 3 || operations.contains(&0) {
        return Err(invalid("unsupported mirror body operation groups"));
    }
    validate_groups(records, classes, feature, label, Some(&identity), operations)
}

fn validate_groups(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    feature: u64,
    label: u64,
    identity: Option<&str>,
    operations: &BTreeSet<u64>,
) -> Result<()> {
    let (_, actual) = resolve_groups(records, classes, feature, label, identity)?;
    if &actual != operations {
        return Err(invalid("whole-body label belongs to another operation"));
    }
    Ok(())
}

/// Historical selector aliases must explicitly converge to the same body.
/// Merely sharing an operation or having similar geometry is insufficient.
pub(super) fn validate_aliases(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64, label: u64, aliases: &[u32]) -> Result<()> {
    if aliases.is_empty() {
        return Ok(());
    }
    let selected = resolve_groups(records, classes, feature, label, None)?;
    for alias in aliases {
        if resolve_groups(records, classes, feature, u64::from(*alias), None)? != selected {
            return Err(invalid("whole-body recipe aliases select different bodies"));
        }
    }
    Ok(())
}

pub(super) fn output_groups(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    feature: u64,
    label: u64,
    handle: u64,
) -> Result<(u64, BTreeSet<u64>)> {
    require_class(classes, handle, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2)?;
    let mut r = payload(records.primary(handle)?, handle)?;
    zeros(&mut r, 6)?;
    let identity = r.wide_string()?;
    resolve_groups(records, classes, feature, label, Some(&identity))
}

pub(super) fn resolve_groups(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    feature: u64,
    label: u64,
    identity: Option<&str>,
) -> Result<(u64, BTreeSet<u64>)> {
    let labels = super::side_roles::label_table(records, classes, feature)?;
    let mut id = *labels.get(&label).ok_or_else(|| invalid("missing whole-body feature label"))?;
    let owned: BTreeSet<_> = labels.values().copied().collect();
    let mut visited = BTreeSet::new();
    let mut operations: Option<BTreeSet<u64>> = None;
    let pattern = classes.get(&feature).is_some_and(|c| c.id == super::circular_patterns::CLASS && c.version == 0);
    loop {
        if visited.len() >= 64 || !visited.insert(id) {
            return Err(invalid("cyclic or excessive whole-body label replacements"));
        }
        // Editing an extrusion's selected profile can replace its body label.
        // The old selector retains its identity; both records must still belong
        // to this feature and exactly the same operation group.
        let expected = if visited.len() == 1 { identity } else { None };
        let (replacement, actual) = read_record(records, classes, id, expected, pattern)?;
        if operations.as_ref().is_some_and(|ops| if pattern { !ops.is_subset(&actual) } else { ops != &actual }) {
            return Err(invalid("whole-body replacement changes operation groups"));
        }
        operations = Some(actual.clone());
        let Some(next) = replacement else { return Ok((id, actual)) };
        if !owned.contains(&next) {
            return Err(invalid("whole-body replacement belongs to another feature"));
        }
        id = next;
    }
}

fn read_record(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    id: u64,
    identity: Option<&str>,
    pattern: bool,
) -> Result<(Option<u64>, BTreeSet<u64>)> {
    require_class(classes, id, BODY, 4)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    let replacement = match r.bytes.get(r.at) {
        Some(0) => {
            r.take(1)?;
            None
        }
        Some(1) => Some(r.reference()?),
        _ => return Err(invalid("unsupported whole-body replacement reference")),
    };
    let actual = r.wide_string()?;
    if actual.is_empty() || identity.is_some_and(|identity| identity != actual) {
        return Err(invalid("empty or inconsistent whole-body identity"));
    }
    zeros(&mut r, 4)?;
    let count = r.u32()?;
    if !(1..=64).contains(&count) {
        return Err(invalid("unsupported whole-body label group"));
    }
    let mut seen = BTreeSet::new();
    for _ in 0..count {
        names(&mut r, records.design.layout, pattern)?;
        let operation = r.u64()?;
        if operation == 0 || !seen.insert(operation) {
            return Err(invalid("whole-body label belongs to another operation"));
        }
        zeros(&mut r, 1)?;
        if r.u32()? != 1 {
            return Err(invalid("unsupported whole-body recipe count"));
        }
    }
    match r.bytes.get(r.at) {
        Some(0) => {
            r.take(1)?;
        }
        Some(1) => {
            require_class(classes, r.reference()?, "7ACC2A03-0261-4879-A14A-A93D661A5BDC", 3)?;
        }
        _ => return Err(invalid("unsupported whole-body recipe reference")),
    }
    zeros(&mut r, 1)?;
    finish(&r)?;
    Ok((replacement, seen))
}

// These cached member labels are consumed and checked structurally. They never
// choose geometry: the feature table, body-label class and operation own the link.
fn names(r: &mut Reader<'_>, layout: u32, repeated: bool) -> Result<()> {
    let values = match layout {
        4 | 5 => {
            let n = r.count(4)?;
            if !(1..=1024).contains(&n) {
                return Err(invalid("whole-body member count exceeds budget"));
            }
            (0..n).map(|_| r.string()).collect::<Result<Vec<_>>>()?
        }
        8 => {
            let text = r.string()?;
            if text.len() > 20 * 1024 {
                return Err(invalid("whole-body member text exceeds budget"));
            }
            let n = r.count(4)?;
            if !(2..=1025).contains(&n) || r.u32()? != 0 {
                return Err(invalid("invalid whole-body member offsets"));
            }
            let mut previous = 0;
            let mut values = Vec::new();
            for _ in 1..n {
                let end = r.u32()? as usize;
                let value = text.get(previous..end).filter(|s| !s.is_empty()).ok_or_else(|| invalid("invalid whole-body member slice"))?;
                values.push(value.to_owned());
                previous = end;
            }
            if previous != text.len() {
                return Err(invalid("whole-body member offsets omit text"));
            }
            values
        }
        _ => return Err(invalid("unsupported whole-body label layout")),
    };
    let mut unique = BTreeSet::new();
    for value in values {
        let id =
            value.parse::<u64>().ok().filter(|id| *id != 0 && id.to_string() == value).ok_or_else(|| invalid("invalid whole-body member label"))?;
        if !unique.insert(id) && !repeated {
            return Err(invalid("duplicate whole-body member label"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, fixture};
    use crate::f3d::relationship_tests::{reference, string, u32, u64, wide};

    pub(in crate::f3d) fn authored(legacy: bool) -> Fixture {
        let mut f = fixture(false);
        let bytes = &mut f.0.get_mut(&9).unwrap().2;
        let at = bytes.len() - 59;
        let mut slot = Vec::new();
        reference(&mut slot, 901);
        bytes[at..at + 11].copy_from_slice(&slot);
        if legacy {
            bytes.remove(bytes.len() - 15);
        }
        let mut table = vec![0; 6];
        u32(&mut table, 1);
        reference(&mut table, 902);
        u64(&mut table, 208);
        u64(&mut table, 900);
        f.put(901, "B7F34D7B-DAC4-41A5-B67D-D672537C3BFA", 2, table, Some(vec![0; 5]));
        let mut body = vec![0; 7];
        wide(&mut body, "independent-body-label");
        u32(&mut body, 0);
        u32(&mut body, 1);
        if legacy {
            u32(&mut body, 2);
            string(&mut body, "19");
            string(&mut body, "3");
        } else {
            string(&mut body, "193");
            u32(&mut body, 3);
            for n in [0, 2, 3] {
                u32(&mut body, n);
            }
        }
        u64(&mut body, 8001);
        body.push(0);
        u32(&mut body, 1);
        body.extend([0; 2]);
        f.put(902, BODY, 4, body, None);
        f
    }

    fn resolve(f: Fixture, legacy: bool, operation: u64, label: u64) -> Result<()> {
        resolve_layout(f, if legacy { 4 } else { 8 }, operation, label)
    }

    fn resolve_layout(f: Fixture, layout: u32, operation: u64, label: u64) -> Result<()> {
        let mut d = f.design();
        d.layout = layout;
        if layout <= 5 {
            d.classes.push(Class {
                id: "A0A15D26-1F3B-4120-A3F1-9CDDA189AB74".into(),
                parent: String::new(),
                version: 1,
                group: String::new(),
                objects: Vec::new(),
            });
        }
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        validate(&Records::new(&d), &classes, 9, operation, label)
    }

    #[test]
    fn whole_body_roles_use_explicit_labels_and_operation_ownership() {
        for legacy in [false, true] {
            resolve(authored(legacy), legacy, 8001, 208).unwrap();
            assert!(resolve(authored(legacy), legacy, 8002, 208).is_err());
            assert!(resolve(authored(legacy), legacy, 8001, 4).is_err(), "circle profile size is not a body label");
            let mut wrong_class = authored(legacy);
            wrong_class.0.get_mut(&902).unwrap().0 = "335CC7BD-BBE5-40AA-9E8C-5EC99D427A3A".into();
            assert!(resolve(wrong_class, legacy, 8001, 208).is_err());
            for keep in 0..authored(legacy).0[&902].2.len() {
                let mut f = authored(legacy);
                f.0.get_mut(&902).unwrap().2.truncate(keep);
                assert!(resolve(f, legacy, 8001, 208).is_err(), "{legacy}/{keep}");
            }
        }
    }

    fn replacement(f: &mut Fixture, from: u64, to: u64) {
        let mut link = vec![0; 6];
        reference(&mut link, to);
        f.0.get_mut(&from).unwrap().2.splice(..7, link);
    }

    fn with_replacement(legacy: bool) -> Fixture {
        let mut f = authored(legacy);
        let body = f.0[&902].2.clone();
        f.put(903, BODY, 4, body, None);
        let table = &mut f.0.get_mut(&901).unwrap().2;
        table[6..10].copy_from_slice(&2_u32.to_le_bytes());
        let mut entry = Vec::new();
        reference(&mut entry, 903);
        u64(&mut entry, 209);
        table.splice(table.len() - 8..table.len() - 8, entry);
        replacement(&mut f, 902, 903);
        f
    }

    #[test]
    fn replaced_body_labels_require_same_feature_operation_and_acyclic_links() {
        resolve_layout(with_replacement(true), 5, 8001, 208).unwrap();
        for legacy in [false, true] {
            resolve(with_replacement(legacy), legacy, 8001, 208).unwrap();
            let mut cycle = with_replacement(legacy);
            replacement(&mut cycle, 903, 902);
            assert!(resolve(cycle, legacy, 8001, 208).is_err());
            let mut self_link = authored(legacy);
            replacement(&mut self_link, 902, 902);
            assert!(resolve(self_link, legacy, 8001, 208).is_err());
            let mut foreign = with_replacement(legacy);
            let bytes = foreign.0[&903].2.clone();
            foreign.put(904, BODY, 4, bytes, None);
            replacement(&mut foreign, 902, 904);
            assert!(resolve(foreign, legacy, 8001, 208).is_err());
            let mut wrong_operation = with_replacement(legacy);
            let bytes = &mut wrong_operation.0.get_mut(&903).unwrap().2;
            let at = bytes.windows(8).position(|b| b == 8001_u64.to_le_bytes()).unwrap();
            bytes[at..at + 8].copy_from_slice(&8002_u64.to_le_bytes());
            assert!(resolve(wrong_operation, legacy, 8001, 208).is_err());
            let mut wrong_class = with_replacement(legacy);
            wrong_class.0.get_mut(&903).unwrap().0 = "335CC7BD-BBE5-40AA-9E8C-5EC99D427A3A".into();
            assert!(resolve(wrong_class, legacy, 8001, 208).is_err());
            for id in [902, 903] {
                for keep in 0..with_replacement(legacy).0[&id].2.len() {
                    let mut f = with_replacement(legacy);
                    f.0.get_mut(&id).unwrap().2.truncate(keep);
                    assert!(resolve(f, legacy, 8001, 208).is_err(), "{legacy}/{id}/{keep}");
                }
            }
        }
    }

    #[test]
    fn body_recipe_aliases_must_converge_on_one_terminal_record() {
        let check = |f: Fixture, aliases: &[u32]| {
            let d = f.design();
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            validate_aliases(&Records::new(&d), &classes, 9, 208, aliases)
        };
        check(with_replacement(false), &[209]).unwrap();
        assert!(check(with_replacement(false), &[210]).is_err());
        let mut independent = with_replacement(false);
        // Identical operation groups and geometry labels do not establish identity.
        independent.0.get_mut(&902).unwrap().2.splice(..17, [0; 7]);
        assert!(check(independent, &[209]).is_err());
        let mut cycle = with_replacement(false);
        replacement(&mut cycle, 903, 902);
        assert!(check(cycle, &[209]).is_err());
    }

    #[test]
    fn whole_body_replacement_depth_is_bounded() {
        for count in [64, 65] {
            let mut f = authored(false);
            let body = f.0[&902].2.clone();
            let mut table = vec![0; 6];
            u32(&mut table, count);
            for i in 0..count {
                let id = 902 + u64::from(i);
                f.put(id, BODY, 4, body.clone(), None);
                reference(&mut table, id);
                u64(&mut table, 208 + u64::from(i));
                if i > 0 {
                    replacement(&mut f, id - 1, id);
                }
            }
            u64(&mut table, 900);
            f.0.get_mut(&901).unwrap().2 = table;
            assert_eq!(resolve(f, false, 8001, 208).is_ok(), count == 64);
        }
    }

    #[test]
    fn packed_member_labels_reject_invalid_offsets_duplicates_and_budgets() {
        let parse = |text: &str, offsets: &[u32]| {
            let mut bytes = Vec::new();
            string(&mut bytes, text);
            u32(&mut bytes, offsets.len() as u32);
            for n in offsets {
                u32(&mut bytes, *n);
            }
            names(&mut Reader { bytes: &bytes, at: 0 }, 8, false)
        };
        assert!(parse("193", &[0, 2, 3]).is_ok());
        for (text, offsets) in [
            ("193", vec![1, 2, 3]),
            ("193", vec![0, 2, 4]),
            ("193", vec![0, 2]),
            ("193", vec![0, 2, 1, 3]),
            ("193", vec![0, 2, 2, 3]),
            ("1919", vec![0, 2, 4]),
            ("019", vec![0, 3]),
            ("0", vec![0, 1]),
            ("x", vec![0, 1]),
        ] {
            assert!(parse(text, &offsets).is_err(), "{text}/{offsets:?}");
        }
        assert!(parse("1", &vec![0; 1026]).is_err());
    }

    #[test]
    fn mirror_label_groups_keep_exact_body_identity_and_operation_membership() {
        let fixture = || {
            let mut f = authored(false);
            f.0.get_mut(&9).unwrap().0 = super::super::mirrors::CLASS.into();
            f.0.get_mut(&9).unwrap().1 = 0;
            f.0.get_mut(&901).unwrap().2[21..29].copy_from_slice(&1_u64.to_le_bytes());
            let mut body = vec![0; 7];
            wide(&mut body, "independent-mirrored-body");
            u32(&mut body, 0);
            u32(&mut body, 3);
            for operation in [8001, 9001, 9002] {
                string(&mut body, "193");
                u32(&mut body, 3);
                for end in [0, 2, 3] {
                    u32(&mut body, end);
                }
                u64(&mut body, operation);
                body.push(0);
                u32(&mut body, 1);
            }
            body.extend([0; 2]);
            f.0.get_mut(&902).unwrap().2 = body;
            let mut handle = vec![0; 6];
            wide(&mut handle, "independent-mirrored-body");
            f.put(903, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2, handle, None);
            f
        };
        let resolve = |f: Fixture, groups| {
            let d = f.design();
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            validate_mirror(&Records::new(&d), &classes, 9, 1, 903, &groups)
        };
        let groups = BTreeSet::from([8001, 9001, 9002]);
        resolve(fixture(), groups.clone()).unwrap();
        for groups in [BTreeSet::from([8001]), BTreeSet::from([8001, 9001, 9003]), BTreeSet::from([0, 9001, 9002])] {
            assert!(resolve(fixture(), groups).is_err());
        }
        for id in [902, 903] {
            for keep in 0..fixture().0[&id].2.len() {
                let mut f = fixture();
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(resolve(f, groups.clone()).is_err(), "{id}/{keep}");
            }
        }
        let mut wrong = fixture();
        wrong.0.get_mut(&903).unwrap().2[10] = b'x';
        assert!(resolve(wrong, groups.clone()).is_err());
        let mut duplicate = fixture();
        let bytes = &mut duplicate.0.get_mut(&902).unwrap().2;
        let at = bytes.windows(8).position(|v| v == 9002_u64.to_le_bytes()).unwrap();
        bytes[at..at + 8].copy_from_slice(&9001_u64.to_le_bytes());
        assert!(resolve(duplicate, groups).is_err());
    }

    #[test]
    #[ignore = "requires local original adapter and unchanged public API resave"]
    fn original_adapter_extrusion_and_loft_targets_own_each_body_label() {
        let corpus = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_CORPUS").unwrap());
        let probes = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
        let mut results = Vec::new();
        for path in [corpus.join("adapter-x20-makita.f3d"), probes.join("corpus-adapter-x20-makita.f3d")] {
            let designs = read_f3d(&std::fs::read(&path).unwrap()).unwrap();
            let d = &designs[0];
            let records = Records::new(d);
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            let owners = d.operation_owners().unwrap();
            let mut targets = Vec::new();
            for (container, owner, count) in [(627, 626, 2), (696, 695, 3)] {
                let target = super::super::loft_faces::target(&records, &classes, container, owner).unwrap();
                assert_eq!(target.feature, 506);
                assert_eq!(target.operations.len(), count);
                for operation in &target.operations {
                    validate(&records, &classes, owners[&operation.operation], operation.operation, u64::from(operation.label)).unwrap();
                }
                targets.push(target);
            }
            results.push(targets);
        }
        assert_eq!(results[0], results[1]);
    }
}
