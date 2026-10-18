//! Managing projections: whole face boundaries against single edges, construction and profile
//! status, removing members and whole links, hiding, freezing, pending moves, repair and linked
//! design files.

use crate::Session;
use serde_json::{Value, json};
use solidcraft_geom::Vec2;
use solidcraft_sketch::CurveKind;

fn run(s: &mut Session, cmd: &str, params: Value) -> Value {
    s.execute(cmd, &params).unwrap_or_else(|e| panic!("{cmd} {params}: {e}"))
}

/// A 40 × 40 × 10 plate with a Ø10 hole through its middle (part A, placed twice: A:2 at
/// x = 100), and part B with a sketch on its XY plane. Returns (session, a1, a2, sketch).
fn holed_parts() -> (Session, u64, u64, u64) {
    let mut s = Session::default();
    let a1 = run(&mut s, "component.create", json!({"name":"A"}))["occurrence"].as_u64().unwrap();
    run(&mut s, "solid.box", json!({"length":40,"width":40,"height":10,"body_name":"Plate"}));
    run(&mut s, "solid.cylinder", json!({"radius":5,"height":10,"base":[20,20,0],"operation":"cut","targets":["Plate"]}));
    let a2 = run(&mut s, "occurrence.copy", json!({"occurrence":a1}))["occurrence"].as_u64().unwrap();
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[100,0,0]}));
    run(&mut s, "component.activate", json!({"component":0}));
    run(&mut s, "component.create", json!({"name":"B"}));
    let sketch = run(&mut s, "sketch.create", json!({"plane":"XY","name":"Receiver"}))["sketch"].as_u64().unwrap();
    (s, a1, a2, sketch)
}

fn top_face_of_second(s: &mut Session, a2: u64) -> String {
    let r = run(s, "sketch.project", json!({"refs":[{"face":[105,5,10],"body":"Plate","path":[a2]}]}));
    r["links"][0]["link"].as_str().unwrap().to_string()
}

/// The kinds of a link's curves in the evaluated sketch (lines, rounds).
fn shape(s: &Session, sketch: u64, link: &str) -> (usize, usize) {
    let st = s.model.state();
    let sk = &st.sketch(sketch).unwrap().sketch;
    let curves = sk.link_curves(link);
    let lines = curves.iter().filter(|c| matches!(sk.curves[**c].kind, CurveKind::Line { .. })).count();
    let rounds = curves.iter().filter(|c| matches!(sk.curves[**c].kind, CurveKind::Circle { .. } | CurveKind::Arc { .. })).count();
    (lines, rounds)
}

fn profiles(s: &Session, sketch: u64) -> usize {
    s.model.state().sketch(sketch).unwrap().profiles.len()
}

fn hole_centre(s: &Session, sketch: u64, link: &str) -> Vec2 {
    let st = s.model.state();
    let sk = &st.sketch(sketch).unwrap().sketch;
    sk.link_curves(link)
        .into_iter()
        .find_map(|c| match sk.curves[c].kind {
            CurveKind::Circle { c, .. } | CurveKind::Arc { c, .. } => sk.point(c),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_face_brings_every_boundary_loop_and_an_edge_only_itself() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let (lines, rounds) = shape(&s, sketch, &face);
    assert_eq!(lines, 4, "the outer boundary's four edges");
    assert!(rounds >= 1, "and the hole's loop");
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 20.0)).len() < 1e-6);
    // The plate's inner face and outer loop make a profile with the hole as an island.
    assert!(profiles(&s, sketch) >= 2, "the plate ring and the hole");
    let edge = run(&mut s, "sketch.project", json!({"refs":[{"edge":[110,0,10],"body":"Plate","path":[a2]}]}))["links"][0]["link"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(shape(&s, sketch, &edge), (1, 0), "an edge brings only itself");
    // Both follow A:2; the face keeps its hole where A:2 puts it.
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,7,0]}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 27.0)).len() < 1e-6);
    assert_eq!(shape(&s, sketch, &face).0, 4);
}

#[test]
fn construction_takes_projections_out_of_profiles_and_survives_moves_and_reopen() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let curves: Vec<String> = {
        let sk = s.doc.sketch(sketch).unwrap();
        sk.link_curves(&face).iter().map(|c| sk.curves[*c].id.clone()).collect()
    };
    assert!(profiles(&s, sketch) >= 1);
    run(&mut s, "sketch.construction", json!({"curves":curves,"value":true}));
    assert_eq!(profiles(&s, sketch), 0, "construction curves bound no profile");
    let sk = s.doc.sketch(sketch).unwrap();
    assert!(sk.link(&face).is_some(), "still linked");
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,5,0]}));
    run(&mut s, "sketch.update_references", json!({}));
    let check = |s: &Session| {
        let st = s.model.state();
        let sk = &st.sketch(sketch).unwrap().sketch;
        assert!(sk.link_curves(&face).iter().all(|c| sk.curves[*c].construction));
        assert!((hole_centre(s, sketch, &face) - Vec2::new(120.0, 25.0)).len() < 1e-6);
        assert_eq!(st.sketch(sketch).unwrap().profiles.len(), 0);
    };
    check(&s);
    let mut reopened = Session { doc: solidcraft_doc::format::read(&s.doc.to_json()).unwrap().into(), ..Default::default() };
    reopened.refresh();
    check(&reopened);
    // Back to normal lines: profiles again, still linked.
    run(&mut s, "sketch.edit", json!({"sketch":sketch}));
    run(&mut s, "sketch.construction", json!({"curves":curves,"value":false}));
    assert!(profiles(&s, sketch) >= 1);
}

#[test]
fn deleted_members_stay_deleted_and_shared_points_stay() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    // One outer edge of the face goes; its neighbours keep their shared corners.
    let (victim, kept_points) = {
        let sk = s.doc.sketch(sketch).unwrap();
        let line = sk.link_curves(&face).into_iter().find(|c| matches!(sk.curves[*c].kind, CurveKind::Line { .. })).unwrap();
        (sk.curves[line].id.clone(), sk.link_points(&face).len())
    };
    run(&mut s, "sketch.delete", json!({"entities":[victim]}));
    let after = |s: &Session| shape(s, sketch, &face);
    assert_eq!(after(&s).0, 3);
    assert_eq!(s.doc.sketch(sketch).unwrap().link_points(&face).len(), kept_points, "the corners are still used by the neighbours");
    // An update, a move of A:2 and a reopen do not bring it back.
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,3,0]}));
    run(&mut s, "sketch.update_references", json!({}));
    assert_eq!(after(&s).0, 3);
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 23.0)).len() < 1e-6);
    let mut reopened = Session { doc: solidcraft_doc::format::read(&s.doc.to_json()).unwrap().into(), ..Default::default() };
    reopened.refresh();
    assert_eq!(after(&reopened).0, 3);
    // Undo brings the member back.
    for _ in 0..3 {
        run(&mut s, "edit.undo", json!({}));
    }
    assert_eq!(after(&s).0, 4);
}

#[test]
fn removing_a_link_removes_its_geometry_and_constraints_but_break_keeps_it() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let edge = run(&mut s, "sketch.project", json!({"refs":[{"edge":[110,40,10],"body":"Plate","path":[a2]}]}))["links"][0]["link"]
        .as_str()
        .unwrap()
        .to_string();
    let line = run(&mut s, "sketch.line", json!({"points":[[0,0],[30,0]]}))["curves"][0].as_str().unwrap().to_string();
    let projected = {
        let sk = s.doc.sketch(sketch).unwrap();
        sk.curves[sk.link_curves(&face)[0]].id.clone()
    };
    run(&mut s, "sketch.constraint.parallel", json!({"a":line,"b":projected}));
    let before = s.doc.sketch(sketch).unwrap().constraints.len();
    let r = run(&mut s, "sketch.remove_link", json!({"link":face}));
    assert_eq!(r["removed"], json!([face]));
    let sk = s.doc.sketch(sketch).unwrap();
    assert!(sk.link(&face).is_none());
    assert!(sk.curves.iter().all(|c| c.link.as_deref() != Some(face.as_str())));
    assert!(sk.points.iter().all(|p| p.link.as_deref() != Some(face.as_str())));
    assert!(sk.constraints.len() < before, "the constraint on it went");
    assert!(sk.curve_index(&line).is_some(), "other geometry stays");
    assert!(sk.link(&edge).is_some(), "other links stay");
    // Break Link keeps the geometry as normal curves.
    let n = s.doc.sketch(sketch).unwrap().link_curves(&edge).len();
    run(&mut s, "sketch.break_link", json!({"link":edge}));
    let sk = s.doc.sketch(sketch).unwrap();
    assert!(sk.link(&edge).is_none());
    assert_eq!(sk.curves.iter().filter(|c| c.link.is_none()).count(), 1 + n);
    // Updating references brings neither back.
    run(&mut s, "sketch.update_references", json!({}));
    assert!(s.doc.sketch(sketch).unwrap().links.is_empty());
}

#[test]
fn hiding_a_link_changes_only_what_is_shown() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let n = profiles(&s, sketch);
    let r = run(&mut s, "sketch.link_visibility", json!({"link":face}));
    assert_eq!(r["visible"], false);
    let st = s.model.state();
    let sk = &st.sketch(sketch).unwrap().sketch;
    assert!(sk.link_curves(&face).iter().all(|c| !crate::view::sketch_curve_visible(sk, *c)));
    let points = crate::view::SketchPointVisibility::new(sk);
    assert!(sk.link_points(&face).iter().all(|p| !points.takes(&sk.points[*p])));
    assert_eq!(profiles(&s, sketch), n, "profiles are untouched");
    let refs = run(&mut s, "sketch.references", json!({}));
    assert_eq!(refs["references"][0]["hidden"], true);
    // It still follows its source while hidden.
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,2,0]}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 22.0)).len() < 1e-6);
    run(&mut s, "sketch.link_visibility", json!({"link":face,"visible":true}));
    let st = s.model.state();
    let sk = &st.sketch(sketch).unwrap().sketch;
    assert!(sk.link_curves(&face).iter().all(|c| crate::view::sketch_curve_visible(sk, *c)));
}

#[test]
fn freeze_holds_the_geometry_and_resume_follows_again() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    run(&mut s, "sketch.freeze_link", json!({"link":face}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,9,0]}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 20.0)).len() < 1e-6, "frozen");
    let refs = run(&mut s, "sketch.references", json!({"sketch":sketch}));
    assert_eq!(refs["references"][0]["status"], "frozen");
    let saved = solidcraft_doc::format::read(&s.doc.to_json()).unwrap();
    assert!(saved.sketch(sketch).unwrap().link(&face).unwrap().frozen);
    run(&mut s, "sketch.resume_link", json!({"sketch":sketch,"link":face}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 29.0)).len() < 1e-6, "follows again");
    run(&mut s, "edit.undo", json!({}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 20.0)).len() < 1e-6, "undo freezes it again");
}

#[test]
fn pending_moves_are_reported_and_captured_by_update() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,4,0],"capture":false}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 20.0)).len() < 1e-6, "a pending move is not followed");
    let refs = run(&mut s, "sketch.references", json!({}));
    assert_eq!(refs["references"][0]["status"], "pending");
    assert_eq!(refs["references"][0]["occurrence"]["name"], "A:2");
    let r = run(&mut s, "sketch.update_references", json!({"capture":true}));
    assert_eq!(r["captured"], json!([a2]));
    assert_eq!(r["ok"], true, "{r}");
    assert!(s.pending_moves.is_empty());
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 24.0)).len() < 1e-6);
    // The stored sketch has it too: current, not stale.
    let refs = run(&mut s, "sketch.references", json!({}));
    assert_eq!(refs["references"][0]["status"], "current");
    run(&mut s, "edit.undo", json!({}));
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 20.0)).len() < 1e-6);
}

#[test]
fn stale_then_lost_keeps_the_updated_geometry_and_repair_reconnects() {
    let (mut s, a1, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"translate":[0,6,0]}));
    assert_eq!(run(&mut s, "sketch.references", json!({}))["references"][0]["status"], "stale");
    let r = run(&mut s, "sketch.update_references", json!({}));
    assert_eq!(r["links"][0]["status"], "changed", "{r}");
    run(&mut s, "selection.delete", json!({"items":[{"type":"occurrence","path":[a2]}]}));
    let r = run(&mut s, "sketch.update_references", json!({}));
    assert_eq!(r["ok"], false);
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(120.0, 26.0)).len() < 1e-6, "last good geometry");
    let ids: Vec<String> =
        s.doc.sketch(sketch).unwrap().link_curves(&face).iter().map(|c| s.doc.sketch(sketch).unwrap().curves[*c].id.clone()).collect();
    // Repaired onto A:1's top face: same shape, same ids.
    run(&mut s, "sketch.edit", json!({"sketch":sketch}));
    let r = run(&mut s, "sketch.repair_link", json!({"link":face,"ref":{"face":[5,5,10],"body":"Plate","path":[a1]}}));
    assert_eq!(r["rebuilt"], false, "{r}");
    let sk = s.doc.sketch(sketch).unwrap();
    assert!(!sk.link(&face).unwrap().lost);
    assert_eq!(sk.link(&face).unwrap().placement.as_ref().unwrap().source, vec![a1]);
    let now: Vec<String> = sk.link_curves(&face).iter().map(|c| sk.curves[*c].id.clone()).collect();
    assert_eq!(now, ids);
    assert!((hole_centre(&s, sketch, &face) - Vec2::new(20.0, 20.0)).len() < 1e-6);
}

#[test]
fn update_with_files_reports_an_unavailable_design_and_keeps_geometry() {
    let dir = std::env::temp_dir().join(format!("solidcraft-placed-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let part = dir.join("bar.solidcraft");
    let save = |length: f64| {
        let mut p = Session::default();
        run(&mut p, "solid.box", json!({"length":length,"width":5,"height":5,"body_name":"Bar"}));
        run(&mut p, "file.save", json!({"path":part}));
    };
    save(10.0);
    let mut s = Session::default();
    let inserted = run(&mut s, "component.insert_design", json!({"path":part}));
    let first = inserted["occurrence"].as_u64().unwrap();
    let second = run(&mut s, "occurrence.copy", json!({"occurrence":first}))["occurrence"].as_u64().unwrap();
    run(&mut s, "occurrence.move", json!({"occurrence":second,"translate":[0,50,0]}));
    run(&mut s, "component.activate", json!({"component":0}));
    run(&mut s, "component.create", json!({"name":"B"}));
    let sketch = run(&mut s, "sketch.create", json!({"plane":"XY"}))["sketch"].as_u64().unwrap();
    let body = inserted["bodies"][0].as_str().unwrap().to_string();
    let link = run(&mut s, "sketch.project", json!({"refs":[{"edge":[5,50,5],"body":body,"path":[second]}]}))["links"][0]["link"]
        .as_str()
        .unwrap()
        .to_string();
    run(&mut s, "sketch.finish", json!({}));
    let length = |s: &Session| {
        let st = s.model.state();
        let sk = &st.sketch(sketch).unwrap().sketch;
        let CurveKind::Line { a, b } = sk.curves[sk.link_curves(&link)[0]].kind else { panic!() };
        sk.point(a).unwrap().dist(sk.point(b).unwrap())
    };
    assert!((length(&s) - 10.0).abs() < 1e-6);
    save(16.0);
    let r = run(&mut s, "sketch.update_references", json!({"files":true}));
    assert_eq!(r["ok"], true, "{r}");
    assert!((length(&s) - 16.0).abs() < 1e-6, "the edited design's edge, at the second placement");
    std::fs::remove_file(&part).unwrap();
    let r = run(&mut s, "sketch.update_references", json!({"files":true}));
    assert_eq!(r["ok"], false);
    assert_eq!(r["files"]["unavailable"].as_array().unwrap().len(), 1, "{r}");
    assert!((length(&s) - 16.0).abs() < 1e-6, "the last bodies stay usable");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn management_commands_refuse_bad_input_without_changes() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let before = s.doc.to_json();
    for (cmd, p) in [
        ("sketch.remove_link", json!({"link":"nope"})),
        ("sketch.freeze_link", json!({})),
        ("sketch.resume_link", json!({"entities":["nope"]})),
        ("sketch.link_visibility", json!({"links":[face, 7]})),
        ("sketch.repair_link", json!({"link":face})),
        ("sketch.repair_link", json!({"link":face,"ref":{"edge":[1e300,0,0],"body":"Plate","path":[a2, a2]}})),
        ("sketch.references", json!({"sketch":"no such"})),
        ("sketch.update_references", json!({"sketch":sketch + 1000})),
    ] {
        assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}");
        assert_eq!(s.doc.to_json(), before, "{cmd}");
    }
}

#[test]
fn a_turned_source_keeps_its_face_projection_and_ids() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let ids = |s: &Session, stored: bool| -> Vec<String> {
        let st = s.model.state();
        let sk = if stored { s.doc.sketch(sketch).unwrap().clone() } else { st.sketch(sketch).unwrap().sketch.clone() };
        sk.link_curves(&face).iter().map(|c| sk.curves[*c].id.clone()).collect()
    };
    let before = ids(&s, true);
    run(&mut s, "sketch.finish", json!({}));
    // A:2 turned 30 degrees about its corner at (100, 0): the hole centre (120, 20) turns with it.
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"space":"world","axis":[0,0,1],"angle":"30 deg","origin":[100,0,0]}));
    let (c, n) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let turned = Vec2::new(100.0 + 20.0 * c - 20.0 * n, 20.0 * n + 20.0 * c);
    assert!(!s.model.state().sketch(sketch).unwrap().sketch.link(&face).unwrap().lost);
    assert!((hole_centre(&s, sketch, &face) - turned).len() < 1e-6, "{:?}", hole_centre(&s, sketch, &face));
    assert_eq!(ids(&s, false), before, "the same curves, turned");
    let r = run(&mut s, "sketch.update_references", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(ids(&s, true), before);
    // Turned again, saved and reopened: still the same curves.
    run(&mut s, "occurrence.move", json!({"occurrence":a2,"space":"world","axis":[0,0,1],"angle":"95 deg","origin":[100,0,0]}));
    let mut reopened = Session { doc: solidcraft_doc::format::read(&s.doc.to_json()).unwrap().into(), ..Default::default() };
    reopened.refresh();
    assert!(!reopened.model.state().sketch(sketch).unwrap().sketch.link(&face).unwrap().lost);
    assert_eq!(ids(&reopened, false), before);
}

#[test]
fn a_tilted_source_changes_shape_visibly_and_never_panics() {
    let (mut s, _, a2, sketch) = holed_parts();
    let face = top_face_of_second(&mut s, a2);
    let edge = run(&mut s, "sketch.project", json!({"refs":[{"edge":[110,0,10],"body":"Plate","path":[a2]}]}))["links"][0]["link"]
        .as_str()
        .unwrap()
        .to_string();
    run(&mut s, "sketch.finish", json!({}));
    for angle in ["20 deg", "70 deg", "90 deg"] {
        run(&mut s, "occurrence.move", json!({"occurrence":a2,"space":"world","axis":[1,0,0],"angle":angle,"origin":[100,0,0]}));
        let r = run(&mut s, "sketch.update_references", json!({}));
        let refs = run(&mut s, "sketch.references", json!({}));
        for entry in refs["references"].as_array().unwrap() {
            assert!(["current", "lost"].contains(&entry["status"].as_str().unwrap()), "{angle}: {entry} {r}");
        }
    }
    // The edge stays a line (its projection shortens or collapses to a point, which is lost).
    let st = s.model.state();
    let sk = &st.sketch(sketch).unwrap().sketch;
    assert!(sk.link(&edge).is_some() && sk.link(&face).is_some());
}
