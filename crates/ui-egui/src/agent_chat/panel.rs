//! The agent chat panel: docked left or right (switched by dragging its header to the other side,
//! or in Preferences), resizable, folded to a thin strip with Minimise, closed with ×. Its side,
//! width and fold are preferences, so they come back next time.

use egui::{Align, Align2, Color32, Id, Layout, Rect, RichText, Sense, Stroke, pos2, vec2};

use super::*;
use crate::theme::Tokens;

pub const MIN_WIDTH: f32 = 280.0;
pub const MAX_WIDTH: f32 = 760.0;
/// The folded panel's width.
const STRIP: f32 = 30.0;
/// The keyboard shortcut that shows and hides the panel (also from its own text field).
pub const SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::A);

fn panel_id(left: bool) -> Id {
    Id::new(if left { "sc_agent_chat_left" } else { "sc_agent_chat_right" })
}

/// Forget the panel's own idea of its width, so the preference applies on the next frame.
pub fn forget_width(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        for left in [true, false] {
            d.remove::<egui::containers::panel::PanelState>(panel_id(left));
        }
    });
}

/// Show or hide the panel (a folded panel unfolds first).
pub fn toggle(app: &mut SolveApp) {
    let c = &mut app.agent_chat;
    if !c.open {
        c.open = true;
        app.preferences.agent_chat_minimized = false;
    } else if app.preferences.agent_chat_minimized {
        app.preferences.agent_chat_minimized = false;
    } else {
        c.open = false;
    }
}

/// The tooltip of the chat's buttons: its name and shortcut.
pub fn tip() -> String {
    let key = if cfg!(target_os = "macos") { "Cmd+Shift+A" } else { "Ctrl+Shift+A" };
    format!("{} ({key})", tr("ui.agent_chat"))
}

/// The panel, when the chat is turned on and open. Call before the central viewport is laid out.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    poll(app, &ctx);
    if !app.preferences.agent_chat_enabled {
        return;
    }
    if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT)) {
        toggle(app);
    }
    if !app.agent_chat.open {
        return;
    }
    let t = Tokens::get();
    let left = app.preferences.agent_chat_side == "left";
    let side = |id: Id| if left { egui::Panel::left(id) } else { egui::Panel::right(id) };
    if app.preferences.agent_chat_minimized {
        side(Id::new("sc_agent_chat_strip"))
            .exact_size(STRIP)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(t.panel))
            .show(ui, |ui| strip(app, ui));
        return;
    }
    let width = app.preferences.agent_chat_width.clamp(MIN_WIDTH, MAX_WIDTH);
    let r = side(panel_id(left))
        .default_size(width)
        .size_range(MIN_WIDTH..=MAX_WIDTH)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| body(app, ui, left));
    crate::scenario::publish_rect("agent_chat:panel", r.response.rect);
    let w = r.response.rect.width();
    if w.is_finite() && (MIN_WIDTH..=MAX_WIDTH).contains(&w) {
        app.preferences.agent_chat_width = w;
    }
}

/// A small drawn button: `kind` is `minimize`, `close` or `expand`.
fn glyph_button(ui: &mut egui::Ui, kind: &str, tip: &str) -> egui::Response {
    let t = Tokens::get();
    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, t.hover);
    }
    let c = r.center();
    let s = Stroke::new(1.4, t.text);
    match kind {
        "minimize" => {
            ui.painter().line_segment([c + vec2(-5.0, 4.0), c + vec2(5.0, 4.0)], s);
        }
        "close" => {
            ui.painter().line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], s);
            ui.painter().line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], s);
        }
        _ => {}
    }
    crate::scenario::publish_handle(&format!("agent_chat:{kind}"), r.center());
    resp.on_hover_text(tip)
}

fn state_color(state: State) -> Color32 {
    let t = Tokens::get();
    match state {
        State::Ready | State::Busy => t.accent,
        State::NeedsAuth | State::Exited => t.warning,
        _ => t.text_dim,
    }
}

/// The folded panel: the chat icon (unfolds it), the agent's state, and a mark while something
/// waits for the user.
fn strip(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let r = ui.max_rect();
    let button = Rect::from_center_size(pos2(r.center().x, r.top() + 22.0), vec2(26.0, 26.0));
    let resp = ui.interact(button, Id::new("sc_agent_chat_unfold"), Sense::click());
    crate::scenario::publish_handle("agent_chat:unfold", button.center());
    if resp.hovered() {
        ui.painter().rect_filled(button, 4.0, t.hover);
    }
    crate::icons::paint(ui.painter(), button.shrink(4.0), "agent_chat", t.icon, t.icon_fill, t.accent);
    let state = app.agent_chat.state();
    ui.painter().circle_filled(pos2(r.center().x, button.bottom() + 10.0), 3.5, state_color(state));
    let waiting = app.agent_chat.waiting();
    if waiting > 0 {
        let at = button.right_top() + vec2(-3.0, 3.0);
        ui.painter().circle_filled(at, 6.5, t.warning);
        ui.painter().text(at, Align2::CENTER_CENTER, waiting.min(9).to_string(), egui::FontId::proportional(9.5), Color32::BLACK);
    }
    ui.painter().line_segment(
        if app.preferences.agent_chat_side == "left" { [r.right_top(), r.right_bottom()] } else { [r.left_top(), r.left_bottom()] },
        Stroke::new(1.0, t.border),
    );
    let tip = format!("{} · {}", tip(), state.label());
    if resp.on_hover_text(tip).clicked() {
        app.preferences.agent_chat_minimized = false;
    }
}

/// The header: a grip to drag the panel to the other side, the title and state, Connect or Stop,
/// Minimise and Close.
fn header(app: &mut SolveApp, ui: &mut egui::Ui, left: bool) {
    let t = Tokens::get();
    let state = app.agent_chat.state();
    ui.horizontal(|ui| {
        let title = {
            let (grip, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
            for row in 0..3 {
                for col in 0..2 {
                    ui.painter().circle_filled(grip.left_top() + vec2(2.5 + col as f32 * 5.0, 4.0 + row as f32 * 5.0), 1.1, t.text_dim);
                }
            }
            let l = ui.label(RichText::new(tr("ui.agent_chat")).strong().size(14.0));
            grip.union(l.rect)
        };
        let mut s = state.label().to_string();
        if let Some(turn) = &app.agent_chat.turn
            && state == State::Busy
        {
            let limit = app.preferences.agent_chat_turn_limit;
            s = format!("{s} {}", trf("ui.agent_chat_changes_of", &[("n", &turn.changes.to_string()), ("limit", &limit.to_string())]));
        }
        ui.label(RichText::new(s).color(state_color(state)).small());
        // Drag the header across the window to dock on the other side.
        let drag = ui.interact(title, Id::new("sc_agent_chat_grip"), Sense::drag());
        crate::scenario::publish_handle("agent_chat:grip", title.center());
        dock_drag(app, ui, &drag, left);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if glyph_button(ui, "close", tr("ui.agent_chat_close")).clicked() {
                app.agent_chat.open = false;
            }
            if glyph_button(ui, "minimize", tr("ui.agent_chat_minimize")).clicked() {
                app.preferences.agent_chat_minimized = true;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let b = if state.running() {
                    ui.button(tr("ui.agent_chat_stop")).on_hover_text(tr("ui.agent_chat_stop_tip"))
                } else {
                    ui.button(tr("ui.agent_chat_connect"))
                };
                crate::scenario::publish_handle("agent_chat:connect", b.rect.center());
                if b.clicked() {
                    if state.running() {
                        disconnect(app);
                    } else {
                        connect(app, ui.ctx());
                    }
                }
            }
        });
    });
}

/// While the header is dragged: the cursor grabs and the side it would dock on lights up; on
/// release over the other half of the window the panel moves there.
fn dock_drag(app: &mut SolveApp, ui: &egui::Ui, drag: &egui::Response, left: bool) {
    if drag.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    let screen = ui.ctx().content_rect();
    let target = |x: f32| x < screen.center().x;
    if drag.dragged()
        && let Some(p) = ui.ctx().pointer_interact_pos()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let to_left = target(p.x);
        let w = app.preferences.agent_chat_width.clamp(MIN_WIDTH, MAX_WIDTH);
        let top = screen.top() + crate::toolbar::APP_BAR_HEIGHT;
        let zone = if to_left {
            Rect::from_min_max(pos2(screen.left(), top), pos2(screen.left() + w, screen.bottom()))
        } else {
            Rect::from_min_max(pos2(screen.right() - w, top), pos2(screen.right(), screen.bottom()))
        };
        let t = Tokens::get();
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("sc_agent_chat_dock_hint")));
        painter.rect(zone.shrink(4.0), 8.0, t.accent.gamma_multiply(0.18), Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
        painter.text(zone.center(), Align2::CENTER_CENTER, tr(if to_left { "ui.agent_chat_dock_left" } else { "ui.agent_chat_dock_right" }), egui::FontId::proportional(14.0), t.text);
    }
    if drag.drag_stopped()
        && let Some(p) = ui.ctx().pointer_interact_pos().or_else(|| ui.ctx().pointer_latest_pos())
        && target(p.x) != left
    {
        app.preferences.agent_chat_side = if left { "right" } else { "left" }.into();
    }
}

fn body(app: &mut SolveApp, ui: &mut egui::Ui, left: bool) {
    header(app, ui, left);
    #[cfg(target_arch = "wasm32")]
    {
        ui.label(tr("ui.agent_chat_desktop_only"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    desktop_body(app, ui);
}

#[cfg(not(target_arch = "wasm32"))]
fn desktop_body(app: &mut SolveApp, ui: &mut egui::Ui) {
    if app.agent_chat.detection.is_none() && app.agent_chat.detecting.is_none() {
        refresh(app, ui.ctx());
    }
    let state = app.agent_chat.state();
    picker(app, ui);
    if state == State::NeedsAuth {
        auth_box(app, ui);
    }
    ui.separator();
    // The input sits at the bottom; the conversation takes the rest.
    let input_h = 92.0;
    let avail = (ui.available_height() - input_h).max(80.0);
    egui::ScrollArea::vertical().id_salt("sc_agent_chat_scroll").max_height(avail).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        conversation(app, ui);
    });
    ui.separator();
    let busy = state == State::Busy;
    let edit = egui::TextEdit::multiline(&mut app.agent_chat.input)
        .hint_text(tr("ui.agent_chat_placeholder"))
        .desired_rows(2)
        .desired_width(f32::INFINITY)
        .id_salt("sc_agent_chat_input");
    let r = ui.add(edit);
    crate::scenario::publish_rect("agent_chat:input", r.rect);
    let enter = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
    ui.horizontal(|ui| {
        let send_b = ui.add_enabled(!busy, egui::Button::new(tr("ui.agent_chat_send")));
        crate::scenario::publish_handle("agent_chat:send", send_b.rect.center());
        if (send_b.clicked() || (enter && !busy)) && !app.agent_chat.input.trim().is_empty() {
            let text = std::mem::take(&mut app.agent_chat.input).trim_end_matches('\n').to_string();
            send(app, ui.ctx(), text);
        }
        if busy && ui.button(tr("ui.agent_chat_cancel")).clicked() {
            cancel(app);
        }
        // A picture of the view goes with the message to agents that take pictures.
        let images = app.agent_chat.images || !state.running();
        let attach = ui
            .add_enabled(images, egui::Checkbox::new(&mut app.agent_chat.attach_view, tr("ui.agent_chat_attach_view")))
            .on_hover_text(tr("ui.agent_chat_attach_view_tip"))
            .on_disabled_hover_text(tr("ui.agent_chat_no_images"));
        crate::scenario::publish_handle("agent_chat:attach", attach.rect.center());
    });
}

/// A found agent as one line: its command line (what the preference holds).
#[cfg(not(target_arch = "wasm32"))]
fn line(f: &discover::Found) -> String {
    f.argv.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
}

#[cfg(not(target_arch = "wasm32"))]
fn login_chip(ui: &mut egui::Ui, login: discover::Login) {
    let t = Tokens::get();
    let (text, color) = match login {
        discover::Login::LooksLoggedIn => (tr("ui.agent_chat_signed_in"), t.accent),
        discover::Login::KeyInEnvironment => (tr("ui.agent_chat_key_in_env"), t.accent),
        discover::Login::NotLoggedIn => (tr("ui.agent_chat_not_signed_in"), t.warning),
        discover::Login::Unknown => return,
    };
    egui::Frame::NONE.stroke(Stroke::new(1.0, color)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(6, 1)).show(ui, |ui| {
        ui.label(RichText::new(text).color(color).size(11.0));
    });
}

/// The agent picker: the agents found (version, whether they look signed in), a custom command,
/// Refresh, and install hints for the ones missing. Folds to one line once an agent runs.
#[cfg(not(target_arch = "wasm32"))]
fn picker(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let current = app.preferences.agent_command.clone();
    let found: Vec<discover::Found> = app.agent_chat.detection.as_ref().map(|d| d.found.clone()).unwrap_or_default();
    // The agent the chat would start: the chosen one, else the first found.
    let chosen = if current.trim().is_empty() { found.first().map(line).unwrap_or_default() } else { current.clone() };
    let open = app.agent_chat.picking.unwrap_or(!app.agent_chat.state().running());
    if !open {
        ui.horizontal(|ui| {
            let name = found.iter().find(|f| line(f) == chosen).map(|f| match &f.version {
                Some(v) => format!("{} {v}", f.name),
                None => f.name.to_string(),
            });
            let mut s = if app.agent_chat.agent.is_empty() { name.unwrap_or_else(|| short_command(&chosen)) } else { app.agent_chat.agent.clone() };
            if let Some(a) = &app.agent_chat.auth_status {
                s = format!("{s} · {a}");
            }
            ui.label(RichText::new(s).color(t.text_dim));
            let b = ui.link(tr("ui.agent_chat_change"));
            crate::scenario::publish_handle("agent_chat:change", b.rect.center());
            if b.clicked() {
                app.agent_chat.picking = Some(true);
            }
        });
        return;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("ui.agent_chat_pick")).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let refreshing = app.agent_chat.detecting.is_some();
            if ui.add_enabled(!refreshing, egui::Button::new(tr("ui.agent_chat_refresh")).small()).clicked() {
                refresh(app, ui.ctx());
            }
            if app.agent_chat.state().running() && ui.small_button(tr("ui.agent_chat_done")).clicked() {
                app.agent_chat.picking = Some(false);
            }
        });
    });
    if app.agent_chat.detecting.is_some() {
        ui.label(RichText::new(tr("ui.agent_chat_detecting")).color(t.text_dim));
    }
    for f in &found {
        let l = line(f);
        let selected = l == chosen && !app.agent_chat.custom;
        let fill = if selected { t.accent_soft } else { Color32::TRANSPARENT };
        let r = egui::Frame::NONE.fill(fill).corner_radius(5.0).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let _ = ui.radio(selected, "");
                ui.label(RichText::new(f.name).strong().color(t.text));
                if let Some(v) = &f.version {
                    ui.label(RichText::new(v).color(t.text_dim).size(11.0));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| login_chip(ui, f.login));
            });
        });
        let resp = ui.interact(r.response.rect, Id::new(("sc_agent_pick", f.id)), Sense::click());
        crate::scenario::publish_handle(&format!("agent_chat:pick:{}", f.id), resp.rect.center());
        if resp.on_hover_text(&l).clicked() {
            app.preferences.agent_command = l;
            app.agent_chat.custom = false;
        }
    }
    let custom_selected = app.agent_chat.custom || (!current.trim().is_empty() && !found.iter().any(|f| line(f) == current));
    ui.horizontal(|ui| {
        if ui.radio(custom_selected, tr("ui.agent_chat_add_custom")).clicked() {
            app.agent_chat.custom = true;
            if app.agent_chat.custom_text.is_empty() {
                app.agent_chat.custom_text = current.clone();
            }
        }
    });
    if custom_selected {
        if app.agent_chat.custom_text.is_empty() && !app.agent_chat.custom {
            app.agent_chat.custom_text = current.clone();
        }
        let r = ui.add(egui::TextEdit::singleline(&mut app.agent_chat.custom_text).desired_width(f32::INFINITY).hint_text("my-agent --acp"));
        if r.lost_focus() || r.changed() {
            app.preferences.agent_command = app.agent_chat.custom_text.trim().to_string();
        }
        ui.label(RichText::new(tr("ui.agent_chat_custom_hint")).color(t.text_dim).size(11.0));
    }
    let missing: Vec<discover::Missing> = app.agent_chat.detection.as_ref().map(|d| d.missing.clone()).unwrap_or_default();
    if !missing.is_empty() {
        let heading = if found.is_empty() { tr("ui.agent_chat_none_found") } else { tr("ui.agent_chat_not_installed") };
        egui::CollapsingHeader::new(RichText::new(heading).color(t.text_dim)).id_salt("sc_agent_missing").default_open(found.is_empty()).show(ui, |ui| {
            for m in missing {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(m.name).strong());
                    if ui.small_button(tr("ui.agent_chat_copy")).on_hover_text(m.install).clicked() {
                        ui.ctx().copy_text(m.install.to_string());
                    }
                });
                ui.add(egui::Label::new(RichText::new(m.install).monospace().size(11.0).color(t.text_dim)).selectable(true).wrap());
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn auth_box(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    egui::Frame::group(ui.style()).stroke(Stroke::new(1.0, t.warning)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(tr("ui.agent_chat_auth_title")).strong());
        ui.label(RichText::new(tr("ui.agent_chat_auth_needed")).color(t.text_dim));
        let methods = app.agent_chat.auth.clone();
        let base = app.agent_chat.argv.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
        if methods.is_empty() {
            ui.label(tr("ui.agent_chat_auth_none"));
        }
        for m in methods {
            ui.add_space(4.0);
            let desc = m.description.trim();
            ui.label(RichText::new(if desc.is_empty() || desc == m.name { m.name.clone() } else { format!("{} — {desc}", m.name) }).strong());
            match &m.kind {
                acp::AuthKind::Terminal { args, .. } => {
                    ui.label(tr("ui.agent_chat_auth_terminal"));
                    let cmd = format!("{base} {}", args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "));
                    ui.horizontal_wrapped(|ui| {
                        egui::Frame::NONE.fill(t.toolbar).corner_radius(4.0).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(&cmd).monospace().size(11.0)).selectable(true).wrap());
                        });
                        if ui.small_button(tr("ui.agent_chat_copy")).clicked() {
                            ui.ctx().copy_text(cmd.clone());
                        }
                    });
                }
                acp::AuthKind::Agent => {
                    let b = ui.button(trf("ui.agent_chat_auth_agent", &[("name", &m.name)]));
                    crate::scenario::publish_handle(&format!("agent_chat:auth:{}", m.id), b.rect.center());
                    if b.clicked() {
                        app.agent_chat.authenticate(&m.id);
                    }
                }
                acp::AuthKind::EnvVar { vars } => {
                    ui.label(trf("ui.agent_chat_auth_env", &[("vars", &vars.join(", "))]));
                }
            }
        }
        ui.add_space(4.0);
        if ui.button(tr("ui.agent_chat_auth_retry")).clicked() {
            connect(app, ui.ctx());
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn conversation(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let mut answer: Option<(usize, Option<String>)> = None;
    let mut approval: Option<(u64, bool)> = None;
    let mut undo: Option<usize> = None;
    let current_doc = app.session.doc.clone();
    for (n, item) in app.agent_chat.items.iter().enumerate() {
        match item {
            Item::User { text, image } => {
                ui.add_space(6.0);
                egui::Frame::NONE.fill(t.accent_soft).corner_radius(6.0).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(text).color(t.text)).selectable(true).wrap());
                    if *image {
                        ui.label(RichText::new(tr("ui.agent_chat_view_attached")).color(t.text_dim).size(11.0));
                    }
                });
            }
            Item::Agent { thought, text } => {
                let rt = if *thought { RichText::new(text).italics().color(t.text_dim) } else { RichText::new(text).color(t.text) };
                ui.add(egui::Label::new(rt).selectable(true).wrap());
            }
            Item::Tool { title, kind, status, output, .. } => {
                let (color, filled) = match status {
                    Some(ToolStatus::Completed) => (t.accent, true),
                    Some(ToolStatus::Failed) => (t.error, true),
                    Some(ToolStatus::InProgress) => (t.warning, true),
                    _ => (t.text_dim, false),
                };
                let head = if kind.is_empty() || kind == "other" { title.clone() } else { format!("{title}  [{kind}]") };
                ui.horizontal(|ui| {
                    status_dot(ui, color, filled);
                    egui::CollapsingHeader::new(RichText::new(head).color(t.text).monospace().size(11.5)).id_salt(("sc_agent_tool", n)).show(ui, |ui| {
                        if let Some(o) = output {
                            ui.add(egui::Label::new(RichText::new(o).monospace().size(11.0)).selectable(true).wrap());
                        }
                    });
                });
            }
            Item::Plan(entries) => {
                for (c, s) in entries {
                    let (color, filled) = match s.as_str() {
                        "completed" => (t.accent, true),
                        "in_progress" => (t.warning, true),
                        _ => (t.text_dim, false),
                    };
                    ui.horizontal(|ui| {
                        status_dot(ui, color, filled);
                        ui.label(RichText::new(c).color(t.text_dim));
                    });
                }
            }
            Item::Permission { title, options, answer: given, .. } => {
                egui::Frame::group(ui.style()).stroke(Stroke::new(1.0, t.accent)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(trf("ui.agent_chat_permission", &[("title", title)])).strong());
                    match given {
                        Some(a) => {
                            ui.label(RichText::new(trf("ui.agent_chat_answered", &[("option", a)])).color(t.text_dim));
                        }
                        None => {
                            ui.horizontal_wrapped(|ui| {
                                for o in options {
                                    let b = egui::Button::new(&o.name);
                                    let b = if o.kind.starts_with("allow") { b.fill(t.accent_soft) } else { b };
                                    let r = ui.add(b);
                                    crate::scenario::publish_handle(&format!("agent_chat:permission:{}", o.id), r.rect.center());
                                    if r.clicked() {
                                        answer = Some((n, Some(o.id.clone())));
                                    }
                                }
                            });
                        }
                    }
                });
            }
            Item::Approval { id, title, answer: given } => {
                egui::Frame::group(ui.style()).stroke(Stroke::new(1.5, t.warning)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(tr("ui.agent_chat_approval")).strong());
                    ui.add(egui::Label::new(RichText::new(title).monospace().size(11.5)).selectable(true).wrap());
                    match given {
                        Some(true) => {
                            ui.label(RichText::new(tr("ui.agent_chat_allowed")).color(t.text_dim));
                        }
                        Some(false) => {
                            ui.label(RichText::new(tr("ui.agent_chat_denied")).color(t.text_dim));
                        }
                        None => {
                            ui.horizontal(|ui| {
                                let allow = ui.add(egui::Button::new(tr("ui.agent_chat_allow")).fill(t.accent_soft));
                                let deny = ui.button(tr("ui.agent_chat_deny"));
                                crate::scenario::publish_handle("agent_chat:allow", allow.rect.center());
                                crate::scenario::publish_handle("agent_chat:deny", deny.rect.center());
                                if allow.clicked() {
                                    approval = Some((*id, true));
                                }
                                if deny.clicked() {
                                    approval = Some((*id, false));
                                }
                            });
                        }
                    }
                });
            }
            Item::Changes { steps, doc, undone } => {
                ui.horizontal(|ui| {
                    let text = trf("ui.agent_chat_changes", &[("n", &steps.to_string())]);
                    ui.label(RichText::new(text).color(t.text_dim).size(11.5));
                    if *undone {
                        ui.label(RichText::new(tr("ui.agent_chat_undone")).color(t.text_dim).size(11.5));
                    } else if Arc::ptr_eq(doc, &current_doc) {
                        let b = ui.small_button(tr("ui.agent_chat_undo_changes"));
                        crate::scenario::publish_handle("agent_chat:undo", b.rect.center());
                        if b.clicked() {
                            undo = Some(n);
                        }
                    }
                });
            }
            Item::Notice { text, error } => {
                ui.label(RichText::new(text).color(if *error { t.error } else { t.text_dim }));
            }
        }
        ui.add_space(2.0);
    }
    if app.agent_chat.items.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new(tr("ui.agent_chat_intro")).color(t.text_dim));
        ui.add_space(6.0);
        ui.label(RichText::new(tr("ui.agent_chat_privacy")).color(t.text_dim).size(11.0));
    }
    if let Some((n, o)) = answer {
        app.agent_chat.answer(n, o.as_deref());
    }
    if let Some((id, allow)) = approval {
        approve(app, id, allow);
    }
    if let Some(n) = undo {
        undo_changes(app, n);
    }
}

/// A small status mark drawn as a shape (no glyph a system font might lack): filled when the
/// step is running or done, hollow while it waits.
#[cfg(not(target_arch = "wasm32"))]
fn status_dot(ui: &mut egui::Ui, color: Color32, filled: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    if filled {
        ui.painter().circle_filled(r.center(), 4.0, color);
    } else {
        ui.painter().circle_stroke(r.center(), 3.5, Stroke::new(1.2, color));
    }
}

/// A custom command as the picker shows it: the program's file name and its arguments, cut short.
pub(super) fn short_command(line: &str) -> String {
    let argv = acp::split_command(line);
    let mut parts: Vec<String> = argv.iter().map(|a| a.rsplit(['/', '\\']).next().unwrap_or(a).to_string()).collect();
    if parts.is_empty() {
        parts.push(line.to_string());
    }
    let s = parts.join(" ");
    if s.chars().count() > 40 { format!("{}…", s.chars().take(40).collect::<String>()) } else { s }
}

/// Quote an argument for the command field when it holds spaces.
fn quote(a: &str) -> String {
    if a.is_empty() || a.contains(char::is_whitespace) || a.contains('"') { format!("\"{}\"", a.replace('"', "\\\"")) } else { a.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_commands_show_short_in_the_picker() {
        assert_eq!(short_command("python3 -I /very/long/path/to/mock_agent.py"), "python3 -I mock_agent.py");
        assert!(short_command(&"x".repeat(100)).ends_with('…'));
    }

    #[test]
    fn the_shortcut_shows_folds_and_hides() {
        let mut app = SolveApp::new(solidcraft_engine::Session::default(), crate::Services::default());
        toggle(&mut app);
        assert!(app.agent_chat.open);
        app.preferences.agent_chat_minimized = true;
        toggle(&mut app);
        assert!(app.agent_chat.open && !app.preferences.agent_chat_minimized, "a folded panel unfolds");
        toggle(&mut app);
        assert!(!app.agent_chat.open);
    }
}
