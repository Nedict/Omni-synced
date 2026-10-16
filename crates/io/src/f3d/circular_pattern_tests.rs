use super::*;
use crate::f3d::import_tests::{Fixture, common, volume};
use crate::f3d::relationship_tests::{reference, refs, string, u32, u64, wide};
use std::f64::consts::PI;

// Independently authored relationships and analytic cylinders; no exported bytes.
fn authored(symmetric: bool) -> Fixture {
    let mut f = crate::f3d::mirrors::tests::authored_feature(false);
    let mut b = vec![0; 6];
    reference(&mut b, 910);
    reference(&mut b, 930);
    for ids in [&[][..], &[950, 952][..], &[], &[]] {
        refs(&mut b, ids);
    }
    common(&mut b, "Circular", false);
    f.put(19, CLASS, 0, b, None);
    let output = f.0[&950].clone();
    f.0.insert(952, output);
    let mut b = vec![0; 6];
    b.push(u8::from(symmetric));
    reference(&mut b, 914);
    reference(&mut b, 903);
    refs(&mut b, &[]);
    b.extend([0; 8]);
    for n in [2, 1, u32::MAX] {
        u32(&mut b, n);
    }
    b.push(1);
    u32(&mut b, 7); // four retired operation slots beyond the current count
    for index in 2..=8 {
        u32(&mut b, index);
        u32(&mut b, 0);
        u64(&mut b, 8000 + u64::from(index));
    }
    refs(&mut b, &[911, 912, 915]);
    reference(&mut b, 913);
    reference(&mut b, 19);
    reference(&mut b, 921);
    u32(&mut b, 0);
    f.put(910, HELPER, 0, b, None);
    let mut b = vec![0; 6];
    u32(&mut b, 3);
    reference(&mut b, 910);
    f.put(915, "AD719C1A-11AF-4537-BB41-030A9091D36B", 0, b, None);
    for (id, binding, name, unit, label, expression, value) in
        [(916, 913, "copies", "", "countU", "3", 3.), (917, 914, "sweep", "deg", "TotalAngle", "180 deg", PI)]
    {
        let mut b = vec![0; 24];
        reference(&mut b, binding);
        wide(&mut b, expression);
        b.extend([0; 10]);
        for text in [label, "", unit, name] {
            wide(&mut b, text);
        }
        b.extend(value.to_le_bytes());
        b.extend([0; 12]);
        f.put(id, "7A6A3D31-BE74-4E19-9115-20642944C2E4", 9, b, None);
        let mut b = vec![0; 4];
        b.push(1);
        refs(&mut b, &[19]);
        b.extend([0; 5]);
        if binding == 913 {
            b.push(1);
            u32(&mut b, 3);
        } else {
            b.extend(value.to_le_bytes());
        }
        reference(&mut b, id);
        b.extend([0; 8]);
        reference(&mut b, 19);
        f.put(
            binding,
            if binding == 913 { "A085449A-5144-4B2B-B455-7F7035A40559" } else { "D91D429C-1BB5-4640-B49F-69AF14F463E9" },
            if binding == 913 { 1 } else { 0 },
            b,
            None,
        );
    }
    let mut b = vec![0; 6];
    u32(&mut b, 8);
    b.extend([0; 64]);
    u32(&mut b, 7);
    b.push(0);
    for _ in 0..3 {
        b.extend((-1_f64).to_le_bytes());
    }
    refs(&mut b, &[901]);
    b.extend([0; 2]);
    refs(&mut b, &[]);
    b.extend([0; 60]);
    reference(&mut b, 19);
    f.put(903, "F2A7590D-6654-4674-B393-A2AEF4FEC48A", 4, b, None);
    let mut b = vec![0; 6];
    for value in [0_f64, 0., 0., 0., 1., 0.] {
        b.extend(value.to_le_bytes());
    }
    b.extend([0; 21]);
    u32(&mut b, u32::MAX);
    b.push(1);
    wide(&mut b, "Y");
    b.extend([0; 37]);
    reference(&mut b, 904);
    f.put(900, AXIS, 0, b, None);
    let mut b = vec![0; 8];
    refs(&mut b, &[900]);
    reference(&mut b, 3);
    f.put(904, "2972A3B8-3627-49BC-8C3D-18707605EC44", 1, b, None);
    f
}

#[test]
fn circular_pattern_history_keeps_parameters_and_ignores_retired_instances() {
    for symmetric in [false, true] {
        let d = authored(symmetric).design();
        let history = d.history().unwrap();
        let HistoryKind::CircularPattern(p) = &history.last().unwrap().kind else { panic!("pattern") };
        assert_eq!((p.axis, p.count, p.angle, p.symmetric, p.instances), (900, 913, 914, symmetric, 3));
        assert_eq!(p.features, [9]);
        assert_eq!(p.outputs, [950, 952]);
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        assert_eq!(origin_axis(&Records::new(&d), &classes, 900, 3).unwrap(), Vec3::Y);
        assert!(origin_axis(&Records::new(&d), &classes, 900, 19).is_err());
    }
}

#[test]
fn circular_pattern_import_regenerates_after_parameter_edits_and_reopen() {
    let mut doc = authored(false).design().to_document("Pattern").unwrap();
    volume(&doc, 3. * PI * 30. * 30. * 17.);
    doc.change_param("copies", "5", None, None).unwrap();
    doc.change_param("height", "11 mm", None, None).unwrap();
    doc.change_param("sweep", "-120 deg", None, None).unwrap();
    volume(&doc, 5. * PI * 30. * 30. * 11.);
    let bytes = crate::write_design_container(&doc).unwrap();
    let reopened = crate::read_design(&bytes).unwrap();
    assert_eq!(doc, reopened);
    volume(&reopened, 5. * PI * 30. * 30. * 11.);
}

#[test]
fn circular_pattern_rejects_broken_and_truncated_relationships() {
    for id in [19, 910, 913, 915, 903, 901, 902, 930, 921, 950] {
        for keep in 0..authored(false).0[&id].2.len() {
            let mut f = authored(false);
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(f.design().history().is_err(), "record {id}, prefix {keep}");
        }
    }
    for (id, offset, value) in [(910, 6, 2), (910, 41, 0), (910, 45, 0), (913, 26, 4), (915, 6, 2), (903, 6, 9)] {
        let mut f = authored(false);
        f.0.get_mut(&id).unwrap().2[offset] = value;
        assert!(f.design().history().is_err(), "record {id}, byte {offset}");
    }
}

#[test]
fn circular_pattern_operation_map_certifies_live_and_retired_slots() {
    let make = |bad_owner| {
        let mut f = authored(false);
        let mut b = vec![0; 6];
        u32(&mut b, 4);
        for (feature, tags) in [(912, vec![8002]), (915, vec![8003]), (999, vec![8004]), (998, vec![])] {
            u64(&mut b, if bad_owner && feature == 912 { 911 } else { feature });
            u32(&mut b, tags.len() as u32);
            for tag in tags {
                u64(&mut b, tag);
            }
        }
        u32(&mut b, 3);
        for (tag, feature) in [(8002, 912), (8003, 915), (8004, 999)] {
            u64(&mut b, tag);
            u64(&mut b, if bad_owner && feature == 912 { 911 } else { feature });
        }
        u32(&mut b, 1);
        u64(&mut b, 8002); // retained retirement marker after recreating instance 2
        reference(&mut b, 4);
        f.put(180, "5D89B935-3F04-4CAB-81F5-E7B309E7E13F", 1, b, None);
        f
    };
    assert_eq!(make(false).design().operation_owners().unwrap(), BTreeMap::from([(8002, 912), (8003, 915)]));
    assert!(make(true).design().operation_owners().is_err());
}

#[test]
fn symmetric_circular_pattern_placement_handles_even_odd_and_signed_sweeps() {
    for (count, angle, expected) in [
        (4, 180., vec![-120_f64, -60., 60.]),
        (5, 180., vec![-90., -45., 45., 90.]),
        (4, -180., vec![120., 60., -60.]),
        (4, 360., vec![-180., -90., 90.]),
        (3, 360., vec![-120., 120.]),
        (1, 180., vec![]),
    ] {
        let doc = solidcraft_doc::Document::new("Placement");
        let pattern = solidcraft_doc::PatternKind::Circular {
            origin: (Vec3::ZERO).into(),
            axis: (Vec3::Z).into(),
            count: (count.to_string()).into(),
            angle: format!("{angle} deg").into(),
            symmetric: true,
            alignment: Default::default(),
            reversed: false,
        };
        let model = solidcraft_doc::Model::new();
        let matrices = solidcraft_doc::pattern_matrices(&doc, &model.state(), &pattern).unwrap();
        assert_eq!(matrices.len(), expected.len());
        for (matrix, angle) in matrices.iter().zip(expected) {
            let angle = angle.to_radians();
            assert!((matrix[0][0] - angle.cos()).abs() < 1e-10);
            assert!((matrix[0][1] - angle.sin()).abs() < 1e-10);
        }
    }
    let old = serde_json::json!({"type":"circular", "origin":[0,0,0], "axis":[0,0,1], "count":"3", "angle":"90 deg"});
    let pattern: solidcraft_doc::PatternKind = serde_json::from_value(old).unwrap();
    assert!(matches!(pattern, solidcraft_doc::PatternKind::Circular { symmetric: false, .. }));
}

#[test]
#[ignore = "requires local circular-pattern controls and user archives"]
fn native_circular_pattern_controls_and_legacy_histories() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBE_DIR").expect("set SOLIDCRAFT_F3D_PROBE_DIR"));
    for mode in ["body", "feature"] {
        for (suffix, count, angle, symmetric, axis) in [
            ("4-full", 4, 360., false, Vec3::Z),
            ("6-full", 6, 360., false, Vec3::Z),
            ("4-half", 4, 180., false, Vec3::Z),
            ("4-symmetric", 4, 180., true, Vec3::Z),
            ("4-negative", 4, -180., false, Vec3::Z),
            ("4-y-axis", 4, 360., false, Vec3::Y),
        ] {
            let name = format!("circular-pattern-{mode}-{suffix}");
            let d = read_f3d(&std::fs::read(root.join(format!("{name}.f3d"))).unwrap()).unwrap().remove(0);
            let h = d.history().unwrap_or_else(|e| panic!("{name}: {e}"));
            let HistoryKind::CircularPattern(p) = &h.last().unwrap().kind else { panic!("{name}") };
            assert_eq!((p.instances, p.symmetric), (count, symmetric));
            let parameters = d.parameters().unwrap();
            let a = parameters.iter().find(|v| v.binding == Some(p.angle)).unwrap();
            assert!((a.value.to_degrees() - angle).abs() < 1e-9);
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            assert_eq!(origin_axis(&Records::new(&d), &classes, p.axis, 3).unwrap(), axis);
            let doc = d.to_document(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
            volume(&doc, count as f64 * PI * 3. * 3. * 10.);
            let mut model = solidcraft_doc::Model::new();
            model.evaluate(&doc);
            let state = model.state();
            assert_eq!(state.bodies.len(), count);
            let mut expected = vec![Vec3::new(20., 0., 5.)];
            let step = angle.to_radians() / if angle.abs() == 360. { count as f64 } else { (count - 1) as f64 };
            for i in 0..count {
                let offset = i as i64 - if symmetric { (count / 2) as i64 } else { 0 };
                if offset == 0 {
                    continue;
                }
                let a = offset as f64 * step;
                expected.push(if axis == Vec3::Z {
                    Vec3::new(20. * a.cos(), 20. * a.sin(), 5.)
                } else {
                    Vec3::new(20. * a.cos() + 5. * a.sin(), 0., 5. * a.cos() - 20. * a.sin())
                });
            }
            for (body, expected) in state.bodies.iter().zip(expected) {
                assert!(solidcraft_kernel::measure(&body.body).unwrap().centroid.dist(expected) < 1e-5, "{name}: placement");
            }
        }
    }
    for (name, count) in [("Round", 10), ("Style 7", 30)] {
        for path in [root.join("additional-corpus/knobs").join(format!("{name}.f3d")), root.join(format!("pattern-corpus-{name}.f3d"))] {
            let d = read_f3d(&std::fs::read(path).unwrap()).unwrap().remove(0);
            let entry = d.timeline().unwrap().into_iter().find(|e| e.class == CLASS).unwrap();
            let HistoryKind::CircularPattern(p) = entry.feature.unwrap().kind else { panic!("pattern") };
            assert_eq!(p.instances, count);
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            assert_eq!(origin_axis(&Records::new(&d), &classes, p.axis, 3).unwrap(), Vec3::Z);
        }
    }
}

#[test]
#[ignore = "requires local body-label replacement controls; no native archives are committed"]
fn native_body_label_replacements_keep_pattern_sources_after_profile_edits() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBE_DIR").unwrap());
    for (name, area, count, height, x, y) in [
        ("rectangle", 12., 4, 10., 20., 0.),
        ("left-triangle", 6., 4, 10., 18. + 4. / 3., 0.5),
        ("right-triangle", 6., 4, 10., 22. - 4. / 3., -0.5),
        ("six", 6., 6, 10., 22. - 4. / 3., -0.5),
        ("height20", 6., 6, 20., 22. - 4. / 3., -0.5),
        ("restored-rectangle", 12., 6, 20., 20., 0.),
    ] {
        let d = read_f3d(&std::fs::read(root.join(format!("body-label-{name}.f3d"))).unwrap()).unwrap().remove(0);
        let mut doc = d.to_document(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(doc.features.len(), 3);
        let check = |doc: &solidcraft_doc::Document, count: usize, height: f64, sweep: f64| {
            volume(doc, area * count as f64 * height);
            let mut model = solidcraft_doc::Model::new();
            model.evaluate(doc);
            let state = model.state();
            assert_eq!(state.bodies.len(), count);
            for (index, body) in state.bodies.iter().enumerate() {
                let angle = sweep * index as f64 / if sweep == 2. * PI { count as f64 } else { (count - 1) as f64 };
                let expected = Vec3::new(x * angle.cos() - y * angle.sin(), x * angle.sin() + y * angle.cos(), height / 2.);
                assert!(solidcraft_kernel::measure(&body.body).unwrap().centroid.dist(expected) < 1e-6, "{name}: body {index}");
            }
        };
        check(&doc, count, height, 2. * PI);
        doc.change_param("d1", "15 mm", None, None).unwrap();
        doc.change_param("d3", "5", None, None).unwrap();
        doc.change_param("d4", "180 deg", None, None).unwrap();
        check(&doc, 5, 15., PI);
        let bytes = crate::write_design_container(&doc).unwrap();
        let reopened = crate::read_design(&bytes).unwrap();
        assert_eq!(doc, reopened);
        check(&reopened, 5, 15., PI);
    }
}

#[test]
#[ignore = "requires local patterned Combine controls; no native archives are committed"]
fn native_patterned_combine_keeps_selected_copies_after_profile_replacement() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBE_DIR").unwrap());
    for keep in [false, true] {
        for (prefix, suffix, area, features) in [
            ("pattern-combine", "rectangle", 12., 6),
            ("pattern-combine", "left-triangle", 6., 6),
            ("pattern-combine", "right-triangle", 6., 6),
            ("pattern-fillet-combine", "before", 12., 6),
            ("pattern-fillet-combine", "inserted", 12. - (4. - PI) * 0.3 * 0.3, 7),
        ] {
            let name = format!("{prefix}-{}-{suffix}", u8::from(keep));
            let d = read_f3d(&std::fs::read(root.join(format!("{name}.f3d"))).unwrap()).unwrap().remove(0);
            let mut doc = d.to_document(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(doc.features.len(), features);
            let check = |doc: &solidcraft_doc::Document, height: f64| {
                volume(doc, 1600. * PI * height - if keep { 0. } else { 4. * area * height });
                let mut model = solidcraft_doc::Model::new();
                model.evaluate(doc);
                let state = model.state();
                assert_eq!(state.bodies.len(), if keep { 5 } else { 1 });
                let target = &state.bodies[0];
                let measured = solidcraft_kernel::measure(&target.body).unwrap();
                let expected = 1600. * PI * height - 4. * area * height;
                assert!((measured.volume - expected).abs() < expected * 1e-3, "{name}: target volume {} != {expected}", measured.volume);
                assert!(measured.centroid.dist(Vec3::new(0., 0., height / 2.)) < 1e-5, "{name}: target centroid");
            };
            check(&doc, 10.);
            doc.change_param("d1", "15 mm", None, None).unwrap();
            doc.change_param("d3", "15 mm", None, None).unwrap();
            check(&doc, 15.);
            let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            assert_eq!(doc, reopened);
            check(&reopened, 15.);
        }
    }
}

fn authored_body_outputs() -> Fixture {
    let mut f = authored(false);
    f.0.get_mut(&910).unwrap().2[45] = 2;
    let mut body_source = crate::f3d::mirrors::tests::authored_output();
    for id in [920, 941] {
        f.0.insert(id, body_source.0.remove(&id).unwrap());
    }
    for (feature, table, entries) in [(9, 970, vec![(7, 973)]), (19, 971, vec![(1, 974), (2, 975)])] {
        let bytes = &mut f.0.get_mut(&feature).unwrap().2;
        let at = bytes.len() - 59;
        let mut link = Vec::new();
        reference(&mut link, table);
        bytes[at..at + 11].copy_from_slice(&link);
        let mut b = vec![0; 6];
        u32(&mut b, entries.len() as u32);
        for (label, id) in entries {
            reference(&mut b, id);
            u64(&mut b, label);
        }
        u64(&mut b, 100);
        f.put(table, "B7F34D7B-DAC4-41A5-B67D-D672537C3BFA", 2, b, Some(vec![0; 5]));
    }
    for (id, identity, groups) in
        [(973, "original-source-body", vec![8001]), (974, "copy-one", vec![8001, 8100, 8002]), (975, "copy-two", vec![8001, 8100, 8003])]
    {
        let mut b = vec![0; 7];
        wide(&mut b, identity);
        u32(&mut b, 0);
        u32(&mut b, groups.len() as u32);
        for op in groups {
            string(&mut b, "19");
            u32(&mut b, 2);
            u32(&mut b, 0);
            u32(&mut b, 2);
            u64(&mut b, op);
            b.push(0);
            u32(&mut b, 1);
        }
        b.extend([0; 2]);
        f.put(id, "ABA79BE7-D8D2-4C17-B59B-C2035E111F77", 4, b, None);
    }
    // Reverse registration order relative to the copy instance indices.
    for (handle, identity) in [(950, "copy-two"), (952, "copy-one")] {
        let mut b = vec![0; 6];
        wide(&mut b, identity);
        b.extend([0, 1]);
        reference(&mut b, 951);
        reference(&mut b, 19);
        let mut secondary = vec![0; 4];
        reference(&mut secondary, 4);
        f.put(handle, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2, b, Some(secondary));
    }
    f
}

#[test]
fn patterned_body_selector_uses_live_instance_not_output_registration_order() {
    let owners = BTreeMap::from([(8001, 9), (8100, 19), (8002, 912), (8003, 915)]);
    let resolve = |f: Fixture, owners: &BTreeMap<u64, u64>, handle, label| {
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        body_output(&Records::new(&d), &classes, owners, 19, Some(handle), 8100, label).map(|output| output.index.unwrap())
    };
    assert_eq!(resolve(authored_body_outputs(), &owners, 952, 1).unwrap(), 0);
    assert_eq!(resolve(authored_body_outputs(), &owners, 950, 2).unwrap(), 1);
    assert!(resolve(authored_body_outputs(), &owners, 950, 1).is_err());
    assert!(resolve(authored_body_outputs(), &owners, 80, 1).is_err());
    for (tag, owner) in [(8001, 19), (8100, 9), (8002, 915)] {
        let mut wrong = owners.clone();
        wrong.insert(tag, owner);
        assert!(resolve(authored_body_outputs(), &wrong, 952, 1).is_err());
    }
    for id in [974, 971, 952, 912, 941] {
        for keep in 0..authored_body_outputs().0[&id].2.len() {
            let mut f = authored_body_outputs();
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(resolve(f, &owners, 952, 1).is_err(), "{id}/{keep}");
        }
    }
    for tag in [0_u64, 8004, 9999] {
        let mut f = authored_body_outputs();
        let bytes = &mut f.0.get_mut(&974).unwrap().2;
        let at = bytes.windows(8).position(|b| b == 8002_u64.to_le_bytes()).unwrap();
        bytes[at..at + 8].copy_from_slice(&tag.to_le_bytes());
        assert!(resolve(f, &owners, 952, 1).is_err(), "operation {tag}");
    }
}

#[test]
fn retired_pattern_copy_requires_null_handle_and_explicit_retired_operation() {
    let owners = BTreeMap::from([(8001, 9), (8100, 19), (8002, 912), (8003, 915)]);
    let resolve = |tag: u64, handle| {
        let mut f = authored_body_outputs();
        let bytes = &mut f.0.get_mut(&974).unwrap().2;
        let at = bytes.windows(8).position(|b| b == 8002_u64.to_le_bytes()).unwrap();
        bytes[at..at + 8].copy_from_slice(&tag.to_le_bytes());
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        body_output(&Records::new(&d), &classes, &owners, 19, handle, 8100, 1)
    };
    assert_eq!(resolve(8004, None).unwrap().index, None);
    assert!(resolve(8004, Some(952)).is_err());
    assert!(resolve(8002, None).is_err());
    assert!(resolve(9999, None).is_err());
}

#[test]
fn pattern_replacements_preserve_identity_and_return_added_contributors_for_validation() {
    let mut f = authored_body_outputs();
    let mut replacement = f.0[&974].2.clone();
    let mut r = Reader { bytes: &replacement, at: 7 };
    r.wide_string().unwrap();
    let count_at = r.at + 4;
    replacement[count_at..count_at + 4].copy_from_slice(&4_u32.to_le_bytes());
    let mut group = Vec::new();
    // A cached member can recur after a blend; it is not a body identity.
    string(&mut group, "1919");
    for n in [3, 0, 2, 4] {
        u32(&mut group, n);
    }
    u64(&mut group, 8101);
    group.push(0);
    u32(&mut group, 1);
    replacement.splice(replacement.len() - 2..replacement.len() - 2, group);
    f.put(976, "ABA79BE7-D8D2-4C17-B59B-C2035E111F77", 4, replacement, None);
    let table = &mut f.0.get_mut(&971).unwrap().2;
    table[6..10].copy_from_slice(&3_u32.to_le_bytes());
    let mut entry = Vec::new();
    reference(&mut entry, 976);
    u64(&mut entry, 3);
    table.splice(table.len() - 8..table.len() - 8, entry);
    let mut link = vec![0; 6];
    reference(&mut link, 976);
    f.0.get_mut(&974).unwrap().2.splice(..7, link);
    let check = |f: Fixture| {
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let records = Records::new(&d);
        let owners = BTreeMap::from([(8001, 9), (8100, 19), (8002, 912), (8003, 915)]);
        super::super::body_labels::validate_aliases(&records, &classes, 19, 1, &[3])?;
        body_output(&records, &classes, &owners, 19, Some(952), 8100, 1)
    };
    let result = check(Fixture(f.0.clone())).unwrap();
    assert_eq!(result.index, Some(0));
    assert_eq!(result.source_feature, 9);
    assert_eq!(result.contributors, BTreeSet::from([8101]));
    let bytes = &mut f.0.get_mut(&976).unwrap().2;
    let at = bytes.windows(8).position(|b| b == 8001_u64.to_le_bytes()).unwrap();
    bytes[at..at + 8].copy_from_slice(&8999_u64.to_le_bytes());
    assert!(check(f).is_err(), "replacement cannot discard original operation groups");
}
