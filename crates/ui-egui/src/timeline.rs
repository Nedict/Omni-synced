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
#[derive(Clone, Debug, PartialEq)]
enum Drag {
    Marker,
    Feature(u64),
    Items(DragItems),
}

#[derive(Clone, Debug, PartialEq)]
struct DragItems {
    /// Every feature that moves in timeline order, including contents of selected folders.
    reorder: Vec<u64>,
    /// Features selected directly, whose folder membership changes on an indented drop.
    features: Vec<u64>,
    /// Folders selected directly, whose parent changes on an indented drop.
    groups: Vec<u64>,
    /// Horizontal pointer position where the drag began, used to detect intentional nesting.
    origin_x: f32,
}

#[derive(Clone, Copy, Debug)]
struct VerticalSlot {
    y: f32,
    position: usize,
    before_group: Option<u64>,
    after_group: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimelineItem {
    Group(u64),
    Feature(u64),
}

impl TimelineItem {
    fn key(self) -> String {
        match self {
            TimelineItem::Group(id) => format!("g:{id}"),
            TimelineItem::Feature(id) => format!("f:{id}"),
        }
    }
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
    delete_features(app, vec![id]);
}

fn delete_features(app: &mut SolveApp, ids: Vec<u64>) {
    let mut with = std::collections::BTreeSet::new();
    let mut fail = std::collections::BTreeSet::new();
    for id in &ids {
        let deps = app.session.execute("timeline.dependents", &json!({ "feature": id })).unwrap_or_default();
        for (key, output) in [("deleted_with_it", &mut with), ("would_fail", &mut fail)] {
            output
                .extend(deps.get(key).and_then(|value| value.as_array()).into_iter().flatten().filter_map(|value| value.as_str().map(String::from)));
        }
    }
    if with.is_empty() && fail.is_empty() {
        let _ = app.run("timeline.delete", json!({ "features": ids }));
    } else {
        app.dialog = Some(crate::dialogs::Dialog::confirm_delete_many(ids, with.into_iter().collect(), fail.into_iter().collect()));
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
    let groups = app.session.doc.timeline_groups.clone();
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        ui.label(RichText::new("FEATURE HISTORY").size(11.0).strong().color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let count = selected_feature_ids(app).len() + app.tree.picked_timeline_groups.len();
            if count > 0 {
                ui.menu_button(format!("{count} selected"), |ui| selection_actions(app, ui, &groups));
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

fn nearest_vertical_slot(slots: &[VerticalSlot], y: f32) -> Option<VerticalSlot> {
    slots.iter().min_by(|a, b| (a.y - y).abs().total_cmp(&(b.y - y).abs())).copied()
}

fn valid_folder_target(items: &DragItems, groups: &[solidcraft_engine::doc::TimelineGroup], target: u64) -> bool {
    !items.groups.iter().any(|source| *source == target || group_is_descendant(groups, target, *source))
}

/// Resolve the folder implied by a vertical drop. A folder row is always a target. Slots
/// between two children of the same folder are targets too; at a folder boundary the pointer
/// must be moved one indentation level to the right so an ordinary reorder remains possible.
fn vertical_drop_group(pointer: egui::Pos2, slot: VerticalSlot, folder_drop_zones: &[(Rect, u64)], origin_x: f32) -> Option<u64> {
    if let Some((_, group)) = folder_drop_zones.iter().find(|(rect, _)| rect.contains(pointer)) {
        return Some(*group);
    }
    if slot.before_group.is_some() && slot.before_group == slot.after_group {
        return slot.before_group;
    }
    let group = slot.before_group.or(slot.after_group)?;
    (pointer.x >= origin_x + 12.0).then_some(group)
}

fn vertical_marker(app: &mut SolveApp, ui: &mut egui::Ui, drag: &mut Option<Drag>) {
    let t = Tokens::get();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::drag());
    let line = Rect::from_min_max(pos2(rect.left() + 8.0, rect.center().y - 1.5), pos2(rect.right() - 5.0, rect.center().y + 1.5));
    let colour = t.accent;
    ui.painter().rect_filled(line, 1.5, colour);
    ui.painter().circle_filled(pos2(line.right(), line.center().y), 4.0, colour);
    crate::scenario::publish_handle("timeline:rollback", rect.center());
    if response.drag_started() {
        *drag = Some(Drag::Marker);
    }
    response.on_hover_text("Rollback bar: drag to roll the model back; new features are inserted here");
    let _ = app;
}

fn group_for(groups: &[solidcraft_engine::doc::TimelineGroup], feature: u64) -> Option<u64> {
    groups.iter().find(|group| group.features.contains(&feature)).map(|group| group.id)
}

fn selected_feature_ids(app: &SolveApp) -> Vec<u64> {
    app.session
        .doc
        .features
        .iter()
        .filter(|feature| {
            app.session.selection.iter().any(|selection| matches!(selection, solidcraft_engine::Sel::Feature { id } if *id == feature.id))
        })
        .map(|feature| feature.id)
        .collect()
}

fn action_feature_ids(app: &SolveApp, groups: &[solidcraft_engine::doc::TimelineGroup]) -> Vec<u64> {
    let mut picked: std::collections::BTreeSet<u64> = selected_feature_ids(app).into_iter().collect();
    for group in groups.iter().filter(|group| app.tree.picked_timeline_groups.contains(&group.id)) {
        picked.extend(group.features.iter().copied());
        for descendant in groups.iter().filter(|candidate| group_is_descendant(groups, candidate.id, group.id)) {
            picked.extend(descendant.features.iter().copied());
        }
    }
    app.session.doc.features.iter().filter(|feature| picked.contains(&feature.id)).map(|feature| feature.id).collect()
}

fn selected_drag_items(app: &SolveApp, groups: &[solidcraft_engine::doc::TimelineGroup], origin_x: f32) -> DragItems {
    DragItems {
        reorder: action_feature_ids(app, groups),
        features: selected_feature_ids(app),
        groups: app.tree.picked_timeline_groups.clone(),
        origin_x,
    }
}

fn select_only(app: &mut SolveApp, item: TimelineItem) {
    match item {
        TimelineItem::Group(id) => {
            app.tree.picked_timeline_groups.clear();
            app.tree.picked_timeline_groups.push(id);
            let _ = app.run("select.clear", json!({}));
        }
        TimelineItem::Feature(id) => {
            app.tree.picked_timeline_groups.clear();
            let _ = app.run("select.set", json!({"items": [{"type": "feature", "id": id}]}));
        }
    }
    app.tree.timeline_anchor = Some(item.key());
}

fn select_item(app: &mut SolveApp, ui: &egui::Ui, item: TimelineItem, order: &[TimelineItem]) {
    let modifiers = ui.input(|input| input.modifiers);
    if modifiers.shift
        && let Some(anchor) = app.tree.timeline_anchor.as_deref()
        && let (Some(from), Some(to)) = (order.iter().position(|entry| entry.key() == anchor), order.iter().position(|entry| *entry == item))
    {
        let (lo, hi) = (from.min(to), from.max(to));
        let range = &order[lo..=hi];
        let mut group_ids: Vec<u64> = range
            .iter()
            .filter_map(|entry| match entry {
                TimelineItem::Group(id) => Some(*id),
                TimelineItem::Feature(_) => None,
            })
            .collect();
        if modifiers.command || modifiers.ctrl {
            group_ids.extend(app.tree.picked_timeline_groups.iter().copied());
            group_ids.sort_unstable();
            group_ids.dedup();
        }
        app.tree.picked_timeline_groups = group_ids;
        let items: Vec<_> = range
            .iter()
            .filter_map(|entry| match entry {
                TimelineItem::Feature(id) => Some(json!({"type": "feature", "id": id})),
                TimelineItem::Group(_) => None,
            })
            .collect();
        if items.is_empty() && !(modifiers.command || modifiers.ctrl) {
            let _ = app.run("select.clear", json!({}));
        } else if !items.is_empty() {
            let _ = app.run("select.set", json!({"items": items, "add": modifiers.command || modifiers.ctrl}));
        }
        return;
    }

    app.tree.timeline_anchor = Some(item.key());
    let additive = modifiers.command || modifiers.ctrl;
    match item {
        TimelineItem::Group(id) => {
            if !additive {
                app.tree.picked_timeline_groups.clear();
                let _ = app.run("select.clear", json!({}));
            }
            if app.tree.picked_timeline_groups.contains(&id) {
                app.tree.picked_timeline_groups.retain(|group| *group != id);
            } else {
                app.tree.picked_timeline_groups.push(id);
            }
        }
        TimelineItem::Feature(id) => {
            if !additive {
                app.tree.picked_timeline_groups.clear();
                let _ = app.run("select.set", json!({"items": [{"type": "feature", "id": id}]}));
            } else {
                let selected = selected_feature_ids(app);
                if selected.contains(&id) {
                    let items: Vec<_> =
                        selected.into_iter().filter(|selected| *selected != id).map(|selected| json!({"type": "feature", "id": selected})).collect();
                    let _ = app.run("select.set", json!({"items": items}));
                } else {
                    let _ = app.run("select.set", json!({"items": [{"type": "feature", "id": id}], "add": true}));
                }
            }
        }
    }
}

fn selection_actions(app: &mut SolveApp, ui: &mut egui::Ui, groups: &[solidcraft_engine::doc::TimelineGroup]) {
    let features = selected_feature_ids(app);
    let selected_groups = app.tree.picked_timeline_groups.clone();
    let has_selection = !features.is_empty() || !selected_groups.is_empty();
    let directly_selected = selected_feature_ids(app);
    if has_selection {
        if ui.button("Add to New Folder").clicked() {
            let result = app.run("timeline.group", json!({"features": features}));
            if let Ok(result) = result
                && let Some(group) = result.get("group").and_then(serde_json::Value::as_u64)
            {
                app.tree.collapsed.remove(&format!("timeline_group:{group}"));
                if !selected_groups.is_empty() {
                    let _ = app.run("timeline.move_to_group", json!({"groups": selected_groups, "group": group}));
                }
            }
            ui.close();
        }
        let grouped = features.iter().any(|feature| group_for(groups, *feature).is_some()) || !selected_groups.is_empty();
        if grouped && ui.button("Remove from Folder").clicked() {
            let mut params = json!({"group": null});
            if !features.is_empty() {
                params["features"] = json!(features);
            }
            if !selected_groups.is_empty() {
                params["groups"] = json!(selected_groups);
            }
            let _ = app.run("timeline.move_to_group", params);
            ui.close();
        }
    }
    if !app.tree.picked_timeline_groups.is_empty() {
        if !features.is_empty() || !selected_groups.is_empty() {
            ui.separator();
        }
        if ui.button("Delete Selected Folders").on_hover_text("Keep their features and nested folders in the history").clicked() {
            let ids = app.tree.picked_timeline_groups.clone();
            let _ = app.run("timeline.ungroup", json!({"groups": ids}));
            app.tree.picked_timeline_groups.clear();
            ui.close();
        }
    }
    if !directly_selected.is_empty() {
        ui.separator();
        if ui.button("Delete Selected Features…").clicked() {
            delete_features(app, directly_selected);
            ui.close();
        }
    }
}

/// Delete selected timeline folders while preserving their features and nested folders.
pub fn delete_selected_folders(app: &mut SolveApp) -> bool {
    if app.tree.picked_timeline_groups.is_empty() {
        return false;
    }
    let groups = app.tree.picked_timeline_groups.clone();
    if app.run("timeline.ungroup", json!({"groups": groups})).is_ok() {
        app.tree.picked_timeline_groups.clear();
    }
    true
}

fn group_is_descendant(groups: &[solidcraft_engine::doc::TimelineGroup], group: u64, ancestor: u64) -> bool {
    let mut current = groups.iter().find(|candidate| candidate.id == group).and_then(|candidate| candidate.parent);
    let mut steps = 0usize;
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        steps += 1;
        if steps > groups.len() {
            return true;
        }
        current = groups.iter().find(|candidate| candidate.id == id).and_then(|candidate| candidate.parent);
    }
    false
}

fn group_depth(groups: &[solidcraft_engine::doc::TimelineGroup], id: u64) -> usize {
    let mut depth = 0;
    let mut current = groups.iter().find(|group| group.id == id).and_then(|group| group.parent);
    while let Some(parent) = current {
        depth += 1;
        if depth > groups.len() {
            break;
        }
        current = groups.iter().find(|group| group.id == parent).and_then(|group| group.parent);
    }
    depth
}

fn group_is_open(app: &SolveApp, groups: &[solidcraft_engine::doc::TimelineGroup], id: u64) -> bool {
    let mut current = Some(id);
    let mut steps = 0usize;
    while let Some(group_id) = current {
        if app.tree.collapsed.contains(&format!("timeline_group:{group_id}")) {
            return false;
        }
        steps += 1;
        if steps > groups.len() {
            return false;
        }
        current = groups.iter().find(|group| group.id == group_id).and_then(|group| group.parent);
    }
    true
}

fn group_ancestors_are_open(app: &SolveApp, groups: &[solidcraft_engine::doc::TimelineGroup], id: u64) -> bool {
    let mut current = groups.iter().find(|group| group.id == id).and_then(|group| group.parent);
    let mut steps = 0usize;
    while let Some(group_id) = current {
        if app.tree.collapsed.contains(&format!("timeline_group:{group_id}")) {
            return false;
        }
        steps += 1;
        if steps > groups.len() {
            return false;
        }
        current = groups.iter().find(|group| group.id == group_id).and_then(|group| group.parent);
    }
    true
}

fn group_first_feature(
    groups: &[solidcraft_engine::doc::TimelineGroup],
    features: &[(u64, String, &'static str, bool, &'static str, FeatureKind)],
    id: u64,
) -> Option<usize> {
    let group = groups.iter().find(|group| group.id == id)?;
    let direct = group.features.iter().filter_map(|feature| features.iter().position(|item| item.0 == *feature)).min();
    let children = groups.iter().filter(|child| child.parent == Some(id)).filter_map(|child| group_first_feature(groups, features, child.id)).min();
    direct.into_iter().chain(children).min()
}

fn visible_order(
    app: &SolveApp,
    features: &[(u64, String, &'static str, bool, &'static str, FeatureKind)],
    groups: &[solidcraft_engine::doc::TimelineGroup],
) -> Vec<TimelineItem> {
    let mut order = Vec::new();
    let mut shown = std::collections::BTreeSet::new();
    for (index, (id, ..)) in features.iter().enumerate() {
        let mut at_index: Vec<_> = groups.iter().filter(|group| group_first_feature(groups, features, group.id) == Some(index)).collect();
        at_index.sort_by_key(|group| (group_depth(groups, group.id), group.id));
        for group in at_index {
            if group_ancestors_are_open(app, groups, group.id) && shown.insert(group.id) {
                order.push(TimelineItem::Group(group.id));
            }
        }
        let group_id = group_for(groups, *id);
        let open = group_id.is_none_or(|group_id| group_is_open(app, groups, group_id));
        if open {
            order.push(TimelineItem::Feature(*id));
        }
    }
    let mut trailing: Vec<_> = groups.iter().filter(|group| !shown.contains(&group.id)).collect();
    trailing.sort_by_key(|group| (group_first_feature(groups, features, group.id).unwrap_or(usize::MAX), group_depth(groups, group.id), group.id));
    for group in trailing {
        if group_ancestors_are_open(app, groups, group.id) && shown.insert(group.id) {
            order.push(TimelineItem::Group(group.id));
        }
    }
    order
}

fn vertical_group_row(
    app: &mut SolveApp,
    ui: &mut egui::Ui,
    group: &solidcraft_engine::doc::TimelineGroup,
    groups: &[solidcraft_engine::doc::TimelineGroup],
    order: &[TimelineItem],
    drag: &mut Option<Drag>,
    depth: usize,
    drop_zones: &mut Vec<(Rect, u64)>,
) -> bool {
    let t = Tokens::get();
    let key = format!("timeline_group:{}", group.id);
    let open = !app.tree.collapsed.contains(&key);
    let selected = app.tree.picked_timeline_groups.contains(&group.id);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 23.0), Sense::click_and_drag());
    drop_zones.push((rect, group.id));
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 3.0, if selected { t.accent_soft } else { t.hover });
    }
    let offset = depth as f32 * 14.0;
    let centre = pos2(rect.left() + 9.0 + offset, rect.center().y);
    let arrow = if open {
        vec![centre + vec2(-4.0, -2.0), centre + vec2(4.0, -2.0), centre + vec2(0.0, 3.0)]
    } else {
        vec![centre + vec2(-2.0, -4.0), centre + vec2(-2.0, 4.0), centre + vec2(3.0, 0.0)]
    };
    ui.painter().add(egui::Shape::convex_polygon(arrow, t.text_dim, Stroke::NONE));
    let icon = Rect::from_center_size(pos2(rect.left() + 27.5 + offset, rect.center().y), vec2(17.0, 17.0));
    icons::paint(ui.painter(), icon, "folder", t.icon, t.icon_fill, t.accent);
    ui.painter().text(
        pos2(rect.left() + 40.0 + offset, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &group.name,
        egui::FontId::proportional(12.0),
        t.text,
    );
    crate::scenario::publish_handle(&format!("timeline-folder:{}", group.name), rect.center());
    let fold_rect = Rect::from_min_max(pos2(rect.left() + offset, rect.top()), pos2(rect.left() + offset + 18.0, rect.bottom()));
    crate::scenario::publish_handle(&format!("timeline-folder-fold:{}", group.name), fold_rect.center());
    let fold = ui.interact(fold_rect, ui.id().with(("timeline_group_fold", group.id)), Sense::click());
    if fold.clicked() {
        if open {
            app.tree.collapsed.insert(key.clone());
        } else {
            app.tree.collapsed.remove(&key);
        }
    } else if response.clicked() {
        select_item(app, ui, TimelineItem::Group(group.id), order);
    }
    if response.drag_started() {
        if !app.tree.picked_timeline_groups.contains(&group.id) {
            select_only(app, TimelineItem::Group(group.id));
        }
        let origin_x = response.interact_pointer_pos().map_or(rect.center().x, |pointer| pointer.x);
        *drag = Some(Drag::Items(selected_drag_items(app, groups, origin_x)));
    }
    if response.double_clicked() {
        crate::context_menu::start_rename(app, crate::context_menu::RenameWhat::TimelineGroup(group.id), rect.center());
    }
    if response.secondary_clicked() && !selected {
        select_only(app, TimelineItem::Group(group.id));
    }
    response.context_menu(|ui| {
        if ui.button("Rename Folder").clicked() {
            crate::context_menu::start_rename(app, crate::context_menu::RenameWhat::TimelineGroup(group.id), rect.center());
            ui.close();
        }
        ui.separator();
        selection_actions(app, ui, groups);
    });
    open
}

fn vertical_features(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    ui.spacing_mut().item_spacing.y = 0.0;
    let features: Vec<_> = app
        .session
        .doc
        .features
        .iter()
        .map(|feature| (feature.id, feature.name.clone(), icon_of(&feature.kind), feature.suppressed, feature.kind.type_name(), feature.kind.clone()))
        .collect();
    let groups = app.session.doc.timeline_groups.clone();
    let order = visible_order(app, &features, &groups);
    let first_indices: std::collections::BTreeMap<u64, Option<usize>> =
        groups.iter().map(|group| (group.id, group_first_feature(&groups, &features, group.id))).collect();
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
    let mut folder_drop_zones = Vec::new();
    let mut shown_groups = std::collections::BTreeSet::new();
    let mut previous_visible_group = None;
    let mut hovered = None;
    let mut edit = None;

    for (index, (id, name, icon, suppressed, ty, kind)) in features.iter().enumerate() {
        let group_id = group_for(&groups, *id);
        let mut at_index: Vec<_> = groups.iter().filter(|group| first_indices.get(&group.id).copied().flatten() == Some(index)).collect();
        at_index.sort_by_key(|group| (group_depth(&groups, group.id), group.id));
        for group in at_index {
            if group_ancestors_are_open(app, &groups, group.id) && shown_groups.insert(group.id) {
                vertical_group_row(app, ui, group, &groups, &order, &mut drag, group_depth(&groups, group.id), &mut folder_drop_zones);
            }
        }
        let group_open = group_id.is_none_or(|id| group_is_open(app, &groups, id));
        if index == marker {
            slots.push(VerticalSlot {
                y: ui.cursor().top(),
                position: index,
                before_group: previous_visible_group,
                after_group: group_open.then_some(group_id).flatten(),
            });
            vertical_marker(app, ui, &mut drag);
        }
        if !group_open {
            continue;
        }
        slots.push(VerticalSlot { y: ui.cursor().top(), position: index, before_group: previous_visible_group, after_group: group_id });
        let indent = group_id.map(|id| (group_depth(&groups, id) + 1) as f32 * 14.0).unwrap_or(0.0);
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click_and_drag());
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
        let icon_rect = Rect::from_center_size(pos2(rect.left() + 13.0 + indent, rect.center().y), vec2(18.0, 18.0));
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
            if !selected {
                select_only(app, TimelineItem::Feature(*id));
            }
            let origin_x = response.interact_pointer_pos().map_or(rect.center().x, |pointer| pointer.x);
            drag = Some(Drag::Items(selected_drag_items(app, &groups, origin_x)));
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
            select_item(app, ui, TimelineItem::Feature(*id), &order);
        }
        if response.double_clicked() {
            edit = Some(*id);
        }
        let sketch = match kind {
            FeatureKind::Extrude { sketch, .. } | FeatureKind::Revolve { sketch, .. } | FeatureKind::Sweep { sketch, .. } => Some(*sketch),
            _ => None,
        };
        if response.secondary_clicked() && !selected {
            select_only(app, TimelineItem::Feature(*id));
        }
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
            selection_actions(app, ui, &groups);
            ui.separator();
            if ui.button("Delete").clicked() {
                delete_feature(app, *id);
                ui.close();
            }
        });
        previous_visible_group = group_id;
    }
    let mut trailing: Vec<_> = groups.iter().filter(|group| !shown_groups.contains(&group.id)).collect();
    trailing.sort_by_key(|group| (first_indices.get(&group.id).copied().flatten().unwrap_or(usize::MAX), group_depth(&groups, group.id), group.id));
    for group in trailing {
        if group_ancestors_are_open(app, &groups, group.id) && shown_groups.insert(group.id) {
            vertical_group_row(app, ui, group, &groups, &order, &mut drag, group_depth(&groups, group.id), &mut folder_drop_zones);
        }
    }
    slots.push(VerticalSlot { y: ui.cursor().top(), position: n, before_group: previous_visible_group, after_group: None });
    if marker >= n {
        vertical_marker(app, ui, &mut drag);
    }
    crate::dialogs_assembly::timeline_joints_vertical(app, ui);

    if let (Some(active), Some(pointer)) = (drag.as_ref(), pointer) {
        if let Some(slot) = nearest_vertical_slot(&slots, pointer.y) {
            let folder_target = match active {
                Drag::Items(items) => vertical_drop_group(pointer, slot, &folder_drop_zones, items.origin_x)
                    .filter(|target| valid_folder_target(items, &groups, *target)),
                Drag::Marker | Drag::Feature(_) => None,
            };
            let hovered_folder = folder_target
                .and_then(|target| folder_drop_zones.iter().find(|(rect, group)| *group == target && rect.contains(pointer)).map(|(rect, _)| *rect));
            if let Some(rect) = hovered_folder {
                ui.painter().rect_stroke(rect.shrink(1.0), 3.0, Stroke::new(2.0, t.sketch_accent), egui::StrokeKind::Inside);
            } else {
                let left = folder_target
                    .map(|group| ui.min_rect().left() + 12.0 + (group_depth(&groups, group) + 1) as f32 * 14.0)
                    .unwrap_or(ui.min_rect().left() + 4.0);
                ui.painter().line_segment(
                    [pos2(left, slot.y), pos2(ui.max_rect().right() - 5.0, slot.y)],
                    Stroke::new(2.0, if matches!(active, Drag::Marker) { t.accent } else { t.sketch_accent }),
                );
            }
        }
    }
    if ui.input(|input| input.pointer.any_released())
        && let Some(active) = drag.take()
        && let Some(pointer) = pointer
    {
        if let Some(slot) = nearest_vertical_slot(&slots, pointer.y) {
            match active {
                Drag::Marker => {
                    let _ = app.run("timeline.roll_to", if slot.position >= n { json!({}) } else { json!({"position": slot.position}) });
                }
                Drag::Items(items) => {
                    let folder_target = vertical_drop_group(pointer, slot, &folder_drop_zones, items.origin_x)
                        .filter(|target| valid_folder_target(&items, &groups, *target));
                    if let Some(group) = folder_target {
                        let mut params = json!({"group": group});
                        if !items.features.is_empty() {
                            params["features"] = json!(items.features);
                        }
                        if !items.groups.is_empty() {
                            params["groups"] = json!(items.groups);
                        }
                        let _ = app.run("timeline.move_to_group", params);
                        app.tree.collapsed.remove(&format!("timeline_group:{group}"));
                    }
                    if !items.reorder.is_empty() {
                        let _ = app.run("timeline.reorder_many", json!({"features": items.reorder, "position": slot.position}));
                    }
                }
                Drag::Feature(id) => {
                    let _ = app.run("timeline.reorder_many", json!({"features": [id], "position": slot.position}));
                }
            }
        }
    }
    ui.data_mut(|data| match drag {
        Some(active) => {
            data.insert_temp(drag_id, active.clone());
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
                if let (Some(dg), Some(pp)) = (drag.as_ref(), pointer) {
                    let slot = slot_at(&xs, end, pp.x);
                    let sx = xs.get(slot).copied().unwrap_or(end) - 4.0;
                    p.line_segment(
                        [pos2(sx, r.top() + 3.0), pos2(sx, r.bottom() - 3.0)],
                        Stroke::new(2.0, if matches!(dg, Drag::Marker) { t.accent } else { t.sketch_accent }),
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
                        Drag::Items(items) => {
                            let _ = app.run("timeline.reorder_many", json!({"features": items.reorder, "position": slot}));
                        }
                    }
                }
                ui.data_mut(|d| match drag {
                    Some(dg) => {
                        d.insert_temp(drag_id, dg.clone());
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

    #[test]
    fn rollback_bar_above_last_feature_can_be_dragged_to_the_end() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        harness.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "name": "First"})).unwrap();
        harness
            .app
            .run("solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [20, 0, 0], "operation": "new_body", "name": "Last"}))
            .unwrap();
        harness.app.run("timeline.roll_to", json!({"position": 1})).unwrap();
        harness.frames(4);

        let last = handle_at("timeline:Last").unwrap();
        harness.step(&json!({"drag": [{"handle": "timeline:rollback"}, [last.x, last.y + 15.0]]})).unwrap();
        assert_eq!(harness.app.session.doc.marker, None, "dropping below the final row must restore the fully evaluated history");
    }

    #[test]
    fn feature_folder_selection_can_move_as_a_block() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        for (name, body, x) in [("First", "FirstBody", 0), ("Second", "SecondBody", 20), ("Third", "ThirdBody", 40)] {
            harness
                .app
                .run(
                    "solid.box",
                    json!({"length": 10, "width": 10, "height": 10, "corner": [x, 0, 0], "operation": "new_body", "name": name, "body_name": body}),
                )
                .unwrap();
        }
        harness.app.run("timeline.group", json!({"name": "Pair", "features": ["First", "Second"]})).unwrap();
        harness.frames(4);

        let third = handle_at("timeline:Third").unwrap();
        harness.step(&json!({"drag": [{"handle": "timeline-folder:Pair"}, [third.x, third.y + 15.0]]})).unwrap();
        let names: Vec<_> = harness.app.session.doc.features.iter().map(|feature| feature.name.as_str()).collect();
        assert_eq!(names, vec!["Third", "First", "Second"]);
    }

    #[test]
    fn selected_folders_and_features_can_be_dragged_into_a_folder() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        harness.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "name": "Grouped"})).unwrap();
        harness
            .app
            .run("solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [20, 0, 0], "operation": "new_body", "name": "Loose"}))
            .unwrap();
        let group = harness.app.run("timeline.group", json!({"name": "Folder", "features": ["Grouped"]})).unwrap()["group"].as_u64().unwrap();
        let destination = harness.app.run("timeline.group", json!({"name": "Destination"})).unwrap()["group"].as_u64().unwrap();
        harness.frames(4);

        harness.step(&json!({"click": {"handle": "timeline-folder:Folder"}})).unwrap();
        harness.step(&json!({"click": {"handle": "timeline:Loose"}, "ctrl": true})).unwrap();
        assert_eq!(harness.app.tree.picked_timeline_groups, vec![group]);
        assert!(harness
            .app
            .session
            .selection
            .iter()
            .any(|selection| matches!(selection, solidcraft_engine::Sel::Feature { id } if harness.app.session.doc.feature(*id).is_some_and(|feature| feature.name == "Loose"))));
        harness.step(&json!({"drag": [{"handle": "timeline:Loose"}, {"handle": "timeline-folder:Destination"}]})).unwrap();
        let source = harness.app.session.doc.timeline_groups.iter().find(|entry| entry.id == group).unwrap();
        let target = harness.app.session.doc.timeline_groups.iter().find(|entry| entry.id == destination).unwrap();
        let loose = harness.app.session.doc.find_feature("Loose").unwrap().id;
        assert_eq!(source.parent, Some(destination), "the selected folder should nest in the drop target");
        assert!(target.features.contains(&loose), "the selected feature should move into the drop target");
    }

    #[test]
    fn vertical_folder_drop_targets_match_the_three_visual_drop_modes() {
        let inside = VerticalSlot { y: 40.0, position: 1, before_group: Some(7), after_group: Some(7) };
        let at_end = VerticalSlot { y: 60.0, position: 2, before_group: Some(7), after_group: None };
        let folder_rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 20.0));

        assert_eq!(vertical_drop_group(pos2(80.0, 10.0), at_end, &[(folder_rect, 7)], 80.0), Some(7), "dropping over a folder row");
        assert_eq!(vertical_drop_group(pos2(80.0, 40.0), inside, &[], 80.0), Some(7), "dropping between expanded folder items");
        assert_eq!(vertical_drop_group(pos2(91.0, 60.0), at_end, &[], 80.0), None, "small horizontal movement remains a plain reorder");
        assert_eq!(vertical_drop_group(pos2(92.0, 60.0), at_end, &[], 80.0), Some(7), "moving right at the folder end nests the item");
    }

    #[test]
    fn selected_count_does_not_move_the_history_rows() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        harness.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "name": "Fixed"})).unwrap();
        harness.frames(4);
        let before = handle_at("timeline:Fixed").unwrap();

        harness.step(&json!({"click": {"handle": "timeline:Fixed"}})).unwrap();
        let after = handle_at("timeline:Fixed").unwrap();
        assert!((after.y - before.y).abs() < 0.5, "selection count shifted the row from {} to {}", before.y, after.y);
    }

    #[test]
    fn nested_folder_delete_keeps_parent_and_features() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        harness.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "name": "Feature"})).unwrap();
        let root = harness.app.run("timeline.group", json!({"name": "Root"})).unwrap()["group"].as_u64().unwrap();
        let child =
            harness.app.run("timeline.group", json!({"name": "Child", "parent": root, "features": ["Feature"]})).unwrap()["group"].as_u64().unwrap();
        harness.frames(4);
        harness.step(&json!({"click": {"handle": "timeline-folder:Child"}})).unwrap();
        harness.step(&json!({"key": "Delete"})).unwrap();
        assert!(harness.app.session.doc.timeline_groups.iter().all(|group| group.id != child));
        assert!(harness.app.session.doc.timeline_groups.iter().any(|group| group.id == root));
        assert!(harness.app.session.doc.find_feature("Feature").is_some());
    }

    #[test]
    fn collapsed_folder_stays_visible_while_its_contents_hide() {
        let mut harness = Harness::new();
        harness.app.preferences.timeline_left = true;
        harness.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "name": "Inside"})).unwrap();
        harness.app.run("timeline.group", json!({"name": "Folder", "features": ["Inside"]})).unwrap();
        harness.frames(4);
        assert!(handle_at("timeline:Inside").is_some());

        harness.step(&json!({"click": {"handle": "timeline-folder-fold:Folder"}})).unwrap();
        assert!(handle_at("timeline-folder:Folder").is_some(), "collapsed folder row disappeared");
        assert!(handle_at("timeline:Inside").is_none(), "collapsed folder still showed its contents");
    }
}
