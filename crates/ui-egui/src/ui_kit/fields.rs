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
pub(crate) fn field(ui: &mut Ui, value: &mut String) -> bool {
    editor(ui, value, "", true).0
}

/// Expression editor with empty-value hint. Tokens: `same as field`.
pub(crate) fn hint_field(ui: &mut Ui, value: &mut String, hint: &str) -> bool {
    editor(ui, value, hint, true).0
}

/// Names, thread designations and plain numeric counts do not accept expression completion.
/// Plain text editor without expression completion; returns (Enter, changed). Tokens: `same as field`.
pub(crate) fn text_field(ui: &mut Ui, value: &mut String, hint: &str) -> (bool, bool) {
    let (enter, changed, _) = editor(ui, value, hint, false);
    (enter, changed)
}

/// Expression field for a single-value popup which keeps ownership of keyboard focus.
/// Tokens: field tokens. Preserves the legacy Edit Dimension focus behavior.
pub(crate) fn focused_field(ui: &mut Ui, value: &mut String) -> bool {
    let (enter, _, response) = editor(ui, value, "", true);
    response.request_focus();
    enter
}

/// Plain text editor which keeps keyboard focus (a name asked for in a one-field card); returns
/// accepted Enter. Tokens: `same as field`.
pub(crate) fn focused_text_field(ui: &mut Ui, value: &mut String, hint: &str) -> bool {
    let (enter, _, response) = editor(ui, value, hint, false);
    response.request_focus();
    enter
}

/// Multiline plain text with a stable focus ID. The caller owns Enter/Shift+Enter routing.
/// Tokens: control_height, control_fill, form_inset and font_label.
pub(crate) fn multiline_text_field(ui: &mut Ui, label: &str, id: egui::Id, value: &mut String) -> Response {
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
pub(crate) struct SearchOption<'a> {
    pub value: &'a str,
    pub label: &'a str,
    pub handle: Option<&'a str>,
}

/// Searchable dropdown with an optional custom-value action and loading message.
/// Returns a chosen value; callers keep domain-specific validation and discovery.
/// Tokens: control_height, space_24, font_label and scope control tokens.
pub(crate) fn searchable_dropdown(
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
    super::layout::publish_geometry(ui, &response);
    // A completion's Enter accepts the suggestion, rather than submitting the dialog.
    let completed = completion && crate::params_dialog::complete(ui, &response, value);
    (!completed && response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)), response.changed(), response)
}

/// The one dropdown arrow every kit dropdown draws (the kit chevron, dimmed), instead of egui's triangle.
fn dropdown_arrow(ui: &Ui, rect: egui::Rect, _visuals: &egui::style::WidgetVisuals, _open: bool) {
    crate::icons::paint_ink(ui.painter(), rect, "chevron", Tokens::get().text_dim);
}

/// Native dropdown with kit arrow and scenario option handles. Tokens: `text_dim; scope control/font tokens`.
pub(crate) fn dropdown(ui: &mut Ui, id: &str, selected: &mut usize, options: &[&str]) -> Response {
    let text = options.get(*selected).copied().map(crate::i18n::tr_label).unwrap_or("");
    let row = label(ui).to_string();
    dropdown_menu(ui, id, text, |ui| {
        for (index, option) in options.iter().enumerate() {
            let response = ui.selectable_value(selected, index, crate::i18n::tr_label(option));
            scenario::publish_handle(&format!("dialog:{row}:{option}"), response.rect.center());
        }
    })
}

/// Dropdown with caller-supplied kit choices/actions, including disabled entries and stable
/// application handles. Tokens: dropdown tokens. Use `dropdown` for a simple indexed list.
pub(crate) fn dropdown_menu(ui: &mut Ui, id: impl egui::AsIdSalt, current: &str, add: impl FnOnce(&mut Ui)) -> Response {
    let response = egui::ComboBox::from_id_salt(id)
        .width(dimension(ui.available_width()))
        .height(Tokens::get().control_height * 14.0)
        .selected_text(current)
        .icon(dropdown_arrow)
        .show_ui(ui, add)
        .response;
    publish(ui, &response, egui::WidgetType::ComboBox);
    response
}

/// Selection target state; filled targets retain their reserved trailing space.
pub(crate) struct Selection {
    pub active: bool,
    pub filled: bool,
}

pub(crate) fn selection_field(ui: &mut Ui, text: &str, options: Selection) -> Response {
    let Selection { active, filled } = options;
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

/// Paired values with compact captions and optional independent scenario/accessibility names.
pub(crate) fn value_pair(ui: &mut Ui, label: &str, values: [&mut String; 2], axes: [&str; 2], names: Option<[&str; 2]>) -> bool {
    let names = names.unwrap_or(axes);
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
pub(crate) fn fraction_field(ui: &mut Ui, value: &mut f64) -> Response {
    let response = ui.add(egui::DragValue::new(value).speed(0.01).range(0.01..=0.99).fixed_decimals(2).custom_parser(crate::prefs::parse_number));
    scenario::publish_target(&format!("dialog:fraction:{}", label(ui)), response.rect);
    response
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;

/// Stable-ID inline editor for tables. Completion is owned by the caller so Enter can commit
/// a cell without submitting its panel. Tokens: control_height/fill, form_inset, font_label, text/error.
pub(crate) fn cell_editor(ui: &mut Ui, id: egui::Id, value: &mut String, hint: &str, invalid: bool) -> Response {
    bounded_cell_editor(ui, id, value, hint, invalid, usize::MAX)
}

/// Stable-ID cell/name editor with a caller-supplied character cap. Tokens: cell_editor tokens.
pub(crate) fn bounded_cell_editor(ui: &mut Ui, id: egui::Id, value: &mut String, hint: &str, invalid: bool, limit: usize) -> Response {
    let t = Tokens::get();
    let response = ui.add_sized(
        vec2(dimension(ui.available_width()), row_height()),
        egui::TextEdit::singleline(value)
            .char_limit(limit)
            .id(id)
            .hint_text(crate::i18n::tr_label(hint))
            .font(egui::FontId::monospace(t.font_label))
            .background_color(t.control_fill)
            .text_color(if invalid { t.error } else { t.text })
            .margin(vec2(t.form_inset, 0.0))
            .clip_text(true),
    );
    publish(ui, &response, egui::WidgetType::TextEdit);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), crate::i18n::tr_label(label(ui)));
        info.current_text_value = Some(value.clone());
        info
    });
    crate::theme::TabAccent::number_focus(ui, &response);
    response
}

/// Search input with native accessibility and a stable caller ID. Tokens: cell_editor tokens.
pub(crate) fn search_field(ui: &mut Ui, id: egui::Id, value: &mut String, hint: &str) -> Response {
    cell_editor(ui, id, value, hint, false)
}

/// Native colour chooser under the shared form styling. Colour is document data.
pub(crate) fn color_field(ui: &mut Ui, rgb: &mut [u8; 3]) -> Response {
    let response = ui.color_edit_button_srgb(rgb);
    scenario::publish_target(&format!("dialog:color:{}", label(ui)), response.rect);
    response
}

/// Continuous scalar control. Formatting never rounds saved values during drawing.
/// Tokens: control_height, space_24/4, scope control/font tokens.
pub(crate) fn slider(ui: &mut Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>) -> Response {
    let t = Tokens::get();
    ui.spacing_mut().slider_width = dimension(ui.available_width() - t.space_24 - t.space_24 - t.space_4);
    let response = ui.add(egui::Slider::new(value, range).max_decimals_opt(None).custom_formatter(|v, _| format!("{v:.2}")));
    scenario::publish_target(&format!("dialog:slider:{}", label(ui)), response.rect);
    response
}

/// Locale-aware bounded scalar field, used for dimensionless material properties.
pub(crate) fn scalar_field(ui: &mut Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>) -> Response {
    let response = ui.add(egui::DragValue::new(value).range(range).speed(0.01).custom_parser(crate::prefs::parse_number));
    scenario::publish_target(&format!("dialog:scalar:{}", label(ui)), response.rect);
    response
}

/// Length held in millimetres, dragged or typed in the design's units ("0.5 in", "2 cm + 3 mm" parse).
/// Tokens: control_height, scope control/font tokens.
pub(crate) fn length_field(ui: &mut Ui, value: &mut f64, units: &str, decimals: u8) -> Response {
    let response = ui.add(length_drag(value, units, decimals));
    scenario::publish_target(&format!("dialog:length:{}", label(ui)), response.rect);
    response
}

/// Angle in degrees, dragged or typed. Tokens: control_height, scope control/font tokens.
pub(crate) fn angle_field(ui: &mut Ui, value: &mut f64) -> Response {
    let response = ui.add(egui::DragValue::new(value).speed(1.0).suffix(" deg").custom_parser(crate::prefs::parse_number));
    scenario::publish_target(&format!("dialog:angle:{}", label(ui)), response.rect);
    response
}

/// Bounded multiline data input. Caller owns validation and keyboard semantics.
/// Tokens: control_height/fill, form_inset, font_label.
pub(crate) fn text_area(ui: &mut Ui, label: &str, id: egui::Id, value: &mut String, rows: usize, limit: usize) -> Response {
    let t = Tokens::get();
    let response = ui.add_sized(
        vec2(dimension(ui.available_width()), row_height() * rows.clamp(1, 16) as f32),
        egui::TextEdit::multiline(value)
            .id(id)
            .desired_rows(rows.clamp(1, 16))
            .char_limit(limit)
            .font(egui::FontId::monospace(t.font_label))
            .background_color(t.control_fill)
            .margin(vec2(t.form_inset, 0.0)),
    );
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label);
        info.current_text_value = Some(value.clone());
        info
    });
    response
}

/// Secret input with a stable ID. Never publishes its value in accessibility/scenario metadata.
/// Tokens: cell_editor tokens.
pub(crate) fn password_field(ui: &mut Ui, id: egui::Id, value: &mut String, hint: &str) -> Response {
    let t = Tokens::get();
    let response = ui.add_sized(
        vec2(dimension(ui.available_width()), row_height()),
        egui::TextEdit::singleline(value)
            .id(id)
            .password(true)
            .hint_text(hint)
            .font(egui::FontId::monospace(t.font_label))
            .background_color(t.control_fill)
            .margin(vec2(t.form_inset, 0.0)),
    );
    publish(ui, &response, egui::WidgetType::TextEdit);
    response
}

/// Bounded integer or decimal preference with its original drag step and unit suffix.
/// Tokens: scope control/font tokens. Values and limits are caller data.
pub(crate) fn number_field<T: egui::emath::Numeric>(
    ui: &mut Ui,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    speed: f64,
    suffix: &str,
) -> Response {
    let response = ui.add(egui::DragValue::new(value).range(range).speed(speed).suffix(suffix).custom_parser(crate::prefs::parse_number));
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::drag_value(ui.is_enabled(), value.to_f64());
        info.label = Some(crate::i18n::tr_label(label(ui)).to_string());
        info
    });
    super::layout::publish_geometry(ui, &response);
    response
}

/// Stepped slider retaining caller units/formatting (percent, multiplier, seconds).
/// Tokens: control_height, space_24/4, scope control/font tokens.
pub(crate) fn formatted_slider<T: egui::emath::Numeric>(
    ui: &mut Ui,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    step: f64,
    format: impl Fn(f64) -> String + 'static,
) -> Response {
    let t = Tokens::get();
    ui.spacing_mut().slider_width = dimension(ui.available_width() - t.space_24 * 2.0 - t.space_4);
    let response = ui.add(egui::Slider::new(value, range).step_by(step).custom_formatter(move |v, _| format(v)));
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), value.to_f64(), crate::i18n::tr_label(label(ui))));
    super::layout::publish_geometry(ui, &response);
    response
}

/// Growing plain-text message editor; its parent owns scrolling and Enter routing.
/// Tokens: control_fill, form_inset, font_label. Use text_area for bounded data input.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn message_field(ui: &mut Ui, id: egui::Id, value: &mut String, hint: &str) -> Response {
    let t = Tokens::get();
    ui.add(
        egui::TextEdit::multiline(value)
            .id(id)
            .hint_text(hint)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .font(egui::FontId::proportional(t.font_label))
            .background_color(t.control_fill)
            .margin(vec2(t.form_inset, t.space_4)),
    )
}

/// Edit a millimetre value through a control expressed in the current design units.
/// Keep the stored value untouched unless the user actually changes the field.
fn length_drag<'a>(value: &'a mut f64, units: &str, decimals: u8) -> egui::DragValue<'a> {
    let (scale, unit) = crate::prefs::unit_scale(units);
    egui::DragValue::new(value)
        .speed(0.5)
        .suffix(format!(" {unit}"))
        .custom_formatter(move |mm, _| crate::prefs::length_number(mm, unit, decimals))
        .custom_parser(move |text| crate::prefs::parse_length_mm(text, scale))
}

/// Timeline/drive slider without a numeric editor; the owning surface shows its value.
/// Whole steps round only while manipulating a timeline. Tokens: control_height, space_4.
pub(crate) fn timeline_slider(ui: &mut Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>, whole_steps: bool) -> Response {
    ui.spacing_mut().slider_width = dimension(ui.available_width() - Tokens::get().space_4);
    let slider = egui::Slider::new(value, range).show_value(false);
    let response = ui.add(if whole_steps { slider.integer() } else { slider });
    publish(ui, &response, egui::WidgetType::Slider);
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *value, crate::i18n::tr_label(label(ui))));
    response
}

/// Placement editor whose owner routes Enter and Tab (including rejected expressions).
/// Completion remains with the caller. Tokens: cell_editor tokens.
pub(crate) fn placement_field(ui: &mut Ui, id: egui::Id, value: &mut String, hint: &str) -> Response {
    let t = Tokens::get();
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .id(id)
            .desired_width(t.control_height * 3.0 - t.space_6)
            .return_key(None)
            .event_filter(egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, ..Default::default() })
            .font(egui::FontId::proportional(t.font_body))
            .background_color(t.control_fill)
            .text_color(t.text)
            .margin(vec2(t.space_4, t.space_2))
            .clip_text(true),
    );
    publish(ui, &response, egui::WidgetType::TextEdit);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), crate::i18n::tr_label(hint));
        info.current_text_value = Some(value.clone());
        info
    });
    crate::theme::TabAccent::number_focus(ui, &response);
    response.on_hover_text(crate::i18n::tr_label(hint))
}
