//! Guided local setup and saved connection login; all side effects run in the native worker.
use crate::SolveApp;
use serde_json::json;
use solidcraft_engine::pdm::{ConnectionKind, ConnectionStage};

pub struct GuidedState {
    pub label: String,
    pub administrator: String,
    listing_requested: bool,
    pub pending_recovery: Option<(String, String)>,
}
impl Default for GuidedState {
    fn default() -> Self {
        Self { label: String::new(), administrator: "admin".into(), listing_requested: false, pending_recovery: None }
    }
}

fn valid_form(label: &str, administrator: &str) -> bool {
    !label.trim().is_empty()
        && label.len() <= 128
        && !label.chars().any(char::is_control)
        && !administrator.is_empty()
        && administrator.len() <= 64
        && administrator != "owner"
        && administrator.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

pub fn connection_window(app: &mut SolveApp, ctx: &egui::Context) {
    if !app.pdm.open {
        app.pdm.guided.listing_requested = false;
        app.pdm.guided.pending_recovery = None;
        return;
    }
    // Exactly one asynchronous inventory request per opening. Wait for an earlier
    // job instead of dropping the request or starting a parallel native worker.
    let Ok(status) = app.pdm.runtime.status() else {
        return;
    };
    if !app.pdm.guided.listing_requested && status.available && status.busy.is_none() {
        app.pdm.guided.listing_requested = true;
        super::dispatch(app, "pdm.connections", json!({}));
    }
    let Ok(status) = app.pdm.runtime.status() else {
        return;
    };
    let idle = status.available && status.busy.is_none() && app.pdm.guided.pending_recovery.is_none();
    let disconnected = status.connection.is_none() && status.port.is_none();
    let mut open = true;
    let mut action = None;
    crate::frame::window(ctx, app.tr("ui.pdm_connect"), crate::frame::Width::Normal).id(egui::Id::new("pdm_connection")).open(&mut open).show(
        ctx,
        |ui| {
            if status.busy.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(app.tr("ui.pdm_busy"));
                });
            }
            if let Some(error) = &status.error {
                ui.colored_label(crate::theme::Tokens::get().error, error);
            }
            // These values are fresh authenticated results, never copied from a saved row.
            if let Some(active) = &status.connection {
                ui.heading(&active.label);
                egui::Grid::new("pdm_verified_connection").show(ui, |ui| {
                    ui.label(app.tr("ui.pdm_account"));
                    ui.label(&active.account);
                    ui.end_row();
                    ui.label(app.tr("ui.pdm_role"));
                    ui.label(&active.role);
                    ui.end_row();
                    ui.label(app.tr("ui.pdm_address"));
                    ui.label(&active.origin);
                    ui.end_row();
                    if let Some(root) = &status.managed.root {
                        ui.label(app.tr("ui.pdm_working_folder"));
                        ui.add(egui::Label::new(root.as_str()).selectable(true).wrap());
                        ui.end_row();
                    }
                });
                let response = ui.add_enabled(idle, egui::Button::new(app.tr("ui.pdm_disconnect")));
                super::handle("disconnect", &response);
                if response.clicked() {
                    action = Some(("pdm.disconnect", json!({})));
                }
                ui.separator();
            } else if status.port.is_some() {
                // A loopback connection has no saved verified account summary.
                // Its disconnect action must remain visible outside Advanced.
                ui.heading(app.tr("ui.pdm_connected_archive"));
                if let Some(root) = &status.managed.root {
                    ui.add(egui::Label::new(root.as_str()).selectable(true).wrap());
                }
                let response = ui.add_enabled(idle, egui::Button::new(app.tr("ui.pdm_disconnect")));
                super::handle("disconnect", &response);
                if response.clicked() {
                    action = Some(("pdm.disconnect", json!({})));
                }
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.heading(app.tr("ui.pdm_recent_connections"));
                let response = ui.add_enabled(idle, egui::Button::new(app.tr("ui.pdm_refresh_connections")));
                super::handle("connections_refresh", &response);
                if response.clicked() {
                    action = Some(("pdm.connections", json!({})));
                }
            });
            if status.connections.is_empty() && status.busy.is_none() {
                ui.label(app.tr("ui.pdm_no_connections"));
            }
            egui::ScrollArea::vertical().id_salt("pdm_recent_connections").max_height(180.0).show(ui, |ui| {
                for recent in &status.connections {
                    ui.push_id(&recent.id, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(&recent.label);
                            ui.label(app.tr(match recent.kind {
                                ConnectionKind::Local => "ui.pdm_local_archive",
                                ConnectionKind::Saved => "ui.pdm_saved_connection",
                            }));
                            let active = status.connection.as_ref().is_some_and(|active| active.id == recent.id);
                            ui.label(app.tr(if active {
                                "ui.pdm_connected"
                            } else {
                                match recent.stage {
                                    ConnectionStage::Pending => "ui.pdm_setup_incomplete",
                                    ConnectionStage::SavedUnverified => "ui.pdm_saved_unverified",
                                }
                            }));
                            let key = if matches!(recent.stage, ConnectionStage::Pending) { "ui.pdm_resume_setup" } else { "ui.pdm_open_connection" };
                            let response = ui.add_enabled(idle && disconnected, egui::Button::new(app.tr(key)));
                            super::handle(&format!("connection:{}", recent.id), &response);
                            if response.clicked() {
                                action = Some(("pdm.connection.open", json!({"connection_id":recent.id})));
                            }
                            let response = ui.add_enabled(
                                idle && disconnected && !crate::documents::any_dirty(app),
                                egui::Button::new(app.tr("ui.pdm_recover_connection")),
                            );
                            super::handle(&format!("connection_recover:{}", recent.id), &response);
                            if response.clicked() {
                                app.pdm.guided.pending_recovery = Some((recent.id.clone(), recent.label.clone()));
                            }
                        });
                    });
                }
            });
            ui.separator();
            ui.heading(app.tr("ui.pdm_create_local_archive"));
            ui.label(app.tr("ui.pdm_create_local_hint"));
            ui.add_enabled_ui(idle && disconnected, |ui| {
                ui.label(app.tr("ui.pdm_archive_name"));
                let response = ui.text_edit_singleline(&mut app.pdm.guided.label);
                super::handle("setup_label", &response);
                ui.label(app.tr("ui.pdm_administrator"));
                let response = ui.text_edit_singleline(&mut app.pdm.guided.administrator);
                super::handle("setup_administrator", &response);
                let response = ui.add_enabled(
                    valid_form(&app.pdm.guided.label, &app.pdm.guided.administrator),
                    egui::Button::new(app.tr("ui.pdm_create_archive")),
                );
                super::handle("setup_create", &response);
                if response.clicked() {
                    action = Some(("pdm.setup.create", json!({"label":app.pdm.guided.label.trim(),"administrator":app.pdm.guided.administrator})));
                }
            });
            // Transitional loopback compatibility; never the default onboarding path.
            let advanced = egui::CollapsingHeader::new(app.tr("ui.pdm_advanced")).id_salt("pdm_advanced_connection").show(ui, |ui| {
                ui.add_enabled_ui(idle && disconnected, |ui| {
                    ui.label(app.tr("ui.pdm_port"));
                    let response = ui.text_edit_singleline(&mut app.pdm.port);
                    super::handle("port", &response);
                    ui.label(app.tr("ui.pdm_working_folder"));
                    let response = ui.text_edit_singleline(&mut app.pdm.root);
                    super::handle("root", &response);
                    let response = ui.button(app.tr("ui.pdm_connect"));
                    super::handle("connect", &response);
                    if response.clicked() {
                        let mut params = json!({"port":app.pdm.port.parse::<u16>().ok()});
                        if !app.pdm.root.trim().is_empty() {
                            params["root"] = json!(app.pdm.root);
                        }
                        action = Some(("pdm.connect", params));
                    }
                    let response = ui.add_enabled(!app.pdm.root.trim().is_empty(), egui::Button::new(app.tr("ui.pdm_recover_workspace")));
                    super::handle("recover_workspace", &response);
                    if response.clicked() {
                        app.pdm.recovery_confirmation = true;
                    }
                });
            });
            super::handle("advanced", &advanced.header_response);
        },
    );
    app.pdm.open = open;
    if !open {
        app.pdm.guided.listing_requested = false;
    }
    if let Some((id, params)) = action {
        super::dispatch(app, id, params);
    }
    if open && let Some((connection_id, label)) = app.pdm.guided.pending_recovery.clone() {
        let mut confirm_open = true;
        crate::frame::window(ctx, app.tr("ui.pdm_recover_workspace"), crate::frame::Width::Normal)
            .id(egui::Id::new("pdm_guided_recovery"))
            .open(&mut confirm_open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.strong(label);
                ui.add(egui::Label::new(app.tr("ui.pdm_recover_warning")).wrap());
                let dirty = crate::documents::any_dirty(app);
                if dirty {
                    ui.label(app.tr("ui.pdm_save_first"));
                }
                let response =
                    ui.add_enabled(status.available && status.busy.is_none() && !dirty, egui::Button::new(app.tr("ui.pdm_recover_confirm")));
                super::handle("connection_recover_confirm", &response);
                if response.clicked() {
                    app.pdm.guided.pending_recovery = None;
                    super::dispatch(app, "pdm.connection.recover", json!({"connection_id":connection_id,"restore":true}));
                }
                let response = ui.button(app.tr("ui.cancel"));
                super::handle("connection_recover_cancel", &response);
                if response.clicked() {
                    app.pdm.guided.pending_recovery = None;
                }
            });
        if !confirm_open {
            app.pdm.guided.pending_recovery = None;
        }
    }
}

#[cfg(test)]
mod form_tests {
    use super::*;
    #[test]
    fn public_setup_form_accepts_only_bounded_display_label_and_account_id() {
        assert!(valid_form("My archive", "admin"));
        assert!(!valid_form(" ", "admin"));
        assert!(!valid_form("Archive", "owner"));
        assert!(!valid_form("Archive", "Admin user"));
        assert!(!valid_form(&"x".repeat(129), "admin"));
        assert!(!valid_form("Archive\nOther", "admin"));
    }
}
