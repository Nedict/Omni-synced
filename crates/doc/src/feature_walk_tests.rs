//! Golden records of what the walks over a feature report, for every variant (and every fillet
//! style, pattern, hole and construction definition):
//! - expressions: `inputs` (label, expression, kind, in order: the order names the `dN`
//!   parameters) and `expressions` (every expression, placement points included). Each
//!   expression field holds its own name, so a moved, missing or relabelled input shows;
//! - references: the sketches a feature is built from, and the fields `remap_refs` and
//!   `remap_bodies` rewrite when everything is copied under new ids and names;
//! - geometry: the fields `transform_geometry` moves, the face points, the profile selections and
//!   the placement point.
//!
//! The records live in `feature_walk_tests/*.txt`; to record an intended change, run the tests
//! with `FEATURE_WALK_BLESS=1`.

mod fixtures;

use std::collections::BTreeMap;

use fixtures::{changed, fixtures, with_points, with_refs};

use crate::FeatureKind;

/// Per fixture: its inputs, one per line (`label = expression (kind)`), then every expression.
fn record() -> String {
    let mut out = String::new();
    for (name, v) in fixtures() {
        let kind: FeatureKind = serde_json::from_value(v).unwrap_or_else(|e| panic!("{name}: {e}"));
        out += &format!("{name}\n");
        let inputs = kind.inputs();
        if !inputs.is_empty() {
            out += "  inputs:\n";
            for (label, e, k) in inputs {
                out += &format!("    {label} = {e} ({k:?})\n");
            }
        }
        let expressions = kind.expressions();
        if !expressions.is_empty() {
            out += &format!("  expressions: {}\n", expressions.join(", "));
        }
    }
    out
}

/// Per fixture: the sketches it is built from, and the fields that copying with every sketch,
/// feature and body renamed rewrites (`remap_refs`: sketches and features; `remap_bodies`).
fn ref_record() -> String {
    let sketches: BTreeMap<u64, u64> = (1..=8).map(|i| (i, i + 100)).collect();
    let names = BTreeMap::from([("F".to_string(), "F2".to_string()), ("CP".to_string(), "CP2".to_string())]);
    let bodies = BTreeMap::from([("B".to_string(), "B2".to_string())]);
    let mut out = String::new();
    for (name, v) in fixtures() {
        let kind: FeatureKind = serde_json::from_value(with_refs(name, v)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let before = serde_json::to_value(&kind).unwrap();
        let mut refs = kind.clone();
        refs.remap_refs(&sketches, &names);
        let mut by_refs = Vec::new();
        changed(&before, &serde_json::to_value(&refs).unwrap(), "", &mut by_refs);
        let mut body = kind.clone();
        body.remap_bodies(&bodies);
        let mut by_bodies = Vec::new();
        changed(&before, &serde_json::to_value(&body).unwrap(), "", &mut by_bodies);
        out += &format!("{name}\n");
        let sketches = kind.input_sketches();
        if !sketches.is_empty() {
            out += &format!("  sketches: {sketches:?}\n");
        }
        if !by_refs.is_empty() {
            out += &format!("  remap_refs: {}\n", by_refs.join(", "));
        }
        if !by_bodies.is_empty() {
            out += &format!("  remap_bodies: {}\n", by_bodies.join(", "));
        }
    }
    out
}

/// A variant with a field that refers to a sketch (a `SketchId`, sketch points or loft
/// sections) must report it from `input_sketches`, or deleting the sketch leaves the feature
/// broken. New variants are caught by the walk's exhaustive match; this catches a
/// sketch field added to an existing variant (its fixture has to set it).
/// Per fixture (with every reference and position set): the fields `transform_geometry` changes
/// under a turn and a shift (every point, direction and plane it maps), the face points that name
/// faces, the profile selections with their sketches, and a primitive's placement point.
fn geometry_record() -> String {
    // x → y → z → x, then (1, 2, 3): every point and every direction of the fixtures changes.
    let m = [[0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 0.0, 0.0, 0.0], [1.0, 2.0, 3.0, 1.0]];
    let mut out = String::new();
    for (name, v) in fixtures() {
        let kind: FeatureKind = serde_json::from_value(with_points(name, with_refs(name, v))).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut moved = kind.clone();
        moved.transform_geometry(&m);
        let mut paths = Vec::new();
        changed(&serde_json::to_value(&kind).unwrap(), &serde_json::to_value(&moved).unwrap(), "", &mut paths);
        out += &format!("{name}\n");
        if !paths.is_empty() {
            out += &format!("  moves: {}\n", paths.join(", "));
        }
        let faces: Vec<String> = kind.face_points().iter().map(|p| format!("[{}, {}, {}]", p.x, p.y, p.z)).collect();
        if !faces.is_empty() {
            out += &format!("  face points: {}\n", faces.join(", "));
        }
        let profiles: Vec<String> = kind
            .profile_sels()
            .iter()
            .map(|(sketch, sel)| format!("{sketch} {}", serde_json::to_value(sel).unwrap()["type"].as_str().unwrap_or("?")))
            .collect();
        if !profiles.is_empty() {
            out += &format!("  profiles: {}\n", profiles.join(", "));
        }
        if let Some(p) = kind.point_expr() {
            out += &format!("  placement: {}\n", p.iter().map(|e| e.as_str()).collect::<Vec<_>>().join(", "));
        }
    }
    out
}

/// Compares `got` with the golden file `name` in `feature_walk_tests/`, line by line. With
/// `FEATURE_WALK_BLESS` set it rewrites the file instead, for an intended change (say why in the
/// pull request).
fn check_golden(name: &str, want: &str, got: &str) {
    if std::env::var_os("FEATURE_WALK_BLESS").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/feature_walk_tests").join(name);
        std::fs::write(&path, got).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        return;
    }
    for (line, (want, got)) in want.lines().zip(got.lines()).enumerate() {
        assert_eq!(want, got, "{name} line {}; rerun with FEATURE_WALK_BLESS=1 if the change is intended", line + 1);
    }
    assert_eq!(want.lines().count(), got.lines().count(), "{name}: the record has a different number of lines");
}

/// Recorded from the hand-written `inputs_mut` and `expressions` the walk replaced.
const GOLDEN_EXPRS: &str = include_str!("feature_walk_tests/golden_exprs.txt");

/// Recorded from the hand-written `transform_geometry`, `face_points`, `profile_sels` and
/// `point_expr` the walk replaced.
const GOLDEN_GEOMETRY: &str = include_str!("feature_walk_tests/golden_geometry.txt");

/// Recorded from the hand-written `input_sketches`, `remap_refs` and `remap_bodies` the walk
/// replaced, with two fixes: a coil's `targets` and a surface trim's plane are remapped too.
const GOLDEN_REFS: &str = include_str!("feature_walk_tests/golden_refs.txt");

#[test]
fn imported_groups_keep_saved_parameter_order_and_mutability() {
    use crate::expr::Kind::{Angle, Length};
    use serde_json::json;
    for (value, expected) in [
        (
            json!({"type":"fillet", "edges":[], "radius":"first", "style":{"type":"grouped", "groups":[
                {"count":1, "radius":"second"}, {"count":2, "radius":"third"}
            ]}}),
            vec![("Radius", "first", Length), ("Group radius", "second", Length), ("Group radius", "third", Length)],
        ),
        (
            json!({"type":"chamfer", "edges":[], "distance":"first", "groups":[
                {"count":1, "distance":"second", "distance2":"third", "angle":"fourth"}
            ]}),
            vec![
                ("Distance", "first", Length),
                ("Group distance", "second", Length),
                ("Group distance 2", "third", Length),
                ("Group angle", "fourth", Angle),
            ],
        ),
    ] {
        let mut kind: FeatureKind = serde_json::from_value(value).unwrap();
        assert_eq!(kind.inputs(), expected.iter().map(|(l, e, k)| (*l, e.to_string(), *k)).collect::<Vec<_>>());
        for (_, expression, _) in kind.inputs_mut() {
            *expression = format!("{expression}_edited").into();
        }
        assert_eq!(kind.expressions(), expected.iter().map(|(_, e, _)| format!("{e}_edited")).collect::<Vec<_>>());
    }
}

#[test]
fn face_loft_sections_do_not_reference_a_placeholder_sketch() {
    use serde_json::json;
    let mut kind: FeatureKind = serde_json::from_value(json!({"type":"loft", "sections":[
        {"face":{"target":{"body":"B", "origin_feature":2, "name":"cap", "point":[0,0,10]},
            "component":0, "path":[], "author_path":[], "coordinates":[0,0], "x_axis":[1,0]}},
        {"sketch":4, "profiles":{"type":"all"}},
        {"sketch":5, "point":"p1"}
    ]}))
    .unwrap();
    assert_eq!(kind.input_sketches(), [4, 5]);
    assert_eq!(kind.profile_sels().iter().map(|(s, _)| *s).collect::<Vec<_>>(), [4]);
    kind.remap_refs(&BTreeMap::from([(0, 100), (4, 104), (5, 105)]), &BTreeMap::new());
    assert_eq!(kind.input_sketches(), [104, 105]);
    assert_eq!(kind.profile_sels_mut().iter().map(|(s, _)| *s).collect::<Vec<_>>(), [104]);
    let FeatureKind::Loft { sections, .. } = kind else { panic!("expected loft") };
    assert_eq!(sections[0].sketch, 0);
}

#[test]
fn to_object_plane_keeps_references_without_renumbering_inputs() {
    use crate::walk::{Geo, Walk};
    use serde_json::json;
    let mut kind: FeatureKind = serde_json::from_value(json!({"type":"extrude", "sketch":1,
        "profiles":{"type":"all"}, "extent":{"distance":"height", "to_offset":"gap",
            "to_plane":{"type":"offset", "distance":"plane_offset", "base":{"type":"construction", "name":"CP"}},
            "to":[1,2,3], "from":[4,5,6]}}))
    .unwrap();
    assert_eq!(kind.expressions(), ["height", "gap"]);
    kind.remap_refs(&BTreeMap::from([(1, 101)]), &BTreeMap::from([("CP".into(), "CP2".into())]));
    assert_eq!(kind.input_sketches(), [101]);
    assert_eq!(serde_json::to_value(&kind).unwrap()["extent"]["to_plane"]["base"]["name"], "CP2");
    let mut planes = 0;
    kind.walk_geometry(&mut |g| {
        if matches!(g, Geo::Plane(_)) {
            planes += 1
        }
    });
    assert_eq!(planes, 1);
    kind.walk_geometry_mut(&mut |g| {
        if let Geo::Plane(p) = g {
            *p = crate::PlaneRef::Construction { name: "CP3".into() }
        }
    });
    assert_eq!(serde_json::to_value(&kind).unwrap()["extent"]["to_plane"]["name"], "CP3");
    assert_eq!(kind.face_points(), [crate::walk::Vec3::new(1., 2., 3.), crate::walk::Vec3::new(4., 5., 6.)]);
}

#[test]
fn legacy_offset_expressions_remain_visible_without_new_model_parameters() {
    let mut value = fixtures().into_iter().find(|(name, _)| *name == "sketch").unwrap().1;
    value["sketch"]["offsets"] = serde_json::json!([
        {"sources":[], "outputs":[], "distance":2, "distance_expression":"wall", "closed":true}
    ]);
    let kind: FeatureKind = serde_json::from_value(value).unwrap();
    assert_eq!(kind.expressions(), ["sketch.angle", "sketch.offset", "wall"]);
    assert_eq!(kind.inputs().len(), 2);
}

#[test]
fn text_and_imported_offset_dependencies_coexist_after_renaming() {
    let mut value = fixtures().into_iter().find(|(name, _)| *name == "sketch").unwrap().1;
    value["id"] = serde_json::json!(1);
    value["name"] = serde_json::json!("Mixed sketch");
    value["sketch"]["offsets"] = serde_json::json!([
        {"sources":[], "outputs":[], "distance":2, "distance_expression":"wall", "closed":true}
    ]);
    value["sketch"]["links"] = serde_json::json!([{
        "id":"j1", "kind":"project", "source":{
            "type":"text", "text":"A", "at":[0,0], "height":10, "angle":0,
            "exprs":{"height":"tall"}
        }
    }]);
    let mut doc = crate::Document::default();
    doc.add_user_param("wall", "2 mm", None, None).unwrap();
    doc.add_user_param("tall", "10 mm", None, None).unwrap();
    doc.features.push(serde_json::from_value(value).unwrap());
    for name in ["wall", "tall"] {
        assert!(doc.param_graph().contains(&("Feature:Mixed sketch".into(), name.into())));
        assert!(doc.features_using_param(name).contains(&1));
    }
    doc.rename_param("wall", "thickness").unwrap();
    doc.rename_param("tall", "letter_size").unwrap();
    for name in ["thickness", "letter_size"] {
        assert!(doc.param_graph().contains(&("Feature:Mixed sketch".into(), name.into())));
        assert!(doc.features_using_param(name).contains(&1));
    }
    assert!(doc.param_graph().iter().all(|(_, name)| name != "wall" && name != "tall"));
}

#[test]
fn input_sketches_lists_every_variant_with_a_sketch_field() {
    let src = include_str!("document.rs").replace("\r\n", "\n");
    let body = src.split("pub enum FeatureKind {").nth(1).and_then(|s| s.split("\n}\n").next()).unwrap();
    let mut with_sketch = Vec::new();
    let mut variant = String::new();
    for line in body.lines() {
        let t = line.trim();
        if line.starts_with("    ") && !line.starts_with("     ") && t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            variant = t.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_lowercase();
            continue;
        }
        let Some((_, ty)) = t.strip_prefix("pub ").unwrap_or(t).split_once(':') else { continue };
        let refers = ty.contains("SketchId") || ty.contains("SketchPoints") || ty.contains("LoftSection");
        if refers && !t.starts_with("//") && !with_sketch.contains(&variant) {
            with_sketch.push(variant.clone());
        }
    }
    assert!(with_sketch.len() >= 10, "the FeatureKind parse found too few sketch users: {with_sketch:?}");
    for v in with_sketch {
        let uses = fixtures().into_iter().filter(|(_, f)| f["type"].as_str().unwrap().replace('_', "") == v).any(|(name, f)| {
            let kind: FeatureKind = serde_json::from_value(with_refs(name, f)).unwrap();
            !kind.input_sketches().is_empty()
        });
        assert!(uses, "FeatureKind::{v} has a sketch field but input_sketches reports no sketch for its fixtures");
    }
}

#[test]
fn geometry_matches_the_golden_record() {
    check_golden("golden_geometry.txt", GOLDEN_GEOMETRY, &geometry_record());
}

#[test]
fn references_match_the_golden_record() {
    check_golden("golden_refs.txt", GOLDEN_REFS, &ref_record());
}

#[test]
fn inputs_and_expressions_match_the_golden_record() {
    check_golden("golden_exprs.txt", GOLDEN_EXPRS, &record());
}

#[test]
fn the_record_covers_every_variant() {
    let src = include_str!("document.rs").replace("\r\n", "\n");
    let body = src.split("pub enum FeatureKind {").nth(1).and_then(|s| s.split("\n}\n").next()).unwrap();
    let names: Vec<String> = fixtures().into_iter().map(|(_, v)| v["type"].as_str().unwrap().replace('_', "")).collect();
    for line in body.lines().filter(|l| l.starts_with("    ") && !l.starts_with("     ")) {
        let t = line.trim();
        if t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            let variant = t.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_lowercase();
            assert!(names.contains(&variant), "FeatureKind::{t} has no fixture in feature_walk_tests");
        }
    }
}
