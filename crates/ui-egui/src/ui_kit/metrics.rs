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

/// Allocate exactly one control-height row. Long labels elide; controls cannot widen the card.
/// The design's label column is `form_label_width` wide; a dialog with longer labels widens the
/// column for all of its rows (up to 1.6 times that), so the controls still line up and labels
/// are not elided. The widest label of the last pass sizes the column, so it settles within
/// two frames and shrinks again when the long rows go away.
/// Shared label width, capped at 1.6x. Tokens: `form_label_width, font_label, dialog_label, space_4`.
pub(super) fn label_column(ui: &Ui, text: &str, row_width: f32) -> f32 {
    let t = Tokens::get();
    let base = t.form_label_width;
    let natural = ui
        .painter()
        .layout_no_wrap(crate::i18n::tr_label(text).to_string(), egui::FontId::proportional(t.font_label), t.dialog_label)
        .size()
        .x
        .ceil();
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
    (widest + t.space_4).clamp(base, base * 1.6).min(row_width * 0.5)
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
