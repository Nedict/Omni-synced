//! Surface commands: Patch, Stitch, Thicken and Trim (surface bodies and turning them into
//! solids), and the planned surface toolset (`planned!`: registered, not built yet).

use serde_json::Value;
use solidcraft_doc::{FaceAt, FeatureKind, PlaneRef, ProfileSel, expr::Kind};

use super::CommandSpec;
use super::features::{add_feature, check_expr, feature_sketch, operation, plane_param, profiles};
use crate::params::{bad, bool_, expr, str_, string_list, vec3};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("surface.patch", "Patch", patch)
        .at("SURFACE", "CREATE")
        .icon("patch")
        .params("sketch?: id|name and profiles? (planar patches of sketch regions) | edges: [[x,y,z]] (points on a closed loop of a body's edges: one face on them, planar when the loop is, else a smooth B-spline surface) and body?; name?, body_name?"),
    CommandSpec::new("surface.stitch", "Stitch", stitch)
        .at("SURFACE", "MODIFY")
        .icon("stitch")
        .params("bodies: [names] (surfaces sewn into the first; a solid when they close); tolerance?: expr (default: tight)"),
    CommandSpec::new("surface.thicken", "Thicken", thicken)
        .at("SURFACE", "CREATE")
        .icon("thicken")
        .params("bodies: [surface names]; thickness: expr (negative: against the normal); symmetric?: bool; operation?: new|join|cut|intersect; targets?"),
    CommandSpec::new("surface.extend", "Extend", extend)
        .at("SURFACE", "MODIFY")
        .icon("extend")
        .params("body: planar surface name; edges: [[x,y,z]] (points on straight edges to push out); distance: expr"),
    CommandSpec::new("surface.trim", "Trim", trim)
        .at("SURFACE", "MODIFY")
        .icon("trim")
        .params("body: surface name; plane: XY|XZ|YZ|plane name|{origin, normal} or tool: {body, point} (a face, its surface extended); keep: [x,y,z] (a point on the side to keep)"),
    planned!("surface.extrude", "Surface Extrude", "757")
        .at("SURFACE", "CREATE")
        .icon("surface_extrude")
        .params("planned: the parameters of solid.extrude, with open curve chains as well as profiles (walls only, no caps); caps?: bool (close a closed profile)"),
    planned!("surface.revolve", "Surface Revolve", "757")
        .at("SURFACE", "CREATE")
        .icon("surface_revolve")
        .params("planned: the parameters of solid.revolve, with open curve chains as well as profiles (no caps); caps?: bool"),
    planned!("surface.sweep", "Surface Sweep", "757")
        .at("SURFACE", "CREATE")
        .icon("surface_sweep")
        .params("planned: the parameters of solid.sweep, with an open profile chain as well as a closed one (no caps); caps?: bool"),
    planned!("surface.loft", "Surface Loft", "757")
        .at("SURFACE", "CREATE")
        .icon("surface_loft")
        .params("planned: the parameters of solid.loft, with open sections as well as closed ones (no caps); caps?: bool"),
    planned!("surface.offset", "Offset Surface", "758")
        .at("SURFACE", "CREATE")
        .icon("surface_offset")
        .params("planned: faces: [{body, point}]; distance: expr (signed, along the normals; 0 copies the faces); chain?: bool (the whole tangent chain of each pick); tolerance?: expr (B-spline fit, default 0.01 mm)"),
    planned!("surface.ruled", "Ruled Surface", "762")
        .at("SURFACE", "CREATE")
        .icon("ruled")
        .params("planned: body; edges: [[x,y,z]] (points on an edge chain); distance: expr; mode?: normal|tangent|angle|direction; angle?: expr; direction?: [x,y,z]; flip?: bool; corners?: mitre|round"),
    planned!("surface.boundary", "Boundary Surface", "763")
        .at("SURFACE", "CREATE")
        .icon("boundary")
        .params("planned: u: [curves], v?: [curves] (sketch curves, 3D sketch wires or {body, point} edges; 2–4 boundaries, more make a network); continuity?: [{curve, g: 0|1|2}] for edge boundaries; operation?: new|join"),
    planned!("surface.blend", "Blend Surface", "764")
        .at("SURFACE", "CREATE")
        .icon("surface_blend")
        .params("planned: from, to: {body, edges: [[x,y,z]]} (two edge chains apart); continuity?: [g0|g1|g2, g0|g1|g2] (each side); bulge?: [number, number]; align?: auto|[[x,y,z], [x,y,z]]; stitch?: bool"),
    planned!("surface.untrim", "Untrim", "761")
        .at("SURFACE", "MODIFY")
        .icon("untrim")
        .params("planned: face: {body, point}; holes?: all|[[x,y,z]] (points on the inner loops to remove); outer?: bool (restore the natural bounds); in_place?: bool (a surface body only)"),
    planned!("surface.unstitch", "Unstitch", "766")
        .at("SURFACE", "MODIFY")
        .icon("unstitch")
        .params("planned: body; faces?: [[x,y,z]] (points on the faces to split off; default: every face its own surface body)"),
    planned!("surface.reverse_normal", "Reverse Normal", "766")
        .at("SURFACE", "MODIFY")
        .icon("reverse_normal")
        .params("planned: body (a surface); faces?: [[x,y,z]] (default: all of them)"),
    planned!("surface.fillet", "Surface Fillet", "769")
        .at("SURFACE", "MODIFY")
        .icon("surface_fillet")
        .params("planned: a, b: {body, point} (two faces of separate bodies that meet or cross; the points pick the side the ball rolls on); radius: expr; trim?: both|first|none"),
];

fn patch(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "surface.patch";
    let edges: Vec<solidcraft_geom::Vec3> = p.get("edges").and_then(Value::as_array).map(|a| a.iter().filter_map(vec3).collect()).unwrap_or_default();
    if !edges.is_empty() {
        if edges.len() > 10_000 {
            return Err(bad(cmd, "too many edges"));
        }
        let body = str_(p, "body").map(str::to_string);
        return add_feature(s, p, FeatureKind::Patch { sketch: None, profiles: ProfileSel::All, body, edges });
    }
    let sketch = feature_sketch(s, p, cmd)?;
    let profiles = profiles(p, cmd)?;
    add_feature(s, p, FeatureKind::Patch { sketch: Some(sketch), profiles, body: None, edges: Vec::new() })
}

fn bodies_param(s: &Session, p: &Value, cmd: &str) -> Result<Vec<String>> {
    let bodies = string_list(p, "bodies");
    if bodies.is_empty() {
        return Err(bad(cmd, "`bodies` must list bodies"));
    }
    let st = s.model.state();
    if let Some(b) = bodies.iter().find(|b| st.body(b).is_none()) {
        return Err(bad(cmd, format!("no body `{b}`")));
    }
    Ok(bodies)
}

fn stitch(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "surface.stitch";
    let bodies = bodies_param(s, p, cmd)?;
    let tolerance = expr(p, "tolerance").unwrap_or_else(|| "0".into());
    check_expr(s, &tolerance, Kind::Length, cmd, "tolerance")?;
    add_feature(s, p, FeatureKind::Stitch { bodies, tolerance })
}

fn thicken(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "surface.thicken";
    let bodies = bodies_param(s, p, cmd)?;
    let thickness = expr(p, "thickness").ok_or_else(|| bad(cmd, "`thickness` is required"))?;
    check_expr(s, &thickness, Kind::Length, cmd, "thickness")?;
    let kind = FeatureKind::Thicken {
        bodies,
        thickness,
        symmetric: bool_(p, "symmetric").unwrap_or(false),
        operation: operation(p, cmd)?,
        targets: string_list(p, "targets"),
    };
    add_feature(s, p, kind)
}

fn extend(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "surface.extend";
    let body = str_(p, "body").ok_or_else(|| bad(cmd, "`body` is required"))?.to_string();
    if s.model.state().body(&body).is_none() {
        return Err(bad(cmd, format!("no body `{body}`")));
    }
    let edges: Vec<solidcraft_geom::Vec3> = p.get("edges").and_then(Value::as_array).map(|a| a.iter().filter_map(vec3).collect()).unwrap_or_default();
    if edges.is_empty() || edges.len() > 10_000 {
        return Err(bad(cmd, "`edges` must list points on the edges to extend"));
    }
    let distance = expr(p, "distance").ok_or_else(|| bad(cmd, "`distance` is required"))?;
    check_expr(s, &distance, Kind::Length, cmd, "distance")?;
    add_feature(s, p, FeatureKind::SurfaceExtend { body, edges, distance })
}

fn trim(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "surface.trim";
    let body = str_(p, "body").ok_or_else(|| bad(cmd, "`body` is required"))?.to_string();
    if s.model.state().body(&body).is_none() {
        return Err(bad(cmd, format!("no body `{body}`")));
    }
    let keep = p.get("keep").and_then(vec3).ok_or_else(|| bad(cmd, "`keep` must be [x, y, z] on the side to keep"))?;
    let (plane, tool) = match p.get("tool").filter(|t| !t.is_null()) {
        Some(t) => {
            let tb = t.get("body").and_then(Value::as_str).ok_or_else(|| bad(cmd, "`tool.body` must name a body"))?.to_string();
            let point = t.get("point").and_then(vec3).ok_or_else(|| bad(cmd, "`tool.point` must be [x, y, z] on the tool face"))?;
            (PlaneRef::Origin { name: "XY".into() }, Some(FaceAt { body: tb, point }))
        }
        None => (plane_param(s, p.get("plane"), cmd)?, None),
    };
    add_feature(s, p, FeatureKind::SurfaceTrim { body, plane, tool, keep })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::json;

    fn run(s: &mut Session, c: &str, p: Value) -> Value {
        s.execute(c, &p).unwrap_or_else(|e| panic!("{c}: {e}"))
    }
    fn volume(s: &Session, name: &str) -> f64 {
        let st = s.world_state();
        solidcraft_kernel::measure(&st.body(name).unwrap().body).unwrap().volume
    }

    /// The planned surface commands are registered but refuse to run: disabled with the
    /// issue that tracks them, an error (never a panic) from every caller, and the design left
    /// as it was.
    #[test]
    fn planned_commands_refuse_and_leave_the_design_alone() {
        let planned = [
            "surface.extrude",
            "surface.revolve",
            "surface.sweep",
            "surface.loft",
            "surface.offset",
            "surface.ruled",
            "surface.boundary",
            "surface.blend",
            "surface.untrim",
            "surface.unstitch",
            "surface.reverse_normal",
            "surface.fillet",
            "inspect.continuity",
        ];
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "B"}));
        let before = s.doc.features.len();
        for id in planned {
            let spec = crate::find_command(id).unwrap_or_else(|| panic!("{id} is not registered"));
            let info = spec.info(&s);
            assert!(!info.enabled, "{id} must stay disabled until it is built");
            let why = info.disabled_reason.unwrap_or_default();
            assert!(why.starts_with("not supported yet (issue #"), "{id}: {why}");
            assert!(!spec.icon.is_empty() && !spec.params.is_empty(), "{id}: icon and params doc");
            assert!(s.execute(id, &json!({"body": "B"})).is_err(), "{id} ran");
            let e = (spec.run)(&mut s, &json!({})).unwrap_err().to_string();
            assert!(e.starts_with("not supported yet: ") && e.contains("(issue #"), "{id}: {e}");
        }
        assert_eq!(s.doc.features.len(), before, "a planned command changed the design");
    }

    /// A rectangle patched, trimmed by a plane and thickened; a box's open top patched; a rim
    /// that is not planar patched.
    #[test]
    fn patch_trim_thicken_and_stitch() {
        let mut s = Session::default();
        run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
        run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [10, 10]}));
        run(&mut s, "sketch.finish", json!({}));
        run(&mut s, "surface.patch", json!({"sketch": "S", "body_name": "Sheet"}));
        let st = s.world_state();
        assert!(st.body("Sheet").unwrap().body.is_surface());
        run(&mut s, "surface.trim", json!({"body": "Sheet", "plane": {"origin": [3, 0, 0], "normal": [1, 0, 0]}, "keep": [8, 5, 0]}));
        run(&mut s, "surface.extend", json!({"body": "Sheet", "edges": [[10, 5, 0]], "distance": 5}));
        run(&mut s, "surface.thicken", json!({"bodies": ["Sheet"], "thickness": "2 mm", "body_name": "Slab"}));
        let v = volume(&s, "Slab");
        assert!((v - 240.0).abs() < 1e-9, "{v}");
        // Stitch the patch of a box's open top back on.
        run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [20, 0, 0], "body_name": "Cup"}));
        run(&mut s, "surface.patch", json!({"edges": [[25, 0, 10], [30, 5, 10], [25, 10, 10], [20, 5, 10]], "body": "Cup", "body_name": "Lid"}));
        assert!(s.world_state().body("Lid").unwrap().body.is_surface());
        // A rim that is not planar: a box with its top cut away by a sideways cylinder.
        run(&mut s, "solid.box", json!({"length": 28, "width": 28, "height": 20, "corner": [-14, -14, 0], "body_name": "Saddle"}));
        run(
            &mut s,
            "solid.cylinder",
            json!({"radius": 40, "height": 120, "base": [0, -60, 50], "axis": "Y", "operation": "cut", "targets": ["Saddle"]}),
        );
        let z = |x: f64| 50.0 - (1600.0 - x * x).sqrt();
        let rim = json!([[14, 0, z(14.0)], [0, 14, z(0.0)], [-14, 0, z(-14.0)], [0, -14, z(0.0)]]);
        run(&mut s, "surface.patch", json!({"edges": rim, "body": "Saddle", "body_name": "Cover"}));
        let st = s.world_state();
        let cover = &st.body("Cover").unwrap().body;
        assert!(cover.is_surface() && cover.face_count() == 1);
        // The cover is a smooth surface on the rim, close to the cylinder that cut it (its area
        // within 1% of the cylinder's inside the rim).
        let area = solidcraft_kernel::measure(cover).unwrap().area;
        let want = 28.0 * 40.0 * 2.0 * (14.0f64 / 40.0).asin();
        assert!((area - want).abs() < want * 1e-2, "{area} vs {want}");
        assert!(s.execute("surface.thicken", &json!({"bodies": ["Nope"], "thickness": 1})).is_err());
        assert!(s.execute("surface.trim", &json!({"body": "Sheet"})).is_err());
    }
}
