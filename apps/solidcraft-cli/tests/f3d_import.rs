//! Independent public-API measurements are local evidence, never committed assets.
use serde_json::{Value, json};
use solidcraft_engine::Session;

#[test]
#[ignore = "requires local native two-edge construction-point controls"]
fn native_two_edge_points_preserve_sources_names_and_edits() {
    use solidcraft_engine::doc::{
        Document, FeatureKind,
        construct::{ConstructGeom, GeoRef, PointDef},
    };
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for mode in ["body", "body-reversed", "sketch", "sketch-reversed"] {
        let check = |s: &mut Session, height: f64| {
            let m = volume(s, 0.6 * height, mode);
            assert!((m["total"]["volume_mm3"].as_f64().unwrap() - 600. * height).abs() < 0.01);
            let state = s.model.state();
            let [point] = state.construct.as_slice() else {
                panic!("missing point {mode}");
            };
            let ConstructGeom::Point(p) = point.geom else {
                panic!("wrong construction type");
            };
            assert!(p.dist(solidcraft_engine::geom::Vec3::new(0., 0., if mode.starts_with("body") { height } else { 0. })) < 1e-6, "{mode}: {p:?}");
            assert!(s.model.results.iter().all(|r| r.warning.is_none()));
        };
        for suffix in ["baseline", "height50", "renamed"] {
            let mut s = Session::default();
            s.execute("doc.open", &json!({"path":dir.join(format!("construction-two-edges-{mode}-{suffix}.f3d"))}))
                .unwrap_or_else(|e| panic!("{mode}/{suffix}: {e}"));
            assert_eq!(s.doc.features.len(), 3);
            assert_eq!(s.doc.features[2].name, if suffix == "renamed" { "Corner datum" } else { "Point1" });
            let FeatureKind::ConstructionPoint { def: PointDef::TwoEdges { a, b } } = &s.doc.features[2].kind else {
                panic!("point history missing");
            };
            for source in [a, b] {
                if mode.starts_with("body") {
                    assert!(matches!(source, GeoRef::Edge { name: Some(_), .. }));
                } else {
                    assert!(matches!(source, GeoRef::SketchCurve { .. }));
                }
            }
            check(&mut s, if suffix == "height50" { 50. } else { 40. });
            if suffix == "baseline" {
                let original = s.doc.to_json();
                s.execute("parameters.change", &json!({"name":"d1","expression":"65 mm"})).unwrap();
                check(&mut s, 65.);
                check(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), 65.);
                s.execute("edit.undo", &json!({})).unwrap();
                assert_eq!(s.doc.to_json(), original);
                check(&mut s, 40.);
            }
        }
    }
}

#[test]
#[ignore = "requires local native chamfer controls"]
fn chamfers_preserve_equal_distance_groups_and_source_edits() {
    use solidcraft_engine::doc::{Document, FeatureKind};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for mode in ["vertical", "bottom", "top", "pair", "tangent", "sets"] {
        let check = |s: &mut Session, d: f64, second: f64, height: f64, suppressed: bool| {
            let length = if matches!(mode, "bottom" | "top") { 20. } else { height };
            let removed = if suppressed {
                0.
            } else {
                length
                    * (d * d
                        + if mode == "pair" {
                            d * d
                        } else if mode == "sets" {
                            second * second
                        } else {
                            0.
                        })
                    / 2.
            };
            let expected = 20. * 30. * height - removed;
            let measured = volume(s, expected / 1000., mode);
            assert!((measured["total"]["volume_mm3"].as_f64().unwrap() - expected).abs() < 0.01, "{mode}: exact triangular prism removal");
            assert!(s.model.results.iter().all(|r| r.warning.is_none()), "{mode}: references must survive source edits");
        };
        for (suffix, d, height, suppressed) in [
            ("baseline", 2., 40., false),
            ("distance4", 4., 40., false),
            ("height50", 2., 50., false),
            ("suppressed", 2., 40., true),
            ("restored", 2., 40., false),
        ] {
            let mut s = Session::default();
            let path = dir.join(format!("chamfer-{mode}-{suffix}.f3d"));
            s.execute("doc.open", &json!({"path":path})).unwrap_or_else(|e| panic!("{mode}/{suffix}: {e}"));
            assert_eq!(s.doc.features.len(), 3);
            assert!(
                matches!(&s.doc.features[2].kind, FeatureKind::Chamfer { tangent_chain, groups, .. } if *tangent_chain == (mode == "tangent") && groups.len() == usize::from(mode == "sets"))
            );
            assert_eq!(s.doc.features[2].suppressed, suppressed);
            assert_eq!(s.doc.features[2].edge_names.len(), if matches!(mode, "pair" | "sets") { 2 } else { 1 });
            check(&mut s, d, 3., height, suppressed);
            if suffix == "baseline" {
                let original = s.doc.to_json();
                let mut edits = vec![(2, 0, "4 mm", 4., 3., 40.), (1, 0, "50 mm", 2., 3., 50.)];
                if mode == "sets" {
                    edits.push((2, 1, "5 mm", 2., 5., 40.));
                }
                for (feature, input, value, d, second, height) in edits {
                    let parameter = s.doc.features[feature].param_names[input].clone();
                    s.execute("parameters.change", &json!({"name":parameter,"expression":value})).unwrap();
                    check(&mut s, d, second, height, false);
                    check(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), d, second, height, false);
                    s.execute("edit.undo", &json!({})).unwrap();
                    assert_eq!(s.doc.to_json(), original);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires local native asymmetric chamfer controls"]
fn native_asymmetric_chamfers_preserve_setbacks_angles_and_edits() {
    use solidcraft_engine::doc::Document;
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for mode in ["two", "two-flipped", "angle", "angle-flipped"] {
        let check = |s: &mut Session, distance: f64, second: f64, angle: f64, height: f64, suppressed: bool| {
            let other = if mode.starts_with("two") { second } else { distance * angle.to_radians().tan() };
            let (x, y) = if mode.ends_with("flipped") { (distance, other) } else { (other, distance) };
            let removed = if suppressed { 0. } else { x * y * height / 2. };
            let base = 600. * height;
            let expected = base - removed;
            let m = volume(s, expected / 1000., mode);
            assert!((m["total"]["volume_mm3"].as_f64().unwrap() - expected).abs() < 0.01, "{mode}");
            for (axis, (center, cut_center)) in [(10., x / 3.), (15., y / 3.), (height / 2., height / 2.)].into_iter().enumerate() {
                let want = (base * center - removed * cut_center) / expected;
                assert!((m["bodies"][0]["center_of_mass"][axis].as_f64().unwrap() - want).abs() < 1e-6, "{mode} axis {axis}");
            }
            assert!(s.model.results.iter().all(|r| r.warning.is_none()));
        };
        for suffix in ["baseline", "distance4", "height50", "suppressed", "restored", if mode.starts_with("two") { "second5" } else { "angle60" }] {
            let mut session = Session::default();
            session
                .execute("doc.open", &json!({"path":dir.join(format!("chamfer-{mode}-{suffix}.f3d"))}))
                .unwrap_or_else(|e| panic!("{mode}/{suffix}: {e}"));
            let d = if suffix == "distance4" { 4. } else { 2. };
            let other = if suffix == "second5" { 5. } else { 3. };
            let a = if suffix == "angle60" { 60. } else { 30. };
            let h = if suffix == "height50" { 50. } else { 40. };
            check(&mut session, d, other, a, h, suffix == "suppressed");
            if suffix == "baseline" {
                let original = session.doc.to_json();
                for (name, value, d, other, a, h) in [
                    ("d3", "4 mm", 4., 3., 30., 40.),
                    ("d4", if mode.starts_with("two") { "5 mm" } else { "60 deg" }, 2., 5., 60., 40.),
                    ("d1", "50 mm", 2., 3., 30., 50.),
                ] {
                    session.execute("parameters.change", &json!({"name":name,"expression":value})).unwrap();
                    check(&mut session, d, other, a, h, false);
                    check(&mut Session::new(Document::from_json(&session.doc.to_json()).unwrap()), d, other, a, h, false);
                    session.execute("edit.undo", &json!({})).unwrap();
                    assert_eq!(session.doc.to_json(), original);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires local rotated and reordered chamfer controls"]
fn native_asymmetric_chamfer_orientation_matches_every_edge_in_reordered_frames() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let measurements: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-chamfer-frame-measurements.json")).unwrap()).unwrap();
    assert_eq!(measurements.as_object().unwrap().len(), 288);
    for (name, expected) in measurements.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let m = volume(&mut s, expected["volume_cm3"].as_f64().unwrap(), name);
        assert!((m["total"]["volume_mm3"].as_f64().unwrap() - expected["volume_cm3"].as_f64().unwrap() * 1000.).abs() < 0.01, "{name}");
        for axis in 0..3 {
            let actual = m["bodies"][0]["center_of_mass"][axis].as_f64().unwrap();
            let want = expected["centers_cm"][0][axis].as_f64().unwrap() * 10.;
            assert!((actual - want).abs() < 1e-6, "{name} axis {axis}: {actual} vs {want}");
        }
    }
}

#[test]
#[ignore = "requires local independent-radius fillet controls"]
fn grouped_fillets_preserve_native_radius_parameters_and_source_edits() {
    use solidcraft_engine::doc::{Document, FeatureKind, FilletStyle};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let check = |s: &mut Session, radii: [f64; 2], height: f64, suppressed: bool| {
        let removed = if suppressed { 0. } else { (1. - std::f64::consts::PI / 4.) * radii.iter().map(|r| r * r).sum::<f64>() * height };
        let expected = 20. * 30. * height - removed;
        let m = volume(s, expected / 1000., "grouped fillet");
        assert!((m["total"]["volume_mm3"].as_f64().unwrap() - expected).abs() < removed * 0.005 + 0.01);
        assert!(s.model.results.iter().all(|r| r.warning.is_none()));
    };
    for (suffix, radii, height, suppressed) in [
        ("baseline", [2., 3.], 40., false),
        ("radius4", [4., 3.], 40., false),
        ("height50", [2., 3.], 50., false),
        ("suppressed", [2., 3.], 40., true),
        ("restored", [2., 3.], 40., false),
    ] {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("fillet-sets-{suffix}.f3d"))})).unwrap();
        assert_eq!(s.doc.features.len(), 3);
        let feature = &s.doc.features[2];
        assert!(matches!(&feature.kind, FeatureKind::Fillet { style: FilletStyle::Grouped { groups }, .. } if groups.len()==1 && groups[0].count==1));
        assert_eq!(feature.param_names, ["d4", "d6"]);
        assert_eq!(feature.edge_names.len(), 2);
        assert_eq!(feature.suppressed, suppressed);
        check(&mut s, radii, height, suppressed);
        if suffix == "baseline" {
            let before = s.doc.to_json();
            for (parameter, value, radii, height) in [("d4", "4 mm", [4., 3.], 40.), ("d6", "5 mm", [2., 5.], 40.), ("d1", "50 mm", [2., 3.], 50.)] {
                s.execute("parameters.change", &json!({"name":parameter,"expression":value})).unwrap();
                check(&mut s, radii, height, false);
                check(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), radii, height, false);
                s.execute("edit.undo", &json!({})).unwrap();
                assert_eq!(s.doc.to_json(), before);
            }
        }
    }
}

#[test]
#[ignore = "requires local mixed-circle/line blend controls"]
fn native_mixed_profile_blends_preserve_selected_corners_and_edits() {
    use solidcraft_engine::io;
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for sign in [1., -1.] {
        for reverse in [0, 1] {
            for mode in ["fillet", "chamfer"] {
                let check = |s: &mut Session, size: f64, height: f64, label: &str| {
                    // Integral of top - lower circle over [-5,5].
                    let r: f64 = 30.;
                    let half: f64 = 5.;
                    let top: f64 = -25.;
                    let area = 2. * half * top + half * (r * r - half * half).sqrt() + r * r * (half / r).asin();
                    let moment = (top * top - r * r) * half + half.powi(3) / 3.;
                    let quarter = std::f64::consts::PI / 4.;
                    let (cut, inset) = if mode == "fillet" {
                        (2. * size * size * (1. - quarter), size * (5. / 6. - quarter) / (1. - quarter))
                    } else {
                        (size * size, size / 3.)
                    };
                    let want = (area - cut) * height.abs();
                    volume(s, want / 1000., label);
                    // The large circular carrier sets inspect.measure's chord
                    // tolerance; certify the smaller corner removal separately
                    // with a fixed fine chord tolerance in millimetres.
                    let state = s.model.state();
                    let [body] = state.bodies.as_slice() else { panic!("{label}: one solid") };
                    let m = body.body.tessellate(1e-5).unwrap().measure();
                    assert!(
                        (m.volume - want).abs() < cut * height.abs() * 0.005 + 0.01,
                        "{label}: removed material, actual={}, expected={want}",
                        m.volume
                    );
                    for (axis, want) in [0., (moment - cut * (top - inset)) / (area - cut), height / 2.].into_iter().enumerate() {
                        let got = [m.centroid.x, m.centroid.y, m.centroid.z][axis];
                        assert!((got - want).abs() < 2e-3, "{label}: centroid {axis}: {got} != {want}");
                    }
                    assert!(s.model.results.iter().all(|r| r.warning.is_none()));
                };
                for (suffix, size, height) in [("baseline", 2., 40. * sign), ("size1", 1., 40. * sign), ("height50", 2., 50. * sign)] {
                    let name = format!("mixed-{mode}-{sign}-{reverse}-{suffix}");
                    let mut s = Session::default();
                    s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
                    assert_eq!(s.doc.features.len(), 3);
                    assert_eq!(s.doc.features[2].edge_names.len(), 2);
                    check(&mut s, size, height, &name);
                    let before = s.doc.to_json();
                    for (feature, expression, size, height) in
                        [(2, "1.5 mm".into(), 1.5, height), (1, format!("{} mm", 60. * sign), size, 60. * sign)]
                    {
                        let parameter = s.doc.features[feature].param_names[0].clone();
                        s.execute("parameters.change", &json!({"name":parameter,"expression":expression})).unwrap();
                        check(&mut s, size, height, &name);
                        let bytes = io::write_design_container(&s.doc).unwrap();
                        let mut reopened = Session::new(io::read_design(&bytes).unwrap());
                        assert_eq!(s.doc, reopened.doc);
                        check(&mut reopened, size, height, &name);
                        s.execute("edit.undo", &json!({})).unwrap();
                        assert_eq!(s.doc.to_json(), before);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "requires local signed-extrusion fillet and chamfer controls"]
fn native_signed_extrusion_blends_preserve_caps_and_edits() {
    use solidcraft_engine::{doc::FeatureKind, io};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for sign in [1., -1.] {
        for mode in ["fillet", "chamfer"] {
            for location in ["vertical", "origin", "end"] {
                let check = |s: &mut Session, size: f64, height: f64, label: &str| {
                    let length = if location == "vertical" { height.abs() } else { 20. };
                    let (area, inset) = if mode == "fillet" {
                        let quarter = std::f64::consts::PI / 4.;
                        (size * size * (1. - quarter), size * (5. / 6. - quarter) / (1. - quarter))
                    } else {
                        (size * size / 2., size / 3.)
                    };
                    let whole = 600. * height.abs();
                    let removed = area * length;
                    let want = whole - removed;
                    let m = volume(s, want / 1000., label);
                    assert!((m["total"]["volume_mm3"].as_f64().unwrap() - want).abs() < removed * 0.005 + 0.01, "{label}: removed volume");
                    let center = [10., 15., height / 2.];
                    let cut_center = match location {
                        "vertical" => [inset, inset, height / 2.],
                        "origin" => [10., inset, height.signum() * inset],
                        _ => [10., inset, height - height.signum() * inset],
                    };
                    for axis in 0..3 {
                        let want = (whole * center[axis] - removed * cut_center[axis]) / (whole - removed);
                        let actual = m["bodies"][0]["center_of_mass"][axis].as_f64().unwrap();
                        assert!((actual - want).abs() < 2e-4, "{label}: axis {axis} center {actual} != {want}");
                    }
                    assert!(s.model.results.iter().all(|r| r.warning.is_none()), "{label}: persistent edge fallback");
                };
                for (suffix, height) in [("baseline", 40. * sign), ("height50", 50. * sign)] {
                    if sign > 0. && suffix != "baseline" {
                        continue;
                    }
                    let name = format!("signed-{mode}-{sign}-{location}-{suffix}");
                    let mut s = Session::default();
                    s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
                    assert_eq!(s.doc.features.len(), 3);
                    let feature = &s.doc.features[2];
                    assert!(matches!(&feature.kind, FeatureKind::Fillet { .. }) == (mode == "fillet"));
                    assert_eq!(feature.edge_names.len(), 1);
                    check(&mut s, 2., height, &name);
                    let before = s.doc.to_json();
                    for (index, expression, size, height) in [(2, "4 mm".into(), 4., height), (1, format!("{} mm", 60. * sign), 2., 60. * sign)] {
                        let parameter = s.doc.features[index].param_names[0].clone();
                        s.execute("parameters.change", &json!({"name":parameter,"expression":expression})).unwrap();
                        check(&mut s, size, height, &name);
                        let bytes = io::write_design_container(&s.doc).unwrap();
                        let mut reopened = Session::new(io::read_design(&bytes).unwrap());
                        assert_eq!(s.doc, reopened.doc);
                        check(&mut reopened, size, height, &name);
                        s.execute("edit.undo", &json!({})).unwrap();
                        assert_eq!(s.doc.to_json(), before);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "requires local constant-radius fillet controls"]
fn constant_fillets_retain_selected_edges_radius_and_source_edits() {
    use solidcraft_engine::doc::{Document, FeatureKind};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for mode in ["vertical", "bottom", "top", "pair", "tangent"] {
        let analytic = |radius: f64, height: f64, suppressed: bool| {
            let length = match mode {
                "vertical" | "tangent" => height,
                "pair" => 2. * height,
                _ => 20.,
            };
            (20. * 30. * height - if suppressed { 0. } else { (1. - std::f64::consts::PI / 4.) * radius.powi(2) * length }) / 1000.
        };
        let check = |s: &mut Session, r, h, suppressed, name: &str| {
            let want = analytic(r, h, suppressed);
            let measured = volume(s, want, name);
            // Scale tolerance to the removed material. A whole-block tolerance
            // could hide a fillet on a neighboring, differently sized edge.
            let tolerance = (20. * 30. * h - want * 1000.).abs() * 0.005 + 0.01;
            assert!((measured["total"]["volume_mm3"].as_f64().unwrap() - want * 1000.).abs() < tolerance, "{name}: fillet removal volume");
        };
        for (suffix, r, h, suppressed) in [
            ("baseline", 2., 40., false),
            ("radius4", 4., 40., false),
            ("height50", 2., 50., false),
            ("suppressed", 2., 40., true),
            ("restored", 2., 40., false),
        ] {
            let name = format!("fillet-{mode}-{suffix}");
            let mut s = Session::default();
            s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.doc.features.len(), 3);
            assert!(matches!(s.doc.features[2].kind, FeatureKind::Fillet { tangent_chain, .. } if tangent_chain == (mode == "tangent")));
            assert_eq!(s.doc.features[2].suppressed, suppressed);
            assert_eq!(s.doc.features[2].edge_names.len(), if mode == "pair" { 2 } else { 1 });
            check(&mut s, r, h, suppressed, &name);
            if suffix == "baseline" {
                let original = s.doc.to_json();
                for (feature, value, r, h) in [(2, "4 mm", 4., 40.), (1, "50 mm", 2., 50.)] {
                    let parameter = s.doc.features[feature].param_names[0].clone();
                    s.execute("parameters.change", &json!({"name":parameter,"expression":value})).unwrap();
                    check(&mut s, r, h, false, &name);
                    assert!(s.model.results.iter().all(|r| r.warning.is_none()), "{name}: edge references must not fall back to old points");
                    check(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), r, h, false, &name);
                    s.execute("edit.undo", &json!({})).unwrap();
                    assert_eq!(s.doc.to_json(), original);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires local Combine operation and retention controls"]
fn combine_bodies_keep_editable_sources_operations_and_tool_retention() {
    use solidcraft_engine::doc::{Document, FeatureKind, Operation};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for (mode, operation, base_volume) in [("join", Operation::Join, 12.), ("cut", Operation::Cut, 4.), ("intersect", Operation::Intersect, 4.)] {
        for keep in [false, true] {
            let name = format!("combine-{mode}-{}", if keep { "keep" } else { "consume" });
            let mut s = Session::default();
            s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
            assert_eq!(s.doc.features.len(), 5);
            assert!(matches!(&s.doc.features[4].kind, FeatureKind::Combine { target, tools, operation: op, keep_tools }
                if target == "Body1" && tools == &["Body2"] && *op == operation && *keep_tools == keep));
            let expected = base_volume + if keep { 8. } else { 0. };
            assert_eq!(volume(&mut s, expected, &name)["body_count"], if keep { 2 } else { 1 });
            let original = s.doc.to_json();
            // Heights remain native parameter references, not cached solid meshes.
            assert!(matches!(s.doc.features[1].kind, FeatureKind::Extrude { .. }));
            let distance = s.doc.features[1].param_names[0].clone();
            s.execute("parameters.change", &json!({"name":distance,"expression":"30 mm"})).unwrap();
            let edited = match operation {
                Operation::Join => 16.,
                Operation::Cut => 8.,
                _ => 4.,
            } + if keep { 8. } else { 0. };
            volume(&mut s, edited, &name);
            volume(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), edited, &name);
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(s.doc.to_json(), original);
            let feature = s.doc.features[4].name.clone();
            s.execute("timeline.suppress", &json!({"feature":feature,"suppressed":true})).unwrap();
            assert_eq!(volume(&mut s, 16., &name)["body_count"], 2);
            s.execute("edit.undo", &json!({})).unwrap();
            volume(&mut s, expected, &name);
        }
    }
    for (name, expected) in [("combine-suppressed", 16.), ("combine-restored", 12.)] {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        assert_eq!(s.doc.features[4].suppressed, name.ends_with("suppressed"));
        volume(&mut s, expected, name);
    }
    // Combined mirrors use a different mode; unsupported history remains atomic.
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length":10,"width":10,"height":10})).unwrap();
    let before = s.doc.to_json();
    assert!(s.execute("doc.open", &json!({"path":dir.join("mirror-mode-combine.f3d")})).is_err());
    assert_eq!(s.doc.to_json(), before);
}

#[test]
#[ignore = "requires local Combine joined, cut, and mirrored source controls"]
fn combine_modified_and_mirrored_sources_remain_editable() {
    use solidcraft_engine::doc::{Document, FeatureKind};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for source in ["join", "cut", "mirror", "mirror-target"] {
        for (mode, slot) in [("join", 0), ("cut", 1), ("intersect", 2)] {
            for keep in [false, true] {
                let name = format!("combine-source-{source}-{mode}-{}", if keep { "keep" } else { "consume" });
                let mut s = Session::default();
                s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
                let mirrored = source.starts_with("mirror");
                let base = match source {
                    "join" => [12., 4., 8.],
                    "cut" => [11.5, 3.5, 3.5],
                    _ => [12., 4., 4.],
                }[slot];
                let expected = base + if keep { 8. } else { 0. };
                let measure = volume(&mut s, expected, &name);
                assert_eq!(measure["body_count"], if keep { 2 } else { 1 });
                assert_eq!(s.doc.features.len(), if mirrored { 5 } else { 7 });
                let combined = s.doc.features.last().unwrap();
                assert!(matches!(&combined.kind, FeatureKind::Combine { target, tools, .. }
                    if target == if source == "mirror-target" { "Body2" } else { "Body1" }
                    && tools == &[if source == "mirror-target" { "Body1" } else { "Body2" }]));
                if source == "mirror-target" && mode == "cut" && !keep {
                    assert_eq!(measure["bodies"][0]["bbox"]["min"], json!([20., 0., 0.]));
                    assert_eq!(measure["bodies"][0]["bbox"]["max"], json!([30., 20., 20.]));
                }
                let feature = combined.name.clone();
                let original = s.doc.to_json();
                let distance = s.doc.features[1].param_names[0].clone();
                s.execute("parameters.change", &json!({"name":distance,"expression":"30 mm"})).unwrap();
                let changed = if mirrored {
                    base * 1.5 + if keep { 12. } else { 0. }
                } else {
                    base + if slot != 2 { 4. } else { 0. } + if keep { 8. } else { 0. }
                };
                volume(&mut s, changed, &name);
                volume(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), changed, &name);
                s.execute("edit.undo", &json!({})).unwrap();
                assert_eq!(s.doc.to_json(), original);
                if mirrored {
                    let plane = s.doc.features[2].param_names[0].clone();
                    s.execute("parameters.change", &json!({"name":plane,"expression":"17 mm"})).unwrap();
                    let changed = [13.6, 5.6, 2.4][slot] + if keep { 8. } else { 0. };
                    volume(&mut s, changed, &name);
                    volume(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), changed, &name);
                    s.execute("edit.undo", &json!({})).unwrap();
                }
                s.execute("timeline.suppress", &json!({"feature":feature,"suppressed":true})).unwrap();
                volume(
                    &mut s,
                    match source {
                        "join" => 20.,
                        "cut" => 15.,
                        _ => 16.,
                    },
                    &name,
                );
                s.execute("edit.undo", &json!({})).unwrap();
                volume(&mut s, expected, &name);
            }
        }
    }
}

fn volume(s: &mut Session, want_cm3: f64, label: &str) -> Value {
    for r in &s.model.results {
        assert!(r.error.is_none(), "{label}: feature {} ({}): {:?}", r.name, r.id, r.error);
    }
    let measure = s.execute("inspect.measure", &json!({})).unwrap();
    let got = measure["total"]["volume_mm3"].as_f64().unwrap();
    let want = want_cm3 * 1000.;
    assert!((got - want).abs() < want.abs().max(1.) * 1e-3, "{label}: volume {got} != {want}");
    measure
}

#[test]
#[ignore = "requires local public API operation deletion controls"]
fn operation_ownership_excludes_deleted_tags_and_retains_suppressed_producers() {
    use std::collections::{BTreeMap, BTreeSet};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-operation-lifecycle-measurements.json")).unwrap()).unwrap();
    let mut maps = BTreeMap::new();
    for (name, row) in api.as_object().unwrap() {
        let bytes = std::fs::read(dir.join(format!("{name}.f3d"))).unwrap();
        let native = solidcraft_engine::io::f3d::read_f3d(&bytes).unwrap();
        let native = &native[0];
        let owners = native.operation_owners().unwrap();
        let features: BTreeSet<_> = native.history().unwrap().iter().map(|f| f.id).collect();
        assert_eq!(owners.values().copied().collect::<BTreeSet<_>>(), features);
        let mut s = Session::new(native.to_document(name).unwrap());
        let measured = volume(&mut s, row["volume"].as_f64().unwrap(), name);
        assert_eq!(measured["body_count"], row["body_count"]);
        assert_eq!(
            s.doc.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            row["timeline"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect::<Vec<_>>()
        );
        maps.insert(name.as_str(), owners);
    }
    let baseline = &maps["operation-map-baseline"];
    assert_eq!(baseline, &maps["operation-map-suppressed"]);
    let deleted = &maps["operation-map-deleted"];
    let recreated = &maps["operation-map-recreated"];
    let retired: BTreeSet<_> = baseline.keys().filter(|tag| !deleted.contains_key(tag)).collect();
    assert_eq!(retired.len(), 2);
    assert!(retired.iter().all(|tag| !recreated.contains_key(tag)));
    assert!(deleted.iter().all(|(tag, feature)| recreated.get(tag) == Some(feature)));
    assert_eq!(recreated.len(), baseline.len());
}

#[test]
#[ignore = "requires local face-section loft public API controls"]
fn loft_face_sections_preserve_cap_body_and_parent_dependencies() {
    check_endpoint_histories(&[
        "fusion-face-loft-measurements.json",
        "fusion-face-loft-extra-measurements.json",
        "fusion-face-loft-rectangle-measurements.json",
        "fusion-face-loft-rectangle-joined-measurements.json",
        "fusion-face-loft-rectangle-side-measurements.json",
        "fusion-face-loft-sides-measurements.json",
        "fusion-compound-cap-measurements.json",
        "fusion-face-loft-compound-sides-measurements.json",
        "fusion-compound-polygons-measurements.json",
        "fusion-serial-join-measurements.json",
        "fusion-triple-join-measurements.json",
    ]);
}

#[test]
#[ignore = "requires local To Object public API controls"]
fn to_object_endpoints_preserve_target_source_and_offset_edits() {
    check_endpoint_histories(&[
        "fusion-to-entity-measurements.json",
        "fusion-to-entity-hint-measurements.json",
        "fusion-to-entity-opposite-hint-measurements.json",
        "fusion-to-entity-maximum-measurements.json",
        "fusion-to-entity-flip-measurements.json",
        "fusion-to-entity-legacy-measurements.json",
    ]);
}

fn check_endpoint_histories(files: &[&str]) {
    check_selected_histories(files, |_| true);
}

#[test]
#[ignore = "requires local public API body mirror controls"]
fn body_mirrors_preserve_sources_planes_suppression_and_edits() {
    check_selected_histories(&["fusion-mirror-measurements.json", "fusion-mirror-modes-measurements.json"], |name| {
        !name.contains("feature-plane") && !name.ends_with("-combine")
    });
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    {
        let name = "mirror-mode-combine";
        let mut s = Session::default();
        s.execute("solid.box", &json!({"length":5,"width":7,"height":9})).unwrap();
        let before = s.doc.clone();
        assert!(s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).is_err(), "{name}");
        assert_eq!(s.doc, before);
    }
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-mirror-modes-measurements.json")).unwrap()).unwrap();
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("mirror-mode-suppressed.f3d")})).unwrap();
    let original = s.doc.clone();
    s.execute("timeline.suppress", &json!({"feature":"Mirror1","suppressed":false})).unwrap();
    let measure = volume(&mut s, api["mirror-mode-separate"]["volume"].as_f64().unwrap(), "unsuppressed mirror");
    assert_eq!(measure["body_count"], 2);
    let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
    volume(&mut Session::new(reopened), api["mirror-mode-separate"]["volume"].as_f64().unwrap(), "reopened mirror");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, original);
}

#[test]
#[ignore = "requires local public API feature mirror controls"]
fn feature_mirrors_preserve_new_join_cut_operations_and_edits() {
    check_selected_histories(
        &["fusion-mirror-measurements.json", "fusion-feature-mirror-measurements.json", "fusion-feature-mirror-pair-measurements.json"],
        |name| name.contains("feature-plane") || name.starts_with("feature-mirror-"),
    );
}

fn check_selected_histories(files: &[&str], include: impl Fn(&str) -> bool) {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for file in files {
        let api: Value = serde_json::from_slice(&std::fs::read(dir.join(file)).unwrap()).unwrap();
        let verify = |s: &mut Session, row: &Value, label: &str| {
            let measured = volume(s, row["volume"].as_f64().unwrap(), label);
            assert_eq!(measured["body_count"], row["body_count"], "{label}");
            let mut bounds = measured["bodies"].as_array().unwrap().iter().map(|b| &b["bbox"]).collect::<Vec<_>>();
            for expected in row["bounds"].as_array().unwrap() {
                let i = bounds
                    .iter()
                    .position(|b| {
                        [(0, "min"), (1, "max")].into_iter().all(|(end, key)| {
                            (0..3).all(|axis| (b[key][axis].as_f64().unwrap() - expected[end][axis].as_f64().unwrap() * 10.).abs() < 0.01)
                        })
                    })
                    .unwrap_or_else(|| panic!("{label}: missing body bounds {expected}, got {bounds:?}"));
                bounds.remove(i);
            }
            assert_eq!(s.doc.features.len(), row["timeline"].as_array().unwrap().len());
            for (f, t) in s.doc.features.iter().zip(row["timeline"].as_array().unwrap()) {
                assert_eq!(f.name, t["name"].as_str().unwrap());
            }
        };
        for (name, row) in api.as_object().unwrap() {
            if !include(name) {
                continue;
            }
            let mut s = Session::default();
            s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
            verify(&mut s, row, name);
            verify(&mut Session::new(solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap()), row, name);
            if name.ends_with("-baseline") {
                let original = s.doc.clone();
                let edits: Vec<(String, String, String)> = if let Some(edits) = row["edits"].as_array() {
                    edits
                        .iter()
                        .map(|edit| {
                            (
                                edit["suffix"].as_str().unwrap().into(),
                                edit["parameter"].as_str().unwrap().into(),
                                edit["expression"].as_str().unwrap().into(),
                            )
                        })
                        .collect()
                } else {
                    [
                        ("source-changed", "source_parameter", if name.contains("rectangle") { "45 mm" } else { "30 mm" }),
                        ("parent-changed", "parent_parameter", "15 mm"),
                        (
                            "plane-changed",
                            "plane_parameter",
                            if name.contains("bottom") {
                                "-40 mm"
                            } else if name.contains("side") {
                                "70 mm"
                            } else {
                                "50 mm"
                            },
                        ),
                    ]
                    .into_iter()
                    .map(|(suffix, parameter, expression)| (suffix.into(), row[parameter].as_str().unwrap().into(), expression.into()))
                    .collect()
                };
                let edit_count = edits.len();
                for (suffix, parameter, expr) in edits {
                    s.execute("parameters.change", &json!({"name":parameter,"expression":expr})).unwrap();
                    let expected = &api[name.replace("baseline", &suffix)];
                    verify(&mut s, expected, &suffix);
                    verify(
                        &mut Session::new(solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap()),
                        expected,
                        &suffix,
                    );
                }
                for _ in 0..edit_count {
                    s.execute("edit.undo", &json!({})).unwrap();
                }
                assert_eq!(s.doc, original);
                verify(&mut s, row, name);
            }
        }
    }
}

#[test]
#[ignore = "requires local loft public API controls"]
fn loft_sections_follow_source_and_plane_edits() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-loft-measurements.json")).unwrap()).unwrap();
    let verify = |s: &mut Session, row: &Value, label: &str| {
        let measured = volume(s, row["volume"].as_f64().unwrap(), label);
        assert_eq!(measured["body_count"], row["body_count"], "{label}");
        for (end, key) in [(0, "min"), (1, "max")] {
            for axis in 0..3 {
                // Curved spline bounds are tessellated; use the same 0.01 mm
                // geometric resolution as the native comparison controls.
                assert!(
                    (measured["bodies"][0]["bbox"][key][axis].as_f64().unwrap() - row["bounds"][0][end][axis].as_f64().unwrap() * 10.).abs() < 0.01,
                    "{label}: bounds {key}/{axis}"
                );
            }
        }
        assert_eq!(s.doc.features.len(), row["timeline"].as_array().unwrap().len());
        for (f, t) in s.doc.features.iter().zip(row["timeline"].as_array().unwrap()) {
            assert_eq!(f.name, t["name"].as_str().unwrap());
        }
    };
    for (name, row) in api.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        verify(&mut s, row, name);
        verify(&mut Session::new(solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap()), row, name);
        if name.ends_with("-baseline") {
            let original = s.doc.clone();
            for (suffix, parameter, expression) in [("source-changed", "source_parameter", "30 mm"), ("plane-changed", "height_parameter", "40 mm")] {
                s.execute("parameters.change", &json!({"name":row[parameter],"expression":expression})).unwrap();
                let expected = &api[name.replace("baseline", suffix)];
                verify(&mut s, expected, suffix);
                verify(
                    &mut Session::new(solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap()),
                    expected,
                    suffix,
                );
            }
            for _ in 0..2 {
                s.execute("edit.undo", &json!({})).unwrap();
            }
            assert_eq!(s.doc, original);
            verify(&mut s, row, name);
        }
    }
    for mode in ["baseline", "reversed", "suppressed", "surface", "unmerged"] {
        let mut s = Session::default();
        let result = s.execute("doc.open", &json!({"path":dir.join(format!("loft-flags-{mode}.f3d"))}));
        if matches!(mode, "surface" | "unmerged") {
            assert!(result.is_err());
            assert!(s.doc.features.is_empty());
        } else {
            result.unwrap();
            volume(&mut s, if mode == "suppressed" { 0. } else { 7. * std::f64::consts::PI }, mode);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn native_import_reconstructs_and_regenerates_controlled_history() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    for source in [
        "fusion-measurements.json",
        "fusion-feature-measurements.json",
        "fusion-circle-measurements.json",
        "fusion-history-measurements.json",
        "fusion-extent-measurements.json",
        "fusion-start-offset-measurements.json",
        "fusion-offset-dimension-measurements.json",
        "fusion-angular-dimension-measurements.json",
        "fusion-angular-stable-measurements.json",
        "fusion-polygon-measurements.json",
        "fusion-loop-measurements.json",
        "fusion-arc-measurements.json",
        "fusion-arc-support-measurements.json",
        "fusion-plane-measurements.json",
        "fusion-user-parameter-measurements.json",
        "fusion-user-rectangle-measurements.json",
        "fusion-aligned-measurements.json",
        "fusion-connections-measurements.json",
        "fusion-projection-history-measurements.json",
        "fusion-projected-rectangle-measurements.json",
        "fusion-body-projection-measurements.json",
        "fusion-body-reference-top-measurements.json",
        "fusion-body-reference-bottom-measurements.json",
        "fusion-body-reference-second-measurements.json",
        "fusion-body-rectangle-top-measurements.json",
        "fusion-body-rectangle-bottom-measurements.json",
        "fusion-body-rectangle-second-measurements.json",
    ] {
        let cases: Value = serde_json::from_slice(&std::fs::read(dir.join(source)).unwrap()).unwrap();
        for (name, api) in cases.as_object().unwrap() {
            if name == "extent-two-sided" {
                continue;
            } // Dedicated unsupported-schema atomicity check below.
            let mut s = Session::default();
            let result = s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(result["editable_history_imported"], true);
            assert!(s.path.is_none(), "the source F3D must not become a JSON save target");
            assert!(s.is_dirty());
            let measure = volume(&mut s, api["volume"].as_f64().unwrap(), name);
            if let Some(bounds) = api["bounds"].as_array() {
                for (end, key) in [(0, "min"), (1, "max")] {
                    for axis in 0..3 {
                        let actual = measure["bodies"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|b| b["bbox"][key][axis].as_f64().unwrap())
                            .reduce(|a, b| if end == 0 { a.min(b) } else { a.max(b) })
                            .unwrap();
                        assert!((actual - bounds[end][axis].as_f64().unwrap() * 10.).abs() < 1e-5, "{name}: bounds {end}/{axis}");
                    }
                }
            }
            if let Some(timeline) = api["timeline"].as_array() {
                assert_eq!(s.doc.features.len(), timeline.len(), "{name}");
                for (f, t) in s.doc.features.iter().zip(timeline) {
                    assert_eq!(f.name, t["name"].as_str().unwrap(), "{name}");
                }
            }
            // Expressions are retained as dependencies, not replaced by cached values.
            let definitions = s.doc.all_param_exprs();
            for p in api["parameters"].as_array().unwrap() {
                let imported = definitions.iter().find(|i| i.0 == p["name"].as_str().unwrap()).unwrap();
                assert_eq!(imported.1, p["expression"].as_str().unwrap(), "{name}");
            }
            let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
            assert_eq!(doc, *s.doc, "{name}: round trip");
            volume(&mut Session::new(doc), api["volume"].as_f64().unwrap(), name);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn native_fixation_matches_independent_point_and_circle_controls() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-point-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
        assert_eq!(sketch.curves.len(), 1);
        assert_eq!(sketch.curves[0].fixed, api["circle_fixed"].as_bool().unwrap(), "{name}");
        assert_eq!(sketch.points.len(), api["points"].as_array().unwrap().len(), "{name}");
        assert_eq!(sketch.points[0].fixed, api["origin_fixed"].as_bool().unwrap(), "{name}");
        assert_eq!(sketch.points[1].fixed, api["center_fixed"].as_bool().unwrap(), "{name}");
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_CORPUS pointing to the supplied archives"]
fn supplied_minimal_imports_without_a_fusion_roundtrip() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS"));
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("Minimal.f3d")})).unwrap();
    assert_eq!(s.doc.features.len(), 2);
    volume(&mut s, std::f64::consts::PI * 5. * 5. * 10., "supplied Minimal");
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn imported_plane_dependencies_move_bodies_after_parameter_and_feature_edits() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    for (name, axis, child) in [("plane-xy-baseline", 2, 0.), ("plane-xz-baseline", 1, 0.), ("plane-chained-baseline", 2, 10.)] {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let original = s.doc.clone();
        assert_eq!(s.doc.features[0].param_names, ["d1"]);
        for (command, params, start) in [
            ("parameters.change", json!({"name":"d1","expression":"35 mm"}), 35. + child),
            ("timeline.edit", json!({"feature":s.doc.features[0].id,"set":{"plane":{"distance":"-20 mm"}}}), -20. + child),
        ] {
            s.execute(command, &params).unwrap();
            let measure = volume(&mut s, 1.5 * std::f64::consts::PI, name);
            assert!((measure["bodies"][0]["bbox"]["min"][axis].as_f64().unwrap() - start).abs() < 1e-5, "{name}");
            assert!((measure["bodies"][0]["bbox"]["max"][axis].as_f64().unwrap() - start - 15.).abs() < 1e-5, "{name}");
            let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
            assert_eq!(doc, *s.doc);
            let measure = volume(&mut Session::new(doc), 1.5 * std::f64::consts::PI, name);
            assert!((measure["bodies"][0]["bbox"]["min"][axis].as_f64().unwrap() - start).abs() < 1e-5, "{name}: reopen");
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(s.doc, original);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn imported_user_parameters_preserve_roles_comments_favorites_and_dependencies() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let mut cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-user-parameter-measurements.json")).unwrap()).unwrap();
    let rectangles: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-user-rectangle-measurements.json")).unwrap()).unwrap();
    cases.as_object_mut().unwrap().extend(rectangles.as_object().unwrap().clone());
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let rows = s.doc.param_rows();
        let inspected = s.execute("file.inspect_f3d", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        for p in api["parameters"].as_array().unwrap() {
            let actual = rows.iter().find(|r| r.name == p["name"].as_str().unwrap()).unwrap();
            let user = p["type"].as_str().unwrap().ends_with("::UserParameter");
            assert_eq!(actual.source == "user", user);
            assert_eq!(actual.comment, p["comment"].as_str().unwrap(), "{name}: {}", actual.name);
            assert_eq!(actual.favorite, p["favorite"].as_bool().unwrap());
            let record = inspected["designs"][0]["parameters"]["records"].as_array().unwrap().iter().find(|r| r["name"] == p["name"]).unwrap();
            assert_eq!(record["user"], user);
            assert_eq!(record["comment"], p["comment"]);
            assert_eq!(record["favorite"], p["favorite"]);
        }
        volume(&mut s, api["volume"].as_f64().unwrap(), name);
        let original = s.doc.clone();
        s.execute("parameters.change", &json!({"name":"base","expression":"30 mm"})).unwrap();
        let edited_volume = if name.starts_with("user-rectangle-") { 54. } else { 13.5 * std::f64::consts::PI };
        volume(&mut s, edited_volume, name);
        let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        assert_eq!(doc, *s.doc);
        volume(&mut Session::new(doc), edited_volume, name);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
        s.execute("parameters.rename", &json!({"name":"base","new_name":"size"})).unwrap();
        s.execute("parameters.change", &json!({"name":"size","expression":"20 mm"})).unwrap();
        s.execute("parameters.change", &json!({"name":"scale","expression":"3"})).unwrap();
        volume(&mut s, if name.starts_with("user-rectangle-") { 24. } else { 6. * std::f64::consts::PI }, name);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn imported_rectangle_dimensions_regenerate_and_undo() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("rectangle-baseline.f3d")})).unwrap();
    let original = s.doc.clone();
    for (name, expr, want) in [("d1", "45 mm", 36.), ("d2", "35 mm", 42.), ("d3", "55 mm", 33.), ("d1", "d2 * 2", 32.)] {
        s.execute("parameters.change", &json!({"name":name,"expression":expr})).unwrap();
        volume(&mut s, want, name);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(*s.doc, *original);
    }
    assert_eq!(s.doc.features[1].param_names, ["d3", "d4"]);
    s.execute("parameters.change", &json!({"name":"d1","expression":"d3 / 2"})).unwrap();
    let before = s.doc.clone();
    let extrude = s.doc.features[1].id;
    s.execute("timeline.edit", &json!({"feature":extrude,"set":{"extent":{"distance":"60 mm"}}})).unwrap();
    volume(&mut s, 36., "native feature input drives width");
    let rows = s.doc.param_rows();
    assert_eq!(rows.len(), 4);
    let height = rows.iter().find(|p| p.name == "d3").unwrap();
    assert_eq!(height.source, "feature");
    assert_eq!(height.expression, "60 mm");
    let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
    volume(&mut Session::new(reopened), 36., "reopened native feature input");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(*s.doc, *before);
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn aligned_dimensions_remain_editable_with_reversed_native_references() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-aligned-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
        assert_eq!(
            sketch.constraints.iter().filter(|c| matches!(c.kind, solidcraft_engine::doc::sketch::ConstraintKind::Distance { .. })).count(),
            1
        );
        volume(&mut s, api["volume"].as_f64().unwrap(), name);
        let original = s.doc.clone();
        s.execute("parameters.change", &json!({"name":"diagonal","expression":"65 mm"})).unwrap();
        let width: f64 = if name.contains("wide") { 4. } else { 3. };
        let depth = (6.5_f64.powi(2) - width.powi(2)).sqrt();
        let measure = volume(&mut s, width * depth, name);
        assert!((measure["bodies"][0]["bbox"]["max"][1].as_f64().unwrap() - depth * 10.).abs() < 1e-5, "{name}");
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        volume(&mut Session::new(reopened), width * depth, name);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn reordered_connections_and_shared_curves_preserve_editable_dimensions() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-connections-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        volume(&mut s, api["volume"].as_f64().unwrap(), name);
        let original = s.doc.clone();
        let (parameter, expression, want) = if name.contains("ring") {
            let parameter = api["parameters"].as_array().unwrap().iter().find(|p| p["expression"] == "60 mm").unwrap()["name"].as_str().unwrap();
            (parameter, "80 mm", 12. * std::f64::consts::PI)
        } else if name.ends_with("vertical-first") {
            let parameter = api["parameters"].as_array().unwrap().iter().find(|p| p["expression"] == "40 mm").unwrap()["name"].as_str().unwrap();
            (parameter, "55 mm", 16.5)
        } else {
            ("diagonal", "65 mm", 3. * (6.5_f64.powi(2) - 9.).sqrt())
        };
        s.execute("parameters.change", &json!({"name":parameter,"expression":expression})).unwrap();
        volume(&mut s, want, name);
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        volume(&mut Session::new(reopened), want, name);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn isolated_lines_keep_endpoints_fixation_and_construction() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-line-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
        let line = &sketch.curves[0];
        assert!(!line.fixed, "native line fixation must not immobilize its endpoints");
        let held = sketch.constraints.iter().any(|c| matches!(c.kind, solidcraft_engine::doc::sketch::ConstraintKind::LineAngle { l: 0, .. }));
        assert_eq!(held, api["fixed"].as_bool().unwrap());
        assert_eq!(line.construction, api["construction"].as_bool().unwrap());
        let solidcraft_engine::doc::sketch::CurveKind::Line { a, b } = line.kind else { panic!("line") };
        for (p, key) in [(a, "start"), (b, "end")] {
            let point = sketch.points[p].pos;
            assert!((point.x - api[key][0].as_f64().unwrap() * 10.).abs() < 1e-8);
            assert!((point.y - api[key][1].as_f64().unwrap() * 10.).abs() < 1e-8);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn driven_dimensions_cannot_silently_become_driving_constraints() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-dimension-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        let before = s.doc.clone();
        let model = s.model.state();
        let result = s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))}));
        if api["driving"].as_bool().unwrap() {
            result.unwrap_or_else(|e| panic!("{name}: {e}"));
        } else {
            assert!(result.unwrap_err().to_string().contains("driven parameter measurement dependencies"), "{name}");
            assert!(std::sync::Arc::ptr_eq(&before, &s.doc));
            assert!(std::sync::Arc::ptr_eq(&model, &s.model.state()));
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn imported_arcs_preserve_api_endpoints_and_independent_flags() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-arc-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
        let (curve, indices) = sketch
            .curves
            .iter()
            .find_map(
                |curve| {
                    if let solidcraft_engine::doc::sketch::CurveKind::Arc { c, a, b } = curve.kind { Some((curve, [c, a, b])) } else { None }
                },
            )
            .unwrap();
        assert!(!curve.fixed, "{name}: native arc fixation must not immobilize its endpoints");
        let ci = sketch.curve_index(&curve.id).unwrap();
        let held = sketch
            .constraints
            .iter()
            .any(|c| matches!(c.kind, solidcraft_engine::doc::sketch::ConstraintKind::Radius { c, .. } if c == ci) && c.param.is_none());
        assert_eq!(held, api["arc"]["fixed"].as_bool().unwrap(), "{name}");
        assert_eq!(curve.construction, api["arc"]["construction"].as_bool().unwrap(), "{name}");
        for (index, key) in indices.into_iter().zip(["center", "start", "end"]) {
            let p = sketch.points[index].pos;
            assert!((p.x - api["arc"][key][0].as_f64().unwrap() * 10.).abs() < 1e-8, "{name}: {key} x");
            assert!((p.y - api["arc"][key][1].as_f64().unwrap() * 10.).abs() < 1e-8, "{name}: {key} y");
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn imported_hole_dimensions_regenerate_reopen_and_undo() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("ring-baseline.f3d")})).unwrap();
    let original = s.doc.clone();
    for (name, expression, pi) in [("d1", "70 mm", 22.5), ("d2", "30 mm", 13.5), ("d3", "35 mm", 28.)] {
        s.execute("parameters.change", &json!({"name":name,"expression":expression})).unwrap();
        volume(&mut s, pi * std::f64::consts::PI, "edited annulus");
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        volume(&mut Session::new(reopened), pi * std::f64::consts::PI, "reopened annulus");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(*s.doc, *original);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn native_parameters_remain_editable_and_failed_import_is_atomic() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("minimal-roundtrip.f3d")})).unwrap();
    let original = s.doc.clone();
    for (name, expression, want) in [
        ("d1", "80 mm", std::f64::consts::PI * 4. * 4. * 10.),
        ("d2", "125 mm", std::f64::consts::PI * 5. * 5. * 12.5),
        ("d2", "d1 / 2", std::f64::consts::PI * 5. * 5. * 5.),
    ] {
        s.execute("parameters.change", &json!({"name":name,"expression":expression})).unwrap();
        volume(&mut s, want, "edit");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(*s.doc, *original);
    }
    let before = s.doc.clone();
    let model = s.model.state();
    assert!(s.execute("doc.open", &json!({"path":dir.join("extent-two-sided.f3d")})).is_err());
    assert!(std::sync::Arc::ptr_eq(&before, &s.doc));
    assert!(std::sync::Arc::ptr_eq(&model, &s.model.state()));
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn radius_and_arc_diameter_controls_preserve_values_edits_and_reopening() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-radial-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        let before = s.doc.clone();
        let model = s.model.state();
        let result = s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))}));
        if !api["driving"].as_bool().unwrap() {
            assert!(result.unwrap_err().to_string().contains("driven parameter measurement dependencies"), "{name}");
            assert!(std::sync::Arc::ptr_eq(&before, &s.doc));
            assert!(std::sync::Arc::ptr_eq(&model, &s.model.state()));
            continue;
        }
        result.unwrap_or_else(|e| panic!("{name}: {e}"));
        volume(&mut s, api["volume"].as_f64().unwrap(), name);
        let radial = !name.starts_with("arc-diameter");
        let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
        let dimension = sketch.constraints.iter().find(|c| c.param.as_deref() == Some("d1")).unwrap();
        assert_eq!(matches!(dimension.kind, solidcraft_engine::doc::sketch::ConstraintKind::Radius { .. }), radial, "{name}");
        let original = s.doc.clone();
        s.execute("parameters.change", &json!({"name":"d1","expression":if radial {"40 mm"} else {"80 mm"}})).unwrap();
        let want = if name.starts_with("circle") { 32. } else { 16. } * std::f64::consts::PI;
        volume(&mut s, want, name);
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        volume(&mut Session::new(reopened), want, name);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(*s.doc, *original);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to controlled local Fusion exports"]
fn geometric_controls_preserve_relationships_and_remain_solvable_after_edits() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-constraint-measurements.json")).unwrap()).unwrap();
    for (name, api) in cases.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
        assert_eq!(sketch.constraints.len(), api["constraints"].as_array().unwrap().len(), "{name}");
        assert_eq!(sketch.points.len(), api["points"].as_array().unwrap().len(), "{name}");
        assert_eq!(sketch.curves.len(), api["curves"].as_array().unwrap().len(), "{name}");
        if name.ends_with("-constrained") {
            let native = name.strip_prefix("constraint-").unwrap().strip_suffix("-constrained").unwrap();
            let expected = match native {
                "equal-lines" | "equal-circles" => "equal",
                "point-on-line" => "point_on_curve",
                "horizontal-points" => "horizontal_points",
                "vertical-points" => "vertical_points",
                other => other,
            };
            assert_eq!(serde_json::to_value(&sketch.constraints[0]).unwrap()["type"], expected, "{name}");
            let mut edited = sketch.clone();
            for (i, point) in edited.points.iter_mut().enumerate().filter(|(_, p)| !p.fixed) {
                point.pos.x += (i % 3) as f64 * 0.3;
                point.pos.y -= (i % 2) as f64 * 0.2;
            }
            let report = solidcraft_engine::doc::sketch::solve(&mut edited);
            assert!(report.ok(), "{name}: {report:?}");
            assert!(report.max_residual < 1e-6, "{name}");
        }
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        assert_eq!(*s.doc, reopened, "{name}");
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with projected-circle history controls"]
fn projected_history_follows_source_edits_after_reopening() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-projection-history-measurements.json")).unwrap()).unwrap();
    let base = &cases["projection-history-base"];
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("projection-history-base.f3d")})).unwrap();
    let original = s.doc.clone();
    let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[1].kind else { panic!("projected sketch") };
    assert_eq!(sketch.links.len(), 1);
    assert!(sketch.curves[0].link.is_some());
    for (parameter, expression, expected) in
        [("diameter_parameter", "60 mm", "projection-history-diameter60"), ("height_parameter", "45 mm", "projection-history-height45")]
    {
        s.execute("parameters.change", &json!({"name":base[parameter],"expression":expression})).unwrap();
        volume(&mut s, cases[expected]["volume"].as_f64().unwrap(), expected);
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        let mut reopened = Session::new(reopened);
        volume(&mut reopened, cases[expected]["volume"].as_f64().unwrap(), expected);
        reopened.execute("parameters.change", &json!({"name":base["diameter_parameter"],"expression":"80 mm"})).unwrap();
        volume(&mut reopened, 16. * std::f64::consts::PI * if parameter == "height_parameter" { 4.5 } else { 3. }, "edit after reopen");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
        volume(&mut s, base["volume"].as_f64().unwrap(), "undo projection edit");
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with body-reference controls"]
fn projected_body_circle_keeps_its_body_through_edits_and_reopening() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    for case in ["top", "bottom", "second"] {
        let name = format!("body-reference-{case}");
        let api: Value = serde_json::from_slice(&std::fs::read(dir.join(format!("fusion-{name}-measurements.json"))).unwrap()).unwrap();
        let api = &api[&name];
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        let source = s.doc.find_feature(api["source_feature"].as_str().unwrap()).unwrap();
        let producer = source.id;
        let height = source.param_names[0].clone();
        let projected = s
            .doc
            .features
            .iter()
            .filter_map(|f| match &f.kind {
                solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } if !sketch.links.is_empty() => Some(f.id),
                _ => None,
            })
            .next()
            .unwrap();
        let verify = |s: &Session| {
            let state = s.model.state();
            let sketch = &state.sketch(projected).unwrap().sketch;
            assert_eq!(sketch.curves.len(), 1);
            assert!(!sketch.links[0].lost);
            assert_eq!(sketch.links[0].edge_circle.len(), 2);
            let role = if case == "bottom" { "start" } else { "top" };
            assert!(sketch.links[0].edge_circle.iter().all(|n| n.contains(&format!("F{producer}:{role}"))));
            assert!(
                matches!(&sketch.links[0].source, solidcraft_engine::doc::sketch::LinkSource::Edge {body,..} if body==api["source_body"].as_str().unwrap())
            );
        };
        verify(&s);
        s.execute("parameters.change", &json!({"name":api["diameter_parameter"],"expression":"160 mm"})).unwrap();
        s.execute("parameters.change", &json!({"name":height,"expression":"5 mm"})).unwrap();
        verify(&s);
        let expected = 64. * std::f64::consts::PI * if case == "second" { 8.5 } else { 3.5 };
        volume(&mut s, expected, &name);
        let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        let mut reopened = Session::new(reopened);
        verify(&reopened);
        volume(&mut reopened, expected, &name);
        reopened.execute("parameters.change", &json!({"name":api["diameter_parameter"],"expression":"60 mm"})).unwrap();
        verify(&reopened);
        volume(&mut reopened, expected * 9. / 64., &name);
        s.execute("edit.undo", &json!({})).unwrap();
        verify(&s);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with projected-rectangle history controls"]
fn projected_rectangle_keeps_shared_source_vertices_through_edits() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-projected-rectangle-measurements.json")).unwrap()).unwrap();
    let base = &cases["projected-rectangle-baseline"];
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("projected-rectangle-baseline.f3d")})).unwrap();
    let original = s.doc.clone();
    let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &s.doc.features[1].kind else { panic!("projected sketch") };
    assert_eq!(sketch.links.len(), 4);
    for (parameter, expression, expected) in [("width_parameter", "45 mm", 36.), ("depth_parameter", "35 mm", 42.)] {
        s.execute("parameters.change", &json!({"name":base[parameter],"expression":expression})).unwrap();
        volume(&mut s, expected, "projected rectangle edit");
        let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        volume(&mut Session::new(doc), expected, "projected rectangle reopened");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
        volume(&mut s, 24., "projected rectangle undo");
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with rectangular body-edge controls"]
fn projected_body_rectangle_keeps_corners_and_body_identity_through_edits() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    for case in ["top", "bottom", "second"] {
        let name = format!("body-rectangle-{case}");
        let api: Value = serde_json::from_slice(&std::fs::read(dir.join(format!("fusion-{name}-measurements.json"))).unwrap()).unwrap();
        let base = &api[format!("{name}-baseline")];
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}-baseline.f3d"))})).unwrap();
        let original = s.doc.clone();
        let producer = s.doc.find_feature(base["source_feature"].as_str().unwrap()).unwrap().id;
        let verify = |s: &Session| {
            let state = s.model.state();
            let sk = state.sketch(if case == "second" { 4 } else { 3 }).unwrap();
            assert_eq!(sk.sketch.curves.len(), 4);
            assert_eq!(sk.sketch.links.len(), 4);
            for link in &sk.sketch.links {
                assert!(!link.lost);
                let role = if case == "bottom" { "start" } else { "top" };
                assert!(link.edge_name.as_ref().unwrap().contains(&format!("F{producer}:{role}")));
                assert!(
                    matches!(&link.source, solidcraft_engine::doc::sketch::LinkSource::Edge {body,..} if body==base["source_body"].as_str().unwrap())
                );
            }
        };
        verify(&s);
        for (param, expression, stage) in [
            ("width_parameter", "45 mm", "width45"),
            ("depth_parameter", "35 mm", "depth35"),
            ("source_height_parameter", "10 mm", "source-height10"),
        ] {
            s.execute("parameters.change", &json!({"name":base[param], "expression":expression})).unwrap();
            verify(&s);
            let expected = api[format!("{name}-{stage}")]["volume"].as_f64().unwrap();
            volume(&mut s, expected, &name);
            let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
            let mut reopened = Session::new(doc);
            verify(&reopened);
            volume(&mut reopened, expected, &name);
        }
        for _ in 0..3 {
            s.execute("edit.undo", &json!({})).unwrap();
        }
        assert_eq!(s.doc, original);
        verify(&s);
        volume(&mut s, base["volume"].as_f64().unwrap(), &name);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with native extrusion start-offset controls"]
fn extrusion_start_offset_expressions_follow_edits_reopening_and_undo() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("start-offset-expression.f3d")})).unwrap();
    let original = s.doc.clone();
    let verify = |s: &mut Session, volume_cm3: f64, lo: f64, hi: f64| {
        let measure = volume(s, volume_cm3, "start offset");
        assert!((measure["bodies"][0]["bbox"]["min"][2].as_f64().unwrap() - lo).abs() < 1e-6);
        assert!((measure["bodies"][0]["bbox"]["max"][2].as_f64().unwrap() - hi).abs() < 1e-6);
    };
    s.execute("parameters.change", &json!({"name":"d1","expression":"60 mm"})).unwrap();
    verify(&mut s, 90. * std::f64::consts::PI, 6., 106.);
    s.execute("parameters.change", &json!({"name":"d4","expression":"-d2 / 4"})).unwrap();
    s.execute("parameters.change", &json!({"name":"d2","expression":"80 mm"})).unwrap();
    verify(&mut s, 72. * std::f64::consts::PI, -20., 60.);
    let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
    verify(&mut Session::new(reopened), 72. * std::f64::consts::PI, -20., 60.);
    for _ in 0..3 {
        s.execute("edit.undo", &json!({})).unwrap();
    }
    assert_eq!(s.doc, original);
    verify(&mut s, 250. * std::f64::consts::PI, 10., 110.);
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with independent symmetry controls"]
fn symmetry_history_preserves_points_lines_circles_and_solid_edits() {
    use solidcraft_engine::doc::{Document, FeatureKind, sketch::ConstraintKind};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for (kind, symmetric, equal) in [("points", 1, 0), ("lines", 2, 0), ("circles", 1, 1)] {
        for suffix in ["baseline", "constrained", "restored"] {
            let mut s = Session::default();
            s.execute("doc.open", &json!({"path":dir.join(format!("symmetry-{kind}-{suffix}.f3d"))})).unwrap();
            let FeatureKind::Sketch { sketch, .. } = &s.doc.features[0].kind else { panic!("sketch") };
            assert_eq!(
                sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Symmetric { .. })).count(),
                if suffix == "constrained" { symmetric } else { 0 }
            );
            assert_eq!(
                sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Equal { .. })).count(),
                if suffix == "constrained" { equal } else { 0 }
            );
            assert_eq!(Document::from_json(&s.doc.to_json()).unwrap(), *s.doc);
        }
    }
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-symmetry-solid-measurements.json")).unwrap()).unwrap();
    for (name, expected) in api.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        let m = volume(&mut s, expected["volume"].as_f64().unwrap(), name);
        assert_eq!(m["body_count"], 2);
    }
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("symmetry-solids-base.f3d")})).unwrap();
    s.execute("parameters.change", &json!({"name":"d1","expression":"30 mm"})).unwrap();
    volume(&mut s, 13.5 * std::f64::consts::PI, "symmetry diameter edit");
    let mut reopened = Session::new(Document::from_json(&s.doc.to_json()).unwrap());
    reopened.execute("parameters.change", &json!({"name":"d1","expression":"40 mm"})).unwrap();
    volume(&mut reopened, 24. * std::f64::consts::PI, "symmetry after reopening");
    s.execute("edit.undo", &json!({})).unwrap();
    volume(&mut s, 6. * std::f64::consts::PI, "symmetry undo");
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with native offset dimension controls"]
fn offset_dimensions_follow_api_bounds_through_edits_reopening_and_undo() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-offset-dimension-measurements.json")).unwrap()).unwrap();
    for (name, base) in api.as_object().unwrap().iter().filter(|(n, _)| n.ends_with("-baseline")) {
        let changed = &api[name.replace("-baseline", "-changed")];
        let verify = |s: &mut Session, expected: &Value| {
            let m = volume(s, expected["volume"].as_f64().unwrap(), name);
            assert_eq!(m["body_count"], 1);
            for (end, key) in [(0, "min"), (1, "max")] {
                for axis in 0..3 {
                    let got = m["bodies"][0]["bbox"][key][axis].as_f64().unwrap();
                    let want = expected["bounds"][end][axis].as_f64().unwrap() * 10.;
                    assert!((got - want).abs() < 1e-5, "{name}: {key}/{axis}: {got} != {want}");
                }
            }
        };
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        let original = s.doc.clone();
        verify(&mut s, base);
        s.execute("parameters.change", &json!({"name":base["parameter"],"expression":changed["expression"]})).unwrap();
        verify(&mut s, changed);
        let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
        verify(&mut Session::new(doc), changed);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
        verify(&mut s, base);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with angular quadrant and boundary controls"]
fn angular_dimensions_preserve_quadrants_and_boundary_edit_directions() {
    use solidcraft_engine::doc::sketch::CurveKind;
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for file in ["fusion-angular-stable-measurements.json", "fusion-angular-boundary-measurements.json"] {
        let api: Value = serde_json::from_slice(&std::fs::read(dir.join(file)).unwrap()).unwrap();
        for (name, base) in api.as_object().unwrap().iter().filter(|(n, _)| n.ends_with("-baseline")) {
            let changed = &api[name.replace("-baseline", "-changed")];
            let verify = |s: &mut Session, expected: &Value| {
                if let Some(v) = expected["volume"].as_f64() {
                    volume(s, v, name);
                }
                let state = s.model.state();
                let sketch = &state.sketches[0].sketch;
                let lines = expected["lines"].as_array().unwrap();
                assert_eq!(sketch.curves.len(), lines.len());
                for curve in &sketch.curves {
                    let CurveKind::Line { a, b } = curve.kind else { panic!("finite line") };
                    let actual = [sketch.points[a].pos, sketch.points[b].pos];
                    assert!(
                        lines.iter().any(|line| {
                            ["start", "end"].iter().enumerate().all(|(i, key)| {
                                (actual[i].x - line[key][0].as_f64().unwrap() * 10.).abs() < 1e-5
                                    && (actual[i].y - line[key][1].as_f64().unwrap() * 10.).abs() < 1e-5
                            })
                        }),
                        "{name}: unmatched line {actual:?}"
                    );
                }
            };
            let mut s = Session::default();
            s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
            let original = s.doc.clone();
            verify(&mut s, base);
            s.execute("parameters.change", &json!({"name":base["parameter"],"expression":changed["expression"]})).unwrap();
            verify(&mut s, changed);
            let doc = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
            verify(&mut Session::new(doc), changed);
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(s.doc, original);
            verify(&mut s, base);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with independently fixed line/point controls"]
fn fixed_line_support_preserves_free_endpoint_dimension_edits() {
    use solidcraft_engine::doc::sketch::CurveKind;
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-line-support-measurements.json")).unwrap()).unwrap();
    let base = &api["fixed-line-support-baseline"];
    let changed = &api["fixed-line-support-changed"];
    assert_eq!(base["fixed"], true);
    assert_eq!(base["start_fixed"], true);
    assert_eq!(base["end_fixed"], false);
    let verify = |s: &Session, expected: &Value| {
        let state = s.model.state();
        let sk = &state.sketches[0].sketch;
        let CurveKind::Line { a, b } = sk.curves[0].kind else { panic!("line") };
        for (point, key) in [(a, "start"), (b, "end")] {
            let p = sk.points[point].pos;
            assert!((p.x - expected[key][0].as_f64().unwrap() * 10.).abs() < 1e-5);
            assert!((p.y - expected[key][1].as_f64().unwrap() * 10.).abs() < 1e-5);
        }
    };
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("fixed-line-support-baseline.f3d")})).unwrap();
    let original = s.doc.clone();
    verify(&s, base);
    s.execute("parameters.change", &json!({"name":base["parameter"],"expression":changed["expression"]})).unwrap();
    verify(&s, changed);
    let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
    verify(&Session::new(reopened), changed);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, original);
    verify(&s, base);
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with independently fixed arc/point controls"]
fn fixed_arc_support_preserves_endpoint_edits_volume_and_bounds() {
    use solidcraft_engine::doc::sketch::CurveKind;
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-arc-support-measurements.json")).unwrap()).unwrap();
    let base = &api["fixed-arc-support-baseline"];
    let changed = &api["fixed-arc-support-changed"];
    assert_eq!(base["arc"]["fixed"], true);
    assert_eq!(base["arc"]["start_fixed"], true);
    assert_eq!(base["arc"]["end_fixed"], false);
    let verify = |s: &mut Session, expected: &Value| {
        let measure = volume(s, expected["volume"].as_f64().unwrap(), "fixed arc support");
        for (end, key) in [(0, "min"), (1, "max")] {
            for axis in 0..3 {
                let got = measure["bodies"][0]["bbox"][key][axis].as_f64().unwrap();
                assert!((got - expected["bounds"][end][axis].as_f64().unwrap() * 10.).abs() < 1e-5);
            }
        }
        let state = s.model.state();
        let sk = &state.sketches[0].sketch;
        let (c, a, b) = sk.curves.iter().find_map(|curve| if let CurveKind::Arc { c, a, b } = curve.kind { Some((c, a, b)) } else { None }).unwrap();
        for (point, key) in [(c, "center"), (a, "start"), (b, "end")] {
            let p = sk.points[point].pos;
            assert!((p.x - expected["arc"][key][0].as_f64().unwrap() * 10.).abs() < 1e-5);
            assert!((p.y - expected["arc"][key][1].as_f64().unwrap() * 10.).abs() < 1e-5);
        }
    };
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("fixed-arc-support-baseline.f3d")})).unwrap();
    let original = s.doc.clone();
    verify(&mut s, base);
    s.execute("parameters.change", &json!({"name":base["parameter"],"expression":changed["expression"]})).unwrap();
    verify(&mut s, changed);
    let reopened = solidcraft_engine::io::read_design(&solidcraft_engine::io::write_design(&s.doc)).unwrap();
    verify(&mut Session::new(reopened), changed);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, original);
    verify(&mut s, base);
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with symmetry creation-order controls"]
fn symmetry_roles_survive_axis_creation_order_and_reversed_entity_pairs() {
    use solidcraft_engine::doc::{
        Document,
        sketch::{ConstraintKind, CurveKind},
    };
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-symmetry-order-measurements.json")).unwrap()).unwrap();
    assert_eq!(api.as_object().unwrap().len(), 30);
    let verify = |s: &mut Session, expected: &Value| {
        assert!(s.model.results.iter().all(|r| r.error.is_none()));
        let state = s.model.state();
        let sketch = &state.sketches[0].sketch;
        let symmetries: Vec<_> =
            sketch.constraints.iter().filter_map(|c| if let ConstraintKind::Symmetric { l, .. } = c.kind { Some(l) } else { None }).collect();
        assert_eq!(symmetries.len(), if expected["kind"] == "lines" { 2 } else { 1 });
        for l in symmetries {
            let CurveKind::Line { a, b } = sketch.curves[l].kind else { panic!("axis") };
            for (point, key) in [(a, "start"), (b, "end")] {
                let p = sketch.points[point].pos;
                assert!((p.x - expected["axis"][key][0].as_f64().unwrap() * 10.).abs() < 1e-5);
                assert!((p.y - expected["axis"][key][1].as_f64().unwrap() * 10.).abs() < 1e-5);
            }
        }
        for entity in expected["entities"].as_array().unwrap() {
            for key in ["position", "start", "end", "center"] {
                let Some(pos) = entity[key].as_array() else { continue };
                assert!(
                    sketch.points.iter().any(|p| {
                        (p.pos.x - pos[0].as_f64().unwrap() * 10.).abs() < 1e-5 && (p.pos.y - pos[1].as_f64().unwrap() * 10.).abs() < 1e-5
                    }),
                    "missing {key}: {entity}"
                );
            }
        }
        if expected["kind"] == "circles" {
            let m = volume(s, expected["volume"].as_f64().unwrap(), "symmetry creation order");
            assert_eq!(m["body_count"], expected["body_count"]);
            for (end, key) in [(0, "min"), (1, "max")] {
                for axis in 0..3 {
                    let extremes = |values: Vec<f64>| values.into_iter().reduce(|a, b| if end == 0 { a.min(b) } else { a.max(b) }).unwrap();
                    let actual = extremes(m["bodies"].as_array().unwrap().iter().map(|b| b["bbox"][key][axis].as_f64().unwrap()).collect());
                    let want = extremes(expected["bounds"].as_array().unwrap().iter().map(|b| b[end][axis].as_f64().unwrap() * 10.).collect());
                    assert!((actual - want).abs() < 1e-5);
                }
            }
        }
    };
    for (name, expected) in api.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        verify(&mut s, expected);
        let original = s.doc.clone();
        verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), expected);
        if expected["kind"] == "circles" && name.ends_with("-baseline") {
            let changed = &api[name.replace("-baseline", "-changed")];
            s.execute("parameters.change", &json!({"name":"d1","expression":"30 mm"})).unwrap();
            verify(&mut s, changed);
            verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), changed);
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(s.doc, original);
            verify(&mut s, expected);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with isolated native line-midpoint controls"]
fn line_midpoints_preserve_rotation_and_reject_center_translation() {
    use solidcraft_engine::doc::{
        Document,
        sketch::{ConstraintKind, CurveKind},
    };
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-midpoint-isolation-measurements.json")).unwrap()).unwrap();
    let verify = |s: &Session, expected: &Value| {
        assert!(s.model.results.iter().all(|r| r.error.is_none()));
        let state = s.model.state();
        let sk = &state.sketches[0].sketch;
        let constrained = !expected["constraints"].as_array().unwrap().is_empty();
        assert_eq!(sk.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Midpoint { .. })).count(), if constrained { 2 } else { 0 });
        for key in ["outer", "inner"] {
            assert!(
                sk.curves.iter().any(|c| {
                    let CurveKind::Line { a, b } = c.kind else { return false };
                    [(a, "start"), (b, "end")].iter().all(|(p, end)| {
                        let point = sk.points[*p].pos;
                        (point.x - expected[key][end][0].as_f64().unwrap() * 10.).abs() < 1e-5
                            && (point.y - expected[key][end][1].as_f64().unwrap() * 10.).abs() < 1e-5
                    })
                }),
                "missing {key}: {expected}"
            );
        }
    };
    for suffix in ["baseline", "translated", "endpoint-moved", "constraint-deleted", "unconstrained-translated"] {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("midpoint-isolated-{suffix}.f3d"))})).unwrap_or_else(|e| panic!("{suffix}: {e}"));
        verify(&s, &api[suffix]);
        verify(&Session::new(Document::from_json(&s.doc.to_json()).unwrap()), &api[suffix]);
        if suffix == "baseline" {
            let original = s.doc.clone();
            s.execute("sketch.edit", &json!({"sketch":s.doc.features[0].id})).unwrap();
            s.execute("sketch.move", &json!({"entities":["c250"],"translate":[0,10]})).unwrap();
            s.execute("sketch.finish", &json!({})).unwrap();
            verify(&s, &api["translated"]);
            verify(&Session::new(Document::from_json(&s.doc.to_json()).unwrap()), &api["translated"]);
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(s.doc, original);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with native line-midpoint extrusion controls"]
fn line_midpoint_profile_edits_preserve_volume_bounds_and_history() {
    use solidcraft_engine::doc::Document;
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-midpoint-solid-measurements.json")).unwrap()).unwrap();
    let verify = |s: &mut Session, expected: &Value| {
        let m = volume(s, expected["volume"].as_f64().unwrap(), "line-midpoint extrusion");
        assert_eq!(m["body_count"], 1);
        for (end, key) in [(0, "min"), (1, "max")] {
            for axis in 0..3 {
                assert!((m["bodies"][0]["bbox"][key][axis].as_f64().unwrap() - expected["bounds"][end][axis].as_f64().unwrap() * 10.).abs() < 1e-5);
            }
        }
    };
    for (suffix, expected) in api.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("midpoint-solid-{suffix}.f3d"))})).unwrap_or_else(|e| panic!("{suffix}: {e}"));
        verify(&mut s, expected);
        verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), expected);
    }
    let base = &api["baseline"];
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":dir.join("midpoint-solid-baseline.f3d")})).unwrap();
    let original = s.doc.clone();
    for name in ["d7", "d9", "d10"] {
        assert!(!s.doc.params.iter().find(|p| p.name == name).unwrap().model);
    }
    s.execute("parameters.change", &json!({"name":base["x_parameter"],"expression":"d7 + 1.1 mm"})).unwrap();
    verify(&mut s, &api["dependent-baseline"]);
    s.execute("parameters.change", &json!({"name":"d7","expression":"9 mm"})).unwrap();
    verify(&mut s, &api["dependent-changed"]);
    verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), &api["dependent-changed"]);
    for _ in 0..2 {
        s.execute("edit.undo", &json!({})).unwrap();
    }
    assert_eq!(s.doc, original);
    verify(&mut s, base);
    for (parameter, expression, suffix) in [("x_parameter", "8.1 mm", "x-changed"), ("y_parameter", "10 mm", "y-changed")] {
        s.execute("parameters.change", &json!({"name":base[parameter],"expression":expression})).unwrap();
        verify(&mut s, &api[suffix]);
        verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), &api[suffix]);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, original);
        verify(&mut s, base);
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with native offset generator controls"]
fn offset_generators_preserve_distance_source_edits_and_history() {
    use solidcraft_engine::doc::{Document, sketch::CurveKind};
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    for family in ["history", "legacy", "center"] {
        let api: Value = serde_json::from_slice(&std::fs::read(dir.join(format!("fusion-offset-{family}-measurements.json"))).unwrap()).unwrap();
        let verify = |s: &mut Session, expected: &Value| {
            assert!(s.model.results.iter().all(|r| r.error.is_none()));
            let state = s.model.state();
            let sketch = &state.sketches[0].sketch;
            assert_eq!(sketch.offsets.len(), 1);
            assert!(sketch.offsets[0].distance_expression.is_some());
            let point_matches = |index: usize, coords: &Value| {
                let p = sketch.points[index].pos;
                (p.x - coords[0].as_f64().unwrap() * 10.).abs() < 1e-5 && (p.y - coords[1].as_f64().unwrap() * 10.).abs() < 1e-5
            };
            for key in ["parents", "children"] {
                for curve in expected[key].as_array().unwrap() {
                    assert!(
                        sketch.curves.iter().any(|c| match &c.kind {
                            CurveKind::Line { a, b } if curve["type"] == "adsk::fusion::SketchLine" =>
                                point_matches(*a, &curve["start"]) && point_matches(*b, &curve["end"]),
                            CurveKind::Circle { c, r } if curve["type"] == "adsk::fusion::SketchCircle" =>
                                point_matches(*c, &curve["center"]) && (*r - curve["radius"].as_f64().unwrap() * 10.).abs() < 1e-5,
                            _ => false,
                        }),
                        "missing {key} curve {curve}; expected {expected}; actual {:?}",
                        sketch
                    );
                }
            }
            drop(state);
            if expected["kind"] != "line" {
                let m = volume(s, expected["volume"].as_f64().unwrap(), "offset generator");
                assert_eq!(m["body_count"], 1);
                for (end, key) in [(0, "min"), (1, "max")] {
                    for axis in 0..3 {
                        assert!(
                            (m["bodies"][0]["bbox"][key][axis].as_f64().unwrap() - expected["bounds"][0][end][axis].as_f64().unwrap() * 10.).abs()
                                < 1e-5
                        );
                    }
                }
            }
        };
        for (name, expected) in api.as_object().unwrap() {
            let mut s = Session::default();
            if expected["kind"] == "line" {
                let error = s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_err();
                assert!(error.to_string().contains("open offset endpoint"), "{name}: {error}");
                continue;
            }
            s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
            verify(&mut s, expected);
            verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), expected);
            if !name.ends_with("-baseline") {
                continue;
            }
            let original = s.doc.clone();
            let distance = expected["sign"].as_i64().unwrap() * 8;
            s.execute("parameters.change", &json!({"name":expected["offset_parameter"],"expression":format!("{distance} mm")})).unwrap();
            let changed = &api[name.replace("-baseline", "-distance-changed")];
            verify(&mut s, changed);
            verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), changed);
            s.execute("parameters.change", &json!({"name":expected["source_parameter"],"expression":"80 mm"})).unwrap();
            let changed = &api[name.replace("-baseline", "-source-changed")];
            verify(&mut s, changed);
            verify(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), changed);
            for _ in 0..2 {
                s.execute("edit.undo", &json!({})).unwrap();
            }
            assert_eq!(s.doc, original);
            verify(&mut s, expected);
        }
    }
}

#[test]
#[ignore = "requires local multiple-profile public API controls"]
fn multiple_profile_regions_preserve_holes_bodies_edits_and_undo() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-multiple-profile-measurements.json")).unwrap()).unwrap();
    let verify = |s: &mut Session, row: &Value| {
        let measured = volume(s, row["volume"].as_f64().unwrap(), "multiple profiles");
        assert_eq!(measured["body_count"], row["body_count"]);
        let mut boxes: Vec<_> = measured["bodies"].as_array().unwrap().iter().map(|b| &b["bbox"]).collect();
        for expected in row["bounds"].as_array().unwrap() {
            let index = boxes
                .iter()
                .position(|b| {
                    [(0, "min"), (1, "max")].into_iter().all(|(end, key)| {
                        (0..3).all(|axis| (b[key][axis].as_f64().unwrap() - expected[end][axis].as_f64().unwrap() * 10.).abs() < 1e-5)
                    })
                })
                .expect("missing body bounds");
            boxes.remove(index);
        }
    };
    for (name, row) in api.as_object().unwrap() {
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        verify(&mut s, row);
        if name.ends_with("-baseline") {
            let original = s.doc.clone();
            for (suffix, parameter, expression) in
                [("diameter-changed", "diameter_parameter", "30 mm"), ("height-changed", "height_parameter", "20 mm")]
            {
                s.execute("parameters.change", &json!({"name":row[parameter],"expression":expression})).unwrap();
                let expected = &api[name.replace("baseline", suffix)];
                verify(&mut s, expected);
                verify(&mut Session::new(solidcraft_engine::doc::Document::from_json(&s.doc.to_json()).unwrap()), expected);
            }
            for _ in 0..2 {
                s.execute("edit.undo", &json!({})).unwrap();
            }
            assert_eq!(s.doc, original);
            verify(&mut s, row);
        }
    }
}

#[test]
#[ignore = "requires local split-profile public API controls"]
fn partial_profile_pieces_preserve_selected_regions_after_edits() {
    verify_profile_piece_controls("fusion-partial-profile-measurements.json", 15);
}

#[test]
#[ignore = "requires local rotated arc and circle public API controls"]
fn rotated_arcs_and_circle_seams_preserve_regions_after_edits() {
    verify_profile_piece_controls("fusion-piece-parameter-measurements.json", 60);
}

fn verify_profile_piece_controls(measurements: &str, count: usize) {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: Value = serde_json::from_slice(&std::fs::read(dir.join(measurements)).unwrap()).unwrap();
    assert_eq!(api.as_object().unwrap().len(), count);
    let verify = |s: &mut Session, row: &Value| {
        let measured = volume(s, row["volume"].as_f64().unwrap(), "partial profiles");
        assert_eq!(measured["body_count"], row["body_count"]);
        for (end, key) in [(0, "min"), (1, "max")] {
            for axis in 0..3 {
                let got = measured["bodies"][0]["bbox"][key][axis].as_f64().unwrap();
                let expected = row["bounds"][0][end][axis].as_f64().unwrap() * 10.;
                // BodyMeasure bounds come from tessellation, whose fine chord
                // tolerance is at most 5e-5 of the body diagonal (minimum 1e-4 mm).
                let diagonal = (0..3)
                    .map(|i| {
                        let lo = row["bounds"][0][0][i].as_f64().unwrap() * 10.;
                        let hi = row["bounds"][0][1][i].as_f64().unwrap() * 10.;
                        (hi - lo).powi(2)
                    })
                    .sum::<f64>()
                    .sqrt();
                let tolerance = (diagonal * 5e-5).max(1e-4) + 1e-5;
                assert!((got - expected).abs() < tolerance, "bound {key}/{axis}: {got} != {expected}");
            }
        }
    };
    for (name, row) in api.as_object().unwrap() {
        eprintln!("{name}");
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap_or_else(|e| panic!("{name}: {e}"));
        verify(&mut s, row);
        if name.ends_with("-baseline") {
            let original = s.doc.clone();
            for (suffix, parameter, expression) in
                [("source-changed", "source_parameter", row["changed"].as_str().unwrap()), ("height-changed", "height_parameter", "20 mm")]
            {
                s.execute("parameters.change", &json!({"name":row[parameter],"expression":expression})).unwrap();
                let expected = &api[name.replace("baseline", suffix)];
                verify(&mut s, expected);
                verify(&mut Session::new(solidcraft_engine::doc::Document::from_json(&s.doc.to_json()).unwrap()), expected);
            }
            for _ in 0..2 {
                s.execute("edit.undo", &json!({})).unwrap();
            }
            assert_eq!(s.doc, original);
            verify(&mut s, row);
        }
    }
}

#[test]
#[ignore = "requires local independent-radius fillet controls"]
fn imported_history_survives_container_save_reopen_edit_and_json_export() {
    use solidcraft_engine::io;
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let source = probes.join("fillet-sets-baseline.f3d");
    let original = std::fs::read(&source).unwrap();
    let dir = std::env::temp_dir().join(format!("solidcraft-f3d-container-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Imported.solidcraft");
    let mut imported = Session::default();
    imported.execute("doc.open", &json!({"path":source})).unwrap();
    assert!(imported.path.is_none());
    assert!(imported.is_dirty());
    assert!(imported.execute("file.save", &json!({})).is_err());
    imported.execute("file.save_as", &json!({"path":path})).unwrap();
    assert!(!imported.is_dirty());
    let baseline = std::fs::read(&path).unwrap();
    assert!(io::is_design_container(&baseline));
    let mut reopened = Session::default();
    reopened.execute("doc.open", &json!({"path":path})).unwrap();
    assert_eq!(reopened.doc, imported.doc);
    reopened.execute("parameters.change", &json!({"name":"d6","expression":"5 mm"})).unwrap();
    let edited = reopened.doc.clone();
    reopened.execute("file.save", &json!({"compression":"compact"})).unwrap();
    assert_eq!(std::fs::read(io::backup_path(&path)).unwrap(), baseline);
    reopened.execute("doc.open", &json!({"path":path})).unwrap();
    assert_eq!(reopened.doc, edited);
    assert!(reopened.model.results.iter().all(|r| r.error.is_none() && r.warning.is_none()));
    let removed = (1. - std::f64::consts::PI / 4.) * (2_f64.powi(2) + 5_f64.powi(2)) * 40.;
    let want = 20. * 30. * 40. - removed;
    let actual = reopened.execute("inspect.measure", &json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((actual - want).abs() < removed * 0.005 + 0.01);
    reopened.execute("parameters.change", &json!({"name":"d4","expression":"4 mm"})).unwrap();
    reopened.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(reopened.doc, edited);
    let plain = dir.join("Exported.json");
    reopened.execute("file.export", &json!({"path":plain})).unwrap();
    let bytes = std::fs::read(&plain).unwrap();
    assert!(!io::is_design_container(&bytes));
    assert_eq!(io::read_design(&bytes).unwrap(), *edited);
    assert_eq!(reopened.path.as_deref(), path.to_str());
    assert_eq!(std::fs::read(&source).unwrap(), original);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires local face-extrusion controls"]
fn native_face_extrusions_keep_cap_boundary_edits_and_reopen() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let rows: Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-face-extrude-measurements.json")).unwrap()).unwrap();
    let verify = |session: &mut Session, name: &str, row: &Value| {
        let param = |key: &str| row["parameters"].as_array().unwrap().iter().find(|p| p["name"] == key).unwrap()["value"].as_f64().unwrap() * 10.;
        let r = param("d1") / 2.;
        let height = param("d2");
        let chamfer = name.contains("-chamfer-");
        let (cap, wall) = if !chamfer {
            (0., 0.)
        } else if name.contains("-bottom-") {
            (param("d5"), param("d4"))
        } else {
            (param("d4"), param("d5"))
        };
        let distance = if name.ends_with("-suppressed") { 0. } else { param(if chamfer { "d6" } else { "d4" }) };
        let pi = std::f64::consts::PI;
        let expected = pi * r * r * height - pi * wall * (r * cap - cap * cap / 3.) + pi * (r - cap).powi(2) * distance;
        assert!((row["volume_cm3"].as_f64().unwrap() * 1000. - expected).abs() < expected * 1e-6, "analytic control {name}");
        let measured = volume(session, expected / 1000., name);
        assert_eq!(measured["body_count"], row["body_count"], "{name}");
        let removed = pi * wall * (r * cap - cap * cap / 3.);
        let depth = if chamfer { wall * (r * cap / 3. - cap * cap / 12.) / (r * cap - cap * cap / 3.) } else { 0. };
        let top = name.contains("-top-");
        let removed_z = if top { height - depth } else { depth };
        let added_z = if top { height + distance / 2. } else { -distance / 2. };
        let moment = pi * r * r * height * height / 2. - removed * removed_z + pi * (r - cap).powi(2) * distance * added_z;
        let total = measured["total"]["volume_mm3"].as_f64().unwrap();
        for (axis, want) in [0., 0., moment / expected].into_iter().enumerate() {
            let got: f64 = measured["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|body| body["volume_mm3"].as_f64().unwrap() * body["center_of_mass"][axis].as_f64().unwrap())
                .sum::<f64>()
                / total;
            assert!((got - want).abs() < 0.02, "{name}: center axis {axis}: {got} != {want}");
        }
        assert_eq!(session.doc.features.len(), row["timeline"].as_array().unwrap().len() + 1, "one linked helper sketch: {name}");
        let bytes = solidcraft_engine::io::write_design_container(&session.doc).unwrap();
        let mut reopened = Session::new(solidcraft_engine::io::read_design(&bytes).unwrap());
        volume(&mut reopened, expected / 1000., name);
        assert_eq!(reopened.doc, session.doc);
    };
    let mut failures = Vec::new();
    for (name, row) in rows.as_object().unwrap() {
        let mut session = Session::default();
        if let Err(error) = session.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})) {
            failures.push(format!("{name}: {error}"));
            continue;
        }
        verify(&mut session, name, row);
        if !name.ends_with("-baseline") {
            continue;
        }
        let chamfer = name.contains("-chamfer-");
        let mut edits = vec![
            ("diameter60", "d1", "60 mm"),
            ("height50", "d2", "50 mm"),
            ("distance", if chamfer { "d6" } else { "d4" }, if name.contains("-cut-") { "-8 mm" } else { "20 mm" }),
        ];
        if chamfer {
            edits.push(("blend4", "d4", "4 mm"));
        }
        for (suffix, parameter, expression) in edits {
            let mut edited = Session::new((*session.doc).clone());
            edited.execute("parameters.change", &json!({"name":parameter,"expression":expression})).unwrap();
            let changed = name.replace("baseline", suffix);
            verify(&mut edited, &changed, &rows[&changed]);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "requires local face-projection controls"]
fn native_face_cap_projections_preserve_lineage_edits_and_reopen() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let mut combined = serde_json::Map::new();
    for name in ["fusion-face-projection-measurements.json", "fusion-face-projection-joined-measurements.json"] {
        let rows: Value = serde_json::from_slice(&std::fs::read(dir.join(name)).unwrap()).unwrap();
        combined.extend(rows.as_object().unwrap().clone());
    }
    assert_eq!(combined.len(), 132);
    let rows = Value::Object(combined);
    let verify = |session: &mut Session, name: &str, row: &Value| {
        let param = |key: &str| row["parameters"].as_array().unwrap().iter().find(|p| p["name"] == key).unwrap()["value"].as_f64().unwrap() * 10.;
        let r = param("d1") / 2.;
        let height = param("d2");
        let chamfer = name.contains("-chamfer-");
        let (cap, wall) = if !chamfer {
            (0., 0.)
        } else if name.contains("-bottom-") {
            (param("d5"), param("d4"))
        } else {
            (param("d4"), param("d5"))
        };
        let distance = if name.ends_with("-suppressed") { 0. } else { param(if chamfer { "d6" } else { "d4" }) };
        let pi = std::f64::consts::PI;
        let expected = pi * r * r * height - pi * wall * (r * cap - cap * cap / 3.) + pi * (r - cap).powi(2) * (distance + 7.);
        assert!((row["volume_cm3"].as_f64().unwrap() * 1000. - expected).abs() < expected * 1e-6, "analytic control {name}");
        if name.starts_with("face-projection-joined-") && name.contains("-new-") {
            use solidcraft_engine::doc::{FeatureKind, Operation};
            let parent =
                session.doc.features.iter().rev().find(|f| matches!(f.kind, FeatureKind::Extrude { operation: Operation::NewBody, .. })).unwrap();
            let FeatureKind::Extrude { operation: Operation::Join, targets, .. } = &session.doc.features.last().unwrap().kind else {
                panic!("joined projection")
            };
            assert_eq!(targets, &parent.body_names, "explicit target must preserve the unrelated first body: {name}");
        }
        let measured = volume(session, expected / 1000., name);
        assert_eq!(measured["body_count"], row["body_count"], "{name}");
        let removed = pi * wall * (r * cap - cap * cap / 3.);
        let depth = if chamfer { wall * (r * cap / 3. - cap * cap / 12.) / (r * cap - cap * cap / 3.) } else { 0. };
        let top = name.contains("-top-");
        let removed_z = if top { height - depth } else { depth };
        let added_z = if top { height + distance / 2. } else { -distance / 2. };
        let projected_z = if top { height + distance + 3.5 } else { -distance - 3.5 };
        let moment = pi * r * r * height * height / 2. - removed * removed_z + pi * (r - cap).powi(2) * (distance * added_z + 7. * projected_z);
        let total = measured["total"]["volume_mm3"].as_f64().unwrap();
        for (axis, want) in [0., 0., moment / expected].into_iter().enumerate() {
            let got: f64 = measured["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|body| body["volume_mm3"].as_f64().unwrap() * body["center_of_mass"][axis].as_f64().unwrap())
                .sum::<f64>()
                / total;
            assert!((got - want).abs() < 0.02, "{name}: center axis {axis}: {got} != {want}");
        }
        assert_eq!(session.doc.features.len(), row["timeline"].as_array().unwrap().len() + 1, "one linked helper sketch: {name}");
        let bytes = solidcraft_engine::io::write_design_container(&session.doc).unwrap();
        let mut reopened = Session::new(solidcraft_engine::io::read_design(&bytes).unwrap());
        volume(&mut reopened, expected / 1000., name);
        assert_eq!(reopened.doc, session.doc);
    };
    let mut failures = Vec::new();
    for (name, row) in rows.as_object().unwrap() {
        eprintln!("BEGIN face projection {name}");
        let mut session = Session::default();
        if let Err(error) = session.execute("doc.open", &json!({"path":dir.join(format!("{name}.f3d"))})) {
            failures.push(format!("{name}: {error}"));
            continue;
        }
        verify(&mut session, name, row);
        if !name.ends_with("-baseline") {
            continue;
        }
        let chamfer = name.contains("-chamfer-");
        let mut edits = vec![
            ("diameter60", "d1", "60 mm"),
            ("height50", "d2", "50 mm"),
            ("distance", if chamfer { "d6" } else { "d4" }, if name.contains("-cut-") { "-8 mm" } else { "20 mm" }),
        ];
        if chamfer {
            edits.push(("blend4", "d4", "4 mm"));
        }
        for (suffix, parameter, expression) in edits {
            let mut edited = Session::new((*session.doc).clone());
            edited.execute("parameters.change", &json!({"name":parameter,"expression":expression})).unwrap();
            let changed = name.replace("baseline", suffix);
            verify(&mut edited, &changed, &rows[&changed]);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "requires local Style 5 timeline checkpoints"]
fn native_style5_projected_cap_checkpoint_preserves_history() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let mut before = Session::default();
    before.execute("doc.open", &json!({"path":dir.join("legacy-face-projection-Extrude5.f3d")})).unwrap();
    let measured = before.execute("inspect.measure", &json!({})).unwrap();
    let expected = measured["total"]["volume_mm3"].as_f64().unwrap() / 1000.;
    let mut projected = Session::default();
    projected.execute("doc.open", &json!({"path":dir.join("legacy-face-projection-Sketch4.f3d")})).unwrap();
    volume(&mut projected, expected, "Style 5 projected cap");
    assert_eq!(projected.doc.features.len(), before.doc.features.len() + 1);
    let bytes = solidcraft_engine::io::write_design_container(&projected.doc).unwrap();
    let mut reopened = Session::new(solidcraft_engine::io::read_design(&bytes).unwrap());
    assert_eq!(projected.doc, reopened.doc);
    volume(&mut reopened, expected, "Reopened Style 5 projected cap");
}
