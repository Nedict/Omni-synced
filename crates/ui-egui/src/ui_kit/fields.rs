//! Fields elements for dense cards; see `docs/ui-kit.md` for tokens and usage.
use super::{
    layout::{label, publish},
    metrics::*,
    text::paint_text,
};
use crate::{scenario, theme::Tokens};
use egui::{Response, RichText, Ui, vec2};

/// Expression editor with the application's existing completion and number-focus behaviour.
/// Values carry their own unit text ("20 mm", "45 deg"): there is no unit suffix. Returns accepted Enter.
/// Tokens: `control_height, control_fill, form_inset, font_label, space_4; scope tokens`.
pub fn field(ui: &mut Ui, value: &mut String) -> bool {
    editor(ui, value, "", true).0
}

/// Expression editor with empty-value hint. Tokens: `same as field`.
pub fn hint_field(ui: &mut Ui, value: &mut String, hint: &str) -> bool {
    editor(ui, value, hint, true).0
}

/// Names, thread designations and plain numeric counts do not accept expression completion.
/// Plain text editor without expression completion; returns (Enter, changed). Tokens: `same as field`.
pub fn text_field(ui: &mut Ui, value: &mut String, hint: &str) -> (bool, bool) {
    let (enter, changed, _) = editor(ui, value, hint, false);
    (enter, changed)
}

/// Expression field for a single-value popup which keeps ownership of keyboard focus.
/// Tokens: field tokens. Preserves the legacy Edit Dimension focus behavior.
pub fn focused_field(ui: &mut Ui, value: &mut String) -> bool {
    let (enter, _, response) = editor(ui, value, "", true);
    response.request_focus();
    enter
}

/// Multiline plain text with a stable focus ID. The caller owns Enter/Shift+Enter routing.
/// Tokens: control_height, control_fill, form_inset and font_label.
pub fn multiline_text_field(ui: &mut Ui, label: &str, id: egui::Id, value: &mut String) -> Response {
    super::layout::sized_row(ui, label, row_height() * 3.0, |ui| {
        let t = Tokens::get();
        let response = ui.add_sized(
            vec2(dimension(ui.available_width()), row_height() * 3.0),
            egui::TextEdit::multiline(value)
                .id(id)
                .desired_rows(3)
                .font(egui::FontId::monospace(t.font_label))
                .background_color(t.control_fill)
                .margin(vec2(t.form_inset, 0.0)),
        );
        publish(ui, &response, egui::WidgetType::TextEdit);
        response
    })
}

/// Supplied entry in a searchable dropdown. Handles are optional stable application aliases.
pub struct SearchOption<'a> {
    pub value: &'a str,
    pub label: &'a str,
    pub handle: Option<&'a str>,
}

/// Searchable dropdown with an optional custom-value action and loading message.
/// Returns a chosen value; callers keep domain-specific validation and discovery.
/// Tokens: control_height, space_24, font_label and scope control tokens.
pub fn searchable_dropdown(
    ui: &mut Ui,
    id: &str,
    current: &str,
    filter: &mut String,
    options: &[SearchOption<'_>],
    custom: Option<&str>,
    pending: Option<&str>,
) -> Option<String> {
    let mut chosen = None;
    let row = label(ui).to_owned();
    let response = egui::ComboBox::from_id_salt(id)
        .width(dimension(ui.available_width()))
        .height(Tokens::get().space_24 * 14.0)
        .selected_text(current)
        .icon(dropdown_arrow)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            let search = ui.add(
                egui::TextEdit::singleline(filter)
                    .id_salt(format!("{id}_search"))
                    .hint_text(crate::i18n::tr_label("Search"))
                    .desired_width(f32::INFINITY),
            );
            scenario::publish_handle(&format!("dialog:{row} search"), search.rect.center());
            if let Some(custom) = custom {
                let response = ui.selectable_label(false, custom);
                scenario::publish_handle(&format!("dialog:{row} file"), response.rect.center());
                if response.clicked() {
                    chosen = Some(filter.trim().to_owned());
                    ui.close();
                }
                return;
            }
            let query = filter.to_lowercase();
            egui::ScrollArea::vertical()
                .id_salt(format!("{id}_list"))
                .max_height(Tokens::get().space_24 * 10.0)
                .min_scrolled_height(Tokens::get().space_24 * 10.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for option in options.iter().filter(|o| o.label.to_lowercase().contains(&query)) {
                        let response = ui.selectable_label(current == option.label, option.label);
                        if let Some(handle) = option.handle {
                            scenario::publish_handle(handle, response.rect.center());
                        }
                        if response.clicked() {
                            chosen = Some(option.value.to_owned());
                            ui.close();
                        }
                    }
                    if let Some(pending) = pending {
                        ui.label(pending);
                    }
                });
        })
        .response;
    publish(ui, &response, egui::WidgetType::ComboBox);
    chosen
}

/// Completion/focus implementation shared by field variants. Tokens: `same as field`.
fn editor(ui: &mut Ui, value: &mut String, hint: &str, completion: bool) -> (bool, bool, Response) {
    let t = Tokens::get();
    let width = dimension(ui.available_width());
    let font = egui::FontId::monospace(t.font_label);
    let line_height = ui.fonts_mut(|fonts| fonts.row_height(&font)) + ui.spacing().extra_text_line_spacing;
    // Plex Mono's taller metrics must fit the 24 pt field without pushing the next row down.
    // Margins use whole logical pixels; round down so the editor cannot grow beyond its row.
    let margin_y = ((row_height() - line_height) * 0.5).floor().clamp(0.0, t.space_4);
    let response = ui.add_sized(
        vec2(width, row_height()),
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .font(font)
            .background_color(t.control_fill)
            .margin(vec2(t.form_inset, margin_y))
            .clip_text(true),
    );
    crate::theme::TabAccent::number_focus(ui, &response);
    let label = crate::i18n::tr_label(label(ui));
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label);
        info.current_text_value = Some(value.clone());
        info
    });
    scenario::publish_handle(&format!("dialog:{}", self::label(ui)), response.rect.center());
    scenario::publish_rect(&format!("dialog:widget:{}", self::label(ui)), response.rect);
    // A completion's Enter accepts the suggestion, rather than submitting the dialog.
    let completed = completion && crate::params_dialog::complete(ui, &response, value);
    (!completed && response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)), response.changed(), response)
}

/// The one dropdown arrow every kit dropdown draws (the kit chevron, dimmed), instead of egui's triangle.
fn dropdown_arrow(ui: &Ui, rect: egui::Rect, _visuals: &egui::style::WidgetVisuals, _open: bool) {
    crate::icons::paint_ink(ui.painter(), rect, "chevron", Tokens::get().text_dim);
}

/// Native dropdown with kit arrow and scenario option handles. Tokens: `text_dim; scope control/font tokens`.
pub fn dropdown(ui: &mut Ui, id: &str, selected: &mut usize, options: &[&str]) -> Response {
    let text = options.get(*selected).copied().map(crate::i18n::tr_label).unwrap_or("");
    let row = label(ui).to_string();
    let response =
        egui::ComboBox::from_id_salt(id).width(dimension(ui.available_width())).selected_text(text).icon(dropdown_arrow).show_ui(ui, |ui| {
            for (index, option) in options.iter().enumerate() {
                let response = ui.selectable_value(selected, index, crate::i18n::tr_label(option));
                scenario::publish_handle(&format!("dialog:{row}:{option}"), response.rect.center());
            }
        });
    publish(ui, &response.response, egui::WidgetType::ComboBox);
    response.response
}

/// Filled point/curve/edge selection target. Tokens: `control_height, control_radius, selected_bg/ink, accent, form_inset, space_24/8, font_label, text_dim`.
pub fn selection_field(ui: &mut Ui, text: &str, active: bool) -> Response {
    selection_field_state(ui, text, active, true)
}

/// Selection target with explicit filled state. Tokens: `same as selection_field`.
pub fn selection_field_state(ui: &mut Ui, text: &str, active: bool, filled: bool) -> Response {
    let t = Tokens::get();
    let (rect, response) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), row_height()), egui::Sense::CLICK);
    ui.painter().rect(rect, t.control_radius, t.selected_bg, egui::Stroke::new(if active { 1.0 } else { 0.0 }, t.accent), egui::StrokeKind::Inside);
    let end = if filled { t.space_24 } else { t.space_8 };
    let text_rect = egui::Rect::from_min_max(
        rect.min + vec2(t.form_inset.min(rect.width()), 0.0),
        egui::pos2((rect.right() - end).max(rect.left() + t.form_inset.min(rect.width())), rect.bottom()),
    );
    paint_text(ui, text_rect, text, crate::fonts::medium(t.font_label), if filled { t.selected_ink } else { t.text_dim }, false);
    publish(ui, &response, egui::WidgetType::Button);
    scenario::publish_rect(&format!("dialog:field:{}", label(ui)), rect);
    scenario::publish_count(&format!("dialog:selection:{}:filled", label(ui)), f64::from(filled));
    response
}

/// Two labelled expression values in one aligned row, e.g. X/Y. Returns accepted Enter.
/// Tokens: row/label column metrics, space_8 and field's font/control tokens.
/// Two labelled numeric fields in one row; returns accepted Enter. Tokens: `row/field tokens, space_8, dialog_label`.
pub fn value_pair(ui: &mut Ui, label: &str, values: [&mut String; 2], axes: [&str; 2]) -> bool {
    value_pair_named(ui, label, values, axes, axes)
}

/// Paired values with compact axis captions and independent stable field names.
/// Tokens: value_pair tokens. Use when existing scenario/accessibility names differ from X/Y.
pub fn value_pair_named(ui: &mut Ui, label: &str, values: [&mut String; 2], axes: [&str; 2], names: [&str; 2]) -> bool {
    let t = Tokens::get();
    super::row(ui, label, |ui| {
        let width = ((dimension(ui.available_width()) - t.space_8) / 2.0).max(0.0);
        let mut enter = false;
        ui.spacing_mut().item_spacing.x = t.space_8;
        for ((axis, value), name) in axes.into_iter().zip(values).zip(names) {
            ui.allocate_ui_with_layout(vec2(width, row_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                ui.label(RichText::new(axis).size(t.font_label).color(t.dialog_label));
                enter |= super::layout::named_control(ui, name, |ui| field(ui, value));
            });
        }
        enter
    })
}

/// Variable-radius location editor. Tokens: scope's control and typography tokens.
/// Retains the established 0.01 step, fractional range and locale parser.
/// Locale-aware variable-radius position editor. Tokens: `scope control/font tokens`.
pub fn fraction_field(ui: &mut Ui, value: &mut f64) -> Response {
    let response = ui.add(egui::DragValue::new(value).speed(0.01).range(0.01..=0.99).fixed_decimals(2).custom_parser(crate::prefs::parse_number));
    scenario::publish_rect(&format!("dialog:fraction:{}", label(ui)), response.rect);
    scenario::publish_handle(&format!("dialog:fraction:{}", label(ui)), response.rect.center());
    response
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
