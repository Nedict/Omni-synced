//! Form: a 4×4 control grid, pulled, then thickened into a solid on the timeline.

use serde_json::{Value, json};
use solidcraft_doc::{FeatureKind, FormPull};
use super::CommandSpec;
use super::features::{add_feature, check_expr};
use crate::params::{bad, expr, req_expr, str_, vec3};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("form.create", "Create Form", create)
        .at("FORM", "CREATE")
        .icon("box")
        .params("shape?: box|cylinder; length, width, thickness: expr; pulls?: [[index, x, y, z]] on the 4×4 grid; name?"),
    CommandSpec::new("form.pull", "Pull Form", pull)
        .at("FORM", "MODIFY")
        .icon("move")
        .params("feature?: name (default: the latest form); index: 0…15; delta: [x,y,z]"),
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
    if shape != "box" && shape != "cylinder" {
        return Err(bad(cmd, "shape must be box or cylinder"));
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
        FeatureKind::Form { shape, length, width, thickness, pulls: pulls_of(p, cmd)? },
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
