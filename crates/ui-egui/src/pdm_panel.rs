//! Thin local PDM window. Credentials remain exclusively in the host service.
use crate::{SolveApp, documents, theme::Tokens};
use serde_json::json;
use solidcraft_engine::{
    Session,
    pdm::{Failure, Job, ManagedTask, Outcome, Preview, Reference, Runtime, Task, cached_path_key},
};
use std::sync::{
    Arc,
    mpsc::{Receiver, TryRecvError},
};

#[path = "pdm_guided_panel.rs"]
mod guided;

pub struct Completion {
    pub result: Result<Outcome, Failure>,
    /// A separately loaded design; opening it always uses a NEW tab.
    pub open: Option<Box<Session>>,
}
pub type Start = Arc<dyn Fn(Job) -> Receiver<Completion> + Send + Sync>;

/// A single bounded decoded preview. Invalid content is cached too, avoiding repeated
/// decode work on every frame. Its identity includes the immutable version and hash.
#[derive(Default)]
struct PreviewCache {
    key: Option<(Reference, String)>,
    texture: Option<egui::TextureHandle>,
}
impl PreviewCache {
    fn update(&mut self, ctx: &egui::Context, preview: Option<&Preview>) {
        let key = preview.map(|p| (p.reference.clone(), p.sha256.clone()));
        if self.key == key {
            return;
        }
        self.key = key;
        self.texture = preview
            .filter(|p| p.png.len() <= 512 * 1024)
            .and_then(|p| crate::home::decode_thumbnail(p.png.clone()))
            .map(|image| ctx.load_texture("pdm-native-preview", image, egui::TextureOptions::LINEAR));
    }
}

pub struct Panel {
    pub guided: guided::GuidedState,
    pub open: bool,
    pub workspace_active: bool,
    pub root: String,
    pub logical_path: String,
    pub folder: String,
    pub detail_tab: usize,
    pub cancel_confirmation: bool,
    pub recovery_confirmation: bool,
    pub restore_confirmation: Option<Reference>,
    pub pending_edit: Option<String>,
    pub pending_edit_path: Option<String>,
    resume_edit: Option<(String, String)>,
    pub ram_paths: std::collections::BTreeSet<String>,
    preview_cache: PreviewCache,
    reset_ram_on_completion: bool,
    last_activity: web_time::Instant,
    refresh_delay: std::time::Duration,
    pub runtime: Runtime,
    pub port: String,
    pub document: String,
    pub parent: String,
    pub selected: Option<Reference>,
    pub dependencies: Vec<Reference>,
    pending: Option<(u64, Receiver<Completion>)>,
    ready: Option<(u64, Completion)>,
    pending_open_epoch: Option<u64>,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            guided: guided::GuidedState::default(),
            open: false,
            workspace_active: false,
            root: String::new(),
            logical_path: String::new(),
            folder: String::new(),
            detail_tab: 0,
            cancel_confirmation: false,
            recovery_confirmation: false,
            restore_confirmation: None,
            pending_edit: None,
            pending_edit_path: None,
            resume_edit: None,
            ram_paths: Default::default(),
            preview_cache: PreviewCache::default(),
            reset_ram_on_completion: false,
            last_activity: web_time::Instant::now(),
            refresh_delay: std::time::Duration::from_secs(10),
            runtime: Runtime::default(),
            port: "48231".into(),
            document: String::new(),
            parent: String::new(),
            selected: None,
            dependencies: Vec::new(),
            pending: None,
            ready: None,
            pending_open_epoch: None,
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
        let previous = app.pdm.runtime.status().ok();
        let context_changed = match (&previous, &completion.result) {
            (
                Some(previous),
                Ok(Outcome::Managed { port, state } | Outcome::ManagedRefreshed { port, state } | Outcome::ManagedConnected { port, state, .. }),
            ) => previous.port != Some(*port) || previous.managed.server_id != state.server_id || previous.managed.workspace != state.workspace,
            _ => false,
        };
        let managed = matches!(&completion.result, Ok(Outcome::Managed { .. } | Outcome::ManagedConnected { .. }));
        let success = completion.result.is_ok();
        let accepted = match app.pdm.runtime.finish(id, completion.result) {
            Ok(()) => true,
            Err(error) => {
                app.set_status(error, true);
                false
            }
        };
        app.pdm.last_activity = web_time::Instant::now();
        app.pdm.refresh_delay =
            if success { std::time::Duration::from_secs(10) } else { (app.pdm.refresh_delay * 2).min(std::time::Duration::from_secs(60)) };
        if accepted && success && context_changed {
            app.pdm.document.clear();
            app.pdm.parent.clear();
            app.pdm.logical_path.clear();
            app.pdm.folder.clear();
            app.pdm.detail_tab = 0;
            app.pdm.selected = None;
            app.pdm.dependencies.clear();
            app.pdm.restore_confirmation = None;
            app.pdm.guided.pending_recovery = None;
            app.pdm.cancel_confirmation = false;
            app.pdm.recovery_confirmation = false;
            app.pdm.pending_edit = None;
            app.pdm.pending_edit_path = None;
            app.pdm.resume_edit = None;
            app.pdm.ram_paths.clear();
            app.pdm.preview_cache = PreviewCache::default();
        }
        if accepted && success && managed && app.pdm.reset_ram_on_completion {
            app.pdm.ram_paths.clear();
        }
        if accepted && let Some(parent) = next_parent {
            app.pdm.parent = parent.to_string();
        }
        if accepted
            && success
            && let Some(mut session) = completion.open.take()
        {
            let applied = app
                .pdm
                .pending_open_epoch
                .take()
                .ok_or_else(|| "Reopen this design from disk after workspace recovery".to_string())
                .and_then(|epoch| documents::bind_fresh_pdm_load(app, &mut session, epoch))
                .and_then(|()| documents::apply_managed(app, *session));
            if let Err(error) = applied {
                app.set_status(format!("PDM copy was saved but could not open: {error}"), true);
            }
        }
    }
    if app.pdm.resume_edit.is_some()
        && app.pdm.pending.is_none()
        && app.pdm.ready.is_none()
        && app.pdm.runtime.status().is_ok_and(|s| s.busy.is_none())
    {
        let permitted = app.session.path.as_ref().is_some_and(|p| app.pdm.runtime.managed_for_path(p).is_some_and(|e| e.checked_out));
        if let Some((path, command)) = app.pdm.resume_edit.take()
            && permitted
            && app.session.path.as_ref().is_some_and(|current| cached_path_key(current) == path)
        {
            app.start(&command);
        }
    }
    if app.pdm.pending.is_none() && app.pdm.ready.is_none() {
        match app.pdm.runtime.take_job() {
            Ok(Some(job)) => {
                let id = job.id;
                // Recovery restores disk but must not bless an old editor. Normal
                // native load jobs are serialized by this same Runtime.
                app.pdm.pending_open_epoch =
                    if matches!(&job.task, Task::Managed(ManagedTask::Recover { .. } | ManagedTask::RecoverConnection { .. })) {
                        None
                    } else {
                        app.pdm.runtime.begin_pdm_reload().ok()
                    };
                app.pdm.reset_ram_on_completion = matches!(
                    &job.task,
                    Task::Managed(
                        ManagedTask::CreateLocal { .. }
                            | ManagedTask::OpenConnection { .. }
                            | ManagedTask::RecoverConnection { .. }
                            | ManagedTask::Restore { .. }
                            | ManagedTask::Connect { .. }
                            | ManagedTask::Recover { .. }
                            | ManagedTask::Archive { .. }
                            | ManagedTask::Checkout { .. }
                            | ManagedTask::CancelCheckout { .. }
                            | ManagedTask::GetLatest { .. }
                            | ManagedTask::Open { .. }
                    )
                );
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
    if let Ok(status) = app.pdm.runtime.status()
        && status.port.is_some()
        && status.available
    {
        let remaining = app.pdm.refresh_delay.saturating_sub(app.pdm.last_activity.elapsed());
        if remaining.is_zero() && status.busy.is_none() && app.pdm.pending.is_none() && app.pdm.ready.is_none() {
            app.pdm.last_activity = web_time::Instant::now();
            if let Err(error) = app.run("pdm.refresh", json!({})) {
                app.set_status(error, true);
            }
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(remaining.max(std::time::Duration::from_millis(50)));
        }
    }
}

fn handle(name: &str, response: &egui::Response) {
    crate::scenario::publish_handle(&format!("pdm:{name}"), response.rect.center());
}

/// Gregorian UTC display, bounded to four-digit years and without platform time APIs.
fn utc_time(milliseconds: u64) -> String {
    if milliseconds > 253_402_300_799_999 {
        return "—".into();
    }
    let seconds = milliseconds / 1000;
    let days = seconds / 86_400;
    let year_start = |year: u64| {
        let previous = year - 1;
        365 * (year - 1970) + previous / 4 - previous / 100 + previous / 400 - 477
    };
    let (mut low, mut high) = (1970_u64, 10_000_u64);
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if year_start(middle) <= days {
            low = middle;
        } else {
            high = middle;
        }
    }
    let year = low;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let lengths = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut day = days - year_start(year);
    for (month, length) in lengths.into_iter().enumerate() {
        if day < length {
            return format!("{year:04}-{:02}-{:02} {:02}:{:02} UTC", month + 1, day + 1, seconds % 86_400 / 3600, seconds % 3600 / 60);
        }
        day -= length;
    }
    "—".into()
}

fn reference_label(reference: &Reference, catalog: &[solidcraft_engine::pdm::CatalogEntry]) -> String {
    let name = catalog.iter().find(|entry| entry.document == reference.document).map(|entry| entry.logical_path.as_str());
    let fallback: String = reference.document.chars().take(12).collect();
    format!("{} · v{}", name.unwrap_or(&fallback), reference.version)
}

/// Enter the archive browser without changing the active document.
pub fn enter(app: &mut SolveApp) -> Result<(), String> {
    if app.tool.is_some() || app.dialog.is_some() || app.session.active_sketch.is_some() || app.file_load.busy() {
        return Err("Finish the current operation before entering PDM".into());
    }
    crate::render_workspace::enter(app, false)?;
    app.pdm.workspace_active = true;
    app.pdm.open = false;
    app.ui.tab = "PDM".into();
    app.home.open = false;
    Ok(())
}

fn dispatch(app: &mut SolveApp, id: &str, params: serde_json::Value) {
    if let Err(error) = app.run(id, params) {
        app.set_status(error, true);
    }
}

/// Same commands serve the modelling ribbon and the archive workspace.
pub fn actions(app: &mut SolveApp, ui: &mut egui::Ui) {
    let Ok(status) = app.pdm.runtime.status() else {
        return;
    };
    let idle = status.available && status.busy.is_none();
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        for (id, key, name) in [
            ("pdm.panel", "ui.pdm_connect", "connect_settings"),
            ("pdm.checkout", "ui.pdm_checkout", "checkout"),
            ("pdm.cancel_checkout", "ui.pdm_cancel_checkout", "cancel_checkout"),
            ("pdm.archive", "ui.pdm_archive", "archive"),
            ("pdm.refresh", "ui.pdm_refresh", "refresh"),
            ("pdm.get_latest", "ui.pdm_get_latest", "get_latest"),
        ] {
            let response = ui.add_enabled(idle && (id == "pdm.panel" || status.port.is_some()), egui::Button::new(app.tr(key)));
            handle(name, &response);
            if response.clicked() {
                action = Some(id);
            }
        }
    });
    if let Some(id) = action {
        if id == "pdm.panel" {
            app.pdm.open = true;
        } else if id == "pdm.cancel_checkout" {
            app.pdm.cancel_confirmation = true;
        } else {
            let params = if app.pdm.workspace_active && !app.pdm.document.is_empty() && matches!(id, "pdm.checkout" | "pdm.get_latest") {
                json!({"document":app.pdm.document})
            } else {
                json!({})
            };
            dispatch(app, id, params);
        }
    }
}

pub fn workspace(app: &mut SolveApp, ui: &mut egui::Ui) {
    let Ok(status) = app.pdm.runtime.status() else {
        return;
    };
    let catalog = &status.managed.catalog;
    let local = &status.managed.local;
    ui.heading(app.tr("ui.pdm_title"));
    if let Some(root) = status.managed.root.as_deref() {
        let folder = root.rsplit(['/', '\\']).find(|part| !part.is_empty()).unwrap_or(root);
        ui.label(format!("{}: {folder}", app.tr("ui.pdm_working_folder"))).on_hover_text(root);
    }
    if status.port.is_none() {
        ui.label(app.tr("ui.pdm_connect_hint"));
    }
    if status.busy.is_some() {
        ui.spinner();
        ui.label(app.tr("ui.pdm_busy"));
    }
    if let Some(error) = &status.error {
        ui.colored_label(Tokens::get().error, error);
    }
    let versions: Vec<_> =
        if status.managed.history.is_empty() { status.versions.clone() } else { status.managed.history.iter().map(|v| v.version.clone()).collect() };
    ui.separator();
    let mut action = None;
    ui.columns(2, |columns| {
        let left = &mut columns[0];
        left.set_max_width(230.0);
        left.heading(app.tr("ui.pdm_folders"));
        if left.selectable_label(app.pdm.folder.is_empty(), "/").clicked() {
            app.pdm.folder.clear();
        }
        let mut folders = std::collections::BTreeSet::new();
        for entry in catalog {
            let path = &entry.logical_path;
            let mut parent = String::new();
            let parts: Vec<_> = path.split('/').collect();
            for part in parts.iter().take(parts.len().saturating_sub(1)) {
                if !parent.is_empty() {
                    parent.push('/');
                }
                parent.push_str(part);
                folders.insert(parent.clone());
            }
        }
        for folder in folders {
            if left.selectable_label(app.pdm.folder == folder, &folder).clicked() {
                app.pdm.folder = folder;
            }
        }
        let right = &mut columns[1];
        egui::ScrollArea::both().id_salt("pdm_catalog").show(right, |ui| {
            egui::Grid::new("pdm_files").striped(true).show(ui, |ui| {
                for key in [
                    "ui.pdm_file",
                    "ui.pdm_local_status",
                    "ui.pdm_server_version",
                    "ui.pdm_local_version",
                    "ui.pdm_checkout_owner",
                    "ui.pdm_checked_in",
                ] {
                    ui.strong(app.tr(key));
                }
                ui.end_row();
                for entry in catalog {
                    let document = &entry.document;
                    let path = &entry.logical_path;
                    if !app.pdm.folder.is_empty() && !path.starts_with(&format!("{}/", app.pdm.folder)) {
                        continue;
                    }
                    let response = ui.selectable_label(app.pdm.document == *document, path.as_str());
                    handle(&format!("file:{document}"), &response);
                    if response.clicked() {
                        app.pdm.document = document.to_string();
                        app.pdm.selected = None;
                        action = Some(("pdm.history", json!({"document":document})));
                    }
                    let copy = local.iter().find(|v| &v.document == document);
                    let head = entry.head;
                    let base = copy.map(|v| v.base_version);
                    let label = if copy.is_none_or(|v| !v.present) {
                        "ui.pdm_absent"
                    } else if copy.is_some_and(|v| v.modified) {
                        "ui.pdm_modified"
                    } else if base == head {
                        "ui.pdm_current"
                    } else {
                        "ui.pdm_outdated"
                    };
                    ui.label(app.tr(label));
                    ui.label(head.map_or_else(|| "—".into(), |v| v.to_string()));
                    ui.label(base.map_or_else(|| "—".into(), |v| v.to_string()));
                    if let Some(lock) = &entry.checkout {
                        ui.label(format!("{} · {} · {}", lock.owner, lock.machine, utc_time(lock.checked_out_unix_ms)));
                    } else {
                        ui.label("—");
                    }
                    ui.label(entry.head_checked_in_unix_ms.map_or_else(|| "—".into(), utc_time));
                    ui.end_row();
                }
            });
        });
    });
    ui.separator();
    ui.horizontal(|ui| {
        for (index, key) in ["ui.pdm_versions", "ui.pdm_uses", "ui.pdm_where_used", "ui.pdm_preview"].iter().enumerate() {
            let label = app.tr(key).to_string();
            ui.selectable_value(&mut app.pdm.detail_tab, index, label);
        }
    });
    egui::ScrollArea::vertical().id_salt("pdm_details").max_height(220.0).show(ui, |ui| match app.pdm.detail_tab {
        0 => {
            for version in versions.iter().filter(|v| v.reference.document == app.pdm.document) {
                let reference = &version.reference;
                let response =
                    ui.selectable_label(app.pdm.selected.as_ref() == Some(reference), format!("v{} · {}", reference.version, version.sha256));
                handle(&format!("version:{}:{}", reference.document, reference.version), &response);
                if let Some(details) = status.managed.history.iter().find(|v| v.version.reference == *reference) {
                    ui.label(format!("{} · {} · {}", details.author, utc_time(details.checked_in_unix_ms), details.comment));
                }
                if response.clicked() {
                    app.pdm.selected = Some(reference.clone());
                    action = Some(("pdm.preview", json!({"document":reference.document,"version":reference.version})));
                }
            }
        }
        1 => {
            if let Some(version) = versions.iter().find(|v| app.pdm.selected.as_ref() == Some(&v.reference)) {
                for reference in &version.dependencies {
                    ui.label(reference_label(reference, catalog));
                }
            }
        }
        2 => {
            for reference in &status.managed.where_used {
                ui.label(reference_label(reference, catalog));
            }
        }
        _ => {
            let preview = status
                .managed
                .preview
                .as_ref()
                .filter(|p| p.reference.document == app.pdm.document && app.pdm.selected.as_ref().is_none_or(|r| r == &p.reference));
            app.pdm.preview_cache.update(ui.ctx(), preview);
            if let Some(texture) = &app.pdm.preview_cache.texture {
                ui.image((texture.id(), texture.size_vec2()));
            } else {
                ui.label(app.tr("ui.pdm_preview_unavailable"));
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(!app.pdm.document.is_empty() && status.busy.is_none(), egui::Button::new(app.tr("ui.pdm_open_document"))).clicked() {
            action = Some(("pdm.open_document", json!({"document":app.pdm.document})));
        }
        if ui
            .add_enabled(
                app.pdm.selected.as_ref().is_some_and(|r| r.document == app.pdm.document) && status.busy.is_none(),
                egui::Button::new(app.tr("ui.pdm_open_archived")),
            )
            .clicked()
            && let Some(reference) = &app.pdm.selected
        {
            action = Some(("pdm.open_archived", json!({"document":reference.document,"version":reference.version})));
        }
        let selected = app.pdm.selected.as_ref().filter(|r| r.document == app.pdm.document);
        let local = selected.and_then(|r| status.managed.local.iter().find(|entry| entry.document == r.document));
        let enabled = status.busy.is_none()
            && local.is_some_and(|entry| entry.present && !entry.checked_out && !entry.modified)
            && !documents::managed_target_dirty(app, selected.map(|r| r.document.as_str()));
        let response = ui.add_enabled(enabled, egui::Button::new(app.tr("ui.pdm_restore_version")));
        handle("restore_version", &response);
        if response.clicked() {
            app.pdm.restore_confirmation = selected.cloned();
        }
    });
    if let Some((id, params)) = action {
        dispatch(app, id, params);
    }
}

pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    guided::connection_window(app, ctx);
    if let Some(reference) = app.pdm.restore_confirmation.clone() {
        egui::Modal::new(egui::Id::new("pdm_restore_version_confirmation")).show(ctx, |ui| {
            ui.heading(app.tr("ui.pdm_restore_version"));
            ui.label(app.tr("ui.pdm_restore_version_warning"));
            ui.label(format!("{} - v{}", reference.document, reference.version));
            let response = ui.button(app.tr("ui.pdm_restore_version_confirm"));
            handle("restore_version_confirm", &response);
            if response.clicked() {
                app.pdm.restore_confirmation = None;
                dispatch(app, "pdm.restore_version", json!({"document":reference.document,"version":reference.version,"restore":true}));
            }
            if ui.button(app.tr("ui.cancel")).clicked() {
                app.pdm.restore_confirmation = None;
            }
        });
    }
    if app.pdm.recovery_confirmation {
        egui::Modal::new(egui::Id::new("pdm_recover_confirmation")).show(ctx, |ui| {
            ui.heading(app.tr("ui.pdm_recover_workspace"));
            ui.label(app.tr("ui.pdm_recover_warning"));
            ui.label(format!("127.0.0.1:{}", app.pdm.port));
            ui.label(&app.pdm.root);
            if ui.button(app.tr("ui.pdm_recover_confirm")).clicked() {
                app.pdm.recovery_confirmation = false;
                dispatch(app, "pdm.recover_workspace", json!({"port":app.pdm.port.parse::<u16>().ok(),"root":app.pdm.root,"restore":true}));
            }
            if ui.button(app.tr("ui.cancel")).clicked() {
                app.pdm.recovery_confirmation = false;
            }
        });
    }
    if app.pdm.pending_edit.is_some() && app.pdm.pending_edit_path != app.session.path.as_ref().map(|path| cached_path_key(path)) {
        app.pdm.pending_edit = None;
    }
    if let Some(command) = app.pdm.pending_edit.clone() {
        egui::Modal::new(egui::Id::new("pdm_readonly_confirmation")).show(ctx, |ui| {
            ui.heading(app.tr("ui.pdm_readonly_title"));
            ui.label(app.tr("ui.pdm_readonly_hint"));
            if ui.add_enabled(!app.session.pdm_read_only, egui::Button::new(app.tr("ui.pdm_checkout"))).clicked() {
                app.pdm.pending_edit = None;
                app.pdm.resume_edit = app.session.path.as_ref().map(|path| (cached_path_key(path), command.clone()));
                dispatch(app, "pdm.checkout", json!({}));
            }
            if ui.button(app.tr("ui.pdm_edit_ram")).clicked() {
                if let Some(path) = &app.session.path {
                    app.pdm.ram_paths.insert(cached_path_key(path));
                }
                app.pdm.pending_edit = None;
                app.start(&command);
            }
            if ui.button(app.tr("ui.cancel")).clicked() {
                app.pdm.pending_edit = None;
            }
        });
    }
    if app.pdm.cancel_confirmation {
        egui::Modal::new(egui::Id::new("pdm_undo_confirmation")).show(ctx, |ui| {
            ui.heading(app.tr("ui.pdm_cancel_checkout"));
            ui.label(app.tr("ui.pdm_discard_warning"));
            if ui.button(app.tr("ui.pdm_discard_confirm")).clicked() {
                app.pdm.cancel_confirmation = false;
                let mut params = json!({"discard":true});
                if app.pdm.workspace_active && !app.pdm.document.is_empty() {
                    params["document"] = json!(app.pdm.document);
                }
                dispatch(app, "pdm.cancel_checkout", params);
            }
            if ui.button(app.tr("ui.cancel")).clicked() {
                app.pdm.cancel_confirmation = false;
            }
        });
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    fn preview() -> Preview {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(240, 150, image::Rgba([20, 80, 160, 255])));
        let mut output = std::io::Cursor::new(Vec::new());
        image.write_to(&mut output, image::ImageFormat::Png).unwrap();
        Preview { reference: Reference { document: "part".into(), version: 1 }, sha256: "00".repeat(32), png: output.into_inner() }
    }

    #[test]
    fn utc_times_are_readable_and_bound_input() {
        assert_eq!(utc_time(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc_time(951_782_400_000), "2000-02-29 00:00 UTC");
        assert_eq!(utc_time(4_107_542_400_000), "2100-03-01 00:00 UTC");
        assert_eq!(utc_time(253_402_300_799_999), "9999-12-31 23:59 UTC");
        assert_eq!(utc_time(u64::MAX), "—");
    }

    #[test]
    fn native_preview_cache_reuses_exact_identity_and_clears_stale_images() {
        let ctx = egui::Context::default();
        let mut cache = PreviewCache::default();
        let mut preview = preview();
        cache.update(&ctx, Some(&preview));
        let first = cache.texture.as_ref().unwrap().id();
        assert_eq!(cache.texture.as_ref().unwrap().size(), [240, 150]);
        cache.update(&ctx, Some(&preview));
        assert_eq!(cache.texture.as_ref().unwrap().id(), first);
        preview.reference.version = 2;
        cache.update(&ctx, Some(&preview));
        assert_ne!(cache.texture.as_ref().unwrap().id(), first);
        cache.update(&ctx, None);
        assert!(cache.texture.is_none());
    }

    #[test]
    fn malformed_or_oversized_preview_never_keeps_a_previous_texture() {
        let ctx = egui::Context::default();
        let mut cache = PreviewCache::default();
        let mut preview = preview();
        cache.update(&ctx, Some(&preview));
        preview.reference.version = 2;
        preview.png = vec![0; 24];
        cache.update(&ctx, Some(&preview));
        assert!(cache.texture.is_none());
        let rejected = cache.key.clone();
        cache.update(&ctx, Some(&preview));
        assert_eq!(cache.key, rejected);
        preview.reference.version = 3;
        preview.png = vec![0; 512 * 1024 + 1];
        cache.update(&ctx, Some(&preview));
        assert!(cache.texture.is_none());
    }
}
