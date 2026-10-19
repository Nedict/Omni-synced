//! Dense editable tables. Geometry and typography share the form label-column tokens.
use super::{metrics::*, text::paint_text};
use crate::{scenario, theme::Tokens};
use egui::{Response, Ui, vec2};

/// A column's minimum width is a multiple of the measured form label column.
pub struct Column<'a> {
    pub label: &'a str,
    pub width: f32,
}

pub struct Table<'a> {
    columns: &'a [Column<'a>],
    widths: Vec<f32>,
    id: String,
    rows: usize,
}

/// Header, horizontally scrolling body and empty state. Rows are exactly control_height.
/// Tokens: form_label_width, font_label, dialog_label, space_4/8, dialog_rule, text/dim, selected_bg.
pub fn table<R>(ui: &mut Ui, id: &str, columns: &[Column<'_>], empty: &str, add: impl FnOnce(&mut Ui, &mut Table<'_>) -> R) -> R {
    let t = Tokens::get();
    let widths: Vec<_> = columns.iter().map(|c| header_column(ui, c.label) * dimension(c.width)).collect();
    let mut table = Table { columns, widths, id: id.to_owned(), rows: 0 };
    egui::ScrollArea::horizontal()
        .id_salt(id)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let width = table.width();
            let (rect, _) = ui.allocate_exact_size(vec2(width, row_height()), egui::Sense::hover());
            let mut left = rect.left();
            for (column, width) in table.columns.iter().zip(&table.widths) {
                let cell = egui::Rect::from_min_size(egui::pos2(left, rect.top()), vec2(*width, rect.height()));
                let response = ui.put(cell, egui::Label::new(egui::RichText::new(crate::i18n::tr_label(column.label)).color(t.text_dim)).truncate());
                response.on_hover_text(crate::i18n::tr_label(column.label));
                scenario::publish_rect(&format!("dialog:column:{id}:{}", column.label), cell);
                left = cell.right() + t.space_8;
            }
            ui.painter().hline(rect.x_range(), rect.bottom(), egui::Stroke::new(1.0, t.dialog_rule));
            let result = add(ui, &mut table);
            if table.rows == 0 {
                super::note(ui, id, empty);
            }
            result
        })
        .inner
}

impl Table<'_> {
    fn width(&self) -> f32 {
        self.widths.iter().sum::<f32>() + Tokens::get().space_8 * self.widths.len().saturating_sub(1) as f32
    }

    /// A selectable row with independently interactive cells and actions.
    pub fn row(&mut self, ui: &mut Ui, key: &str, selected: bool, add: impl FnOnce(&mut TableRow<'_>)) -> Response {
        self.row_visible(ui, key, selected, true, add)
    }

    /// Keep off-screen rows' space without building their cells. Force active drafts to stay live.
    pub fn row_visible(&mut self, ui: &mut Ui, key: &str, selected: bool, force: bool, add: impl FnOnce(&mut TableRow<'_>)) -> Response {
        let t = Tokens::get();
        let (rect, response) = ui.allocate_exact_size(vec2(self.width(), row_height()), egui::Sense::click());
        self.rows += 1;
        if !force && !ui.is_rect_visible(rect) {
            return response;
        }
        if selected || response.hovered() {
            ui.painter().rect_filled(rect, t.control_radius, if selected { t.selected_bg } else { t.hover });
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, key));
        scenario::publish_rect(&format!("dialog:table:{}:{key}", self.id), rect);
        ui.push_id((&self.id, key), |ui| {
            add(&mut TableRow { ui, widths: &self.widths, columns: self.columns, rect, column: 0, left: rect.left() });
        });
        response
    }
}

pub struct TableRow<'a> {
    ui: &'a mut Ui,
    widths: &'a [f32],
    columns: &'a [Column<'a>],
    rect: egui::Rect,
    column: usize,
    left: f32,
}

impl TableRow<'_> {
    /// Place an inline field, text, choice or row actions in the next clipped cell.
    pub fn cell<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        let width = self.widths.get(self.column).copied().unwrap_or(0.0);
        let label = self.columns.get(self.column).map_or("", |c| c.label);
        let rect = egui::Rect::from_min_size(egui::pos2(self.left, self.rect.top()), vec2(width, self.rect.height()));
        self.column += 1;
        self.left += width + Tokens::get().space_8;
        self.ui
            .scope_builder(
                egui::UiBuilder::new()
                    .id_salt(self.column)
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center))
                    .ui_stack_info(egui::UiStackInfo::default().with_tag_value(super::layout::LABEL, label.to_owned())),
                |ui| {
                    ui.set_clip_rect(ui.clip_rect().intersect(rect));
                    ui.spacing_mut().text_edit_width = width;
                    add(ui)
                },
            )
            .inner
    }
}

/// Read-only clipped cell retaining a full tooltip and accessible label. Tokens: font_label, text.
pub fn cell_text(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get();
    let (rect, response) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), row_height()), egui::Sense::click());
    paint_text(ui, rect, text, egui::FontId::proportional(t.font_label), t.text, false);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    scenario::publish_rect(&format!("dialog:cell:{text}"), rect);
    scenario::publish_handle(&format!("dialog:cell:{text}"), rect.center());
    response.on_hover_text(text)
}

/// Clipped row name beside compact actions. Tokens: control_height and scope font/spacing.
pub fn label_actions(ui: &mut Ui, text: &str, add: impl FnOnce(&mut Ui)) {
    let width = dimension(ui.available_width());
    ui.allocate_ui_with_layout(vec2(width * LABEL_ACTIONS_SHARE, row_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        cell_text(ui, text);
    });
    add(ui);
}
