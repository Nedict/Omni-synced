//! Manage cards keep their commands, dense chrome and public interaction handles.
use crate::{
    dialogs,
    scenario::{self, Harness},
    shell::LayoutMode,
    theme::Tokens,
};
use serde_json::json;
use solidcraft_engine::Sel;

fn fixture() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.call("ui.resize", json!({"width":1600,"height":1200}));
    h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
    h.app.run("parameters.add", json!({"name":"width","expression":"10 mm"})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Body1".into() }];
    h
}

fn chrome(h: &Harness) {
    for key in ["dialog:close", "dialog:ok", "dialog:cancel"] {
        assert!(scenario::handle_at(key).is_some(), "missing {key}");
    }
    assert_eq!(scenario::rect_of("dialog:header").unwrap().height(), Tokens::get().form_header_height);
    assert!(scenario::rect_of("dialog:hint:manage").is_some());
    let footer = scenario::rect_of("dialog:footer").unwrap();
    assert!(h.ctx.content_rect().contains_rect(footer));
}

fn form(command: &str, labels: &[&str]) {
    for layout in [LayoutMode::Docked, LayoutMode::Floating] {
        let mut h = fixture();
        h.call("ui.set", json!({"layout":layout}));
        h.app.start(command);
        h.frames(3);
        chrome(&h);
        for label in labels {
            assert!(scenario::rect_of(&format!("dialog:row:{label}")).is_some(), "{command}: {label}");
        }
        let before = dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
        h.frames(3);
        assert_eq!(before, dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap());
        h.step(&json!({"click":{"handle":"dialog:cancel"}})).unwrap();
        assert!(h.app.dialog.is_none());
    }
}

#[test]
fn physical_material_is_dense_and_keeps_assignment() {
    form("material.assign", &["Bodies", "Material"]);
    let mut h = fixture();
    h.app.start("material.assign");
    h.frames(3);
    let commands = dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(commands[0].0, "material.assign");
    assert_eq!(commands[0].1["bodies"], json!(["Body1"]));
    assert!(commands[0].1["material"].is_string());
}

#[test]
fn appearance_is_dense_and_preserves_color_opacity_and_targets() {
    form("appearance.assign", &["Objects", "Apply To", "Colour", "Opacity", "Library", "Search"]);
    let mut h = fixture();
    h.app.run("appearance.assign", json!({"bodies":["Body1"],"color":"#336699","opacity":0.123456})).unwrap();
    h.app.start("appearance.assign");
    let before = dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    h.frames(4);
    let after = dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(before, after);
    assert_eq!(after[0].1["opacity"], 0.123456);
}

#[test]
fn sheet_rule_manager_is_dense() {
    form("sheet.manage_rules", &["Rule", "Thickness", "K Factor", "Active rule"]);
}

#[test]
fn plastic_rule_manager_is_dense() {
    form("plastic.manage_rules", &["Rule", "Material", "Active rule"]);
}

#[test]
fn plastic_assignment_remains_dense() {
    form("plastic.assign_rule", &["Bodies", "Rule"]);
}

fn configurations(command: &str) {
    let mut h = fixture();
    h.app.run("config.configure", json!({"params":["width"]})).unwrap();
    h.app.start(command);
    h.frames(4);
    chrome(&h);
    for label in ["Configuration", "width", "Actions"] {
        assert!(scenario::rect_of(&format!("dialog:column:configuration_table:{label}")).is_some());
    }
    assert_eq!(scenario::rect_of("dialog:table:configuration_table:Default").unwrap().height(), Tokens::get().control_height);
    assert!(h.widget("Add Column").is_some());
    assert!(h.widget("Add Row").is_some());
    h.step(&json!({"widget":"Add Row"})).unwrap();
    assert_eq!(h.app.session.doc.configs.rows.len(), 2);
    h.step(&json!({"click":{"handle":"config:cell:Configuration2:param:width"}})).unwrap();
    h.step(&json!({"key":"A","cmd":true})).unwrap();
    h.step(&json!({"text":"25 mm"})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    assert!(h.app.dialog.is_some(), "cell Enter leaves the configuration table open");
    assert_eq!(h.app.session.doc.configs.rows.iter().find(|row| row.name == "Configuration2").unwrap().cells[0], "25 mm");
    assert_eq!(h.app.session.doc.param("width").unwrap().expr, "10 mm", "editing an inactive row does not activate it");
    h.step(&json!({"click":{"handle":"dialog:ok"}})).unwrap();
    assert!(h.app.dialog.is_none());
    assert_eq!(h.app.session.doc.configs.rows.len(), 2);
}

#[test]
fn configuration_table_is_dense_and_adds_rows() {
    configurations("config.table");
}

#[test]
fn configure_alias_is_dense_and_adds_rows() {
    configurations("config.configure");
}

#[test]
fn parameters_are_dense_and_keep_inline_edits_order_and_undo() {
    let mut h = fixture();
    h.app.run("parameters.add", json!({"name":"height","expression":"20 mm"})).unwrap();
    h.app.start("parameters.change");
    h.frames(4);
    chrome(&h);
    for label in ["Name", "Unit", "Expression", "Value", "Comment", "Actions"] {
        assert!(scenario::rect_of(&format!("dialog:column:user:{label}")).is_some());
    }
    h.step(&json!({"click":{"handle":"param:height"}})).unwrap();
    h.step(&json!({"key":"A","cmd":true})).unwrap();
    h.step(&json!({"text":"25 mm"})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.doc.param("height").unwrap().expr, "25 mm");
    assert!(h.app.params.is_some(), "cell Enter does not close the editor");
    h.step(&json!({"click":{"handle":"param:before:height"}})).unwrap();
    let names: Vec<_> = h.app.session.doc.params.iter().filter(|p| !p.model).map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["height", "width"]);
    h.app.run("edit.undo", json!({})).unwrap();
    let names: Vec<_> = h.app.session.doc.params.iter().filter(|p| !p.model).map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["width", "height"]);
    h.step(&json!({"click":{"handle":"params:close"}})).unwrap();
    assert!(h.app.params.is_none());
}

#[test]
fn material_review_is_dense_and_cancel_keeps_history() {
    let mut h = fixture();
    let before = (h.app.session.doc.clone(), h.app.session.undo.len());
    h.app.start("material.properties.validate");
    h.frames(3);
    chrome(&h);
    assert!(h.widget("Review card").is_some());
    h.step(&json!({"click":{"handle":"dialog:cancel"}})).unwrap();
    assert!(h.app.material_card_review.is_none());
    assert!(std::sync::Arc::ptr_eq(&before.0, &h.app.session.doc));
    assert_eq!(before.1, h.app.session.undo.len());
}

#[test]
fn wide_managers_leave_the_active_sketch_palette_visible() {
    for command in ["parameters.change", "config.table", "material.properties.validate"] {
        for width in [1000, 1600] {
            let mut h = fixture();
            h.call("ui.resize", json!({"width":width,"height":1000}));
            h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            h.frames(3);
            h.app.start(command);
            h.frames(4);
            let palette = h.app.viewport.palette_rect.unwrap();
            let header = scenario::rect_of("dialog:header").unwrap();
            assert!(header.right() <= palette.left(), "{command}, {width}: {header:?} overlaps {palette:?}");
            assert!(h.ctx.content_rect().contains_rect(header));
            assert!(scenario::handle_at("sketch_palette:status_header").is_some());
        }
    }
}

#[test]
fn appearance_card_keeps_the_standard_width_with_no_preselection() {
    let mut h = fixture();
    h.app.session.selection.clear();
    h.app.start("appearance.assign");
    h.frames(4);
    let header = scenario::rect_of("dialog:header").unwrap();
    let card = h.ctx.memory(|m| m.area_rect(egui::Id::new(("sc_dialog", "APPEARANCE")))).unwrap();
    assert!(card.width() <= Tokens::get().form_card_width + 2.0, "card {card:?}, header {header:?}");
}
