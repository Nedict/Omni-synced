use super::*;
use crate::f3d::import_tests::{Fixture, common};
use crate::f3d::relationship_tests::{reference, refs, u32};

fn authored(version: u32, edges: bool) -> Fixture {
    let mut f = super::tests::authored(version, true, false);
    let mut feature = vec![0; 6];
    refs(&mut feature, &[900]);
    feature.extend([0; 4]);
    feature.push(1);
    if version == 3 {
        feature.push(0);
    }
    feature.push(1);
    feature.extend([0; 9]);
    refs(&mut feature, if edges { &[901, 910] } else { &[910] });
    common(&mut feature, "Face fillet", false);
    f.0.get_mut(&19).unwrap().2 = feature;
    let modern = version == 3;
    let mut set = vec![0; if modern { 8 } else { 7 }];
    set.extend([1, 0]);
    if modern {
        set.push(0);
    }
    set.extend([1, 0, 0]);
    if edges {
        reference(&mut set, 901);
    } else {
        set.push(0);
    }
    set.extend(vec![0; if modern { 19 } else { 17 }]);
    reference(&mut set, 910);
    set.extend([0; 3]);
    reference(&mut set, 902);
    set.extend([0; 4]);
    u32(&mut set, 0);
    reference(&mut set, 903);
    set.extend([0; 4]);
    f.0.get_mut(&900).unwrap().2 = set;
    f.put(910, CONTAINER, 1, vec![], None);
    f
}

#[test]
fn face_and_mixed_fillet_inputs_retain_separate_selections() {
    for version in [2, 3] {
        for edges in [false, true] {
            let history = authored(version, edges).design().history().unwrap();
            assert_eq!(
                history.last().unwrap().kind,
                HistoryKind::Fillet {
                    sets: vec![FilletSet {
                        container: edges.then_some(901),
                        face_container: Some(910),
                        radius: 902,
                        weight: 903,
                        tangent_chain: true,
                    }],
                }
            );
        }
    }
}

#[test]
fn face_fillet_inputs_reject_truncation_flags_aliases_and_wrong_classes() {
    for version in [2, 3] {
        for edges in [false, true] {
            for id in [19, 900] {
                let f = authored(version, edges);
                for keep in 0..f.0[&id].2.len() {
                    let mut copy = Fixture(f.0.clone());
                    copy.0.get_mut(&id).unwrap().2.truncate(keep);
                    assert!(copy.design().history().is_err(), "{version}/{edges}/{id}/{keep}");
                }
            }
            let face = (if version == 3 { 14 + 19 } else { 12 + 17 }) + if edges { 11 } else { 1 };
            for offset in [if version == 3 { 14 } else { 12 }, face, face + 11] {
                let mut f = authored(version, edges);
                f.0.get_mut(&900).unwrap().2[offset] = 2;
                assert!(f.design().history().is_err());
            }
            let mut f = authored(version, edges);
            f.0.get_mut(&900).unwrap().2[face + 1..face + 9].copy_from_slice(&901_u64.to_le_bytes());
            assert!(f.design().history().is_err(), "face container must match the feature's ordered selection list");
            let mut f = authored(version, edges);
            f.0.get_mut(&910).unwrap().0 = SET.into();
            assert!(f.design().history().is_err());
            if !edges {
                let mut f = authored(version, false);
                f.0.get_mut(&900).unwrap().2.splice(face..face + 11, [0]);
                let error = f.design().history().unwrap_err().to_string();
                assert!(error.contains("no edge or face selection"), "{error}");
            }
        }
    }
}

#[test]
#[ignore = "requires local controlled face-selection archives"]
fn native_face_fillet_controls_and_style2_preserve_selection_kinds() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    for mode in ["edges", "face", "mixed", "groups"] {
        for suffix in ["baseline", "radius3"] {
            let bytes = std::fs::read(root.join(format!("face-fillet-{mode}-{suffix}.f3d"))).unwrap();
            let designs = read_f3d(&bytes).unwrap();
            let history = designs[0].history().unwrap();
            let HistoryKind::Fillet { sets } = &history.last().unwrap().kind else { panic!("fillet") };
            assert_eq!(sets.len(), if mode == "groups" { 2 } else { 1 });
            assert_eq!(sets[0].container.is_some(), matches!(mode, "edges" | "mixed"));
            assert_eq!(sets[0].face_container.is_some(), mode != "edges");
            let records = Records::new(&designs[0]);
            let classes = designs[0].classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            let count = |container| super::super::loft_faces::objects(&records, &classes, container, history.last().unwrap().id).unwrap().len();
            if let Some(container) = sets[0].container {
                assert_eq!(count(container), if mode == "edges" { 4 } else { 1 });
            }
            if let Some(container) = sets[0].face_container {
                // Fusion's API exposes one face, while these archives retain
                // two selectors with identical complete bounded-face recipes.
                // Preserve both records; a later resolver must certify identity
                // before treating them as the same selected face.
                let selectors = super::super::loft_faces::objects(&records, &classes, container, history.last().unwrap().id).unwrap();
                assert_eq!(selectors.len(), 2);
                let identities = selectors
                    .into_iter()
                    .map(|selector| {
                        let mut r = payload(records.primary(selector).unwrap(), selector).unwrap();
                        zeros(&mut r, 8).unwrap();
                        super::super::recipe_incidence::identity(&records, &classes, r.reference().unwrap(), true).unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(identities[0], identities[1]);
            }
            if mode == "groups" {
                assert!(sets[1].face_container.is_none());
                assert_eq!(count(sets[1].container.unwrap()), 1);
                assert_ne!(sets[0].radius, sets[1].radius);
            }
        }
    }
    for name in ["additional-corpus/knobs/Style 2.f3d", "style2-face-fillet-resaved.f3d"] {
        let designs = read_f3d(&std::fs::read(root.join(name)).unwrap()).unwrap();
        let history = designs[0].history().unwrap();
        let feature = history.iter().find(|f| f.name == "Fillet7").unwrap();
        let HistoryKind::Fillet { sets } = &feature.kind else { panic!("fillet") };
        assert_eq!(sets.len(), 1);
        assert!(sets[0].container.is_some() && sets[0].face_container.is_some());
        assert!(sets[0].tangent_chain);
    }
}

#[test]
#[ignore = "requires the local adapter archive"]
fn native_face_only_adapter_fillet_retains_its_selection() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_CORPUS").unwrap());
    let designs = read_f3d(&std::fs::read(root.join("adapter-x20-makita.f3d")).unwrap()).unwrap();
    let timeline = designs[0].timeline().unwrap();
    let feature = timeline.into_iter().find(|entry| entry.id == 18835).unwrap().feature.unwrap();
    let HistoryKind::Fillet { sets } = feature.kind else { panic!("fillet") };
    assert_eq!(sets.len(), 1);
    assert!(sets[0].container.is_none());
    assert_eq!(sets[0].face_container, Some(18837));
    assert!(sets[0].tangent_chain);
}
