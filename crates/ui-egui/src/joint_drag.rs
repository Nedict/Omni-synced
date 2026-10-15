//! Dragging a part that joints place moves it as far as its joints allow: each frame runs
//! `joint.reach`, taking the point grabbed toward the pointer in the plane through it that faces
//! the view. The whole drag is one undo step.

use egui::Pos2;
use serde_json::json;
use solidcraft_engine::doc::{apply_point, joints, mat_inverse};
use solidcraft_engine::geom::Vec3;

use crate::SolveApp;
use crate::viewport::{Hit, Proj};

/// A drag under way.
#[derive(Clone, Debug)]
pub struct Drag {
    occurrence: u64,
    /// The point grabbed, in the occurrence's own coordinates.
    local: Vec3,
    /// The plane the pointer moves the point in: through where it was grabbed, facing the view.
    origin: Vec3,
    normal: Vec3,
    /// The undo history's length when it began.
    depth: usize,
}

/// Begin a drag on the part under `p` when joints place it and leave it free to move (it is
/// neither grounded nor fixed to what is). True when one began.
pub fn start(app: &mut SolveApp, proj: &Proj, p: Pos2) -> bool {
    let Some((body, at)) = crate::viewport::pick(app, proj, p).into_iter().find_map(|h| match h {
        Hit::Face { body, point, .. } => Some((body, point)),
        _ => None,
    }) else {
        return false;
    };
    let s = &app.session;
    let Some(occurrence) = crate::dialogs_assembly::occurrence_of_body(s, &body).filter(|o| *o != 0) else { return false };
    if !s.doc.assembly.joints.iter().any(|j| !j.suppressed && (j.a.occurrence == occurrence || j.b.occurrence == occurrence)) {
        return false;
    }
    let sol = joints::solve(&s.doc, &s.model.state());
    if !sol.world.contains_key(&occurrence) || sol.dof.get(&occurrence).is_none_or(|d| *d == 0) {
        return false;
    }
    let Some(local) = mat_inverse(&joints::occurrence_world(&s.doc, occurrence)).map(|inv| apply_point(&inv, at)) else { return false };
    let normal = proj.ray(p).1;
    app.viewport.joint_drag = Some(Drag { occurrence, local, origin: at, normal, depth: app.session.undo.len() });
    true
}

pub fn active(app: &SolveApp) -> bool {
    app.viewport.joint_drag.is_some()
}

/// Reach for the point under `p`.
pub fn to(app: &mut SolveApp, proj: &Proj, p: Pos2) {
    let Some(d) = app.viewport.joint_drag.clone() else { return };
    let (o, dir) = proj.ray(p);
    let facing = dir.dot(d.normal);
    if facing.abs() < 1e-9 {
        return;
    }
    let target = o + dir * ((d.origin - o).dot(d.normal) / facing);
    if !target.is_finite() {
        return;
    }
    let at = apply_point(&joints::occurrence_world(&app.session.doc, d.occurrence), d.local);
    let params =
        json!({"occurrence": d.occurrence, "at": [at.x, at.y, at.z], "point": [target.x, target.y, target.z], "quiet": true, "continuous": true});
    if app.run("joint.reach", params).is_ok() {
        // One undo step for the whole drag.
        app.session.undo.truncate(d.depth + 1);
    }
}

pub fn end(app: &mut SolveApp) {
    app.viewport.joint_drag = None;
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use solidcraft_engine::doc::{apply_point, joints};
    use solidcraft_engine::geom::Vec3;

    use crate::scenario::Harness;

    fn run(h: &mut Harness, id: &str, p: Value) -> Value {
        h.app.run(id, p.clone()).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
    }

    fn screen(h: &mut Harness, p: Vec3) -> egui::Pos2 {
        let [x, y] = h.at(&json!({"world": [p.x, p.y, p.z]})).expect("on screen");
        egui::pos2(x as f32, y as f32)
    }

    /// Where the arm's point `local` (its own coordinates) is.
    fn arm_at(h: &Harness, local: Vec3) -> Vec3 {
        let s = &h.app.session;
        let c = s.doc.find_component("Arm").unwrap_or(0);
        let o = s.doc.occurrence_of(c).map(|o| o.id).unwrap_or(0);
        apply_point(&joints::occurrence_world(&s.doc, o), local)
    }

    /// An arm hinged about +Z on a grounded base, dragged by its far end a quarter turn round
    /// (seen from the top): it turns there in one undo step, and undo turns it back.
    #[test]
    fn dragging_a_jointed_part_turns_it_about_its_hinge() {
        let mut h = Harness::new();
        h.app.home.open = false;
        for (name, corner, size) in [("Base", [-5, -5, 0], [10, 10, 5]), ("Arm", [-3, -3, 5], [63, 6, 5])] {
            run(&mut h, "component.activate", json!({"component": "root"}));
            run(&mut h, "component.create", json!({"name": name}));
            run(&mut h, "solid.box", json!({"length": size[0], "width": size[1], "height": size[2], "corner": corner}));
        }
        run(&mut h, "component.activate", json!({"component": "root"}));
        let base = h.app.session.doc.find_component("Base").and_then(|c| h.app.session.doc.occurrence_of(c)).map(|o| o.id).unwrap_or(0);
        run(&mut h, "occurrence.ground", json!({"occurrence": base, "grounded": true}));
        let at = |o: &str| json!({"occurrence": o, "point": [0, 0, 5], "z": [0, 0, 1]});
        run(&mut h, "joint.create", json!({"type": "revolute", "a": at("Base"), "b": at("Arm"), "flip": true, "name": "Hinge"}));
        h.call("ui.view", json!({"view": "top"}));
        h.app.cam.distance = 200.0;
        h.frames(3);
        let undo = h.app.session.undo.len();
        let end = Vec3::new(55.0, 0.0, 10.0);
        let (a, b) = (screen(&mut h, end), screen(&mut h, Vec3::new(0.0, 55.0, 10.0)));
        h.app.synthetic.push(egui::Event::PointerMoved(a));
        h.app.synthetic.push(egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        h.app.synthetic.extend((1..=12).map(|k| egui::Event::PointerMoved(a + (b - a) * (k as f32 / 12.0))));
        while !h.app.synthetic.is_empty() {
            h.frame();
        }
        h.frames(2);
        assert!(h.app.viewport.joint_drag.is_some() && h.app.viewport.boxsel.is_none(), "the drag moves the arm, not a selection box");
        h.app.synthetic.push(egui::Event::PointerButton {
            pos: b,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        while !h.app.synthetic.is_empty() {
            h.frame();
        }
        h.frames(2);
        assert!(h.app.viewport.joint_drag.is_none());
        assert!(arm_at(&h, end).dist(Vec3::new(0.0, 55.0, 10.0)) < 0.5, "{:?}", arm_at(&h, end));
        assert_eq!(h.app.session.undo.len(), undo + 1, "one undo step for the drag");
        assert!(!h.app.session.log.iter().any(|l| l.contains("Reach")), "frames are not echoed: {:?}", h.app.session.log);
        run(&mut h, "edit.undo", json!({}));
        assert!(arm_at(&h, end).dist(end) < 1e-9, "{:?}", arm_at(&h, end));
    }
}
