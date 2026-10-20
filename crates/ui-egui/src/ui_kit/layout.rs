//! Layout elements for dense cards; see `docs/ui-kit.md` for tokens and usage.
use super::{metrics::*, text::paint_text};
use crate::{scenario, theme::Tokens};
use egui::{Response, RichText, Ui, vec2};

pub(super) const FORM: &str = "sc_dense_form";

pub(super) const LABEL: &str = "sc_form_label";

/// Mark a form without global state: children inherit this tag through their UI stack.
/// Inherited form styling and label-column identity. Tokens: `space_4/8, form_gap_y, control_height, font_label, accent, selected_bg, dialog_rule, control_fill/border/radius, text`.
pub fn scope<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag(FORM)), |ui| {
        let t = Tokens::get();
        ui.spacing_mut().item_spacing = vec2(t.space_4, t.form_gap_y);
        ui.spacing_mut().interact_size.y = row_height();
        ui.spacing_mut().button_padding = vec2(t.space_8, t.space_4);
        ui.style_mut().text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(t.font_label));
        ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(t.font_label));
        let visuals = ui.visuals_mut();
        visuals.selection.stroke = egui::Stroke::new(1.0, t.accent);
        visuals.selection.bg_fill = t.selected_bg;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, t.dialog_rule);
        for state in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered, &mut visuals.widgets.active, &mut visuals.widgets.open] {
            state.bg_fill = t.control_fill;
            state.weak_bg_fill = t.control_fill;
            state.bg_stroke = egui::Stroke::new(1.0, t.control_border);
            state.corner_radius = t.control_radius.into();
            state.expansion = 0.0;
            state.fg_stroke.color = t.text;
        }
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        add(ui)
    })
    .inner
}

/// Detect an inherited form scope. Tokens: `none`.
pub(crate) fn contains(ui: &Ui) -> bool {
    ui.stack().iter().any(|node| node.tags().contains(FORM))
}

/// Read the enclosing row accessibility label. Tokens: `none`.
pub(super) fn label(ui: &Ui) -> &str {
    ui.stack().iter().find_map(|node| node.tags().get_downcast::<String>(LABEL)).map(String::as_str).unwrap_or("")
}

/// Publish accessibility and scenario geometry. Tokens: `none`.
pub(super) fn publish(ui: &Ui, response: &Response, kind: egui::WidgetType) {
    let label = label(ui);
    response.widget_info(|| egui::WidgetInfo::labeled(kind, ui.is_enabled(), crate::i18n::tr_label(label)));
    publish_geometry(ui, response);
}

/// Aligned label/control row, 24 pt today. Tokens: `control_height, form_label_width, font_label, dialog_label, space_8`.
pub fn row<R>(ui: &mut Ui, text: &str, control: impl FnOnce(&mut Ui) -> R) -> R {
    sized_row(ui, text, row_height(), control)
}

/// Internal form geometry for controls which need several lines (e.g. text areas).
pub(super) fn sized_row<R>(ui: &mut Ui, text: &str, height: f32, control: impl FnOnce(&mut Ui) -> R) -> R {
    let t = Tokens::get();
    let width = dimension(ui.available_width());
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let label_width = label_column(ui, text, width);
    let label_rect = egui::Rect::from_min_size(rect.min, vec2(label_width, rect.height()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(label_rect).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        // Left aligned in the label column, as in the design.
        ui.add(egui::Label::new(RichText::new(crate::i18n::tr_label(text)).size(t.font_label).color(t.dialog_label)).truncate())
            .on_hover_text(crate::i18n::tr_label(text));
    });
    let control_rect = egui::Rect::from_min_max(egui::pos2((label_rect.right() + t.space_8).min(rect.right()), rect.top()), rect.max);
    let result = ui
        .scope_builder(
            egui::UiBuilder::new()
                .id_salt(text)
                .max_rect(control_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center))
                .ui_stack_info(egui::UiStackInfo::default().with_tag_value(LABEL, text.to_string())),
            |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(control_rect));
                ui.spacing_mut().text_edit_width = control_rect.width();
                control(ui)
            },
        )
        .inner;
    scenario::publish_rect(&format!("dialog:row:{text}"), rect);
    scenario::publish_rect(&format!("dialog:label:{text}"), label_rect);
    scenario::publish_rect(&format!("dialog:control:{text}"), control_rect);
    scenario::publish_handle(&format!("dialog:row:{text}"), rect.center());
    result
}

/// A dim one-line caption in the control column of the row labelled `aligned_to`, directly under that
/// row's control. It says in words what a colour or an icon shows, so colour is never the only cue.
/// Dim helper beneath a row control. Tokens: `form_icon_size, font_small, text_dim, space_8; label-column tokens`.
pub fn caption(ui: &mut Ui, aligned_to: &str, text: &str) {
    let t = Tokens::get();
    let width = dimension(ui.available_width());
    let (rect, _) = ui.allocate_exact_size(vec2(width, t.form_icon_size), egui::Sense::hover());
    let left = (rect.left() + label_column(ui, aligned_to, width) + t.space_8).min(rect.right());
    let text_rect = egui::Rect::from_min_max(egui::pos2(left, rect.top()), rect.max);
    paint_text(ui, text_rect, text, egui::FontId::proportional(t.font_small), t.text_dim, false);
    scenario::publish_rect(&format!("dialog:caption:{aligned_to}"), text_rect);
}

/// Memory belongs to the enclosing dialog ID; animation is avoided so each row keeps its fixed height.
/// Persisted disclosure with a rule and uppercase title. Tokens: `control_height, space_2, radius_sm, hover, dialog_rule, form_section_icon_size, form_gap_y, font_caption, font_section_tracking, text_dim`.
pub fn section<R>(ui: &mut Ui, id: impl egui::AsIdSalt, title: &str, default_open: bool, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let t = Tokens::get();
    ui.add_space(t.space_2);
    let id = ui.make_persistent_id(id);
    let mut open = ui.ctx().data_mut(|data| data.get_persisted::<bool>(id)).unwrap_or(default_open);
    let (rect, response) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), row_height()), egui::Sense::click());
    // The section's top rule is the border of its header row.
    ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, t.dialog_rule));
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect.shrink2(vec2(0.0, 1.0)), t.radius_sm, t.hover);
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.form_section_icon_size / 2.0, rect.center().y),
        vec2(t.form_section_icon_size, t.form_section_icon_size),
    );
    crate::icons::paint_ink(ui.painter(), icon_rect, if open { "chevron" } else { "chevron_right" }, t.text_dim);
    let text_rect = egui::Rect::from_min_max(egui::pos2((icon_rect.right() + t.form_gap_y).min(rect.right()), rect.top()), rect.max);
    paint_text(ui, text_rect, &crate::i18n::tr_label(title).to_uppercase(), crate::fonts::semibold(t.font_caption), t.text_dim, true);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, ui.is_enabled(), open, crate::i18n::tr_label(title)));
    scenario::publish_handle(&format!("dialog:section:{title}"), response.rect.center());
    scenario::publish_rect(&format!("dialog:section:{title}"), rect);
    if response.clicked() {
        open = !open;
    }
    ui.ctx().data_mut(|data| data.insert_persisted(id, open));
    open.then(|| ui.push_id(id, add).inner)
}

/// Dim helper line beneath a header. Tokens: control_height, font_small and text_dim.
/// Helper text below a card header. Tokens: `control_height, font_small, text_dim`.
pub fn hint(ui: &mut Ui, key: &str, text: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), row_height()), egui::Sense::hover());
    paint_text(ui, rect, text, egui::FontId::proportional(Tokens::get().font_small), Tokens::get().text_dim, false);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    scenario::publish_rect(&format!("dialog:hint:{key}"), rect);
    scenario::publish_handle(&format!("dialog:hint:{key}"), rect.center());
    response.on_hover_text(text)
}

/// Wrapping explanatory text for a card's body (a rule or a caveat too long for the single-line `hint`).
/// Tokens: `font_small, text_dim, space_4`.
pub fn note(ui: &mut Ui, key: &str, text: &str) -> Response {
    let t = Tokens::get();
    let width = dimension(ui.available_width());
    let galley = ui.painter().layout(text.to_string(), egui::FontId::proportional(t.font_small), t.text_dim, width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, galley.size().y + t.space_4 * 2.0), egui::Sense::hover());
    ui.painter().galley(egui::pos2(rect.left(), rect.top() + t.space_4), galley, t.text_dim);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    scenario::publish_rect(&format!("dialog:note:{key}"), rect);
    response
}

/// Picked-item list entry with a remove action. Returns that action's Response.
/// Tokens: control_height, space_8, font_label, text and icon-button tokens.
/// Picked-item row with a remove action. Tokens: `control_height, space_8, font_label, text; icon-button tokens`.
pub fn list_row(ui: &mut Ui, key: &str, text: &str) -> Response {
    let t = Tokens::get();
    let (rect, _) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), row_height()), egui::Sense::hover());
    let remove = egui::Rect::from_min_max(egui::pos2((rect.right() - row_height()).max(rect.left()), rect.top()), rect.max);
    let label = egui::Rect::from_min_max(rect.min, egui::pos2((remove.left() - t.space_8).max(rect.left()), rect.bottom()));
    paint_text(ui, label, text, egui::FontId::proportional(t.font_label), t.text, false);
    let response = ui.push_id(key, |ui| super::icon_button_at(ui, remove, "close", "Remove")).inner;
    scenario::publish_rect(&format!("dialog:list:{key}"), rect);
    scenario::publish_handle(&format!("dialog:list:{key}:remove"), remove.center());
    response
}

/// Full-width rule; Tokens: dialog_rule. Use between groups without extra padding.
/// Full-width rule without extra padding. Tokens: `dialog_rule`.
pub fn divider(ui: &mut Ui, key: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, Tokens::get().dialog_rule));
    scenario::publish_rect(&format!("dialog:divider:{key}"), rect);
}

/// Give a compound row's subfield its accessible name without allocating layout space.
/// Name a subfield without allocating another row. Tokens: `none`.
pub(super) fn named_control<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(egui::UiBuilder::new().id_salt(label).ui_stack_info(egui::UiStackInfo::default().with_tag_value(LABEL, label.to_owned())), add)
        .inner
}

/// Publish geometry without replacing native accessibility. Tokens: `none`.
pub(super) fn publish_geometry(ui: &Ui, response: &Response) {
    let label = label(ui);
    scenario::publish_handle(&format!("dialog:{label}"), response.rect.center());
    scenario::publish_rect(&format!("dialog:widget:{label}"), response.rect);
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;

/// Read-only, wrapping report text. Tokens: font_label, text/error, space_4.
pub fn text_block(ui: &mut Ui, id: &str, text: &str, error: bool) -> Response {
    let t = Tokens::get();
    let response = ui
        .add(egui::Label::new(RichText::new(text).font(egui::FontId::proportional(t.font_label)).color(if error { t.error } else { t.text })).wrap());
    scenario::publish_rect(&format!("dialog:report:{id}"), response.rect);
    scenario::publish_handle(&format!("dialog:report:{id}"), response.rect.center());
    response
}

/// Inline read-only text beside actions. Tokens: font_label, text.
pub fn inline_text(ui: &mut Ui, text: &str) -> Response {
    let response =
        ui.add(egui::Label::new(RichText::new(text).font(egui::FontId::proportional(Tokens::get().font_label)).color(Tokens::get().text)).truncate());
    scenario::publish_rect(&format!("dialog:inline:{text}"), response.rect);
    scenario::publish_handle(&format!("dialog:inline:{text}"), response.rect.center());
    response
}

/// Large read-only licence/report with equal-height virtualized lines and horizontal overflow.
/// Tokens: font_small, text, control_height. Only visible lines are laid out.
pub fn text_lines(ui: &mut Ui, id: &str, text: &str) {
    let t = Tokens::get();
    let lines: Vec<_> = text.lines().collect();
    let font = egui::FontId::monospace(t.font_small);
    let height = ui.fonts_mut(|f| f.row_height(&font));
    let response = egui::ScrollArea::both().id_salt(id).max_height(t.control_height * 15.0).show_rows(ui, height, lines.len(), |ui, rows| {
        for line in lines.get(rows).unwrap_or_default() {
            ui.add(egui::Label::new(RichText::new(*line).font(font.clone()).color(t.text)).extend());
        }
    });
    scenario::publish_rect(&format!("dialog:lines:{id}"), response.inner_rect);
}
