//! Panel elements for dense cards; see `docs/ui-kit.md` for tokens and usage.
use super::{actions::icon_button_at, metrics::*, text::paint_text};
use crate::{scenario, theme::Tokens};
use egui::{Ui, vec2};

/// Feature title/icon/close band. Tokens: `form_header_height, form_icon_size, form_close_size, form_inset, font_label, text, dialog_rule, space_8/10; icon_button_at tokens`.
pub fn header(ui: &mut Ui, icon: &str, title: &str, on_close: impl FnOnce()) {
    header_impl(ui, icon, title, None, on_close);
}

/// Optional fold action for managers which historically hide their inputs without losing edits.
/// Tokens: header tokens and space_4; retains the legacy chevron accessibility identities.
pub fn collapsible_header(ui: &mut Ui, icon: &str, title: &str, collapsed: &mut bool, on_close: impl FnOnce()) {
    header_impl(ui, icon, title, Some(collapsed), on_close);
}

fn header_impl(ui: &mut Ui, icon: &str, title: &str, collapsed: Option<&mut bool>, on_close: impl FnOnce()) {
    let t = Tokens::get();
    let (rect, _) = ui.allocate_exact_size(vec2(dimension(ui.available_width()), header_height()), egui::Sense::hover());
    // Design: padding 0 8 0 10, an 18 pt icon, a 7 pt gap, a 22 pt close button.
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.space_10 + t.form_icon_size / 2.0, rect.center().y),
        vec2(t.form_icon_size, t.form_icon_size),
    );
    crate::icons::paint_icon(ui.painter(), icon_rect, icon, 1.0);
    let close_rect = egui::Rect::from_center_size(
        egui::pos2((rect.right() - t.space_8 - t.form_close_size / 2.0).max(rect.left()), rect.center().y),
        vec2(t.form_close_size, t.form_close_size),
    );
    let close = icon_button_at(ui, close_rect, "close", "Close");
    scenario::publish_handle("dialog:close", close.rect.center());
    if close.clicked() {
        on_close();
    }
    let right = if let Some(collapsed) = collapsed {
        let fold = egui::Rect::from_min_size(egui::pos2(close_rect.left() - t.form_close_size - t.space_4, close_rect.top()), close_rect.size());
        let response = icon_button_at(ui, fold, if *collapsed { "chevron" } else { "chevron_right" }, if *collapsed { "«" } else { "»" })
            .on_hover_text(crate::i18n::tr_label(if *collapsed { "Expand" } else { "Collapse" }));
        scenario::publish_handle("dialog:collapse", response.rect.center());
        if response.clicked() {
            *collapsed = !*collapsed;
        }
        fold.left()
    } else {
        close_rect.left()
    };
    let left = (icon_rect.right() + t.form_inset).min(right);
    let title_rect = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(right.max(left), rect.bottom()));
    paint_text(ui, title_rect, title, crate::fonts::semibold(t.font_label), t.text, false);
    let title_response = ui.interact(title_rect, ui.make_persistent_id("title_text"), egui::Sense::hover());
    title_response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), title));
    scenario::publish_rect("dialog:header", rect);
    ui.painter().hline(rect.x_range(), rect.bottom(), egui::Stroke::new(1.0, t.dialog_rule));
}

/// A bounded docked or floating card. Uses form_card_width and the caller's tokenised frame.
/// Window IDs and content scopes are retained so moving a card does not reset field focus.
pub(crate) struct Panel<'a> {
    title: &'a str,
    id: egui::Id,
    anchor: egui::Pos2,
    viewport: egui::Rect,
    side: crate::frame::Side,
    frame: egui::Frame,
    width: f32,
    ok: &'a str,
    cancel: &'a str,
}

pub(crate) struct FeatureOptions<'a> {
    pub icon: &'a str,
    pub title: &'a str,
    pub drag_key: &'a str,
    pub moved: egui::Vec2,
    pub docked: egui::Pos2,
    pub apply_label: Option<&'a str>,
}

/// State read after rendering the fields, just as in the original feature card.
pub(crate) struct ComputeStatus {
    pub text: Option<String>,
    pub busy: bool,
    pub applying: bool,
}

pub(crate) enum Part {
    Header,
    Body,
    Footer,
}

impl<'a> Panel<'a> {
    /// Feature surface; Tokens: dialog_body and shell_card/dock_frame's border/shadow tokens.
    pub(crate) fn feature_frame(ctx: &egui::Context, layout: crate::shell::LayoutMode, side: crate::frame::Side) -> egui::Frame {
        let t = Tokens::get();
        if layout == crate::shell::LayoutMode::Floating {
            crate::frame::shell_card(t.dialog_body).inner_margin(0.0)
        } else {
            crate::frame::dock_frame(ctx, side).fill(t.dialog_body).inner_margin(0.0)
        }
    }

    /// Palette surface; Tokens: panel, sketch_palette_alpha, radius_xl and shell/dock tokens.
    pub(crate) fn palette_frame(ctx: &egui::Context, layout: crate::shell::LayoutMode) -> egui::Frame {
        let t = Tokens::get();
        if layout == crate::shell::LayoutMode::Floating {
            crate::frame::shell_card(t.panel.gamma_multiply(t.sketch_palette_alpha)).corner_radius(t.radius_xl).inner_margin(0.0)
        } else {
            crate::frame::dock_frame(ctx, crate::frame::Side::Right).fill(t.panel.gamma_multiply(t.sketch_palette_alpha)).inner_margin(0.0)
        }
    }

    pub(crate) fn new(title: &'a str, id: egui::Id, anchor: egui::Pos2, viewport: egui::Rect, side: crate::frame::Side, frame: egui::Frame) -> Self {
        Self { title, id, anchor, viewport, side, frame, width: Tokens::get().form_card_width, ok: "OK", cancel: "Cancel" }
    }

    pub(crate) fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The footer's primary and closing actions when they are not OK and Cancel. An empty `ok`
    /// leaves the primary action out (a card that only closes, e.g. Measure).
    pub(crate) fn actions(mut self, ok: &'a str, cancel: &'a str) -> Self {
        (self.ok, self.cancel) = (ok, cancel);
        self
    }

    /// Table surface width derives from the same card-width token and remains viewport bounded.
    pub(crate) fn wide(self) -> Self {
        self.width(Tokens::get().form_card_width * 3.0)
    }

    /// The painted anchor also drives dragging, including the shell's edge clearance.
    fn bounded_anchor(&self) -> egui::Pos2 {
        let viewport = self.viewport;
        let width = self.width.min(viewport.width().max(0.0));
        let x = match self.side {
            crate::frame::Side::Right => {
                self.anchor.x.clamp(viewport.left() + width, (viewport.right() - Tokens::get().space_8).max(viewport.left() + width))
            }
            crate::frame::Side::Left => self.anchor.x.clamp(viewport.left(), viewport.right() - width),
        };
        egui::pos2(x, self.anchor.y.clamp(viewport.top(), viewport.bottom()))
    }

    pub(crate) fn window(&self) -> egui::Window<'a> {
        let gap = crate::shell::SHELL_GAP;
        let frame = self.frame;
        let viewport = self.viewport;
        let anchor = self.bounded_anchor();
        let side = self.side;
        let margin = frame.total_margin();
        let width = self.width.min(viewport.width().max(0.0));
        let inner_width = (width - margin.sum().x).max(0.0);
        let height = (viewport.bottom() - anchor.y - gap).max(0.0);
        // Keep drawing safe when even the header/footer cannot fit. egui clips the contents to
        // the remaining viewport; the normal small-height case still keeps both actions visible.
        egui::Window::new(self.title)
            .enabled(!crate::issue_report::cad_input_blocked())
            .id(self.id)
            .title_bar(false)
            .pivot(match side {
                crate::frame::Side::Right => egui::Align2::RIGHT_TOP,
                crate::frame::Side::Left => egui::Align2::LEFT_TOP,
            })
            .fixed_pos(anchor)
            .constrain(false)
            .collapsible(false)
            .resizable(false)
            .min_width(inner_width)
            .max_width(inner_width)
            .default_width(inner_width)
            .min_height(0.0)
            .max_height((height - margin.sum().y).max(0.0))
            .scroll(false)
            .frame(frame)
            .default_height(0.0)
            .fade_in(false)
            .fade_out(false)
            .order(egui::Order::Middle)
    }

    /// Feature header, scrolling padded body, pinned actions, drag target and compute status.
    /// Tokens: form_header_height, form_footer_height, space_6/8/10/14/16, dialog_body.
    pub(crate) fn feature(
        &self,
        ui: &mut Ui,
        heading: FeatureOptions<'_>,
        disabled: bool,
        valid: bool,
        body: impl FnOnce(&mut Ui) -> ComputeStatus,
    ) -> (bool, super::Action) {
        self.feature_impl(ui, heading, disabled, valid, None, body)
    }

    /// Same card with a persistent fold state owned by its caller.
    pub(crate) fn feature_collapsible(
        &self,
        ui: &mut Ui,
        heading: FeatureOptions<'_>,
        disabled: bool,
        valid: bool,
        collapsed: &mut bool,
        body: impl FnOnce(&mut Ui) -> ComputeStatus,
    ) -> (bool, super::Action) {
        self.feature_impl(ui, heading, disabled, valid, Some(collapsed), body)
    }

    fn feature_impl(
        &self,
        ui: &mut Ui,
        heading: FeatureOptions<'_>,
        disabled: bool,
        valid: bool,
        mut collapsed: Option<&mut bool>,
        body: impl FnOnce(&mut Ui) -> ComputeStatus,
    ) -> (bool, super::Action) {
        super::scope(ui, |ui| {
            let t = Tokens::get();
            let min = ui.cursor().min;
            let band = egui::Rect::from_min_size(min, vec2(ui.available_width(), super::header_height()));
            crate::frame::drag_dock(ui, band, heading.drag_key, heading.moved, self.bounded_anchor() - heading.docked);
            let mut close = false;
            if let Some(collapsed) = collapsed.as_deref_mut() {
                collapsible_header(ui, heading.icon, heading.title, collapsed, || close = true);
            } else {
                header(ui, heading.icon, heading.title, || close = true);
            }
            let bottom = self.viewport.bottom() - crate::shell::SHELL_GAP - self.frame.inner_margin.bottomf() - self.frame.stroke.width;
            let footer_height = t.form_footer_height;
            let fields_height = (bottom - ui.cursor().top() - footer_height).max(0.0);
            let fields_rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), fields_height));
            let state = if collapsed.as_deref().copied().unwrap_or(false) {
                ComputeStatus { text: None, busy: disabled, applying: disabled }
            } else {
                ui.scope_builder(egui::UiBuilder::new().max_rect(fields_rect), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("feature_fields")
                        .min_scrolled_height(0.0)
                        .max_height(fields_height)
                        .show(ui, |ui| {
                            if disabled {
                                ui.disable();
                            }
                            egui::Frame::NONE
                                .inner_margin(egui::Margin {
                                    left: t.space_10 as i8,
                                    right: t.space_10 as i8,
                                    top: t.space_8 as i8,
                                    bottom: t.space_6 as i8,
                                })
                                .show(ui, body)
                                .inner
                        })
                        .inner
                })
                .inner
            };
            let action = super::footer_with_status(ui, self.ok, self.cancel, valid && !state.applying, heading.apply_label, state.text.as_deref());
            if state.busy {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(ui.min_rect().left(), ui.min_rect().bottom() - super::row_height()),
                    vec2(t.space_16, super::row_height()),
                );
                egui::Spinner::new().size(t.space_14).paint_at(ui, rect);
                let status = state.text.as_deref().unwrap_or_else(|| crate::i18n::tr_label("Updating the preview"));
                ui.interact(rect, ui.id().with("compute_status"), egui::Sense::hover()).on_hover_text(status);
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
            (close, action)
        })
    }

    /// Palette composition retains its compact padding and whole-card scrolling on short views.
    /// Tokens: space_24 and form_footer_height; header/body/footer own their historical padding.
    pub(crate) fn palette(&self, ui: &mut Ui, bottom: f32, top: f32, mut add: impl FnMut(&mut Ui, Part)) {
        let t = Tokens::get();
        ui.set_max_height((bottom - ui.cursor().top()).max(0.0));
        super::scope(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let height = (bottom - top - self.frame.total_margin().sum().y).max(0.0);
            let short = height < t.space_24 * 6.0;
            let mut contents = |ui: &mut Ui| {
                add(ui, Part::Header);
                let body_height = (bottom - ui.cursor().top() - t.form_footer_height).max(0.0);
                let mut body = |ui: &mut Ui| {
                    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(t.space_10 as i8, t.space_4 as i8)).show(ui, |ui| add(ui, Part::Body));
                };
                if short {
                    body(ui);
                } else {
                    let body_rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), body_height));
                    let scroll = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("sc_palette_options")
                                .max_height(body_height)
                                .min_scrolled_height(0.0)
                                .auto_shrink([false, true])
                                .show(ui, body)
                        })
                        .inner;
                    scenario::publish_rect("sketch_palette:options", scroll.inner_rect);
                }
                add(ui, Part::Footer);
            };
            if short {
                let scroll = egui::ScrollArea::vertical()
                    .id_salt("sc_palette_content")
                    .max_height(height)
                    .min_scrolled_height(0.0)
                    .auto_shrink([false, true])
                    .show(ui, contents);
                scenario::publish_rect("sketch_palette:options", scroll.inner_rect);
            } else {
                contents(ui);
            }
        });
    }
}

/// A centred prompt or small form card, one and a half card widths wide: the feature header (icon, title, close), a padded body,
/// and the pinned footer with up to three actions (`apply` is the third, middle action, e.g.
/// "Don't Save"). Floats over the viewport instead of docking, for confirmations, offers and
/// short property forms. Tokens: `form_card_width` (x1.5), header/footer tokens, `dialog_body`,
/// `space_6/8/10`.
pub(crate) struct Prompt<'a> {
    id: egui::Id,
    icon: &'a str,
    title: &'a str,
    anchor: Option<(egui::Align2, egui::Vec2)>,
    placed: Option<(egui::Align2, egui::Pos2)>,
    ok: &'a str,
    cancel: &'a str,
    apply: Option<&'a str>,
    selected: Option<super::Action>,
    enter_submits: bool,
    escape_closes: bool,
    width: f32,
}

impl<'a> Prompt<'a> {
    /// A card with OK and Cancel, centred and movable.
    pub(crate) fn new(id: egui::Id, icon: &'a str, title: &'a str) -> Self {
        Self {
            id,
            icon,
            title,
            anchor: None,
            placed: None,
            ok: "OK",
            cancel: "Cancel",
            apply: None,
            selected: None,
            enter_submits: true,
            escape_closes: true,
            width: Tokens::get().form_card_width * 1.5,
        }
    }

    /// Pin the card at `align` of the screen, `offset` away (it then no longer moves).
    pub(crate) fn anchor(mut self, align: egui::Align2, offset: egui::Vec2) -> Self {
        self.anchor = Some((align, offset));
        self
    }

    /// Start with `pivot` of the card at `pos`; the card can then be moved.
    pub(crate) fn placed(mut self, pivot: egui::Align2, pos: egui::Pos2) -> Self {
        self.placed = Some((pivot, pos));
        self
    }

    /// The primary and closing actions, and an optional third one between them. An empty `ok`
    /// leaves the primary action out.
    pub(crate) fn actions(mut self, ok: &'a str, cancel: &'a str, apply: Option<&'a str>) -> Self {
        (self.ok, self.cancel, self.apply) = (ok, cancel, apply);
        self
    }

    /// A card of editable fields: Enter belongs to the fields and never submits (confirmations
    /// let Enter give the primary action when nothing has focus).
    pub(crate) fn fields(mut self) -> Self {
        self.enter_submits = false;
        self
    }

    /// Esc does not close the card (the header's close and the closing action still do): for a
    /// card holding a long text the user is editing, where Esc must not discard the work.
    pub(crate) fn keep_open_on_escape(mut self) -> Self {
        self.escape_closes = false;
        self
    }

    /// `cards` times the card width instead of one and a half, for wide footers.
    pub(crate) fn cards(mut self, cards: f32) -> Self {
        if cards.is_finite() && cards > 0.0 {
            self.width = Tokens::get().form_card_width * cards;
        }
        self
    }

    /// Mark one action as the one Enter gives.
    pub(crate) fn selected(mut self, action: super::Action) -> Self {
        self.selected = Some(action);
        self
    }

    /// Draw the card. Returns whether the header's close was pressed and the footer's action.
    /// `valid` enables the primary action.
    pub(crate) fn show(self, ctx: &egui::Context, valid: bool, body: impl FnOnce(&mut Ui)) -> (bool, super::Action) {
        let t = Tokens::get();
        let frame = crate::frame::shell_card(t.dialog_body).inner_margin(0.0);
        let inner_width = (self.width - frame.total_margin().sum().x).max(0.0);
        let window = egui::Window::new(self.title)
            .enabled(!crate::issue_report::cad_input_blocked())
            .id(self.id)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .min_width(inner_width)
            .max_width(inner_width)
            .default_width(inner_width)
            .scroll(false)
            .frame(frame)
            .fade_in(false)
            .fade_out(false);
        let window = match (self.anchor, self.placed) {
            (Some((align, offset)), _) => window.anchor(align, offset),
            (None, Some((pivot, pos))) => window.pivot(pivot).default_pos(pos),
            (None, None) => window.pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center() - vec2(0.0, row_height() * 2.5)),
        };
        let shown = window.show(ctx, |ui| {
            super::scope(ui, |ui| {
                let mut close = false;
                header(ui, self.icon, self.title, || close = true);
                egui::Frame::NONE
                    .inner_margin(egui::Margin { left: t.space_10 as i8, right: t.space_10 as i8, top: t.space_8 as i8, bottom: t.space_6 as i8 })
                    .show(ui, body);
                if !self.enter_submits {
                    ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                }
                if !self.escape_closes {
                    ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                }
                let action = super::footer_selected(ui, self.ok, self.cancel, valid, self.apply, self.selected);
                (close, action)
            })
        });
        if let Some(shown) = &shown {
            crate::scenario::publish_rect("dialog:window", shown.response.rect);
        }
        shown.and_then(|r| r.inner).unwrap_or((false, super::Action::None))
    }
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
