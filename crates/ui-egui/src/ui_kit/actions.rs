//! Actions elements for dense cards; see `docs/ui-kit.md` for tokens and usage.
use super::metrics::*;
use crate::{fonts::RichTextExt as _, scenario, theme::Tokens};
use egui::{Response, RichText, Ui, vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    None,
    Ok,
    Cancel,
    Apply,
}

/// Every enabled button shows the pointing hand on hover, as the application's other clickable
/// chrome does. A disabled button stays inert and keeps the arrow.
pub(crate) fn pointer_on_hover(ui: &Ui, response: &Response) {
    if response.enabled() && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// Icon action inside an existing rectangle. Tokens: `form_icon_inset, text, text_dim; icon palette`.
pub fn icon_button_at(ui: &mut Ui, rect: egui::Rect, icon: &str, label: &str) -> Response {
    let t = Tokens::get();
    let response = ui.interact(rect, ui.make_persistent_id(("icon_button", icon, label)), egui::Sense::click());
    crate::icons::paint_ink(
        ui.painter(),
        rect.shrink(t.form_icon_inset),
        icon,
        if response.hovered() || response.has_focus() { t.text } else { t.text_dim },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), crate::i18n::tr_label(label)));
    pointer_on_hover(ui, &response);
    response.on_hover_text(crate::i18n::tr_label(label))
}

/// Enter applies only when no editor owns it. Editors report their own accepted Enter.
/// Esc cancels; unowned Enter submits only when valid. Tokens: `none`.
pub fn keyboard(ctx: &egui::Context, ok_enabled: bool) -> Action {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        Action::Cancel
    } else if ok_enabled && ctx.memory(|m| m.focused().is_none()) && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        Action::Ok
    } else {
        Action::None
    }
}

/// Pinned actions; `apply_label` adds Apply without closing. Tokens: form_footer_height,
/// form_button_height, dialog_footer/rule, control_*, accent, on_accent, font_label, spacing.
/// Pinned OK / optional Apply / Cancel; returns Action. Tokens: `form_footer_height, form_button_height, dialog_footer/rule, radius_lg, control_*, accent, on_accent, font_label, space_6/10/12/16`.
pub fn footer(ui: &mut Ui, ok_label: &str, cancel_label: &str, ok_enabled: bool, apply_label: Option<&str>) -> Action {
    footer_impl(ui, ok_label, cancel_label, ok_enabled, apply_label, None)
}

/// Footer with stable button IDs and compute text. Tokens: `footer tokens, text_dim, space_4/8`.
pub fn footer_with_status(
    ui: &mut Ui,
    ok_label: &str,
    cancel_label: &str,
    ok_enabled: bool,
    apply_label: Option<&str>,
    status: Option<&str>,
) -> Action {
    footer_impl(ui, ok_label, cancel_label, ok_enabled, apply_label, status)
}

/// Shared footer implementation. Tokens: `footer tokens`.
fn footer_impl(ui: &mut Ui, ok_label: &str, cancel_label: &str, ok_enabled: bool, apply_label: Option<&str>, status: Option<&str>) -> Action {
    let t = Tokens::get();
    // The footer is a full-width band (the card has no margin of its own): its own fill, a rule
    // on top, and the buttons right-aligned with 6 pt above/below and 10 pt at the sides.
    let (band, _) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), t.form_footer_height), egui::Sense::hover());
    let corner = t.radius_lg.saturating_sub(1);
    ui.painter().rect_filled(band, egui::CornerRadius { nw: 0, ne: 0, sw: corner, se: corner }, t.dialog_footer);
    ui.painter().hline(band.x_range(), band.top(), egui::Stroke::new(1.0, t.dialog_rule));
    scenario::publish_rect("dialog:footer", band);
    scenario::publish_count("dialog:ok:enabled", f64::from(ok_enabled && ui.is_enabled()));
    let inner = band.shrink2(vec2(t.space_10, t.space_6));
    let mut action = if ui.is_enabled() { keyboard(ui.ctx(), ok_enabled) } else { Action::None };
    let mut actions_left = inner.right();
    let row = ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = t.space_6;
            for (label, kind) in [(ok_label, Action::Ok), (apply_label.unwrap_or(""), Action::Apply), (cancel_label, Action::Cancel)] {
                if kind == Action::Apply && apply_label.is_none() {
                    continue;
                }
                let primary = kind == Action::Ok;
                let live = kind == Action::Cancel || ok_enabled;
                ui.spacing_mut().button_padding = vec2(if primary { t.space_16 } else { t.space_12 }, 0.0);
                let text = RichText::new(crate::i18n::tr_label(label)).size(t.font_label).color(if primary && ok_enabled {
                    t.on_accent
                } else if primary {
                    t.text_dim
                } else {
                    t.text
                });
                let response = ui.add_enabled(
                    live,
                    egui::Button::new(if primary && ok_enabled { text.bold_face() } else { text })
                        .fill(if primary && ok_enabled { t.accent } else { t.control_fill })
                        .stroke(egui::Stroke::new(if primary && ok_enabled { 0.0 } else { 1.0 }, t.control_border))
                        .wrap_mode(egui::TextWrapMode::Extend)
                        .corner_radius(t.control_radius)
                        .min_size(vec2(0.0, t.form_button_height)),
                );
                pointer_on_hover(ui, &response);
                actions_left = actions_left.min(response.rect.left());
                let key = match kind {
                    Action::Ok => "dialog:ok",
                    Action::Apply => "dialog:apply",
                    _ => "dialog:cancel",
                };
                scenario::publish_handle(key, response.rect.center());
                scenario::publish_rect(key, response.rect);
                scenario::publish_handle(&format!("dialog:{label}"), response.rect.center());
                if response.clicked() {
                    action = kind;
                }
            }
        });
    });
    if let Some(status) = status {
        let left = (row.response.rect.left() + t.space_16 + t.space_4).min(actions_left);
        let right = (actions_left - t.space_8).max(left);
        let rect = egui::Rect::from_min_max(egui::pos2(left, inner.top()), egui::pos2(right, inner.bottom()));
        // Added after the actions, so status changes cannot change the button IDs.
        ui.put(rect, egui::Label::new(RichText::new(status).medium().color(t.text_dim)).truncate());
    }
    action
}

/// One equal-width secondary action, with optional existing icon master.
pub struct PairButton<'a> {
    pub key: &'a str,
    pub label: &'a str,
    pub icon: Option<&'a str>,
}

/// Equal secondary footer actions; Tokens: form_button_height, control_*, selected_bg, hover, font_label, spacing.
/// Equal-width secondary actions with optional icons and selected state. Tokens: `form_button_height, control_*, selected_bg, hover, font_label, text, icon/fill, accent, sketch_footer_icon_size, space_4/6/10`.
pub fn button_pair(ui: &mut egui::Ui, key: &str, states: [bool; 2], buttons: [PairButton<'_>; 2]) -> (bool, bool) {
    let t = Tokens::get();
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), egui::Stroke::new(1.0, t.dialog_rule));
    let mut clicked = [false; 2];
    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(t.space_10 as i8, t.space_6 as i8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_6;
            let width = (ui.available_width() - t.space_6) / 2.0;
            for (index, button) in buttons.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(width, t.form_button_height), egui::Sense::click());
                let on = states.get(index).copied().unwrap_or(false);
                ui.painter().rect(
                    rect,
                    t.control_radius,
                    if on {
                        t.selected_bg
                    } else if response.hovered() || response.has_focus() {
                        t.hover
                    } else {
                        t.control_fill
                    },
                    egui::Stroke::new(1.0, t.control_border),
                    egui::StrokeKind::Inside,
                );
                let label = crate::i18n::tr_label(button.label);
                let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(t.font_label), t.text);
                let icon_width = if button.icon.is_some() { t.sketch_footer_icon_size + t.space_4 } else { 0.0 };
                let left = rect.center().x - (galley.size().x + icon_width) / 2.0;
                let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
                if let Some(icon) = button.icon {
                    crate::icons::paint(
                        &painter,
                        egui::Rect::from_center_size(
                            egui::pos2(left + t.sketch_footer_icon_size / 2.0, rect.center().y),
                            vec2(t.sketch_footer_icon_size, t.sketch_footer_icon_size),
                        ),
                        icon,
                        t.icon,
                        t.icon_fill,
                        t.accent,
                    );
                }
                painter.galley(egui::pos2(left + icon_width, rect.center().y - galley.size().y / 2.0), galley, t.text);
                response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, label));
                pointer_on_hover(ui, &response);
                crate::scenario::publish_handle(&format!("{key}:{}", button.key), rect.center());
                crate::scenario::publish_rect(&format!("{key}:{}", button.key), rect);
                if let Some(value) = clicked.get_mut(index) {
                    *value = response.on_hover_text(label).clicked();
                }
            }
        });
    });
    (clicked.first().copied().unwrap_or(false), clicked.get(1).copied().unwrap_or(false))
}

/// Ordinary secondary action inheriting `scope`'s control and font tokens; preserves egui sizing.
/// Ordinary secondary action preserving native sizing. Tokens: `scope font/control/spacing tokens`.
pub fn secondary_button(ui: &mut Ui, label: impl Into<egui::WidgetText>) -> Response {
    let label = label.into();
    let key = label.text().to_owned();
    let response = ui.button(label);
    pointer_on_hover(ui, &response);
    publish_button(ui, &response, &key);
    response
}

/// Accent action. Tokens: accent, on_accent and the scope's control/font/spacing tokens.
/// Accent action outside the footer. Tokens: `accent, on_accent; scope tokens`.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let t = Tokens::get();
    let response = ui.add(egui::Button::new(RichText::new(crate::i18n::tr_label(label)).color(t.on_accent)).fill(t.accent));
    pointer_on_hover(ui, &response);
    scenario::publish_rect(&format!("dialog:button:{label}"), response.rect);
    scenario::publish_handle(&format!("dialog:button:{label}"), response.rect.center());
    response
}

/// Compact inline removal action; Tokens: scope's font and control tokens, egui small padding.
/// Compact inline action preserving native sizing. Tokens: `scope font/control tokens`.
pub fn small_button(ui: &mut Ui, label: &str) -> Response {
    let response = ui.small_button(label);
    pointer_on_hover(ui, &response);
    publish_button(ui, &response, label);
    response
}

/// Allocate a control-height icon action. Tokens: control_height, form_icon_inset, text/dim.
/// Allocated control-height icon action. Tokens: `control_height; icon_button_at tokens`.
pub fn icon_button(ui: &mut Ui, label: &str, icon: &str) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(row_height(), row_height()), egui::Sense::hover());
    let response = icon_button_at(ui, rect, icon, label);
    scenario::publish_rect(&format!("dialog:button:{label}"), rect);
    scenario::publish_handle(&format!("dialog:button:{label}"), rect.center());
    response
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

// Additional identities must never replace an existing field handle in compound rows.
fn publish_button(ui: &Ui, response: &Response, label: &str) {
    let key = format!("dialog:button:{}:{label}", super::layout::label(ui));
    scenario::publish_handle(&key, response.rect.center());
    scenario::publish_rect(&key, response.rect);
}
