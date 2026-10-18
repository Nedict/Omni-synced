//! The 3D mouse calibration window, opened from Preferences › Navigation. It asks the user to
//! leave the cap still, then to move it along each axis both ways, moving on by itself after each
//! (`solidcraft_space_mouse::Wizard`), and saves what it found for the device it started with.
//! The window closes with Preferences, and when another device (or none) is read instead.

use solidcraft_space_mouse::{Problem, Stage, Wizard};

use crate::SolveApp;

/// What each of `solidcraft_space_mouse::STEPS` asks for, in order.
const STEP_PROMPTS: [&str; 12] = [
    "ui.space_mouse_step_slide_right",
    "ui.space_mouse_step_slide_left",
    "ui.space_mouse_step_pull",
    "ui.space_mouse_step_push",
    "ui.space_mouse_step_press",
    "ui.space_mouse_step_lift",
    "ui.space_mouse_step_tilt_back",
    "ui.space_mouse_step_tilt_forward",
    "ui.space_mouse_step_tilt_left",
    "ui.space_mouse_step_tilt_right",
    "ui.space_mouse_step_twist_clockwise",
    "ui.space_mouse_step_twist_counterclockwise",
];

/// The dead zone, as a share of full travel, shown as a percentage.
pub(crate) fn dead_zone_slider(ui: &mut egui::Ui, share: &mut f32) -> egui::Response {
    ui.add(
        egui::Slider::new(share, 0.0..=solidcraft_space_mouse::MAX_DEAD_ZONE_SHARE)
            .step_by(0.01)
            .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
    )
}

/// Starts calibrating the device being read, if there is one.
pub fn start(app: &mut SolveApp) {
    app.space_mouse.calibrating = app.space_mouse.device.clone().map(|device| (device, Wizard::default()));
}

pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some((_, wizard)) = &app.space_mouse.calibrating else { return };
    let (stage, progress, result) = (wizard.stage(), wizard.progress(), wizard.result());
    let mut dead_zone = wizard.dead_zone_share();
    let raw: [i16; 6] = app.space_mouse.raw.into();
    let prompt = match (stage, result) {
        (Stage::Rest, _) => "ui.space_mouse_step_rest",
        (Stage::Step(index), _) => STEP_PROMPTS.get(index).copied().unwrap_or_default(),
        (Stage::Done, Some(Ok(_))) => "ui.space_mouse_calibration_done",
        (Stage::Done, _) => "ui.space_mouse_calibration_no_response",
    };
    // The calibrated motion that barely moved, as Preferences names it.
    let weak = match result {
        Some(Err(Problem::NoResponse(axis))) => crate::space_mouse::MOTIONS.get(axis).copied(),
        _ => None,
    };
    let counter = match stage {
        Stage::Rest => String::new(),
        Stage::Step(index) => format!("{} / {}", index + 1, STEP_PROMPTS.len()),
        Stage::Done => format!("{0} / {0}", STEP_PROMPTS.len()),
    };
    let (mut back, mut over, mut cancel, mut save) = (false, false, false, false);
    crate::frame::window(ctx, app.tr("ui.space_mouse_calibrate_title"), crate::frame::Width::Normal)
        .id(egui::Id::new("sc_space_mouse_calibration"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(app.tr(prompt));
            if let Some(axis) = weak {
                ui.label(egui::RichText::new(app.tr(axis)).strong());
            }
            ui.add_space(6.0);
            ui.add(egui::ProgressBar::new(progress as f32).text(counter));
            // The device's six raw axes as they come, so the jitter at rest is there to see.
            let reading = raw.map(|value| format!("{value:>6}")).join(" ");
            ui.label(egui::RichText::new(format!("{}: {reading}", app.tr("ui.space_mouse_reading"))).monospace());
            // A reassignment made in Preferences stays on top of whatever this records.
            if app.preferences.space_mouse_axis_source != crate::space_mouse::SAME_AXES {
                ui.label(egui::RichText::new(app.tr("ui.space_mouse_axes_reassigned")).small());
            }
            ui.horizontal(|ui| {
                ui.label(app.tr("ui.space_mouse_dead_zone"));
                dead_zone_slider(ui, &mut dead_zone).on_hover_text(app.tr("ui.space_mouse_dead_zone_tip"));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                back = ui.add_enabled(stage != Stage::Rest, egui::Button::new(app.tr("ui.back"))).clicked();
                over = matches!(result, Some(Err(_))) && ui.button(app.tr("ui.start_over")).clicked();
                cancel = ui.button(app.tr("ui.cancel")).clicked();
                save = matches!(result, Some(Ok(_))) && ui.button(app.tr("ui.save")).clicked();
            });
        });
    if cancel {
        app.space_mouse.calibrating = None;
    } else if let Some((device, wizard)) = &mut app.space_mouse.calibrating {
        wizard.set_dead_zone_share(dead_zone);
        let result = wizard.result();
        if save && let Some(Ok(calibration)) = result {
            app.preferences.space_mouse_calibrations.insert(std::mem::take(device), calibration);
            app.space_mouse.calibrating = None;
        } else if over {
            // A new recording; the dead zone set so far stays.
            *wizard = Wizard::default();
            wizard.set_dead_zone_share(dead_zone);
        } else if back {
            wizard.back();
        }
    }
}
