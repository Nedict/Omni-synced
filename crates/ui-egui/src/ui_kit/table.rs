//! Dense editable tables. Geometry and typography share the form label-column tokens.
use super::metrics::*;
use crate::{scenario, theme::Tokens};
use egui::{Response, Ui, vec2};

/// A column's minimum width is a multiple of the measured form label column.
pub(crate) struct Column<'a> {
    label: &'a str,
    width: f32,
    min_content_width: f32,
}

impl<'a> Column<'a> {
    pub(crate) fn new(label: &'a str, width: f32) -> Self {
        Self { label, width, min_content_width: 0.0 }
    }

    /// Keep enough room for the measured cell controls, even in a narrow scrolling card.
    pub(crate) fn min_content_width(mut self, width: f32) -> Self {
        self.min_content_width = dimension(width);
        self
    }
}

pub(crate) struct Table<'a> {
    columns: &'a [Column<'a>],
    widths: Vec<f32>,
    id: String,
    rows: usize,
}

/// Header, horizontally scrolling body and empty state. Rows are exactly control_height.
/// Tokens: form_label_width, font_label, dialog_label, space_4/8, dialog_rule, text/dim, selected_bg.
pub(crate) fn table<R>(ui: &mut Ui, id: &str, columns: &[Column<'_>], empty: &str, add: impl FnOnce(&mut Ui, &mut Table<'_>) -> R) -> R {
    let t = Tokens::get();
    let widths: Vec<_> =
        columns.iter().map(|c| (header_column(ui, c.label) * dimension(c.width)).max(label_width(ui, c.label)).max(c.min_content_width)).collect();
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
                ui.scope_builder(egui::UiBuilder::new().max_rect(cell).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                    ui.set_clip_rect(ui.clip_rect().intersect(cell));
                    ui.add(
                        egui::Label::new(egui::RichText::new(crate::i18n::tr_label(column.label)).size(t.font_label).color(t.text_dim)).truncate(),
                    )
                    .on_hover_text(crate::i18n::tr_label(column.label));
                });
                scenario::publish_rect(&format!("dialog:column:{id}:{}", column.label), cell);
                left = cell.right() + t.space_8;
            }
            ui.painter().hline(rect.x_range(), rect.bottom(), egui::Stroke::new(1.0, t.dialog_rule));
            let result = add(ui, &mut table);
            if table.rows == 0 {
                super::text(ui, empty, super::Text::Note(id));
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
    pub(crate) fn row(&mut self, ui: &mut Ui, key: &str, selected: bool, add: impl FnOnce(&mut TableRow<'_>)) -> Response {
        self.row_visible(ui, key, selected, true, add)
    }

    /// Keep off-screen rows' space without building their cells. Force active drafts to stay live.
    pub(crate) fn row_visible(&mut self, ui: &mut Ui, key: &str, selected: bool, force: bool, add: impl FnOnce(&mut TableRow<'_>)) -> Response {
        let response = self.row_content(ui, key, Some(selected), force, add);
        self.rows += 1;
        response
    }

    /// A non-selectable column-action band attached to the header, using the same cell grid.
    pub(crate) fn column_actions(&mut self, ui: &mut Ui, add: impl FnOnce(&mut TableRow<'_>)) {
        self.row_content(ui, "columns", None, true, add);
    }

    fn row_content(&self, ui: &mut Ui, key: &str, selected: Option<bool>, force: bool, add: impl FnOnce(&mut TableRow<'_>)) -> Response {
        let t = Tokens::get();
        let sense = if selected.is_some() { egui::Sense::click() } else { egui::Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(self.width(), row_height()), sense);
        if !force && !ui.is_rect_visible(rect) {
            return response;
        }
        if let Some(selected) = selected {
            if selected || response.hovered() {
                ui.painter().rect_filled(rect, t.control_radius, if selected { t.selected_bg } else { t.hover });
            }
            response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, key));
        } else {
            ui.painter().rect_filled(rect, 0.0, t.dialog_footer);
            ui.painter().hline(rect.x_range(), rect.bottom(), egui::Stroke::new(1.0, t.dialog_rule));
        }
        scenario::publish_rect(&format!("dialog:table:{}:{key}", self.id), rect);
        ui.push_id((&self.id, key), |ui| {
            add(&mut TableRow {
                ui,
                widths: &self.widths,
                columns: self.columns,
                rect,
                column: 0,
                left: rect.left(),
                id: format!("dialog:table:{}:{key}", self.id),
            });
        });
        response
    }
}

pub(crate) struct TableRow<'a> {
    ui: &'a mut Ui,
    widths: &'a [f32],
    columns: &'a [Column<'a>],
    rect: egui::Rect,
    column: usize,
    left: f32,
    id: String,
}

impl TableRow<'_> {
    /// Place an inline field, text, choice or row actions in the next clipped cell.
    pub(crate) fn cell<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        let width = self.widths.get(self.column).copied().unwrap_or(0.0);
        let label = self.columns.get(self.column).map_or("", |c| c.label);
        let rect = egui::Rect::from_min_size(egui::pos2(self.left, self.rect.top()), vec2(width, self.rect.height()));
        scenario::publish_rect(&format!("{}:cell:{label}", self.id), rect);
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

/// Clipped row name beside compact actions. Tokens: control_height and scope font/spacing.
pub(crate) fn label_actions(ui: &mut Ui, text: &str, add: impl FnOnce(&mut Ui)) {
    let width = dimension(ui.available_width());
    ui.allocate_ui_with_layout(vec2(width * LABEL_ACTIONS_SHARE, row_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        super::text(ui, text, super::Text::Cell);
    });
    add(ui);
}
