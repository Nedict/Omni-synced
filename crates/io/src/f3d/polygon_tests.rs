use super::import_tests::{Fixture, common, fixture, volume};
use super::relationship_tests::{reference, refs, string, u32, u64, wide};

/// An original 30 x 20 x 17 mm rectangle. No exported native bytes are reused.
pub(super) fn rectangle() -> Fixture {
    rectangle_with_dimensions(false, false)
}

fn rectangle_with_dimensions(aligned: bool, reversed: bool) -> Fixture {
    let mut f = fixture(false);
    let depth = if aligned { 4. } else { 2. };
    for (id, connection, x, y, origin) in [(42, 44, 0_f64, 0., true), (41, 43, 3., 0., false), (71, 73, 3., depth, false), (72, 74, 0., depth, false)]
    {
        let mut b = vec![0; 4];
        b.extend([0, 1]);
        u32(&mut b, 0);
        reference(&mut b, connection);
        b.extend([0; 3]);
        b.extend([u8::from(origin), 1, u8::from(origin), 0, 0]);
        for n in [x, y, 0.] {
            b.extend(n.to_le_bytes());
        }
        b.extend([0; 30]);
        refs(&mut b, &[connection]);
        reference(&mut b, 70);
        f.put(id, "C2CEDAE7-1716-47C1-B7B1-07B70081D0FB", 11, b, None);
        f.put(connection, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0, vec![], None);
    }
    for (id, tag, start, end, position, delta) in [
        (31, 17, 42, 41, [0_f64, 0., 0.], [3_f64, 0., 0.]),
        (32, 18, 41, 71, [3., 0., 0.], [0., depth, 0.]),
        (33, 19, 71, 72, [3., depth, 0.], [-3., 0., 0.]),
        (34, 20, 72, 42, [0., depth, 0.], [0., -depth, 0.]),
    ] {
        let mut b = vec![0; 4];
        b.extend([0, 1]);
        u32(&mut b, 2);
        for (key, value) in [("crv_primary_id", tag), ("crv_secondary_id", 2)] {
            string(&mut b, key);
            string(&mut b, "IntrinsicMetaTypeuint64");
            u64(&mut b, value);
        }
        let len = delta.iter().map(|x| x * x).sum::<f64>().sqrt();
        for n in position.into_iter().chain(delta).chain(delta.map(|x| x / len)).chain([0., 0., 1.]) {
            b.extend(n.to_le_bytes());
        }
        reference(&mut b, end);
        reference(&mut b, start);
        b.extend([0; 42]);
        reference(&mut b, 70);
        f.put(id, "DCA267ED-D615-4934-B64F-AD805E8003E2", 2, b, None);
    }
    for (id, line, kind) in [(55, 31, 64), (58, 33, 64), (59, 32, 128), (60, 34, 128)] {
        let mut b = vec![0; 4];
        b.push(1);
        u32(&mut b, 1);
        reference(&mut b, line);
        u32(&mut b, 0);
        b.push(1);
        u32(&mut b, 0);
        reference(&mut b, 70);
        u64(&mut b, kind);
        refs(&mut b, &[line]);
        b.push(0);
        f.put(id, "60403D47-0C49-49B0-BDE8-1679608164A2", 3, b, None);
    }
    for (id, param, binding, a, b, vertical, name, expr, value) in
        [(56, 62, 61, 42, 41, false, "width", "30 mm", 3_f64), (57, 76, 75, 41, 71, true, "heightSketch", "20 mm", 2.)]
    {
        let diagonal = aligned && vertical;
        let (a, expr, value) = if diagonal { (42, "50 mm", 5.) } else { (a, expr, value) };
        let mut data = vec![0; 4];
        data.push(1);
        u32(&mut data, 3);
        reference(&mut data, 0);
        u32(&mut data, if vertical { 5 } else { 4 });
        reference(&mut data, a);
        u32(&mut data, u32::from(vertical));
        reference(&mut data, b);
        u32(&mut data, 0);
        data.push(1);
        u32(&mut data, 0);
        data.extend([0; 220]);
        wide(&mut data, "fx: ");
        wide(&mut data, expr);
        data.extend([0; 44]);
        u64(
            &mut data,
            if diagonal {
                0x4000
            } else if vertical {
                0x800000
            } else {
                0x400000
            },
        );
        reference(&mut data, binding);
        refs(&mut data, &if diagonal && reversed { [b, a] } else { [a, b] });
        data.push(0);
        let mut secondary = vec![0; 4];
        reference(&mut secondary, 70);
        secondary.extend([0; 18]);
        reference(&mut secondary, 70);
        f.put(id, "8C780195-72C0-4a56-A911-E43AB14357F2", 3, data, Some(secondary));
        let mut data = vec![0; 20];
        u32(&mut data, 0);
        reference(&mut data, binding);
        wide(&mut data, expr);
        data.extend([0; 10]);
        for text in ["Linear Dimension-2", "", "mm", name] {
            wide(&mut data, text);
        }
        data.extend(value.to_le_bytes());
        data.extend([0; 12]);
        f.put(param, "7A6A3D31-BE74-4E19-9115-20642944C2E4", 9, data, None);
        let mut data = vec![0; 4];
        data.push(1);
        refs(&mut data, &[8]);
        data.extend([0; 5]);
        data.extend(value.to_le_bytes());
        reference(&mut data, param);
        f.put(binding, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0, data, None);
    }
    let mut b = vec![0; 7];
    refs(&mut b, &[61, 75]);
    u32(&mut b, 1);
    b.push(0);
    reference(&mut b, 70);
    reference(&mut b, 6);
    b.extend([0; 12]);
    common(&mut b, "Sketch", false);
    f.put(8, "8DA771B7-52ED-42FC-94E2-2353FE141373", 5, b, None);
    let mut b = vec![0; 4];
    b.extend([0, 1]);
    u32(&mut b, 0);
    wide(&mut b, "original-rectangle");
    u64(&mut b, 0);
    b.push(1);
    refs(&mut b, &[55, 58, 59, 60]);
    b.push(1);
    refs(&mut b, &[56, 57]);
    b.extend([0; 17]);
    reference(&mut b, 8);
    b.extend([0; 4]);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 6);
    secondary.extend([0; 17]);
    wide(&mut secondary, "Rectangle");
    secondary.push(0);
    refs(&mut secondary, &[42, 41, 71, 72, 31, 32, 33, 34, 56, 57]);
    reference(&mut secondary, 3);
    f.put(70, "44A64366-4BD3-4B24-881A-F94C206E8F2D", 18, b, Some(secondary));
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    u32(&mut b, 1);
    u32(&mut b, 1);
    u32(&mut b, 4);
    for tag in [17, 20, 19, 18] {
        u32(&mut b, 3);
        u64(&mut b, tag);
        u64(&mut b, 2);
        u32(&mut b, 0);
        u32(&mut b, 1);
        u32(&mut b, 1);
        u64(&mut b, 0);
    }
    u32(&mut b, 1);
    b.push(0);
    f.put(101, "0D57BD2F-D09B-43FC-AD57-1E89A118C453", 1, b, None);
    f
}

#[test]
fn polygon_history_retains_dimensions_and_selected_curve_references() {
    let d = rectangle().design();
    let profiles = d.sketch_profiles().unwrap();
    assert_eq!(profiles[0].regions[0][0].curves, [31, 34, 33, 32]);
    assert!(d.circle_profiles().is_err());
    let mut doc = d.to_document("Original rectangular prism").unwrap();
    volume(&doc, 30. * 20. * 17.);
    doc.params.iter_mut().find(|p| p.name == "width").unwrap().expr = "heightSketch * 2".into();
    doc.params.iter_mut().find(|p| p.name == "heightSketch").unwrap().expr = "25 mm".into();
    volume(&doc, 50. * 25. * 17.);
    let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
    assert_eq!(doc, reopened);
    volume(&reopened, 50. * 25. * 17.);
}

#[test]
fn incomplete_polygon_records_never_become_partial_history() {
    for id in [31, 55, 56, 57, 101] {
        let length = rectangle().0[&id].2.len();
        for keep in 0..length {
            let mut f = rectangle();
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(f.design().to_document("Truncated").is_err(), "record {id} prefix {keep}");
        }
    }
    let mut f = rectangle();
    f.0.get_mut(&31).unwrap().1 = 99;
    assert!(f.design().to_document("Unknown line").is_err());
    let mut f = rectangle();
    f.0.get_mut(&101).unwrap().2[33..41].copy_from_slice(&999u64.to_le_bytes());
    assert!(f.design().sketch_profiles().is_err());
}

#[test]
fn aligned_dimensions_retain_diagonal_distance_and_dependency_edits() {
    for reversed in [false, true] {
        let d = rectangle_with_dimensions(true, reversed).design();
        let mut doc = d.to_document("Original diagonal rectangle").unwrap();
        let solidcraft_doc::FeatureKind::Sketch { sketch, .. } = &doc.features[0].kind else { panic!("sketch") };
        let constraint = sketch.constraints.iter().find(|c| c.param.as_deref() == Some("heightSketch")).unwrap();
        assert!(matches!(constraint.kind, solidcraft_doc::sketch::ConstraintKind::Distance { .. }));
        volume(&doc, 30. * 40. * 17.);
        doc.params.iter_mut().find(|p| p.name == "heightSketch").unwrap().expr = "width * 2".into();
        volume(&doc, 30. * 2700_f64.sqrt() * 17.);
        let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, 30. * 2700_f64.sqrt() * 17.);
    }
}

#[test]
fn aligned_dimension_records_validate_flags_references_and_truncation() {
    let fixture = rectangle_with_dimensions(true, true);
    let original = &fixture.0[&57].2;
    for keep in 0..original.len() {
        let mut f = rectangle_with_dimensions(true, true);
        f.0.get_mut(&57).unwrap().2.truncate(keep);
        assert!(f.design().to_document("Truncated diagonal").is_err(), "prefix {keep}");
    }
    // Unknown kind flags and mismatched point membership must fail, regardless
    // of the connection slot metadata that precedes the semantic fields.
    for (offset, bytes) in [(original.len() - 46, 0xdead_u64.to_le_bytes().to_vec()), (original.len() - 11, 999u64.to_le_bytes().to_vec())] {
        let mut f = rectangle_with_dimensions(true, true);
        f.0.get_mut(&57).unwrap().2[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(f.design().to_document("Invalid diagonal").is_err(), "offset {offset}");
    }
}

#[test]
fn linear_connection_slots_and_reference_order_do_not_define_orientation() {
    for aligned in [false, true] {
        let base = rectangle_with_dimensions(aligned, false);
        let expected = base.design().sketch_constraints().unwrap();
        let mut f = rectangle_with_dimensions(aligned, false);
        for id in [56, 57] {
            let data = &mut f.0.get_mut(&id).unwrap().2;
            for (offset, slot) in [(20, 0_u32), (35, u32::MAX), (50, 17)] {
                data[offset..offset + 4].copy_from_slice(&slot.to_le_bytes());
            }
            let len = data.len();
            let first = data[len - 23..len - 12].to_vec();
            let second = data[len - 12..len - 1].to_vec();
            data[len - 23..len - 12].copy_from_slice(&second);
            data[len - 12..len - 1].copy_from_slice(&first);
        }
        let design = f.design();
        assert_eq!(design.sketch_constraints().unwrap(), expected);
        volume(&design.to_document("Reordered graph").unwrap(), 30. * if aligned { 40. } else { 20. } * 17.);
    }
}
