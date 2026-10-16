use std::sync::Arc;

use serde_json::{Value, json};

use super::*;

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn report(s: &mut Session, settings: Option<Settings>) -> Report {
    let params = settings.map_or_else(|| json!({}), |settings| json!({"settings": settings}));
    serde_json::from_value(run(s, ESTIMATE, params)).unwrap()
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < expected.abs().max(1.0) * 1e-9, "{actual} != {expected}");
}

fn panel(s: &mut Session, name: &str) {
    run(s, "solid.box", json!({"length":100,"width":50,"height":20,"body_name":name}));
}

fn reference(basis: Basis) -> Settings {
    Settings {
        reference_name: "Illustrative purchase".into(),
        reference_price: 10.0,
        reference_length_mm: 100.0,
        reference_width_mm: 50.0,
        reference_thickness_mm: 20.0,
        waste_percent: 25.0,
        basis,
    }
}

#[test]
fn default_reference_converts_the_original_purchase_to_a_volume_price() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length":1400,"width":750,"height":40}));
    let result = report(&mut s, None);
    close(result.reference_price_per_m3, 180.0 / 0.042);
    close(result.total.net_volume_m3, 0.042);
    close(result.total.estimated_volume_m3, 0.042 * 1.15);
    close(result.total.cost, 207.0);
    assert_eq!(result.total.body_count, 1);
    assert_eq!(result.settings.basis, Basis::BoundingBox);
}

#[test]
fn analytic_cut_distinguishes_stock_from_net_and_applies_waste_once() {
    let mut s = Session::default();
    panel(&mut s, "Panel");
    run(&mut s, "solid.box", json!({"length":20,"width":10,"height":20,"corner":[40,20,0],"operation":"cut","targets":["Panel"]}));
    let stock = report(&mut s, Some(reference(Basis::BoundingBox)));
    let net = report(&mut s, Some(reference(Basis::NetVolume)));
    assert_eq!(stock.bodies.len(), 1);
    assert_eq!(stock.bodies[0].quantity, 1);
    assert_eq!(stock.bodies[0].dimensions_mm, [100.0, 50.0, 20.0]);
    close(stock.total.net_volume_m3, 0.000096);
    close(stock.total.stock_volume_m3, 0.0001);
    close(stock.total.estimated_volume_m3, 0.000125);
    close(stock.total.cost, 12.5);
    close(net.total.estimated_volume_m3, 0.00012);
    close(net.total.cost, 12.0);
}

#[test]
fn repeated_rotated_components_keep_local_stock_dimensions_and_source_materials() {
    let mut s = Session::default();
    let component = run(&mut s, "component.create", json!({"name":"Shelf"}));
    panel(&mut s, "Oak panel");
    run(&mut s, "material.assign", json!({"bodies":["Oak panel"],"material":"Oak"}));
    run(&mut s, "occurrence.move", json!({"occurrence":component["occurrence"],"axis":[0,0,1],"angle":"45 deg"}));
    run(&mut s, "occurrence.copy", json!({"component":component["component"],"translate":[200,0,0]}));
    run(&mut s, "component.activate", json!({"component":"root"}));
    panel(&mut s, "Steel panel");
    run(&mut s, "material.assign", json!({"bodies":["Steel panel"],"material":"Steel"}));
    let result = report(&mut s, Some(reference(Basis::BoundingBox)));
    let oak = result.bodies.iter().find(|row| row.name == "Oak panel").unwrap();
    let steel = result.bodies.iter().find(|row| row.name == "Steel panel").unwrap();
    assert_eq!(oak.material, "Oak");
    assert_eq!(steel.material, "Steel");
    assert_eq!(oak.quantity, 2);
    assert_eq!(steel.quantity, 1);
    assert_eq!(oak.dimensions_mm, [100.0, 50.0, 20.0]);
    close(oak.stock_volume_m3, 0.0002);
    close(oak.cost, 25.0);
    close(result.total.cost, 37.5);
    assert_eq!(result.total.body_count, 3);
}

#[test]
fn nested_repeated_components_count_every_placement() {
    let mut s = Session::default();
    let parent = run(&mut s, "component.create", json!({"name":"Pair"}));
    run(&mut s, "component.create", json!({"name":"Child"}));
    panel(&mut s, "Panel");
    run(&mut s, "occurrence.copy", json!({"component":parent["component"],"translate":[500,0,0]}));
    let result = report(&mut s, Some(reference(Basis::BoundingBox)));
    assert_eq!(result.bodies[0].quantity, 2);
    assert_eq!(result.total.body_count, 2);
    close(result.total.cost, 25.0);
}

#[test]
fn reference_edits_are_undoable_noops_are_free_and_unrelated_metadata_roundtrips() {
    let mut s = Session::default();
    panel(&mut s, "Panel");
    let foreign = json!({"nested":[1,null,"kept",{"value":true}]});
    run(&mut s, "document.metadata.set", json!({"key":"com.example.shop","value":foreign}));
    s.mark_saved();
    let count = s.undo.len();
    let expected = reference(Basis::NetVolume);
    let response = run(&mut s, SETTINGS, json!({"settings":expected}));
    assert_eq!(response["changed"], true);
    assert_eq!(settings(&s).unwrap(), expected);
    assert_eq!(s.undo.len(), count + 1);
    assert!(s.is_dirty());
    assert_eq!(run(&mut s, SETTINGS, json!({"settings":expected}))["changed"], false);
    assert_eq!(s.undo.len(), count + 1);
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc.metadata.get(METADATA_KEY).is_none());
    assert!(!s.is_dirty());
    run(&mut s, "edit.redo", json!({}));
    assert_eq!(settings(&s).unwrap(), expected);
    for bytes in [solidcraft_io::write_design(&s.doc), solidcraft_io::write_design_container(&s.doc).unwrap()] {
        let doc = solidcraft_io::read_design(&bytes).unwrap();
        assert_eq!(doc.metadata.get("com.example.shop"), Some(&foreign));
        let mut reopened = Session::new(doc);
        assert_eq!(settings(&reopened).unwrap(), expected);
        close(report(&mut reopened, None).total.cost, 12.5);
    }
}

#[test]
fn read_only_override_preserves_document_history_selection_and_revision() {
    let mut s = Session::default();
    panel(&mut s, "Panel");
    run(&mut s, "solid.box", json!({"length":1,"width":1,"height":1,"corner":[200,0,0]}));
    run(&mut s, "edit.undo", json!({}));
    s.mark_saved();
    let saved = s.doc.clone();
    let before = (s.revision, s.undo.len(), s.redo.len(), s.selection.clone(), s.model.is_stale());
    let mut free = reference(Basis::BoundingBox);
    free.reference_price = 0.0;
    close(report(&mut s, Some(free)).total.cost, 0.0);
    assert!(Arc::ptr_eq(&saved, &s.doc));
    assert_eq!(before, (s.revision, s.undo.len(), s.redo.len(), s.selection.clone(), s.model.is_stale()));
    assert!(!s.is_dirty());
    assert!(s.doc.metadata.get(METADATA_KEY).is_none());
    assert_eq!(report(&mut s, None).settings, Settings::default());
}

#[test]
fn invalid_inputs_are_atomic_and_do_not_poison_saved_reference() {
    let mut s = Session::default();
    panel(&mut s, "Panel");
    run(&mut s, SETTINGS, json!({"settings":reference(Basis::BoundingBox)}));
    let saved = s.doc.clone();
    let before = (s.revision, s.undo.len(), s.redo.len());
    let valid = serde_json::to_value(reference(Basis::BoundingBox)).unwrap();
    let mut invalid = Vec::new();
    for (key, values) in [
        ("reference_price", vec![json!(-1), json!(1e300), json!("180"), Value::Null]),
        ("reference_length_mm", vec![json!(0), json!(-1), json!(1e-300), json!(1e300)]),
        ("reference_width_mm", vec![json!(0)]),
        ("reference_thickness_mm", vec![json!(0)]),
        ("waste_percent", vec![json!(-1), json!(1001), json!("15")]),
        ("basis", vec![json!("other"), json!(0)]),
        ("reference_name", vec![json!(""), json!("\n"), json!("x".repeat(129))]),
        ("unknown", vec![json!(1)]),
    ] {
        for value in values {
            let mut settings = valid.clone();
            settings[key] = value;
            invalid.push(json!({"settings":settings}));
        }
    }
    invalid.extend([json!({"settings":null}), json!({"settings":{}}), json!({"settings":false}), json!({"other":1}), json!([])]);
    for params in invalid {
        for command in [ESTIMATE, SETTINGS] {
            assert!(s.execute(command, &params).is_err(), "{command}: {params}");
            assert!(Arc::ptr_eq(&saved, &s.doc));
            assert_eq!(before, (s.revision, s.undo.len(), s.redo.len()));
        }
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut settings = reference(Basis::BoundingBox);
        settings.reference_price = value;
        assert!(settings.validate(SETTINGS).is_err());
        settings = reference(Basis::BoundingBox);
        settings.waste_percent = value;
        assert!(settings.validate(SETTINGS).is_err());
        settings = reference(Basis::BoundingBox);
        settings.reference_length_mm = value;
        assert!(settings.validate(SETTINGS).is_err());
    }
}

#[test]
fn malformed_persisted_settings_are_refused_instead_of_defaulted() {
    let mut s = Session::default();
    panel(&mut s, "Panel");
    run(&mut s, "document.metadata.set", json!({"key":METADATA_KEY,"value":{"reference_price":"bad"}}));
    let saved = s.doc.clone();
    let error = s.execute(ESTIMATE, &json!({})).unwrap_err();
    assert!(error.to_string().contains("invalid type: string \"bad\", expected f64"), "{error}");
    assert!(Arc::ptr_eq(&saved, &s.doc));
    // A complete explicit reference can repair an invalid saved reference.
    run(&mut s, SETTINGS, json!({"settings":reference(Basis::BoundingBox)}));
    close(report(&mut s, None).total.cost, 12.5);
}

#[test]
fn numeric_output_overflow_is_an_error_without_a_partial_total() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length":1000000,"width":1000000,"height":1000000}));
    let mut reference = reference(Basis::BoundingBox);
    reference.reference_price = 1e12;
    reference.reference_length_mm = 100.0;
    reference.reference_width_mm = 100.0;
    reference.reference_thickness_mm = 100.0;
    let doc = s.doc.clone();
    let error = s.execute(ESTIMATE, &json!({"settings":reference})).unwrap_err();
    assert!(error.to_string().contains("estimated cost"), "{error}");
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert!(supported(f64::INFINITY, MAX_COST, "test").is_err());
}

#[test]
fn traversal_limits_refuse_exponentially_repeated_or_excessively_nested_assemblies() {
    for depth in [17, 65] {
        let mut s = Session::default();
        for _ in 0..depth {
            let parent = s.active_component;
            let component = s.doc_mut().add_component(None, parent).unwrap();
            if depth == 17 {
                s.doc_mut().add_occurrence(component, parent, solidcraft_doc::IDENTITY).unwrap();
            }
            s.active_component = component;
        }
        let doc = s.doc.clone();
        let error = s.execute(ESTIMATE, &json!({})).unwrap_err();
        assert!(error.to_string().contains("traversal limits"), "{error}");
        assert!(Arc::ptr_eq(&doc, &s.doc));
    }
}

#[test]
fn stale_failed_surface_and_mesh_models_are_explicit_errors() {
    let mut s = Session::default();
    panel(&mut s, "Panel");
    let before = s.revision;
    s.model.results.last_mut().unwrap().stale = true;
    assert!(s.execute(ESTIMATE, &json!({})).unwrap_err().to_string().contains("stale"));
    assert_eq!(s.revision, before, "an estimate never triggers evaluation");
    s.model.results.last_mut().unwrap().stale = false;
    s.model.results.last_mut().unwrap().error = Some("Synthetic failure".into());
    assert!(s.execute(ESTIMATE, &json!({})).unwrap_err().to_string().contains("failed features"));

    let mut s = Session::default();
    panel(&mut s, "Panel");
    run(&mut s, "sketch.create", json!({"plane":"XY","name":"Surface"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0":[200,0],"p1":[210,10]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "surface.patch", json!({"sketch":"Surface","body_name":"Open sheet"}));
    assert!(s.execute(ESTIMATE, &json!({})).unwrap_err().to_string().contains("Open sheet"));

    let mut s = Session::default();
    panel(&mut s, "Panel");
    let mesh =
        solidcraft_kernel::mesh_body(&[solidcraft_geom::Vec3::ZERO, solidcraft_geom::Vec3::X, solidcraft_geom::Vec3::Y], &[[0, 1, 2]]).unwrap();
    let state = Arc::make_mut(&mut s.model.results.last_mut().unwrap().state);
    state.bodies.push(solidcraft_doc::ModelBody::new("Mesh".into(), mesh, 0));
    assert!(s.execute(ESTIMATE, &json!({})).unwrap_err().to_string().contains("Mesh"));
}

#[test]
fn recorded_sheet_perforations_reduce_only_the_finished_volume() {
    let mut s = Session::default();
    run(&mut s, "sheet.manage_rules", json!({"name":"Steel 0.8","thickness":"0.8","active":true}));
    run(&mut s, "sketch.create", json!({"plane":"XY","name":"Blank"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[100,60]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "sheet.flange", json!({"sketch":"Blank"}));
    let plain = report(&mut s, Some(reference(Basis::NetVolume)));
    run(&mut s, "sheet.perforate", json!({"diameter":"2","pitch":"8","margin":"2","pattern":"square"}));
    let perforated = report(&mut s, Some(reference(Basis::NetVolume)));
    let area = s.model.state().sheets[0].perforated_area();
    assert!(area > 0.0);
    close(plain.total.net_volume_m3 - perforated.total.net_volume_m3, area * 0.8 / 1e9);
    close(plain.total.stock_volume_m3, perforated.total.stock_volume_m3);
    assert!(perforated.total.cost < plain.total.cost);
}

#[test]
fn empty_design_has_a_valid_zero_estimate() {
    let mut s = Session::default();
    let result = report(&mut s, None);
    assert!(result.bodies.is_empty());
    assert_eq!(result.total, Totals::default());
}
