//! Parametric reconstruction of imported mechanical mesh bodies.

use serde_json::Value;
use solidcraft_doc::FeatureKind;
use solidcraft_doc::expr::Kind;

use super::{
    CommandSpec,
    features::{add_feature, check_expr},
};
use crate::{
    Result, Session,
    params::{bad, expr, str_, vec3},
};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("mesh.convert", "Convert Mesh", convert)
        .at("MESH", "MODIFY")
        .icon("import")
        .params("body: mesh body name; mode?: prismatic|faceted (prismatic merges coplanar facets, faceted keeps a face per triangle); tolerance?: length expression (default 0.05 mm, prismatic only); name?"),
    CommandSpec::new("mesh.plane_cut", "Plane Cut", plane_cut)
        .at("MESH", "PREPARE")
        .icon("split")
        .params("body: mesh body name; origin: [x,y,z]; normal: [x,y,z]; keep: [x,y,z] (a point on the side to keep)"),
    CommandSpec::new("mesh.repair", "Repair Mesh", repair).at("MESH", "PREPARE").icon("import").params("body: mesh body name"),
    CommandSpec::new("mesh.reduce", "Reduce Mesh", reduce)
        .at("MESH", "PREPARE")
        .icon("import")
        .params("body: mesh body name; ratio: 0…1 (fraction of detail to keep)"),
    CommandSpec::new("mesh.face_groups", "Face Groups", face_groups)
        .at("MESH", "PREPARE")
        .icon("import")
        .noundo()
        .params("body: mesh body name. Groups triangles by which axis their normal faces"),
    CommandSpec::new("mesh.section", "Mesh Section", section)
        .at("MESH", "CREATE")
        .icon("sketch")
        .params("body: mesh body name; origin: [x,y,z]; normal: [x,y,z]; name?: sketch name. Draws every closed cut loop as sketch lines"),
    CommandSpec::new("mesh.remesh", "Remesh", remesh)
        .at("MESH", "MODIFY")
        .icon("import")
        .params("body: mesh body name; target: triangle count"),
    CommandSpec::new("mesh.smooth", "Smooth Mesh", smooth)
        .at("MESH", "MODIFY")
        .icon("fillet")
        .params("body: mesh body name; iterations?: 1…50"),
    CommandSpec::new("mesh.separate", "Separate Mesh", separate).at("MESH", "MODIFY").icon("split").params("body: mesh body name"),
    CommandSpec::new("mesh.combine", "Combine Mesh", combine)
        .at("MESH", "MODIFY")
        .icon("combine")
        .params("a, b: mesh body names; operation?: join"),
    CommandSpec::new("mesh.merge_groups", "Merge Face Groups", merge_groups)
        .at("MESH", "MODIFY")
        .icon("stitch")
        .params("body: mesh body name. Welds the mesh the way a repair does"),
    CommandSpec::new("mesh.shell", "Shell Mesh", shell)
        .at("MESH", "MODIFY")
        .icon("shell")
        .params("body: mesh body name; thickness: expr"),
    CommandSpec::new("mesh.reverse", "Reverse Mesh", reverse).at("MESH", "MODIFY").icon("mirror").params("body: mesh body name"),
    CommandSpec::new("mesh.deviation", "Mesh Deviation", deviation)
        .noundo()
        .params("body: solid name; mesh: mesh body name → {max, mean} millimetres"),
];

fn convert(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.convert";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?;
    let state = s.model.state();
    let source = state.body(body).ok_or_else(|| bad(cmd, format!("no body `{body}`")))?;
    if !source.body.is_mesh() {
        return Err(bad(cmd, "select a mesh body"));
    }
    if s.doc.model_body_component(source) != s.active_component {
        return Err(bad(cmd, "activate the mesh body's component before converting it"));
    }
    let tolerance = expr(p, "tolerance").unwrap_or_else(|| "0.05 mm".into());
    check_expr(s, &tolerance, Kind::Length, cmd, "tolerance")?;
    let mode = str_(p, "mode").unwrap_or("prismatic").to_ascii_lowercase();
    if mode != "prismatic" && mode != "faceted" {
        return Err(bad(cmd, "mode must be prismatic or faceted"));
    }
    add_feature(s, p, FeatureKind::MeshConvert { body: body.into(), tolerance, mode })
}

fn plane_cut(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.plane_cut";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?.to_string();
    let origin = p.get("origin").and_then(vec3).ok_or_else(|| bad(cmd, "`origin` must be [x,y,z]"))?;
    let normal = p.get("normal").and_then(vec3).ok_or_else(|| bad(cmd, "`normal` must be [x,y,z]"))?;
    let keep = p.get("keep").and_then(vec3).ok_or_else(|| bad(cmd, "`keep` must be [x,y,z]"))?;
    add_feature(s, p, FeatureKind::MeshCut { body, origin, normal, keep })
}

fn repair(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.repair";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?.to_string();
    add_feature(s, p, FeatureKind::MeshRepair { body })
}

fn reduce(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.reduce";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?.to_string();
    let ratio = expr(p, "ratio").unwrap_or_else(|| "0.5".into());
    check_expr(s, &ratio, Kind::Unitless, cmd, "ratio")?;
    add_feature(s, p, FeatureKind::MeshReduce { body, ratio })
}

fn face_groups(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.face_groups";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?;
    let state = s.model.state();
    let source = state.body(body).ok_or_else(|| bad(cmd, format!("no body `{body}`")))?;
    let groups = solidcraft_kernel::mesh_face_groups(&source.body).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(serde_json::json!({
        "body": body,
        "groups": groups.iter().map(|(name, count)| serde_json::json!({"name": name, "triangles": count})).collect::<Vec<_>>(),
    }))
}

fn section(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.section";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?;
    let origin = p.get("origin").and_then(vec3).ok_or_else(|| bad(cmd, "`origin` must be [x,y,z]"))?;
    let normal = p.get("normal").and_then(vec3).and_then(|n| n.normalized()).ok_or_else(|| bad(cmd, "`normal` must be a direction"))?;
    let (positions, triangles) = {
        let state = s.model.state();
        let source = state.body(body).ok_or_else(|| bad(cmd, format!("no body `{body}`")))?;
        source.body.triangle_mesh().map(|(pos, tri)| (pos.to_vec(), tri.to_vec())).ok_or_else(|| bad(cmd, "select a mesh body"))?
    };
    let mut pts = Vec::new();
    for t in &triangles {
        let tri: Vec<solidcraft_geom::Vec3> = t.iter().filter_map(|i| positions.get(*i as usize).copied()).collect();
        let [a, b, c] = tri.as_slice() else { continue };
        for (p0, p1) in [(*a, *b), (*b, *c), (*c, *a)] {
            let (s0, s1) = ((p0 - origin).dot(normal), (p1 - origin).dot(normal));
            if s0 * s1 < 0.0 {
                let t = s0 / (s0 - s1);
                pts.push(p0 + (p1 - p0) * t);
            }
        }
    }
    if pts.len() < 2 {
        return Err(bad(cmd, "the plane misses the mesh"));
    }
    let name = str_(p, "name").unwrap_or("Mesh Section");
    let x = normal.any_perp();
    let y = normal.cross(x);
    s.execute(
        "sketch.create",
        &serde_json::json!({"plane": {"origin": [origin.x, origin.y, origin.z], "x_dir": [x.x, x.y, x.z], "y_dir": [y.x, y.y, y.z]}, "name": name}),
    )?;
    let plane = solidcraft_geom::Plane { origin, x, y };
    let local: Vec<solidcraft_geom::Vec2> = pts.iter().map(|q| plane.to_local(*q)).collect();
    let loops = section_loops(&local);
    if loops.is_empty() {
        return Err(bad(cmd, "the plane misses the mesh"));
    }
    for ring in &loops {
        let chain: Vec<[f64; 2]> = ring.iter().map(|q| [q.x, q.y]).collect();
        if chain.len() >= 2 {
            let _ = s.execute("sketch.line", &serde_json::json!({"points": chain, "infer": false}));
        }
    }
    s.execute("sketch.finish", &serde_json::json!({}))?;
    let points = loops.iter().map(Vec::len).sum::<usize>();
    Ok(serde_json::json!({"sketch": name, "points": points, "loops": loops.len()}))
}

/// Chain section points into closed loops. Points closer than 0.05 mm are the same vertex.
fn section_loops(points: &[solidcraft_geom::Vec2]) -> Vec<Vec<solidcraft_geom::Vec2>> {
    let mut unused: Vec<solidcraft_geom::Vec2> = Vec::new();
    for point in points {
        if unused.iter().all(|kept| kept.dist(*point) > 0.05) {
            unused.push(*point);
        }
    }
    let mut loops = Vec::new();
    while let Some(start) = unused.first().copied() {
        let mut loop_pts = vec![start];
        unused.remove(0);
        while let Some((index, _)) =
            unused.iter().enumerate().filter_map(|(i, point)| loop_pts.last().map(|end| (i, end.dist(*point)))).min_by(|a, b| a.1.total_cmp(&b.1))
        {
            let Some(end) = loop_pts.last().copied() else { break };
            let Some(next) = unused.get(index).copied() else { break };
            if end.dist(next) > 5.0 && loop_pts.len() > 2 {
                break;
            }
            loop_pts.push(next);
            unused.remove(index);
            if loop_pts.len() > 2 && next.dist(start) < 0.2 {
                break;
            }
            if loop_pts.len() > 10_000 {
                break;
            }
        }
        if loop_pts.len() >= 3 {
            loops.push(loop_pts);
        }
    }
    loops
}

fn mesh_feature(s: &mut Session, p: &Value, cmd: &str, kind: impl FnOnce(String) -> Result<FeatureKind>) -> Result<Value> {
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "select a mesh body"))?.to_string();
    add_feature(s, p, kind(body)?)
}

fn remesh(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.remesh";
    let target = expr(p, "target").unwrap_or_else(|| "1000".into());
    check_expr(s, &target, Kind::Unitless, cmd, "target")?;
    mesh_feature(s, p, cmd, |body| Ok(FeatureKind::MeshRemesh { body, target }))
}

fn smooth(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.smooth";
    let iterations = p.get("iterations").and_then(Value::as_u64).unwrap_or(1);
    if iterations == 0 || iterations > 50 {
        return Err(bad(cmd, "iterations are 1…50"));
    }
    mesh_feature(s, p, cmd, |body| Ok(FeatureKind::MeshSmooth { body, iterations: iterations as u32 }))
}

fn separate(s: &mut Session, p: &Value) -> Result<Value> {
    mesh_feature(s, p, "mesh.separate", |body| Ok(FeatureKind::MeshSeparate { body }))
}

fn combine(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.combine";
    let a = str_(p, "a").filter(|b| !b.is_empty()).ok_or_else(|| bad(cmd, "`a` is required"))?.to_string();
    let b = str_(p, "b").filter(|b| !b.is_empty()).ok_or_else(|| bad(cmd, "`b` is required"))?.to_string();
    let operation = str_(p, "operation").unwrap_or("join").to_ascii_lowercase();
    if operation != "join" {
        return Err(bad(cmd, "not supported yet: a mesh combine that is not a join"));
    }
    add_feature(s, p, FeatureKind::MeshCombine { a, b, operation })
}

fn merge_groups(s: &mut Session, p: &Value) -> Result<Value> {
    mesh_feature(s, p, "mesh.merge_groups", |body| Ok(FeatureKind::MeshMergeGroups { body }))
}

fn shell(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.shell";
    let thickness = expr(p, "thickness").unwrap_or_else(|| "1 mm".into());
    check_expr(s, &thickness, Kind::Length, cmd, "thickness")?;
    mesh_feature(s, p, cmd, |body| Ok(FeatureKind::MeshShell { body, thickness }))
}

fn reverse(s: &mut Session, p: &Value) -> Result<Value> {
    mesh_feature(s, p, "mesh.reverse", |body| Ok(FeatureKind::MeshReverse { body }))
}

fn deviation(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "mesh.deviation";
    let body = str_(p, "body").filter(|b| !b.is_empty()).ok_or_else(|| bad(cmd, "`body` is required"))?;
    let mesh = str_(p, "mesh").filter(|b| !b.is_empty()).ok_or_else(|| bad(cmd, "`mesh` is required"))?;
    let state = s.model.state();
    let solid = state.body(body).ok_or_else(|| bad(cmd, format!("no body `{body}`")))?;
    let mesh_body = state.body(mesh).ok_or_else(|| bad(cmd, format!("no body `{mesh}`")))?;
    let (max, mean) = solidcraft_kernel::mesh_deviation(&solid.body, &mesh_body.body).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(serde_json::json!({"max": max, "mean": mean}))
}
