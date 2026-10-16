//! Original small designs with analytic expectations, not captured kernel output.
use serde_json::{Value, json};
use solidcraft_engine::{Session, doc::Document};
use std::path::{Path, PathBuf};

fn recipes() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/history");
    let mut paths: Vec<_> =
        std::fs::read_dir(dir).unwrap().map(|p| p.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    paths.sort();
    assert_eq!(paths.len(), 178);
    if let Some(case) = solidcraft_engine::geom::env::var("HISTORY_CASE") {
        paths.retain(|path| path.file_stem().is_some_and(|name| name == case.as_str()));
        assert_eq!(paths.len(), 1, "HISTORY_CASE must select one existing recipe");
    }
    paths
}

fn recipe(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn replay(value: &Value) -> Session {
    let mut s = Session::default();
    for command in value["commands"].as_array().unwrap() {
        s.execute(command["command"].as_str().unwrap(), &command["params"]).unwrap_or_else(|e| panic!("{command}: {e}"));
    }
    s
}

fn expected_volume(value: &Value) -> f64 {
    value["volume_mm3"].as_f64().unwrap_or_else(|| value["volume_pi_mm3"].as_f64().unwrap() * std::f64::consts::PI)
}

fn volume(s: &mut Session, want: f64, label: &str) -> Value {
    for result in &s.model.results {
        assert!(result.error.is_none(), "{label}: feature {} ({}): {:?}", result.name, result.id, result.error);
    }
    let measure = s.execute("inspect.measure", &json!({})).unwrap();
    let got = measure["total"]["volume_mm3"].as_f64().unwrap();
    // inspect.measure integrates a tessellation of curved faces. The 0.1%
    // tolerance is the existing design/oracle tolerance, not a saved result.
    assert!((got - want).abs() < want.abs().max(1.0) * 1e-3, "{label}: volume {got} != {want}");
    measure
}

fn construction_points(s: &Session, expected: &Value, label: &str) {
    let Some(points) = expected["construction_points"].as_object() else { return };
    let state = s.model.state();
    for (name, value) in points {
        let actual = state.construct.iter().find(|c| &c.name == name).map(|c| &c.geom);
        let Some(solidcraft_engine::doc::construct::ConstructGeom::Point(p)) = actual else {
            panic!("{label}: missing point {name}");
        };
        let expected = solidcraft_engine::geom::Vec3::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap(), value[2].as_f64().unwrap());
        assert!(p.dist(expected) < 1e-6, "{label}: {name} {p:?} != {expected:?}");
    }
}

fn bounds(measure: &Value, expected: &Value, label: &str) {
    if let Some(center) = expected["center_of_mass"].as_array() {
        let total = measure["total"]["volume_mm3"].as_f64().unwrap();
        let tolerance = expected["center_of_mass_tolerance_mm"].as_f64().unwrap_or(1e-5);
        for (axis, expected) in center.iter().enumerate() {
            let moment: f64 = measure["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|body| body["volume_mm3"].as_f64().unwrap() * body["center_of_mass"][axis].as_f64().unwrap())
                .sum();
            let actual = moment / total;
            assert!((actual - expected.as_f64().unwrap()).abs() < tolerance, "{label}: center of mass axis {axis}: {actual}");
        }
    }
    if let Some(tolerance) = expected["volume_tolerance_mm3"].as_f64() {
        let actual = measure["total"]["volume_mm3"].as_f64().unwrap();
        assert!(
            (actual - expected_volume(expected)).abs() < tolerance,
            "{label}: precise removal volume {actual} != {} (tolerance {tolerance})",
            expected_volume(expected)
        );
    }
    if let Some(count) = expected.get("body_count") {
        assert_eq!(&measure["body_count"], count, "{label}: body count");
    }
    let Some(bounds) = expected["bounds"].as_array() else { return };
    // Curved-face measurement bounds come from the display tessellation. Keep
    // exact bounds as the default; curved fixtures can state their chord tolerance.
    let tolerance = expected["bounds_tolerance_mm"].as_f64().unwrap_or(1e-5);
    for (end, key) in [(0, "min"), (1, "max")] {
        for axis in 0..3 {
            let value = measure["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b["bbox"][key][axis].as_f64().unwrap())
                .reduce(|a, b| if end == 0 { a.min(b) } else { a.max(b) })
                .unwrap();
            assert!((value - bounds[end][axis].as_f64().unwrap()).abs() < tolerance, "{label}: bounds {end}/{axis}: {value}");
        }
    }
}

// Old fixtures retain legacy name lookup and shell ordering after migration.
// Compare the recipe inputs independently of those compatibility markers, then
// evaluate the saved fixture itself through every edit, reopen and undo below.
fn recipe_structure(session: &Session) -> Value {
    fn unmark(value: &mut Value) {
        match value {
            Value::String(s) if s.starts_with('?') => {
                s.remove(0);
            }
            Value::Array(values) => values.iter_mut().for_each(unmark),
            _ => {}
        }
    }
    fn references(value: &mut Value) {
        match value {
            Value::Object(o) => {
                if matches!(o.get("type").and_then(Value::as_str), Some("face" | "edge"))
                    && o.contains_key("at")
                    && let Some(name) = o.get_mut("name")
                {
                    unmark(name);
                }
                o.values_mut().for_each(references);
            }
            Value::Array(values) => values.iter_mut().for_each(references),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(&session.doc).unwrap();
    for feature in value["features"].as_array_mut().unwrap() {
        let id = feature["id"].as_u64().unwrap();
        feature.as_object_mut().unwrap().remove("shell_piece_order");
        // The new convention only changes symmetric distance extents. Compare
        // old one-sided fixtures with new command output without rewriting them.
        if let Some(extent) = feature.get_mut("extent").and_then(Value::as_object_mut)
            && extent.get("direction").and_then(Value::as_str) != Some("symmetric")
        {
            extent.remove("convention");
        }
        for key in ["face_names", "edge_names"] {
            if let Some(names) = feature.get_mut(key) {
                unmark(names);
            }
        }
        if let Some(links) = feature.get_mut("sketch").and_then(|s| s.get_mut("links")).and_then(Value::as_array_mut) {
            let before = session.model.state_before(id);
            let sketch = session.model.results.iter().find(|r| r.id == id).unwrap().state.sketch(id).unwrap();
            for link in links {
                let source = session.doc.sketch(id).unwrap().links.iter().find(|l| l.id == link["id"].as_str().unwrap()).unwrap();
                // Old and current numeric face pieces may have different names.
                // Resolve the explicit legacy edge through its exact name table,
                // then compare that edge's current identity; never repick by point.
                let canonical = |value: &mut Value| {
                    let Some(name) = value.as_str().filter(|n| n.starts_with('?')) else { return };
                    let resolved =
                        solidcraft_engine::doc::project::resolve_exact_named_edge(&before, &sketch.plane, source.kind, &source.source, name).unwrap();
                    let solidcraft_engine::sketch::LinkSource::Edge { body, .. } = &source.source else { panic!("edge source") };
                    let names = solidcraft_engine::doc::naming::edge_names(before.body(body).unwrap());
                    *value = json!(names[resolved.edge_index.unwrap()]);
                };
                if let Some(name) = link.get_mut("edge_name") {
                    canonical(name);
                }
                if let Some(names) = link.get_mut("edge_circle").and_then(Value::as_array_mut) {
                    names.iter_mut().for_each(canonical);
                }
                for key in ["face_name", "edge_name", "edge_circle"] {
                    if let Some(names) = link.get_mut(key) {
                        unmark(names);
                    }
                }
                // Circular edges form a set; incidence determines chain order.
                if let Some(edges) = link.get_mut("edge_circle").and_then(Value::as_array_mut) {
                    edges.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                }
            }
        }
        references(feature);
    }
    value
}

#[test]
fn legacy_offset_expression_history_still_edits_and_reopens() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/history/offset-circle-ring.json");
    let recipe = recipe(&path);
    let native = path.parent().unwrap().join("legacy/offset-circle-ring.solidcraft");
    let mut s = Session::default();
    s.execute("doc.open", &json!({"path":native})).unwrap();
    let expected = &recipe["expected"];
    let original = expected_volume(expected);
    bounds(&volume(&mut s, original, "legacy offset"), expected, "legacy offset");
    for edit in recipe["edits"].as_array().unwrap() {
        s.execute(edit["command"].as_str().unwrap(), &edit["params"]).unwrap();
        let expected_volume = expected_volume(edit);
        bounds(&volume(&mut s, expected_volume, "legacy offset edit"), edit, "legacy offset edit");
        let bytes = solidcraft_engine::io::write_design_container(&s.doc).unwrap();
        let mut reopened = Session::new(solidcraft_engine::io::read_design(&bytes).unwrap());
        assert_eq!(s.doc, reopened.doc);
        bounds(&volume(&mut reopened, expected_volume, "legacy offset reopen"), edit, "legacy offset reopen");
        s.execute("edit.undo", &json!({})).unwrap();
        bounds(&volume(&mut s, original, "legacy offset undo"), expected, "legacy offset undo");
    }
}

#[test]
fn simple_history_files_replay_reopen_edit_and_undo() {
    for path in recipes() {
        let label = path.display().to_string();
        let recipe = recipe(&path);
        let native = path.with_extension("solidcraft");
        let mut rebuilt = replay(&recipe);
        let mut s = Session::default();
        s.execute("doc.open", &json!({"path":native})).unwrap();
        // Opening a design with the default title now uses its file stem.
        // Check that title explicitly, then compare the complete feature history.
        let title = if rebuilt.doc.name.trim().is_empty() || rebuilt.doc.name.trim() == "Untitled" {
            native.file_stem().unwrap().to_string_lossy().into_owned()
        } else {
            rebuilt.doc.name.clone()
        };
        assert_eq!(s.doc.name, title, "{label}: file title");
        let mut rebuilt_history = recipe_structure(&rebuilt);
        rebuilt_history["name"] = json!(title);
        assert_recipe_equal(&recipe_structure(&s), &rebuilt_history, &label);
        let expected = &recipe["expected"];
        let original_volume = expected_volume(expected);
        bounds(&volume(&mut rebuilt, original_volume, &format!("{label}: fresh recipe")), expected, &label);
        let measure = volume(&mut s, original_volume, &label);
        assert_eq!(json!(s.doc.features.iter().map(|f| &f.name).collect::<Vec<_>>()), expected["features"], "{label}");
        if let Some(sketches) = expected["sketches"].as_array() {
            for check in sketches {
                let f = s.doc.find_feature(check["name"].as_str().unwrap()).unwrap();
                let sketch = s.doc.sketch(f.id).unwrap();
                assert_eq!(json!(sketch.curves.len()), check["curves"], "{label}");
                assert_eq!(json!(sketch.links.len()), check["links"], "{label}");
            }
        }
        // Aggregate world bounds catch incorrect sketch placement and direction.
        bounds(&measure, expected, &label);
        construction_points(&s, expected, &label);
        construction_points(&rebuilt, expected, &label);
        let packaged = solidcraft_engine::io::write_design_container(&s.doc).unwrap();
        assert_eq!(solidcraft_engine::io::read_design(&packaged).unwrap(), *s.doc, "{label}: container preserves history");
        for edit in recipe["edits"].as_array().unwrap() {
            let label = format!("{label}: edit {} {}", edit["command"], edit["params"]);
            s.execute(edit["command"].as_str().unwrap(), &edit["params"]).unwrap();
            let edited_volume = expected_volume(edit);
            bounds(&volume(&mut s, edited_volume, &label), edit, &label);
            construction_points(&s, edit, &label);
            let reopened = Document::from_json(&s.doc.to_json()).unwrap();
            assert_eq!(reopened, *s.doc);
            let packaged = solidcraft_engine::io::write_design_container(&reopened).unwrap();
            let container_doc = solidcraft_engine::io::read_design(&packaged).unwrap();
            assert_eq!(container_doc, reopened, "{label}: container preserves edited history");
            let mut reopened_session = Session::new(container_doc);
            bounds(&volume(&mut reopened_session, edited_volume, &label), edit, &label);
            construction_points(&reopened_session, edit, &label);
            s.execute("edit.undo", &json!({})).unwrap();
            bounds(&volume(&mut s, original_volume, &label), expected, &label);
            construction_points(&s, expected, &label);
        }
    }
}

fn assert_recipe_equal(saved: &Value, rebuilt: &Value, path: &str) {
    match (saved, rebuilt) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>(), "{path}: recipe fields");
            for (key, value) in a {
                assert_recipe_equal(value, &b[key], &format!("{path}/{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: recipe length");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                assert_recipe_equal(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Number(a), Value::Number(b)) if a.is_f64() && b.is_f64() => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            // Constructive and numerical solving can differ by a few ULPs in
            // saved coordinates (e.g. sqrt(800)). Keep IDs and expressions exact;
            // this only accommodates floating-point roundoff, not solver drift.
            let tolerance = 32. * f64::EPSILON * a.abs().max(b.abs()).max(1.);
            assert!((a - b).abs() <= tolerance, "{path}: authored recipe differs: {a} != {b}");
        }
        _ => assert_eq!(saved, rebuilt, "{path}: authored recipe differs"),
    }
}

#[test]
#[ignore = "explicitly author original SolidCraft fixtures from the checked-in recipes"]
fn write_original_history_fixtures() {
    for path in recipes() {
        if solidcraft_engine::geom::env::var("AUTHOR_PREFIX").is_some_and(|prefix| !path.file_stem().unwrap().to_string_lossy().starts_with(&prefix))
        {
            continue;
        }
        let s = replay(&recipe(&path));
        std::fs::write(path.with_extension("solidcraft"), solidcraft_engine::io::write_design(&s.doc)).unwrap();
    }
}
