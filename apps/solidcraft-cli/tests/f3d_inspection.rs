use serde_json::json;
use solidcraft_engine::Session;

#[test]
#[ignore = "requires the original adapter and unchanged construction-point resave"]
fn legacy_adapter_points_preserve_their_two_edge_construction() {
    use solidcraft_engine::io::f3d::{HistoryKind, read_f3d};
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let mut results = Vec::new();
    for path in [corpus.join("adapter-x20-makita.f3d"), probes.join("adapter-construction-resaved.f3d")] {
        let designs = read_f3d(&std::fs::read(path).unwrap()).unwrap();
        let timeline = designs[0].timeline().unwrap();
        let mut points = Vec::new();
        for (id, expected_definition, expected_edges, expected_z) in [(17126, 17127, [17130, 17134], -26.3), (15316, 15317, [15320, 15324], 26.3)] {
            let feature = &timeline.iter().find(|f| f.id == id).unwrap().feature;
            let feature = feature.as_ref().unwrap();
            let HistoryKind::TwoEdgePoint { definition, edges, position } = &feature.kind else { panic!("two-edge point") };
            assert_eq!(*definition, expected_definition);
            assert_eq!(*edges, expected_edges);
            assert!(position.dist(solidcraft_engine::geom::Vec3::new(-17.1, -13., expected_z)) < 1e-7);
            points.push((feature.name.clone(), *definition, *edges, *position));
        }
        results.push(points);
    }
    assert_eq!(results[0], results[1]);
}

#[test]
#[ignore = "requires original Gridfinity archive and unchanged modern resave"]
fn legacy_fillet_sets_match_the_unchanged_modern_resave() {
    use solidcraft_engine::io::f3d::{HistoryKind, read_f3d};
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let file = "Gridfinity Storage Box Shelves (Parametric) V1.0.f3d";
    let mut decoded = Vec::new();
    for path in [corpus.join(file), probes.join(format!("corpus-{file}"))] {
        let designs = read_f3d(&std::fs::read(path).unwrap()).unwrap();
        let features: Vec<_> =
            designs[0].timeline().unwrap().into_iter().filter(|f| [46870, 47024, 52135].contains(&f.id)).map(|f| f.feature.unwrap()).collect();
        assert_eq!(features.len(), 3);
        for feature in &features {
            let HistoryKind::Fillet { sets } = &feature.kind else { panic!("fillet feature") };
            assert_eq!(sets.len(), 1);
            assert!(sets[0].tangent_chain);
        }
        decoded.push(features);
    }
    assert_eq!(decoded[0], decoded[1]);
}

#[test]
#[ignore = "requires original Gridfinity archive and unchanged modern resave"]
fn legacy_combine_inputs_match_the_unchanged_modern_resave() {
    use solidcraft_engine::{
        doc::Operation,
        io::f3d::{HistoryKind, read_f3d},
    };
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let file = "Gridfinity Storage Box Shelves (Parametric) V1.0.f3d";
    let mut decoded = Vec::new();
    for path in [corpus.join(file), probes.join(format!("corpus-{file}"))] {
        let designs = read_f3d(&std::fs::read(path).unwrap()).unwrap();
        let timeline = designs[0].timeline().unwrap();
        assert_eq!(timeline.len(), 35);
        let feature = timeline.into_iter().find(|f| f.id == 66161).unwrap().feature.unwrap();
        assert_eq!(
            feature.kind,
            HistoryKind::Combine {
                operation: Operation::Join,
                keep_tools: false,
                operation_tag: 450,
                tool_containers: vec![66162, 66169],
                target_container: 66176,
                consumed_handles: vec![31544, 34694],
            }
        );
        decoded.push(feature);
    }
    assert_eq!(decoded[0], decoded[1]);
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with the body-reference API controls"]
fn topology_operation_owners_distinguish_overlapping_source_extrusions() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let mut s = Session::default();
    let original = s.doc.clone();
    for (name, expected) in [
        ("body-reference-top", vec![(300, 207), (301, 242), (302, 242), (303, 279), (304, 334), (305, 334)]),
        ("body-reference-bottom", vec![(300, 207), (301, 242), (302, 242), (303, 279), (304, 334), (305, 334)]),
        ("body-reference-second", vec![(300, 207), (301, 242), (302, 242), (303, 275), (304, 275), (305, 310), (306, 365), (307, 365)]),
    ] {
        let result = s.execute("file.inspect_f3d", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        let got = &result["designs"][0]["operation_owners"]["records"];
        assert_eq!(*got, json!(expected.into_iter().map(|(tag, feature)| json!({"tag":tag,"feature":feature})).collect::<Vec<_>>()), "{name}");
        assert!(std::sync::Arc::ptr_eq(&original, &s.doc));
    }
}

#[test]
fn oversized_f3d_reads_fail_before_archive_decoding() {
    let path = std::env::temp_dir().join(format!("solidcraft-f3d-cap-{}.f3d", std::process::id()));
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    file.set_len(solidcraft_engine::io::f3d::MAX_F3D_BYTES as u64 + 1).unwrap();
    drop(file);
    let mut s = Session::default();
    let doc = s.doc.clone();
    let errors: Vec<_> = ["doc.open", "file.inspect_f3d"].into_iter().map(|command| s.execute(command, &json!({"path":path}))).collect();
    let cli = std::process::Command::new(env!("CARGO_BIN_EXE_solidcraft-cli")).arg("bench-open").arg(&path).output().unwrap();
    std::fs::remove_file(path).unwrap();
    for result in errors {
        assert!(result.unwrap_err().to_string().contains("file too large"));
    }
    assert!(!cli.status.success());
    assert!(String::from_utf8_lossy(&cli.stderr).contains("file too large"));
    assert!(std::sync::Arc::ptr_eq(&doc, &s.doc));
}

#[cfg(unix)]
#[test]
fn f3d_entry_points_refuse_non_regular_files() {
    let path = std::env::temp_dir().join(format!("solidcraft-f3d-device-{}.f3d", std::process::id()));
    // /dev/null is finite, so this regression itself cannot hang even if the
    // regular-file guard is accidentally removed in a future change.
    std::os::unix::fs::symlink("/dev/null", &path).unwrap();
    let mut s = Session::default();
    let errors: Vec<_> = ["doc.open", "file.inspect_f3d"].into_iter().map(|command| s.execute(command, &json!({"path":path}))).collect();
    let cli = std::process::Command::new(env!("CARGO_BIN_EXE_solidcraft-cli")).arg("bench-open").arg(&path).output().unwrap();
    std::fs::remove_file(path).unwrap();
    for result in errors {
        assert!(result.unwrap_err().to_string().contains("not a regular file"));
    }
    assert!(!cli.status.success());
    assert!(String::from_utf8_lossy(&cli.stderr).contains("not a regular file"));
}

#[test]
fn unsupported_f3d_open_preserves_the_current_design() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("solidcraft-invalid-{}.F3D", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    file.write_all(b"invalid native archive").unwrap();
    drop(file);
    let mut session = Session::default();
    session.execute("solid.box", &json!({"length": 10, "width": 20, "height": 30})).unwrap();
    let doc = session.doc.clone();
    let model = session.model.state();
    let error = session.execute("doc.open", &json!({"path": path})).unwrap_err().to_string();
    std::fs::remove_file(path).unwrap();
    assert!(error.contains("F3D"), "{error}");
    assert!(std::sync::Arc::ptr_eq(&doc, &session.doc));
    assert!(std::sync::Arc::ptr_eq(&model, &session.model.state()));
    assert!(session.is_dirty());
    session.execute("edit.undo", &json!({})).unwrap();
    assert!(session.model.state().bodies.is_empty());
}

/// These user-supplied Fusion files are local evidence and are never committed.
#[test]
#[ignore = "requires SOLIDCRAFT_F3D_CORPUS pointing to the supplied test_files directory"]
fn supplied_f3d_indexes() {
    let dir = solidcraft_engine::geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS");
    let mut session = Session::default();
    let doc = session.doc.clone();
    for (name, layout, classes, objects, secondary) in [
        ("adapter-x20-makita.f3d", 4, 213, 5538, 849),
        ("R36S Grip.f3d", 5, 207, 3952, 629),
        ("Gridfinity Storage Box Shelves (Parametric) V1.0.f3d", 5, 223, 4899, 784),
        ("Minimal.f3d", 8, 176, 259, 36),
    ] {
        let path = std::path::Path::new(&dir).join(name);
        let start = std::time::Instant::now();
        let result = session.execute("file.inspect_f3d", &json!({"path": path})).unwrap();
        assert_eq!(result["editable_history_imported"], false);
        let designs = result["designs"].as_array().unwrap();
        assert_eq!(designs.len(), 1);
        let d = &designs[0];
        assert_eq!(d["layout"], layout);
        assert_eq!(d["classes"], classes);
        assert_eq!(d["objects"], objects);
        assert_eq!(d["secondary_records"], secondary);
        if name == "Minimal.f3d" {
            let params = d["parameters"]["records"].as_array().unwrap();
            assert_eq!(params.len(), 3);
            assert_eq!(params[0]["name"], "d1");
            assert_eq!(params[0]["expression"], "100 mm");
            assert_eq!(params[0]["native_value"], 10.0);
            assert_eq!(params[1]["label"], "AlongDistance");
            assert_eq!(params[1]["expression"], "100.00 mm");
            assert_eq!(params[1]["native_value"], 10.0);
            assert_eq!(params[2]["label"], "TaperAngle");
            assert_eq!(params[2]["native_value"], 0.0);
            assert_ne!(params[0]["owner"], params[1]["owner"]);
            assert_eq!(params[1]["owner"], params[2]["owner"]);
        }
        assert!(std::sync::Arc::ptr_eq(&doc, &session.doc));
        assert!(!session.is_dirty());
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        eprintln!("{name}: {objects} indexed objects, {:?}", start.elapsed());
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to local controlled Fusion exports"]
fn controlled_parameter_edits_match_fusion_api() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let mut s = Session::default();
    for measurements_file in ["fusion-measurements.json", "fusion-feature-measurements.json", "fusion-circle-measurements.json"] {
        let measurements: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(measurements_file)).unwrap()).unwrap();
        for (name, measurement) in measurements.as_object().unwrap() {
            let result = s.execute("file.inspect_f3d", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
            let decoded = result["designs"][0]["parameters"]["records"].as_array().unwrap();
            let api = measurement["parameters"].as_array().unwrap();
            assert_eq!(decoded.len(), api.len());
            for p in api {
                let actual = decoded.iter().find(|r| r["name"] == p["name"]).unwrap();
                assert_eq!(actual["expression"], p["expression"]);
                assert_eq!(actual["display_unit"], p["unit"]);
                assert_eq!(actual["native_value"], p["value"]);
            }
            let circles = &result["designs"][0]["sketch_circles"];
            assert_eq!(circles["coordinate_frame"], "sketch_local");
            assert_eq!(circles["length_unit"], "cm");
            let circles = circles["records"].as_array().unwrap();
            assert_eq!(circles.len(), 1);
            assert_eq!(circles[0]["center"], measurement["circle"]["center"]);
            assert_eq!(circles[0]["radius"], measurement["circle"]["radius"]);
            let features =
                result["designs"][0]["history"]["records"].as_array().unwrap_or_else(|| panic!("{name}: {}", result["designs"][0]["history"]));
            for (f, api) in features.iter().zip(measurement["timeline"].as_array().unwrap()) {
                assert_eq!(f["name"], api["name"], "{name}");
            }
            assert_eq!(features[0]["kind"]["transform"], measurement["transform"], "{name}");
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_CORPUS and SOLIDCRAFT_F3D_PROBES with legacy/resaved archive pairs"]
fn legacy_parameters_match_resaved_records_and_public_api_measurements() {
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS"));
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-corpus-measurements.json")).unwrap()).unwrap();
    for (name, count) in [("adapter-x20-makita.f3d", 190), ("R36S Grip.f3d", 70), ("Gridfinity Storage Box Shelves (Parametric) V1.0.f3d", 151)] {
        let original = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(corpus.join(name)).unwrap()).unwrap();
        let upgraded = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(probes.join(format!("corpus-{name}"))).unwrap()).unwrap();
        let mut old = original[0].parameters().unwrap();
        let mut new = upgraded[0].parameters().unwrap();
        old.sort_by_key(|p| p.id);
        new.sort_by_key(|p| p.id);
        assert_eq!(old.len(), count);
        assert_eq!(old, new, "{name}: schema upgrade changed a parameter");
        for p in api[name]["parameters"].as_array().unwrap() {
            let actual = old.iter().find(|r| r.name == p["name"].as_str().unwrap()).unwrap();
            assert_eq!(actual.expression, p["expression"].as_str().unwrap(), "{name}");
            assert_eq!(actual.display_unit, p["unit"].as_str().unwrap(), "{name}");
            let want = p["value"].as_f64().unwrap();
            assert!((actual.value - want).abs() < want.abs().max(1.) * 1e-9, "{name}: {}", actual.name);
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to local controlled Fusion exports"]
fn controlled_history_matches_fusion_api() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let measurements: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-history-measurements.json")).unwrap()).unwrap();
    let mut s = Session::default();
    for (name, measurement) in measurements.as_object().unwrap() {
        let result = s.execute("file.inspect_f3d", &json!({"path":dir.join(format!("{name}.f3d"))})).unwrap();
        let history = &result["designs"][0]["history"];
        let d = &result["designs"][0];
        let constraints = d["sketch_constraints"]["records"].as_array().unwrap_or_else(|| panic!("{name}: {}", d["sketch_constraints"]));
        let profiles = d["circle_profiles"]["records"].as_array().unwrap_or_else(|| panic!("{name}: {}", d["circle_profiles"]));
        let records = history["records"].as_array().unwrap_or_else(|| panic!("{name}: {history}"));
        let timeline = measurement["timeline"].as_array().unwrap();
        assert_eq!(records.len(), timeline.len(), "{name}");
        for (f, api) in records.iter().zip(timeline) {
            assert_eq!(f["name"], api["name"], "{name}");
            assert_eq!(f["suppressed"], api["suppressed"], "{name}");
            if f["kind"]["type"] == "sketch" {
                let sketch = measurement["sketches"].as_array().unwrap().iter().find(|x| x["name"] == api["name"]).unwrap();
                assert_eq!(f["kind"]["transform"], sketch["transform"], "{name}");
                let decoded = constraints.iter().find(|s| s["geometry"] == f["kind"]["geometry"]).unwrap()["constraints"].as_array().unwrap();
                assert_eq!(decoded.iter().filter(|c| c["type"] == "coincident").count(), sketch["constraints"].as_array().unwrap().len());
                assert_eq!(decoded.iter().filter(|c| c["type"] == "diameter").count(), sketch["dimensions"].as_array().unwrap().len());
            } else {
                let extrude = measurement["extrudes"].as_array().unwrap().iter().find(|x| x["name"] == api["name"]).unwrap();
                let operation = ["join", "cut", "intersect", "new_body"][extrude["operation"].as_u64().unwrap() as usize];
                assert_eq!(f["kind"]["operation"], operation, "{name}");
                let profile = profiles.iter().find(|p| p["feature"] == f["id"]).unwrap();
                let circle = d["sketch_circles"]["records"].as_array().unwrap().iter().find(|c| c["id"] == profile["circle"]).unwrap();
                assert_eq!(circle["sketch"], profile["geometry"]);
            }
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES pointing to local controlled Fusion exports"]
fn controlled_construction_and_constraint_removal() {
    let dir = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let measurements: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("fusion-extent-measurements.json")).unwrap()).unwrap();
    for (name, measurement) in measurements.as_object().unwrap() {
        let designs = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(dir.join(format!("{name}.f3d"))).unwrap()).unwrap();
        let d = &designs[0];
        assert_eq!(d.sketch_circles().unwrap()[0].construction, measurement["construction"].as_bool().unwrap(), "{name}");
        let constraints = d.sketch_constraints().unwrap_or_else(|e| panic!("{name}: {e}"));
        let want = if name == "circle-unconstrained" {
            0
        } else if name == "circle-undimensioned" {
            1
        } else {
            2
        };
        assert_eq!(constraints[0].constraints.len(), want, "{name}");
        if name == "extent-two-sided" {
            assert!(d.history().unwrap_err().to_string().contains("unsupported extrusion extent layout"));
        } else {
            for f in d.history().unwrap() {
                if let solidcraft_engine::io::f3d::HistoryKind::Extrude { profile_containers, .. } = f.kind {
                    assert_eq!(d.selected_circle_profiles(f.id, &profile_containers).unwrap().len(), 1, "{name}");
                }
            }
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with public-API projection and centerline controls"]
fn projection_flags_and_line_types_match_public_api() {
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-projection-measurements.json")).unwrap()).unwrap();
    for (name, sketches) in api.as_object().unwrap() {
        let designs = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(probes.join(format!("{name}.f3d"))).unwrap()).unwrap();
        let d = &designs[0];
        let points = d.sketch_points().unwrap();
        let lines = d.sketch_lines().unwrap();
        let circles = d.sketch_circles().unwrap();
        let expected_points: Vec<_> = sketches.as_array().unwrap().iter().flat_map(|s| s["points"].as_array().unwrap()).collect();
        let expected_curves: Vec<_> = sketches.as_array().unwrap().iter().flat_map(|s| s["curves"].as_array().unwrap()).collect();
        assert_eq!(points.len(), expected_points.len(), "{name}");
        assert_eq!(lines.len() + circles.len(), expected_curves.len(), "{name}");
        assert_eq!(points.iter().filter(|p| p.linked).count(), expected_points.iter().filter(|p| p["linked"] == true).count(), "{name}");
        assert_eq!(
            lines.iter().filter(|p| p.reference).count() + circles.iter().filter(|p| p.reference).count(),
            expected_curves.iter().filter(|p| p["reference"] == true).count(),
            "{name}"
        );
        assert_eq!(lines.iter().filter(|p| p.centerline).count(), expected_curves.iter().filter(|p| p["centerline"] == true).count(), "{name}");
        assert_eq!(lines.iter().filter(|p| p.construction).count(), expected_curves.iter().filter(|p| p["construction"] == true).count(), "{name}");
        if name == "projection-linked" {
            let doc = d.to_document(name).unwrap();
            let solidcraft_engine::doc::FeatureKind::Sketch { sketch, .. } = &doc.features[1].kind else { panic!("projected sketch") };
            assert_eq!(sketch.links.len(), 2);
            assert!(sketch.curves.iter().all(|c| c.link.is_some()));
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_CORPUS and SOLIDCRAFT_F3D_PROBES with legacy API measurements"]
fn legacy_timeline_preserves_every_entry_and_decodes_versioned_sketches() {
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-corpus-measurements.json")).unwrap()).unwrap();
    for (name, count, decoded, sketch_count) in
        [("adapter-x20-makita.f3d", 49, 43, 12), ("Gridfinity Storage Box Shelves (Parametric) V1.0.f3d", 35, 22, 3), ("R36S Grip.f3d", 53, 25, 2)]
    {
        let mut s = Session::default();
        let before = s.doc.clone();
        let result = s.execute("file.inspect_f3d", &json!({"path":corpus.join(name)})).unwrap();
        assert!(std::sync::Arc::ptr_eq(&before, &s.doc));
        let entries = result["designs"][0]["timeline"]["records"].as_array().unwrap();
        assert_eq!(entries.len(), count);
        assert_eq!(entries.len(), api[name]["timeline"].as_array().unwrap().len());
        assert_eq!(entries.iter().filter(|r| r["decoded"]["unsupported"].is_null()).count(), decoded);
        let mut sketches = 0;
        for (entry, expected) in entries.iter().zip(api[name]["timeline"].as_array().unwrap()) {
            if entry["class"] == "8DA771B7-52ED-42FC-94E2-2353FE141373" {
                sketches += 1;
                assert_eq!(expected["type"], "adsk::fusion::Sketch");
                assert!(entry["decoded"]["unsupported"].is_null(), "{name}: {entry}");
                assert!(!entry["decoded"]["sketch"]["entities"].as_array().unwrap().is_empty());
                let native = entry["decoded"]["name"].as_str().unwrap();
                // Fusion localizes and renumbers automatic names on import;
                // preserve the original serialized names instead of substituting API names.
                if name == "R36S Grip.f3d" {
                    assert!(matches!(native, "Skizze1" | "Skizze6"));
                } else {
                    assert_eq!(native, expected["name"].as_str().unwrap());
                }
            }
        }
        assert_eq!(sketches, sketch_count);
        assert!(s.execute("doc.open", &json!({"path":corpus.join(name)})).is_err());
        assert!(std::sync::Arc::ptr_eq(&before, &s.doc));
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_PROBES with resaved legacy archives and public-API curve measurements"]
fn original_and_resaved_legacy_geometry_matches_public_api() {
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-legacy-curves.json")).unwrap()).unwrap();
    let close = |a: f64, b: f64| (a - b).abs() < a.abs().max(b.abs()).max(1.) * 1e-8;
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS"));
    for (name, sketches) in api.as_object().unwrap() {
        for path in [probes.join(name), corpus.join(name.strip_prefix("corpus-").unwrap())] {
            let designs = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(&path).unwrap()).unwrap();
            let d = &designs[0];
            let points = d.sketch_points().unwrap_or_else(|e| panic!("{name}: {e}"));
            let mut expected_points: Vec<_> = sketches.as_array().unwrap().iter().flat_map(|s| s["points"].as_array().unwrap()).collect();
            assert_eq!(points.len(), expected_points.len(), "{}: point count", path.display());
            for p in &points {
                let at = expected_points
                    .iter()
                    .position(|want| {
                        p.position.iter().enumerate().all(|(axis, x)| close(*x, want["position"][axis].as_f64().unwrap())) && want["fixed"] == p.fixed
                    })
                    .unwrap_or_else(|| panic!("{}: missing point {p:?}", path.display()));
                expected_points.swap_remove(at);
            }
            let position = |id: u64| points.iter().find(|p| p.id == id).unwrap().position;
            let mut decoded = Vec::new();
            for c in d.sketch_lines().unwrap_or_else(|e| panic!("{name}: {e}")) {
                decoded.push(json!({"type":"adsk::fusion::SketchLine","construction":c.construction,"fixed":c.fixed,"start":c.start,"end":c.end}));
            }
            for c in d.sketch_circles().unwrap_or_else(|e| panic!("{name}: {e}")) {
                decoded.push(
                    json!({"type":"adsk::fusion::SketchCircle","construction":c.construction,"fixed":c.fixed,"center":c.center,"radius":c.radius}),
                );
            }
            for a in d.sketch_arcs().unwrap_or_else(|e| panic!("{name}: {e}")) {
                let c = a.circle;
                decoded.push(json!({"type":"adsk::fusion::SketchArc","construction":c.construction,"fixed":c.fixed,"center":c.center,"radius":c.radius,"start":position(a.start_point),"end":position(a.end_point)}));
            }
            let expected: Vec<_> = sketches
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|s| s["curves"].as_array().unwrap())
                .filter(|c| matches!(c["type"].as_str(), Some("adsk::fusion::SketchLine" | "adsk::fusion::SketchCircle" | "adsk::fusion::SketchArc")))
                .collect();
            assert_eq!(decoded.len(), expected.len(), "{name}: analytic curve count");
            for want in expected {
                let matched = decoded
                    .iter()
                    .position(|got| {
                        for key in ["type", "construction", "fixed"] {
                            if got[key] != want[key] {
                                return false;
                            }
                        }
                        if let Some(radius) = want["radius"].as_f64()
                            && !got["radius"].as_f64().is_some_and(|r| close(r, radius))
                        {
                            return false;
                        }
                        for key in ["start", "end", "center"] {
                            if let Some(p) = want[key].as_array() {
                                for (axis, x) in p.iter().enumerate() {
                                    if !got[key][axis].as_f64().is_some_and(|n| close(n, x.as_f64().unwrap())) {
                                        return false;
                                    }
                                }
                            }
                        }
                        true
                    })
                    .unwrap_or_else(|| panic!("{name}: missing analytic curve {want}"));
                decoded.swap_remove(matched);
            }
        }
    }
}

#[test]
#[ignore = "requires SOLIDCRAFT_F3D_CORPUS and SOLIDCRAFT_F3D_PROBES with legacy constraint measurements"]
fn legacy_constraint_containers_preserve_complete_relationship_lists() {
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-legacy-constraints.json")).unwrap()).unwrap();
    for (name, relationships, dimensions) in
        [("adapter-x20-makita.f3d", 258, 121), ("Gridfinity Storage Box Shelves (Parametric) V1.0.f3d", 272, 68), ("R36S Grip.f3d", 42, 6)]
    {
        let mut expected = None;
        for path in [corpus.join(name), probes.join(format!("corpus-{name}"))] {
            let ds = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(path).unwrap()).unwrap();
            let graphs = ds[0].sketch_constraint_graphs().unwrap();
            assert_eq!(graphs.iter().map(|g| g.relations.len()).sum::<usize>(), relationships);
            assert_eq!(graphs.iter().map(|g| g.dimensions.len()).sum::<usize>(), dimensions);
            let measured = &api[format!("corpus-{name}")];
            assert_eq!(graphs.len(), measured.as_array().unwrap().len());
            let total = measured.as_array().unwrap().iter().map(|s| s["constraints"].as_array().unwrap().len()).sum::<usize>();
            // Adapter's four additional native relationship records are not exposed
            // as public geometric constraints. Keep them in the native graph.
            assert_eq!(relationships + dimensions, total + if name == "adapter-x20-makita.f3d" { 4 } else { 0 });
            let current: Vec<_> =
                graphs.iter().map(|g| (g.geometry, g.feature, g.relations.clone(), g.dimensions.clone(), g.auxiliary.clone(), g.support)).collect();
            if let Some(expected) = &expected {
                assert_eq!(&current, expected);
            } else {
                expected = Some(current);
            }
        }
    }
}

#[test]
#[ignore = "requires local legacy/resaved archives and offset API measurements"]
fn legacy_offset_sources_outputs_and_parameters_match_public_api() {
    use solidcraft_engine::io::f3d::SketchConstraint;
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let api: serde_json::Value =
        serde_json::from_slice(&std::fs::read(probes.join("fusion-legacy-offset-profile-measurements.json")).unwrap()).unwrap();
    let name = "Gridfinity Storage Box Shelves (Parametric) V1.0.f3d";
    let expected = api[name]["offsets"].as_array().unwrap();
    assert_eq!(expected.len(), 6);
    let mut original = None;
    for path in [corpus.join(name), probes.join(format!("corpus-{name}"))] {
        let designs = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(path).unwrap()).unwrap();
        let d = &designs[0];
        let parameters = d.parameters().unwrap();
        let mut curves = std::collections::BTreeMap::new();
        for c in d.sketch_lines().unwrap() {
            curves.insert(c.id, json!({"type":"adsk::fusion::SketchLine","start":c.start,"end":c.end}));
        }
        for c in d.sketch_circles().unwrap() {
            curves.insert(c.id, json!({"type":"adsk::fusion::SketchCircle","center":c.center,"radius":c.radius}));
        }
        for a in d.sketch_arcs().unwrap() {
            let c = a.circle;
            curves.insert(c.id, json!({"type":"adsk::fusion::SketchArc","center":c.center,"radius":c.radius}));
        }
        let graphs = d.sketch_constraints().unwrap();
        let mut decoded = Vec::new();
        for graph in &graphs {
            for c in &graph.constraints {
                let SketchConstraint::OffsetCurves(offset) = c else { continue };
                let parameter = parameters.iter().find(|p| p.binding == Some(offset.binding)).unwrap();
                let want = expected.iter().find(|row| row["parameter"] == parameter.name).unwrap();
                assert!((want["distance"].as_f64().unwrap() - offset.distance).abs() < 1e-9);
                assert!((parameter.value - offset.distance).abs() < 1e-9);
                for (key, ids) in [("parents", &offset.sources), ("children", &offset.outputs)] {
                    let mut actual: Vec<_> = ids.iter().map(|id| &curves[id]).collect();
                    assert_eq!(actual.len(), want[key].as_array().unwrap().len());
                    for entity in want[key].as_array().unwrap() {
                        let index = actual
                            .iter()
                            .position(|c| {
                                if c["type"] != entity["type"] {
                                    return false;
                                }
                                for key in ["start", "end", "center"] {
                                    if let Some(coords) = c[key].as_array() {
                                        for (i, coordinate) in coords.iter().enumerate() {
                                            if (coordinate.as_f64().unwrap() - entity[key][i].as_f64().unwrap()).abs() > 1e-8 {
                                                return false;
                                            }
                                        }
                                    }
                                }
                                c["radius"].as_f64().is_none_or(|r| (r - entity["radius"].as_f64().unwrap()).abs() < 1e-8)
                            })
                            .unwrap_or_else(|| panic!("{} {key}: missing {entity}", parameter.name));
                        actual.remove(index);
                    }
                }
                decoded.push((offset.id, offset.sources.clone(), offset.outputs.clone(), offset.dimension, parameter.name.clone()));
            }
        }
        assert_eq!(decoded.len(), 6);
        if let Some(original) = &original {
            assert_eq!(&decoded, original);
        } else {
            original = Some(decoded);
        }
    }
}

#[test]
#[ignore = "requires local original and resaved legacy archives"]
fn analytic_legacy_profile_boundaries_match_resaved_records() {
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let mut successes = 0;
    for name in ["adapter-x20-makita.f3d", "Gridfinity Storage Box Shelves (Parametric) V1.0.f3d", "R36S Grip.f3d"] {
        let mut original = None;
        for path in [corpus.join(name), probes.join(format!("corpus-{name}"))] {
            let mut ds = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(path).unwrap()).unwrap();
            let d = &mut ds[0];
            let class = d.classes.iter().position(|c| c.id == "4BD53E5A-0B3E-45E5-AE8B-02044306485A").unwrap();
            let ids = d.classes[class].objects.clone();
            let mut decoded = std::collections::BTreeMap::new();
            for id in ids {
                d.classes[class].objects = vec![id];
                if let Ok(profiles) = d.sketch_profiles() {
                    decoded.insert(id, profiles);
                }
            }
            eprintln!("{name}, layout {}: {} analytic profiles", d.layout, decoded.len());
            if let Some(original) = &original {
                assert_eq!(&decoded, original);
                successes += decoded.len();
            } else {
                original = Some(decoded);
            }
        }
    }
    assert_eq!(successes, 20, "analytic legacy profile coverage changed");
}

#[test]
#[ignore = "requires the local Gridfinity original/resave and public API plane measurements"]
fn legacy_offset_planes_match_public_api_dependencies_and_placements() {
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-timeline-shapes.json")).unwrap()).unwrap();
    let name = "Gridfinity Storage Box Shelves (Parametric) V1.0.f3d";
    let mut original = None;
    for path in [corpus.join(name), probes.join(format!("corpus-{name}"))] {
        let designs = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(&path).unwrap()).unwrap();
        let d = &designs[0];
        let planes = d.offset_planes().unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(planes.len(), 2);
        let parameters = d.parameters().unwrap();
        for (plane, row) in planes.iter().zip(api[name].as_array().unwrap()) {
            let parameter = parameters.iter().find(|p| p.binding == Some(plane.distance)).unwrap();
            assert_eq!(parameter.name, row["offset"]["name"]);
            assert_eq!(parameter.expression, row["offset"]["expression"]);
            assert_eq!(parameter.owner, Some(plane.feature));
            assert!((parameter.value - row["offset"]["value"].as_f64().unwrap()).abs() < 1e-9);
            assert!(plane.extra.is_none());
            for (got, key, scale) in [(plane.plane.origin, "origin", 10.), (plane.plane.normal(), "normal", 1.)] {
                for (value, want) in [got.x, got.y, got.z].iter().zip(row["geometry"][key].as_array().unwrap()) {
                    assert!((value - want.as_f64().unwrap() * scale).abs() < 1e-8);
                }
            }
        }
        if let Some(expected) = &original {
            assert_eq!(&planes, expected);
        } else {
            original = Some(planes);
        }
    }
}

#[test]
#[ignore = "requires the local original/resaved adapter and public API extent measurements"]
fn legacy_planar_endpoints_preserve_timeline_names_and_dependencies() {
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-adapter-extents.json")).unwrap()).unwrap();
    let names: Vec<_> = api
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["extentOne"]["type"] == "adsk::fusion::ToEntityExtentDefinition")
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 10);
    let mut original = None;
    for path in [corpus.join("adapter-x20-makita.f3d"), probes.join("corpus-adapter-x20-makita.f3d")] {
        let ds = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(path).unwrap()).unwrap();
        let decoded: Vec<_> =
            ds[0].timeline().unwrap().into_iter().filter_map(|entry| entry.feature.ok()).filter(|f| names.contains(&f.name.as_str())).collect();
        assert_eq!(decoded.len(), 10);
        for feature in &decoded {
            assert!(matches!(
                feature.kind,
                solidcraft_engine::io::f3d::HistoryKind::Extrude { operation: solidcraft_engine::doc::Operation::Cut, .. }
            ));
            assert!(!feature.dependencies.is_empty());
        }
        if let Some(original) = &original {
            assert_eq!(&decoded, original);
        } else {
            original = Some(decoded);
        }
    }
}

#[test]
#[ignore = "requires the local original adapter and unchanged public API resave"]
fn legacy_feature_mirror_preserves_join_source_and_targets() {
    use solidcraft_engine::io::f3d::HistoryKind;
    let corpus = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let mut original = None;
    for path in [corpus.join("adapter-x20-makita.f3d"), probes.join("corpus-adapter-x20-makita.f3d")] {
        let ds = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(&path).unwrap()).unwrap();
        let timeline = ds[0].timeline().unwrap();
        let source = timeline.iter().filter_map(|e| e.feature.as_ref().ok()).find(|f| f.name == "Extrude5").unwrap();
        let mirror = timeline
            .iter()
            .find(|e| e.class == "D1728651-3640-4CCF-8083-AF7703013978")
            .unwrap()
            .feature
            .as_ref()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            matches!(&mirror.kind, HistoryKind::MirrorFeatures { sources, targets, output_count:0, .. } if sources == &[source.id] && targets.len()==1)
        );
        assert_eq!(mirror.name, "Mirror1");
        assert!(!mirror.suppressed);
        let encoded = format!("{mirror:?}");
        if let Some(original) = &original {
            assert_eq!(&encoded, original);
        } else {
            original = Some(encoded);
        }
    }
}

#[test]
#[ignore = "requires local public API controls before and after consuming a mirrored body"]
fn consumed_mirror_output_does_not_suppress_the_timeline_feature() {
    let probes = std::path::PathBuf::from(solidcraft_engine::geom::env::var_os("F3D_PROBES").unwrap());
    let api: serde_json::Value = serde_json::from_slice(&std::fs::read(probes.join("fusion-consumed-mirror-measurements.json")).unwrap()).unwrap();
    let mut original = None;
    for (name, body_count) in [("mirror-consumed-before", 2), ("mirror-consumed-after", 1)] {
        assert_eq!(api[name]["body_count"], body_count);
        assert_eq!(api[name]["timeline"][2]["suppressed"], false);
        let ds = solidcraft_engine::io::f3d::read_f3d(&std::fs::read(probes.join(format!("{name}.f3d"))).unwrap()).unwrap();
        let timeline = ds[0].timeline().unwrap();
        let mirror = timeline.into_iter().nth(2).unwrap().feature.unwrap();
        assert_eq!(mirror.name, "Mirror1");
        assert!(!mirror.suppressed);
        assert!(matches!(mirror.kind, solidcraft_engine::io::f3d::HistoryKind::Mirror { output_count: 1, .. }));
        if let Some(original) = &original {
            assert_eq!(&mirror, original);
        } else {
            original = Some(mirror);
        }
    }
}
