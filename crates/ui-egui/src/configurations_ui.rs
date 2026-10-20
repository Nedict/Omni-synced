//! Manage configuration table. Commands and cell decoding remain in dialogs_motion.
use super::{cell_value, column_parts, configuration_rules_ui, table, unit_of};
use crate::{SolveApp, ui_kit};
use serde_json::{Value, json};
/// The Configurations table, its rules and errors.
pub(super) fn configurations_rows(
    app: &mut SolveApp,
    ui: &mut egui::Ui,
    new_row: &mut String,
    error: &mut Option<String>,
    rule_editor: &mut configuration_rules_ui::Editor,
) {
    // Physical materials for the configuration table's Library: the library's, then the design's own.
    let material_names = app.session.doc.material_names();
    let revision = app.session.revision;
    let v = rule_editor.table.as_ref().filter(|(r, _)| *r == revision).map(|(_, v)| v.clone()).unwrap_or_else(|| {
        let value = app.session.execute("config.table", &json!({})).unwrap_or(Value::Null);
        rule_editor.table = Some((revision, value.clone()));
        value
    });
    let mut tb = table(&v);
    // Bare numbers show with their parameter's unit ("40" is 40 mm), as Fusion shows them.
    let kinds = app.session.doc.all_param_exprs();
    for (_, vals) in &mut tb.rows {
        for (c, val) in tb.columns.iter().zip(vals.iter_mut()) {
            let (kind, n) = column_parts(c);
            if kind == "param"
                && val.trim().parse::<f64>().is_ok()
                && let Some(unit) = app.session.doc.param(n).map(|parameter| parameter.unit.clone()).or_else(|| {
                    kinds.iter().find(|x| x.0 == n).and_then(|x| {
                        unit_of(x.2).map(|unit| {
                            if x.2 == solidcraft_engine::doc::expr::Kind::Length { app.session.doc.units.clone() } else { unit.to_string() }
                        })
                    })
                })
            {
                *val = format!("{} {unit}", val.trim());
            }
        }
    }
    let run = |app: &mut SolveApp, error: &mut Option<String>, id: &str, p: Value| {
        *error = app.run(id, p).err();
    };
    let labels: Vec<_> = std::iter::once(crate::i18n::tr_label("Configuration").to_owned())
        .chain(tb.columns.iter().map(|c| {
            let (kind, name) = column_parts(c);
            v["targets"]
                .as_array()
                .and_then(|targets| targets.iter().find(|target| target["key"].as_str() == Some(c)))
                .and_then(|target| target["label"].as_str())
                .filter(|_| matches!(kind, "component_appearance" | "face_appearance"))
                .map(str::to_string)
                .unwrap_or_else(|| match kind {
                    "material" => format!("{}: {name}", crate::i18n::tr_label("Material")),
                    "appearance" => format!("{}: {name}", crate::i18n::tr_label("Appearance")),
                    "suppress" => format!("{}: {name}", crate::i18n::tr_label("Suppress")),
                    _ => name.to_owned(),
                })
        }))
        .chain(std::iter::once(crate::i18n::tr_label("Actions").to_owned()))
        .collect();
    let gap = ui.spacing().item_spacing.x;
    let name_width = tb.rows.iter().map(|(name, _)| ui_kit::ContentWidth::label(ui, name)).fold(0.0, f32::max);
    let status_width = (ui_kit::ContentWidth::label(ui, "Active") + gap + ui_kit::ContentWidth::button(ui, "Apply"))
        .max(ui_kit::ContentWidth::button(ui, "Activate"));
    let columns: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let min_width = if index == 0 {
                (name_width + gap + status_width).max(ui_kit::ContentWidth::label(ui, "Remove the column"))
            } else if index <= tb.columns.len() {
                tb.rows
                    .iter()
                    .filter_map(|(_, values)| values.get(index - 1))
                    .map(|value| ui_kit::ContentWidth::editor(ui, value))
                    .fold(ui_kit::ContentWidth::button(ui, "×"), f32::max)
            } else {
                ui_kit::ContentWidth::button(ui, "×")
            };
            ui_kit::Column::new(label, 2.0).min_content_width(min_width)
        })
        .collect();
    ui_kit::table(ui, "configuration_table", &columns, crate::i18n::tr("manage.empty"), |ui, table| {
        // The column-action band shares the header's exact widths without posing as a data row.
        if !tb.columns.is_empty() {
            table.column_actions(ui, |row| {
                row.cell(|ui| {
                    ui_kit::text(ui, crate::i18n::tr_label("Remove the column"), ui_kit::Text::Cell);
                });
                for c in &tb.columns {
                    row.cell(|ui| {
                        let (kind, name) = column_parts(c);
                        if ui_kit::button(ui, "×", ui_kit::Button::Normal).on_hover_text(crate::i18n::tr_label("Remove the column")).clicked() {
                            let key = if kind == "suppress" { "feature" } else { kind };
                            run(app, error, "config.column", json!({key:if kind == "face_appearance" {c.as_str()} else {name},"delete":true}));
                        }
                    });
                }
                row.cell(|_| {});
            });
        }
        for (name, values) in &tb.rows {
            let active = tb.active.as_deref() == Some(name.as_str());
            table.row(ui, name, active, |row| {
                row.cell(|ui| {
                    // A measured name slot keeps status/actions inside the Configuration column.
                    ui.allocate_ui_with_layout(
                        egui::vec2(name_width, ui_kit::row_height()),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui_kit::text(ui, name, ui_kit::Text::Cell);
                        },
                    );
                    if active {
                        ui_kit::text(ui, crate::i18n::tr_label("Active"), ui_kit::Text::Inline);
                        if v.get("matches").and_then(Value::as_bool) == Some(false) {
                            let apply = ui_kit::button(ui, crate::i18n::tr_label("Apply"), ui_kit::Button::Small);
                            crate::scenario::publish_handle(&format!("config:activate:{name}"), apply.rect.center());
                            if apply.clicked() {
                                run(app, error, "config.activate", json!({"row":name}));
                            }
                        }
                    } else {
                        let activate = ui_kit::button(ui, crate::i18n::tr_label("Activate"), ui_kit::Button::Small);
                        crate::scenario::publish_handle(&format!("config:activate:{name}"), activate.rect.center());
                        if activate.clicked() {
                            run(app, error, "config.activate", json!({"row":name}));
                        }
                    }
                });
                for (column, value) in tb.columns.iter().zip(values) {
                    row.cell(|ui| {
                        let (kind, _) = column_parts(column);
                        let appearance = matches!(kind, "appearance" | "component_appearance" | "face_appearance");
                        let shown = if appearance {
                            solidcraft_engine::doc::config::appearance_cell(value)
                                .ok()
                                .flatten()
                                .filter(|look| solidcraft_engine::doc::appearance::library(&look.name).as_ref() == Some(look))
                                .map(|look| look.name)
                                .unwrap_or_else(|| value.clone())
                        } else {
                            value.clone()
                        };
                        if kind == "material" || appearance {
                            let mut choice = None;
                            ui_kit::action_menu(ui, "Library", |ui| {
                                let follow = if kind == "appearance" {
                                    "Follow material"
                                } else if appearance {
                                    "Follow inherited appearance"
                                } else {
                                    "Default material"
                                };
                                if ui_kit::button(ui, crate::i18n::tr_label(follow), ui_kit::Button::Normal).clicked() {
                                    choice = Some("Default".to_owned());
                                    ui.close();
                                }
                                let choices: Vec<_> = if kind == "material" {
                                    material_names.iter().filter(|n| *n != "Default").map(String::as_str).collect()
                                } else {
                                    solidcraft_engine::doc::appearance::LIBRARY.iter().map(|(name, _, _)| *name).collect()
                                };
                                for name in choices {
                                    if ui_kit::button(ui, name, ui_kit::Button::Normal).clicked() {
                                        choice = Some(name.to_owned());
                                        ui.close();
                                    }
                                }
                            });
                            if let Some(choice) = choice {
                                run(app, error, "config.row", json!({"name":name,"values":{column:choice}}));
                            }
                        }
                        let key = (name.clone(), column.clone());
                        let mut text = rule_editor.drafts.get(&key).cloned().unwrap_or_else(|| shown.clone());
                        let id = ui.make_persistent_id(&key);
                        let response = ui_kit::cell_editor(ui, id, &mut text, "", false);
                        crate::scenario::publish_handle(&format!("config:cell:{name}:{column}"), response.rect.center());
                        let focused = response.has_focus();
                        let completed = crate::params_dialog::complete(ui, &response, &mut text);
                        if response.changed() || completed {
                            rule_editor.drafts.insert(key.clone(), text.clone());
                        }
                        let enter = !completed && focused && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                        if response.lost_focus() || enter {
                            rule_editor.drafts.remove(&key);
                            if text != shown {
                                run(app, error, "config.row", json!({"name":name,"values":{column:cell_value(column,&text)}}));
                            }
                        }
                    });
                }
                row.cell(|ui| {
                    if ui_kit::button(ui, "×", ui_kit::Button::Small).on_hover_text(crate::i18n::tr_label("Delete the row")).clicked() {
                        run(app, error, "config.row", json!({"name":name,"delete":true}));
                    }
                });
            });
        }
    });
    // New row (a copy of the active one) and new column.
    ui_kit::row(ui, "Configuration", |ui| {
        let id = ui.make_persistent_id("new_configuration");
        ui_kit::cell_editor(ui, id, new_row, "new configuration", false);
    });
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

        if ui_kit::button(ui, crate::i18n::tr_label("Add Row"), ui_kit::Button::Normal)
            .on_hover_text(crate::i18n::tr_label("A new configuration, a copy of the active one"))
            .clicked()
        {
            let name = if new_row.trim().is_empty() {
                (tb.rows.len() + 1..).map(|k| format!("Configuration{k}")).find(|n| !tb.rows.iter().any(|r| &r.0 == n)).unwrap_or_default()
            } else {
                new_row.trim().to_string()
            };
            let mut p = json!({ "name": name });
            if let Some(a) = &tb.active {
                p["from"] = json!(a);
            }
            run(app, error, "config.row", p);
            new_row.clear();
        }
        // A column: a parameter's value or a feature's suppression.
        let mut options: Vec<(String, Value)> = Vec::new();
        for p in app.session.doc.all_param_exprs() {
            if !tb.columns.contains(&format!("param:{}", p.0)) {
                options.push((format!("{} (parameter)", p.0), json!({ "param": p.0 })));
            }
        }
        for f in &app.session.doc.features {
            if !tb.columns.contains(&format!("suppress:{}", f.name)) {
                options.push((format!("Suppress {}", f.name), json!({ "feature": f.name })));
            }
        }
        for body in &app.session.model.state().bodies {
            for kind in ["material", "appearance"] {
                if !tb.columns.contains(&format!("{kind}:{}", body.name)) {
                    options.push((format!("{} ({kind})", body.name), json!({kind: body.name})));
                }
            }
        }
        for component in &app.session.doc.components {
            if component.id != 0 && !tb.columns.contains(&format!("component_appearance:{}", component.id)) {
                options.push((format!("{} (component appearance)", component.name), json!({"component_appearance":component.id})));
            }
        }
        for selected in &app.session.selection {
            if let solidcraft_engine::Sel::Face { body, point, .. } = selected {
                options.push((format!("Selected face on {body} (shared appearance)"), json!({"face_appearance":{"body":body,"point":point}})));
            }
        }
        let mut pick: Option<Value> = None;
        ui_kit::action_menu(ui, "Add Column", |ui| {
            egui::ScrollArea::vertical().max_height(ui_kit::row_height() * 12.0).show(ui, |ui| {
                for (label, p) in &options {
                    if ui_kit::button(ui, label.as_str(), ui_kit::Button::Normal).clicked() {
                        pick = Some(p.clone());
                        ui.close();
                    }
                }
                ui_kit::action_menu(ui, "Face Appearance", |ui| {
                    ui_kit::text(
                        ui,
                        crate::i18n::tr_label("Semantic faces share their appearance across instances."),
                        ui_kit::Text::Note("semantic_faces"),
                    );
                    let state = app.session.model.state();
                    for body in &state.bodies {
                        if body.body.is_mesh() {
                            continue;
                        }
                        ui_kit::action_menu(ui, &format!("Faces on {}", body.name), |ui| {
                            if let Ok(faces) = body.body.faces((body.body.size() * 2e-3).max(1e-3)) {
                                let mut seen = std::collections::BTreeSet::new();
                                let mesh = body.mesh();
                                let mut points = std::collections::BTreeMap::new();
                                for (triangle, index) in mesh.triangles.iter().zip(&mesh.tri_face) {
                                    if let std::collections::btree_map::Entry::Vacant(entry) = points.entry(*index as usize)
                                        && let Some([a, b, c]) = mesh.tri(triangle)
                                    {
                                        entry.insert((a + b + c) * (1. / 3.));
                                    }
                                }
                                for face in faces {
                                    // A curved face centroid can lie on another surface.
                                    // Pick an interior triangle point on this actual face.
                                    let Some(point) = points.get(&face.index).copied() else { continue };
                                    let Ok(target) = app.session.doc.configuration_face_target(body, face.index, point) else { continue };
                                    let supported = if target.certificate.is_some() {
                                        // Capture read this exact face's current source certificate.
                                        // Enumerate its pieces once in the command, not per menu face.
                                        app.session.doc.certify_config_face(&target).is_ok()
                                    } else {
                                        app.session.doc.configuration_face_indices(body, &target).is_ok()
                                    };
                                    if !supported || !seen.insert(target.name.clone()) {
                                        continue;
                                    }
                                    let Some((_, role)) = target.name.split_once(':') else { continue };
                                    if ui_kit::button(ui, role, ui_kit::Button::Normal).clicked() {
                                        pick = Some(json!({"face_appearance":{"target":target}}));
                                        ui.close();
                                    }
                                }
                            }
                        });
                    }
                });
            });
        });
        if let Some(p) = pick {
            // The first column starts the table (with a Default row of the current values).
            let (id, p) = if tb.columns.is_empty() {
                (
                    "config.configure",
                    if let Some(n) = p.get("param") {
                        json!({ "params": [n] })
                    } else if let Some(n) = p.get("feature") {
                        json!({ "features": [n] })
                    } else if let Some(n) = p.get("material") {
                        json!({ "materials": [n] })
                    } else if let Some(n) = p.get("component_appearance") {
                        json!({ "component_appearances": [n] })
                    } else if let Some(n) = p.get("face_appearance") {
                        json!({ "face_appearances": [n] })
                    } else {
                        json!({ "appearances": [p.get("appearance")] })
                    },
                )
            } else {
                ("config.column", p)
            };
            run(app, error, id, p);
        }
    });
    configuration_rules_ui::show(ui, app, error, rule_editor);
    if let Some(styles) = v["style_status"].as_array() {
        for style in styles.iter().filter(|style| style["dormant"].as_bool() == Some(true)) {
            ui_kit::text(
                ui,
                &format!("{}: appearance dormant while its feature is suppressed", style["target"].as_str().unwrap_or("Face")),
                ui_kit::Text::Report("dormant", false),
            );
        }
    }
    if let Some(message) = v["style_error"].as_str() {
        ui_kit::text(ui, message, ui_kit::Text::Report("style_error", true));
    }
    if let Some(message) = error {
        ui_kit::text(ui, message, ui_kit::Text::Report("config_error", true));
    }
    // Table cells and rule editors own Enter. As in the legacy wide manager, it
    // commits an edited cell without also submitting the enclosing card.
    ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
}
