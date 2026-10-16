//! Manufacture workspace: CNC setup editing, CAM companion integration, toolpath overlay.
//!
//! Modelled on [`crate::render_workspace`]: four sub-panels (SETUP, OPERATIONS, SIMULATE, POST),
//! persisted setup editing backed by the `manufacture.settings.*` engine commands, a companion
//! HTTP backend (`CompanionCamBackend`) that is cross-platform (native + wasm via `ehttp`),
//! toolpath line overlays drawn over the shared viewport, and a simulation scrubber.
//!
//! # Transport
//! [`CompanionCamBackend`] uses [`ehttp`] for all HTTP communication.  It never blocks; instead
//! it queues an HTTP action per [`CamBackend::pump`] call (at most twice per second).  The
//! workspace [`tick`] function calls [`CamHost::pump`] on every frame while a job is live.
//!
//! # URL security
//! [`parse_companion_url`] enforces that the endpoint is `http://` (not https), contains no
//! userinfo (`@`), and has a host that is exactly `localhost`, `127.0.0.1`, or `[::1]`.
//! An explicit port is required.  The resulting [`ParsedCompanionUrl`] carries only the
//! normalised origin (`http://HOST:PORT`).
//!
//! # Session state that is NOT persisted
//! - `cam_endpoint` / `cam_token`: cleared on document open; `cam_token` is never written to
//!   storage.  The default endpoint is `http://127.0.0.1:38473`.
//! - The running job handle and its toolpath result.
//!
//! # Workspace switching
//! `active(app)` tests `app.ui.tab == "MANUFACTURE"`.  `enter` / `exit` gate on idle model state.

#![allow(clippy::too_many_lines)] // panel functions are inevitably verbose

use std::sync::{Arc, Mutex};

use egui::{Color32, Stroke};
use serde_json::json;
use solidcraft_manufacture::{
    Severity, ValidationIssue,
    job::{
        CamBackend, CamBackendDescriptor, CamErrorCode, CamHandle, CamHost, CamJobError, CamJobId, CamJobInput, CamJobResult, CamPhase,
        CamProvenance, CamResult,
    },
};
use web_time::{Duration, Instant};

use crate::theme::Tokens;
use crate::{SolveApp, viewport};

#[cfg(test)]
#[path = "manufacture_workspace_tests.rs"]
mod tests;

// ── Sub-panel tabs ─────────────────────────────────────────────────────────────

/// Sub-panels of the Manufacture workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MfgTab {
    #[default]
    Setup,
    Tools,
    Operations,
    Simulate,
    Post,
}

/// Fusion Manufacture workspace tab (ribbon content switches).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CamMode {
    #[default]
    Milling,
    Turning,
    Additive,
    Inspection,
    Fabrication,
}

// ── Default companion endpoint ─────────────────────────────────────────────────

/// Default companion loopback endpoint.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:38473";

/// Maximum bytes accepted from the companion in a single response body.
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// Minimum interval between HTTP poll requests (500 ms → at most 2/sec).
const POLL_MIN_INTERVAL: Duration = Duration::from_millis(500);

// ── State ─────────────────────────────────────────────────────────────────────

/// Per-session manufacture workspace state.  Non-persistent companion / job fields are reset on
/// document open.
pub struct State {
    // Setup editing
    /// Name of the currently active setup (key into `ManufactureSettings::setups`).
    pub active_setup: String,
    /// Which sub-panel is displayed in the right panel.
    pub tab: MfgTab,
    /// Fusion Manufacture MILLING / TURNING workspace tab.
    pub cam_mode: CamMode,
    draft_dirty: bool,
    pub(crate) revision: u64,

    // CAM session — UI-only; not persisted
    /// Companion endpoint URL (default: `http://127.0.0.1:38473`).
    pub cam_endpoint: String,
    /// Bearer token sent in `Authorization: Bearer <token>`.  Never stored on disk.
    pub cam_token: String,
    /// ID of the currently running or most recently finished job (engine-serialised form).
    /// Last CAM job id payload (`{"job": …}`) for simulate / post.
    pub cam_job_id: Option<serde_json::Value>,

    // Toolpath result for overlay / simulate / post
    /// Normalised path segments from the last completed job.
    pub toolpath_segments: Vec<solidcraft_manufacture::PathSegment>,
    /// Scrubber position (segments 0..=simulate_step are drawn).
    pub simulate_step: usize,
    /// Post-processed G-code text from the last completed job.
    pub generated_code: Option<String>,

    // Preflight / validation
    /// Latest soft-validation issues for the active setup.
    pub last_issues: Vec<ValidationIssue>,
    /// Last `manufacture.simulate` JSON result (runtime only).
    pub last_sim: Option<serde_json::Value>,

    /// Active machine plugin id for the UI picker (`none` / `fake` / `mtconnect`).
    pub machine_plugin: String,
    /// MTConnect base URL when using the live/fixture plugin (loopback).
    pub machine_endpoint: String,
    /// Last `manufacture.machine.status` payload (session only).
    pub last_machine_status: Option<serde_json::Value>,
    /// Last machine plugin error string for the panel.
    pub last_machine_error: Option<String>,
    /// Selected tool index in the Tools tab (into the active setup's tool library).
    pub selected_tool: Option<usize>,
    /// Selected operation index in the Operations browser.
    pub selected_op: Option<usize>,
    /// Open Fusion-style drill inspector (session only).
    pub drill_dialog: Option<crate::manufacture_drill::DrillDialogState>,
    /// Open Fusion-style 2D Contour inspector (session only).
    pub contour_dialog: Option<crate::manufacture_contour::ContourDialogState>,
    /// Open Fusion-style New Setup / Edit Setup dialog.
    pub setup_dialog: Option<crate::manufacture_setup::SetupDialogState>,
    /// Open secondary SETUP menu dialog (NC Program / Folder / Pattern / …).
    pub menu_dialog: Option<crate::manufacture_setup::MenuDialogState>,
    /// Open Fusion-style 2D op dialog (Face / Adaptive / Pocket / Slot / …).
    pub twod_dialog: Option<crate::manufacture_2d::TwodDialogState>,
    /// Open Fusion-style 3D milling dialog (Adaptive / Scallop / Parallel / …).
    pub threed_dialog: Option<crate::manufacture_3d::ThreedDialogState>,
    /// Open Fusion-style Hole Recognition dialog (Hole Groups / Libraries / Options).
    pub hole_rec_dialog: Option<crate::manufacture_hole_recognition::HoleRecDialogState>,
    /// Open Fusion-style Multi-Axis dialog (Swarf / Rotary / Finishing / …).
    pub multiaxis_dialog: Option<crate::manufacture_multiaxis::MultiAxisDialogState>,
    /// Open Fusion-style MODIFY dialog (Trim / Delete Passes / …).
    pub modify_dialog: Option<crate::manufacture_modify::ModifyDialogState>,
    /// Fusion NC Program / Post Process dialog.
    pub post_dialog: Option<crate::manufacture_post::PostDialogState>,
    /// Machining Time estimate dialog.
    pub machining_time_dialog: Option<crate::manufacture_post::MachiningTimeState>,
    /// Setup Sheet preview dialog.
    pub setup_sheet_dialog: Option<crate::manufacture_post::SetupSheetState>,
    /// Fusion Tool Library browser dialog.
    pub tool_library_dialog: Option<crate::manufacture_tools::ToolLibraryState>,
    /// Fusion Edit Tool dialog (General / Cutter / Shaft / Holder / Cutting / Post).
    pub edit_tool_dialog: Option<crate::manufacture_tools::EditToolState>,
    /// MANAGE ribbon placeholder dialogs (Form Mill, libraries, defaults).
    pub manage_stub_dialog: Option<crate::manufacture_tools::ManageStubState>,
    /// Fusion Inspect → Sustainability Insights dialog.
    pub sustainability_dialog: Option<crate::manufacture_inspect::SustainabilityState>,
    /// Fusion Turning operation dialog (Tool / Geometry / Radii / Passes / Linking).
    pub turning_dialog: Option<crate::manufacture_turning::TurningDialogState>,
    /// Fusion Part Handling dialog (Subspindle Grab / Bar Pull / Return / Tool Call).
    pub part_handling_dialog: Option<crate::manufacture_part_handling::PartHandlingDialogState>,
    /// Fusion Additive toolpath dialog.
    pub additive_dialog: Option<crate::manufacture_additive::AdditiveDialogState>,
    /// New Setup from G-Code dialog.
    pub from_gcode_dialog: Option<crate::manufacture_additive::FromGcodeState>,
    /// Fusion Inspection → Probing dialog.
    pub inspection_dialog: Option<crate::manufacture_inspection::InspectionDialogState>,
    /// Fusion Inspection → Manual dialog.
    pub manual_inspection_dialog: Option<crate::manufacture_inspection::ManualInspectionDialogState>,
    /// Fusion Fabrication → 2D Profile dialog.
    pub fabrication_dialog: Option<crate::manufacture_fabrication::FabricationDialogState>,
    /// Fusion Fabrication → Nest dialog.
    pub nest_dialog: Option<crate::manufacture_fabrication::NestDialogState>,
    /// Machine / Post / Template Library browser (with linked remotes).
    pub library_dialog: Option<crate::manufacture_libraries::LibraryDialogState>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            active_setup: String::new(),
            tab: MfgTab::default(),
            cam_mode: CamMode::default(),
            draft_dirty: false,
            revision: u64::MAX,
            cam_endpoint: DEFAULT_ENDPOINT.into(),
            cam_token: String::new(),
            cam_job_id: None,
            toolpath_segments: Vec::new(),
            simulate_step: 0,
            generated_code: None,
            last_issues: Vec::new(),
            last_sim: None,
            machine_plugin: "none".into(),
            machine_endpoint: "http://127.0.0.1:8082/VF-2".into(),
            last_machine_status: None,
            last_machine_error: None,
            selected_tool: None,
            selected_op: None,
            drill_dialog: None,
            contour_dialog: None,
            setup_dialog: None,
            menu_dialog: None,
            twod_dialog: None,
            threed_dialog: None,
            hole_rec_dialog: None,
            multiaxis_dialog: None,
            modify_dialog: None,
            post_dialog: None,
            machining_time_dialog: None,
            setup_sheet_dialog: None,
            tool_library_dialog: None,
            edit_tool_dialog: None,
            manage_stub_dialog: None,
            sustainability_dialog: None,
            turning_dialog: None,
            part_handling_dialog: None,
            additive_dialog: None,
            from_gcode_dialog: None,
            inspection_dialog: None,
            manual_inspection_dialog: None,
            fabrication_dialog: None,
            nest_dialog: None,
            library_dialog: None,
        }
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

/// Whether the app is currently in the Manufacture workspace.
pub fn active(app: &SolveApp) -> bool {
    app.ui.tab == "MANUFACTURE"
}

/// Cache key for the setup isolate (which bodies the viewport keeps).
pub fn setup_body_key(app: &SolveApp) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    active(app).hash(&mut h);
    app.manufacture_workspace.active_setup.hash(&mut h);
    if let Some(d) = &app.manufacture_workspace.setup_dialog {
        d.picking.hash(&mut h);
        d.draft.model_bodies.hash(&mut h);
    } else if let Some(s) = app.session.doc.manufacture.setups.get(&app.manufacture_workspace.active_setup) {
        s.model_bodies.hash(&mut h);
    }
    h.finish()
}

/// Bodies outside the selected setup stay hidden. While the setup dialog is picking, every body stays visible so more can be added.
pub fn setup_shows_body(app: &SolveApp, name: &str) -> bool {
    if !active(app) {
        return true;
    }
    if app.manufacture_workspace.setup_dialog.as_ref().is_some_and(|d| d.picking) {
        return true;
    }
    let bodies = if let Some(d) = &app.manufacture_workspace.setup_dialog {
        &d.draft.model_bodies
    } else {
        match app.session.doc.manufacture.setups.get(&app.manufacture_workspace.active_setup) {
            Some(s) => &s.model_bodies,
            None => return true,
        }
    };
    bodies.is_empty() || bodies.iter().any(|n| n == name)
}

/// Switch to the Manufacture workspace; refuses while modelling is active.
pub fn enter(app: &mut SolveApp) {
    if app.session.active_sketch.is_some() || app.tool.is_some() || app.dialog.is_some() || app.file_load.busy() {
        app.set_status(app.tr("manufacture.enter_blocked").to_string(), true);
        return;
    }
    app.ui.tab = "MANUFACTURE".into();
    ensure_setup(app);
    apply_companion_backend(app);
}

/// Ribbon / palette hook for Manufacture commands. Returns `false` when the click was handled
/// here (do not fall through to a bare engine `run` with empty params).
pub fn start_hook(app: &mut SolveApp, id: &str) -> bool {
    if id == "inspect.sustainability" {
        crate::manufacture_inspect::open_sustainability(app);
        return false;
    }
    if !id.starts_with("manufacture.") {
        return true;
    }
    if !active(app) {
        enter(app);
        if !active(app) {
            return false;
        }
    }
    ensure_setup(app);

    match id {
        "manufacture.setup.new" => {
            crate::manufacture_setup::open_new(app);
            app.set_status(app.tr("manufacture.ribbon.setup_dialog").to_string(), false);
            false
        }
        "manufacture.setup.focus" | "manufacture.machines" => {
            let name = app.manufacture_workspace.active_setup.clone();
            if !name.is_empty() && id == "manufacture.setup.focus" {
                crate::manufacture_setup::open_edit(app, &name);
            } else {
                app.manufacture_workspace.tab = MfgTab::Setup;
            }
            app.set_status(app.tr("manufacture.ribbon.setup").to_string(), false);
            false
        }
        "manufacture.nc_program.new" => {
            crate::manufacture_setup::open_menu(app, crate::manufacture_setup::SetupMenuDialog::NcProgram);
            false
        }
        "manufacture.folder.new" => {
            crate::manufacture_setup::open_menu(app, crate::manufacture_setup::SetupMenuDialog::Folder);
            false
        }
        "manufacture.pattern.new" => {
            crate::manufacture_setup::open_menu(app, crate::manufacture_setup::SetupMenuDialog::Pattern);
            false
        }
        "manufacture.manual_nc.new" => {
            crate::manufacture_setup::open_menu(app, crate::manufacture_setup::SetupMenuDialog::ManualNc);
            false
        }
        "manufacture.nest" => {
            app.manufacture_workspace.cam_mode = CamMode::Fabrication;
            crate::manufacture_fabrication::open_nest(app);
            false
        }
        "manufacture.setup.from_gcode" => {
            app.manufacture_workspace.cam_mode = CamMode::Additive;
            crate::manufacture_additive::open_from_gcode(app);
            false
        }
        "manufacture.additive.simulate" => {
            app.manufacture_workspace.cam_mode = CamMode::Additive;
            let setup = app.manufacture_workspace.active_setup.clone();
            crate::manufacture_additive::run_simulate(app, &setup);
            false
        }
        "manufacture.insert_data" => {
            crate::manufacture_setup::open_menu(app, crate::manufacture_setup::SetupMenuDialog::InsertData);
            false
        }
        "manufacture.mfg_model.new" => {
            crate::manufacture_setup::open_menu(app, crate::manufacture_setup::SetupMenuDialog::MfgModel);
            false
        }
        "manufacture.modify.trim" => {
            crate::manufacture_modify::open(app, crate::manufacture_modify::ModifyDialogKind::Trim);
            false
        }
        "manufacture.modify.delete_passes" => {
            crate::manufacture_modify::open(app, crate::manufacture_modify::ModifyDialogKind::DeletePasses);
            false
        }
        "manufacture.modify.leads_links" => {
            crate::manufacture_modify::open(app, crate::manufacture_modify::ModifyDialogKind::LeadsAndLinks);
            false
        }
        "manufacture.modify.replace_tool" => {
            crate::manufacture_modify::open(app, crate::manufacture_modify::ModifyDialogKind::ReplaceTool);
            false
        }
        "manufacture.modify.move_entry" => {
            crate::manufacture_modify::open(app, crate::manufacture_modify::ModifyDialogKind::MoveEntryPositions);
            false
        }
        "manufacture.tools" => {
            crate::manufacture_tools::open_tool_library(app);
            false
        }
        "manufacture.manage.form_mill" => {
            crate::manufacture_tools::open_manage_stub(app, crate::manufacture_tools::ManageStubKind::FormMill);
            false
        }
        "manufacture.manage.solid_holder" => {
            crate::manufacture_tools::open_manage_stub(app, crate::manufacture_tools::ManageStubKind::SolidHolder);
            false
        }
        "manufacture.manage.machine_library" => {
            crate::manufacture_libraries::open(app, crate::manufacture_libraries::LibraryDialogKind::Machine);
            false
        }
        "manufacture.manage.post_library" => {
            crate::manufacture_libraries::open(app, crate::manufacture_libraries::LibraryDialogKind::Post);
            false
        }
        "manufacture.manage.template_library" => {
            crate::manufacture_libraries::open(app, crate::manufacture_libraries::LibraryDialogKind::Template);
            false
        }
        "manufacture.manage.task_manager" => {
            crate::manufacture_tools::open_manage_stub(app, crate::manufacture_tools::ManageStubKind::TaskManager);
            false
        }
        "manufacture.manage.export_defaults" => {
            crate::manufacture_tools::open_manage_stub(app, crate::manufacture_tools::ManageStubKind::ExportDefaults);
            false
        }
        "manufacture.manage.import_defaults" => {
            crate::manufacture_tools::open_manage_stub(app, crate::manufacture_tools::ManageStubKind::ImportDefaults);
            false
        }
        "manufacture.manage.reset_defaults" => {
            crate::manufacture_tools::open_manage_stub(app, crate::manufacture_tools::ManageStubKind::ResetDefaults);
            false
        }
        "manufacture.generate" => {
            app.manufacture_workspace.tab = MfgTab::Operations;
            start_cam_job(app);
            false
        }
        "manufacture.simulate.ui" => {
            app.manufacture_workspace.tab = MfgTab::Simulate;
            if !app.manufacture_workspace.toolpath_segments.is_empty() {
                run_simulation(app);
            } else {
                app.set_status(app.tr("manufacture.sim.no_toolpath").to_string(), true);
            }
            false
        }
        "manufacture.simulate.machine" => {
            app.manufacture_workspace.tab = MfgTab::Simulate;
            if !app.manufacture_workspace.toolpath_segments.is_empty() {
                run_simulation_with_machine(app);
            } else {
                app.set_status(app.tr("manufacture.sim.no_toolpath").to_string(), true);
            }
            false
        }
        "manufacture.post.ui" => {
            crate::manufacture_post::open_post_process(app);
            false
        }
        "manufacture.setup_sheet" => {
            crate::manufacture_post::open_setup_sheet(app);
            false
        }
        "manufacture.machining_time" => {
            crate::manufacture_post::open_machining_time(app);
            false
        }
        "manufacture.op.face"
        | "manufacture.op.adaptive"
        | "manufacture.op.pocket"
        | "manufacture.op.profile"
        | "manufacture.op.slot"
        | "manufacture.op.trace"
        | "manufacture.op.thread"
        | "manufacture.op.bore"
        | "manufacture.op.circular"
        | "manufacture.op.engrave"
        | "manufacture.op.chamfer"
        | "manufacture.op.drill"
        | "manufacture.op.3d_adaptive"
        | "manufacture.op.3d_pocket"
        | "manufacture.op.3d_3plus2"
        | "manufacture.op.3d_steep_shallow"
        | "manufacture.op.3d_flat"
        | "manufacture.op.3d_parallel"
        | "manufacture.op.3d_scallop"
        | "manufacture.op.3d_contour"
        | "manufacture.op.3d_ramp"
        | "manufacture.op.3d_pencil"
        | "manufacture.op.3d_horizontal"
        | "manufacture.op.3d_spiral"
        | "manufacture.op.3d_radial"
        | "manufacture.op.3d_morphed_spiral"
        | "manufacture.op.3d_project"
        | "manufacture.op.3d_blend"
        | "manufacture.op.3d_morph"
        | "manufacture.op.3d_corner"
        | "manufacture.op.3d_flow"
        | "manufacture.op.3d_deburr"
        | "manufacture.op.3d_geodesic"
        | "manufacture.op.hole_recognition"
        | "manufacture.op.ma_rotary_pocket"
        | "manufacture.op.ma_roughing"
        | "manufacture.op.ma_swarf"
        | "manufacture.op.ma_adv_swarf"
        | "manufacture.op.ma_contour"
        | "manufacture.op.ma_rotary_parallel"
        | "manufacture.op.ma_rotary_contour"
        | "manufacture.op.ma_finishing"
        | "manufacture.op.turn_face"
        | "manufacture.op.turn_profile_rough"
        | "manufacture.op.turn_profile_finish"
        | "manufacture.op.turn_groove_rough"
        | "manufacture.op.turn_groove_finish"
        | "manufacture.op.turn_adaptive"
        | "manufacture.op.turn_groove"
        | "manufacture.op.turn_single_groove"
        | "manufacture.op.turn_thread"
        | "manufacture.op.turn_chamfer"
        | "manufacture.op.turn_part"
        | "manufacture.op.turn_trace"
        | "manufacture.part_handling.subspindle_grab"
        | "manufacture.part_handling.bar_pull"
        | "manufacture.part_handling.subspindle_return"
        | "manufacture.part_handling.tool_call"
        | "manufacture.op.probe_wcs"
        | "manufacture.op.probe_geometry"
        | "manufacture.op.inspect_surface"
        | "manufacture.op.part_alignment"
        | "manufacture.op.manual_inspection"
        | "manufacture.op.manual_inspection_record"
        | "manufacture.op.fab_2d_profile" => {
            if id.starts_with("manufacture.op.turn_") || id.starts_with("manufacture.part_handling.") {
                app.manufacture_workspace.cam_mode = CamMode::Turning;
            }
            if id.contains("probe") || id.contains("inspect_surface") || id.contains("part_alignment") || id.contains("manual_inspection") {
                app.manufacture_workspace.cam_mode = CamMode::Inspection;
            }
            if id.contains("fab_2d_profile") {
                app.manufacture_workspace.cam_mode = CamMode::Fabrication;
            }
            let setup = app.manufacture_workspace.active_setup.clone();
            let params = if setup.is_empty() { json!({}) } else { json!({"setup": setup}) };
            match app.run(id, params) {
                Ok(_) => {
                    app.manufacture_workspace.tab = MfgTab::Operations;
                    app.manufacture_workspace.revision = u64::MAX;
                    app.manufacture_workspace.last_issues = app.session.doc.manufacture.issues();
                    if let Some(su) = app.session.doc.manufacture.setups.get(&setup) {
                        let idx = su.operations.len().saturating_sub(1);
                        app.manufacture_workspace.selected_op = Some(idx);
                        open_op_dialog(app, &setup, idx);
                    }
                    app.set_status(app.tr("manufacture.ribbon.op_added").to_string(), false);
                }
                Err(e) => app.set_status(e, true),
            }
            false
        }
        _ => true,
    }
}

fn open_op_dialog(app: &mut SolveApp, setup_name: &str, op_index: usize) {
    let Some(setup) = app.session.doc.manufacture.setups.get(setup_name) else {
        return;
    };
    let Some(op) = setup.operations.get(op_index) else {
        return;
    };
    match op {
        solidcraft_manufacture::Operation::Drill(_) => crate::manufacture_drill::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::HoleRecognition(_) => crate::manufacture_hole_recognition::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::Profile(_) => crate::manufacture_contour::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::Threed(_) => crate::manufacture_3d::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::MultiAxis(_) => crate::manufacture_multiaxis::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::Turning(_) => crate::manufacture_turning::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::PartHandling(_) => crate::manufacture_part_handling::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::Additive(_) => crate::manufacture_additive::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::Inspection(_) => crate::manufacture_inspection::open_inspection(app, setup_name, op_index),
        solidcraft_manufacture::Operation::ManualInspection(_) => crate::manufacture_inspection::open_manual(app, setup_name, op_index),
        solidcraft_manufacture::Operation::FabricationProfile(_) => crate::manufacture_fabrication::open(app, setup_name, op_index),
        solidcraft_manufacture::Operation::Face(_)
        | solidcraft_manufacture::Operation::Adaptive(_)
        | solidcraft_manufacture::Operation::Pocket(_)
        | solidcraft_manufacture::Operation::Slot(_)
        | solidcraft_manufacture::Operation::Trace(_)
        | solidcraft_manufacture::Operation::Thread(_)
        | solidcraft_manufacture::Operation::Bore(_)
        | solidcraft_manufacture::Operation::Circular(_)
        | solidcraft_manufacture::Operation::Engrave(_)
        | solidcraft_manufacture::Operation::Chamfer(_) => {
            crate::manufacture_2d::open(app, setup_name, op_index);
        }
    }
}

#[allow(dead_code)]
fn request_gcode_preview(app: &mut SolveApp) {
    let Some(job_wrap) = app.manufacture_workspace.cam_job_id.clone() else {
        app.set_status(app.tr("manufacture.post.no_job").to_string(), true);
        return;
    };
    let job = job_wrap.get("job").cloned().unwrap_or(job_wrap);
    match app.run("manufacture.gcode", json!({"job": job})) {
        Ok(v) => {
            if let Some(code) = v.get("gcode").and_then(|g| g.as_str()) {
                app.manufacture_workspace.generated_code = Some(code.to_string());
            }
            app.set_status(app.tr("manufacture.post.ready").to_string(), false);
        }
        Err(e) => app.set_status(e, true),
    }
}

/// Return to the SOLID workspace.
pub fn exit(app: &mut SolveApp) {
    app.ui.tab = "SOLID".into();
}

/// Clear transient state on document open/new.
pub fn reset_document(app: &mut SolveApp) {
    // Keep endpoint; clear token and job state.
    let endpoint = std::mem::take(&mut app.manufacture_workspace.cam_endpoint);
    app.manufacture_workspace = State { cam_endpoint: endpoint, ..State::default() };
    if active(app) {
        app.ui.tab = "SOLID".into();
    }
}

/// Per-frame tick: drive the HTTP state machine, check background job completion,
/// and request a repaint while busy.
///
/// Pump is always called first so the HTTP client makes progress even when the
/// job transitions to terminal on the same frame.
pub fn tick(app: &mut SolveApp, ctx: &egui::Context) {
    // Drive the companion HTTP state machine (no-op when no job is live).
    let host = app.session.manufacture_job_host().cloned();
    if let Some(h) = host {
        h.pump();
    }

    // Check job status via the in-memory engine command (never triggers network calls).
    let Some(id) = app.manufacture_workspace.cam_job_id.clone() else { return };

    let phase_str: Option<String> = match app.session.execute("manufacture.status", &id) {
        Ok(status) => status["progress"]["phase"].as_str().map(str::to_string),
        Err(_) => return, // job no longer in registry
    };

    let terminal = phase_str.as_deref().is_none_or(|p| matches!(p, "completed" | "failed" | "cancelled"));
    if !terminal {
        ctx.request_repaint();
        return;
    }

    // Auto-pull results when newly completed and not yet extracted.
    if phase_str.as_deref() == Some("completed")
        && app.manufacture_workspace.toolpath_segments.is_empty()
        && app.manufacture_workspace.generated_code.is_none()
    {
        match app.session.execute("manufacture.result", &id) {
            Ok(v) => {
                if let Some(segs) = v.get("segments").and_then(|s| serde_json::from_value(s.clone()).ok()) {
                    app.manufacture_workspace.toolpath_segments = segs;
                    app.manufacture_workspace.simulate_step = app.manufacture_workspace.toolpath_segments.len();
                }
                if let Some(code) = v.get("post_output").and_then(|c| c.as_str()).map(str::to_string) {
                    app.manufacture_workspace.generated_code = Some(code);
                } else if let Ok(g) = app.session.execute("manufacture.gcode", &id)
                    && let Some(code) = g.get("gcode").and_then(|c| c.as_str()).map(str::to_string)
                {
                    app.manufacture_workspace.generated_code = Some(code);
                }
            }
            Err(e) => app.set_status(e.to_string(), true),
        }
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Make sure `active_setup` is valid; pick the first available setup if empty or stale.
pub(crate) fn ensure_setup(app: &mut SolveApp) {
    let mfg = &app.session.doc.manufacture;
    if !mfg.setups.contains_key(&app.manufacture_workspace.active_setup) {
        app.manufacture_workspace.active_setup = mfg.setups.keys().next().cloned().unwrap_or_default();
    }
    app.manufacture_workspace.last_issues = mfg.issues();
    app.manufacture_workspace.revision = app.session.revision;
}

/// Re-sync setup list and issues when the document revision advances.
fn maybe_sync(app: &mut SolveApp) {
    if app.manufacture_workspace.revision != app.session.revision {
        ensure_setup(app);
    }
}

// ── Floating dialogs (no sticky right aside) ───────────────────────────────────

/// Manufacture uses Fusion-style floating dialogs + Browser setups — not a right dock.
/// Setup / Tool Library / Post / ops open as drag-and-drop windows; dismiss with Cancel/Close/X.
pub fn panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    maybe_sync(app);
    let _ = crate::manufacture_drill::show_dialog(app, ui);
    let _ = crate::manufacture_contour::show_dialog(app, ui);
    crate::manufacture_setup::show_dialogs(app, ui);
    let _ = crate::manufacture_2d::show_dialog(app, ui);
    let _ = crate::manufacture_3d::show_dialog(app, ui);
    let _ = crate::manufacture_hole_recognition::show_dialog(app, ui);
    let _ = crate::manufacture_multiaxis::show_dialog(app, ui);
    let _ = crate::manufacture_modify::show_dialog(app, ui);
    crate::manufacture_post::show_dialogs(app, ui);
    crate::manufacture_tools::show_dialogs(app, ui);
    crate::manufacture_inspect::show_dialogs(app, ui);
    let _ = crate::manufacture_turning::show_dialog(app, ui);
    let _ = crate::manufacture_part_handling::show_dialog(app, ui);
    crate::manufacture_additive::show_dialogs(app, ui);
    crate::manufacture_inspection::show_dialogs(app, ui);
    crate::manufacture_fabrication::show_dialogs(app, ui);
    crate::manufacture_libraries::show_dialogs(app, ui);
}

/// Browser tree: **Setups** → each setup → operations the ribbon added (Face, Drill, 3D Contour, …).
pub fn browser_setup_rows(app: &mut SolveApp, ui: &mut egui::Ui, depth: usize) {
    use crate::browser::{Row, draw_row, is_open, toggle};

    let folder_key = "mfg/setups";
    let folder_open = is_open(app, folder_key, true);
    let folder_row = Row { depth, fold: Some(folder_open), icon: "folder", label: "Setups", ..Default::default() };
    let fr = draw_row(ui, ui.id().with("mfg_setups_folder"), &folder_row);
    if fr.fold {
        toggle(app, folder_key, true);
    }
    if fr.double {
        crate::manufacture_setup::open_new(app);
    }
    if !is_open(app, folder_key, true) {
        return;
    }

    let mut names: Vec<String> = app.session.doc.manufacture.setups.keys().cloned().collect();
    names.sort();
    let active = app.manufacture_workspace.active_setup.clone();

    for name in names {
        let setup_key = format!("mfg/setup/{name}");
        let setup_open = is_open(app, &setup_key, true);
        let selected = active == name;
        let label = name.clone();
        let sr = draw_row(
            ui,
            ui.id().with(("mfg_setup", name.as_str())),
            &Row { depth: depth + 1, fold: Some(setup_open), icon: "box", label: &label, name: true, selected, ..Default::default() },
        );
        if sr.fold {
            toggle(app, &setup_key, true);
        }
        if sr.clicked {
            app.manufacture_workspace.active_setup = name.clone();
            app.manufacture_workspace.selected_op = None;
            app.manufacture_workspace.last_issues = app.session.doc.manufacture.issues();
        }
        if sr.double {
            app.manufacture_workspace.active_setup = name.clone();
            crate::manufacture_setup::open_edit(app, &name);
        }
        if !is_open(app, &setup_key, true) {
            continue;
        }

        let Some(setup) = app.session.doc.manufacture.setups.get(&name).cloned() else {
            continue;
        };
        for (i, op) in setup.operations.iter().enumerate() {
            let tnum = op.tool_number();
            let op_label = format!("[T{tnum}] {}", op.display_name());
            let op_sel = active == name && app.manufacture_workspace.selected_op == Some(i);
            let orow = draw_row(
                ui,
                ui.id().with(("mfg_op", name.as_str(), i)),
                &Row { depth: depth + 2, icon: "extrude", label: &op_label, name: true, selected: op_sel, ..Default::default() },
            );
            if orow.clicked || orow.double {
                app.manufacture_workspace.active_setup = name.clone();
                app.manufacture_workspace.selected_op = Some(i);
                open_op_dialog(app, &name, i);
            }
            // Nested tool line (Fusion-style under the op).
            if let Some(tool) = setup.tools.iter().find(|t| t.number == tnum) {
                let desc = if tool.description.is_empty() { tool_kind_label(tool.kind) } else { tool.description.as_str() };
                let tool_label = format!("#{tnum} - ø{:.2}mm {desc}", tool.diameter);
                let _ = draw_row(
                    ui,
                    ui.id().with(("mfg_op_tool", name.as_str(), i)),
                    &Row { depth: depth + 3, icon: "hole", label: &tool_label, name: true, dim: true, ..Default::default() },
                );
            }
        }
    }

    if app.session.doc.manufacture.setups.is_empty() {
        let hint = draw_row(
            ui,
            ui.id().with("mfg_setups_empty"),
            &Row { depth: depth + 1, icon: "settings", label: "New Setup…", dim: true, ..Default::default() },
        );
        if hint.clicked || hint.double {
            crate::manufacture_setup::open_new(app);
        }
    }
}

// ── Legacy dock panels (kept for CAM controls / later floating panels) ─────────
#[allow(dead_code)]
fn setup_panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.heading(app.tr("manufacture.setup.heading"));

    // Setup selector
    {
        let names: Vec<String> = app.session.doc.manufacture.setups.keys().cloned().collect();
        let current = app.manufacture_workspace.active_setup.clone();
        let label = if current.is_empty() { app.tr("manufacture.setup.none").to_string() } else { current.clone() };
        egui::ComboBox::from_id_salt("mfg_setup_sel").selected_text(label).show_ui(ui, |ui| {
            for name in &names {
                if ui.selectable_label(*name == current, name).clicked() {
                    app.manufacture_workspace.active_setup = name.clone();
                    app.manufacture_workspace.draft_dirty = false;
                    app.manufacture_workspace.last_issues = app.session.doc.manufacture.issues();
                }
            }
        });
    }

    ui.horizontal(|ui| {
        if ui.button(app.tr("manufacture.setup.new")).clicked() {
            crate::manufacture_setup::open_new(app);
        }
        let can_edit = !app.manufacture_workspace.active_setup.is_empty();
        if ui.add_enabled(can_edit, egui::Button::new(app.tr("manufacture.setup.edit"))).clicked() {
            let name = app.manufacture_workspace.active_setup.clone();
            crate::manufacture_setup::open_edit(app, &name);
        }
        let can_delete = !app.manufacture_workspace.active_setup.is_empty();
        if ui.add_enabled(can_delete, egui::Button::new(app.tr("manufacture.setup.delete"))).clicked() {
            delete_active_setup(app);
        }
    });

    // SETUP menu containers summary
    let mfg = &app.session.doc.manufacture;
    if !mfg.nc_programs.is_empty()
        || !mfg.folders.is_empty()
        || !mfg.patterns.is_empty()
        || !mfg.manual_nc.is_empty()
        || !mfg.manufacturing_models.is_empty()
    {
        ui.separator();
        ui.label(app.tr("manufacture.setup.containers"));
        if !mfg.nc_programs.is_empty() {
            ui.small(format!("NC Programs: {}", mfg.nc_programs.len()));
        }
        if !mfg.folders.is_empty() {
            ui.small(format!("Folders: {}", mfg.folders.len()));
        }
        if !mfg.patterns.is_empty() {
            ui.small(format!("Patterns: {}", mfg.patterns.len()));
        }
        if !mfg.manual_nc.is_empty() {
            ui.small(format!("Manual NC: {}", mfg.manual_nc.len()));
        }
        if !mfg.manufacturing_models.is_empty() {
            ui.small(format!("Mfg Models: {}", mfg.manufacturing_models.len()));
        }
    }

    ui.separator();

    // Stock
    let setup_name = app.manufacture_workspace.active_setup.clone();
    if setup_name.is_empty() {
        ui.label(app.tr("manufacture.setup.no_setup"));
    } else {
        let Some(setup) = app.session.doc.manufacture.setups.get(&setup_name).cloned() else {
            ui.label(app.tr("manufacture.setup.not_found"));
            return;
        };
        stock_section(app, ui, &setup);
        ui.separator();
        wcs_section(app, ui, &setup);
    }

    ui.separator();
    ui.heading(app.tr("manufacture.backend.heading"));
    ui.label(app.tr("manufacture.backend.endpoint"));

    let endpoint_changed = ui.text_edit_singleline(&mut app.manufacture_workspace.cam_endpoint).changed();
    ui.label(app.tr("manufacture.backend.token"));
    let token_changed = ui.add(egui::TextEdit::singleline(&mut app.manufacture_workspace.cam_token).password(true)).changed();
    if endpoint_changed || token_changed {
        apply_companion_backend(app);
    }

    // CORS hint for browser (WASM) use.
    #[cfg(target_arch = "wasm32")]
    {
        ui.separator();
        ui.small(app.tr("manufacture.backend.cors_hint"));
        let origin = app.manufacture_workspace.cam_endpoint.clone();
        if let Ok(parsed) = parse_companion_url(&origin) {
            // Show what --origin value the user should pass to solidcraft-cam.
            ui.small(format!("solidcraft-cam --origin {}", parsed.origin));
        }
    }

    // Issues
    let issues = app.manufacture_workspace.last_issues.clone();
    if !issues.is_empty() {
        ui.separator();
        ui.label(app.tr("manufacture.issues.heading"));
        for issue in &issues {
            let color = if issue.severity == Severity::Error { Color32::from_rgb(220, 80, 80) } else { Color32::from_rgb(220, 190, 60) };
            ui.colored_label(color, format!("{}: {}", issue.path, issue.message));
        }
    }

    ui.separator();
    if app.manufacture_workspace.draft_dirty {
        ui.label(app.tr("manufacture.setup.unsaved"));
    }
    if ui.button(app.tr("manufacture.return_design")).clicked() {
        exit(app);
    }
}

fn stock_section(app: &mut SolveApp, ui: &mut egui::Ui, setup: &solidcraft_manufacture::MfgSetup) {
    ui.heading(app.tr("manufacture.setup.stock"));
    ui.label(setup.stock.mode_label());
    match &setup.stock {
        solidcraft_manufacture::Stock::Box { width, depth, height } => {
            ui.label(format!("{}: {} × {} × {} mm", app.tr("manufacture.stock.box"), width, depth, height));
        }
        solidcraft_manufacture::Stock::RelativeBox { side_offset, top_offset, bottom_offset, .. } => {
            ui.label(format!("Relative +side {side_offset} +top {top_offset} +bottom {bottom_offset} mm"));
        }
        solidcraft_manufacture::Stock::Cylinder { diameter, height } => {
            ui.label(format!("{}: ⌀{} × {} mm", app.tr("manufacture.stock.cylinder"), diameter, height));
        }
        solidcraft_manufacture::Stock::RelativeCylinder { radial_offset, front_offset, back_offset, .. } => {
            ui.label(format!("Relative cyl +r {radial_offset} +front {front_offset} +back {back_offset} mm"));
        }
        solidcraft_manufacture::Stock::Tube { outer_diameter, inner_diameter, height } => {
            ui.label(format!("Tube ⌀{outer_diameter}/⌀{inner_diameter} × {height} mm"));
        }
        solidcraft_manufacture::Stock::FromPreceding { continue_rest } => {
            ui.label(format!("From preceding (rest={continue_rest})"));
        }
    }
    ui.label(format!("{} · {}", setup.operation_type.label(), setup.wcs_origin_mode.label()));
    ui.small(format!("Program {} · WCS offset {}", setup.post_opts.program_name, setup.post_opts.wcs_offset));
    if ui.small_button(app.tr("manufacture.stock.edit")).clicked() {
        let name = app.manufacture_workspace.active_setup.clone();
        crate::manufacture_setup::open_edit(app, &name);
    }
}

fn wcs_section(app: &mut SolveApp, ui: &mut egui::Ui, setup: &solidcraft_manufacture::MfgSetup) {
    ui.heading(app.tr("manufacture.setup.wcs"));
    let wcs = &setup.wcs;
    ui.label(format!("{}: [{:.2}, {:.2}, {:.2}]", app.tr("manufacture.wcs.origin"), wcs.origin[0], wcs.origin[1], wcs.origin[2]));
    ui.label(format!(
        "X: [{:.2}, {:.2}, {:.2}]  Y: [{:.2}, {:.2}, {:.2}]",
        wcs.x_axis[0], wcs.x_axis[1], wcs.x_axis[2], wcs.y_axis[0], wcs.y_axis[1], wcs.y_axis[2]
    ));
    ui.separator();
    machine_profile_section(app, ui, setup);
    ui.separator();
    post_profile_section(app, ui, setup);
}

fn machine_profile_section(app: &mut SolveApp, ui: &mut egui::Ui, setup: &solidcraft_manufacture::MfgSetup) {
    ui.heading(app.tr("manufacture.setup.machine"));
    let current = setup.machine.clone().unwrap_or_default();
    let label = if current.is_empty() { app.tr("manufacture.setup.machine_none").to_string() } else { current.clone() };
    let setup_name = app.manufacture_workspace.active_setup.clone();
    let mut picked: Option<String> = None;
    egui::ComboBox::from_id_salt("mfg_machine_sel").selected_text(&label).show_ui(ui, |ui| {
        if ui.selectable_label(current.is_empty(), app.tr("manufacture.setup.machine_none")).clicked() {
            picked = Some(String::new());
        }
        for (id, _) in solidcraft_manufacture::machines::preset_machines() {
            if ui.selectable_label(current == id, id).clicked() {
                picked = Some(id.to_string());
            }
        }
    });
    if let Some(choice) = picked {
        let _ = app.run("manufacture.machines.ensure_presets", json!({}));
        let set = if choice.is_empty() { json!({"machine": serde_json::Value::Null}) } else { json!({"machine": choice}) };
        let _ = app.run("manufacture.settings.set", json!({"name": setup_name, "set": set}));
        app.manufacture_workspace.revision = u64::MAX;
    }
    if let Some(name) = &setup.machine
        && let Some(m) = app.session.doc.manufacture.machines.get(name)
    {
        let tc = &m.tool_changer;
        ui.small(format!("{}: {:?} · {} {}", app.tr("manufacture.setup.atc"), tc.kind, tc.pockets, app.tr("manufacture.setup.atc_pockets")));
        if tc.change_time_s > 0.0 {
            ui.small(format!(
                "{}: {:.1} s · max ⌀ {:.0} · L {:.0} mm",
                app.tr("manufacture.setup.atc_change"),
                tc.change_time_s,
                tc.max_tool_diameter_mm,
                tc.max_tool_length_mm
            ));
        }
        ui.small(format!("{}: {:?} · {:?}", app.tr("manufacture.setup.integration"), m.integration.kind, m.spindle_taper));
        if m.probe.present {
            ui.small(format!("{}: {:?} · T{}", app.tr("manufacture.setup.probe"), m.probe.style, m.probe.tool_number));
        }
    }
    ui.separator();
    machine_plugin_section(app, ui);
}

fn machine_plugin_section(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.heading(app.tr("manufacture.machine_plugin.heading"));
    ui.small(app.tr("manufacture.machine_plugin.hint"));

    let current = app.manufacture_workspace.machine_plugin.clone();
    let label = match current.as_str() {
        "fake" => app.tr("manufacture.machine_plugin.fake").to_string(),
        "mtconnect" => app.tr("manufacture.machine_plugin.mtconnect").to_string(),
        _ => app.tr("manufacture.machine_plugin.none").to_string(),
    };
    let mut picked: Option<String> = None;
    egui::ComboBox::from_id_salt("mfg_machine_plugin_sel").selected_text(&label).show_ui(ui, |ui| {
        for (id, key) in [
            ("none", "manufacture.machine_plugin.none"),
            ("fake", "manufacture.machine_plugin.fake"),
            ("mtconnect", "manufacture.machine_plugin.mtconnect"),
        ] {
            if ui.selectable_label(current == id, app.tr(key)).clicked() {
                picked = Some(id.to_string());
            }
        }
    });
    if let Some(choice) = picked {
        app.manufacture_workspace.machine_plugin = choice.clone();
        apply_machine_plugin(app, &choice);
    }

    if app.manufacture_workspace.machine_plugin == "mtconnect" {
        ui.label(app.tr("manufacture.machine_plugin.endpoint"));
        if ui.text_edit_singleline(&mut app.manufacture_workspace.machine_endpoint).changed() {
            apply_machine_plugin(app, "mtconnect");
        }
        ui.small(app.tr("manufacture.machine_plugin.fixture_note"));
    }

    ui.horizontal(|ui| {
        if ui.button(app.tr("manufacture.machine_plugin.refresh")).clicked() {
            refresh_machine_status(app);
        }
    });

    if let Some(err) = &app.manufacture_workspace.last_machine_error {
        ui.colored_label(Color32::from_rgb(220, 80, 80), err);
    }
    if let Some(st) = &app.manufacture_workspace.last_machine_status {
        let status = &st["status"];
        ui.label(format!(
            "{}: {} · T{} · {} rpm",
            app.tr("manufacture.machine_plugin.status"),
            status["execution"].as_str().unwrap_or("—"),
            status["tool_number"].as_u64().map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
            status["spindle_rpm"].as_f64().map(|n| format!("{n:.0}")).unwrap_or_else(|| "—".into())
        ));
        if let Some(prog) = status["program"].as_str() {
            ui.small(format!("{}: {prog}", app.tr("manufacture.machine_plugin.program")));
        }
        if let Some(src) = status["source"].as_str() {
            ui.small(format!("{}: {src}", app.tr("manufacture.machine_plugin.source")));
        }
    }
}

fn apply_machine_plugin(app: &mut SolveApp, plugin: &str) {
    app.manufacture_workspace.last_machine_error = None;
    let params = match plugin {
        "none" => json!({"plugin": "none"}),
        "fake" => json!({"plugin": "fake"}),
        "mtconnect" => json!({
            "plugin": "mtconnect",
            "fixture": true,
            "endpoint": app.manufacture_workspace.machine_endpoint,
        }),
        _ => json!({"plugin": "none"}),
    };
    match app.run("manufacture.machine.use", params) {
        Ok(_) => refresh_machine_status(app),
        Err(e) => {
            app.manufacture_workspace.last_machine_status = None;
            app.manufacture_workspace.last_machine_error = Some(e.to_string());
        }
    }
}

fn refresh_machine_status(app: &mut SolveApp) {
    if app.manufacture_workspace.machine_plugin == "none" {
        app.manufacture_workspace.last_machine_status = None;
        app.manufacture_workspace.last_machine_error = None;
        return;
    }
    match app.run("manufacture.machine.status", json!({})) {
        Ok(v) => {
            app.manufacture_workspace.last_machine_status = Some(v);
            app.manufacture_workspace.last_machine_error = None;
        }
        Err(e) => {
            app.manufacture_workspace.last_machine_status = None;
            app.manufacture_workspace.last_machine_error = Some(e.to_string());
        }
    }
}

fn post_profile_section(app: &mut SolveApp, ui: &mut egui::Ui, setup: &solidcraft_manufacture::MfgSetup) {
    ui.heading(app.tr("manufacture.setup.post"));
    let current = setup.post.clone().unwrap_or_default();
    let label = if current.is_empty() { app.tr("manufacture.setup.post_none").to_string() } else { current.clone() };
    let setup_name = app.manufacture_workspace.active_setup.clone();
    let mut picked: Option<String> = None;
    egui::ComboBox::from_id_salt("mfg_post_sel").selected_text(&label).show_ui(ui, |ui| {
        if ui.selectable_label(current.is_empty(), app.tr("manufacture.setup.post_none")).clicked() {
            picked = Some(String::new());
        }
        for (id, _) in solidcraft_manufacture::post::preset_posts() {
            if ui.selectable_label(current == id, id).clicked() {
                picked = Some(id.to_string());
            }
        }
    });
    if let Some(choice) = picked {
        let _ = app.run("manufacture.posts.ensure_presets", json!({}));
        let set = if choice.is_empty() { json!({"post": serde_json::Value::Null}) } else { json!({"post": choice}) };
        let _ = app.run("manufacture.settings.set", json!({"name": setup_name, "set": set}));
        app.manufacture_workspace.revision = u64::MAX;
    }
}

fn delete_active_setup(app: &mut SolveApp) {
    // The engine doesn't have a delete command; patch via settings to remove the key.
    // We do this by calling a manufacture.settings.delete if available, otherwise via script.
    // For now, show a status message that direct deletion needs the command palette.
    app.set_status(app.tr("manufacture.setup.delete_via_palette").to_string(), false);
}

// ── TOOLS panel (Fusion-style bit / holder measurements) ──────────────────────

#[allow(dead_code)]
fn tools_panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.heading(app.tr("manufacture.tools.heading"));
    ui.small(app.tr("manufacture.tools.hint"));

    let setup_name = app.manufacture_workspace.active_setup.clone();
    if setup_name.is_empty() {
        ui.label(app.tr("manufacture.setup.no_setup"));
        return;
    }
    let Some(setup) = app.session.doc.manufacture.setups.get(&setup_name).cloned() else {
        ui.label(app.tr("manufacture.setup.not_found"));
        return;
    };

    // Clamp selection if the library shrank.
    if let Some(i) = app.manufacture_workspace.selected_tool
        && i >= setup.tools.len()
    {
        app.manufacture_workspace.selected_tool = setup.tools.len().checked_sub(1);
    }

    if ui.button(app.tr("manufacture.tools.open_library")).clicked() {
        crate::manufacture_tools::open_tool_library(app);
    }

    ui.horizontal(|ui| {
        if ui.button(app.tr("manufacture.tools.add")).clicked() {
            crate::manufacture_tools::open_edit_tool(app, None);
        }
        let can_dup = app.manufacture_workspace.selected_tool.is_some();
        if ui.add_enabled(can_dup, egui::Button::new(app.tr("manufacture.tools.duplicate"))).clicked()
            && let Some(i) = app.manufacture_workspace.selected_tool
        {
            let mut tools = setup.tools.clone();
            if let Some(src) = tools.get(i).cloned() {
                let next_num = tools.iter().map(|t| t.number).max().unwrap_or(0).saturating_add(1).max(1);
                let mut copy = src;
                copy.number = next_num;
                if !copy.description.is_empty() {
                    copy.description = format!("{} (copy)", copy.description);
                }
                tools.push(copy);
                let new_idx = tools.len() - 1;
                if save_setup_tools(app, &setup_name, tools) {
                    app.manufacture_workspace.selected_tool = Some(new_idx);
                }
            }
        }
        let can_del = app.manufacture_workspace.selected_tool.is_some();
        if ui.add_enabled(can_del, egui::Button::new(app.tr("manufacture.tools.delete"))).clicked()
            && let Some(i) = app.manufacture_workspace.selected_tool
        {
            let mut tools = setup.tools.clone();
            if i < tools.len() {
                tools.remove(i);
                let next = if tools.is_empty() { None } else { Some(i.min(tools.len() - 1)) };
                if save_setup_tools(app, &setup_name, tools) {
                    app.manufacture_workspace.selected_tool = next;
                }
            }
        }
    });

    ui.separator();
    ui.label(format!("{} {}", setup.tools.len(), app.tr("manufacture.ops.tool_count")));

    // Refresh setup after possible mutation above.
    let setup = app.session.doc.manufacture.setups.get(&setup_name).cloned().unwrap_or(setup);

    egui::ScrollArea::vertical().id_salt("mfg_tool_list").max_height(140.0).show(ui, |ui| {
        for (i, tool) in setup.tools.iter().enumerate() {
            let selected = app.manufacture_workspace.selected_tool == Some(i);
            let row = format!("T{} · {} · ⌀{:.3} · L{:.1}", tool.number, tool_kind_label(tool.kind), tool.diameter, tool.length);
            if ui.selectable_label(selected, row).clicked() {
                app.manufacture_workspace.selected_tool = Some(i);
            }
        }
    });

    ui.separator();

    let Some(idx) = app.manufacture_workspace.selected_tool else {
        ui.label(app.tr("manufacture.tools.select_hint"));
        return;
    };
    let Some(mut tool) = setup.tools.get(idx).cloned() else {
        return;
    };

    ui.heading(app.tr("manufacture.tools.edit"));
    let mut dirty = false;

    ui.horizontal(|ui| {
        ui.label(app.tr("manufacture.tools.number"));
        let mut num = tool.number;
        if ui.add(egui::DragValue::new(&mut num).speed(1).range(1..=999)).changed() {
            tool.number = num;
            dirty = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label(app.tr("manufacture.tools.kind"));
        let kind_label = tool_kind_label(tool.kind);
        egui::ComboBox::from_id_salt("mfg_tool_kind").selected_text(kind_label).show_ui(ui, |ui| {
            for kind in TOOL_KINDS {
                if ui.selectable_label(tool.kind == *kind, tool_kind_label(*kind)).clicked() {
                    tool.kind = *kind;
                    dirty = true;
                }
            }
        });
    });

    ui.label(app.tr("manufacture.tools.geometry"));
    dirty |= drag_mm(ui, app.tr("manufacture.tools.diameter"), &mut tool.diameter, 0.01, 0.1..=200.0);
    dirty |= drag_mm(ui, app.tr("manufacture.tools.corner_radius"), &mut tool.corner_radius, 0.01, 0.0..=100.0);
    dirty |= drag_mm(ui, app.tr("manufacture.tools.length"), &mut tool.length, 0.1, 1.0..=500.0);
    dirty |= drag_mm(ui, app.tr("manufacture.tools.flute_length"), &mut tool.flute_length, 0.1, 0.1..=500.0);

    ui.horizontal(|ui| {
        ui.label(app.tr("manufacture.tools.flutes"));
        let mut flutes = tool.flutes as u32;
        if ui.add(egui::DragValue::new(&mut flutes).speed(1).range(1..=64)).changed() {
            tool.flutes = flutes as u8;
            dirty = true;
        }
    });

    ui.label(app.tr("manufacture.tools.description"));
    if ui.text_edit_singleline(&mut tool.description).changed() {
        dirty = true;
    }

    // Soft ATC check against assigned machine
    if let Some(mname) = &setup.machine
        && let Some(m) = app.session.doc.manufacture.machines.get(mname)
    {
        let tc = &m.tool_changer;
        if tc.pockets > 0 && tool.number > tc.pockets {
            ui.colored_label(Color32::from_rgb(220, 190, 60), format!("{} (> {})", app.tr("manufacture.tools.warn_pocket"), tc.pockets));
        }
        if tc.max_tool_diameter_mm > 0.0 && tool.diameter > tc.max_tool_diameter_mm {
            ui.colored_label(Color32::from_rgb(220, 190, 60), app.tr("manufacture.tools.warn_diameter"));
        }
        if tc.max_tool_length_mm > 0.0 && tool.length > tc.max_tool_length_mm {
            ui.colored_label(Color32::from_rgb(220, 190, 60), app.tr("manufacture.tools.warn_length"));
        }
    }

    if dirty {
        let mut tools = setup.tools.clone();
        if let Some(slot) = tools.get_mut(idx) {
            *slot = tool;
            let _ = save_setup_tools(app, &setup_name, tools);
        }
    }
}

#[allow(dead_code)]
const TOOL_KINDS: &[solidcraft_manufacture::ToolKind] = &[
    solidcraft_manufacture::ToolKind::FlatEndmill,
    solidcraft_manufacture::ToolKind::BallEndmill,
    solidcraft_manufacture::ToolKind::BullnoseEndmill,
    solidcraft_manufacture::ToolKind::Drill,
    solidcraft_manufacture::ToolKind::Chamfer,
    solidcraft_manufacture::ToolKind::Facemill,
    solidcraft_manufacture::ToolKind::Tap,
    solidcraft_manufacture::ToolKind::Probe,
    solidcraft_manufacture::ToolKind::Waterjet,
    solidcraft_manufacture::ToolKind::Laser,
    solidcraft_manufacture::ToolKind::Plasma,
];

fn tool_kind_label(kind: solidcraft_manufacture::ToolKind) -> &'static str {
    match kind {
        solidcraft_manufacture::ToolKind::FlatEndmill => "Flat endmill",
        solidcraft_manufacture::ToolKind::BallEndmill => "Ball endmill",
        solidcraft_manufacture::ToolKind::BullnoseEndmill => "Bullnose",
        solidcraft_manufacture::ToolKind::Drill => "Drill",
        solidcraft_manufacture::ToolKind::Chamfer => "Chamfer",
        solidcraft_manufacture::ToolKind::Facemill => "Face mill",
        solidcraft_manufacture::ToolKind::Tap => "Tap",
        solidcraft_manufacture::ToolKind::Probe => "Probe",
        solidcraft_manufacture::ToolKind::Waterjet => "Waterjet",
        solidcraft_manufacture::ToolKind::Laser => "Laser",
        solidcraft_manufacture::ToolKind::Plasma => "Plasma",
    }
}

#[allow(dead_code)]
fn drag_mm(ui: &mut egui::Ui, label: &str, value: &mut f64, speed: f64, range: std::ops::RangeInclusive<f64>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed = ui.add(egui::DragValue::new(value).speed(speed).range(range).suffix(" mm")).changed();
    });
    changed
}

#[allow(dead_code)]
fn save_setup_tools(app: &mut SolveApp, setup_name: &str, tools: Vec<solidcraft_manufacture::Tool>) -> bool {
    match serde_json::to_value(&tools) {
        Ok(tools_json) => match app.run("manufacture.settings.set", json!({"name": setup_name, "set": {"tools": tools_json}})) {
            Ok(_) => {
                app.manufacture_workspace.revision = u64::MAX;
                app.manufacture_workspace.last_issues = app.session.doc.manufacture.issues();
                true
            }
            Err(e) => {
                app.set_status(e, true);
                false
            }
        },
        Err(e) => {
            app.set_status(e.to_string(), true);
            false
        }
    }
}

// ── OPERATIONS panel ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn operations_panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.heading(app.tr("manufacture.ops.heading"));

    let setup_name = app.manufacture_workspace.active_setup.clone();
    if setup_name.is_empty() {
        ui.label(app.tr("manufacture.setup.no_setup"));
        cam_controls(app, ui);
        return;
    }

    let Some(setup) = app.session.doc.manufacture.setups.get(&setup_name).cloned() else {
        ui.label(app.tr("manufacture.setup.not_found"));
        return;
    };

    // Tools summary → edit in Tools tab
    ui.horizontal(|ui| {
        ui.label(format!("{} {}", setup.tools.len(), app.tr("manufacture.ops.tool_count")));
        if ui.small_button(app.tr("manufacture.ops.edit_tools")).clicked() {
            app.manufacture_workspace.tab = MfgTab::Tools;
        }
    });
    for tool in &setup.tools {
        ui.small(format!("  T{} – {} ⌀{:.3} L{:.1} ({} flute)", tool.number, tool_kind_label(tool.kind), tool.diameter, tool.length, tool.flutes));
    }

    ui.separator();

    // Operations browser: Fusion-style [T#] name + nested tool
    ui.label(format!("{} {}", setup.operations.len(), app.tr("manufacture.ops.op_count")));
    let selected = app.manufacture_workspace.selected_op;
    for (i, op) in setup.operations.iter().enumerate() {
        let tnum = op.tool_number();
        let label = op.display_name().to_string();
        let row = format!("[T{tnum}] {label}");
        let sel = selected == Some(i);
        if ui.selectable_label(sel, &row).clicked() {
            app.manufacture_workspace.selected_op = Some(i);
            open_op_dialog(app, &setup_name, i);
        }
        if let Some(tool) = setup.tools.iter().find(|t| t.number == tnum) {
            let desc = if tool.description.is_empty() { tool_kind_label(tool.kind) } else { tool.description.as_str() };
            ui.small(format!("      #{tnum} - ø{:.2}mm {desc}", tool.diameter));
        } else {
            ui.small(format!("      #{tnum} — {}", app.tr("manufacture.ops.tool_missing")));
        }
    }

    if let Some(d) = &app.manufacture_workspace.drill_dialog {
        let n = d.draft.hole_centres().len();
        ui.separator();
        ui.label(format!("{}: {n}", app.tr("manufacture.drill.hole_count")));
    }
    if let Some(d) = &app.manufacture_workspace.contour_dialog {
        let n = d.draft.contour_points().len();
        ui.separator();
        ui.label(format!("{}: {n}", app.tr("manufacture.contour.selection_count")));
    }

    ui.separator();
    cam_controls(app, ui);
}

// ── SIMULATE panel ────────────────────────────────────────────────────────────

#[allow(dead_code)]
fn simulate_panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.heading(app.tr("manufacture.sim.heading"));

    if app.manufacture_workspace.toolpath_segments.is_empty() {
        ui.label(app.tr("manufacture.sim.no_toolpath"));
        return;
    }

    let total = app.manufacture_workspace.toolpath_segments.len();
    ui.label(format!("{} {}/{}", app.tr("manufacture.sim.step"), app.manufacture_workspace.simulate_step, total));

    let scrubber_label = app.tr("manufacture.sim.scrubber").to_string();
    let step = &mut app.manufacture_workspace.simulate_step;
    ui.add(egui::Slider::new(step, 0..=total).text(scrubber_label));

    ui.separator();
    ui.heading(app.tr("manufacture.sim.stock_summary"));

    let shown = app.manufacture_workspace.simulate_step;
    let rapids = app.manufacture_workspace.toolpath_segments[..shown]
        .iter()
        .filter(|s| matches!(s, solidcraft_manufacture::PathSegment::Rapid { .. }))
        .count();
    let cuts = shown.saturating_sub(rapids);
    ui.label(format!("{}: {} {}: {}", app.tr("manufacture.sim.rapids"), rapids, app.tr("manufacture.sim.cuts"), cuts));

    if ui.button(app.tr("manufacture.sim.run")).clicked() {
        run_simulation(app);
    }

    if let Some(sim) = &app.manufacture_workspace.last_sim {
        ui.separator();
        ui.label(format!(
            "{}: {:.1} mm³",
            app.tr("manufacture.sim.removed_volume"),
            sim.get("removed_volume_mm3").and_then(|v| v.as_f64()).unwrap_or(0.0)
        ));
        if sim.get("has_errors").and_then(|v| v.as_bool()).unwrap_or(false) {
            ui.colored_label(Color32::from_rgb(220, 80, 80), app.tr("manufacture.sim.has_errors"));
        }
        if let Some(issues) = sim.get("issues").and_then(|v| v.as_array()) {
            for issue in issues {
                let msg = issue.get("message").and_then(|m| m.as_str()).unwrap_or("");
                ui.small(msg);
            }
        }
    }

    // Issues from last preflight / validation
    let issues = app.manufacture_workspace.last_issues.clone();
    if !issues.is_empty() {
        ui.separator();
        ui.heading(app.tr("manufacture.issues.heading"));
        for issue in &issues {
            ui.label(format!("{}: {}", issue.path, issue.message));
        }
    }
}

fn run_simulation(app: &mut SolveApp) {
    run_simulation_inner(app, false);
}

fn run_simulation_with_machine(app: &mut SolveApp) {
    run_simulation_inner(app, true);
}

fn run_simulation_inner(app: &mut SolveApp, with_machine: bool) {
    let setup_name = app.manufacture_workspace.active_setup.clone();
    let Some(job_params) = app.manufacture_workspace.cam_job_id.clone() else {
        app.set_status(app.tr("manufacture.sim.no_toolpath").to_string(), true);
        return;
    };
    let mut params = job_params;
    if let Some(obj) = params.as_object_mut() {
        obj.insert("setup".into(), serde_json::Value::String(setup_name.clone()));
        if with_machine {
            // Use stock box as a stand-in travel envelope when a machine is linked.
            if let Some(setup) = app.session.doc.manufacture.setups.get(&setup_name) {
                if let solidcraft_manufacture::Stock::Box { width, depth, height } = setup.stock {
                    obj.insert("machine_xy_travel".into(), json!([width, depth]));
                    obj.insert("machine_z_min".into(), json!(-height));
                }
                app.set_status(app.tr("manufacture.sim.with_machine").to_string(), false);
            }
        }
    }
    match app.session.execute("manufacture.simulate", &params) {
        Ok(v) => app.manufacture_workspace.last_sim = Some(v),
        Err(e) => app.set_status(e.to_string(), true),
    }
}

// ── POST panel ────────────────────────────────────────────────────────────────

#[allow(dead_code)]
fn post_panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui.heading(app.tr("manufacture.post.heading"));

    if app.manufacture_workspace.generated_code.is_none() && app.manufacture_workspace.cam_job_id.is_some() {
        request_gcode_preview(app);
    }
    if ui.button(app.tr("manufacture.post.ui")).clicked() {
        crate::manufacture_post::open_post_process(app);
    }

    if let Some(code) = &app.manufacture_workspace.generated_code.clone() {
        let code_clone = code.clone();
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            ui.code(&code_clone);
        });
        ui.separator();
        if ui.button(app.tr("manufacture.post.save")).clicked() {
            let code_save = code.clone();
            app.pick_save("output.nc", &["nc", "gcode", "txt"], move |app, path| {
                let Some(path) = path else { return };
                if let Err(e) = std::fs::write(&path, code_save.as_bytes()) {
                    app.set_status(format!("{}: {e}", app.tr("manufacture.post.save_error")), true);
                }
            });
        }
    } else {
        ui.label(app.tr("manufacture.post.no_output"));
        ui.small(app.tr("manufacture.post.no_output_hint"));
    }
}

// ── CAM job controls ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn cam_controls(app: &mut SolveApp, ui: &mut egui::Ui) {
    // Preflight
    if ui.button(app.tr("manufacture.cam.preflight")).clicked() {
        let setup = app.manufacture_workspace.active_setup.clone();
        if setup.is_empty() {
            app.set_status(app.tr("manufacture.cam.no_setup").to_string(), true);
        } else {
            match app.session.execute("manufacture.preflight", &json!({"setup": setup})) {
                Ok(v) => {
                    app.manufacture_workspace.last_issues = app.session.doc.manufacture.issues();
                    let size_kb = v.get("step_bytes").and_then(|b| b.as_u64()).unwrap_or(0) / 1024;
                    app.set_status(format!("{} ({size_kb} KB)", app.tr("manufacture.cam.preflight_ok")), false);
                }
                Err(e) => app.set_status(format!("{}: {e}", app.tr("manufacture.cam.preflight_fail")), true),
            }
        }
    }

    // Job status display
    let busy = job_status_row(app, ui);

    // Start button
    if ui.add_enabled(!busy, egui::Button::new(app.tr("manufacture.cam.start"))).clicked() {
        start_cam_job(app);
    }
}

/// Draw the current job's status row; returns `true` while the job is running.
/// Reads status from the in-memory engine command. No network calls.
#[allow(dead_code)]
fn job_status_row(app: &mut SolveApp, ui: &mut egui::Ui) -> bool {
    let job_id = app.manufacture_workspace.cam_job_id.clone();
    let Some(id) = job_id else { return false };

    // Non-blocking status: reads the local handle only.
    match app.session.execute("manufacture.status", &id) {
        Ok(status) => {
            let phase = status["progress"]["phase"].as_str().unwrap_or("unknown");
            let busy = matches!(phase, "queued" | "planning" | "post_processing" | "cancelling");
            ui.label(match phase {
                "queued" => app.tr("manufacture.cam.queued"),
                "planning" => app.tr("manufacture.cam.planning"),
                "post_processing" => app.tr("manufacture.cam.post_processing"),
                "cancelling" => app.tr("manufacture.cam.cancelling"),
                "cancelled" => app.tr("manufacture.cam.cancelled"),
                "completed" => app.tr("manufacture.cam.completed"),
                "failed" => app.tr("manufacture.cam.failed"),
                _ => app.tr("manufacture.cam.status_unknown"),
            });
            if busy {
                let done = status["progress"]["done"].as_u64().unwrap_or(0);
                let total = status["progress"]["total"].as_u64().unwrap_or(1).max(1);
                ui.add(egui::ProgressBar::new((done as f64 / total as f64).clamp(0.0, 1.0) as f32));
                if ui.button(app.tr("manufacture.cam.cancel")).clicked() {
                    // cancel() transitions to Cancelling; pump will send DELETE.
                    let _ = app.session.execute("manufacture.cancel", &id);
                }
            }
            if let Some(msg) = status["error"]["message"].as_str() {
                ui.colored_label(Color32::from_rgb(220, 80, 80), msg);
            }
            // Check scene staleness
            if let (Some(scene_gen), Ok(rev)) = (status["scene_generation"].as_u64(), Ok::<u64, ()>(app.session.revision))
                && scene_gen != rev
            {
                ui.small(app.tr("manufacture.cam.stale"));
            }
            busy
        }
        Err(e) => {
            ui.label(e.to_string());
            false
        }
    }
}

fn start_cam_job(app: &mut SolveApp) {
    let setup = app.manufacture_workspace.active_setup.clone();
    if setup.is_empty() {
        app.set_status(app.tr("manufacture.cam.no_setup").to_string(), true);
        return;
    }
    // Clear previous result.
    app.manufacture_workspace.toolpath_segments = Vec::new();
    app.manufacture_workspace.simulate_step = 0;
    app.manufacture_workspace.generated_code = None;

    match app.session.execute("manufacture.start", &json!({"setup": setup})) {
        Ok(v) => {
            let job_id = v.get("job").cloned().unwrap_or_default();
            app.manufacture_workspace.cam_job_id = Some(json!({"job": job_id}));
        }
        Err(e) => app.set_status(format!("{}: {e}", app.tr("manufacture.cam.start_fail")), true),
    }
}

// ── URL validation ────────────────────────────────────────────────────────────

/// A validated companion endpoint URL.  Contains only the normalised origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCompanionUrl {
    /// Normalised HTTP origin: `http://HOST:PORT` (no path, query, or fragment).
    pub origin: String,
}

impl ParsedCompanionUrl {
    /// Build a companion API URL by appending `path` to the origin.
    pub fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.origin, path)
    }
}

/// Parse and validate an HTTP companion endpoint URL.
///
/// Enforces:
/// - Scheme is exactly `http` (not https, ftp, etc.)
/// - No userinfo (`@` character) — rejects `http://user:pass@host` and
///   credential-injection tricks like `http://127.0.0.1:PORT@evil.com`
/// - Host is exactly `localhost`, `127.0.0.1`, or `[::1]`
/// - An explicit port number is present
///
/// Returns a [`ParsedCompanionUrl`] whose `origin` is the normalised
/// `http://HOST:PORT` form, stripping any path, query, or fragment.
pub fn parse_companion_url(endpoint: &str) -> Result<ParsedCompanionUrl, String> {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use url::Url;

    let url = Url::parse(endpoint).map_err(|e| format!("invalid companion endpoint URL: {e}"))?;

    // Only http scheme.
    if url.scheme() != "http" {
        return Err(format!("companion endpoint must use http scheme, got: {:?}", url.scheme()));
    }

    // Reject any userinfo component — catches credential-injection URLs like
    // `http://127.0.0.1:PORT@evil.com` where the real host would be `evil.com`.
    if !url.username().is_empty() || url.password().is_some() {
        return Err("companion endpoint must not contain userinfo (user:pass@ is forbidden)".into());
    }

    // Host must be exactly one of the three allowed loopback forms.
    let host = url.host().ok_or_else(|| "companion endpoint has no host".to_string())?;
    let normalized_host: &str = match host {
        url::Host::Domain("localhost") => "localhost",
        url::Host::Ipv4(addr) if addr == Ipv4Addr::LOCALHOST => "127.0.0.1",
        url::Host::Ipv6(addr) if addr == Ipv6Addr::LOCALHOST => "[::1]",
        _ => {
            return Err(format!("companion endpoint host must be localhost, 127.0.0.1, or [::1], got: {}", url.host_str().unwrap_or("?")));
        }
    };

    // Explicit port required — default port 80 is not accepted.
    let port = url.port().ok_or_else(|| format!("companion endpoint requires an explicit port (e.g. http://127.0.0.1:38473), got: {endpoint:?}"))?;

    Ok(ParsedCompanionUrl { origin: format!("http://{}:{}", normalized_host, port) })
}

// ── CompanionCamBackend ────────────────────────────────────────────────────────

// ── Companion API response types ──────────────────────────────────────────────

/// POST /cam/v1/jobs → 202 Accepted
#[derive(serde::Deserialize)]
struct SubmitResp {
    job_id: String,
    #[allow(dead_code)]
    status: String,
}

/// GET /cam/v1/jobs/:id/status → 200 OK
#[derive(serde::Deserialize)]
struct StatusResp {
    #[allow(dead_code)]
    job_id: String,
    /// `"pending"` | `"running"` | `"done"` | `"failed"` | `"cancelled"`
    status: String,
    error: Option<String>,
}

/// GET /cam/v1/jobs/:id/result → 200 OK
#[derive(serde::Deserialize)]
struct ResultResp {
    #[allow(dead_code)]
    job_id: String,
    #[allow(dead_code)]
    status: String,
    result: Option<ResultBody>,
}

#[derive(serde::Deserialize)]
struct ResultBody {
    segments: Vec<solidcraft_manufacture::PathSegment>,
    post_output: Option<String>,
}

// ── Companion HTTP state machine ──────────────────────────────────────────────

/// Tag for what a pending ehttp response should be interpreted as.
/// All captured values are `Send + 'static`.
enum ResponseKind {
    Post { handle: CamHandle, seq: u64 },
    Poll { job_id: String, handle: CamHandle, seq: u64 },
    FetchResult { job_id: String, handle: CamHandle, seq: u64 },
    Cancel { handle: CamHandle, seq: u64 },
}

/// Current phase of the companion HTTP state machine.
enum CompanionPhase {
    /// No active job.
    Idle,
    /// Job queued locally; next pump will POST it.
    Queued { handle: CamHandle, input: Box<CamJobInput> },
    /// POST in flight; waiting for callback.
    PostInflight,
    /// POST succeeded; we have a companion job_id and poll when the interval elapses.
    Polling { job_id: String, handle: CamHandle },
    /// Status poll in flight.
    PollInflight,
    /// Status said done; next pump should GET the result (not yet in flight).
    WantResult { job_id: String, handle: CamHandle },
    /// Result fetch in flight.
    ResultInflight,
    /// DELETE (cancel) in flight.
    CancelInflight,
}

/// Shared mutable state owned by [`CompanionCamBackend`].
struct CompanionInner {
    origin: String,
    token: String,
    phase: CompanionPhase,
    last_action: Instant,
    /// Incremented on each new submit; stale callbacks detect the mismatch and no-op.
    generation: u64,
    // Provenance captured from CamJobInput at submit time.
    job_setup: String,
    job_scene_gen: u64,
    job_step_sha: String,
}

/// Cross-platform HTTP companion backend using [`ehttp`].
///
/// Works on native (ehttp uses a background thread) and WASM (ehttp uses the browser
/// Fetch API).  No `std::thread` assumptions anywhere in this type.
///
/// The endpoint must be an HTTP loopback URL validated by [`parse_companion_url`].
pub struct CompanionCamBackend {
    inner: Arc<Mutex<CompanionInner>>,
}

impl CompanionCamBackend {
    /// Create a new backend from a validated URL and optional bearer token.
    pub fn new(url: ParsedCompanionUrl, token: &str) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CompanionInner {
                origin: url.origin,
                token: token.to_string(),
                phase: CompanionPhase::Idle,
                last_action: Instant::now(),
                generation: 0,
                job_setup: String::new(),
                job_scene_gen: 0,
                job_step_sha: String::new(),
            })),
        }
    }
}

fn lock_inner(inner: &Mutex<CompanionInner>) -> std::sync::MutexGuard<'_, CompanionInner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

impl CamBackend for CompanionCamBackend {
    fn descriptor(&self) -> CamBackendDescriptor {
        CamBackendDescriptor {
            id: "companion".into(),
            label: "Companion CAM".into(),
            availability: solidcraft_manufacture::job::Availability::Available,
            note: None,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    /// Queue a new job. The actual HTTP POST is sent by the next [`pump`] call.
    fn submit(&self, id: CamJobId, input: CamJobInput) -> CamResult<CamHandle> {
        let total_ops = input.settings.setups.get(&input.setup).map_or(0, |s| s.operations.len() as u32);
        let handle = CamHandle::new(id, input.setup.clone(), input.scene_generation, total_ops);

        let mut g = lock_inner(&self.inner);
        // Bump generation to invalidate any in-flight callbacks for the previous job.
        g.generation = g.generation.wrapping_add(1);
        // Save provenance data (needed by the result callback).
        g.job_setup = input.setup.clone();
        g.job_scene_gen = input.scene_generation;
        g.job_step_sha = input.step_sha256.clone();
        g.phase = CompanionPhase::Queued { handle: handle.clone(), input: Box::new(input) };

        Ok(handle)
    }

    /// Drive the HTTP state machine.  Called from the workspace tick; at most twice per second.
    ///
    /// Decides what (if any) HTTP request to send, transitions the phase atomically while
    /// holding the lock, then releases the lock before invoking ehttp::fetch so the callback
    /// (which re-acquires the lock) never deadlocks.
    fn pump(&self) {
        self.pump_with_result();
    }
}

/// Decide the next HTTP action and perform the phase transition atomically.
/// Returns `None` when no request should be sent right now.
fn next_action(g: &mut CompanionInner) -> Option<(ehttp::Request, ResponseKind)> {
    match g.phase {
        // ── Queued: POST the job ────────────────────────────────────────────
        CompanionPhase::Queued { .. } => {
            // Move handle + input out of the Queued phase using a sentinel swap.
            let CompanionPhase::Queued { handle, input } = std::mem::replace(&mut g.phase, CompanionPhase::Idle) else {
                return None;
            };

            let body_bytes = match build_post_body(&input) {
                Ok(b) => b,
                Err(e) => {
                    handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, e));
                    return None;
                }
            };

            let url = format!("{}/cam/v1/jobs", g.origin);
            let request = make_request(ehttp::Method::POST, &url, &g.token, body_bytes, Some("application/json"));

            let seq = g.generation;
            g.phase = CompanionPhase::PostInflight;
            g.last_action = Instant::now();

            Some((request, ResponseKind::Post { handle, seq }))
        }

        // ── Polling: decide whether to poll or cancel ───────────────────────
        CompanionPhase::Polling { ref job_id, ref handle } => {
            // Check if the UI has requested cancellation.
            let cancelling = handle.status().is_ok_and(|s| s.progress.phase == CamPhase::Cancelling);

            if cancelling {
                let job_id = job_id.clone();
                let handle = handle.clone();
                let seq = g.generation;
                let url = format!("{}/cam/v1/jobs/{}", g.origin, job_id);
                let request = make_request(ehttp::Method::DELETE, &url, &g.token, vec![], None);
                g.phase = CompanionPhase::CancelInflight;
                g.last_action = Instant::now();
                return Some((request, ResponseKind::Cancel { handle, seq }));
            }

            // Rate-limit polling to at most twice per second.
            if g.last_action.elapsed() < POLL_MIN_INTERVAL {
                return None;
            }

            let job_id = job_id.clone();
            let handle = handle.clone();
            let seq = g.generation;
            let url = format!("{}/cam/v1/jobs/{}/status", g.origin, job_id);
            let request = make_request(ehttp::Method::GET, &url, &g.token, vec![], None);
            g.phase = CompanionPhase::PollInflight;
            g.last_action = Instant::now();

            Some((request, ResponseKind::Poll { job_id, handle, seq }))
        }

        // All inflight phases: wait for the callback.
        _ => None,
    }
}

/// Build an ehttp request for the companion API.
fn make_request(method: ehttp::Method, url: &str, token: &str, body: Vec<u8>, content_type: Option<&str>) -> ehttp::Request {
    let mut req = ehttp::Request::new(method, url, ehttp::Headers::default()).with_body(body);
    if let Some(ct) = content_type {
        req.headers.insert("Content-Type", ct);
    }
    if !token.is_empty() {
        req.headers.insert("Authorization", format!("Bearer {token}"));
    }
    req
}

/// Build the JSON POST body: `{ "step_b64": "…", "manufacture": { … } }`.
fn build_post_body(input: &CamJobInput) -> Result<Vec<u8>, String> {
    let step_b64 = base64_encode(&input.step_bytes);
    let body = serde_json::json!({
        "step_b64": step_b64,
        "manufacture": *input.settings,
    });
    serde_json::to_vec(&body).map_err(|e| format!("serialize companion POST body: {e}"))
}

/// Apply an ehttp response to the companion state machine.
fn apply_response(g: &mut CompanionInner, kind: ResponseKind, result: ehttp::Result<ehttp::Response>) {
    match kind {
        ResponseKind::Post { handle, seq } => on_post_response(g, handle, seq, result),
        ResponseKind::Poll { job_id, handle, seq } => on_poll_response(g, handle, job_id, seq, result),
        ResponseKind::FetchResult { job_id, handle, seq } => on_result_response(g, handle, job_id, seq, result),
        ResponseKind::Cancel { handle, seq } => on_cancel_response(g, handle, seq, result),
    }
}

fn on_post_response(g: &mut CompanionInner, handle: CamHandle, seq: u64, result: ehttp::Result<ehttp::Response>) {
    if g.generation != seq {
        return;
    } // stale: a newer job replaced this one
    match result {
        Err(e) => {
            handle.adapter_fail(CamJobError::new(CamErrorCode::HostUnavailable, e));
            g.phase = CompanionPhase::Idle;
        }
        Ok(resp) => {
            if resp.bytes.len() > MAX_RESPONSE_BYTES {
                handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, "companion POST response exceeds size limit"));
                g.phase = CompanionPhase::Idle;
                return;
            }
            if !resp.ok {
                let body = resp.text().unwrap_or_default();
                handle.adapter_fail(CamJobError::new(CamErrorCode::InvalidRequest, format!("companion returned HTTP {}: {body}", resp.status)));
                g.phase = CompanionPhase::Idle;
                return;
            }
            match serde_json::from_slice::<SubmitResp>(&resp.bytes) {
                Ok(r) => {
                    handle.adapter_set_progress(CamPhase::Planning, 0);
                    g.phase = CompanionPhase::Polling { job_id: r.job_id, handle };
                }
                Err(e) => {
                    handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, format!("parse companion submit response: {e}")));
                    g.phase = CompanionPhase::Idle;
                }
            }
        }
    }
}

fn on_poll_response(g: &mut CompanionInner, handle: CamHandle, job_id: String, seq: u64, result: ehttp::Result<ehttp::Response>) {
    if g.generation != seq {
        return;
    }
    match result {
        Err(e) => {
            handle.adapter_fail(CamJobError::new(CamErrorCode::HostUnavailable, format!("poll: {e}")));
            g.phase = CompanionPhase::Idle;
        }
        Ok(resp) => {
            if resp.bytes.len() > MAX_RESPONSE_BYTES {
                handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, "companion poll response exceeds size limit"));
                g.phase = CompanionPhase::Idle;
                return;
            }
            if !resp.ok {
                let body = resp.text().unwrap_or_default();
                handle.adapter_fail(CamJobError::new(CamErrorCode::HostUnavailable, format!("poll returned HTTP {}: {body}", resp.status)));
                g.phase = CompanionPhase::Idle;
                return;
            }
            match serde_json::from_slice::<StatusResp>(&resp.bytes) {
                Ok(r) => match r.status.as_str() {
                    "done" => {
                        // Defer the result GET to the next pump (we hold the lock here).
                        g.phase = CompanionPhase::WantResult { job_id, handle };
                    }
                    "failed" => {
                        let msg = r.error.unwrap_or_else(|| "companion job failed".into());
                        handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, msg));
                        g.phase = CompanionPhase::Idle;
                    }
                    "cancelled" => {
                        handle.adapter_set_progress(CamPhase::Cancelled, 0);
                        g.phase = CompanionPhase::Idle;
                    }
                    // "pending" | "running" | anything else: keep polling.
                    _ => {
                        handle.adapter_set_progress(CamPhase::Planning, 0);
                        g.phase = CompanionPhase::Polling { job_id, handle };
                    }
                },
                Err(e) => {
                    handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, format!("parse poll response: {e}")));
                    g.phase = CompanionPhase::Idle;
                }
            }
        }
    }
}

fn on_result_response(g: &mut CompanionInner, handle: CamHandle, _job_id: String, seq: u64, result: ehttp::Result<ehttp::Response>) {
    if g.generation != seq {
        return;
    }
    match result {
        Err(e) => {
            handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, format!("fetch result: {e}")));
            g.phase = CompanionPhase::Idle;
        }
        Ok(resp) => {
            if resp.bytes.len() > MAX_RESPONSE_BYTES {
                handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, "companion result response exceeds size limit"));
                g.phase = CompanionPhase::Idle;
                return;
            }
            if !resp.ok {
                let body = resp.text().unwrap_or_default();
                handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, format!("result returned HTTP {}: {body}", resp.status)));
                g.phase = CompanionPhase::Idle;
                return;
            }
            match serde_json::from_slice::<ResultResp>(&resp.bytes) {
                Ok(r) => {
                    if let Some(body) = r.result {
                        let provenance =
                            CamProvenance { setup: g.job_setup.clone(), scene_generation: g.job_scene_gen, step_sha256: g.job_step_sha.clone() };
                        handle.adapter_complete(CamJobResult {
                            job: handle.id(),
                            provenance,
                            segments: body.segments,
                            post_output: body.post_output,
                        });
                    } else {
                        handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, "result response has no result body"));
                    }
                    g.phase = CompanionPhase::Idle;
                }
                Err(e) => {
                    handle.adapter_fail(CamJobError::new(CamErrorCode::Internal, format!("parse result response: {e}")));
                    g.phase = CompanionPhase::Idle;
                }
            }
        }
    }
}

fn on_cancel_response(g: &mut CompanionInner, handle: CamHandle, seq: u64, _result: ehttp::Result<ehttp::Response>) {
    if g.generation != seq {
        return;
    }
    // Regardless of the HTTP result, mark the job cancelled.
    handle.adapter_set_progress(CamPhase::Cancelled, 0);
    g.phase = CompanionPhase::Idle;
}

/// Fire a deferred result GET when the phase is [`CompanionPhase::WantResult`].
fn next_result_action(g: &mut CompanionInner) -> Option<(ehttp::Request, ResponseKind)> {
    let CompanionPhase::WantResult { ref job_id, ref handle } = g.phase else {
        return None;
    };
    let job_id = job_id.clone();
    let handle = handle.clone();
    let seq = g.generation;
    let url = format!("{}/cam/v1/jobs/{}/result", g.origin, job_id);
    let request = make_request(ehttp::Method::GET, &url, &g.token, vec![], None);
    g.phase = CompanionPhase::ResultInflight;
    g.last_action = Instant::now();
    Some((request, ResponseKind::FetchResult { job_id, handle, seq }))
}

impl CompanionCamBackend {
    fn pump_with_result(&self) {
        let inner = Arc::clone(&self.inner);

        let action: Option<(ehttp::Request, ResponseKind)> = {
            let mut g = lock_inner(&inner);
            next_action(&mut g).or_else(|| next_result_action(&mut g))
        };

        if let Some((request, kind)) = action {
            let inner2 = Arc::clone(&inner);
            ehttp::fetch(request, move |result| {
                let mut g = lock_inner(&inner2);
                apply_response(&mut g, kind, result);
            });
        }
    }
}

// ── base64 encoding ───────────────────────────────────────────────────────────

/// Standard base64 encoding (RFC 4648 alphabet with `+` and `/`, with padding).
/// Used to encode STEP bytes for the companion POST body.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = *chunk.get(1).unwrap_or(&0) as usize;
        let b2 = *chunk.get(2).unwrap_or(&0) as usize;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(triple >> 18) & 63] as char);
        out.push(CHARS[(triple >> 12) & 63] as char);
        out.push(if chunk.len() > 1 { CHARS[(triple >> 6) & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[triple & 63] as char } else { '=' });
    }
    out
}

// ── Backend lifecycle ─────────────────────────────────────────────────────────

/// Install a companion backend into the session when the endpoint/token are configured.
/// Called whenever the user edits the endpoint or token field.
pub fn apply_companion_backend(app: &mut SolveApp) {
    let endpoint = app.manufacture_workspace.cam_endpoint.clone();
    let token = app.manufacture_workspace.cam_token.clone();
    match parse_companion_url(&endpoint) {
        Ok(url) => {
            let backend = CompanionCamBackend::new(url, &token);
            app.session.set_manufacture_job_host(Some(CamHost::new(backend)));
        }
        Err(e) => {
            app.session.set_manufacture_job_host(None);
            app.set_status(format!("{}: {e}", app.tr("manufacture.backend.invalid_url")), true);
        }
    }
}

// ── Viewport: design + toolpath overlay ──────────────────────────────────────

/// Draw the design viewport and overlay toolpath lines on top of it.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui) {
    // Delegate to the normal design viewport (doesn't replace design geometry).
    viewport::show(app, ui);

    // Setup WCS / stock, drill, and contour indicators.
    crate::manufacture_setup::paint_overlays(app, ui);
    crate::manufacture_drill::paint_overlays(app, ui);
    crate::manufacture_contour::paint_overlays(app, ui);
    crate::manufacture_2d::paint_overlays(app, ui);
    crate::manufacture_3d::paint_overlays(app, ui);
    crate::manufacture_hole_recognition::paint_overlays(app, ui);
    crate::manufacture_multiaxis::paint_overlays(app, ui);

    // Toolpath overlay: skip if nothing to draw.
    let shown = app.manufacture_workspace.simulate_step.min(app.manufacture_workspace.toolpath_segments.len());
    if shown == 0 {
        return;
    }

    let rect = match app.viewport.rect {
        Some(r) if r.area() > 0.0 => r,
        _ => return,
    };
    let proj = viewport::projection(app, rect);
    let painter = ui.painter().clone();
    let t = Tokens::get();

    let rapid_color = Color32::from_rgba_premultiplied(160, 160, 160, 180);
    let cut_color = t.accent;
    let arc_color = t.sketch_accent;

    let mut current = [0.0f64; 3];
    for seg in app.manufacture_workspace.toolpath_segments.iter().take(shown) {
        let (to, color): ([f64; 3], Color32) = match seg {
            solidcraft_manufacture::PathSegment::Rapid { to } => (*to, rapid_color),
            solidcraft_manufacture::PathSegment::Linear { to, .. } => (*to, cut_color),
            solidcraft_manufacture::PathSegment::Arc { to, .. } => (*to, arc_color),
        };
        let w_from = solidcraft_engine::geom::Vec3::new(current[0], current[1], current[2]);
        let w_to = solidcraft_engine::geom::Vec3::new(to[0], to[1], to[2]);
        if let (Some(s), Some(e)) = (proj.to_screen(w_from), proj.to_screen(w_to)) {
            painter.line_segment([s, e], Stroke::new(1.5, color));
        }
        current = to;
    }
}
