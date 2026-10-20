//! Metrics elements for dense cards; see `docs/ui-kit.md` for tokens and usage.
use super::layout::FORM;
use crate::theme::Tokens;
use egui::Ui;

/// Row height for controls. Tokens: `control_height`.
pub fn row_height() -> f32 {
    Tokens::get().control_height
}

/// Feature heading height. Tokens: `form_header_height`.
pub fn header_height() -> f32 {
    Tokens::get().form_header_height
}

/// Sanitize an input dimension. Tokens: `none`.
pub(super) fn dimension(value: f32) -> f32 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

/// A table column's header-width basis, as a multiple of `form_label_width`.
pub(super) const TABLE_COLUMN_BASIS: f32 = 3.2;

/// Share of a row given to the name in `label_actions`.
pub(super) const LABEL_ACTIONS_SHARE: f32 = 0.45;

/// Widest a label column may grow, as a multiple of `form_label_width`.
const LABEL_COLUMN_MAX: f32 = 1.6;

/// Measured width of a label in the form label font. Tokens: `font_label, dialog_label`.
fn natural_label_width(ui: &Ui, text: &str) -> f32 {
    let t = Tokens::get();
    ui.painter().layout_no_wrap(crate::i18n::tr_label(text).to_string(), egui::FontId::proportional(t.font_label), t.dialog_label).size().x.ceil()
}

/// Width of a table header column, measured directly: it never touches the form's shared label-column state.
/// Tokens: `form_label_width, font_label, dialog_label, space_4`.
pub(super) fn header_column(ui: &Ui, text: &str) -> f32 {
    let t = Tokens::get();
    let base = t.form_label_width;
    (natural_label_width(ui, text) + t.space_4).clamp(base, base * LABEL_COLUMN_MAX).min(base * TABLE_COLUMN_BASIS * 0.5)
}

/// Allocate exactly one control-height row. Long labels elide; controls cannot widen the card.
/// The design's label column is `form_label_width` wide; a dialog with longer labels widens the
/// column for all of its rows (up to 1.6 times that), so the controls still line up and labels
/// are not elided. The widest label of the last pass sizes the column, so it settles within
/// two frames and shrinks again when the long rows go away.
/// Shared label width, capped at 1.6x. Tokens: `form_label_width, font_label, dialog_label, space_4`.
pub(super) fn label_column(ui: &Ui, text: &str, row_width: f32) -> f32 {
    let t = Tokens::get();
    let base = t.form_label_width;
    let natural = natural_label_width(ui, text);
    let widest = match ui.stack().iter().find(|node| node.tags().contains(FORM)).map(|node| node.id) {
        Some(form) => {
            let key = form.with("label_column");
            let pass = ui.ctx().cumulative_pass_nr();
            let (seen, current, previous) = ui.ctx().data(|d| d.get_temp::<(u64, f32, f32)>(key)).unwrap_or((pass, 0.0, 0.0));
            let (current, previous) = if seen == pass {
                (current, previous)
            } else if seen + 1 == pass {
                (0.0, current)
            } else {
                (0.0, 0.0)
            };
            let current = current.max(natural);
            ui.ctx().data_mut(|d| d.insert_temp(key, (pass, current, previous)));
            current.max(previous)
        }
        None => natural,
    };
    (widest + t.space_4).clamp(base, base * LABEL_COLUMN_MAX).min(row_width * 0.5)
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
