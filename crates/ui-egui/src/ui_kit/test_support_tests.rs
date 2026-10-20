use super::*;
use crate::scenario;
use egui::{Ui, vec2};

pub(crate) fn frame(ctx: &egui::Context, events: Vec<egui::Event>, mut add: impl FnMut(&mut Ui)) {
    scenario::clear_handles();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(320.0, 400.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.set_width(280.0);
            scope(ui, &mut add);
        },
    );
    // These tests exercise layout/input without a renderer.
    output.textures_delta.clear();
}

pub(crate) fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() },
    ]
}

pub(crate) fn key(key: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]
}

/// The cursor the frame asks the platform for: the pointer is at `pos` for the whole frame.
pub(crate) fn frame_cursor(ctx: &egui::Context, pos: egui::Pos2, mut add: impl FnMut(&mut Ui)) -> egui::CursorIcon {
    scenario::clear_handles();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(320.0, 400.0))),
            events: vec![egui::Event::PointerMoved(pos)],
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.set_width(280.0);
            scope(ui, &mut add);
        },
    );
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}
