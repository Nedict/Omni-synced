//! Sketch shell: tool feedback, catalogs, the empty cue, and the plane-to-extrude path.

use serde_json::json;
use solidcraft_engine::sketch::ConstraintKind;

use crate::scenario::Harness;
use crate::tools::Tool;

fn keys() -> Vec<&'static str> {
    crate::tools::catalog_keys().iter().copied().chain(crate::sketch_tools::prompt_keys().iter().copied()).collect()
}

/// Feature: ux-ui-shell-improvements, Property 1: Active-tool feedback is complete for every tool and step
#[test]
fn property_1_every_sketch_tool_step_has_feedback() {
    let h = Harness::new();
    let mut cases = 0usize;
    for spec in solidcraft_engine::command_specs() {
        let Some(mut tool) = Tool::for_command(spec.id) else { continue };
        if !spec.id.starts_with("sketch.") && !spec.id.starts_with("inspect.") && !spec.id.starts_with("canvas.") {
            continue;
        }
        for _ in 0..3 {
            let step = tool.step();
            let name = h.app.tr_label(spec.label);
            let expects = h.app.tr(step.prompt_key);
            let confirm = h.app.tr(step.confirm_key);
            let exit = h.app.tr(step.exit_key);
            assert!(!name.is_empty() && name != spec.id, "{}", spec.id);
            assert!(!expects.is_empty() && expects != step.prompt_key, "{}", spec.id);
            assert!(!confirm.is_empty() && confirm != step.confirm_key, "{}", spec.id);
            assert!(!exit.is_empty() && exit != step.exit_key, "{}", spec.id);
            tool.pts.push((solidcraft_engine::geom::Vec2::new(1.0, 1.0), None));
            tool.picks.push("e".into());
            cases += 1;
        }
    }
    assert!(cases >= 3, "property ran {cases} times");
}

/// Feature: ux-ui-shell-improvements, Property 2: Advancing a step changes the expected input
#[test]
fn property_2_advancing_a_step_changes_the_expected_input() {
    let h = Harness::new();
    let mut cases = 0usize;
    for id in ["sketch.line", "sketch.rectangle.two_point", "sketch.circle.center", "sketch.dimension", "sketch.slot.overall"] {
        let mut tool = Tool::for_command(id).expect(id);
        let first = tool.step();
        tool.pts.push((solidcraft_engine::geom::Vec2::ZERO, None));
        tool.picks.push("a".into());
        let next = tool.step();
        assert_ne!(first.prompt_key, next.prompt_key, "{id}");
        assert_ne!(h.app.tr(first.prompt_key), h.app.tr(next.prompt_key), "{id}");
        cases += 1;
    }
    assert!(cases >= 5);
}

/// Feature: ux-ui-shell-improvements, Property 3: Every prompt and label string resolves through the catalog
#[test]
fn property_3_prompt_keys_resolve_in_every_catalog() {
    let mut cases = 0usize;
    for id in keys() {
        let english = crate::i18n::Locale::for_language("en").text(id);
        assert_ne!(english, id, "{id}");
        assert!(!english.is_empty());
        for lang in ["ja", "de", "ko"] {
            let text = crate::i18n::Locale::for_language(lang).text(id);
            assert_ne!(text, id, "{lang} {id}");
            assert!(!text.is_empty(), "{lang} {id}");
        }
        cases += 1;
    }
    assert!(cases >= 40, "property ran {cases} times");
}

/// Feature: ux-ui-shell-improvements, Property 10: The empty-state cue is shown exactly when the sketch is empty and idle
#[test]
fn property_10_empty_cue_matches_an_idle_empty_sketch() {
    let mut h = Harness::new();
    assert!(!crate::sketch_display::empty_state_visible(&h.app));
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    assert!(crate::sketch_display::empty_state_visible(&h.app));
    h.app.start("sketch.line");
    assert!(!crate::sketch_display::empty_state_visible(&h.app));
    h.app.tool = None;
    assert!(crate::sketch_display::empty_state_visible(&h.app));
    h.app.run("sketch.line", json!({"points": [[0.0, 0.0], [10.0, 0.0]], "infer": false})).unwrap();
    assert!(!crate::sketch_display::empty_state_visible(&h.app));
}

#[test]
fn create_sketch_on_a_plane_arms_the_line_tool() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.step(&json!({"start": "sketch.create"})).unwrap();
    h.step(&json!({"click": {"plane": "XY"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.tool.as_ref().map(|tool| tool.cmd), Some("sketch.line"));
    assert!(h.app.session.active_sketch.is_some());
    h.frames(1);
    assert!(crate::tools::tool_feedback(&h.app).is_some());
    h.app.tool = None;
    assert!(crate::tools::tool_feedback(&h.app).is_none());
}

#[test]
fn a_horizontal_line_stores_a_horizontal_constraint_and_a_zero_click_does_not() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.start("sketch.line");
    h.step(&json!({"click": {"sketch": [0.0, 0.0]}})).unwrap();
    let curves = h.app.session.doc.sketch(h.app.session.active_sketch.unwrap()).unwrap().curves.len();
    h.step(&json!({"click": {"sketch": [0.0, 0.0]}})).unwrap();
    let sketch = h.app.session.doc.sketch(h.app.session.active_sketch.unwrap()).unwrap();
    assert_eq!(sketch.curves.len(), curves, "a repeated point adds no line");
    h.step(&json!({"click": {"sketch": [25.0, 0.0]}})).unwrap();
    let sketch = h.app.session.doc.sketch(h.app.session.active_sketch.unwrap()).unwrap();
    assert!(sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Horizontal { .. })), "horizontal snap becomes a constraint");
}

#[test]
fn closing_a_chain_shades_one_profile_and_finish_opens_extrude() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.start("sketch.line");
    for point in [[0.0, 0.0], [30.0, 0.0], [30.0, 20.0], [0.0, 0.0]] {
        h.step(&json!({"click": {"sketch": point}})).unwrap();
    }
    let sid = h.app.session.active_sketch.unwrap();
    let profiles = h.app.session.world_state().sketch(sid).map(|ss| ss.profiles.len()).unwrap_or(0);
    assert_eq!(profiles, 1, "the closed chain is one profile");
    h.app.start("sketch.finish");
    assert!(matches!(h.app.dialog.as_ref().map(|d| &d.kind), Some(crate::dialogs::Kind::Extrude { .. })));
    let chosen = h.app.dialog.as_ref().unwrap().items();
    assert!(chosen.iter().any(|s| matches!(s, solidcraft_engine::Sel::Profile { sketch, index: 0 } if *sketch == sid)));
    h.step(&json!({"key": "Escape"})).unwrap();
    assert!(h.app.dialog.is_none());
    assert!(h.app.session.doc.feature(sid).is_some(), "cancelling extrude keeps the sketch");
}

#[test]
fn two_profiles_do_not_open_extrude() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0": [0.0, 0.0], "p1": [10.0, 8.0]})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0": [20.0, 0.0], "p1": [30.0, 8.0]})).unwrap();
    h.app.start("sketch.finish");
    assert!(h.app.dialog.is_none());
    assert!(h.app.status.as_ref().is_some_and(|(msg, _, err)| !err && msg.contains("profile")));
}

#[test]
fn coaching_does_not_block_a_sketch_click() {
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.start("sketch.line");
    h.frames(2);
    h.step(&json!({"click": {"sketch": [12.0, 8.0]}})).unwrap();
    assert_eq!(h.app.tool.as_ref().map(|tool| tool.pts.len()), Some(1), "the click reached the line tool");
}

#[test]
fn editing_a_dimension_starts_from_its_value_and_cancel_keeps_the_design() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run("sketch.line", json!({"points": [[0.0, 0.0], [10.0, 0.0]], "infer": false})).unwrap();
    let made = h.app.run("sketch.dimension.commit", json!({"entities": ["l1"], "type": "length", "value": "25 mm"})).unwrap();
    let param = made["param"].as_str().unwrap().to_string();
    let expr = h.app.session.doc.param(&param).unwrap().expr.clone();
    assert!(expr.contains("25"), "{expr}");
    let before = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    crate::dim_view::edit_param(&h.app, &param);
    assert_eq!(crate::dim_view::editing_expr().as_deref(), Some(expr.as_str()));
    h.frames(2);
    h.step(&json!({"key": "Escape"})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before);
    assert_eq!(h.app.session.undo.len(), undo);
}

/// Feature: ux-ui-shell-improvements, Property 7: Editing an existing dimension prepopulates its current value
#[test]
fn property_7_editing_a_dimension_prepopulates_its_value() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run("sketch.line", json!({"points": [[0.0, 0.0], [10.0, 0.0]], "infer": false})).unwrap();
    let mut param = String::new();
    let mut cases = 0usize;
    for n in 1..=100 {
        let value = format!("{n} mm");
        let made = if param.is_empty() {
            h.app.run("sketch.dimension.commit", json!({"entities": ["l1"], "type": "length", "value": value})).unwrap()
        } else {
            h.app.run("sketch.dimension.commit", json!({"param": param, "value": value})).unwrap()
        };
        param = made["param"].as_str().expect("param").to_string();
        let stored = h.app.session.doc.param(&param).expect("stored").expr.clone();
        let measured = h.app.session.doc.eval(&stored, solidcraft_engine::doc::expr::Kind::Length).unwrap();
        crate::dim_view::edit_param(&h.app, &param);
        assert_eq!(crate::dim_view::editing_expr().as_deref(), Some(stored.as_str()));
        assert!((measured - f64::from(n)).abs() < 1e-3, "{stored} measured {measured}");
        crate::dim_view::cancel();
        cases += 1;
    }
    assert!(cases >= 100, "property ran {cases} times");
}

/// Feature: ux-ui-shell-improvements, Property 9: Cancelling a dimension entry changes nothing
#[test]
fn property_9_cancelling_a_dimension_entry_changes_nothing() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run("sketch.line", json!({"points": [[0.0, 0.0], [8.0, 0.0]], "infer": false})).unwrap();
    let made = h.app.run("sketch.dimension.commit", json!({"entities": ["l1"], "type": "length", "value": "12 mm"})).unwrap();
    let param = made["param"].as_str().unwrap().to_string();
    let mut cases = 0usize;
    for n in 0..100 {
        let before = h.app.session.doc.to_json();
        let undo = h.app.session.undo.len();
        crate::dim_view::edit_param(&h.app, &param);
        crate::dim_view::set_editing_expr(&format!("{} mm", 40 + n));
        crate::dim_view::cancel();
        assert!(crate::dim_view::editing_expr().is_none());
        assert_eq!(h.app.session.doc.to_json(), before);
        assert_eq!(h.app.session.undo.len(), undo);
        cases += 1;
    }
    assert!(cases >= 100, "property ran {cases} times");
}

#[test]
fn a_second_extrude_keeps_body1_and_names_body2() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.preferences.look_at_anim = false;
    h.app.run("sketch.create", json!({"plane": "XY", "name": "First"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0": [0.0, 0.0], "p1": [20.0, 20.0]})).unwrap();
    let first = h.app.session.active_sketch.expect("sketch");
    h.app.start("sketch.finish");
    h.frames(1);
    let operation = match h.app.dialog.as_ref().map(|d| &d.kind) {
        Some(crate::dialogs::Kind::Extrude { operation, .. }) => *operation,
        other => panic!("extrude dialog, got {other:?}"),
    };
    assert_eq!(operation, 0);
    assert_eq!(h.app.tr("dialog.operation.new"), "New body");
    h.frames(3);
    h.step(&json!({"widget": "OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "first extrude stayed open: {:?}", h.app.preview.error);
    assert!(crate::browser::sketch_visible(&h.app, first));
    assert!(solidcraft_engine::view::sketch_shown(&h.app.session, first));
    h.app.run("sketch.create", json!({"plane": "XY", "name": "Second"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0": [10.0, 0.0], "p1": [30.0, 20.0]})).unwrap();
    let second = h.app.session.active_sketch.expect("second sketch");
    h.app.start("sketch.finish");
    h.frames(3);
    h.step(&json!({"widget": "OK"})).unwrap();
    let names: Vec<String> = h.app.session.world_state().bodies.iter().map(|b| b.name.clone()).collect();
    assert_eq!(names, vec!["Body1".to_string(), "Body2".to_string()], "{names:?} status {:?}", h.app.status);
    assert!(h.app.session.selection.iter().any(|item| matches!(item, solidcraft_engine::Sel::Body { name } if name == "Body2")));
    assert!(h.app.status.as_ref().is_some_and(|(msg, _, err)| !err && msg.contains("Body2")));
    assert!(crate::browser::sketch_visible(&h.app, first));
    assert!(crate::browser::sketch_visible(&h.app, second));
    let mut volume = |name: &str| h.app.run("inspect.measure", json!({"bodies": [name]})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((volume("Body1") - 4000.0).abs() < 1.0, "Body1");
    assert!((volume("Body2") - 4000.0).abs() < 1.0, "Body2");
}

#[test]
fn join_or_cut_needs_a_chosen_body() {
    let mut h = Harness::new();
    h.app.coaching.enabled = false;
    h.app.session.coaching.enabled = false;
    h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Body1"})).unwrap();
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0": [0.0, 0.0], "p1": [4.0, 4.0]})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    h.app.start("solid.extrude");
    let dialog = h.app.dialog.as_mut().expect("extrude");
    match &mut dialog.kind {
        crate::dialogs::Kind::Extrude { operation, .. } => *operation = 1,
        _ => panic!("extrude"),
    }
    let err = crate::dialogs::apply_commands(&h.app, h.app.dialog.as_ref().expect("dialog")).expect_err("join needs a body");
    assert!(err.contains("body") || err.contains("ソリッド") || err.contains("Körper") || err.contains("솔리드"), "{err}");
    h.app.dialog.as_mut().expect("dialog").extra.insert("targets".into(), json!(["Body1"]));
    assert!(crate::dialogs::apply_commands(&h.app, h.app.dialog.as_ref().expect("dialog")).is_ok());
}
