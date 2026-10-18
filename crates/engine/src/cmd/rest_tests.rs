//! Acceptance checks for the remaining surface, sheet, drawing, machining and appearance work.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

#[test]
fn a_four_wall_tray_unfolds_and_a_rip_undoes() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Base"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [100, 60]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "sheet.flange", json!({"sketch": "Base"}));
    run(&mut s, "sheet.flange", json!({"edges": [[50, 60, 2.5], [100, 30, 2.5], [50, 0, 2.5], [0, 30, 2.5]], "height": 25}));
    let before = s.doc.features.len();
    run(&mut s, "sheet.unfold", json!({}));
    assert_eq!(s.doc.features.len(), before + 1);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc.features.len(), before, "one undo removes the unfold");
    run(&mut s, "sheet.rip", json!({"at": [0, 0, 2.5], "gap": "2.5 mm"}));
    assert_eq!(s.doc.features.len(), before + 1);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc.features.len(), before, "one undo removes the rip");
    run(&mut s, "sheet.rip", json!({"at": [0, 0, 2.5], "gap": "2.5 mm"}));
    run(&mut s, "sheet.unfold", json!({}));
    assert!(s.doc.features.iter().any(|f| matches!(f.kind, solidcraft_doc::FeatureKind::SheetUnfold { .. })));
}

#[test]
fn a_lofted_flange_uses_the_rule_thickness_and_a_closed_contour_is_refused() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "A"}));
    let a = run(&mut s, "sketch.line", json!({"points": [[0, 0], [20, 0]]}))["curves"][0].as_str().unwrap().to_string();
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "B"}));
    let b = run(&mut s, "sketch.line", json!({"points": [[0, 10], [20, 10]]}))["curves"][0].as_str().unwrap().to_string();
    run(&mut s, "sketch.finish", json!({}));
    let made = run(&mut s, "sheet.lofted_flange", json!({"sketch_a": "A", "curves_a": [a], "sketch_b": "B", "curves_b": [b]}));
    assert!((made["developed_length"].as_f64().unwrap() - 20.0).abs() < 1e-6, "{made}");
    let volume = run(&mut s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    assert!((volume - 20.0 * 10.0 * 2.5).abs() < 5.0, "{volume}");

    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Loop"}));
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [10, 0], [10, 10], [0, 0]], "ids": ["c1", "c2", "c3"]}));
    run(&mut s, "sketch.finish", json!({}));
    let n = s.doc.features.len();
    assert!(s.execute("sheet.flange", &json!({"type": "contour", "sketch": "Loop", "curves": ["c1", "c2", "c3"], "distance": 5})).is_err());
    assert_eq!(s.doc.features.len(), n);
}

#[test]
fn a_base_view_outline_follows_the_model_and_a_pdf_exports() {
    let mut s = Session::default();
    run(&mut s, "parameters.add", json!({"name": "height", "expression": "10 mm", "unit": "mm"}));
    run(&mut s, "solid.box", json!({"length": 20, "width": 10, "height": "height", "body_name": "Block"}));
    run(&mut s, "drawing.create", json!({"name": "Sheet"}));
    run(&mut s, "drawing.base_view", json!({"direction": [0, -1, 0], "hidden": true, "scale": 2}));
    let short = s.doc.drawings[0].views[0].outline;
    assert!((short - 10.0).abs() < 1e-3, "{short}");
    assert!(s.doc.drawings[0].views[0].hidden);
    run(&mut s, "parameters.change", json!({"name": "height", "expression": "20 mm"}));
    let tall = s.doc.drawings[0].views[0].outline;
    assert!((tall - 20.0).abs() < 1e-3, "{tall}");
    run(&mut s, "drawing.break_view", json!({"stations": [4, 12]}));
    run(&mut s, "drawing.auxiliary", json!({"direction": [1, 0, 0]}));
    run(&mut s, "drawing.standard", json!({"standard": "first_angle"}));
    run(&mut s, "drawing.balloon", json!({"component": "1", "at": [5, 5]}));
    assert_eq!(s.doc.drawings[0].standard, "first_angle");
    let path = std::env::temp_dir().join("solidcraft-drawing.pdf");
    run(&mut s, "drawing.export_pdf", json!({"path": path.to_string_lossy()}));
    let pdf = std::fs::read_to_string(&path).unwrap();
    assert!(pdf.contains("%PDF") && pdf.contains(" l "), "{pdf}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn machining_paths_stay_off_the_part_and_a_probe_writes_nothing() {
    let mut s = Session::default();
    run(&mut s, "cam.setup", json!({"stock": [50, 40, 20]}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Pocket"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [30, 20]}));
    run(&mut s, "sketch.finish", json!({}));
    let slot = run(&mut s, "cam.slot", json!({"sketch": "Pocket", "tool_diameter": 6, "depth": 4}));
    assert!((slot["area"].as_f64().unwrap() - 24.0 * 14.0).abs() < 1e-6, "{slot}");
    run(&mut s, "solid.sphere", json!({"radius": 10, "body_name": "Ball"}));
    let path = run(&mut s, "cam.parallel", json!({"body": "Ball", "tool_diameter": 4}));
    for sample in path["samples"].as_array().unwrap() {
        let (x, y, z) = (sample[0].as_f64().unwrap(), sample[1].as_f64().unwrap(), sample[2].as_f64().unwrap());
        let radius = (x * x + y * y + z * z).sqrt();
        assert!(radius >= 8.0, "{radius}");
    }
    let jobs = s.doc.cam.len();
    let probed = run(&mut s, "cam.probe", json!({"body": "Ball"}));
    assert!(probed["distance"].as_f64().unwrap() > 10.0);
    assert_eq!(s.doc.cam.len(), jobs);
    run(&mut s, "cam.pocket", json!({"sketch": "Pocket", "tool_diameter": 6, "depth": 10}));
    let sim = run(&mut s, "cam.simulate", json!({}));
    assert!(sim["removed"].as_f64().unwrap() > 1000.0, "{sim}");
    let posted = run(&mut s, "cam.post", json!({}));
    let gcode = posted["gcode"].as_str().unwrap();
    assert!(gcode.contains("G21") && gcode.contains("G54") && gcode.contains("M30"), "{gcode}");
    assert!(s.execute("cam.nest", &json!({"sheet": [10, 10], "profiles": [[8, 8], [8, 8]]})).is_err());
    let placed = run(&mut s, "cam.nest", json!({"sheet": [20, 10], "profiles": [[8, 8], [8, 8]]}));
    assert_eq!(placed["placed"].as_array().unwrap().len(), 2);
}

#[test]
fn an_appearance_keeps_metalness_and_a_bad_configuration_does_not_stick() {
    let mut s = Session::default();
    run(&mut s, "parameters.change", json!({"name": "length", "expression": "10 mm"}));
    run(&mut s, "solid.box", json!({"length": "length", "width": 10, "height": 10, "body_name": "Block"}));
    run(&mut s, "appearance.assign", json!({"bodies": ["Block"], "color": "#808080", "metalness": 0.8, "roughness": 0.2}));
    let look = s.doc.appearances.bodies.get("Block").unwrap();
    assert!((look.metalness - 0.8).abs() < 1e-9 && (look.roughness - 0.2).abs() < 1e-9);
    run(&mut s, "config.configure", json!({"params": ["length"]}));
    run(&mut s, "config.row", json!({"name": "Ok", "values": {"length": "12 mm"}}));
    run(&mut s, "config.row", json!({"name": "Bad", "values": {"length": "0 mm"}}));
    run(&mut s, "config.activate", json!({"row": "Ok"}));
    let active = s.doc.to_json();
    assert!(s.execute("config.activate", &json!({"row": "Bad"})).is_err());
    assert_eq!(s.doc.to_json(), active);
}

#[test]
fn editing_one_joint_occurrence_leaves_its_sibling_where_it_is() {
    let mut s = Session::default();
    for (name, x) in [("A", 0.0), ("B", 30.0)] {
        run(&mut s, "component.activate", json!({"component": "root"}));
        run(&mut s, "component.create", json!({"name": name}));
        run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [x, 0, 0]}));
    }
    run(&mut s, "component.activate", json!({"component": "root"}));
    let occ = |s: &Session, name: &str| {
        let component = s.doc.find_component(name).unwrap();
        s.doc.occurrence_of(component).unwrap().id
    };
    let a = occ(&s, "A");
    let b = occ(&s, "B");
    let before = s.doc.occurrences.iter().find(|o| o.id == b).unwrap().transform;
    run(&mut s, "joint.create", json!({"type": "rigid", "a": {"face": [10, 5, 5]}, "b": {"face": [30, 5, 5]}, "name": "Link"}));
    let placed = s.doc.occurrences.iter().find(|o| o.id == b).unwrap().transform;
    let current = s.doc.assembly.joints.iter().find(|j| j.name == "Link").unwrap().a.occurrence;
    run(&mut s, "joint.edit", json!({"joint": "Link", "path": [current], "side": "a", "name": "Link"}));
    let after = s.doc.occurrences.iter().find(|o| o.id == b).unwrap().transform;
    assert_eq!(placed, after, "editing one occurrence path does not move its sibling");
    assert_eq!(s.doc.assembly.joints.iter().find(|j| j.name == "Link").unwrap().a.occurrence, current);
    let _ = (a, before);
}

#[test]
fn a_plastic_draft_uses_the_active_rule() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 20, "width": 20, "height": 10, "body_name": "Shell"}));
    run(&mut s, "plastic.manage_rules", json!({"name": "ABS (1.5mm)", "active": true, "draft": "4 deg"}));
    run(&mut s, "plastic.draft", json!({"faces": [[20, 10, 5]], "neutral": "XY", "pull": [0, 0, 1]}));
    let angle = s.doc.features.iter().rev().find_map(|f| match &f.kind {
        solidcraft_doc::FeatureKind::Draft { angle, .. } => Some(angle.clone()),
        _ => None,
    });
    assert_eq!(angle.as_deref(), Some("4 deg"));
    run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 2, "corner": [40, 0, 0], "body_name": "Plate"}));
    run(&mut s, "plastic.boss", json!({"position": [60, 20, 2], "diameter": 8, "height": 10, "body": "Plate"}));
    let narrow = run(&mut s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    run(&mut s, "plastic.manage_rules", json!({"name": "ABS (1.5mm)", "draft": "20 deg"}));
    run(&mut s, "timeline.compute_all", json!({}));
    let wide = run(&mut s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    assert!((narrow - wide).abs() > 1.0, "a rule draft edit must rebuild the boss: {narrow} vs {wide}");
}
