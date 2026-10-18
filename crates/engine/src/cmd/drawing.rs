//! Drawing sheets: orthographic views of the model, notes, and an SVG export.

use serde_json::{Value, json};
use solidcraft_doc::{DrawingNote, DrawingSheet, DrawingView};
use solidcraft_geom::Vec3;

use super::CommandSpec;
use crate::params::{bad, str_, vec3};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("drawing.create", "New Drawing", create).at("DRAWING", "CREATE").icon("sketch").params("name?"),
    CommandSpec::new("drawing.base_view", "Base View", base_view)
        .at("DRAWING", "CREATE")
        .icon("sketch")
        .params("sheet?: name (default: the latest); direction?: [x,y,z] (default looking down); origin?: [x,y] on the sheet"),
    CommandSpec::new("drawing.project", "Projected View", project)
        .at("DRAWING", "CREATE")
        .icon("sketch")
        .params("sheet?; direction: [x,y,z]; origin?: [x,y]"),
    CommandSpec::new("drawing.section", "Section View", section)
        .at("DRAWING", "CREATE")
        .icon("sketch")
        .params("sheet?; direction: [x,y,z] (the view looks along this, and the cut is through the model centre); origin?: [x,y]"),
    CommandSpec::new("drawing.dimension", "Drawing Dimension", dimension)
        .at("DRAWING", "MODIFY")
        .icon("dimension")
        .params("sheet?; text: string; at?: [x,y]"),
    CommandSpec::new("drawing.export_svg", "Export Drawing", export_svg).at("DRAWING", "CREATE").icon("sketch").noundo().params("sheet?"),
    CommandSpec::new("drawing.break_view", "Break View", break_view)
        .at("DRAWING", "CREATE")
        .icon("sketch")
        .params("sheet?; stations: [a, b] along the view; origin?: [x,y]. The reported length stays the true model length"),
    CommandSpec::new("drawing.auxiliary", "Auxiliary View", auxiliary)
        .at("DRAWING", "CREATE")
        .icon("sketch")
        .params("sheet?; direction: [x,y,z] of the edge; origin?: [x,y]"),
    CommandSpec::new("drawing.center_mark", "Center Mark", center_mark).at("DRAWING", "MODIFY").icon("dimension").params("sheet?; at: [x,y]; text?"),
    CommandSpec::new("drawing.centerline", "Centerline", centerline).at("DRAWING", "MODIFY").icon("dimension").params("sheet?; at: [x,y]; text?"),
    CommandSpec::new("drawing.note", "Note", note_kind).at("DRAWING", "MODIFY").icon("dimension").params("sheet?; text; at?: [x,y]"),
    CommandSpec::new("drawing.leader", "Leader", leader).at("DRAWING", "MODIFY").icon("dimension").params("sheet?; text; at?: [x,y]"),
    CommandSpec::new("drawing.balloon", "Balloon", balloon).at("DRAWING", "MODIFY").icon("dimension").params("sheet?; component: id; at?: [x,y]"),
    CommandSpec::new("drawing.standard", "Drawing Standard", standard)
        .at("DRAWING", "MODIFY")
        .icon("sketch")
        .params("sheet?; standard: first_angle|third_angle"),
    CommandSpec::new("drawing.export_pdf", "Export Drawing PDF", export_pdf)
        .noundo()
        .params("sheet?; path: a PDF file. The page draws the view rectangle"),
];

pub(super) fn sheet_index(s: &Session, p: &Value) -> Result<usize> {
    let name = str_(p, "sheet");
    let i = s.doc.drawings.iter().position(|d| name.is_none_or(|n| d.name == n)).ok_or_else(|| bad("drawing", "no drawing sheet"))?;
    // The latest sheet when no name is given: position() finds the first. Prefer the last match.
    if name.is_none() {
        return s.doc.drawings.len().checked_sub(1).ok_or_else(|| bad("drawing", "no drawing sheet"));
    }
    Ok(i)
}

fn origin_of(p: &Value) -> [f64; 2] {
    p.get("origin")
        .and_then(Value::as_array)
        .and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]))
        .filter(|v| v.iter().all(|x| x.is_finite()))
        .unwrap_or([0.0, 0.0])
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_(p, "name").unwrap_or("Sheet").to_string();
    if s.doc.drawings.len() >= 100 {
        return Err(bad("drawing.create", "too many drawing sheets"));
    }
    s.doc_mut().drawings.push(DrawingSheet { name: name.clone(), views: Vec::new(), notes: Vec::new(), svg: String::new(), standard: String::new() });
    s.refresh();
    Ok(json!({"sheet": name}))
}

fn push_view(s: &mut Session, p: &Value, kind: &str) -> Result<Value> {
    let cmd = "drawing";
    let direction = p.get("direction").and_then(vec3).unwrap_or(Vec3::Z);
    let direction = direction.normalized().ok_or_else(|| bad(cmd, "`direction` must be non-zero"))?;
    let i = sheet_index(s, p)?;
    let scale = p
        .get("scale")
        .map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string()))
        .filter(|s| !s.is_empty() && s != "null")
        .unwrap_or_default();
    if let Some(n) = p.get("scale").and_then(Value::as_f64)
        && !(n.is_finite() && n > 0.0)
    {
        return Err(bad(cmd, "scale must be positive"));
    }
    let hidden = crate::params::bool_(p, "hidden").unwrap_or(false);
    let bodies = s.model.state().bodies.iter().map(|b| b.name.clone()).collect();
    let view = DrawingView { kind: kind.into(), direction, origin: origin_of(p), scale, hidden, outline: 0.0, lost: Vec::new(), bodies };
    let Some(sheet) = s.doc_mut().drawings.get_mut(i) else { return Err(bad(cmd, "no drawing sheet")) };
    if sheet.views.len() >= 64 {
        return Err(bad(cmd, "too many views on the sheet"));
    }
    sheet.views.push(view);
    sheet.svg.clear();
    let name = sheet.name.clone();
    s.refresh();
    Ok(json!({"sheet": name, "views": s.doc.drawings.get(i).map(|d| d.views.len()).unwrap_or(0)}))
}

fn base_view(s: &mut Session, p: &Value) -> Result<Value> {
    push_view(s, p, "base")
}

fn project(s: &mut Session, p: &Value) -> Result<Value> {
    if p.get("direction").and_then(vec3).is_none() {
        return Err(bad("drawing.project", "`direction` is required"));
    }
    push_view(s, p, "projected")
}

fn section(s: &mut Session, p: &Value) -> Result<Value> {
    if p.get("direction").and_then(vec3).is_none() {
        return Err(bad("drawing.section", "`direction` is required"));
    }
    push_view(s, p, "section")
}

fn dimension(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "drawing.dimension";
    let text = str_(p, "text").filter(|t| !t.is_empty()).ok_or_else(|| bad(cmd, "`text` is required"))?.to_string();
    if text.len() > 400 {
        return Err(bad(cmd, "dimension text is too long"));
    }
    let i = sheet_index(s, p)?;
    let notes = {
        let Some(sheet) = s.doc_mut().drawings.get_mut(i) else { return Err(bad(cmd, "no drawing sheet")) };
        sheet.notes.push(DrawingNote { text, at: origin_of(p) });
        sheet.svg.clear();
        sheet.notes.len()
    };
    s.refresh();
    Ok(json!({"notes": notes}))
}

/// Projected model edges on one sheet, or a top view when that sheet has no views yet.
pub(super) fn model_segments(s: &Session, sheet: usize) -> Vec<([f64; 2], [f64; 2])> {
    let st = s.model.state();
    let mut specs: Vec<(Vec3, [f64; 2], String)> = Vec::new();
    if let Some(sheet) = s.doc.drawings.get(sheet) {
        for view in &sheet.views {
            specs.push((view.direction, view.origin, view.kind.clone()));
        }
    }
    if specs.is_empty() {
        specs.push((Vec3::new(0.0, 0.0, -1.0), [0.0, 0.0], "base".into()));
    }
    let mut out = Vec::new();
    for (direction, origin, kind) in specs {
        if direction.len() < 1e-9 {
            continue;
        }
        let x = direction.any_perp();
        let y = direction.cross(x);
        for b in &st.bodies {
            if b.body.is_mesh() {
                continue;
            }
            let edges = match b.body.edges(0.2) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for e in edges {
                if kind == "section" && e.mid.dot(direction) < -1e-3 {
                    continue;
                }
                let (Some(p0), Some(p1)) = (e.points.first(), e.points.last()) else { continue };
                let a = [p0.dot(x) + origin[0], p0.dot(y) + origin[1]];
                let b = [p1.dot(x) + origin[0], p1.dot(y) + origin[1]];
                if (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6 {
                    continue;
                }
                out.push((a, b));
                if out.len() >= 8_000 {
                    return out;
                }
            }
        }
    }
    out
}

fn export_svg(s: &mut Session, p: &Value) -> Result<Value> {
    let i = sheet_index(s, p)?;
    let sheet = s.doc.drawings.get(i).cloned().ok_or_else(|| bad("drawing.export_svg", "no drawing sheet"))?;
    let mut lines = Vec::new();
    for (a, b) in model_segments(s, i) {
        lines.push(format!(
            "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke=\"#222\" stroke-width=\"0.2\"/>",
            a[0], -a[1], b[0], -b[1]
        ));
    }
    for note in &sheet.notes {
        let safe: String = note.text.chars().filter(|c| c.is_ascii() && *c != '<' && *c != '>' && *c != '&').take(200).collect();
        lines.push(format!("<text x=\"{:.3}\" y=\"{:.3}\" font-size=\"3\" fill=\"#111\">{}</text>", note.at[0], -note.at[1], safe));
    }
    let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-200 -200 400 400\">{}</svg>", lines.join(""));
    if let Some(slot) = s.doc_mut().drawings.get_mut(i) {
        slot.svg = svg.clone();
    }
    s.refresh();
    Ok(json!({"sheet": sheet.name, "svg": svg, "lines": lines.len()}))
}

/// Recompute each view's projected extent and the bodies it has lost.
pub(super) fn sync(s: &mut Session) {
    if s.doc.drawings.is_empty() {
        return;
    }
    let state = s.model.state();
    let names: Vec<String> = state.bodies.iter().map(|b| b.name.clone()).collect();
    let mut corners = Vec::new();
    for body in &state.bodies {
        let bounds = body.mesh().bounds();
        for x in [bounds.min.x, bounds.max.x] {
            for y in [bounds.min.y, bounds.max.y] {
                for z in [bounds.min.z, bounds.max.z] {
                    corners.push(Vec3::new(x, y, z));
                }
            }
        }
    }
    for sheet in &mut s.doc_mut().drawings {
        for view in &mut sheet.views {
            view.lost = view.bodies.iter().filter(|name| !names.iter().any(|have| have == *name)).cloned().collect();
            let up = {
                let axis = if view.direction.cross(Vec3::X).len() > 1e-6 { Vec3::X } else { Vec3::Y };
                view.direction.cross(axis).normalized().unwrap_or(Vec3::Z)
            };
            let mut span = 0.0;
            if let Some(first) = corners.first() {
                let mut lo = first.dot(up);
                let mut hi = lo;
                for point in &corners {
                    let h = point.dot(up);
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
                span = hi - lo;
            }
            view.outline = span;
        }
    }
}

fn break_view(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "drawing.break_view";
    let stations = p.get("stations").and_then(Value::as_array).ok_or_else(|| bad(cmd, "`stations` must be two positions"))?;
    if stations.len() != 2 || stations.iter().any(|v| !v.as_f64().is_some_and(|n| n.is_finite())) {
        return Err(bad(cmd, "`stations` must be two finite positions"));
    }
    push_view(s, p, "break")
}

fn auxiliary(s: &mut Session, p: &Value) -> Result<Value> {
    if p.get("direction").and_then(vec3).is_none() {
        return Err(bad("drawing.auxiliary", "`direction` is required"));
    }
    push_view(s, p, "auxiliary")
}

fn annotate(s: &mut Session, p: &Value, kind: &str) -> Result<Value> {
    let cmd = "drawing";
    let text = if kind == "balloon" {
        str_(p, "component").filter(|c| !c.is_empty()).ok_or_else(|| bad(cmd, "`component` is required"))?.to_string()
    } else {
        str_(p, "text").unwrap_or(kind).chars().take(400).collect()
    };
    let i = sheet_index(s, p)?;
    let Some(sheet) = s.doc_mut().drawings.get_mut(i) else { return Err(bad(cmd, "no drawing sheet")) };
    sheet.notes.push(DrawingNote { text: format!("{kind}:{text}"), at: origin_of(p) });
    Ok(json!({"kind": kind, "notes": sheet.notes.len()}))
}

fn note_kind(s: &mut Session, p: &Value) -> Result<Value> {
    annotate(s, p, "note")
}

fn center_mark(s: &mut Session, p: &Value) -> Result<Value> {
    annotate(s, p, "center_mark")
}

fn centerline(s: &mut Session, p: &Value) -> Result<Value> {
    annotate(s, p, "centerline")
}

fn leader(s: &mut Session, p: &Value) -> Result<Value> {
    annotate(s, p, "leader")
}

fn balloon(s: &mut Session, p: &Value) -> Result<Value> {
    annotate(s, p, "balloon")
}

fn standard(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "drawing.standard";
    let name = str_(p, "standard").unwrap_or("third_angle");
    if !matches!(name, "first_angle" | "third_angle") {
        return Err(bad(cmd, "standard must be first_angle or third_angle"));
    }
    let i = sheet_index(s, p)?;
    let Some(sheet) = s.doc_mut().drawings.get_mut(i) else { return Err(bad(cmd, "no drawing sheet")) };
    sheet.standard = name.to_string();
    Ok(json!({"standard": name}))
}

fn export_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "drawing.export_pdf";
    let path = str_(p, "path").filter(|n| !n.is_empty()).ok_or_else(|| bad(cmd, "`path` is required"))?;
    let _ = sheet_index(s, p)?;
    sync(s);
    let outline = s.doc.drawings.iter().flat_map(|d| d.views.iter()).map(|v| v.outline).fold(0.0, f64::max);
    let span = outline.max(10.0);
    let stream = format!("10 10 m {span:.2} 10 l {span:.2} 40 l 10 40 l h S\n");
    let pdf = format!(
        "%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Count 1/Kids[3 0 R]>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 400 300]/Contents 4 0 R>>endobj\n4 0 obj<</Length {}>>stream\n{}endstream\nendobj\ntrailer<</Root 1 0 R>>\n%%EOF\n",
        stream.len(),
        stream
    );
    std::fs::write(path, pdf).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(json!({"path": path, "outline": outline}))
}
