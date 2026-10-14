use super::*;
use crate::{dialogs::Kind, scenario::Harness};
use solidcraft_engine::{Sel, Session};

#[test]
fn catalog_clicked_load_apply_export_and_ok_keep_one_design_undo_step() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1600,"height":1400}));
    let path = h.dir.join("source-library.json").to_string_lossy().into_owned();
    let mut source = Session::default();
    source.execute("solid.box", &json!({"length":10,"width":10,"height":10})).unwrap();
    source.execute("appearance.assign", &json!({"bodies":["Body1"],"appearance":"Warm oak colour","color":[180,120,70],"opacity":0.8})).unwrap();
    source.execute("library.export", &json!({"path":path})).unwrap();
    h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Body1"],"color":[20,20,20]})).unwrap();
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
    h.step(&json!({"widget":"Load library"})).unwrap();
    h.step(&json!({"click":{"handle":"library:entry:0"}})).unwrap();
    h.step(&json!({"widget":"Apply library appearance"})).unwrap();
    assert_eq!(h.app.session.doc.appearances.bodies["Body1"].name, "Warm oak colour");
    assert_eq!(h.app.session.undo.len(), undo + 1);
    let revision = h.app.session.revision;
    h.step(&json!({"widget":"Export design library..."})).unwrap();
    let exported = h.dir.join("my-library.solidcraft-library.json");
    let catalog = Catalog::read(&exported.to_string_lossy()).unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].definition.name(), "Warm oak colour");
    assert_eq!(h.app.session.revision, revision);
    h.step(&json!({"widget":"OK"})).unwrap();
    assert!(h.app.dialog.is_none());
    assert_eq!(h.app.session.undo.len(), undo + 1);
}

#[test]
fn open_library_loads_the_file_chosen_in_the_picker() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1600,"height":1400}));
    let path = h.dir.join("picked-library.json").to_string_lossy().into_owned();
    let mut source = Session::default();
    source.execute("solid.box", &json!({"length":10,"width":10,"height":10})).unwrap();
    source.execute("appearance.assign", &json!({"bodies":["Body1"],"appearance":"Warm oak colour","color":[180,120,70],"opacity":0.8})).unwrap();
    source.execute("library.export", &json!({"path":path})).unwrap();
    h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Body1".into() }];
    h.app.start("appearance.assign");
    h.frames(3);
    h.step(&json!({"choose_file":"picked-library.json"})).unwrap();
    h.step(&json!({"widget":"Open library..."})).unwrap();
    let Some(Kind::Appearance(state)) = h.app.dialog.as_ref().map(|d| &d.kind) else { panic!("the Appearance dialog closed") };
    assert_eq!(state.local.path, path);
    assert_eq!(state.local.catalog.as_ref().map(|c| c.entries.len()), Some(1));
    assert!(state.note.as_deref().is_some_and(|n| n.starts_with("Opened ")), "{:?}", state.note);
}
