//! Original, analytic regressions for the live face-profile import path.
use super::*;
use crate::f3d::import_tests::{fixture, volume};

#[test]
fn linked_cap_extrusions_follow_source_dimensions_and_reopen() {
    for high in [false, true] {
        for chamfer in [false, true] {
            for operation in [Operation::NewBody, Operation::Join, Operation::Cut] {
                let mut doc = fixture(false).design().to_document("Authored face extrusion").unwrap();
                let producer = doc.features[1].id;
                let body = doc.features[1].body_names[0].clone();
                let FeatureKind::Extrude { extent: mut extension, .. } = doc.features[1].kind.clone() else { panic!("source extrusion") };
                let mut model = solidcraft_doc::Model::new();
                model.evaluate(&doc);
                if chamfer {
                    let cap = format!("F{producer}:{}", if high { "top" } else { "start" });
                    let names = solidcraft_doc::naming::edge_names(model.state().body(&body).unwrap())
                        .into_iter()
                        .filter(|name| name.split('|').any(|face| solidcraft_doc::naming::strip_piece(face) == cap))
                        .collect::<Vec<_>>();
                    let z = if high { 17. } else { 0. };
                    let id = doc
                        .add_feature(
                            FeatureKind::Chamfer {
                                edges: vec![(Vec3::new(0., 30., z)).into(), Vec3::new(0., -30., z).into()],
                                distance: "2 mm".into(),
                                distance2: Some("3 mm".into()),
                                angle: None,
                                flip: false,
                                tangent_chain: false,
                                groups: vec![],
                                body: Some((body.clone()).into()),
                            },
                            Some("Trimmed cap"),
                        )
                        .unwrap();
                    doc.features.iter_mut().find(|f| f.id == id).unwrap().edge_names = names;
                    model.evaluate(&doc);
                }
                assert!(model.results.iter().all(|r| r.error.is_none()));
                let face = super::super::loft_faces::Face::cap(9, 8001, high, false);
                let anchor = face.anchor(&doc, &model.state(), producer, &[(producer, None)]).unwrap();
                let plane = doc.resolve_face_anchor(&anchor, 0, &model.state()).unwrap();
                let sketch = face_extrusion_sketch(&mut doc, &model.state(), anchor, plane, "Authored cap").unwrap();
                let distance = if operation == Operation::Cut { -4. } else { 6. };
                extension.distance = format!("{distance} mm").into();
                let targets = if operation == Operation::NewBody { vec![] } else { vec![body] };
                let added = doc
                    .add_feature(
                        FeatureKind::Extrude {
                            sketch: (sketch).into(),
                            profiles: ProfileSel::All,
                            extent: extension,
                            operation,
                            targets: targets.into_iter().map(Into::into).collect(),
                            thin: None,
                        },
                        Some("Extension"),
                    )
                    .unwrap();
                if operation == Operation::NewBody {
                    doc.features.iter_mut().find(|f| f.id == added).unwrap().body_names = vec!["Extension body".into()];
                }
                let expected = |radius: f64, height: f64, cap: f64| {
                    let pi = std::f64::consts::PI;
                    pi * radius * radius * height - if chamfer { pi * 3. * (radius * cap - cap * cap / 3.) } else { 0. }
                        + pi * (radius - cap).powi(2) * distance
                };
                let mut cap = if chamfer { 2. } else { 0. };
                volume(&doc, expected(30., 17., cap));
                let cut_anchor = if operation == Operation::Cut && !chamfer {
                    model.evaluate(&doc);
                    let end = super::super::loft_faces::Face::cap(9, 8002, true, false);
                    let anchor = end.anchor(&doc, &model.state(), producer, &[(added, None)]).unwrap();
                    let plane = doc.resolve_face_anchor(&anchor, 0, &model.state()).unwrap();
                    assert!((plane.origin.z - if high { 13. } else { 4. }).abs() < 1e-8);
                    assert!(matches!(anchor.target.certificate, Some(solidcraft_doc::config::FaceCertificate::Lineage)));
                    // The exact history may not substitute an unchanged stock
                    // face, a foreign target, or stale pre-edit geometry.
                    assert_eq!(end.anchor(&doc, &model.state(), producer, &[(producer, None)]).is_err(), high);
                    let mut changed = doc.clone();
                    let FeatureKind::Extrude { extent, .. } = &mut changed.feature_mut(added).unwrap().kind else { panic!("cut") };
                    extent.distance = "-8 mm".into();
                    assert!(end.anchor(&changed, &model.state(), producer, &[(added, None)]).is_err());
                    let FeatureKind::Extrude { targets, .. } = &mut changed.feature_mut(added).unwrap().kind else { panic!("cut") };
                    targets.clear();
                    assert!(end.anchor(&changed, &model.state(), producer, &[(added, None)]).is_err());
                    Some(anchor)
                } else {
                    None
                };
                doc.change_param("d7", "80 mm", None, None).unwrap();
                doc.change_param("height", "21 mm", None, None).unwrap();
                if chamfer {
                    let parameter = doc.find_feature("Trimmed cap").unwrap().param_names[0].clone();
                    doc.change_param(&parameter, "4 mm", None, None).unwrap();
                    cap = 4.;
                }
                volume(&doc, expected(40., 21., cap));
                let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
                assert_eq!(doc, reopened);
                volume(&reopened, expected(40., 21., cap));
                if let Some(anchor) = cut_anchor {
                    model.evaluate(&reopened);
                    let plane = reopened.resolve_face_anchor(&anchor, 0, &model.state()).unwrap();
                    assert!((plane.origin.z - if high { 17. } else { 4. }).abs() < 1e-8);
                }
                doc.features[1].suppressed = true;
                model.evaluate(&doc);
                assert!(model.results.iter().any(|r| r.error.is_some()), "lost source must not use stale cached geometry");
            }
        }
    }
}
