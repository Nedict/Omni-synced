//! A single outside contour: offset a sketch profile by the tool radius and emit G-code text.

use serde_json::{Value, json};
use solidcraft_doc::CamJob;
use solidcraft_geom::Vec2;

use super::CommandSpec;
use crate::params::{bad, str_};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("cam.contour", "2D Contour", contour)
    .at("UTILITIES", "MAKE")
    .icon("extrude")
    .params("sketch: name; tool_diameter: mm; depth: mm (positive, cut downward); name?")];

fn contour(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "cam.contour";
    let sketch = str_(p, "sketch").filter(|n| !n.is_empty()).ok_or_else(|| bad(cmd, "`sketch` is required"))?.to_string();
    let tool = p.get("tool_diameter").and_then(Value::as_f64).ok_or_else(|| bad(cmd, "`tool_diameter` must be a positive number of millimetres"))?;
    let depth = p.get("depth").and_then(Value::as_f64).ok_or_else(|| bad(cmd, "`depth` must be a positive number of millimetres"))?;
    if !(tool.is_finite() && depth.is_finite() && tool > 0.0 && depth > 0.0 && tool < 1e5 && depth < 1e5) {
        return Err(bad(cmd, "tool diameter and depth must be positive"));
    }
    let st = s.model.state();
    let ss = st.sketches.iter().find(|k| k.name == sketch).ok_or_else(|| bad(cmd, format!("no sketch `{sketch}`")))?;
    let loop_pts = ss
        .profiles
        .first()
        .map(|r| r.region.outer.polyline(0.05))
        .filter(|p| p.len() >= 3)
        .ok_or_else(|| bad(cmd, "the sketch needs one closed profile"))?;
    let c = loop_pts.iter().fold(Vec2::ZERO, |a, q| a + *q) / loop_pts.len() as f64;
    let radius = tool * 0.5;
    let path: Vec<Vec2> = loop_pts
        .iter()
        .map(|q| {
            let d = (*q - c).normalized().unwrap_or(Vec2::X);
            *q + d * radius
        })
        .collect();
    let world = |q: Vec2| ss.plane.to_world(q);
    let mut g = String::from("G21\nG90\nG0 Z5.000\n");
    if let Some(first) = path.first() {
        let w = world(*first);
        g.push_str(&format!("G0 X{:.3} Y{:.3}\nG1 Z-{:.3} F200\n", w.x, w.y, depth));
    }
    for q in path.iter().chain(path.first()) {
        let w = world(*q);
        g.push_str(&format!("G1 X{:.3} Y{:.3}\n", w.x, w.y));
    }
    g.push_str("G0 Z5.000\n");
    if s.doc.cam.len() >= 100 {
        return Err(bad(cmd, "too many toolpaths"));
    }
    let name = str_(p, "name").unwrap_or("Contour").to_string();
    s.doc_mut().cam.push(CamJob { name: name.clone(), sketch, tool_diameter: tool, depth, gcode: g.clone(), kind: "contour".into(), area: 0.0 });
    s.refresh();
    Ok(json!({"name": name, "gcode": g}))
}
