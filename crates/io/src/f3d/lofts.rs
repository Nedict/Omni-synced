//! Ordered free sketch sections, observed using independent solid/surface,
//! merge, reverse-order and suppression controls. Other modes fail explicitly.
use super::history::{empty, finish, operation, payload, references, require_class, zeros};
use super::*;

pub(super) const CLASS: &str = "B6678572-C095-4DD1-9348-614677DD0B4A";
const SECTION: &str = "2F3200BA-FDFB-48C6-97D4-71A4A0D86190";

pub(super) fn inputs(
    r: &mut Reader<'_>,
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    feature: u64,
) -> Result<(solidcraft_doc::Operation, Vec<u64>)> {
    empty(r)?;
    zeros(r, 1)?;
    if !super::attributes::read(r)?.is_empty() {
        return Err(invalid("unsupported loft attributes"));
    }
    if r.take(4)? != [1, 1, 1, 1] {
        return Err(invalid("unsupported loft flags or unmerged tangent edges"));
    }
    let operation = operation(r.u32()?)?;
    zeros(r, 1)?;
    if r.u32()? != u32::MAX {
        return Err(invalid("unsupported loft connection mode"));
    }
    zeros(r, 19)?;
    if r.take(1)? != [1] {
        return Err(invalid("surface loft history is not supported yet"));
    }
    zeros(r, 4)?;
    let helper = r.reference()?;
    require_class(classes, helper, "B0387387-0FAA-4928-B189-FEB34071D524", 0)?;
    let bytes = records.primary(helper)?;
    let mut helper_record = payload(bytes, helper)?;
    empty(&mut helper_record)?;
    zeros(&mut helper_record, 2)?;
    if helper_record.reference()? != feature {
        return Err(invalid("inconsistent loft evaluation owner"));
    }
    // Evaluation/cache data is opaque. The final owner is independently stored.
    helper_record.at = bytes.len().checked_sub(11).filter(|at| *at >= helper_record.at).ok_or_else(|| invalid("truncated loft evaluation"))?;
    if helper_record.reference()? != feature {
        return Err(invalid("inconsistent loft evaluation back-reference"));
    }
    finish(&helper_record)?;
    let sections = references(r)?;
    if !(2..=50).contains(&sections.len()) {
        return Err(invalid("loft requires 2..50 ordered sections"));
    }
    let mut seen = BTreeSet::new();
    let mut containers = Vec::new();
    for section in sections {
        require_class(classes, section, SECTION, 1)?;
        if !seen.insert(section) {
            return Err(invalid("duplicate loft section"));
        }
        let mut s = payload(records.primary(section)?, section)?;
        // Neutral end conditions only; never discard tangency/curvature inputs.
        zeros(&mut s, 14)?;
        if s.reference()? != feature {
            return Err(invalid("inconsistent loft section owner"));
        }
        let container = s.reference()?;
        require_class(classes, container, "2CA5A1CD-C99B-4C9B-91AF-57989148E841", 1)?;
        let kind = s.u32()?;
        let objects = super::loft_faces::objects(records, classes, container, feature)?;
        let [object] = objects.as_slice() else {
            return Err(invalid("loft section requires one source"));
        };
        match kind {
            2 => require_class(classes, *object, "4BD53E5A-0B3E-45E5-AE8B-02044306485A", 0)?,
            3 => {
                super::loft_faces::selector_version(classes, *object)?;
            }
            _ => return Err(invalid("loft point sections are not supported yet")),
        }
        finish(&s)?;
        containers.push(container);
    }
    if r.u32()? != u32::MAX {
        return Err(invalid("unsupported loft connection tail"));
    }
    zeros(r, 14)?;
    Ok((operation, containers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, common, volume};
    use crate::f3d::relationship_tests::{reference, refs, u32, wide};

    fn fixture(suppressed: bool) -> Fixture {
        let mut f = crate::f3d::projection_tests::projected_circle();
        for id in 63..=67 {
            f.0.remove(&id);
        }
        // The upper section projects the driving lower circle onto a plane
        // 20 mm higher. Changing its source diameter must update both sections.
        let mut transform = vec![0; 7];
        for n in [1_f64, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 2., 0., 0., 0., 1.] {
            transform.extend(n.to_le_bytes());
        }
        transform.push(0);
        reference(&mut transform, 3);
        f.put(160, "F47A46FB-DA27-4AFA-8A72-1B38FA596E23", 0, transform, None);
        let mut link = Vec::new();
        reference(&mut link, 160);
        f.0.get_mut(&170).unwrap().3.as_mut().unwrap()[4..15].copy_from_slice(&link);
        let mut profile = vec![0; 6];
        reference(&mut profile, 111);
        u32(&mut profile, 1);
        wide(&mut profile, "original-loft-section");
        wide(&mut profile, "70");
        profile.extend([0; 20]);
        reference(&mut profile, 9);
        f.put(110, "4BD53E5A-0B3E-45E5-AE8B-02044306485A", 0, profile, None);
        let mut tracking = f.0[&101].2.clone();
        let mut link = Vec::new();
        reference(&mut link, 110);
        tracking[6..17].copy_from_slice(&link);
        f.put(111, "0D57BD2F-D09B-43FC-AD57-1E89A118C453", 1, tracking, None);
        let mut container = vec![0; 6];
        refs(&mut container, &[110]);
        container.extend([0; 20]);
        reference(&mut container, 9);
        f.put(112, "2CA5A1CD-C99B-4C9B-91AF-57989148E841", 1, container, None);
        for (id, container) in [(221, 112), (220, 102)] {
            let mut b = vec![0; 14];
            reference(&mut b, 9);
            reference(&mut b, container);
            u32(&mut b, 2);
            f.put(id, SECTION, 1, b, None);
        }
        let mut helper = vec![0; 6];
        reference(&mut helper, 9);
        helper.extend([0; 20]);
        reference(&mut helper, 9);
        f.put(210, "B0387387-0FAA-4928-B189-FEB34071D524", 0, helper, None);
        let mut b = vec![0; 6];
        b.extend([1; 4]);
        u32(&mut b, 4);
        b.push(0);
        u32(&mut b, u32::MAX);
        b.extend([0; 19]);
        b.push(1);
        b.extend([0; 4]);
        reference(&mut b, 210);
        refs(&mut b, &[221, 220]);
        u32(&mut b, u32::MAX);
        b.extend([0; 14]);
        for _ in 0..4 {
            refs(&mut b, &[]);
        }
        common(&mut b, "Loft", suppressed);
        f.put(9, CLASS, 9, b, None);
        f
    }

    #[test]
    fn loft_preserves_section_order_source_edits_suppression_and_reopen() {
        let d = fixture(false).design();
        assert!(matches!(&d.history().unwrap()[2].kind, HistoryKind::Loft { section_containers, .. } if section_containers == &[112, 102]));
        let mut doc = d.to_document("Original loft").unwrap();
        volume(&doc, 18_000. * std::f64::consts::PI);
        doc.change_param("d7", "40 mm", None, None).unwrap();
        volume(&doc, 8_000. * std::f64::consts::PI);
        let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, 8_000. * std::f64::consts::PI);
        volume(&fixture(true).design().to_document("Suppressed loft").unwrap(), 0.);
    }

    #[test]
    fn malformed_loft_modes_owners_and_truncated_sections_are_rejected() {
        for id in [9, 210, 220, 221] {
            for keep in 0..fixture(false).0[&id].2.len() {
                // The helper's cache is deliberately opaque; only interpreted
                // prefixes and the owner suffix must survive truncation.
                let mut f = fixture(false);
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(f.design().history().is_err(), "{id}/{keep}");
            }
        }
        for (id, at, byte) in [(9, 9, 0), (9, 38, 0), (220, 0, 1), (220, 15, 8), (220, 26, 99), (220, 36, 3), (210, 7, 8)] {
            let mut f = fixture(false);
            f.0.get_mut(&id).unwrap().2[at] = byte;
            assert!(f.design().history().is_err(), "{id}/{at}");
        }
        let mut f = fixture(false);
        let link = f.0[&9].2[58..69].to_vec();
        f.0.get_mut(&9).unwrap().2[69..80].copy_from_slice(&link);
        assert!(f.design().history().is_err());
    }
}
