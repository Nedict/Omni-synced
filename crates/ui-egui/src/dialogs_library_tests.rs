use super::*;
use crate::{dialogs::Kind, scenario::Harness};
use solidcraft_engine::{Sel, Session};

#[test]
fn catalog_clicked_load_apply_export_and_ok_keep_one_design_undo_step() {
    clicked_catalog(None).unwrap();
}

#[test]
fn catalog_clicked_pbr_load_apply_export_and_ok_retains_the_optical_response() {
    clicked_catalog(Some(solidcraft_engine::doc::pbr_appearance::PbrParameters::new(1., 0.3, 1.5).unwrap())).unwrap();
}

fn clicked_catalog(pbr: Option<solidcraft_engine::doc::pbr_appearance::PbrParameters>) -> Result<(), Box<dyn std::error::Error>> {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1600,"height":1400}));
    let path = h.dir.join("source-library.json").to_string_lossy().into_owned();
    let mut source = Session::default();
    source.execute("solid.box", &json!({"length":10,"width":10,"height":10}))?;
    source.execute(
        "appearance.assign",
        &json!({"bodies":["Body1"],"appearance":"Warm oak colour","color":[180,120,70],"opacity":if pbr.is_some(){1.0}else{0.8},"pbr":pbr}),
    )?;
    source.execute("library.export", &json!({"path":path}))?;
    h.app.run("solid.box", json!({"length":10,"width":10,"height":10}))?;
    h.app.run("appearance.assign", json!({"bodies":["Body1"],"color":[20,20,20]}))?;
    h.app.session.mark_saved();
    h.app.session.selection = vec![Sel::Body { name: "Body1".into() }];
    h.app.start("appearance.assign");
    if let Some(dialog) = &mut h.app.dialog
        && let Kind::Appearance(state) = &mut dialog.kind
    {
        state.local.path = path;
    }
    h.frames(3);
    let undo = h.app.session.undo.len();
    h.step(&json!({"widget":"Load library"}))?;
    h.step(&json!({"click":{"handle":"library:entry:0"}}))?;
    h.step(&json!({"widget":"Apply library appearance"}))?;
    assert_eq!(h.app.session.doc.appearances.bodies["Body1"].name, "Warm oak colour");
    assert_eq!(h.app.session.doc.appearances.bodies["Body1"].pbr, pbr);
    if let Some(dialog) = &h.app.dialog
        && let Kind::Appearance(state) = &dialog.kind
    {
        assert_eq!(state.pbr, pbr);
    }
    assert_eq!(h.app.session.undo.len(), undo + 1);
    let revision = h.app.session.revision;
    h.step(&json!({"widget":"Export design library..."}))?;
    let exported = h.dir.join("my-library.solidcraft-library.json");
    let catalog = Catalog::read(&exported.to_string_lossy())?;
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].definition.name(), "Warm oak colour");
    if let Definition::Appearance(look) = &catalog.entries[0].definition {
        assert_eq!(look.pbr, pbr);
    }
    assert_eq!(h.app.session.revision, revision);
    h.step(&json!({"widget":"OK"}))?;
    assert!(h.app.dialog.is_none());
    assert_eq!(h.app.session.undo.len(), undo + 1);
    Ok(())
}
