//! Sketch Move/Copy: a 2D version of the body Move window (Free, Translate, Rotate) on the
//! sketch plane, with the same icon row and a planar triad on the canvas.

use serde_json::json;
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;
use solidcraft_engine::geom::{Vec2, Vec3};

use crate::SolveApp;
use crate::dialogs::{Dialog, Kind, field, icon_choice, row_label};
use crate::gizmo::{self, Change, Drag};
use crate::sketch_modify_ui::Mod;

pub const TYPES: [&str; 3] = ["Free Move", "Translate", "Rotate"];
const ICONS: [&str; 3] = ["move_free", "move_translate", "move_rotate"];
pub const FREE: usize = 0;
pub const TRANSLATE: usize = 1;
pub const ROTATE: usize = 2;

pub fn kind(mode: usize) -> Mod {
    Mod::Move { mode, x: "0 mm".into(), y: "0 mm".into(), angle: "0 deg".into(), copy: false }
}

pub fn type_row(d: &mut Dialog, ui: &mut egui::Ui) {
    let Kind::SketchModify(Mod::Move { mode, .. }) = d.kind else { return };
    row_label(ui, "Move Type");
    let mut m = mode;
    icon_choice(ui, &TYPES, &ICONS, &mut m);
    ui.end_row();
    if m != mode {
        let entities = d.inputs.first().map(|i| i.items.clone()).unwrap_or_default();
        let center = d.inputs.get(1).map(|i| i.items.clone()).unwrap_or_default();
        d.kind = Kind::SketchModify(kind(m));
        if let Some(i) = d.inputs.first_mut() {
            i.items = entities;
        }
        if let Some(i) = d.inputs.get_mut(1) {
            i.items = center;
        }
        d.extra.remove("drag0");
    }
}

fn plane_axes(app: &SolveApp) -> Option<(Vec3, Vec3, Vec3)> {
    let world = app.session.world_state();
    let ss = world.sketch(app.session.active_sketch?)?;
    let o = ss.plane.origin;
    let (u, v) = (ss.plane.x.normalized()?, ss.plane.y.normalized()?);
    Some((o, u, v))
}

fn center_world(app: &SolveApp, d: &Dialog) -> Option<Vec3> {
    if let Some(Sel::SketchPoint { id }) = d.inputs.get(1).and_then(|i| i.items.first()) {
        let sid = app.session.active_sketch?;
        let world = app.session.world_state();
        let ss = world.sketch(sid)?;
        let p = ss.sketch.point(ss.sketch.resolve_point(id)?)?;
        return Some(ss.plane.to_world(p));
    }
    let sid = app.session.active_sketch?;
    let world = app.session.world_state();
    let ss = world.sketch(sid)?;
    let mut bb: Option<(Vec2, Vec2)> = None;
    for sel in d.inputs.first()?.items.iter() {
        let p = match sel {
            Sel::SketchCurve { id } => {
                let idx = ss.sketch.curve_index(id)?;
                let pts = ss.sketch.polyline(idx);
                if pts.is_empty() {
                    continue;
                }
                pts.iter().fold(Vec2::ZERO, |s, q| s + *q) * (1.0 / pts.len() as f64)
            }
            Sel::SketchPoint { id } => ss.sketch.point(ss.sketch.resolve_point(id)?)?,
            _ => continue,
        };
        bb = Some(match bb {
            None => (p, p),
            Some((lo, hi)) => (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y))),
        });
    }
    Some(match bb {
        Some((lo, hi)) => ss.plane.to_world((lo + hi) * 0.5),
        None => ss.plane.to_world(Vec2::ZERO),
    })
}

fn len(app: &SolveApp, s: &str) -> Result<f64, String> {
    app.session.doc.eval(s, ValueKind::Length).map_err(|e| e.to_string()).and_then(|v| if v.is_finite() { Ok(v) } else { Err("not a number".into()) })
}

fn ang(app: &SolveApp, s: &str) -> Result<f64, String> {
    app.session.doc.eval(s, ValueKind::Angle).map_err(|e| e.to_string()).and_then(|v| if v.is_finite() { Ok(v) } else { Err("not a number".into()) })
}

fn mm(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{} mm", if s == "-0" || s.is_empty() { "0" } else { s })
}

fn deg(v: f64) -> String {
    let s = format!("{:.4}", v.to_degrees());
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{} deg", if s == "-0" || s.is_empty() { "0" } else { s })
}

pub fn triad(app: &SolveApp, d: &Dialog) -> Option<gizmo::Triad> {
    let Kind::SketchModify(Mod::Move { mode, x, y, .. }) = &d.kind else { return None };
    let (_, u, v) = plane_axes(app)?;
    let mut c = center_world(app, d)?;
    if matches!(*mode, FREE | TRANSLATE) {
        let t = Vec2::new(len(app, x).unwrap_or(0.0), len(app, y).unwrap_or(0.0));
        let world = app.session.world_state();
        let ss = world.sketch(app.session.active_sketch?)?;
        c = ss.plane.to_world(ss.plane.to_local(c) + t);
    }
    let n = u.cross(v).normalized().unwrap_or(u.cross(Vec3::Z).normalized().unwrap_or(Vec3::Z));
    Some(gizmo::Triad {
        center: c,
        translate: matches!(*mode, FREE | TRANSLATE),
        rotate: matches!(*mode, FREE | ROTATE),
        ring_axis: if matches!(*mode, FREE | ROTATE) { Some(n) } else { None },
        translate_u: if matches!(*mode, FREE | TRANSLATE) { Some(u) } else { None },
        translate_v: if matches!(*mode, FREE | TRANSLATE) { Some(v) } else { None },
    })
}

pub fn drag(app: &SolveApp, d: &mut Dialog, g: Drag) {
    let Kind::SketchModify(Mod::Move { mode, x, y, angle, .. }) = &d.kind else { return };
    if g.started || !d.extra.contains_key("drag0") {
        let v: Vec<f64> = match *mode {
            ROTATE => vec![ang(app, angle).unwrap_or(0.0)],
            _ => vec![len(app, x).unwrap_or(0.0), len(app, y).unwrap_or(0.0), ang(app, angle).unwrap_or(0.0)],
        };
        d.extra.insert("drag0".into(), json!(v));
    }
    let v0: Vec<f64> = d.extra.get("drag0").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let Kind::SketchModify(Mod::Move { mode, x, y, angle, .. }) = &mut d.kind else { return };
    match (*mode, g.change) {
        (ROTATE, Change::RotateAbout { angle: th, .. }) | (ROTATE, Change::Rotate { angle: th, .. }) => {
            *angle = deg(v0.first().copied().unwrap_or(0.0) + th);
        }
        (FREE, Change::RotateAbout { angle: th, .. }) | (FREE, Change::Rotate { angle: th, .. }) => {
            *angle = deg(v0.get(2).copied().unwrap_or(0.0) + th);
        }
        (FREE | TRANSLATE, Change::Translate(t)) => {
            if let Some((_, u, v)) = plane_axes(app) {
                *x = mm(v0.first().copied().unwrap_or(0.0) + t.dot(u));
                *y = mm(v0.get(1).copied().unwrap_or(0.0) + t.dot(v));
            }
        }
        _ => {}
    }
    if g.done {
        d.extra.remove("drag0");
    }
}

pub fn value_rows(ui: &mut egui::Ui, mode: usize, x: &mut String, y: &mut String, angle: &mut String, copy: &mut bool) -> bool {
    let mut enter = false;
    let mut row = |ui: &mut egui::Ui, l: &str, v: &mut String| {
        row_label(ui, l);
        enter |= field(ui, v);
        ui.end_row();
    };
    match mode {
        FREE | TRANSLATE => {
            row(ui, "X Distance", x);
            row(ui, "Y Distance", y);
            if mode == FREE {
                row(ui, "Angle", angle);
            }
        }
        ROTATE => row(ui, "Angle", angle),
        _ => {}
    }
    row_label(ui, "Create Copy");
    crate::dialogs::check(ui, copy);
    ui.end_row();
    enter
}
