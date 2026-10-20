use crate::ui_kit::test_support::*;
use crate::ui_kit::*;

use crate::{scenario, theme::Tokens};
use egui::Ui;

#[test]
fn checkbox_grid_and_close_header_are_clickable() {
    let ctx = crate::fonts::test_context();
    let (mut first, mut second, mut closed) = (false, true, false);
    let mut draw = |ui: &mut Ui| {
        header(ui, "extrude", "Extrude", || closed = true);
        checkbox_grid(ui, &mut [("First option", &mut first), ("Second option", &mut second)], 2);
    };
    frame(&ctx, vec![], &mut draw);
    let first_pos = scenario::handle_at("dialog:First option").unwrap();
    let second_pos = scenario::handle_at("dialog:Second option").unwrap();
    assert!((first_pos.y - second_pos.y).abs() < 0.01);
    assert!(second_pos.x > first_pos.x);
    frame(&ctx, pointer(first_pos, true), &mut draw);
    frame(&ctx, pointer(first_pos, false), &mut draw);
    let close = scenario::handle_at("dialog:close").unwrap();
    frame(&ctx, pointer(close, true), &mut draw);
    frame(&ctx, pointer(close, false), &mut draw);
    assert!(first && second && closed);
}

#[test]
fn heading_weight_field_surfaces_and_disclosures_follow_tokens_in_both_themes() {
    fn inspect(shape: &egui::Shape, t: Tokens, title: &mut usize, section: &mut usize, fields: &mut usize) {
        match shape {
            egui::Shape::Text(text) => {
                if text.galley.job.text == "3 mm" {
                    assert_eq!(text.galley.job.sections.first().unwrap().format.font_id.family, egui::FontFamily::Monospace);
                }
                if text.galley.job.text == "Fillet" {
                    assert_eq!(text.galley.job.sections.first().unwrap().format.font_id, crate::fonts::semibold(t.font_label));
                    *title += 1;
                }
                if text.galley.job.text == "OPTIONS" {
                    let format = &text.galley.job.sections.first().unwrap().format;
                    assert_eq!(format.font_id, crate::fonts::semibold(t.font_caption));
                    assert!(format.extra_letter_spacing > 0.0);
                    *section += 1;
                }
            }
            egui::Shape::Rect(rect) if rect.fill == t.control_fill && rect.stroke.width == 1.0 && rect.stroke.color == t.control_border => {
                *fields += 1
            }
            _ => {}
        }
    }
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        let ctx = crate::fonts::test_context();
        ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut value = "3 mm".to_string();
        let output = crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| {
            ui.set_width(300.0);
            scope(ui, |ui| {
                header(ui, "fillet", "Fillet", || {});
                row(ui, "Radius", |ui| {
                    field(ui, &mut value);
                });
                section(ui, "options", "Options", true, |_| {});
            });
        });
        let (mut title, mut section, mut fields) = (0, 0, 0);
        for shape in &output.shapes {
            crate::test_support::each_shape(&shape.shape, &mut |shape| inspect(shape, Tokens::get(), &mut title, &mut section, &mut fields));
        }
        assert_eq!(scenario::rect_of("dialog:header").unwrap().height(), Tokens::get().form_header_height);
        assert_eq!(title, 1, "the semibold heading is painted once");
        assert_eq!(section, 1, "semibold small caps are letter spaced and painted once");
        assert!(fields > 0, "the actual text editor has a token field fill and one pixel border");
    }
    crate::theme::set_dark(previous);
}

#[test]
fn panel_keeps_actions_and_body_geometry_in_both_layouts() {
    for layout in [crate::shell::LayoutMode::Docked, crate::shell::LayoutMode::Floating] {
        let ctx = crate::fonts::test_context();
        let vp = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
        for _ in 0..3 {
            scenario::clear_handles();
            crate::test_support::frame(&ctx, egui::RawInput { screen_rect: Some(vp), ..Default::default() }, |_| {
                let side = crate::frame::Side::Right;
                let frame = Panel::feature_frame(&ctx, layout, side);
                let anchor = egui::pos2(vp.right() - crate::shell::SHELL_GAP, vp.top());
                let panel = Panel::new("Fillet", egui::Id::new("gallery_feature"), anchor, vp, side, frame);
                panel.window().show(&ctx, |ui| {
                    panel.feature(
                        ui,
                        FeatureOptions {
                            icon: "fillet",
                            title: "Fillet",
                            drag_key: "gallery",
                            moved: egui::Vec2::ZERO,
                            docked: anchor,
                            apply_label: Some("Apply"),
                        },
                        false,
                        true,
                        |ui| {
                            let mut radius = "3 mm".to_owned();
                            row(ui, "Radius", |ui| {
                                field(ui, &mut radius);
                            });
                            ComputeStatus { text: None, busy: false, applying: false }
                        },
                    );
                });
            });
        }
        let header = scenario::rect_of("dialog:header").unwrap();
        let body = scenario::rect_of("dialog:row:Radius").unwrap();
        let footer = scenario::rect_of("dialog:footer").unwrap();
        assert_eq!(header.height(), Tokens::get().form_header_height);
        assert_eq!(body.height(), Tokens::get().control_height);
        assert_eq!(footer.height(), Tokens::get().form_footer_height);
        assert!(body.top() >= header.bottom());
        assert!(footer.top() >= body.bottom());
        assert!(vp.contains_rect(header) && vp.contains_rect(body) && vp.contains_rect(footer));
        assert!(footer.contains(scenario::handle_at("dialog:ok").unwrap()));
        assert!(footer.contains(scenario::handle_at("dialog:apply").unwrap()));
    }
}

#[test]
fn palette_scrolls_short_views_and_retains_its_status_and_action_handles() {
    for layout in [crate::shell::LayoutMode::Docked, crate::shell::LayoutMode::Floating] {
        for height in [125.0, 480.0] {
            let ctx = crate::fonts::test_context();
            let vp = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, height));
            for _ in 0..3 {
                scenario::clear_handles();
                crate::test_support::frame(&ctx, egui::RawInput { screen_rect: Some(vp), ..Default::default() }, |_| {
                    let frame = Panel::palette_frame(&ctx, layout);
                    let bottom = vp.bottom() - crate::shell::SHELL_GAP;
                    let panel = Panel::new(
                        "Options",
                        egui::Id::new("gallery_palette"),
                        egui::pos2(vp.right() - crate::shell::SHELL_GAP, 0.0),
                        vp,
                        crate::frame::Side::Right,
                        frame,
                    );
                    panel.window().show(&ctx, |ui| {
                        panel.palette(ui, bottom, 0.0, |ui, part| match part {
                            Part::Header => {
                                status_header(
                                    ui,
                                    "sketch_palette",
                                    "Sketch 1",
                                    StatusHeader { icon: "sketch", text: "Fully constrained", complete: true, fraction: 1.0 },
                                );
                            }
                            Part::Body => {
                                checkbox_row(ui, "snap", "Snap", true);
                            }
                            Part::Footer => {
                                button_pair(
                                    ui,
                                    "sketch_palette",
                                    [false, false],
                                    [
                                        PairButton { key: "look_at", label: "ui.look_at", icon: Some("lookat") },
                                        PairButton { key: "slice", label: "Slice", icon: None },
                                    ],
                                );
                            }
                        });
                    });
                });
            }
            let scroll = scenario::rect_of("sketch_palette:options").unwrap();
            assert!(scroll.is_finite() && scroll.is_positive());
            assert!(vp.contains_rect(scroll));
            assert!(scenario::handle_at("sketch_palette:toggle:snap").is_some());
            assert!(scenario::handle_at("sketch_palette:look_at").is_some());
            assert!(scenario::handle_at("sketch_palette:slice").is_some());
        }
    }
}

#[test]
fn wide_tables_remain_bounded_with_pinned_actions_on_small_viewports() {
    for layout in [crate::shell::LayoutMode::Floating, crate::shell::LayoutMode::Docked] {
        for width in [240.0, 640.0, 1200.0] {
            let ctx = crate::fonts::test_context();
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 360.0));
            for _ in 0..3 {
                scenario::clear_handles();
                crate::test_support::frame(&ctx, egui::RawInput { screen_rect: Some(viewport), ..Default::default() }, |_| {
                    let side = crate::frame::Side::Right;
                    let anchor = viewport.right_top();
                    let panel = Panel::new("Table", egui::Id::new("wide"), anchor, viewport, side, Panel::feature_frame(&ctx, layout, side)).wide();
                    panel.window().show(&ctx, |ui| {
                        panel.feature(
                            ui,
                            FeatureOptions {
                                icon: "fillet",
                                title: "Table",
                                drag_key: "wide",
                                moved: egui::Vec2::ZERO,
                                docked: anchor,
                                apply_label: None,
                            },
                            false,
                            true,
                            |ui| {
                                for _ in 0..30 {
                                    text(ui, "Table body", Text::Note("row"));
                                }
                                ComputeStatus { text: None, busy: false, applying: false }
                            },
                        );
                    });
                });
            }
            let header = scenario::rect_of("dialog:header").unwrap();
            let footer = scenario::rect_of("dialog:footer").unwrap();
            assert!(viewport.expand(1.0).contains_rect(header), "{width}: {header:?}");
            assert!(viewport.expand(1.0).contains_rect(footer), "{width}: {footer:?}");
        }
    }
}

#[test]
fn prompt_is_one_and_a_half_cards_wide_with_its_actions_in_both_themes() {
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        let ctx = crate::fonts::test_context();
        let t = Tokens::get();
        let mut action = Action::None;
        for _ in 0..3 {
            scenario::clear_handles();
            crate::test_support::frame(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                    focused: true,
                    ..Default::default()
                },
                |ui| {
                    action = Prompt::new(egui::Id::new("prompt_test"), "delete", "Delete")
                        .actions("Delete", "Cancel", Some("Keep"))
                        .selected(Action::Apply)
                        .show(ui.ctx(), true, |ui| {
                            text(ui, "Three features go with it.", Text::Report("body", false));
                        })
                        .1;
                },
            );
        }
        assert_eq!(action, Action::None);
        let header = scenario::rect_of("dialog:header").unwrap();
        let footer = scenario::rect_of("dialog:footer").unwrap();
        assert!((header.width() - (t.form_card_width * 1.5 - 2.0)).abs() <= 3.0, "{dark}: {header:?}");
        assert_eq!(header.width(), footer.width());
        assert!(footer.top() > header.bottom());
        let (ok, apply, cancel) =
            (scenario::rect_of("dialog:ok").unwrap(), scenario::rect_of("dialog:apply").unwrap(), scenario::rect_of("dialog:cancel").unwrap());
        assert!(cancel.right() < apply.left() && apply.right() < ok.left(), "Cancel, the third action, then the primary one");
        assert!(footer.contains_rect(ok) && footer.contains_rect(cancel));
        assert!(scenario::handle_at("dialog:Keep").is_some() && scenario::handle_at("dialog:close").is_some());
    }
    crate::theme::set_dark(previous);
}

#[test]
fn escape_closes_a_prompt_unless_it_keeps_the_card_open() {
    for keep in [false, true] {
        let ctx = crate::fonts::test_context();
        let mut action = Action::None;
        for frame in 0..3 {
            let events = if frame == 2 { crate::ui_kit::test_support::key(egui::Key::Escape) } else { Vec::new() };
            crate::test_support::frame(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    let prompt = Prompt::new(egui::Id::new(("prompt_escape", keep)), "settings", "Report").actions("Open", "Close", None);
                    let prompt = if keep { prompt.keep_open_on_escape() } else { prompt };
                    action = prompt
                        .show(ui.ctx(), true, |ui| {
                            text(ui, "A long text the user is editing.", Text::Report("body", false));
                        })
                        .1;
                },
            );
        }
        assert_eq!(action, if keep { Action::None } else { Action::Cancel }, "keep_open_on_escape={keep}");
    }
}
