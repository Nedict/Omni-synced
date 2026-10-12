//! The timeline (bottom): features in order, the rollback marker, errors and warnings.

use egui::{Color32, Rect, RichText, Sense, Stroke, UiBuilder, pos2, vec2};
use serde_json::json;
use solidcraft_engine::doc::FeatureKind;

use crate::theme::Tokens;
use crate::{SolveApp, icons};

fn icon_of(k: &FeatureKind) -> &'static str {
    match k {
        FeatureKind::Sketch { .. } => "sketch",
        FeatureKind::Extrude { .. } => "extrude",
        FeatureKind::Revolve { .. } => "revolve",
        FeatureKind::Fillet { .. } => "fillet",
        FeatureKind::Chamfer { .. } => "chamfer",
        FeatureKind::Box { .. } => "box",
        FeatureKind::Cylinder { .. } => "cylinder",
        FeatureKind::Sphere { .. } => "sphere",
        FeatureKind::Torus { .. } => "torus",
        FeatureKind::Combine { .. } => "combine",
        FeatureKind::Pattern { pattern: solidcraft_engine::doc::PatternKind::Rectangular { .. }, .. } => "pattern_rect",
        FeatureKind::Pattern { .. } => "pattern_circ",
        FeatureKind::Mirror { .. } => "mirror",
        FeatureKind::ConstructionPlane { .. } => "plane",
        FeatureKind::ConstructionAxis { .. } => "axis",
        FeatureKind::ConstructionPoint { .. } => "point",
        FeatureKind::Hole { .. } => "hole",
        FeatureKind::Loft { .. } => "loft",
        FeatureKind::Sweep { .. } => "sweep",
        FeatureKind::Shell { .. } => "shell",
        FeatureKind::Draft { .. } => "draft",
        FeatureKind::Split { .. } => "split",
        FeatureKind::Move { .. } => "move",
        FeatureKind::Import { .. } | FeatureKind::MeshImport { .. } | FeatureKind::MeshConvert { .. } => "import",
        other => crate::workspace::feature_icon(other).unwrap_or("feature"),
    }
}

/// What a drag in the timeline is moving.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Marker,
    Feature(u64),
}

/// Slot index (0..=n) nearest to screen x, given the left edges of the feature buttons.
fn slot_at(xs: &[f32], end: f32, x: f32) -> usize {
    let mut best = (0usize, f32::INFINITY);
    for (i, sx) in xs.iter().chain(std::iter::once(&end)).enumerate() {
        let d = (sx - 2.0 - x).abs();
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// Ask before deleting a feature others depend on; delete right away otherwise.
fn delete_feature(app: &mut SolveApp, id: u64) {
    let deps = app.session.execute("timeline.dependents", &json!({ "feature": id })).unwrap_or_default();
    let list = |k: &str| -> Vec<String> {
        deps.get(k).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect()
    };
    let (with, fail) = (list("deleted_with_it"), list("would_fail"));
    if with.is_empty() && fail.is_empty() {
        let _ = app.run("timeline.delete", json!({ "features": [id.to_string()] }));
    } else {
        app.dialog = Some(crate::dialogs::Dialog::confirm_delete(id, with, fail));
    }
}

/// A standalone left feature-history panel, used when the parts browser is hidden.
pub fn left_panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    egui::Panel::left("sc_timeline_left")
        .exact_size(250.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(6, 6)))
        .show(ui, |ui| left_timeline(app, ui));
}

/// Vertical feature history with named rows, folders and a horizontal rollback bar. The caller
/// gives this a bounded height so the parts and scene browser can use the space below it.
pub fn left_timeline(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    ui.horizontal(|ui| {
        ui.label(RichText::new("FEATURE HISTORY").size(11.0).strong().color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("+").on_hover_text("New feature folder").clicked()
                && let Ok(result) = app.run("timeline.group", json!({}))
                && let Some(group) = result.get("group").and_then(serde_json::Value::as_u64)
            {
                app.tree.collapsed.remove(&format!("timeline_group:{group}"));
            }
        });
    });
    timeline_transport(app, ui);
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("sc_left_timeline_scroll")
        .auto_shrink([false, false])
        .scroll_source(egui::scroll_area::ScrollSource { scroll_bar: true, mouse_wheel: true, ..egui::scroll_area::ScrollSource::NONE })
        .show(ui, |ui| vertical_features(app, ui));
}

fn timeline_transport(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let n = app.session.doc.features.len();
    let marker = app.session.doc.marker.unwrap_or(n);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let controls = [("first", Some(0usize)), ("back", Some(marker.saturating_sub(1))), ("forward", Some((marker + 1).min(n))), ("last", None)];
        for (what, target) in controls {
            let (rect, response) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
            if response.hovered() {
                ui.painter().rect_filled(rect, 3.0, t.hover);
            }
            transport_glyph(ui.painter(), rect.center(), what, t.icon);
            if response
                .on_hover_text(match what {
                    "first" => "Roll back to the start",
                    "back" => "Step back one feature",
                    "forward" => "Step forward one feature",
                    _ => "Play to the end",
                })
                .clicked()
            {
                let params = match target {
                    Some(position) if position < n => json!({"position": position}),
                    _ => json!({}),
                };
                let _ = app.run("timeline.roll_to", params);
            }
        }
    });
}

fn nearest_vertical_slot(slots: &[(f32, usize)], y: f32, fallback: usize) -> usize {
    slots.iter().min_by(|a, b| (a.0 - y).abs().total_cmp(&(b.0 - y).abs())).map(|slot| slot.1).unwrap_or(fallback)
}

fn vertical_marker(app: &mut SolveApp, ui: &mut egui::Ui, drag: &mut Option<Drag>) {
    let t = Tokens::get();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::drag());
    let line = Rect::from_min_max(pos2(rect.left() + 8.0, rect.center().y - 1.5), pos2(rect.right() - 5.0, rect.center().y + 1.5));
    let colour = t.accent;
    ui.painter().rect_filled(line, 1.5, colour);
    ui.painter().circle_filled(pos2(line.right(), line.center().y), 4.0, colour);
    if response.drag_started() {
        *drag = Some(Drag::Marker);
    }
    response.on_hover_text("Rollback bar: drag to roll the model back; new features are inserted here");
    let _ = app;
}

fn group_for(groups: &[solidcraft_engine::doc::TimelineGroup], feature: u64) -> Option<u64> {
    groups.iter().find(|group| group.features.contains(&feature)).map(|group| group.id)
}

fn vertical_group_row(app: &mut SolveApp, ui: &mut egui::Ui, group: &solidcraft_engine::doc::TimelineGroup) -> bool {
    let t = Tokens::get();
    let key = format!("timeline_group:{}", group.id);
    let open = !app.tree.collapsed.contains(&key);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 27.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 3.0, t.hover);
    }
    let centre = pos2(rect.left() + 9.0, rect.center().y);
    let arrow = if open {
        vec![centre + vec2(-4.0, -2.0), centre + vec2(4.0, -2.0), centre + vec2(0.0, 3.0)]
    } else {
        vec![centre + vec2(-2.0, -4.0), centre + vec2(-2.0, 4.0), centre + vec2(3.0, 0.0)]
    };
    ui.painter().add(egui::Shape::convex_polygon(arrow, t.text_dim, Stroke::NONE));
    let icon = Rect::from_min_size(pos2(rect.left() + 18.0, rect.top() + 4.0), vec2(19.0, 19.0));
    icons::paint(ui.painter(), icon, "folder", t.icon, t.icon_fill, t.accent);
    ui.painter().text(pos2(rect.left() + 42.0, rect.center().y), egui::Align2::LEFT_CENTER, &group.name, egui::FontId::proportional(12.0), t.text);
    crate::scenario::publish_handle(&format!("timeline-folder:{}", group.name), rect.center());
    if response.clicked() {
        if open {
            app.tree.collapsed.insert(key.clone());
        } else {
            app.tree.collapsed.remove(&key);
        }
    }
    if response.double_clicked() {
        crate::context_menu::start_rename(app, crate::context_menu::RenameWhat::TimelineGroup(group.id), rect.center());
    }
    response.context_menu(|ui| {
        if ui.button("Rename Folder").clicked() {
            crate::context_menu::start_rename(app, crate::context_menu::RenameWhat::TimelineGroup(group.id), rect.center());
            ui.close();
        }
        if ui.button("Remove Folder").on_hover_text("Keep its features in the history").clicked() {
            let _ = app.run("timeline.ungroup", json!({"group": group.id}));
            ui.close();
        }
    });
    open
}

fn vertical_features(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let features: Vec<_> = app
        .session
        .doc
        .features
        .iter()
        .map(|feature| (feature.id, feature.name.clone(), icon_of(&feature.kind), feature.suppressed, feature.kind.type_name(), feature.kind.clone()))
        .collect();
    let groups = app.session.doc.timeline_groups.clone();
    let n = features.len();
    let marker = app.session.doc.marker.unwrap_or(n);
    let from_view = match &app.viewport.hover {
        Some(crate::viewport::Hit::Face { body, .. } | crate::viewport::Hit::Edge { body, .. } | crate::viewport::Hit::Vertex { body, .. }) => {
            app.session.world_state().body(body).map(|body| body.feature)
        }
        _ => None,
    };
    let drag_id = ui.id().with("tl_vertical_drag");
    let mut drag: Option<Drag> = ui.data(|data| data.get_temp(drag_id));
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let mut slots = Vec::new();
    let mut shown_groups = std::collections::BTreeSet::new();
    let mut open_groups = std::collections::BTreeMap::new();
    let mut hovered = None;
    let mut edit = None;

    for (index, (id, name, icon, suppressed, ty, kind)) in features.iter().enumerate() {
        let group_id = group_for(&groups, *id);
        if let Some(group_id) = group_id
            && shown_groups.insert(group_id)
            && let Some(group) = groups.iter().find(|group| group.id == group_id)
        {
            open_groups.insert(group_id, vertical_group_row(app, ui, group));
        }
        let group_open = group_id.and_then(|id| open_groups.get(&id).copied()).unwrap_or(true);
        if index == marker {
            slots.push((ui.cursor().top(), index));
            vertical_marker(app, ui, &mut drag);
        }
        if !group_open {
            continue;
        }
        slots.push((ui.cursor().top(), index));
        let indent = if group_id.is_some() { 16.0 } else { 0.0 };
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click_and_drag());
        crate::scenario::publish_handle(&format!("timeline:{name}"), rect.center());
        let result = app.session.model.result(*id);
        let error = result.and_then(|result| result.error.clone());
        let warning = result.and_then(|result| result.warning.clone());
        let rolled = index >= marker;
        let selected =
            app.session.selection.iter().any(|selection| matches!(selection, solidcraft_engine::Sel::Feature { id: selected } if selected == id));
        let background = if error.is_some() {
            t.timeline_item_error
        } else if selected {
            t.accent_soft
        } else if response.hovered() || from_view == Some(*id) {
            t.hover
        } else {
            Color32::TRANSPARENT
        };
        if background != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 3.0, background);
        }
        let icon_rect = Rect::from_min_size(pos2(rect.left() + 4.0 + indent, rect.top() + 4.0), vec2(22.0, 22.0));
        let ink = if rolled || *suppressed { t.border } else { t.icon };
        icons::paint(ui.painter(), icon_rect, icon, ink, if rolled { t.panel_header } else { t.icon_fill }, if rolled { t.border } else { t.accent });
        if *suppressed {
            ui.painter().line_segment([icon_rect.left_bottom(), icon_rect.right_top()], Stroke::new(1.5, t.text_dim));
        }
        let text_colour = if rolled || *suppressed { t.text_dim } else { t.text };
        ui.painter().text(
            pos2(icon_rect.right() + 7.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(12.0),
            text_colour,
        );
        if warning.is_some() && error.is_none() {
            icons::paint(
                ui.painter(),
                Rect::from_min_size(pos2(rect.right() - 15.0, rect.center().y - 6.0), vec2(12.0, 12.0)),
                "warning",
                t.icon,
                t.warning,
                t.warning,
            );
        }
        if response.hovered() {
            hovered = Some(*id);
        }
        if response.drag_started() {
            drag = Some(Drag::Feature(*id));
        }
        let mut tip = format!("{name}  ({ty})");
        if let Some(error) = &error {
            tip.push_str(&format!("\nError: {error}"));
        }
        if let Some(warning) = &warning {
            tip.push_str(&format!("\nWarning: {warning}"));
        }
        tip.push_str("\nDouble-click to edit · drag to reorder · right-click for more");
        let response = response.on_hover_text(tip);
        if response.clicked() {
            let _ = app.run("select.set", json!({"items": [{"type": "feature", "id": id}]}));
        }
        if response.double_clicked() {
            edit = Some(*id);
        }
        let sketch = match kind {
            FeatureKind::Extrude { sketch, .. } | FeatureKind::Revolve { sketch, .. } | FeatureKind::Sweep { sketch, .. } => Some(*sketch),
            _ => None,
        };
        response.context_menu(|ui| {
            if ui.button("Edit Feature").clicked() {
                edit = Some(*id);
                ui.close();
            }
            if let Some(sketch) = sketch
                && ui.button("Edit Profile Sketch").clicked()
            {
                app.edit_sketch(sketch);
                ui.close();
            }
            if ui.button("Rename Feature").clicked() {
                crate::context_menu::start_rename(app, crate::context_menu::RenameWhat::Feature(*id), rect.center());
                ui.close();
            }
            if ui.button(if *suppressed { "Unsuppress Feature" } else { "Suppress Feature" }).clicked() {
                let _ = app.run("timeline.suppress", json!({"feature": id}));
                ui.close();
            }
            if ui.button("Roll Back Here").clicked() {
                let _ = app.run("timeline.roll_to", json!({"feature": id}));
                ui.close();
            }
            ui.separator();
            if group_id.is_some() && ui.button("Remove from Folder").clicked() {
                let _ = app.run("timeline.move_to_group", json!({"features": [id], "group": null}));
                ui.close();
            }
            ui.menu_button("Move to Folder", |ui| {
                for group in &groups {
                    if Some(group.id) != group_id && ui.button(&group.name).clicked() {
                        let _ = app.run("timeline.move_to_group", json!({"features": [id], "group": group.id}));
                        app.tree.collapsed.remove(&format!("timeline_group:{}", group.id));
                        ui.close();
                    }
                }
                if groups.is_empty() {
                    ui.label("No folders yet");
                }
            });
            if ui.button("New Folder with Feature").clicked() {
                let _ = app.run("timeline.group", json!({"features": [id]}));
                ui.close();
            }
            ui.separator();
            if ui.button("Delete").clicked() {
                delete_feature(app, *id);
                ui.close();
            }
        });
    }
    for group in groups.iter().filter(|group| group.features.iter().all(|feature| !features.iter().any(|item| item.0 == *feature))) {
        if shown_groups.insert(group.id) {
            vertical_group_row(app, ui, group);
        }
    }
    if marker >= n {
        slots.push((ui.cursor().top(), n));
        vertical_marker(app, ui, &mut drag);
    }
    crate::dialogs_assembly::timeline_joints_vertical(app, ui);

    if let (Some(active), Some(pointer)) = (drag, pointer) {
        let slot = nearest_vertical_slot(&slots, pointer.y, n);
        let y = slots.iter().find(|(_, position)| *position == slot).map(|(y, _)| *y).unwrap_or(ui.cursor().top());
        ui.painter().line_segment(
            [pos2(ui.min_rect().left() + 4.0, y), pos2(ui.max_rect().right() - 5.0, y)],
            Stroke::new(2.0, if active == Drag::Marker { t.accent } else { t.sketch_accent }),
        );
    }
    if ui.input(|input| input.pointer.any_released())
        && let Some(active) = drag.take()
        && let Some(pointer) = pointer
    {
        let slot = nearest_vertical_slot(&slots, pointer.y, n);
        match active {
            Drag::Marker => {
                let _ = app.run("timeline.roll_to", if slot >= n { json!({}) } else { json!({"position": slot}) });
            }
            Drag::Feature(id) => {
                let from = app.session.doc.feature_index(id).unwrap_or(0);
                let to = if slot > from { slot.saturating_sub(1) } else { slot };
                if to != from {
                    let _ = app.run("timeline.reorder", json!({"feature": id, "position": to}));
                }
            }
        }
    }
    ui.data_mut(|data| match drag {
        Some(active) => {
            data.insert_temp(drag_id, active);
        }
        None => data.remove::<Drag>(drag_id),
    });
    app.viewport.hover_feature = hovered;
    if let Some(feature) = edit {
        app.edit_feature(feature);
    }
}

pub fn timeline(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    egui::Panel::bottom("sc_timeline").exact_size(40.0).frame(egui::Frame::NONE.fill(t.timeline)).show(ui, |ui| {
        let r = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, t.border));
        let n = app.session.doc.features.len();
        let marker = app.session.doc.marker.unwrap_or(n);
        // Playback controls.
        let mut x = r.left() + 10.0;
        let controls = [("first", Some(0usize)), ("back", Some(marker.saturating_sub(1))), ("forward", Some((marker + 1).min(n))), ("last", None)];
        for (what, target) in controls {
            let br = Rect::from_center_size(pos2(x + 11.0, r.center().y), vec2(22.0, 22.0));
            let resp = ui.interact(br, ui.id().with(("tl", what)), Sense::click());
            if resp.hovered() {
                p.rect_filled(br, 3.0, t.hover);
            }
            transport_glyph(&p, br.center(), what, t.icon);
            if resp
                .on_hover_text(match what {
                    "first" => "Roll back to the start",
                    "back" => "Step back one feature",
                    "forward" => "Step forward one feature",
                    _ => "Play to the end",
                })
                .clicked()
            {
                let pos = match target {
                    Some(k) if k < n => json!({"position": k}),
                    _ => json!({}),
                };
                let _ = app.run("timeline.roll_to", pos);
            }
            x += 24.0;
        }
        x += 14.0;
        // The features scroll horizontally in the space right of the playback controls.
        let strip = Rect::from_min_max(pos2(x, r.top()), r.right_bottom());
        ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
            egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
                let p = ui.painter().clone();
                let mut x = ui.max_rect().left();
                let origin = x;
                let feats: Vec<_> = app
                    .session
                    .doc
                    .features
                    .iter()
                    .map(|f| (f.id, f.name.clone(), icon_of(&f.kind), f.suppressed, f.kind.type_name(), f.kind.clone()))
                    .collect();
                // The feature whose body is under the cursor in the viewport.
                let st = app.session.world_state();
                let from_view = match &app.viewport.hover {
                    Some(
                        crate::viewport::Hit::Face { body, .. } | crate::viewport::Hit::Edge { body, .. } | crate::viewport::Hit::Vertex { body, .. },
                    ) => st.body(body).map(|b| b.feature),
                    _ => None,
                };
                let drag_id = ui.id().with("tl_drag");
                let mut drag: Option<Drag> = ui.data(|d| d.get_temp(drag_id));
                let pointer = ui.input(|i| i.pointer.hover_pos());
                let mut xs: Vec<f32> = Vec::new();
                let mut hovered: Option<u64> = None;
                let mut edit: Option<u64> = None;
                let mut marker_rect: Option<Rect> = None;
                for (i, (id, name, icon, suppressed, ty, kind)) in feats.iter().enumerate() {
                    if i == marker {
                        marker_rect = Some(Rect::from_min_size(pos2(x, r.top() + 5.0), vec2(6.0, r.height() - 10.0)));
                        x += 10.0;
                    }
                    xs.push(x);
                    let br = Rect::from_min_size(pos2(x, r.top() + 6.0), vec2(28.0, 28.0));
                    crate::scenario::publish_handle(&format!("timeline:{name}"), br.center());
                    let resp = ui.interact(br, ui.id().with(("feat", *id)), Sense::click_and_drag());
                    let res = app.session.model.result(*id);
                    let err = res.and_then(|r| r.error.clone());
                    let warn = res.and_then(|r| r.warning.clone());
                    let rolled = i >= marker;
                    let is_it = |s: &solidcraft_engine::Sel| matches!(s, solidcraft_engine::Sel::Feature { id: f } if f == id);
                    let sel = app.session.selection.iter().any(is_it) || app.dialog.as_ref().is_some_and(|d| d.items().iter().any(is_it));
                    if resp.hovered() {
                        hovered = Some(*id);
                    }
                    let bg = if err.is_some() {
                        t.timeline_item_error
                    } else if sel {
                        t.accent_soft
                    } else if resp.hovered() || from_view == Some(*id) {
                        t.hover
                    } else {
                        t.timeline_item
                    };
                    let edge = if err.is_some() {
                        t.error
                    } else if from_view == Some(*id) {
                        t.accent
                    } else {
                        t.border
                    };
                    p.rect(br, 3.0, bg, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
                    let ink = if rolled || *suppressed { t.border } else { t.icon };
                    icons::paint(
                        &p,
                        br.shrink(4.0),
                        icon,
                        ink,
                        if rolled { t.panel_header } else { t.icon_fill },
                        if rolled { t.border } else { t.accent },
                    );
                    if *suppressed {
                        p.line_segment([br.left_bottom() + vec2(4.0, -4.0), br.right_top() + vec2(-4.0, 4.0)], Stroke::new(1.5, t.text_dim));
                    }
                    if warn.is_some() && err.is_none() {
                        icons::paint(
                            &p,
                            Rect::from_min_size(br.right_top() - vec2(10.0, 0.0), vec2(10.0, 10.0)),
                            "warning",
                            t.icon,
                            t.warning,
                            t.warning,
                        );
                    }
                    let mut tip = format!("{name}  ({ty})");
                    if let Some(e) = &err {
                        tip += &format!("\nError: {e}");
                    }
                    if let Some(w) = &warn {
                        tip += &format!("\nWarning: {w}");
                    }
                    if let Some(r) = res {
                        tip += &format!("\n{:.1} ms", r.ms);
                    }
                    tip += "\nDouble-click to edit · drag to reorder · right-click for more";
                    if resp.drag_started() {
                        drag = Some(Drag::Feature(*id));
                    }
                    let resp = resp.on_hover_text(tip);
                    if resp.clicked() {
                        // A dialog input that takes features (pattern objects) gets the click.
                        let takes = app.dialog.as_ref().and_then(|d| d.active_input()).is_some_and(|i| i.accept & crate::selection::FEATURES != 0);
                        if takes {
                            if let Some(mut d) = app.dialog.take() {
                                d.pick(&app.session, solidcraft_engine::Sel::Feature { id: *id });
                                app.dialog = Some(d);
                            }
                        } else {
                            let _ = app.run("select.set", json!({"items": [{"type": "feature", "id": id}]}));
                        }
                    }
                    if resp.double_clicked() {
                        edit = Some(*id);
                    }
                    let sketch_of = match kind {
                        FeatureKind::Extrude { sketch, .. } | FeatureKind::Revolve { sketch, .. } | FeatureKind::Sweep { sketch, .. } => {
                            Some(*sketch)
                        }
                        _ => None,
                    };
                    resp.context_menu(|ui| {
                        if ui.button("Edit Feature").clicked() {
                            edit = Some(*id);
                            ui.close();
                        }
                        if let Some(sk) = sketch_of
                            && ui.button("Edit Profile Sketch").clicked()
                        {
                            app.edit_sketch(sk);
                            ui.close();
                        }
                        if ui.button("Rename").clicked() {
                            app.dialog = Some(crate::dialogs::Dialog::rename(*id, name));
                            ui.close();
                        }
                        if ui.button(if *suppressed { "Unsuppress Features" } else { "Suppress Features" }).clicked() {
                            let _ = app.run("timeline.suppress", json!({"feature": id}));
                            ui.close();
                        }
                        if ui.button("Roll History Marker Here").clicked() {
                            let _ = app.run("timeline.roll_to", json!({ "feature": id }));
                            ui.close();
                        }
                        if ui.button("Find in Browser").clicked() {
                            find_in_browser(app, *id);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Delete").clicked() {
                            delete_feature(app, *id);
                            ui.close();
                        }
                    });
                    x += 32.0;
                }
                let end = x;
                if marker >= feats.len() {
                    marker_rect = Some(Rect::from_min_size(pos2(x, r.top() + 5.0), vec2(6.0, r.height() - 10.0)));
                }
                // The history marker: drag it to roll the model back or forward.
                if let Some(mr) = marker_rect {
                    let resp = ui.interact(mr.expand2(vec2(3.0, 0.0)), ui.id().with("tl_marker"), Sense::drag());
                    if resp.drag_started() {
                        drag = Some(Drag::Marker);
                    }
                    let col = if resp.hovered() || drag == Some(Drag::Marker) { t.accent } else { t.timeline_marker };
                    p.rect_filled(mr, 1.0, col);
                    resp.on_hover_text("History marker: drag to roll the model back; new features go here");
                }
                let content_end = crate::dialogs_assembly::timeline_joints(app, ui, &p, end + 10.0, r);
                // The scroll range follows the drawn content (the buttons don't take layout space themselves).
                ui.allocate_exact_size(vec2((content_end - origin).max(0.0), r.height()), Sense::hover());
                // Drag feedback, and the drop.
                if let (Some(dg), Some(pp)) = (drag, pointer) {
                    let slot = slot_at(&xs, end, pp.x);
                    let sx = xs.get(slot).copied().unwrap_or(end) - 4.0;
                    p.line_segment(
                        [pos2(sx, r.top() + 3.0), pos2(sx, r.bottom() - 3.0)],
                        Stroke::new(2.0, if dg == Drag::Marker { t.accent } else { t.sketch_accent }),
                    );
                }
                let released = ui.input(|i| i.pointer.any_released());
                // (Taking the drag ends it even when the pointer has left the window.)
                if released
                    && let Some(dg) = drag.take()
                    && let Some(pp) = pointer
                {
                    let slot = slot_at(&xs, end, pp.x);
                    match dg {
                        Drag::Marker => {
                            let _ = app.run("timeline.roll_to", if slot >= n { json!({}) } else { json!({ "position": slot }) });
                        }
                        Drag::Feature(id) => {
                            let from = app.session.doc.feature_index(id).unwrap_or(0);
                            // Dropping after itself means the slot shifts by one.
                            let to = if slot > from { slot - 1 } else { slot };
                            if to != from {
                                let _ = app.run("timeline.reorder", json!({ "feature": id, "position": to }));
                            }
                        }
                    }
                }
                ui.data_mut(|d| match drag {
                    Some(dg) => {
                        d.insert_temp(drag_id, dg);
                    }
                    None => d.remove::<Drag>(drag_id),
                });
                app.viewport.hover_feature = hovered;
                if let Some(id) = edit {
                    app.edit_feature(id);
                }
            });
        });
    });
}

/// Select what a feature made, so the browser shows it.
fn find_in_browser(app: &mut SolveApp, id: u64) {
    let st = app.session.world_state();
    let bodies: Vec<serde_json::Value> = st.bodies.iter().filter(|b| b.feature == id).map(|b| json!({"type": "body", "name": b.name})).collect();
    let items = if bodies.is_empty() { vec![json!({"type": "feature", "id": id})] } else { bodies };
    let _ = app.run("select.set", json!({ "items": items }));
}

/// Transport buttons drawn as shapes (the UI font has no media glyphs).
fn transport_glyph(p: &egui::Painter, c: egui::Pos2, what: &str, col: Color32) {
    let tri = |dir: f32, dx: f32| {
        let pts = vec![pos2(c.x + dx - 4.0 * dir, c.y - 5.0), pos2(c.x + dx + 4.0 * dir, c.y), pos2(c.x + dx - 4.0 * dir, c.y + 5.0)];
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    };
    let bar = |dx: f32| p.line_segment([pos2(c.x + dx, c.y - 5.0), pos2(c.x + dx, c.y + 5.0)], Stroke::new(1.6, col));
    match what {
        "first" => {
            bar(-5.0);
            tri(-1.0, 1.0);
        }
        "back" => tri(-1.0, 0.0),
        "forward" => tri(1.0, 0.0),
        _ => {
            tri(1.0, -1.0);
            bar(5.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Harness, handle_at};

    #[test]
    fn left_history_shows_named_features_and_folders_above_the_browser() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        harness.app.run("solid.box", json!({"length": 20, "width": 15, "height": 10, "name": "Base"})).unwrap();
        harness.app.run("solid.fillet", json!({"edges": [[20, 7.5, 10]], "radius": 1, "name": "Round"})).unwrap();
        harness.app.run("timeline.group", json!({"name": "Housing", "features": ["Base", "Round"]})).unwrap();
        harness.frames(4);

        let folder = handle_at("timeline-folder:Housing").unwrap();
        let base = handle_at("timeline:Base").unwrap();
        let round = handle_at("timeline:Round").unwrap();
        let browser = handle_at("row:Untitled").unwrap();
        assert!(folder.x < 250.0 && base.x < 250.0 && round.x < 250.0);
        assert!(folder.y < base.y && base.y < round.y);
        assert!(round.y < browser.y, "the parts/scene browser belongs below the feature history");
    }
}
