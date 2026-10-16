use std::sync::Arc;

use serde_json::json;
use solidcraft_engine::cmd::material_estimate::METADATA_KEY;

use super::*;
use crate::scenario::Harness;

fn board() -> Harness {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":1400,"width":750,"height":40,"body_name":"Example board"})).unwrap();
    h
}

fn type_price(h: &mut Harness, text: &str) {
    h.step(&json!({"click":{"handle":"estimate:price"},"double":true})).unwrap();
    h.step(&json!({"key":"A","cmd":true})).unwrap();
    h.step(&json!({"text":text})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    h.frames(2);
}

#[test]
fn native_price_edit_updates_cost_and_close_does_not_save_the_draft() {
    let mut h = board();
    let doc = h.app.session.doc.clone();
    let undo = h.app.session.undo.len();
    let revision = h.app.session.revision;
    h.step(&json!({"start":"material.estimate"})).unwrap();
    let before = h.app.material_estimate.as_ref().unwrap().report.as_ref().unwrap().total.cost;
    type_price(&mut h, "360");
    let window = h.app.material_estimate.as_ref().unwrap();
    assert_eq!(window.settings.reference_price, 360.0);
    assert!((window.report.as_ref().unwrap().total.cost - before * 2.0).abs() < 1e-6);
    h.step(&json!({"widget":"Close"})).unwrap();
    assert!(h.app.material_estimate.is_none());
    assert!(Arc::ptr_eq(&doc, &h.app.session.doc));
    assert_eq!(h.app.session.undo.len(), undo);
    assert_eq!(h.app.session.revision, revision);
    assert!(h.app.session.doc.metadata.get(METADATA_KEY).is_none());
}

#[test]
fn saved_reference_is_one_undo_step_and_reopens_with_the_saved_price() {
    let mut h = board();
    h.app.run("document.metadata.set", json!({"key":"com.example.untouched","value":{"id":7}})).unwrap();
    let undo = h.app.session.undo.len();
    h.step(&json!({"start":"material.estimate"})).unwrap();
    type_price(&mut h, "250");
    h.step(&json!({"widget":"Save reference"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    assert_eq!(h.app.session.doc.metadata.get(METADATA_KEY).unwrap()["reference_price"], 250.0);
    assert_eq!(h.app.session.doc.metadata.get("com.example.untouched").unwrap(), &json!({"id":7}));
    h.step(&json!({"widget":"Close"})).unwrap();
    h.step(&json!({"start":"material.estimate"})).unwrap();
    assert_eq!(h.app.material_estimate.as_ref().unwrap().settings.reference_price, 250.0);
    h.app.run("edit.undo", json!({})).unwrap();
    h.frames(2);
    assert_eq!(h.app.material_estimate.as_ref().unwrap().settings, Settings::default());
    assert!(h.app.session.doc.metadata.get(METADATA_KEY).is_none());
}

#[test]
fn model_edits_refresh_quantities_and_invalid_reference_clears_the_old_total() {
    let mut h = board();
    h.step(&json!({"start":"material.estimate"})).unwrap();
    assert_eq!(h.app.material_estimate.as_ref().unwrap().report.as_ref().unwrap().total.body_count, 1);
    h.app.run("solid.box", json!({"length":100,"width":100,"height":10,"corner":[2000,0,0],"operation":"new_body"})).unwrap();
    h.frames(2);
    assert_eq!(h.app.material_estimate.as_ref().unwrap().report.as_ref().unwrap().total.body_count, 2);
    let mut window = h.app.material_estimate.take().unwrap();
    window.settings.reference_length_mm = 0.0;
    window.refresh(&mut h.app);
    assert!(window.report.is_none());
    assert!(window.error.is_some());
}

#[test]
fn programmatic_estimate_never_opens_a_window_and_switching_designs_closes_it() {
    let mut h = board();
    h.app.run("material.estimate", json!({})).unwrap();
    assert!(h.app.material_estimate.is_none());
    h.step(&json!({"start":"material.estimate"})).unwrap();
    crate::documents::open_tab(&mut h.app, solidcraft_engine::Session::default()).unwrap();
    assert!(h.app.material_estimate.is_none());
    h.step(&json!({"start":"material.estimate"})).unwrap();
    h.app.run("file.new", json!({"name":"Other design"})).unwrap();
    assert!(h.app.material_estimate.is_none());
}

#[test]
fn french_estimator_shows_localized_controls() {
    let mut h = board();
    h.app.preferences.language = "fr".into();
    h.step(&json!({"start":"material.estimate"})).unwrap();
    assert!(h.widget("Estimer les matériaux").is_some());
    assert!(h.widget("Enregistrer la référence").is_some());
    assert!(h.widget("Prix du plateau de référence (€)").is_some());
}
