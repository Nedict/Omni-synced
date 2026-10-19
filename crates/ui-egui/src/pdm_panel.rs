//! Thin local PDM window. Credentials remain exclusively in the host service.
use crate::{SolveApp, documents, theme::Tokens};
use serde_json::json;
use solidcraft_engine::{
    Session,
    pdm::{Failure, Job, Outcome, Reference, Runtime},
};
use std::sync::{
    Arc,
    mpsc::{Receiver, TryRecvError},
};

pub struct Completion {
    pub result: Result<Outcome, Failure>,
    /// A separately loaded design; opening it always uses a NEW tab.
    pub open: Option<Box<Session>>,
}
pub type Start = Arc<dyn Fn(Job) -> Receiver<Completion> + Send + Sync>;

pub struct Panel {
    pub open: bool,
    pub runtime: Runtime,
    pub port: String,
    pub document: String,
    pub parent: String,
    pub selected: Option<Reference>,
    pub dependencies: Vec<Reference>,
    pending: Option<(u64, Receiver<Completion>)>,
    ready: Option<(u64, Completion)>,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            open: false,
            runtime: Runtime::default(),
            port: "48231".into(),
            document: String::new(),
            parent: String::new(),
            selected: None,
            dependencies: Vec::new(),
            pending: None,
            ready: None,
        }
    }
}

pub fn tick(app: &mut SolveApp, ctx: &egui::Context) {
    app.session.pdm = app.pdm.runtime.clone();
    let _ = app.pdm.runtime.set_available(app.services.pdm.is_some());
    if app.pdm.ready.is_none()
        && let Some((id, receiver)) = app.pdm.pending.as_ref()
    {
        match receiver.try_recv() {
            Ok(completion) => {
                app.pdm.ready = Some((*id, completion));
                app.pdm.pending = None;
            }
            Err(TryRecvError::Disconnected) => {
                app.pdm.ready =
                    Some((*id, Completion { result: Err(Failure { message: "Local PDM worker stopped".into(), disconnect: true }), open: None }));
                app.pdm.pending = None;
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
    }
    // Opening must wait for other file operations, never replace their active design.
    let can_open = !app.file_picker_busy()
        && !app.file_load.busy()
        && app.docs.closing.is_none()
        && app.docs.quitting.is_none()
        && app.dialog.is_none()
        && app.params.is_none()
        && app.design_properties.is_none()
        && app.tree.capture_prompt.is_none()
        && app.tool.is_none()
        && app.applying.is_none()
        && app.deferred.is_none();
    let needs_open = app.pdm.ready.as_ref().is_some_and(|(_, completion)| completion.open.is_some());
    if (can_open || !needs_open)
        && let Some((id, mut completion)) = app.pdm.ready.take()
    {
        let next_parent = match &completion.result {
            Ok(Outcome::CheckedIn(version)) if app.pdm.document == version.reference.document => Some(version.reference.version),
            _ => None,
        };
        let success = completion.result.is_ok();
        let accepted = match app.pdm.runtime.finish(id, completion.result) {
            Ok(()) => true,
            Err(error) => {
                app.set_status(error, true);
                false
            }
        };
        if accepted && let Some(parent) = next_parent {
            app.pdm.parent = parent.to_string();
        }
        if accepted
            && success
            && let Some(mut session) = completion.open.take()
        {
            session.pdm = app.pdm.runtime.clone();
            if let Err(error) = documents::open_tab(app, *session) {
                app.set_status(format!("PDM copy was saved but could not open: {error}"), true);
            }
        }
    }
    if app.pdm.pending.is_none() && app.pdm.ready.is_none() {
        match app.pdm.runtime.take_job() {
            Ok(Some(job)) => {
                let id = job.id;
                if let Some(start) = &app.services.pdm {
                    app.pdm.pending = Some((id, start(job)));
                    ctx.request_repaint();
                } else {
                    let _ = app.pdm.runtime.finish(id, Err(Failure { message: "Local PDM is unavailable in this host".into(), disconnect: true }));
                }
            }
            Err(error) => app.set_status(error, true),
            Ok(None) => {}
        }
    }
}

fn handle(name: &str, response: &egui::Response) {
    crate::scenario::publish_handle(&format!("pdm:{name}"), response.rect.center());
}

pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    if !app.pdm.open {
        return;
    }
    let status = match app.pdm.runtime.status() {
        Ok(status) => status,
        Err(error) => {
            app.set_status(error, true);
            return;
        }
    };
    let mut open = true;
    let mut action = None;
    let tokens = Tokens::get();
    egui::Window::new(app.tr("ui.pdm_title")).id(egui::Id::new("local_pdm_panel")).open(&mut open).default_width(560.0).resizable(true).show(
        ctx,
        |ui| {
            ui.colored_label(tokens.warning, app.tr("ui.pdm_experimental"));
            ui.label(app.tr("ui.pdm_credential_hint"));
            if !status.available {
                ui.colored_label(tokens.error, app.tr("ui.pdm_host_unavailable"));
                return;
            }
            let idle = status.busy.is_none();
            ui.horizontal(|ui| {
                ui.label(app.tr("ui.pdm_port"));
                let response = ui.add_enabled(idle && status.port.is_none(), egui::TextEdit::singleline(&mut app.pdm.port).desired_width(80.0));
                handle("port", &response);
                if status.port.is_none() {
                    let response = ui.add_enabled(idle, egui::Button::new(app.tr_label("Connect Local PDM")));
                    handle("connect", &response);
                    if response.clicked() {
                        action = Some(("pdm.connect", json!({"port": app.pdm.port.parse::<u16>().ok()})));
                    }
                } else {
                    ui.label(format!("127.0.0.1:{}", status.port.unwrap_or(0)));
                    let response = ui.add_enabled(idle, egui::Button::new(app.tr_label("Disconnect Local PDM")));
                    handle("disconnect", &response);
                    if response.clicked() {
                        action = Some(("pdm.disconnect", json!({})));
                    }
                    let response = ui.add_enabled(idle, egui::Button::new(app.tr_label("Refresh PDM Versions")));
                    handle("refresh", &response);
                    if response.clicked() {
                        action = Some(("pdm.refresh", json!({})));
                    }
                }
            });
            if status.busy.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(app.tr("ui.pdm_busy"));
                });
            }
            if let Some(error) = &status.error {
                ui.colored_label(tokens.error, error);
            }
            ui.separator();
            ui.label(app.tr("ui.pdm_versions"));
            egui::ScrollArea::vertical().id_salt("pdm_versions").max_height(180.0).show(ui, |ui| {
                for version in &status.versions {
                    ui.horizontal(|ui| {
                        let reference = &version.reference;
                        let selected = app.pdm.selected.as_ref() == Some(reference);
                        let response = ui.selectable_label(selected, format!("{} · v{}", reference.document, reference.version));
                        handle(&format!("version:{}:{}", reference.document, reference.version), &response);
                        if response.clicked() {
                            app.pdm.selected = Some(reference.clone());
                            app.pdm.document = reference.document.clone();
                            app.pdm.parent = reference.version.to_string();
                            app.pdm.dependencies = version.dependencies.clone();
                        }
                        let mut dependency = app.pdm.dependencies.contains(reference);
                        let response = ui.checkbox(&mut dependency, app.tr("ui.pdm_use_dependency"));
                        handle(&format!("dependency:{}:{}", reference.document, reference.version), &response);
                        if response.changed() {
                            app.pdm.dependencies.retain(|r| r != reference);
                            if dependency {
                                app.pdm.dependencies.push(reference.clone());
                            }
                        }
                    });
                }
            });
            if let Some(version) = status.versions.iter().find(|v| app.pdm.selected.as_ref() == Some(&v.reference)) {
                ui.label(app.tr("ui.pdm_pinned_dependencies"));
                if version.dependencies.is_empty() {
                    ui.label(app.tr("ui.pdm_no_dependencies"));
                }
                for reference in &version.dependencies {
                    ui.label(format!("{} · v{}", reference.document, reference.version));
                }
            }
            let response =
                ui.add_enabled(idle && status.port.is_some() && app.pdm.selected.is_some(), egui::Button::new(app.tr("ui.pdm_retrieve_open")));
            handle("retrieve", &response);
            if response.clicked()
                && let Some(reference) = &app.pdm.selected
            {
                action = Some(("pdm.retrieve", json!({"document":reference.document,"version":reference.version,"open":true})));
            }
            if let Some(retrieved) = &status.retrieved {
                let name = retrieved.path.rsplit(['/', '\\']).next().unwrap_or("working.solidcraft");
                ui.label(format!("{}: {} · v{} · {name}", app.tr("ui.pdm_saved_copy"), retrieved.reference.document, retrieved.reference.version))
                    .on_hover_text(&retrieved.path);
            }
            ui.separator();
            ui.label(app.tr("ui.pdm_check_in_active"));
            if let Some(path) = &app.session.path {
                ui.label(path.rsplit(['/', '\\']).next().unwrap_or(path)).on_hover_text(path);
            } else {
                ui.label(app.tr("ui.pdm_unsaved"));
            }
            ui.horizontal(|ui| {
                ui.label(app.tr("ui.pdm_document_id"));
                let response = ui.text_edit_singleline(&mut app.pdm.document);
                handle("document", &response);
                ui.label(app.tr("ui.pdm_expected_parent"));
                let response = ui.add(egui::TextEdit::singleline(&mut app.pdm.parent).desired_width(65.0));
                handle("parent", &response);
            });
            ui.label(app.tr("ui.pdm_new_parent_hint"));
            for reference in &app.pdm.dependencies {
                ui.label(format!("{}: {} · v{}", app.tr("ui.pdm_dependency"), reference.document, reference.version));
            }
            let can_check_in = idle
                && status.port.is_some()
                && !app.session.is_dirty()
                && app.session.active_sketch.is_none()
                && app.session.pending_moves.is_empty()
                && app.dialog.is_none()
                && app.tool.is_none()
                && app.applying.is_none()
                && app.deferred.is_none()
                && app.session.path.as_deref().is_some_and(solidcraft_engine::io::is_design_path);
            let response = ui.add_enabled(can_check_in, egui::Button::new(app.tr_label("Check In Saved Design")));
            handle("check_in", &response);
            if response.clicked() {
                let parent = if app.pdm.parent.trim().is_empty() {
                    serde_json::Value::Null
                } else {
                    app.pdm.parent.parse::<u64>().ok().map_or(json!("invalid"), |v| json!(v))
                };
                action = Some(("pdm.check_in", json!({"document":app.pdm.document,"parent":parent,"dependencies":app.pdm.dependencies})));
            }
            if !can_check_in {
                ui.label(app.tr("ui.pdm_save_first"));
            }
            ui.label(app.tr("ui.pdm_link_limit"));
        },
    );
    app.pdm.open = open;
    if let Some((command, params)) = action
        && let Err(error) = app.run(command, params)
    {
        app.set_status(error, true);
    }
}
