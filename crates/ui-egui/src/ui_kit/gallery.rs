//! Table-driven layout/paint references in both themes. No window or GPU required.
use super::*;
use crate::{scenario, theme::Tokens};
use egui::{Ui, vec2};

struct Sample {
    value: String,
    second: String,
    selected: usize,
    checked: bool,
}

struct Element {
    name: &'static str,
    rect: &'static str,
    handle: Option<&'static str>,
    draw: fn(&mut Ui, &mut Sample),
}

macro_rules! element {
    ($name:literal, $rect:literal, $handle:expr, $draw:expr) => {
        Element { name: $name, rect: $rect, handle: $handle, draw: $draw }
    };
    ($name:literal, $draw:expr) => {
        element!($name, "gallery:allocation", None, $draw)
    };
}

// One entry per element/variant: names, rendering and geometry expectations stay together.
const ELEMENTS: &[Element] = &[
    element!("scope", |ui, _| {
        scope(ui, |ui| {
            ui.allocate_space(vec2(row_height(), row_height()));
        });
    }),
    element!("row_height", |ui, _| {
        ui.allocate_space(vec2(row_height(), row_height()));
    }),
    element!("header_height", |ui, _| {
        ui.allocate_space(vec2(header_height(), header_height()));
    }),
    element!("timeline_slider", |ui, _| {
        timeline_slider(ui, &mut 12.0, 0.0..=100.0, true);
    }),
    element!("placement_field", |ui, s| {
        placement_field(ui, ui.id().with("placement"), &mut s.value, "Distance");
    }),
    element!("bounded_cell_editor", |ui, s| {
        bounded_cell_editor(ui, ui.id().with("name"), &mut s.value, "Name", false, 128);
    }),
    element!("password_field", |ui, s| {
        password_field(ui, ui.id().with("secret"), &mut s.value, "API key");
    }),
    element!("number_field", |ui, _| {
        number_field(ui, &mut 3_u32, 1..=200, 1.0, "");
    }),
    element!("formatted_slider", |ui, _| {
        formatted_slider(ui, &mut 1.0, 0.5..=2.5, 0.05, |v| format!("{:.0} %", v * 100.0));
    }),
    element!("dropdown_menu", |ui, _| {
        dropdown_menu(ui, "gallery_menu", "Preset", |ui| {
            list_picker(ui, "preset", "Preset", true);
        });
    }),
    element!("message_field", |ui, s| {
        message_field(ui, ui.id().with("message"), &mut s.value, "Message");
    }),
    element!("text_lines", |ui, _| {
        text_lines(ui, "gallery", "Licence\nSecond line");
    }),
    element!("table", "dialog:table:gallery:row", None, |ui, s| {
        table(ui, "gallery", &[Column::new("Name", 1.0), Column::new("Expression", 1.5)], "No rows", |ui, table| {
            table.row(ui, "row", true, |row| {
                row.cell(|ui| {
                    text(ui, "Width", Text::Cell);
                });
                row.cell(|ui| {
                    cell_editor(ui, ui.id().with("value"), &mut s.value, "", false);
                });
            });
        });
    }),
    element!("list_picker", "dialog:list:gallery", Some("dialog:list:gallery"), |ui, _| {
        list_picker(ui, "gallery", "Preset", true);
    }),
    element!("inline_text", "dialog:inline:Active", Some("dialog:inline:Active"), |ui, _| {
        text(ui, "Active", Text::Inline);
    }),
    element!("cell_text", "dialog:cell:Width", Some("dialog:cell:Width"), |ui, _| {
        text(ui, "Width", Text::Cell);
    }),
    element!("label_actions", |ui, _| {
        label_actions(ui, "Configuration", |ui| {
            button(ui, "Apply", Button::Normal);
        });
    }),
    element!("text", "dialog:report:gallery", Some("dialog:report:gallery"), |ui, _| {
        text(ui, "Validation report with retained units: 10 mm", Text::Report("gallery", false));
    }),
    element!("search_field", |ui, s| {
        search_field(ui, ui.id().with("search"), &mut s.value, "Search");
    }),
    element!("cell_editor", |ui, s| {
        cell_editor(ui, ui.id().with("cell"), &mut s.value, "Expression", true);
    }),
    element!("color_field", |ui, _| {
        color_field(ui, &mut [120, 150, 180]);
    }),
    element!("slider", |ui, _| {
        slider(ui, &mut 0.5, 0.0..=1.0);
    }),
    element!("scalar_field", |ui, _| {
        scalar_field(ui, &mut 0.5, 0.0..=1.0);
    }),
    element!("length_field", |ui, _| {
        length_field(ui, &mut 12.7, "in", 2);
    }),
    element!("angle_field", |ui, _| {
        angle_field(ui, &mut 45.0);
    }),
    element!("text_area", |ui, s| {
        text_area(ui, "Gallery", ui.id().with("area"), &mut s.value, 5, 1024);
    }),
    element!("color_swatch", |ui, _| {
        color_swatch(ui, ui.id().with("swatch"), Tokens::get().accent, true, "Sample", 1);
    }),
    element!("swatch_grid", |ui, _| {
        swatch_grid(ui, &[1, 2, 3], |ui, n| {
            color_swatch(ui, ui.id().with(n), Tokens::get().accent, true, "Sample", *n);
        });
    }),
    element!("action_menu", |ui, _| {
        action_menu(ui, "Actions", |ui| {
            button(ui, "Add", Button::Normal);
        });
    }),
    element!("enabled_button", |ui, _| {
        button(ui, "Add", Button::Enabled(true));
    }),
    element!("header", "dialog:header", Some("dialog:close"), |ui, _| {
        header(ui, "fillet", "Fillet", || {});
    }),
    element!("collapsible_header", "dialog:header", Some("dialog:collapse"), |ui, _| {
        collapsible_header(ui, "fillet", "Fillet", &mut false, || {});
    }),
    element!("row", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            field(ui, &mut s.value);
        });
    }),
    element!("expr_row", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        expr_row(ui, "Gallery", &mut s.value);
    }),
    element!("hint_row", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        hint_row(ui, "Gallery", &mut s.value, "Value");
    }),
    element!("text_row", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        text_row(ui, "Gallery", &mut s.value, "Name");
    }),
    element!("choice_row", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        choice_row(ui, "Gallery", "gallery", &mut s.selected, &["One", "Two"]);
    }),
    element!("multiline_text_field", "dialog:widget:Gallery", Some("dialog:Gallery"), |ui, s| {
        multiline_text_field(ui, "Gallery", ui.id().with("text"), &mut s.value);
    }),
    element!("searchable_dropdown", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, _| {
        row(ui, "Gallery", |ui| {
            searchable_dropdown(ui, "gallery", "One", &mut String::new(), &[SearchOption { value: "one", label: "One", handle: None }], None, None);
        });
    }),
    element!("text_emphasis", "dialog:row:Gallery", Some("dialog:Bold"), |ui, s| {
        row(ui, "Gallery", |ui| {
            text_emphasis(ui, &mut s.checked, &mut false);
        });
    }),
    element!("value_pair_named", "dialog:row:Position", Some("dialog:Distance X"), |ui, s| {
        value_pair(ui, "Position", [&mut s.value, &mut s.second], ["X", "Y"], Some(["Distance X", "Distance Y"]));
    }),
    element!("field", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            field(ui, &mut s.value);
        });
    }),
    element!("focused_field", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            focused_field(ui, &mut s.value);
        });
    }),
    element!("hint_field", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            hint_field(ui, &mut s.value, "Length");
        });
    }),
    element!("text_field", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            text_field(ui, &mut s.value, "Name");
        });
    }),
    element!("focused_text_field", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            focused_text_field(ui, &mut s.value, "Name");
        });
    }),
    element!("fraction_field", "dialog:row:Gallery", Some("dialog:fraction:Gallery"), |ui, _| {
        row(ui, "Gallery", |ui| {
            fraction_field(ui, &mut 0.5);
        });
    }),
    element!("dropdown", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            dropdown(ui, "gallery", &mut s.selected, &["One", "Two"]);
        });
    }),
    element!("selection_field", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, _| {
        row(ui, "Gallery", |ui| {
            selection_field(ui, "Point 1", Selection { active: true, filled: true });
        });
    }),
    element!("selection_field_empty", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, _| {
        row(ui, "Gallery", |ui| {
            selection_field(ui, "Select", Selection { active: false, filled: false });
        });
    }),
    element!("segmented", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            segmented(ui, &mut s.selected, &["One", "Two"], Segments::default());
        });
    }),
    element!("segmented_compact", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            segmented(ui, &mut s.selected, &["Left", "Center", "Right"], Segments { compact: true, ..Default::default() });
        });
    }),
    element!("segmented_states", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            segmented(ui, &mut s.selected, &["One", "Two"], Segments { states: &[true, true], ..Default::default() });
        });
    }),
    element!("segmented_icons", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            segmented_icons(ui, &mut s.selected, &[IconSegment { key: "normal", icon: "normal", tip: "Normal".into() }], &[]);
        });
    }),
    element!("checkbox", "dialog:row:Gallery", Some("dialog:Gallery"), |ui, s| {
        row(ui, "Gallery", |ui| {
            checkbox(ui, "Option", &mut s.checked);
        });
    }),
    element!("primary_button", "dialog:row:Gallery", Some("dialog:button:Create"), |ui, _| {
        row(ui, "Gallery", |ui| {
            primary_button(ui, "Create");
        });
    }),
    element!("button", "dialog:row:Gallery", Some("dialog:button:Gallery:Add"), |ui, _| {
        row(ui, "Gallery", |ui| {
            button(ui, "Add", Button::Normal);
        });
    }),
    element!("small_button", "dialog:row:Gallery", Some("dialog:button:Gallery:×"), |ui, _| {
        row(ui, "Gallery", |ui| {
            button(ui, "×", Button::Small);
        });
    }),
    element!("checkbox_row", "sketch_palette:toggle:gallery", Some("sketch_palette:toggle:gallery"), |ui, _| {
        checkbox_row(ui, "gallery", "Option", true);
    }),
    element!("checkbox_grid", "dialog:checkbox:First", Some("dialog:First"), |ui, s| {
        checkbox_grid(ui, &mut [("First", &mut s.checked), ("Second", &mut false)], 2);
    }),
    element!("options", "dialog:checkbox:First", Some("dialog:First"), |ui, s| {
        options(ui, "gallery", &mut [("First", &mut s.checked), ("Second", &mut false)]);
    }),
    element!("footer", "dialog:footer", Some("dialog:ok"), |ui, _| {
        footer(ui, "OK", "Cancel", true, Footer::default());
    }),
    element!("footer_apply", "dialog:footer", Some("dialog:apply"), |ui, _| {
        footer(ui, "OK", "Cancel", true, Footer { apply: Some("Apply"), ..Default::default() });
    }),
    element!("footer_status", "dialog:footer", Some("dialog:ok"), |ui, _| {
        footer(ui, "OK", "Cancel", true, Footer { status: Some("Computing…"), ..Default::default() });
    }),
    element!("button_pair", "sketch_palette:look_at", Some("sketch_palette:look_at"), |ui, _| {
        button_pair(
            ui,
            "sketch_palette",
            [false, true],
            [PairButton { key: "look_at", label: "ui.look_at", icon: Some("lookat") }, PairButton { key: "slice", label: "Slice", icon: None }],
        );
    }),
    element!("icon_button_at", |ui, _| {
        let (rect, _) = ui.allocate_exact_size(vec2(row_height(), row_height()), egui::Sense::hover());
        icon_button_at(ui, rect, "close", "Close");
    }),
    element!("keyboard", |ui, _| {
        assert_eq!(keyboard(ui.ctx(), true), Action::None);
        ui.allocate_space(vec2(row_height(), row_height()));
    }),
    element!("list_row", "dialog:list:picked", Some("dialog:list:picked:remove"), |ui, _| {
        list_row(ui, "picked", "Point 1");
    }),
    element!("value_pair", "dialog:row:Position", Some("dialog:Y"), |ui, s| {
        value_pair(ui, "Position", [&mut s.value, &mut s.second], ["X", "Y"], None);
    }),
    element!("toggle_icon", "dialog:Alignment:Center", Some("dialog:Alignment:Center"), |ui, s| {
        toggle_icon(ui, "Alignment", "Center", &mut s.checked, "pattern_center");
    }),
    element!("tool_choices", "sketch_palette:feature:line", Some("sketch_palette:feature:line"), |ui, _| {
        tool_choices(ui, "Line", "line", &[ToolChoice { key: "line", icon: "normal", label: "Line", enabled: true }]);
    }),
    element!("status_header", "sketch_palette:meter", None, |ui, _| {
        ui.spacing_mut().item_spacing.y = 0.0;
        status_header(
            ui,
            "sketch_palette",
            "Sketch 1",
            StatusHeader { icon: "sketch", text: "2 degrees of freedom", complete: false, fraction: 0.5 },
        );
    }),
    element!("progress", "dialog:progress:gallery", None, |ui, _| {
        progress(ui, "gallery", 0.4);
    }),
    element!("hint", "dialog:hint:gallery", Some("dialog:hint:gallery"), |ui, _| {
        text(ui, "Pick a point", Text::Hint("gallery"));
    }),
    element!("note", "dialog:note:gallery", None, |ui, _| {
        text(ui, "Counts include the original. Center balances copies; Flip chooses the extra side for even totals.", Text::Note("gallery"));
    }),
    element!("caption", "dialog:caption:Gallery", None, |ui, _| {
        caption(ui, "Gallery", "Construction");
    }),
    element!("section", "dialog:section:Advanced", Some("dialog:section:Advanced"), |ui, _| {
        section(ui, "gallery", "Advanced", false, |_| {});
    }),
    element!("divider", "dialog:divider:gallery", None, |ui, _| {
        divider(ui, "gallery");
    }),
    element!("popup_list", |ui, _| {
        let anchor = ui.allocate_space(vec2(100.0, row_height())).1;
        popup_list(
            ui.ctx(),
            ui.id().with("popup"),
            anchor,
            0,
            &[PopupOption { text: "Width", detail: "10 mm", color: Tokens::get().text }],
            "Choose",
        );
        // The popup is on a separate layer; reserve its painted allocation for the same checks.
        ui.allocate_space(vec2(ui.available_width(), row_height() * 5.0));
    }),
];

fn example(ui: &mut Ui, element: &Element) -> (&'static str, Option<&'static str>) {
    let mut sample = Sample { value: "12 mm".into(), second: "25 mm".into(), selected: 0, checked: true };
    let rect = ui.scope(|ui| (element.draw)(ui, &mut sample)).response.rect;
    scenario::publish_rect("gallery:allocation", rect);
    (element.rect, element.handle)
}

#[test]
fn every_element_has_geometry_and_stays_in_its_allocation_in_both_themes() {
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        for element in ELEMENTS {
            let name = element.name;
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
                                    published = example(ui, element);
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
            action = footer(ui, "OK", "Cancel", enabled, Footer { apply: Some("Apply"), ..Default::default() });
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
        value_pair(ui, "Position", [&mut x, &mut y], ["X", "Y"], None);
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
                    selection_field(ui, "Edge 1", Selection { active: true, filled: true });
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
                button(ui, "×", Button::Small);
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
                segmented(
                    ui,
                    &mut selected,
                    &["Long translated left", "Long translated centre", "Long translated right"],
                    Segments { compact: true, ..Default::default() },
                );
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

#[test]
fn table_cells_keep_alignment_fixed_rows_editing_and_action_targets() {
    for dark in [false, true] {
        let ctx = crate::fonts::test_context();
        ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut first = "10 mm".to_owned();
        let mut second = "20 mm".to_owned();
        let mut removed = false;
        let mut draw = |ui: &mut Ui| {
            scope(ui, |ui| {
                table(ui, "cells", &[Column::new("Name", 1.0), Column::new("Expression", 1.5), Column::new("Actions", 0.7)], "Empty", |ui, table| {
                    for (name, value) in [("Width", &mut first), ("Height", &mut second)] {
                        table.row(ui, name, name == "Height", |row| {
                            row.cell(|ui| {
                                text(ui, name, Text::Cell);
                            });
                            row.cell(|ui| {
                                let response = cell_editor(ui, ui.id().with("expression"), value, "Expression", false);
                                scenario::publish_handle(&format!("gallery:cell:{name}"), response.rect.center());
                                scenario::publish_rect(&format!("gallery:cell:{name}"), response.rect);
                            });
                            row.cell(|ui| {
                                if button(ui, "Delete", Button::Small).clicked() {
                                    removed = true;
                                }
                            });
                        });
                    }
                });
            });
        };
        for _ in 0..3 {
            super::test_support::frame(&ctx, vec![], &mut draw);
        }
        let first_rect = scenario::rect_of("gallery:cell:Width").unwrap();
        let second_rect = scenario::rect_of("gallery:cell:Height").unwrap();
        let header = scenario::rect_of("dialog:column:cells:Expression").unwrap();
        assert_eq!(first_rect.left(), header.left());
        assert_eq!(second_rect.left(), header.left());
        assert_eq!(first_rect.height(), Tokens::get().control_height);
        assert_eq!(second_rect.height(), Tokens::get().control_height);
        let at = scenario::handle_at("gallery:cell:Height").unwrap();
        super::test_support::frame(&ctx, super::test_support::pointer(at, true), &mut draw);
        super::test_support::frame(&ctx, super::test_support::pointer(at, false), &mut draw);
        super::test_support::frame(&ctx, vec![egui::Event::Text("x".into())], &mut draw);
        let delete = scenario::handle_at("dialog:button:Actions:Delete").unwrap();
        super::test_support::frame(&ctx, super::test_support::pointer(delete, true), &mut draw);
        super::test_support::frame(&ctx, super::test_support::pointer(delete, false), &mut draw);
        assert_eq!(first, "10 mm");
        assert!(second.contains('x'));
        assert!(removed);
    }
}

#[test]
fn popup_suggestions_and_selectable_lists_keep_click_selection() {
    for dark in [false, true] {
        let ctx = crate::fonts::test_context();
        ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut selected = None;
        let mut draw = |ui: &mut Ui| {
            selected = popup_list(
                ui.ctx(),
                egui::Id::new("suggestions"),
                egui::Rect::from_min_size(egui::pos2(10.0, 10.0), vec2(220.0, 24.0)),
                0,
                &[
                    PopupOption { text: "width", detail: "Parameter · 10 mm", color: Tokens::get().text },
                    PopupOption { text: "height", detail: "Parameter · 20 mm", color: Tokens::get().text },
                ],
                "Choose",
            );
        };
        for _ in 0..3 {
            super::test_support::frame(&ctx, vec![], &mut draw);
        }
        let at = scenario::handle_at("dialog:suggestion:1").unwrap();
        super::test_support::frame(&ctx, super::test_support::pointer(at, true), &mut draw);
        super::test_support::frame(&ctx, super::test_support::pointer(at, false), &mut draw);
        assert_eq!(selected, Some(1));
        let mut picked = false;
        let mut draw = |ui: &mut Ui| {
            scope(ui, |ui| {
                if list_picker(ui, "preset", "Preset", picked).clicked() {
                    picked = !picked;
                }
            });
        };
        super::test_support::frame(&ctx, vec![], &mut draw);
        let at = scenario::handle_at("dialog:list:preset").unwrap();
        super::test_support::frame(&ctx, super::test_support::pointer(at, true), &mut draw);
        super::test_support::frame(&ctx, super::test_support::pointer(at, false), &mut draw);
        assert!(picked);
    }
}

#[test]
fn many_draggable_samples_wrap_inside_the_available_width() {
    let ctx = crate::fonts::test_context();
    let mut rects = Vec::new();
    let mut bounds = egui::Rect::NOTHING;
    super::test_support::frame(&ctx, vec![], |ui| {
        ui.set_width(Tokens::get().form_label_width * 2.0);
        scope(ui, |ui| {
            bounds = ui.available_rect_before_wrap();
            swatch_grid(ui, &(0..20).collect::<Vec<_>>(), |ui, index| {
                let response = color_swatch(ui, ui.id().with(index), Tokens::get().accent, false, "Sample", *index);
                rects.push(response.rect);
            });
        });
    });
    assert_eq!(rects.len(), 20);
    assert!(rects.iter().all(|rect| rect.right() <= bounds.right() && rect.width() == Tokens::get().control_height));
    assert!(rects.last().unwrap().top() > rects.first().unwrap().top(), "samples must wrap to additional rows");
}

#[test]
fn gallery_covers_every_inline_element() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui_kit");
    let names: std::collections::BTreeSet<_> = ELEMENTS.iter().map(|e| e.name).collect();
    assert_eq!(names.len(), ELEMENTS.len(), "duplicate gallery entries");
    for file in ["actions.rs", "choice.rs", "fields.rs", "layout.rs", "metrics.rs", "status.rs", "table.rs"] {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        let ast = syn::parse_file(&source).unwrap();
        for item in ast.items {
            if let syn::Item::Fn(item) = item
                && matches!(&item.vis, syn::Visibility::Restricted(v) if v.path.is_ident("crate"))
            {
                let name = item.sig.ident.to_string();
                // The inherited row label is metadata, not a renderable element.
                if name != "label" {
                    assert!(names.contains(name.as_str()), "{file}: {name} needs a gallery entry");
                }
            }
        }
    }
}
