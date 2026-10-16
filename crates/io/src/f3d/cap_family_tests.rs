use super::*;

#[test]
#[ignore = "requires the local Style 2 archive"]
fn native_style2_first_fillet_resolves_its_complete_cap_incidence() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    let bytes = std::fs::read(root.join("additional-corpus/knobs/Style 2.f3d")).unwrap();
    let designs = read_f3d(&bytes).unwrap();
    let design = &designs[0];
    let records = Records::new(design);
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    let history = design.history().unwrap();
    let feature = history.iter().find(|f| f.id == 3342).unwrap();
    let HistoryKind::Fillet { sets } = &feature.kind else { panic!("fillet") };
    let mut doc = import_f3d_with_preference(&bytes, "Cap incidence", ImportPreference::Editability).unwrap().document;
    let index = doc
        .features
        .iter()
        .position(|f| f.name == "Fillet3")
        .unwrap_or_else(|| panic!("first shoulder fillet must import: {:?}", doc.import_warnings));
    // Validate the actual imported feature and an edited saved copy before
    // resolving its input selectors against the preceding history state.
    let mut rounded = doc.clone();
    rounded.features.truncate(index + 1);
    let fillet = rounded.features[index].clone();
    assert_eq!(fillet.edge_names.len(), 2);
    let FeatureKind::Fillet { radius, .. } = &fillet.kind else { panic!("fillet") };
    let radius = rounded.eval(radius, solidcraft_doc::expr::Kind::Length).unwrap();
    let mut evaluator = solidcraft_doc::Model::new();
    let mut volumes = Vec::new();
    for scale in [1., 0.75] {
        let mut edited = rounded.clone();
        edited.change_param(&fillet.param_names[0], &format!("{} mm", radius * scale), None, None).unwrap();
        let reopened = crate::read_design(&crate::write_design_container(&edited).unwrap()).unwrap();
        assert_eq!(edited, reopened);
        evaluator.evaluate(&reopened);
        assert!(
            evaluator.results.iter().all(|r| r.error.is_none()),
            "{:?}",
            evaluator.results.iter().filter_map(|r| r.error.as_ref()).collect::<Vec<_>>()
        );
        volumes.push(solidcraft_kernel::measure(&evaluator.state().bodies[0].body).unwrap().volume);
    }
    assert!(volumes[0] > volumes[1] && volumes[1] > 0., "smaller concave round must add less material: {volumes:?}");
    doc.features.truncate(index);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let features = history.iter().filter_map(|native| doc.find_feature(&native.name).map(|f| (native.id, f.id))).collect();
    let operations = design.operation_owners().unwrap();
    let all_profiles = design.sketch_profiles().unwrap();
    let profiles = all_profiles.iter().map(|p| (p.feature, p)).collect();
    let lines = design.sketch_lines().unwrap().into_iter().map(|line| (line.id, line)).collect();
    let circles = design.sketch_circles().unwrap().into_iter().map(|circle| (circle.id, circle)).collect();
    let state = model.state();
    let context = Context {
        records: &records,
        classes: &classes,
        operations: &operations,
        features: &features,
        profiles: &profiles,
        lines: &lines,
        circles: &circles,
        doc: &doc,
        state: &state,
        model: &model,
        before: None,
        depth: 0,
    };
    let mut failures = Vec::new();
    for set in sets {
        assert!(set.face_container.is_none());
        for selector in super::super::super::loft_faces::objects(&records, &classes, set.container.unwrap(), feature.id).unwrap() {
            match context.resolve(selector, feature.id) {
                Ok(selected) => assert_eq!(selected.len(), 1),
                Err(error) => failures.push(format!("{selector}: {error}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// Independent integral of the two concave shoulder rounds clipped by the
// outer cylinder. The substitution makes both integration endpoints smooth.
fn shoulder_added_volume(radius: f64, half_width: f64, round: f64) -> f64 {
    let f = |t: f64| {
        let x = half_width + round - round * t.sin();
        4. * (radius * radius - x * x).sqrt() * round * round * t.cos() * (1. - t.cos())
    };
    let n = 4096;
    let h = std::f64::consts::FRAC_PI_2 / f64::from(n);
    let sum: f64 = (1..n).map(|i| f(f64::from(i) * h) * if i % 2 == 0 { 2. } else { 4. }).sum();
    h / 3. * (f(0.) + f(std::f64::consts::FRAC_PI_2) + sum)
}

#[test]
fn split_caps_resolve_one_chord_with_distinct_vertices_on_the_same_cylinder() {
    let bytes = include_bytes!("../../../../apps/solidcraft-cli/tests/history/split-cylinder-cap-fillet.solidcraft");
    for diameter in [20., 22.] {
        let mut rounded = crate::read_design(bytes).unwrap();
        rounded.change_param("diameter", &format!("{diameter} mm"), None, None).unwrap();
        let mut doc = rounded.clone();
        doc.features.truncate(4);
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        assert!(model.results.iter().all(|r| r.error.is_none()));
        let state = model.state();
        let body = state.body("Knob").unwrap();
        let caps = super::super::super::loft_faces::joined_cap_indices(&doc, &state, 2, &BTreeMap::from([(2, true)])).unwrap();
        assert_eq!(caps.len(), 2, "the upper band splits the base cap");
        assert!(
            super::super::super::loft_faces::joined_cap_anchor(&doc, &state, 2, &BTreeMap::from([(2, true)])).is_err(),
            "a whole cap family cannot silently become one attached face"
        );
        let family = |name: &str| solidcraft_doc::naming::face_family_indices(body, name).unwrap().into_iter().collect::<BTreeSet<_>>();
        let cylinder = Term::from([(9001, 3), (9002, 3), (9002, 5)]);
        let cap = Term::from([(9001, 2)]);
        let mut cylinder_faces = family("F2:side:c1");
        cylinder_faces.extend(family("F4:side:c1"));
        assert!(cylinder_faces.len() > 2);
        let stock = body.body.tessellate_with(1e-4, true).unwrap().measure().volume;
        for (curve, label, x) in [("l2", 4, 3.), ("l4", 6, -3.)] {
            let side = Term::from([(9002, label)]);
            let indices = BTreeMap::from([
                (cap.clone(), caps.clone()),
                (cylinder.clone(), cylinder_faces.clone()),
                (side.clone(), family(&format!("F4:side:{curve}"))),
            ]);
            let vertex = |signature| recipe_incidence::Vertex {
                faces: BTreeSet::from([cap.clone(), cylinder.clone(), side.clone()]),
                seams: BTreeSet::new(),
                signatures: vec![[signature; 8]],
            };
            let mut edge = recipe_incidence::Edge { faces: [cap.clone(), side.clone()], ends: vec![vertex(1), vertex(2)] };
            let selected = edges::resolve(body, &edge, &indices).unwrap();
            assert_eq!(selected.len(), 1);
            assert!(selected[0].point.dist(Vec3::new(x, 0., 8.)) < 1e-7);
            edge.ends[1] = edge.ends[0].clone();
            assert!(edges::resolve(body, &edge, &indices).is_err(), "a repeated complete vertex identity is not a finite edge");
        }
        for r in [1., 1.5] {
            rounded.change_param("round_radius", &format!("{r} mm"), None, None).unwrap();
            let reopened = crate::read_design(&crate::write_design_container(&rounded).unwrap()).unwrap();
            model.evaluate(&reopened);
            assert!(model.results.iter().all(|result| result.error.is_none()));
            let actual = model.state().body("Knob").unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
            let expected = shoulder_added_volume(diameter / 2., 3., r);
            assert!((actual - stock - expected).abs() < 0.02, "diameter={diameter}, r={r}: added {} != {expected}", actual - stock);
        }
    }
}

#[test]
#[ignore = "requires the local Style 2 archive"]
fn native_style2_second_fillet_retains_the_complete_selected_graph() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    let bytes = std::fs::read(root.join("additional-corpus/knobs/Style 2.f3d")).unwrap();
    let designs = read_f3d(&bytes).unwrap();
    let design = &designs[0];
    let records = Records::new(design);
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    let history = design.history().unwrap();
    let feature = history.iter().find(|f| f.id == 3371).unwrap();
    let HistoryKind::Fillet { sets } = &feature.kind else { panic!("fillet") };
    let mut doc = import_f3d_with_preference(&bytes, "Cap incidence", ImportPreference::Editability).unwrap().document;
    let complete = doc.clone();
    if let Some(index) = doc.features.iter().position(|f| f.name == "Fillet4") {
        doc.features.truncate(index);
    }
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let features = history.iter().filter_map(|native| doc.find_feature(&native.name).map(|f| (native.id, f.id))).collect();
    let operations = design.operation_owners().unwrap();
    let all_profiles = design.sketch_profiles().unwrap();
    let profiles = all_profiles.iter().map(|p| (p.feature, p)).collect();
    let lines = design.sketch_lines().unwrap().into_iter().map(|line| (line.id, line)).collect();
    let circles = design.sketch_circles().unwrap().into_iter().map(|circle| (circle.id, circle)).collect();
    let state = model.state();
    let context = Context {
        records: &records,
        classes: &classes,
        operations: &operations,
        features: &features,
        profiles: &profiles,
        lines: &lines,
        circles: &circles,
        doc: &doc,
        state: &state,
        model: &model,
        before: None,
        depth: 0,
    };
    for set in sets {
        let objects = super::super::super::loft_faces::objects(&records, &classes, set.container.unwrap(), feature.id).unwrap();
        let selected = context.resolve_group(&objects, feature.id).unwrap();
        assert_eq!(selected.len(), 8, "six native edges include two kernel-seam subdivisions");
        let body = state.body(&selected[0].body).unwrap();
        let points: Vec<_> = selected
            .iter()
            .map(|selection| {
                let hit = solidcraft_doc::naming::locate_edge(body, &selection.name, selection.point).unwrap();
                assert!(hit.on_point, "the exact named edge must retain its selected point");
                hit.point
            })
            .collect();
        let expanded = solidcraft_kernel::tangent_edge_points(&body.body, &points).unwrap();
        assert_eq!(expanded.len(), 8, "tangent continuation must not select internal cylinder seams");
    }
    let doc = complete;
    let index = doc.features.iter().position(|f| f.name == "Fillet4").unwrap_or_else(|| panic!("Fillet4 must import: {:?}", doc.import_warnings));
    let mut rounded = doc.clone();
    rounded.features.truncate(index + 1);
    let fillet = rounded.features[index].clone();
    assert_eq!(fillet.edge_names.len(), 8);
    let FeatureKind::Fillet { radius, .. } = &fillet.kind else { panic!("fillet") };
    let old = rounded.eval(radius, solidcraft_doc::expr::Kind::Length).unwrap();
    let mut volumes = Vec::new();
    for scale in [1., 0.75] {
        let mut edited = rounded.clone();
        edited.change_param(&fillet.param_names[0], &format!("{} mm", old * scale), None, None).unwrap();
        let reopened = crate::read_design(&crate::write_design_container(&edited).unwrap()).unwrap();
        assert_eq!(edited, reopened);
        let mut evaluator = solidcraft_doc::Model::new();
        evaluator.evaluate(&reopened);
        assert!(evaluator.results.iter().all(|r| r.error.is_none()));
        volumes.push(solidcraft_kernel::measure(&evaluator.state().bodies[0].body).unwrap().volume);
    }
    assert!(volumes.iter().all(|v| v.is_finite() && *v > 0.) && (volumes[0] - volumes[1]).abs() > 1e-6);
}

#[test]
fn a_complete_symmetric_blend_group_requires_every_chain_and_shared_vertex() {
    let doc = crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/split-cylinder-cap-fillet.solidcraft")).unwrap();
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let body = state.body("Knob").unwrap();
    let family = |name: &str| solidcraft_doc::naming::face_family_indices(body, name).unwrap().into_iter().collect::<BTreeSet<_>>();
    let cap = Term::from([(9001, 2)]);
    let cylinder = Term::from([(9001, 3), (9002, 3), (9002, 5)]);
    let mut curved = family("F2:side:c1");
    curved.extend(family("F4:side:c1"));
    let mut indices = BTreeMap::from([(cap.clone(), family("F2:top")), (cylinder.clone(), curved)]);
    let fillet = doc.find_feature("Shoulder rounds").unwrap();
    let mut native = Vec::new();
    for (i, curve) in ["l2", "l4"].iter().enumerate() {
        let side = Term::from([(9002, 4 + i as i32 * 2)]);
        let round = Term::from([(9003, 1 + i as i32)]);
        indices.insert(side.clone(), family(&format!("F4:side:{curve}")));
        let source = fillet.edge_names.iter().find(|name| name.ends_with(&format!("F4:side:{curve}"))).unwrap();
        indices.insert(round.clone(), family(&format!("F5:blend:{source}")));
        let vertex = |index, third: &Term| recipe_incidence::Vertex {
            faces: BTreeSet::from([cylinder.clone(), round.clone(), third.clone()]),
            seams: BTreeSet::new(),
            signatures: vec![[10 * i as u32 + index; 8]],
        };
        let (a, b, c, d) = (vertex(1, &side), vertex(2, &cap), vertex(3, &cap), vertex(4, &side));
        native.extend([
            recipe_incidence::Edge { faces: [round.clone(), cylinder.clone()], ends: vec![a, b.clone()] },
            recipe_incidence::Edge { faces: [cylinder.clone(), cap.clone()], ends: vec![b, c.clone()] },
            recipe_incidence::Edge { faces: [round, cylinder.clone()], ends: vec![c, d] },
        ]);
    }
    let inputs: Vec<_> = native.iter().map(|edge| (edge.clone(), edges::candidates(body, edge, &indices).unwrap())).collect();
    assert_eq!(inputs.iter().map(|(_, choices)| choices.len()).collect::<Vec<_>>(), [2, 1, 2, 2, 1, 2]);
    let selected = group::resolve(body, &inputs).unwrap();
    assert_eq!(selected.len(), 8);
    let expanded = solidcraft_kernel::tangent_edge_points(&body.body, &selected.iter().map(|s| s.point).collect::<Vec<_>>()).unwrap();
    assert_eq!(expanded.len(), 8, "tangent continuation must not add smooth cylinder seams to the selected rim");
    let expected: BTreeSet<_> = selected.iter().map(|s| s.name.clone()).collect();
    let mut reordered = inputs.clone();
    reordered.reverse();
    for (edge, choices) in &mut reordered {
        edge.ends.reverse();
        choices.reverse();
    }
    let actual: BTreeSet<_> = group::resolve(body, &reordered).unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(actual, expected, "selection order cannot choose a symmetry branch");
    let mut missing = inputs.clone();
    missing.remove(0);
    assert!(group::resolve(body, &missing).is_err(), "one ambiguous side cannot choose one of the pair");
    let mut duplicate = inputs.clone();
    duplicate[2] = duplicate[0].clone();
    assert!(group::resolve(body, &duplicate).is_err(), "repeating a native edge cannot stand in for its opposite");
    let mut disconnected = inputs.clone();
    disconnected[0].0.ends[1].signatures = vec![[99; 8]];
    assert!(group::resolve(body, &disconnected).is_err(), "shared native endpoints must remain shared");
    let mut glued = inputs.clone();
    glued[0].0.ends[0] = glued[2].0.ends[1].clone();
    assert!(group::resolve(body, &glued).is_err(), "distinct physical endpoints cannot be glued together");
}

#[test]
fn limiting_shoulder_round_consumes_its_planar_walls() {
    let mut doc = crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/split-cylinder-cap-fillet.solidcraft")).unwrap();
    doc.change_param("grip_height", "5 mm", None, None).unwrap();
    doc.change_param("round_radius", "5 mm", None, None).unwrap();
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().filter_map(|r| r.error.as_ref()).collect::<Vec<_>>());
    let state = model.state();
    let body = state.body("Knob").unwrap();
    assert!(body.body.validity().is_empty());
    assert_eq!(body.body.crossed_face_loops().unwrap(), 0);
    for curve in ["l2", "l4"] {
        assert!(
            solidcraft_doc::naming::face_family_indices(body, &format!("F4:side:{curve}")).is_err(),
            "fully consumed wall {curve} must disappear"
        );
    }
    let a = 3_f64;
    let area = 2. * (a * (100. - a * a).sqrt() + 100. * (a / 10.).asin());
    let expected = std::f64::consts::PI * 100. * 8. + area * 5. + shoulder_added_volume(10., a, 5.);
    let actual = body.body.tessellate_with(1e-4, true).unwrap().measure().volume;
    assert!((actual - expected).abs() < 0.1, "limiting shoulder volume {actual} != {expected}");
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(doc, reopened);
    doc.change_param("round_radius", "4.9 mm", None, None).unwrap();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    for curve in ["l2", "l4"] {
        assert!(
            solidcraft_doc::naming::face_family_indices(state.body("Knob").unwrap(), &format!("F4:side:{curve}")).is_ok(),
            "a genuine remaining wall must not disappear"
        );
    }
    doc.change_param("round_radius", "5.1 mm", None, None).unwrap();
    model.evaluate(&doc);
    assert!(model.results.iter().any(|r| r.error.is_some()), "a round beyond the wall height must be refused");
}
