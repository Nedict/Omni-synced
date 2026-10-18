//! Undo/redo, the timeline, deletion and selection.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solidcraft_doc::{Feature, FeatureKind};

use super::CommandSpec;
use crate::params::{bad, bool_, num, str_, string_list};
use crate::{EngineError, Result, Sel, Session, Snapshot};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("edit.undo", "Undo", undo).icon("undo").key("Ctrl+Z").noundo().doc_only(),
    CommandSpec::new("edit.redo", "Redo", redo).icon("redo").key("Ctrl+Y").noundo().doc_only(),
    CommandSpec::new("edit.checkpoint", "Checkpoint", checkpoint)
        .noundo()
        .params("→ {checkpoint: id}: remember the design, active sketch and undo history (restore with edit.restore_checkpoint)"),
    CommandSpec::new("edit.restore_checkpoint", "Restore Checkpoint", restore_checkpoint)
        .noundo()
        .params("checkpoint: id; forget?: bool (true = drop it without going back). Goes back to it, undo history included"),
    CommandSpec::new("timeline.compute_all", "Compute All", compute_all).at("SOLID", "MODIFY").icon("compute").noundo(),
    CommandSpec::new("timeline.delete", "Delete", delete)
        .at("SOLID", "MODIFY")
        .icon("delete")
        .key("Delete")
        .params("features: [id or name] (a sketch takes its dependent features with it) → deleted: [ids]; failing?: [{feature, error}] (later features that worked and now fail)"),
    CommandSpec::new("timeline.rename", "Rename", rename).params("feature: id|name, name (references to it follow)"),
    CommandSpec::new("timeline.rollback", "Move Timeline Marker", rollback).params("position: number of features to keep active (omit = end)"),
    CommandSpec::new("timeline.roll_to", "Roll History Marker", rollback)
        .params("position: number of features to keep active (omit = end) | feature: id|name (the marker goes right after it)"),
    CommandSpec::new("timeline.reorder", "Reorder Feature", reorder)
        .params("feature: id|name, position: its new index; refused when a feature that worked would fail"),
    CommandSpec::new("timeline.redefine", "Edit Feature", redefine)
        .params("feature: id|name, command: the feature's command id, params: its parameters (the feature is rebuilt in place)"),
    CommandSpec::new("timeline.dependents", "Feature Dependents", dependents)
        .noundo()
        .params("feature: id|name → features deleted with it, and features that would fail without it"),
    CommandSpec::new("timeline.suppress", "Suppress Feature", suppress).params("feature: id|name, suppressed?: bool (default toggles)"),
    CommandSpec::new("timeline.edit", "Edit Feature", edit_feature).params(
        "feature: id|name, set: {fields to change, merged into the feature's definition as timeline.inspect returns it, \
         e.g. {\"extent\": {\"distance\": \"30\"}} (extrude), {\"angle\": \"90 deg\"} (revolve); \
         expressions are text} → recomputed: features rebuilt, restored: features whose earlier result for the same inputs was reused",
    ),
    CommandSpec::new("timeline.inspect", "Inspect Feature", inspect_feature).noundo().params(
        "feature: id|name → type, definition (the fields timeline.edit's set changes, with their current values), \
         inputs: [{label, param, expression, unit}] (named inputs parameters.change can also set), error, warning",
    ),
    CommandSpec::new("select.set", "Select", select_set)
        .noundo()
        .params("items: [{type: body|edge|face|feature|sketch_curve|sketch_point|profile, …}], add?: bool"),
    CommandSpec::new("select.clear", "Clear Selection", select_clear).noundo(),
    CommandSpec::new("select.loop", "Select Loop", select_loop).noundo().params("edge?: index on the first body. The face loop through that edge"),
    CommandSpec::new("select.ring", "Select Ring", select_ring).noundo().params("edge?: index. Edges parallel to that edge"),
    CommandSpec::new("select.grow", "Grow Selection", select_grow).noundo().params("adds edges that share an end with the selection"),
    CommandSpec::new("select.shrink", "Shrink Selection", select_shrink).noundo(),
    CommandSpec::new("select.invert", "Invert Selection", select_invert).noundo().params("edges of the first body that are not selected"),
];

fn undo(s: &mut Session, _p: &Value) -> Result<Value> {
    let Some(prev) = s.undo.pop() else { return Err(EngineError::Other("nothing to undo".into())) };
    let now = go_to(s, prev);
    let label = now.label.clone();
    s.redo.push(now);
    Ok(json!({"undone": label}))
}

fn redo(s: &mut Session, _p: &Value) -> Result<Value> {
    let Some(next) = s.redo.pop() else { return Err(EngineError::Other("nothing to redo".into())) };
    let now = go_to(s, next);
    let label = now.label.clone();
    s.undo.push(now);
    Ok(json!({"redone": label}))
}

/// Put the session at `snap`; returns where it was, under the same label.
fn go_to(s: &mut Session, snap: Snapshot) -> Snapshot {
    let was = Snapshot { label: snap.label, doc: s.doc.clone(), active_sketch: s.active_sketch, pending_moves: s.pending_moves.clone() };
    let same = std::sync::Arc::ptr_eq(&s.doc, &snap.doc);
    s.doc = snap.doc;
    s.active_sketch = snap.active_sketch;
    s.pending_moves = snap.pending_moves;
    s.selection.clear();
    // A new document is evaluated by `execute`.
    if same {
        s.refresh();
    }
    was
}

fn checkpoint(s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({"checkpoint": s.checkpoint()}))
}

fn restore_checkpoint(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "edit.restore_checkpoint";
    let id = p.get("checkpoint").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "`checkpoint` must be an id from edit.checkpoint"))?;
    if bool_(p, "forget") == Some(true) {
        s.drop_checkpoint(id);
        return Ok(json!({"checkpoint": id, "restored": false}));
    }
    s.restore_checkpoint(id)?;
    Ok(json!({"checkpoint": id, "restored": true}))
}

fn compute_all(s: &mut Session, _p: &Value) -> Result<Value> {
    s.model = solidcraft_doc::Model::new();
    s.refresh();
    let errors = s.model.results.iter().filter(|r| r.error.is_some()).count();
    Ok(json!({"features": s.model.results.len(), "errors": errors}))
}

fn feature_id(s: &Session, v: Option<&Value>, cmd: &str) -> Result<u64> {
    let key = match v {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(x)) => x.clone(),
        _ => return Err(bad(cmd, "`feature` must be an id or name")),
    };
    s.doc.find_feature(&key).map(|f| f.id).ok_or_else(|| bad(cmd, format!("no feature `{key}`")))
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.delete";
    let mut keys = string_list(p, "features");
    if let Some(Value::Array(a)) = p.get("features") {
        keys.extend(a.iter().filter_map(Value::as_u64).map(|n| n.to_string()));
    }
    if let Some(f) = p.get("feature") {
        keys.push(match f {
            Value::Number(n) => n.to_string(),
            Value::String(x) => x.clone(),
            _ => String::new(),
        });
    }
    if keys.is_empty() {
        return Err(bad(cmd, "`features` must list features"));
    }
    // Features that work now; any of them failing after the delete is reported (a later
    // feature that names a deleted body, #267).
    let worked: std::collections::BTreeSet<u64> = s.model.results.iter().filter(|r| r.error.is_none() && !r.skipped).map(|r| r.id).collect();
    let mut gone = Vec::new();
    for k in keys {
        let Some(id) = s.doc.find_feature(&k).map(|f| f.id) else {
            if gone.is_empty() {
                return Err(bad(cmd, format!("no feature `{k}`")));
            }
            continue;
        };
        gone.extend(s.doc_mut().delete_feature(id)?);
    }
    if s.active_sketch.is_some_and(|a| gone.contains(&a)) {
        s.active_sketch = None;
    }
    s.selection.clear();
    s.refresh();
    let failing: Vec<Value> = s
        .model
        .results
        .iter()
        .filter(|r| worked.contains(&r.id))
        .filter_map(|r| r.error.as_ref().map(|e| json!({"feature": r.name, "error": e})))
        .collect();
    if failing.is_empty() {
        return Ok(json!({"deleted": gone}));
    }
    Ok(json!({"deleted": gone, "failing": failing}))
}

fn rename(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.rename";
    let id = feature_id(s, p.get("feature"), cmd)?;
    let name =
        str_(p, "name").map(str::trim).filter(|n| !n.is_empty() && n.len() <= 128).ok_or_else(|| bad(cmd, "`name` must be 1…128 characters"))?;
    let old = s.doc.feature(id).map(|f| f.name.clone()).ok_or_else(|| bad(cmd, "feature"))?;
    if s.doc.features.iter().any(|f| f.name == name && f.id != id) {
        return Err(bad(cmd, format!("another feature is called `{name}`")));
    }
    if s.doc.feature(id).is_some_and(|f| matches!(f.kind, FeatureKind::ConstructionPlane { .. })) {
        super::features::check_plane_name(name, cmd)?;
    }
    if name == old {
        return Ok(json!({"feature": id, "name": name}));
    }
    let names = BTreeMap::from([(old.clone(), name.to_string())]);
    let sketches = BTreeMap::new();
    let doc = s.doc_mut();
    if let Some(f) = doc.feature_mut(id) {
        f.name = name.to_string();
    }
    for f in doc.features.iter_mut() {
        f.kind.remap_refs(&sketches, &names);
    }
    Ok(json!({"feature": id, "name": name}))
}

fn rollback(s: &mut Session, p: &Value) -> Result<Value> {
    let n = s.doc.features.len();
    let pos = match p.get("feature") {
        Some(f) => {
            let id = feature_id(s, Some(f), "timeline.roll_to")?;
            s.doc.feature_index(id).map(|i| i + 1)
        }
        None => num(p, "position").map(|x| x.max(0.0) as usize),
    };
    s.doc_mut().marker = match pos {
        Some(k) if k < n => Some(k),
        _ => None,
    };
    s.selection.clear();
    Ok(json!({"marker": s.doc.marker.unwrap_or(n), "features": n}))
}

fn suppress(s: &mut Session, p: &Value) -> Result<Value> {
    let id = feature_id(s, p.get("feature"), "timeline.suppress")?;
    let want = bool_(p, "suppressed");
    let f = s.doc_mut().feature_mut(id).ok_or_else(|| EngineError::Other("feature".into()))?;
    f.suppressed = want.unwrap_or(!f.suppressed);
    Ok(json!({"feature": id, "suppressed": f.suppressed}))
}

fn merge(dst: &mut Value, src: &Value, depth: usize) {
    if depth > 16 {
        return;
    }
    match (dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s {
                match d.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge(slot, v, depth + 1),
                    _ => {
                        d.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (d, s) => *d = s.clone(),
    }
}

/// `profiles` (also a loft section's) given as the feature commands take it (`"all"`,
/// `[index]`, `[{curves}]`, `[{point}]`...) becomes the stored form; the stored form passes.
fn command_profiles(set: &Value, cmd: &str) -> Result<Value> {
    let conv = |v: &mut Value| -> Result<()> {
        if let Some(pv) = v.get("profiles").filter(|x| x.get("type").is_none()) {
            let sel = super::features::profiles(&json!({"profiles": pv}), cmd)?;
            let sv = serde_json::to_value(sel).map_err(|e| EngineError::Other(e.to_string()))?;
            if let Some(o) = v.as_object_mut() {
                o.insert("profiles".into(), sv);
            }
        }
        Ok(())
    };
    let mut set = set.clone();
    conv(&mut set)?;
    if let Some(secs) = set.get_mut("sections").and_then(Value::as_array_mut) {
        for sec in secs.iter_mut().take(10_000) {
            conv(sec)?;
        }
    }
    Ok(set)
}

/// Fields of a stored feature left out of its definition: bookkeeping timeline.edit recomputes, and
/// the name and suppression, which timeline.rename and timeline.suppress change.
const NOT_EDITABLE: &[&str] = &["id", "type", "name", "suppressed", "param_names", "edge_refs", "edge_names", "face_names"];

fn inspect_feature(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.inspect";
    let id = feature_id(s, p.get("feature"), cmd)?;
    let f = s.doc.feature(id).ok_or_else(|| bad(cmd, "feature"))?;
    let mut def = serde_json::to_value(f).map_err(|e| EngineError::Other(e.to_string()))?;
    let mut omitted = Vec::new();
    if let Some(o) = def.as_object_mut() {
        for k in NOT_EDITABLE {
            o.remove(*k);
        }
        // Large payloads that timeline.edit cannot change.
        let heavy: &[(&str, &str)] = match &f.kind {
            FeatureKind::Sketch { .. } => &[("sketch", "the sketch geometry (sketch.inspect; edit it with the sketch commands)")],
            FeatureKind::Import { .. } => &[("step", "the imported STEP text"), ("components", "the imported product tree")],
            FeatureKind::MeshImport { .. } => &[("meshes", "the imported meshes")],
            _ => &[],
        };
        for (k, why) in heavy {
            if o.remove(*k).is_some() {
                omitted.push(json!({"field": k, "what": why}));
            }
        }
    }
    let inputs: Vec<Value> = f
        .kind
        .inputs()
        .into_iter()
        .enumerate()
        .map(|(i, (label, e, k))| {
            json!({"label": label, "param": f.param_names.get(i).filter(|n| !n.is_empty()), "expression": e, "unit": k.default_unit()})
        })
        .collect();
    let marker = s.doc.marker.unwrap_or(usize::MAX);
    let rolled_back = s.doc.features.iter().position(|x| x.id == id).is_some_and(|i| i >= marker);
    let r = s.model.result(id);
    Ok(json!({
        "id": f.id, "name": f.name, "type": f.kind.type_name(), "suppressed": f.suppressed, "rolled_back": rolled_back,
        "definition": def, "omitted": omitted, "inputs": inputs,
        "error": r.and_then(|r| r.error.clone()), "warning": r.and_then(|r| r.warning.clone()),
        "boolean_diagnostics": r.map(|r| &r.boolean_diagnostics),
    }))
}

fn edit_feature(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.edit";
    let id = feature_id(s, p.get("feature"), cmd)?;
    let set = p.get("set").filter(|v| v.is_object()).ok_or_else(|| bad(cmd, "`set` must be an object of fields"))?;
    let f = s.doc.feature(id).cloned().ok_or_else(|| bad(cmd, "feature"))?;
    let mut v = serde_json::to_value(&f).map_err(|e| EngineError::Other(e.to_string()))?;
    let set = &command_profiles(set, cmd)?;
    merge(&mut v, set, 0);
    let mut nf: Feature = serde_json::from_value(v).map_err(|e| bad(cmd, format!("invalid feature: {e}")))?;
    nf.id = f.id;
    if std::mem::discriminant(&nf.kind) != std::mem::discriminant(&f.kind) {
        return Err(bad(cmd, "the feature type cannot change"));
    }
    if let FeatureKind::BodyTransfer { bodies, from, to, transform } = &nf.kind {
        solidcraft_doc::validate_body_transfer(&s.doc, bodies, *from, *to, transform)?;
        nf.component = *to;
    }
    if let (FeatureKind::Sketch { sketch: a, .. }, FeatureKind::Sketch { sketch: b, .. }) = (&nf.kind, &f.kind)
        && a != b
    {
        return Err(bad(cmd, "edit sketch geometry with the sketch commands"));
    }
    // New edges for a blend: remember where they sit now (in the model before the feature).
    if let (
        FeatureKind::Fillet { edges: a, body, .. } | FeatureKind::Chamfer { edges: a, body, .. },
        FeatureKind::Fillet { edges: b, .. } | FeatureKind::Chamfer { edges: b, .. },
    ) = (&nf.kind, &f.kind)
        && a != b
    {
        let before = s.model.state_before(id);
        nf.edge_refs = solidcraft_doc::edge_refs(&before, body, a);
        nf.edge_names = solidcraft_doc::edge_names_for(&before, body, a);
    }
    // New or reordered flange edges: their names now, so each edge's own angle and height stay
    // with the edge they were given for.
    if let (FeatureKind::SheetFlange { edges: a, .. }, FeatureKind::SheetFlange { edges: b, .. }) = (&nf.kind, &f.kind) {
        let points = |e: &[solidcraft_doc::FlangeEdge]| e.iter().map(|x| x.point).collect::<Vec<_>>();
        if points(a) != points(b) {
            nf.edge_names = solidcraft_doc::sheet::sheet_edge_names(&s.model.state_before(id), &points(a));
        }
    }
    // Profiles picked by index or point are kept by their curves, as when the feature was made.
    if nf.kind.clone().profile_sels_mut() != f.kind.clone().profile_sels_mut() {
        super::features::stable_profiles(s, &mut nf.kind)?;
    }
    // New face points: their faces' names now.
    if nf.kind.face_points() != f.kind.face_points() {
        nf.face_names = solidcraft_doc::naming::face_names_at(&s.model.state_before(id), &nf.kind.face_points());
    }
    if let Some(slot) = s.doc_mut().feature_mut(id) {
        *slot = nf;
    }
    s.refresh();
    if let Some(e) = s.model.result(id).and_then(|r| r.error.clone()) {
        return Err(EngineError::Other(e));
    }
    Ok(json!({"feature": id, "recomputed": s.model.last_recomputed, "restored": s.model.last_restored}))
}

fn select_set(s: &mut Session, p: &Value) -> Result<Value> {
    let items = p.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    if items.len() > 10_000 {
        return Err(bad("select.set", "too many items"));
    }
    let mut sel: Vec<Sel> = Vec::new();
    for it in items {
        let one: Sel = serde_json::from_value(it).map_err(|e| bad("select.set", e.to_string()))?;
        if let Sel::Occurrence { path } = &one {
            s.occurrence_bodies(path, true).map_err(|e| bad("select.set", e.to_string()))?;
        }
        sel.push(one);
    }
    if !bool_(p, "add").unwrap_or(false) {
        s.selection.clear();
    }
    for x in sel {
        if !s.selection.contains(&x) {
            s.selection.push(x);
        }
    }
    s.revision += 1;
    Ok(json!({"selected": s.selection.len()}))
}

fn select_clear(s: &mut Session, _p: &Value) -> Result<Value> {
    s.selection.clear();
    s.revision += 1;
    Ok(json!({"selected": 0}))
}

/// Features that worked in `before` and fail in `after`: (name, error).
fn newly_broken(before: &solidcraft_doc::Model, after: &solidcraft_doc::Model) -> Vec<(String, String)> {
    after
        .results
        .iter()
        .filter_map(|r| {
            let e = r.error.clone()?;
            let was_ok = before.result(r.id).is_some_and(|b| b.error.is_none() && !b.skipped);
            was_ok.then(|| (r.name.clone(), e))
        })
        .collect()
}

fn reorder(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.reorder";
    let id = feature_id(s, p.get("feature"), cmd)?;
    let n = s.doc.features.len();
    let to = num(p, "position").map(|x| x.max(0.0) as usize).ok_or_else(|| bad(cmd, "`position` must be a number"))?.min(n.saturating_sub(1));
    let from = s.doc.feature_index(id).ok_or_else(|| bad(cmd, "feature"))?;
    if from == to {
        return Ok(json!({"feature": id, "position": to}));
    }
    let mut next = (*s.doc).clone();
    let f = next.features.remove(from);
    let name = f.name.clone();
    next.features.insert(to, f);
    // Everything that evaluated before must still evaluate.
    let mut after = solidcraft_doc::Model::new();
    after.evaluate(&next);
    let broken = newly_broken(&s.model, &after);
    if !broken.is_empty() {
        let list: Vec<String> = broken.iter().map(|(n, e)| format!("{n} ({e})")).collect();
        return Err(bad(cmd, format!("moving {name} there breaks {}", list.join("; "))));
    }
    *s.doc_mut() = next;
    Ok(json!({"feature": id, "position": to}))
}

fn input_keys(kind: &FeatureKind) -> Vec<(String, solidcraft_doc::expr::Kind)> {
    let mut keys: Vec<_> = kind.inputs().into_iter().map(|(label, _, unit)| (label.to_string(), unit)).collect();
    if let FeatureKind::Fillet { style, .. } = kind {
        if let Some((key, _)) = keys.first_mut() {
            *key = "fillet.radius".into();
        }
        if let solidcraft_doc::FilletStyle::Variable { mid, .. } = style {
            for ((key, _), (position, _)) in keys.iter_mut().skip(2).zip(mid) {
                *key = format!("fillet.mid.{}", position.to_bits());
            }
        }
    }
    keys
}

fn redefine(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.redefine";
    let id = feature_id(s, p.get("feature"), cmd)?;
    let cid = str_(p, "command").ok_or_else(|| bad(cmd, "`command` must name the feature's command"))?;
    let spec = super::find_command(cid).ok_or_else(|| bad(cmd, format!("no command `{cid}`")))?;
    // Only modeling constructors may run here. In particular, a scratch session does not
    // isolate filesystem writes performed by export commands.
    let constructor = super::features::COMMANDS
        .iter()
        .chain(super::features_more::COMMANDS)
        .chain(super::construct::COMMANDS)
        .chain(super::surface::COMMANDS)
        .any(|candidate| candidate.id == spec.id)
        || matches!(
            spec.id,
            "sheet.flange"
                | "sheet.hem"
                | "sheet.fold"
                | "sheet.unfold"
                | "sheet.refold"
                | "sheet.convert"
                | "plastic.boss"
                | "plastic.lip"
                | "plastic.snap_fit"
                | "plastic.rest"
        );
    if !constructor {
        return Err(bad(cmd, format!("`{cid}` does not make a feature")));
    }
    let mut params = p.get("params").cloned().unwrap_or_else(|| json!({}));
    let idx = s.doc.feature_index(id).ok_or_else(|| bad(cmd, "feature"))?;
    let old = s.doc.feature(id).cloned().ok_or_else(|| bad(cmd, "feature"))?;
    if let FeatureKind::Revolve { convention, .. } = &old.kind
        && cid == "solid.revolve"
        && let Some(parameters) = params.as_object_mut()
    {
        parameters.entry("convention").or_insert_with(|| json!(convention));
    }
    if !old.participants.is_empty()
        && let Some(parameters) = params.as_object_mut()
    {
        parameters.entry("participants").or_insert_with(|| json!(old.participants));
    }
    // Run the constructor on a scratch session: failed edits must not disturb sketch mode,
    // selections, component authoring state or the original feature's definition.
    let mut trial = s.scratch();
    trial.active_component = old.component;
    if let FeatureKind::Emboss { face: Some(face), .. } = &old.kind {
        trial.active_occurrence_path = (!face.author_path.is_empty()).then(|| face.author_path.clone());
    }
    trial.active_sketch = None;
    trial.selection = s.selection.clone();
    // Keep the original identity as a suppressed placeholder. The constructor and
    // any ownership boundary it adds go AFTER this slot, so discarding its temporary
    // feature leaves auxiliary history after the rewritten original.
    if let Some(slot) = trial.doc_mut().feature_mut(id) {
        slot.suppressed = true;
    }
    trial.doc_mut().marker = Some(idx + 1);
    trial.refresh();
    let r = spec.run_nested(&mut trial, &params)?;
    let made = r.get("feature").and_then(Value::as_u64).ok_or_else(|| bad(cmd, format!("`{cid}` did not make a feature")))?;
    let mut new = trial.doc.feature(made).cloned().ok_or_else(|| bad(cmd, format!("`{cid}` did not make a feature")))?;
    if let (FeatureKind::Emboss { face: Some(before), .. }, FeatureKind::Emboss { face: Some(after), .. }) = (&old.kind, &mut new.kind)
        && before.body == after.body
        && before.path == after.path
        && before.author_path == after.author_path
        && before.at.dist(after.at) < 1e-8
    {
        *after = before.clone();
    }
    if std::mem::discriminant(&new.kind) != std::mem::discriminant(&old.kind) {
        return Err(bad(cmd, format!("{} is not a {} feature", old.name, cid)));
    }
    // Optional inputs can move other inputs' positions (e.g. Distance 2 goes before Taper).
    // Keep each existing model parameter attached to its semantic input, rather than its
    // old index. Constructor-assigned names for new inputs cannot reuse the old dimensions.
    let mut old_inputs: Vec<_> = input_keys(&old.kind).into_iter().zip(&old.param_names).collect();
    let param_names: Vec<String> = input_keys(&new.kind)
        .iter()
        .enumerate()
        .map(|(i, (label, kind))| {
            if let Some(k) = old_inputs.iter().position(|((old_label, old_kind), _)| old_label == label && old_kind == kind) {
                old_inputs.remove(k).1.clone()
            } else {
                new.param_names.get(i).cloned().unwrap_or_default()
            }
        })
        .collect();
    trial.doc_mut().features.retain(|f| f.id != made);
    let added = trial.doc.features.len().saturating_sub(s.doc.features.len());
    trial.doc_mut().marker = s.doc.marker.map(|marker| if marker > idx { marker.saturating_add(added) } else { marker });
    if made == s.doc.next_id && trial.doc.next_id == made.saturating_add(1) {
        trial.doc_mut().next_id = s.doc.next_id;
    }
    if let Some(slot) = trial.doc_mut().feature_mut(id) {
        slot.suppressed = old.suppressed;
        slot.kind = new.kind;
        slot.edge_refs = new.edge_refs;
        slot.edge_names = new.edge_names;
        slot.face_names = new.face_names;
        slot.param_names = param_names;
        if params.get("participants").is_some() {
            slot.participants = new.participants;
        }
    }
    // New optional inputs need model parameter names, while existing dimensions keep theirs.
    trial.doc_mut().name_feature_inputs(id);
    trial.refresh();
    if let Some(e) = trial.model.result(id).and_then(|r| r.error.clone()) {
        return Err(EngineError::Other(e));
    }
    let original_ids: std::collections::BTreeSet<u64> = s.doc.features.iter().map(|feature| feature.id).collect();
    let added_ids: std::collections::BTreeSet<u64> =
        trial.doc.features.iter().filter(|feature| !original_ids.contains(&feature.id)).map(|feature| feature.id).collect();
    let adds_transfer =
        trial.doc.features.iter().any(|feature| added_ids.contains(&feature.id) && matches!(feature.kind, FeatureKind::BodyTransfer { .. }));
    let baseline = if adds_transfer && s.doc.marker.is_some() {
        let mut baseline = s.scratch();
        baseline.doc_mut().marker = None;
        baseline.refresh();
        Some(baseline)
    } else {
        None
    };
    let proof = if (adds_transfer && trial.doc.marker.is_some()) || added_ids.iter().any(|id| trial.model.result(*id).is_none()) {
        let mut proof = trial.scratch();
        proof.doc_mut().marker = None;
        proof.refresh();
        Some(proof)
    } else {
        None
    };
    let validated = proof.as_ref().map(|proof| &proof.model).unwrap_or(&trial.model);
    let original_model = baseline.as_ref().map(|baseline| &baseline.model).unwrap_or(&s.model);
    for feature in &trial.doc.features {
        let auxiliary = added_ids.contains(&feature.id);
        let protected_transfer = matches!(feature.kind, FeatureKind::BodyTransfer { .. })
            && original_model.result(feature.id).is_some_and(|result| !result.skipped && result.error.is_none());
        if (auxiliary || protected_transfer)
            && let Some(error) = validated.result(feature.id).and_then(|result| result.error.as_ref())
        {
            return Err(bad(cmd, format!("{}: {error}", feature.name)));
        }
    }
    s.pending_moves = trial.pending_moves;
    s.doc = trial.doc;
    s.refresh();
    if let Some(e) = s.model.result(id).and_then(|r| r.error.clone()) {
        return Err(EngineError::Other(e));
    }
    Ok(json!({"feature": id, "recomputed": s.model.last_recomputed, "restored": s.model.last_restored}))
}

fn dependents(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.dependents";
    let id = feature_id(s, p.get("feature"), cmd)?;
    let mut next = (*s.doc).clone();
    let gone = next.delete_feature(id)?;
    let mut after = solidcraft_doc::Model::new();
    after.evaluate(&next);
    let names = |ids: &[u64]| -> Vec<String> { ids.iter().filter(|x| **x != id).filter_map(|x| s.doc.feature(*x).map(|f| f.name.clone())).collect() };
    let broken: Vec<String> = newly_broken(&s.model, &after).into_iter().map(|(n, _)| n).collect();
    Ok(json!({"deleted_with_it": names(&gone), "would_fail": broken}))
}

fn first_body_edges(s: &Session, cmd: &str) -> Result<(String, Vec<solidcraft_kernel::EdgeInfo>)> {
    let body = s.model.state().bodies.first().cloned().ok_or_else(|| bad(cmd, "the design has no body"))?;
    let edges = body.body.edges(0.05).map_err(|e| bad(cmd, e.to_string()))?;
    if edges.is_empty() || edges.len() > 10_000 {
        return Err(bad(cmd, "the body has no edges, or too many"));
    }
    Ok((body.name, edges))
}

fn edge_ends(edge: &solidcraft_kernel::EdgeInfo) -> Option<(solidcraft_geom::Vec3, solidcraft_geom::Vec3)> {
    Some((*edge.points.first()?, *edge.points.last()?))
}

fn ends_touch(a: solidcraft_geom::Vec3, b: solidcraft_geom::Vec3) -> bool {
    a.dist(b) < 1e-3
}

fn edge_neighbors(edges: &[solidcraft_kernel::EdgeInfo], index: usize) -> Vec<usize> {
    let Some((a, b)) = edges.get(index).and_then(edge_ends) else { return Vec::new() };
    edges
        .iter()
        .enumerate()
        .filter(|(other, edge)| {
            *other != index && edge_ends(edge).is_some_and(|(c, d)| ends_touch(a, c) || ends_touch(a, d) || ends_touch(b, c) || ends_touch(b, d))
        })
        .map(|(other, _)| other)
        .collect()
}

fn face_loop(edges: &[solidcraft_kernel::EdgeInfo], start: usize) -> Vec<usize> {
    fn walk(edges: &[solidcraft_kernel::EdgeInfo], start: usize, path: &mut Vec<usize>) -> bool {
        if path.len() > 4 {
            return false;
        }
        if path.len() == 4 {
            return edge_neighbors(edges, *path.last().unwrap_or(&start)).contains(&start);
        }
        let here = *path.last().unwrap_or(&start);
        for next in edge_neighbors(edges, here) {
            if path.contains(&next) {
                continue;
            }
            path.push(next);
            if walk(edges, start, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = vec![start];
    if walk(edges, start, &mut path) { path } else { vec![start] }
}

fn remember_edges(s: &mut Session, body: &str, edges: &[solidcraft_kernel::EdgeInfo], indexes: &[usize]) {
    s.selection = indexes
        .iter()
        .filter_map(|index| {
            let edge = edges.get(*index)?;
            Some(Sel::Edge { body: body.to_string(), index: edge.index, point: edge.mid })
        })
        .collect();
}

fn select_loop(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "select.loop";
    let (name, edges) = first_body_edges(s, cmd)?;
    let start = p.get("edge").and_then(Value::as_u64).unwrap_or(0) as usize;
    if start >= edges.len() {
        return Err(bad(cmd, "that edge is not on the body"));
    }
    let indexes = face_loop(&edges, start);
    let count = indexes.len();
    remember_edges(s, &name, &edges, &indexes);
    Ok(json!({"count": count}))
}

fn select_ring(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "select.ring";
    let (name, edges) = first_body_edges(s, cmd)?;
    let start = p.get("edge").and_then(Value::as_u64).unwrap_or(0) as usize;
    let Some(edge) = edges.get(start) else { return Err(bad(cmd, "that edge is not on the body")) };
    let Some((a, b)) = edge_ends(edge) else { return Err(bad(cmd, "that edge has no ends")) };
    let dir = (b - a).normalized().ok_or_else(|| bad(cmd, "that edge has no direction"))?;
    let mut indexes = vec![start];
    for (i, other) in edges.iter().enumerate() {
        if i == start {
            continue;
        }
        let Some((c, d)) = edge_ends(other) else { continue };
        let Some(other_dir) = (d - c).normalized() else { continue };
        if other_dir.dot(dir).abs() > 0.98 {
            indexes.push(i);
        }
        if indexes.len() >= 64 {
            break;
        }
    }
    let count = indexes.len();
    remember_edges(s, &name, &edges, &indexes);
    Ok(json!({"count": count}))
}

fn selected_edge_indexes(s: &Session, edges: &[solidcraft_kernel::EdgeInfo]) -> Vec<usize> {
    s.selection
        .iter()
        .filter_map(|item| match item {
            Sel::Edge { index, .. } => edges.iter().position(|edge| edge.index == *index),
            _ => None,
        })
        .collect()
}

fn select_grow(s: &mut Session, _p: &Value) -> Result<Value> {
    let cmd = "select.grow";
    let (name, edges) = first_body_edges(s, cmd)?;
    let mut indexes = selected_edge_indexes(s, &edges);
    if indexes.is_empty() {
        return Err(bad(cmd, "nothing is selected"));
    }
    let extra: Vec<usize> = indexes.iter().flat_map(|index| edge_neighbors(&edges, *index)).collect();
    for index in extra {
        if !indexes.contains(&index) && indexes.len() < 10_000 {
            indexes.push(index);
        }
    }
    let count = indexes.len();
    remember_edges(s, &name, &edges, &indexes);
    Ok(json!({"count": count}))
}

fn select_shrink(s: &mut Session, _p: &Value) -> Result<Value> {
    let cmd = "select.shrink";
    let (name, edges) = first_body_edges(s, cmd)?;
    let mut indexes = selected_edge_indexes(s, &edges);
    if indexes.is_empty() {
        return Err(bad(cmd, "nothing is selected"));
    }
    indexes.pop();
    let count = indexes.len();
    remember_edges(s, &name, &edges, &indexes);
    Ok(json!({"count": count}))
}

fn select_invert(s: &mut Session, _p: &Value) -> Result<Value> {
    let cmd = "select.invert";
    let (name, edges) = first_body_edges(s, cmd)?;
    let chosen = selected_edge_indexes(s, &edges);
    let indexes: Vec<usize> = (0..edges.len()).filter(|index| !chosen.contains(index)).take(10_000).collect();
    let count = indexes.len();
    remember_edges(s, &name, &edges, &indexes);
    Ok(json!({"count": count}))
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
