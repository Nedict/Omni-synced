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
        .params("body: mesh body name; origin: [x,y,z]; normal: [x,y,z]; name?: sketch name. Draws the cut as sketch lines"),
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
        source
            .body
            .triangle_mesh()
            .map(|(pos, tri)| (pos.to_vec(), tri.to_vec()))
            .ok_or_else(|| bad(cmd, "select a mesh body"))?
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
    let local: Vec<[f64; 2]> = pts.iter().map(|q| {
        let v = plane.to_local(*q);
        [v.x, v.y]
    }).collect();
    // A polyline through the unique section points, in the order found.
    let mut chain = Vec::new();
    for q in local {
        if chain.last().is_none_or(|p: &[f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) > 1e-4) {
            chain.push(q);
        }
    }
    if chain.len() >= 2 {
        let _ = s.execute("sketch.line", &serde_json::json!({"points": chain, "infer": false}));
    }
    s.execute("sketch.finish", &serde_json::json!({}))?;
    Ok(serde_json::json!({"sketch": name, "points": chain.len()}))
}
