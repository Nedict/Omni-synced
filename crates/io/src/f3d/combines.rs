//! Whole-body Combine inputs verified with operation, retention and suppression controls.
use super::history::{empty, finish, payload, references, require_class, zeros};
use super::*;
use solidcraft_doc::Operation;

pub(super) const CLASS: &str = "2A94257F-2020-4B19-9A68-103A2672F1B7";
const HELPER: &str = "2E3AC990-483D-4A55-8662-C064B976A957";
const CONTAINER: &str = "2CA5A1CD-C99B-4C9B-91AF-57989148E841";

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
        return Err(invalid("unsupported Combine attributes"));
    }
    let operation = match r.u32()? {
        0 => Operation::Join,
        1 => Operation::Cut,
        2 => Operation::Intersect,
        _ => return Err(invalid("unsupported Combine operation")),
    };
    zeros(r, 4)?;
    let keep_tools = match r.take(1)? {
        [0] => false,
        [1] => true,
        _ => return Err(invalid("unsupported Combine tool retention")),
    };
    zeros(r, 1)?;
    let helpers = references(r)?;
    let [helper] = helpers.as_slice() else { return Err(invalid("unsupported Combine helper count")) };
    let helper_version =
        classes.get(helper).filter(|c| c.id == HELPER).map(|c| c.version).ok_or_else(|| invalid("unsupported Combine helper class"))?;
    let mut h = payload(records.primary(*helper)?, *helper)?;
    match helper_version {
        3 => zeros(&mut h, 7)?,
        5 => {
            zeros(&mut h, 24)?;
            if h.u32()? != 3 {
                return Err(invalid("unsupported Combine helper mode"));
            }
            zeros(&mut h, 14)?;
        }
        _ => return Err(invalid("unsupported Combine helper version")),
    }
    if h.take(2)? != [1, 0] || h.reference()? != owner {
        return Err(invalid("inconsistent Combine helper owner"));
    }
    zeros(&mut h, 7)?;
    finish(&h)?;
    let operation_tag = r.u64()?;
    if operation_tag == 0 {
        return Err(invalid("missing Combine operation identity"));
    }
    let tool_containers = references(r)?;
    if !(1..=64).contains(&tool_containers.len()) || tool_containers.iter().collect::<BTreeSet<_>>().len() != tool_containers.len() {
        return Err(invalid("invalid Combine tool containers"));
    }
    for container in &tool_containers {
        require_class(classes, *container, CONTAINER, 1)?;
    }
    let roles = super::history::body_roles(r, classes, body_version)?;
    if roles.iter().take(2).any(|ids| !ids.is_empty()) {
        return Err(invalid("unsupported Combine output body roles"));
    }
    let consumed_handles = roles.get(2).cloned().ok_or_else(|| invalid("missing Combine consumed bodies"))?;
    if consumed_handles.len() > 64
        || consumed_handles.iter().collect::<BTreeSet<_>>().len() != consumed_handles.len()
        || keep_tools != consumed_handles.is_empty()
    {
        return Err(invalid("Combine consumed bodies disagree with tool retention"));
    }
    for handle in &consumed_handles {
        require_class(classes, *handle, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2)?;
    }
    let targets = roles.get(3).ok_or_else(|| invalid("missing Combine target"))?;
    let [target_container] = targets.as_slice() else { return Err(invalid("Combine requires one target container")) };
    require_class(classes, *target_container, CONTAINER, 1)?;
    if tool_containers.contains(target_container) {
        return Err(invalid("Combine target is also a tool"));
    }
    Ok(HistoryKind::Combine { operation, keep_tools, operation_tag, tool_containers, target_container: *target_container, consumed_handles })
}

pub(super) fn objects(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, owner: u64, operation: Option<u64>) -> Result<Vec<u64>> {
    require_class(classes, id, CONTAINER, 1)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 5)?;
    let attributes = super::attributes::read(&mut r)?;
    let expected = operation.map(|tag| BTreeMap::from([("DcFeatureOperationIdFlag".to_owned(), tag)])).unwrap_or_default();
    if attributes != expected {
        return Err(invalid("Combine container operation disagrees with feature"));
    }
    let objects = references(&mut r)?;
    if !(1..=64).contains(&objects.len()) || objects.iter().collect::<BTreeSet<_>>().len() != objects.len() {
        return Err(invalid("invalid Combine body selection"));
    }
    for object in &objects {
        require_class(classes, *object, "9716F783-676D-42E0-93F9-EBF273E7C035", 1)?;
    }
    // Inherited evaluation data cannot select a body. Its final owner must agree.
    r.at = r.bytes.len().checked_sub(11).filter(|at| *at >= r.at).ok_or_else(|| invalid("truncated Combine container"))?;
    if r.reference()? != owner {
        return Err(invalid("Combine container belongs to another feature"));
    }
    finish(&r)?;
    Ok(objects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, common, fixture};
    use crate::f3d::relationship_tests::{reference, refs, string, u32, u64};

    fn authored(operation: u32, keep: bool, suppressed: bool) -> Fixture {
        let mut f = fixture(false);
        let mut timeline = vec![0; 6];
        reference(&mut timeline, 4);
        refs(&mut timeline, &[8, 9, 19]);
        f.0.get_mut(&5).unwrap().2 = timeline;
        let mut b = vec![0; 6];
        u32(&mut b, operation);
        b.extend([0; 4]);
        b.extend([u8::from(keep), 0]);
        refs(&mut b, &[900]);
        u64(&mut b, 7000);
        refs(&mut b, &[901]);
        for ids in [&[][..], &[], if keep { &[] } else { &[903] }, &[902]] {
            refs(&mut b, ids);
        }
        common(&mut b, "Combine", suppressed);
        f.put(19, CLASS, 1, b, None);
        let mut b = vec![0; 24];
        u32(&mut b, 3);
        b.extend([0; 14]);
        b.extend([1, 0]);
        reference(&mut b, 19);
        b.extend([0; 7]);
        f.put(900, HELPER, 5, b, None);
        for id in [901, 902] {
            f.put(id, CONTAINER, 1, vec![], None);
        }
        f.put(903, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2, vec![], None);
        f
    }

    #[test]
    fn combine_modes_retention_and_suppression_are_independent() {
        for (code, operation) in [(0, Operation::Join), (1, Operation::Cut), (2, Operation::Intersect)] {
            for keep in [false, true] {
                for suppressed in [false, true] {
                    let history = authored(code, keep, suppressed).design().history().unwrap();
                    let feature = history.last().unwrap();
                    assert_eq!(feature.suppressed, suppressed);
                    assert_eq!(
                        feature.kind,
                        HistoryKind::Combine {
                            operation,
                            keep_tools: keep,
                            operation_tag: 7000,
                            tool_containers: vec![901],
                            target_container: 902,
                            consumed_handles: if keep { vec![] } else { vec![903] },
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn legacy_helpers_and_empty_optional_metadata_preserve_combine_inputs() {
        for version in [3, 5] {
            for metadata in [false, true] {
                let mut f = authored(0, false, false);
                if version == 3 {
                    let helper = f.0.get_mut(&900).unwrap();
                    helper.1 = 3;
                    helper.2.drain(7..42);
                }
                if metadata {
                    let record = &mut f.0.get_mut(&19).unwrap().2;
                    record[5] = 1;
                    record.splice(6..6, [0; 4]);
                }
                assert_eq!(f.design().history().unwrap(), authored(0, false, false).design().history().unwrap());
                let mut f = authored(0, false, false);
                f.0.get_mut(&900).unwrap().1 = 4;
                assert!(f.design().history().is_err());
            }
        }
        for keep in 0..27 {
            let mut f = authored(0, false, false);
            let helper = f.0.get_mut(&900).unwrap();
            helper.1 = 3;
            helper.2.drain(7..42);
            helper.2.truncate(keep);
            assert!(f.design().history().is_err(), "legacy helper truncated at {keep}");
        }
    }

    #[test]
    fn damaged_combine_inputs_reject_unknown_modes_and_broken_ownership() {
        for id in [19, 900] {
            for keep in 0..authored(0, false, false).0[&id].2.len() {
                let mut f = authored(0, false, false);
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(f.design().history().is_err(), "{id}/{keep}");
            }
        }
        for (id, at, value) in [(19, 6, 3), (19, 14, 2), (19, 14, 1), (19, 15, 1), (19, 21, 0), (19, 44, 0), (900, 24, 2), (900, 45, 9)] {
            let mut f = authored(0, false, false);
            f.0.get_mut(&id).unwrap().2[at] = value;
            assert!(f.design().history().is_err(), "{id}/{at}/{value}");
        }
    }

    #[test]
    fn containers_require_exact_operation_and_owner_and_unique_sources() {
        for tag in [None, Some(7000)] {
            for ids in [vec![904], vec![904, 904], vec![], vec![9999]] {
                let mut f = authored(0, false, false);
                f.put(904, "9716F783-676D-42E0-93F9-EBF273E7C035", 1, vec![], None);
                let mut b = vec![0; 5];
                b.push(u8::from(tag.is_some()));
                if let Some(tag) = tag {
                    u32(&mut b, 1);
                    string(&mut b, "DcFeatureOperationIdFlag");
                    string(&mut b, "IntrinsicMetaTypeuint64");
                    u64(&mut b, tag);
                }
                refs(&mut b, &ids);
                reference(&mut b, 19);
                f.0.get_mut(&901).unwrap().2 = b;
                let d = f.design();
                let records = Records::new(&d);
                let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
                assert_eq!(objects(&records, &classes, 901, 19, tag).is_ok(), ids == [904]);
                assert!(objects(&records, &classes, 901, 9, tag).is_err());
                assert!(objects(&records, &classes, 901, 19, Some(7001)).is_err());
            }
        }
    }
}
