//! Wood purchase estimates through the command engine. Draft reference edits are read-only;
//! saving the reference is one undoable command. Geometry is never measured by the UI.
use egui::RichText;
use serde_json::json;
use solidcraft_engine::cmd::material_estimate::{Basis, Report, Settings};

use crate::{SolveApp, i18n, i18n::tr, theme::Tokens};

pub struct EstimateWindow {
    settings: Settings,
    opened: Settings,
    report: Option<Report>,
    error: Option<String>,
    revision: u64,
    saved: bool,
}

impl EstimateWindow {
    fn from_app(app: &mut SolveApp) -> Self {
        let mut window = Self {
            settings: Settings::default(),
            opened: Settings::default(),
            report: None,
            error: None,
            revision: app.session.revision,
            saved: false,
        };
        window.load(app);
        window
    }

    fn query(app: &mut SolveApp, params: serde_json::Value) -> Result<Report, String> {
        let value = app.run("material.estimate", params)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    fn load(&mut self, app: &mut SolveApp) {
        match Self::query(app, json!({})) {
            Ok(report) => {
                self.settings = report.settings.clone();
                self.opened = report.settings.clone();
                self.saved = app.session.doc.metadata.get(solidcraft_engine::cmd::material_estimate::METADATA_KEY).is_some();
                self.report = Some(report);
                self.error = None;
            }
            Err(error) => {
                self.report = None;
                self.error = Some(error);
            }
        }
        self.revision = app.session.revision;
    }

    fn refresh(&mut self, app: &mut SolveApp) {
        match Self::query(app, json!({"settings": self.settings})) {
            Ok(report) => {
                self.report = Some(report);
                self.error = None;
            }
            Err(error) => {
                self.report = None;
                self.error = Some(error);
            }
        }
        self.revision = app.session.revision;
    }

    fn save(&mut self, app: &mut SolveApp) {
        match app.run("material.estimate.settings", json!({"settings": self.settings})) {
            Ok(_) => {
                self.opened = self.settings.clone();
                self.saved = true;
                self.refresh(app);
            }
            Err(error) => {
                self.report = None;
                self.error = Some(error);
            }
        }
    }
}

pub fn open(app: &mut SolveApp) {
    app.material_estimate = Some(EstimateWindow::from_app(app));
}

fn number(ui: &mut egui::Ui, value: &mut f64, handle: &str, min: f64, max: f64) -> bool {
    let response = ui.add(egui::DragValue::new(value).range(min..=max).speed(1.0).max_decimals(2));
    crate::scenario::publish_handle(handle, response.rect.center());
    response.changed()
}

pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(mut window) = app.material_estimate.take() else { return };
    if window.revision != app.session.revision {
        if window.settings == window.opened {
            window.load(app);
        } else {
            window.refresh(app);
        }
    }
    let mut open = true;
    let (mut changed, mut save, mut close) = (false, false, false);
    crate::frame::window(ctx, i18n::tr_label("Estimate Materials"), crate::frame::Width::Wide)
        .id(egui::Id::new("sc_material_estimate"))
        .open(&mut open)
        .default_width(670.0)
        .show(ctx, |ui| {
            ui.label(tr("ui.material_estimate.scope"));
            ui.label(RichText::new(tr("ui.material_estimate.example")).small().color(Tokens::get().text_dim));
            egui::Grid::new("sc_material_estimate_reference").num_columns(2).spacing([12.0, 7.0]).show(ui, |ui| {
                ui.label(tr("ui.material_estimate.reference"));
                changed |= ui.add(egui::TextEdit::singleline(&mut window.settings.reference_name).char_limit(128).desired_width(280.0)).changed();
                ui.end_row();
                ui.label(tr("ui.material_estimate.price"));
                changed |= number(ui, &mut window.settings.reference_price, "estimate:price", 0.0, 1e9);
                ui.end_row();
                ui.label(tr("ui.material_estimate.dimensions"));
                ui.horizontal(|ui| {
                    changed |= number(ui, &mut window.settings.reference_length_mm, "estimate:length", 0.01, 1e6);
                    ui.label("×");
                    changed |= number(ui, &mut window.settings.reference_width_mm, "estimate:width", 0.01, 1e6);
                    ui.label("×");
                    changed |= number(ui, &mut window.settings.reference_thickness_mm, "estimate:thickness", 0.01, 1e6);
                });
                ui.end_row();
                ui.label(tr("ui.material_estimate.basis"));
                ui.horizontal_wrapped(|ui| {
                    changed |= ui
                        .radio_value(&mut window.settings.basis, Basis::BoundingBox, tr("ui.material_estimate.stock"))
                        .on_hover_text(tr("ui.material_estimate.stock_tip"))
                        .changed();
                    changed |= ui.radio_value(&mut window.settings.basis, Basis::NetVolume, tr("ui.material_estimate.net")).changed();
                });
                ui.end_row();
                ui.label(tr("ui.material_estimate.waste"));
                changed |= number(ui, &mut window.settings.waste_percent, "estimate:waste", 0.0, 1000.0);
                ui.end_row();
            });
            if changed {
                window.refresh(app);
            }
            if let Some(error) = &window.error {
                ui.colored_label(Tokens::get().error, error);
            }
            if let Some(report) = &window.report {
                if report.total.body_count == 0 {
                    ui.label(tr("ui.material_estimate.empty"));
                }
                ui.separator();
                ui.label(i18n::format("ui.material_estimate.rate", &[("price", &format!("{:.2}", report.reference_price_per_m3))]));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:.2} €", report.total.cost)).strong().size(25.0));
                    ui.label(tr("ui.material_estimate.material_only"));
                });
                ui.label(i18n::format(
                    "ui.material_estimate.summary",
                    &[
                        ("count", &report.total.body_count.to_string()),
                        ("net", &format!("{:.2}", report.total.net_volume_m3 * 1000.0)),
                        ("stock", &format!("{:.2}", report.total.stock_volume_m3 * 1000.0)),
                        ("purchase", &format!("{:.2}", report.total.estimated_volume_m3 * 1000.0)),
                    ],
                ));
                ui.add_space(5.0);
                egui::ScrollArea::both()
                    .id_salt("sc_material_estimate_parts")
                    .max_height((ctx.content_rect().height() - 480.0).clamp(90.0, 270.0))
                    .show(ui, |ui| {
                        egui::Grid::new("sc_material_estimate_table").striped(true).spacing([12.0, 5.0]).show(ui, |ui| {
                            for key in ["part", "quantity", "size", "volume", "cost"] {
                                ui.strong(tr(&format!("ui.material_estimate.{key}")));
                            }
                            ui.end_row();
                            for row in &report.bodies {
                                ui.label(&row.name).on_hover_text(&row.material);
                                ui.label(row.quantity.to_string());
                                ui.label(row.dimensions_mm.iter().map(|v| format!("{v:.1}")).collect::<Vec<_>>().join(" × "));
                                ui.label(format!("{:.2}", row.estimated_volume_m3 * 1000.0));
                                ui.label(format!("{:.2} €", row.cost));
                                ui.end_row();
                            }
                        });
                    });
            }
            ui.add_space(4.0);
            ui.label(RichText::new(tr("ui.material_estimate.limits")).small().color(Tokens::get().text_dim));
            ui.separator();
            ui.horizontal(|ui| {
                let response = ui.add_enabled(window.report.is_some(), egui::Button::new(tr("ui.material_estimate.save")));
                crate::scenario::publish_handle("estimate:save", response.rect.center());
                save = response.clicked();
                ui.label(
                    RichText::new(tr(if window.saved && window.settings == window.opened {
                        "ui.material_estimate.saved_note"
                    } else {
                        "ui.material_estimate.draft_note"
                    }))
                    .small()
                    .color(Tokens::get().text_dim),
                );
                close = ui.button(tr("ui.close")).clicked();
            });
        });
    if save {
        window.save(app);
    }
    if open && !close {
        app.material_estimate = Some(window);
    }
}

#[cfg(test)]
#[path = "material_estimate_tests.rs"]
mod tests;
