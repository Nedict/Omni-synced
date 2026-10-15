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
    CommandSpec::new("drawing.export_svg", "Export Drawing", export_svg)
        .at("DRAWING", "CREATE")
        .icon("sketch")
        .noundo()
        .params("sheet?"),
];

fn sheet_index(s: &Session, p: &Value) -> Result<usize> {
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
    s.doc_mut().drawings.push(DrawingSheet { name: name.clone(), views: Vec::new(), notes: Vec::new(), svg: String::new() });
    s.refresh();
    Ok(json!({"sheet": name}))
}

fn push_view(s: &mut Session, p: &Value, kind: &str) -> Result<Value> {
    let cmd = "drawing";
    let direction = p.get("direction").and_then(vec3).unwrap_or(Vec3::Z);
    let direction = direction.normalized().ok_or_else(|| bad(cmd, "`direction` must be non-zero"))?;
    let i = sheet_index(s, p)?;
    let view = DrawingView { kind: kind.into(), direction, origin: origin_of(p) };
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

fn export_svg(s: &mut Session, p: &Value) -> Result<Value> {
    let i = sheet_index(s, p)?;
    let sheet = s.doc.drawings.get(i).cloned().ok_or_else(|| bad("drawing.export_svg", "no drawing sheet"))?;
    let st = s.model.state();
    let mut lines = Vec::new();
    for view in &sheet.views {
        let x = view.direction.any_perp();
        let y = view.direction.cross(x);
        for b in &st.bodies {
            if b.body.is_mesh() {
                continue;
            }
            let edges = match b.body.edges(0.2) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for e in edges {
                if view.kind == "section" {
                    let mid = e.mid;
                    if (mid).dot(view.direction) < -1e-3 {
                        continue;
                    }
                }
                let project = |q: Vec3| [q.dot(x) + view.origin[0], q.dot(y) + view.origin[1]];
                if let (Some(p0), Some(p1)) = (e.points.first(), e.points.last()) {
                    let a = project(*p0);
                    let b = project(*p1);
                    lines.push(format!(
                        "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke=\"#222\" stroke-width=\"0.2\"/>",
                        a[0], -a[1], b[0], -b[1]
                    ));
                }
            }
        }
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
