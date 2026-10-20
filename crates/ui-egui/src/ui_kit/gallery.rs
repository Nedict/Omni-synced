//! Layout/paint reference for every element in both themes. No window or GPU required.
use super::*;
use crate::{scenario, theme::Tokens};
use egui::{Ui, vec2};

const ELEMENTS: &[&str] = &[
    "header",
    "row",
    "caption",
    "hint",
    "note",
    "section",
    "divider",
    "field",
    "focused_field",
    "hint_field",
    "text_field",
    "multiline_text_field",
    "searchable_dropdown",
    "text_emphasis",
    "value_pair_named",
    "dropdown",
    "selection_field",
    "selection_field_state",
    "segmented",
    "segmented_compact",
    "segmented_states",
    "segmented_icons",
    "checkbox",
    "checkbox_row",
    "checkbox_grid",
    "options",
    "footer",
    "footer_apply",
    "footer_status",
    "button_pair",
    "primary",
    "secondary",
    "small",
    "icon",
    "list_row",
    "value_pair",
    "fraction",
    "status_header",
    "tool_choices",
    "toggle_icon",
];

/// Each example publishes a named rect and at least one handle (except non-interactive dividers).
fn example(ui: &mut Ui, name: &str) -> (&'static str, Option<&'static str>) {
    let mut value = "12 mm".to_owned();
    let mut second = "25 mm".to_owned();
    let mut selected = 0;
    let mut checked = true;
    match name {
        "header" => {
            header(ui, "fillet", "Fillet", || {});
            ("dialog:header", Some("dialog:close"))
        }
        "row" => {
            row(ui, "Gallery", |ui| {
                field(ui, &mut value);
            });
            ("dialog:row:Gallery", Some("dialog:Gallery"))
        }
        "caption" => {
            caption(ui, "Gallery", "Helper text");
            ("dialog:caption:Gallery", None)
        }
        "hint" => {
            hint(ui, "gallery", "Pick a point or curve");
            ("dialog:hint:gallery", Some("dialog:hint:gallery"))
        }
        "note" => {
            note(ui, "gallery", "Counts include the original. Center balances copies; Flip chooses the extra side for even totals.");
            ("dialog:note:gallery", None)
        }
        "section" => {
            section(ui, "gallery", "Options", true, |_| {});
            ("dialog:section:Options", Some("dialog:section:Options"))
        }
        "divider" => {
            divider(ui, "gallery");
            ("dialog:divider:gallery", None)
        }
        "multiline_text_field" => {
            multiline_text_field(ui, "Gallery", egui::Id::new("gallery_text"), &mut value);
            ("dialog:widget:Gallery", Some("dialog:Gallery"))
        }
        "searchable_dropdown" => {
            row(ui, "Gallery", |ui| {
                searchable_dropdown(
                    ui,
                    "gallery_search",
                    "One",
                    &mut String::new(),
                    &[SearchOption { value: "one", label: "One", handle: None }],
                    None,
                    None,
                );
            });
            ("dialog:row:Gallery", Some("dialog:Gallery"))
        }
        "text_emphasis" => {
            row(ui, "Gallery", |ui| text_emphasis(ui, &mut checked, &mut false));
            ("dialog:row:Gallery", Some("dialog:Bold"))
        }
        "value_pair_named" => {
            value_pair_named(ui, "Position", [&mut value, &mut second], ["X", "Y"], ["Distance X", "Distance Y"]);
            ("dialog:row:Position", Some("dialog:Distance X"))
        }
        "field"
        | "focused_field"
        | "hint_field"
        | "text_field"
        | "fraction"
        | "dropdown"
        | "selection_field"
        | "selection_field_state"
        | "segmented"
        | "segmented_compact"
        | "segmented_states"
        | "segmented_icons"
        | "checkbox"
        | "primary"
        | "secondary"
        | "small" => {
            row(ui, "Gallery", |ui| match name {
                "field" => {
                    field(ui, &mut value);
                }
                "focused_field" => {
                    focused_field(ui, &mut value);
                }
                "hint_field" => {
                    hint_field(ui, &mut value, "Length");
                }
                "text_field" => {
                    text_field(ui, &mut value, "Name");
                }
                "fraction" => {
                    fraction_field(ui, &mut 0.5);
                }
                "dropdown" => {
                    dropdown(ui, "gallery", &mut selected, &["One", "Two"]);
                }
                "selection_field" => {
                    selection_field(ui, "Point 1", true);
                }
                "selection_field_state" => {
                    selection_field_state(ui, "Select", false, false);
                }
                "segmented" => {
                    segmented(ui, &mut selected, &["One", "Two"]);
                }
                "segmented_compact" => {
                    segmented_compact(ui, &mut selected, &["Left", "Center", "Right"]);
                }
                "segmented_states" => {
                    segmented_states(ui, &mut selected, &["One", "Two"], &[true, true]);
                }
                "segmented_icons" => {
                    segmented_icons(ui, &mut selected, &[IconSegment { key: "normal", icon: "normal", tip: "Normal".into() }], &[]);
                }
                "checkbox" => {
                    checkbox(ui, "Option", &mut checked);
                }
                "primary" => {
                    primary_button(ui, "Create");
                }
                "secondary" => {
                    secondary_button(ui, "Add");
                }
                "small" => {
                    small_button(ui, "×");
                }
                _ => {}
            });
            let handle = match name {
                "primary" => "dialog:button:Create",
                "secondary" => "dialog:button:Gallery:Add",
                "small" => "dialog:button:Gallery:×",
                "fraction" => "dialog:fraction:Gallery",
                _ => "dialog:Gallery",
            };
            ("dialog:row:Gallery", Some(handle))
        }
        "checkbox_row" => {
            checkbox_row(ui, "gallery", "Option", true);
            ("sketch_palette:toggle:gallery", Some("sketch_palette:toggle:gallery"))
        }
        "checkbox_grid" | "options" => {
            let mut second = false;
            let mut items = [("First", &mut checked), ("Second", &mut second)];
            if name == "options" {
                options(ui, "gallery", &mut items);
            } else {
                checkbox_grid(ui, &mut items, 2);
            }
            ("dialog:checkbox:First", Some("dialog:First"))
        }
        "footer" | "footer_apply" | "footer_status" => {
            if name == "footer_status" {
                footer_with_status(ui, "OK", "Cancel", true, None, Some("Computing…"));
            } else {
                footer(ui, "OK", "Cancel", true, (name == "footer_apply").then_some("Apply"));
            }
            ("dialog:footer", Some(if name == "footer_apply" { "dialog:apply" } else { "dialog:ok" }))
        }
        "button_pair" => {
            button_pair(
                ui,
                "sketch_palette",
                [false, true],
                [PairButton { key: "look_at", label: "ui.look_at", icon: Some("lookat") }, PairButton { key: "slice", label: "Slice", icon: None }],
            );
            ("sketch_palette:look_at", Some("sketch_palette:look_at"))
        }
        "icon" => {
            icon_button(ui, "Remove", "close");
            ("dialog:button:Remove", Some("dialog:button:Remove"))
        }
        "list_row" => {
            list_row(ui, "picked", "Point 1");
            ("dialog:list:picked", Some("dialog:list:picked:remove"))
        }
        "value_pair" => {
            value_pair(ui, "Position", [&mut value, &mut second], ["X", "Y"]);
            ("dialog:row:Position", Some("dialog:Y"))
        }
        "toggle_icon" => {
            toggle_icon(ui, "Alignment", "Center", &mut checked, "pattern_center");
            ("dialog:Alignment:Center", Some("dialog:Alignment:Center"))
        }
        "tool_choices" => {
            tool_choices(ui, "Line", "line", &[ToolChoice { key: "line", icon: "normal", label: "Line", enabled: true }]);
            ("sketch_palette:feature:line", Some("sketch_palette:feature:line"))
        }
        "status_header" => {
            ui.spacing_mut().item_spacing.y = 0.0;
            status_header(
                ui,
                "sketch_palette",
                "Sketch 1",
                StatusHeader { icon: "sketch", text: "2 degrees of freedom", complete: false, fraction: 0.5 },
            );
            ("sketch_palette:meter", None)
        }
        _ => panic!("missing gallery example {name}"),
    }
}

#[test]
fn every_element_has_geometry_and_stays_in_its_allocation_in_both_themes() {
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        for &name in ELEMENTS {
            let ctx = crate::fonts::test_context();
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            for _ in 0..3 {
                scenario::clear_handles();
                let mut allocation = egui::Rect::NOTHING;
                let mut published = ("", None);
                let mut output = ctx.run_ui(
                    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 600.0))), ..Default::default() },
                    |ui| {
                        ui.set_width(320.0);
                        allocation = ui
                            .scope(|ui| {
                                scope(ui, |ui| {
                                    published = example(ui, name);
                                })
                            })
                            .response
                            .rect;
                    },
                );
                output.textures_delta.clear();
                let rect = scenario::rect_of(published.0).unwrap_or_else(|| panic!("{name}: missing {}", published.0));
                assert!(rect.is_finite() && rect.is_positive(), "{name}: {rect:?}");
                assert!(allocation.expand(1.0).contains_rect(rect), "{name}: {rect:?} outside {allocation:?}");
                if let Some(handle) = published.1 {
                    assert!(
                        rect.contains(scenario::handle_at(handle).unwrap_or_else(|| panic!("{name}: missing {handle}"))),
                        "{name}: handle outside {rect:?}"
                    );
                }
                if published.0.starts_with("dialog:row:") {
                    assert_eq!(rect.height(), Tokens::get().control_height, "{name}");
                }
                for shape in output.shapes {
                    // Native icon-only buttons keep an empty text shape. It has no vertices;
                    // its sentinel bounds do not describe any painted pixels.
                    if matches!(&shape.shape, egui::Shape::Text(text) if text.galley.num_vertices == 0) {
                        continue;
                    }
                    let painted = shape.shape.visual_bounding_rect().intersect(shape.clip_rect);
                    if painted.is_positive() {
                        assert!(allocation.expand(1.0).contains_rect(painted), "{name}: paint {painted:?} outside {allocation:?}: {:?}", shape.shape);
                    }
                }
            }
        }
    }
    crate::theme::set_dark(previous);
}

#[test]
fn apply_does_not_submit_and_obeys_validation() {
    for enabled in [false, true] {
        let ctx = crate::fonts::test_context();
        let mut action = Action::None;
        let mut draw = |ui: &mut Ui| {
            action = footer(ui, "OK", "Cancel", enabled, Some("Apply"));
        };
        super::test_support::frame(&ctx, vec![], &mut draw);
        let at = scenario::handle_at("dialog:apply").unwrap();
        super::test_support::frame(&ctx, super::test_support::pointer(at, true), &mut draw);
        super::test_support::frame(&ctx, super::test_support::pointer(at, false), &mut draw);
        assert_eq!(action, if enabled { Action::Apply } else { Action::None });
    }
}

#[test]
fn picked_item_remove_and_paired_values_have_independent_targets() {
    let ctx = crate::fonts::test_context();
    let mut removed = false;
    let mut draw = |ui: &mut Ui| {
        removed = list_row(ui, "point-1", "Point 1").clicked();
    };
    super::test_support::frame(&ctx, vec![], &mut draw);
    let at = scenario::handle_at("dialog:list:point-1:remove").unwrap();
    super::test_support::frame(&ctx, super::test_support::pointer(at, true), &mut draw);
    super::test_support::frame(&ctx, super::test_support::pointer(at, false), &mut draw);
    assert!(removed);

    let mut x = String::new();
    let mut y = String::new();
    let mut draw = |ui: &mut Ui| {
        value_pair(ui, "Position", [&mut x, &mut y], ["X", "Y"]);
    };
    super::test_support::frame(&ctx, vec![], &mut draw);
    let row = scenario::rect_of("dialog:row:Position").unwrap();
    let first = scenario::handle_at("dialog:X").unwrap();
    let second = scenario::handle_at("dialog:Y").unwrap();
    assert_eq!(row.height(), Tokens::get().control_height);
    assert!(row.contains(first) && row.contains(second) && first.x < second.x);
    super::test_support::frame(&ctx, super::test_support::pointer(second, true), &mut draw);
    super::test_support::frame(&ctx, super::test_support::pointer(second, false), &mut draw);
    super::test_support::frame(&ctx, vec![egui::Event::Text("25".into())], &mut draw);
    assert_eq!(x, "");
    assert_eq!(y, "25");
}

#[test]
fn gallery_rows_share_a_label_column_after_measurement_in_both_themes() {
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        let ctx = crate::fonts::test_context();
        let mut value = "3".to_owned();
        for _ in 0..3 {
            super::test_support::frame(&ctx, vec![], |ui| {
                row(ui, "Edges", |ui| {
                    selection_field(ui, "Edge 1", true);
                });
                row(ui, "Counterbore depth", |ui| {
                    field(ui, &mut value);
                });
            });
        }
        let first = scenario::rect_of("dialog:control:Edges").unwrap();
        let second = scenario::rect_of("dialog:control:Counterbore depth").unwrap();
        assert_eq!(first.left(), second.left());
        assert_eq!(first.height(), Tokens::get().control_height);
        assert_eq!(second.height(), Tokens::get().control_height);
        let width = scenario::rect_of("dialog:label:Edges").unwrap().width();
        assert!(width >= Tokens::get().form_label_width && width <= Tokens::get().form_label_width * 1.6);
    }
    crate::theme::set_dark(previous);
}

#[test]
fn compound_row_actions_preserve_the_original_field_handle() {
    let ctx = crate::fonts::test_context();
    let mut value = "3 mm".to_owned();
    let mut expected = None;
    super::test_support::frame(&ctx, vec![], |ui| {
        row(ui, "Mid Radius", |ui| {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(Tokens::get().space_24 * 3.0, row_height()),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        field(ui, &mut value);
                    },
                );
                expected = scenario::handle_at("dialog:Mid Radius");
                small_button(ui, "×");
            });
        });
    });
    assert_eq!(scenario::handle_at("dialog:Mid Radius"), expected);
    assert!(expected.is_some());
    assert_ne!(scenario::handle_at("dialog:button:Mid Radius:×"), expected);
}

#[test]
fn emphasis_and_compact_modes_keep_independent_actions_in_both_themes() {
    for dark in [false, true] {
        let ctx = crate::fonts::test_context();
        ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut bold = false;
        let mut italic = false;
        let mut selected = 0;
        let mut draw = |ui: &mut Ui| {
            row(ui, "Style", |ui| text_emphasis(ui, &mut bold, &mut italic));
            row(ui, "Alignment", |ui| {
                segmented_compact(ui, &mut selected, &["Long translated left", "Long translated centre", "Long translated right"]);
            });
        };
        super::test_support::frame(&ctx, vec![], &mut draw);
        for handle in ["dialog:Bold", "dialog:Italic", "dialog:Alignment:Long translated centre"] {
            let at = scenario::handle_at(handle).unwrap();
            super::test_support::frame(&ctx, super::test_support::pointer(at, true), &mut draw);
            super::test_support::frame(&ctx, super::test_support::pointer(at, false), &mut draw);
        }
        assert!(bold && italic);
        assert_eq!(selected, 1);
    }
}
