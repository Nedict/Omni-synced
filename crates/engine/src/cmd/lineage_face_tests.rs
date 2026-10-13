//! Sketches on flat faces that no construction certificate names: a Cut's walls and floor, a
//! chamfer, a pattern's copies, a side a Join merged from two features, the pieces of a split
//! face (#491, #492, #493, #557). They attach by the face's exact history name
//! (`FaceCertificate::Lineage`), follow the face through parameter edits and survive a save.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use crate::Session;
use serde_json::{Value, json};
use solidcraft_doc::FaceAttachmentStatus;
use solidcraft_doc::config::FaceCertificate;

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap_or_else(|e| panic!("{command} {params}: {e}"))
}

fn sketch_on(s: &mut Session, at: [f64; 3], name: &str) {
    run(s, "sketch.create", json!({"plane": {"face": at}, "name": name}));
    run(s, "sketch.finish", json!({}));
}

fn status(s: &Session, sketch: &str) -> FaceAttachmentStatus {
    let id = s.doc.find_feature(sketch).map(|f| f.id).unwrap();
    s.placed_document().sketch_face_attachment(id, &s.model)
}

fn healthy(s: &Session, sketch: &str) -> (solidcraft_geom::Plane, Option<FaceCertificate>) {
    match status(s, sketch) {
        FaceAttachmentStatus::Healthy { plane, target, .. } => (plane, target.certificate),
        other => panic!("{sketch}: {other:?}"),
    }
}

fn no_errors(s: &Session) {
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
}

fn reopened(s: &Session) -> Session {
    let mut t = Session::new(solidcraft_doc::Document::from_json(&s.doc.to_json()).unwrap());
    t.refresh();
    no_errors(&t);
    t
}

/// A 40 x 30 x 10 block with a slot cut 5 deep across it from x = `slot_x` (a parameter), 10 wide.
fn slotted_block() -> Session {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [40, 30]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 10, "name": "Block"}));
    run(&mut s, "parameters.add", json!({"name": "slot_x", "expression": "15 mm"}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "SlotSketch"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [15, -5], "p1": [25, 35]}));
    run(&mut s, "sketch.dimension", json!({"entities": ["origin", "p1"], "type": "horizontal", "value": "slot_x"}));
    run(&mut s, "sketch.dimension", json!({"entities": ["l1"], "value": "10 mm"}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"sketch": "SlotSketch", "distance": 12, "start_offset": 5, "operation": "cut", "name": "Slot"}));
    no_errors(&s);
    s
}

#[test]
fn a_cut_wall_and_floor_take_a_sketch_that_follows_the_cut() {
    let mut s = slotted_block();
    sketch_on(&mut s, [15., 15., 7.5], "Wall");
    sketch_on(&mut s, [20., 15., 5.], "Floor");
    let (wall, certificate) = healthy(&s, "Wall");
    assert_eq!(certificate, Some(FaceCertificate::Lineage));
    // The wall faces into the slot (+x), its frame's origin over the design origin.
    assert!((wall.normal().x - 1.).abs() < 1e-9 && wall.origin.x.abs() > 14.9 && (wall.origin.x - 15.).abs() < 1e-9, "{wall:?}");
    assert!((wall.origin.y.abs() + wall.origin.z.abs()) < 1e-9, "{wall:?}");
    let (floor, _) = healthy(&s, "Floor");
    assert!((floor.normal().z - 1.).abs() < 1e-9 && (floor.origin.z - 5.).abs() < 1e-9, "{floor:?}");

    // The slot moves: the sketch on its wall moves with it.
    run(&mut s, "parameters.change", json!({"name": "slot_x", "expression": "20 mm"}));
    no_errors(&s);
    let (wall, _) = healthy(&s, "Wall");
    assert!((wall.origin.x - 20.).abs() < 1e-9, "{wall:?}");
    let t = reopened(&s);
    assert!((healthy(&t, "Wall").0.origin.x - 20.).abs() < 1e-9);
    assert!((healthy(&t, "Floor").0.origin.z - 5.).abs() < 1e-9);
}

#[test]
fn the_pieces_of_a_split_top_take_a_sketch() {
    let mut s = slotted_block();
    // The slot splits the block's top in two; each piece takes a sketch (on that face's plane).
    for (name, x) in [("Left", 5.), ("Right", 35.)] {
        sketch_on(&mut s, [x, 15., 10.], name);
        assert!((healthy(&s, name).0.origin.z - 10.).abs() < 1e-9, "{name}");
    }
    run(&mut s, "parameters.change", json!({"name": "slot_x", "expression": "20 mm"}));
    no_errors(&s);
    assert!((healthy(&s, "Left").0.origin.z - 10.).abs() < 1e-9);
}

#[test]
fn a_chamfer_face_takes_a_sketch_that_follows_the_chamfer() {
    let mut s = Session::default();
    run(&mut s, "parameters.add", json!({"name": "bevel", "expression": "3 mm"}));
    run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 10, "name": "Base"}));
    run(&mut s, "solid.chamfer", json!({"edges": [[40, 15, 10]], "distance": "bevel", "name": "Bevel"}));
    no_errors(&s);
    sketch_on(&mut s, [38.5, 15., 8.5], "OnBevel");
    let (plane, certificate) = healthy(&s, "OnBevel");
    assert_eq!(certificate, Some(FaceCertificate::Lineage));
    let n = std::f64::consts::FRAC_1_SQRT_2;
    assert!((plane.normal().x - n).abs() < 1e-9 && (plane.normal().z - n).abs() < 1e-9, "{plane:?}");
    // A bigger chamfer moves the face inward: the sketch goes with it.
    let height = |p: &solidcraft_geom::Plane| (p.origin - solidcraft_geom::Vec3::new(40., 0., 10.)).dot(p.normal());
    assert!((height(&plane) + 3. * n).abs() < 1e-9, "{plane:?}");
    run(&mut s, "parameters.change", json!({"name": "bevel", "expression": "5 mm"}));
    no_errors(&s);
    assert!((height(&healthy(&s, "OnBevel").0) + 5. * n).abs() < 1e-9);
    healthy(&reopened(&s), "OnBevel");
}

/// A plate with a boss patterned 3 times along x, `pitch` apart.
fn patterned_bosses(sketch_on_seed_first: bool) -> Session {
    let mut s = Session::default();
    run(&mut s, "parameters.add", json!({"name": "pitch", "expression": "20 mm"}));
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [80, 20]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 5}));
    run(&mut s, "sketch.create", json!({"plane": {"face": [40, 10, 5]}}));
    run(&mut s, "sketch.circle.center", json!({"center": [10, 10], "radius": 4}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 6, "operation": "join", "name": "Boss"}));
    if sketch_on_seed_first {
        sketch_on(&mut s, [10., 10., 11.], "Seed");
    }
    run(&mut s, "solid.pattern.rectangular", json!({"features": ["Boss"], "dir1": [1, 0, 0], "count1": 3, "spacing1": "pitch", "name": "Row"}));
    no_errors(&s);
    s
}

#[test]
fn each_patterned_copy_takes_its_own_sketch() {
    let mut s = patterned_bosses(true);
    // A sketch made on the seed before the pattern keeps its construction certificate: the
    // copies carry it too, but are named as copies.
    assert!(matches!(healthy(&s, "Seed").1, Some(FaceCertificate::ExtrudeCap { .. })));
    for (name, x) in [("First", 10.), ("Middle", 30.), ("Last", 50.)] {
        sketch_on(&mut s, [x, 10., 11.], name);
        assert!((healthy(&s, name).0.origin.z - 11.).abs() < 1e-9, "{name}");
    }
    run(&mut s, "parameters.change", json!({"name": "pitch", "expression": "25 mm"}));
    no_errors(&s);
    for name in ["Seed", "First", "Middle", "Last"] {
        healthy(&s, name);
    }
    let t = reopened(&s);
    for name in ["Seed", "Middle", "Last"] {
        healthy(&t, name);
    }
}

#[test]
fn a_piece_of_a_face_split_on_purpose_still_needs_a_fixed_plane() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Modified"}));
    run(&mut s, "solid.split_face", json!({"body": "Modified", "faces": [[5, 5, 10]], "plane": {"origin": [5, 0, 0], "normal": [1, 0, 0]}}));
    let error = s.execute("sketch.create", &json!({"plane": {"face": [2.5, 5, 10]}})).unwrap_err().to_string();
    assert!(error.contains("fixed:true"), "{error}");
}
