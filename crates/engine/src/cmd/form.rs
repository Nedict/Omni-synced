//! Form: a 4×4 control grid, pulled, then thickened into a solid on the timeline.

use super::CommandSpec;
use super::features::{add_feature, check_expr};
use crate::params::{bad, expr, req_expr, str_, vec3};
use crate::{Result, Session};
use serde_json::{Value, json};
use solidcraft_doc::{FeatureKind, FormEdit, FormPull};
use solidcraft_geom::Vec3;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("form.create", "Create Form", create).at("FORM", "CREATE").icon("box").params(
        "shape?: box|plane|cylinder|sphere|quadball|torus; length, width, thickness: expr; pulls?: [[index, x, y, z]] on the 4×4 grid; name?",
    ),
    CommandSpec::new("form.pull", "Pull Form", pull)
        .at("FORM", "MODIFY")
        .icon("move")
        .params("feature?: name (default: the latest form); index: 0…15; delta: [x,y,z]"),
    CommandSpec::new("form.edit_point", "Edit Form Point", edit_point)
        .at("FORM", "MODIFY")
        .icon("move")
        .params("feature?; index: 0…15; move?: [x,y,z] | rotate?: radians about Z | scale?: factor about the cage centre"),
    CommandSpec::new("form.insert_edge", "Insert Form Edge", insert_edge)
        .at("FORM", "MODIFY")
        .icon("line")
        .params("feature?; index: edge 0…23. Separates that edge's control points"),
    CommandSpec::new("form.insert_point", "Insert Form Point", insert_point).at("FORM", "MODIFY").icon("point").params("feature?; index: 0…15"),
    CommandSpec::new("form.subdivide", "Subdivide Form", subdivide)
        .at("FORM", "MODIFY")
        .icon("pattern_rect")
        .params("feature?; level: 1…6. Each level inserts edge and face midpoints, growing the cage from 4×4 toward at most 20000 points"),
    CommandSpec::new("form.bridge", "Bridge Form", bridge).at("FORM", "MODIFY").icon("loft").params("feature?; a, b: control indices 0…15"),
    CommandSpec::new("form.weld", "Weld Form", weld).at("FORM", "MODIFY").icon("combine").params("feature?; index: 0…15"),
    CommandSpec::new("form.crease", "Crease Form", crease).at("FORM", "MODIFY").icon("chamfer").params("feature?; index: edge 0…23"),
    CommandSpec::new("form.uncrease", "Uncrease Form", uncrease).at("FORM", "MODIFY").icon("fillet").params("feature?; index: edge 0…23"),
    CommandSpec::new("form.symmetry", "Form Symmetry", symmetry).at("FORM", "MODIFY").icon("mirror").params("feature?; plane: yz|xz|xy"),
    CommandSpec::new("form.extrude_face", "Extrude Form Face", extrude_face)
        .at("FORM", "MODIFY")
        .icon("extrude")
        .params("feature?; index: 0…15; delta: [x,y,z]"),
    CommandSpec::new("form.finish", "Finish Form", finish).at("FORM", "MODIFY").icon("finish").params("feature?; mode?: solid|surface"),
    CommandSpec::new("form.thicken", "Thicken Form", thicken_form).at("FORM", "MODIFY").icon("shell").params("feature?; thickness: expr"),
    CommandSpec::new("form.match", "Match Form Edge", match_edge)
        .at("FORM", "MODIFY")
        .icon("align")
        .params("feature?; edge: 0…23; target_edges: count to meet. The cage is refined until that edge is split into at least this many segments"),
];

fn pulls_of(p: &Value, cmd: &str) -> Result<Vec<FormPull>> {
    let mut out = Vec::new();
    for row in p.get("pulls").and_then(Value::as_array).into_iter().flatten().take(64) {
        let Some(arr) = row.as_array() else { return Err(bad(cmd, "each pull is [index, x, y, z]")) };
        let index = arr.first().and_then(Value::as_u64).ok_or_else(|| bad(cmd, "a pull index is 0…15"))?;
        if index > 15 {
            return Err(bad(cmd, "a pull index is 0…15"));
        }
        let delta = vec3(&Value::Array(arr.iter().skip(1).cloned().collect())).ok_or_else(|| bad(cmd, "a pull delta is [x, y, z]"))?;
        out.push(FormPull { index: index as u32, delta });
    }
    Ok(out)
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.create";
    let shape = str_(p, "shape").unwrap_or("box").to_ascii_lowercase();
    if !matches!(shape.as_str(), "box" | "plane" | "cylinder" | "sphere" | "quadball" | "torus") {
        return Err(bad(cmd, "shape must be box, plane, cylinder, sphere, quadball or torus"));
    }
    let length = req_expr(cmd, p, "length")?;
    let width = req_expr(cmd, p, "width")?;
    let thickness = expr(p, "thickness").unwrap_or_else(|| "2 mm".into());
    for (name, e) in [("length", &length), ("width", &width), ("thickness", &thickness)] {
        check_expr(s, e, solidcraft_doc::expr::Kind::Length, cmd, name)?;
    }
    add_feature(
        s,
        p,
        FeatureKind::Form {
            shape,
            length,
            width,
            thickness,
            pulls: pulls_of(p, cmd)?,
            edits: Vec::new(),
            symmetry: None,
            creases: Vec::new(),
            finished: None,
        },
    )
}

fn pull(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.pull";
    let index = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "`index` is 0…15"))?;
    if index > 15 {
        return Err(bad(cmd, "`index` is 0…15"));
    }
    let delta = p.get("delta").and_then(vec3).ok_or_else(|| bad(cmd, "`delta` must be [x, y, z]"))?;
    let name = str_(p, "feature").map(str::to_string);
    let id = s
        .doc
        .features
        .iter()
        .rev()
        .find(|f| name.as_ref().is_none_or(|n| f.name == *n || f.id.to_string() == *n) && matches!(f.kind, FeatureKind::Form { .. }))
        .map(|f| f.id)
        .ok_or_else(|| bad(cmd, "no form feature to pull"))?;
    if let Some(f) = s.doc_mut().feature_mut(id)
        && let FeatureKind::Form { pulls, .. } = &mut f.kind
    {
        if let Some(existing) = pulls.iter_mut().find(|q| q.index == index as u32) {
            existing.delta += delta;
        } else {
            pulls.push(FormPull { index: index as u32, delta });
        }
    }
    s.refresh();
    let err = s.model.result(id).and_then(|r| r.error.clone());
    if let Some(e) = err {
        return Err(bad(cmd, e));
    }
    Ok(json!({"feature": id}))
}

const MAX_EDITS: usize = 10_000;

fn latest_form(s: &Session, p: &Value, cmd: &str) -> Result<u64> {
    let name = str_(p, "feature").map(str::to_string);
    s.doc
        .features
        .iter()
        .rev()
        .find(|f| name.as_ref().is_none_or(|n| f.name == *n || f.id.to_string() == *n) && matches!(f.kind, FeatureKind::Form { .. }))
        .map(|f| f.id)
        .ok_or_else(|| bad(cmd, "no form feature"))
}

fn push_edit(s: &mut Session, cmd: &str, id: u64, edit: FormEdit) -> Result<Value> {
    if let Some(f) = s.doc_mut().feature_mut(id)
        && let FeatureKind::Form { edits, .. } = &mut f.kind
    {
        if edits.len() >= MAX_EDITS {
            return Err(bad(cmd, "a form keeps at most 10000 edits"));
        }
        edits.push(edit);
    }
    s.refresh();
    if let Some(e) = s.model.result(id).and_then(|r| r.error.clone()) {
        return Err(bad(cmd, e));
    }
    Ok(json!({"feature": id}))
}

fn edit_point(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.edit_point";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i <= 15).ok_or_else(|| bad(cmd, "`index` is 0…15"))? as u32;
    let delta = if let Some(d) = p.get("move").and_then(vec3) {
        d
    } else if let Some(turns) = p.get("rotate").and_then(Value::as_f64).filter(|n| n.is_finite()) {
        let c = turns.cos();
        let sn = turns.sin();
        Vec3::new(c - 1.0, sn, 0.0)
    } else if let Some(scale) = p.get("scale").and_then(Value::as_f64).filter(|n| n.is_finite() && n.abs() < 1e6) {
        Vec3::new(0.0, 0.0, scale - 1.0)
    } else {
        return Err(bad(cmd, "give `move`, `rotate` or `scale`"));
    };
    if !delta.is_finite() {
        return Err(bad(cmd, "the edit must be finite"));
    }
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::MovePoint { index, delta })
}

fn insert_edge(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.insert_edge";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i < 24).ok_or_else(|| bad(cmd, "`index` is an edge 0…23"))? as u32;
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::InsertEdge { index })
}

fn insert_point(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.insert_point";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i <= 15).ok_or_else(|| bad(cmd, "`index` is 0…15"))? as u32;
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::InsertPoint { index })
}

fn form_level(s: &Session, id: u64) -> u32 {
    let Some(f) = s.doc.feature(id) else { return 0 };
    let FeatureKind::Form { edits, .. } = &f.kind else { return 0 };
    edits
        .iter()
        .fold(0u32, |n, e| match e {
            FormEdit::Subdivide { level } => n.saturating_add(*level),
            _ => n,
        })
        .min(6)
}

fn subdivide(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.subdivide";
    let level = p.get("level").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "`level` is 1…6"))?;
    if level == 0 || level > 6 {
        return Err(bad(cmd, "subdivision level is at most 6"));
    }
    let id = latest_form(s, p, cmd)?;
    let total = form_level(s, id).saturating_add(level as u32);
    if total > 6 {
        return Err(bad(cmd, "subdivision level is at most 6"));
    }
    let points = solidcraft_kernel::cage_points(total).map_err(|e| bad(cmd, e.to_string()))?;
    let mut out = push_edit(s, cmd, id, FormEdit::Subdivide { level: level as u32 })?;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("points".into(), json!(points));
    }
    Ok(out)
}

fn bridge(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.bridge";
    let a = p.get("a").and_then(Value::as_u64).filter(|i| *i <= 15).ok_or_else(|| bad(cmd, "`a` is 0…15"))? as u32;
    let b = p.get("b").and_then(Value::as_u64).filter(|i| *i <= 15).ok_or_else(|| bad(cmd, "`b` is 0…15"))? as u32;
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::Bridge { a, b })
}

fn weld(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.weld";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i <= 15).ok_or_else(|| bad(cmd, "`index` is 0…15"))? as u32;
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::Weld { index })
}

fn crease(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.crease";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i < 24).ok_or_else(|| bad(cmd, "`index` is an edge 0…23"))? as u32;
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::Crease { index })
}

fn uncrease(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.uncrease";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i < 24).ok_or_else(|| bad(cmd, "`index` is an edge 0…23"))? as u32;
    let id = latest_form(s, p, cmd)?;
    let doc = s.doc_mut();
    let Some(f) = doc.feature_mut(id) else { return Err(bad(cmd, "no form feature")) };
    if let FeatureKind::Form { edits, creases, .. } = &mut f.kind {
        edits.retain(|e| !matches!(e, FormEdit::Crease { index: i } if *i == index));
        creases.retain(|i| *i != index);
    }
    s.refresh();
    Ok(json!({"feature": id}))
}

fn symmetry(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.symmetry";
    let plane = str_(p, "plane").unwrap_or("yz").to_ascii_lowercase();
    if !matches!(plane.as_str(), "yz" | "xz" | "xy") {
        return Err(bad(cmd, "plane must be yz, xz or xy"));
    }
    let id = latest_form(s, p, cmd)?;
    if let Some(f) = s.doc_mut().feature_mut(id)
        && let FeatureKind::Form { symmetry, .. } = &mut f.kind
    {
        *symmetry = Some(plane);
    }
    s.refresh();
    if let Some(e) = s.model.result(id).and_then(|r| r.error.clone()) {
        return Err(bad(cmd, e));
    }
    Ok(json!({"feature": id}))
}

fn extrude_face(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.extrude_face";
    let index = p.get("index").and_then(Value::as_u64).filter(|i| *i <= 15).ok_or_else(|| bad(cmd, "`index` is 0…15"))? as u32;
    let delta = p.get("delta").and_then(vec3).filter(|d| d.is_finite()).ok_or_else(|| bad(cmd, "`delta` must be [x,y,z]"))?;
    let id = latest_form(s, p, cmd)?;
    push_edit(s, cmd, id, FormEdit::ExtrudeFace { index, delta })
}

fn finish(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.finish";
    let mode = str_(p, "mode").unwrap_or("solid").to_ascii_lowercase();
    if !matches!(mode.as_str(), "solid" | "surface") {
        return Err(bad(cmd, "mode must be solid or surface"));
    }
    let id = latest_form(s, p, cmd)?;
    if let Some(f) = s.doc_mut().feature_mut(id)
        && let FeatureKind::Form { finished, .. } = &mut f.kind
    {
        *finished = Some(mode);
    }
    s.refresh();
    if let Some(e) = s.model.result(id).and_then(|r| r.error.clone()) {
        return Err(bad(cmd, e));
    }
    let surface = s.model.state().bodies.iter().any(|b| b.feature == id && b.body.is_surface());
    Ok(json!({"feature": id, "surface": surface}))
}

fn thicken_form(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.thicken";
    let thickness = crate::params::req_expr(cmd, p, "thickness")?;
    super::features::check_expr(s, &thickness, solidcraft_doc::expr::Kind::Length, cmd, "thickness")?;
    let id = latest_form(s, p, cmd)?;
    if let Some(f) = s.doc_mut().feature_mut(id)
        && let FeatureKind::Form { thickness: slot, finished, .. } = &mut f.kind
    {
        *slot = thickness;
        *finished = Some("solid".into());
    }
    s.refresh();
    if let Some(e) = s.model.result(id).and_then(|r| r.error.clone()) {
        return Err(bad(cmd, e));
    }
    Ok(json!({"feature": id}))
}

fn match_edge(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "form.match";
    let edge = p.get("edge").and_then(Value::as_u64).filter(|i| *i < 24).ok_or_else(|| bad(cmd, "`edge` is 0…23"))? as u32;
    let targets = p.get("target_edges").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "`target_edges` is a count"))?;
    if targets == 0 || targets > 64 {
        return Err(bad(cmd, "`target_edges` is 1…64"));
    }
    let id = latest_form(s, p, cmd)?;
    if targets == 1 {
        return push_edit(s, cmd, id, FormEdit::InsertEdge { index: edge });
    }
    let mut pieces = 1u32;
    let mut want = 0u32;
    while pieces < targets as u32 {
        want += 1;
        if want > 6 {
            return Err(bad(cmd, "subdivision level is at most 6"));
        }
        pieces = pieces.saturating_mul(2);
    }
    let have = form_level(s, id);
    if have >= want {
        let points = solidcraft_kernel::cage_points(have).map_err(|e| bad(cmd, e.to_string()))?;
        return Ok(json!({"feature": id, "points": points, "edge": edge}));
    }
    let add = want - have;
    let points = solidcraft_kernel::cage_points(have + add).map_err(|e| bad(cmd, e.to_string()))?;
    let mut out = push_edit(s, cmd, id, FormEdit::Subdivide { level: add })?;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("points".into(), json!(points));
        obj.insert("edge".into(), json!(edge));
    }
    Ok(out)
}
