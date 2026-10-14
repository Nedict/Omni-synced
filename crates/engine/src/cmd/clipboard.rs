//! Copy and paste of timeline features. Pasted features are independent copies: sketches get
//! their own dimension parameters, feature inputs their own names, and references inside the
//! copied set point at the copies.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solidcraft_doc::{FeatureKind, PlaneRef};

use super::CommandSpec;
use crate::params::{bad, string_list, vec3};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("timeline.copy", "Copy Features", copy).noundo().params("features: [ids or names] (in timeline order on paste)"),
    CommandSpec::new("timeline.paste", "Paste Features", paste)
        .params("translate?: [x,y,z] (moves the copies' geometry); the copies go at the marker"),
];

fn copy(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.copy";
    let mut keys = string_list(p, "features");
    if let Some(Value::Array(a)) = p.get("features") {
        keys.extend(a.iter().filter_map(Value::as_u64).map(|n| n.to_string()));
    }
    if keys.is_empty() || keys.len() > 1000 {
        return Err(bad(cmd, "`features` must list 1…1000 features"));
    }
    let mut ids = Vec::new();
    for k in &keys {
        ids.push(s.doc.find_feature(k).map(|f| f.id).ok_or_else(|| bad(cmd, format!("no feature `{k}`")))?);
    }
    // Keep timeline order so sketches come before the features that use them.
    let copied: Vec<solidcraft_doc::Feature> = s.doc.features.iter().filter(|f| ids.contains(&f.id)).cloned().collect();
    let names: Vec<String> = copied.iter().map(|f| f.name.clone()).collect();
    s.clipboard = copied;
    Ok(json!({"copied": names}))
}

/// Origin planes become explicit planes so a translation can move them.
fn explicit_planes(p: &mut PlaneRef) {
    match p {
        PlaneRef::Origin { name } => {
            if let Some(plane) = solidcraft_geom::Plane::named(name) {
                *p = PlaneRef::Custom { plane };
            }
        }
        PlaneRef::Offset { base, .. } | PlaneRef::AtAngle { base, .. } => explicit_planes(base),
        _ => {}
    }
}

fn paste(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.paste";
    if s.clipboard.is_empty() {
        return Err(bad(cmd, "nothing copied (timeline.copy first)"));
    }
    let translate = match p.get("translate") {
        Some(v) => Some(vec3(v).ok_or_else(|| bad(cmd, "`translate` must be [x, y, z]"))?),
        None => None,
    };
    let feats = s.clipboard.clone();
    let comp = s.active_component;
    let mut sketches: BTreeMap<u64, u64> = BTreeMap::new();
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();
    let mut used: std::collections::BTreeSet<String> = s.model.state().bodies.iter().map(|body| body.name.clone()).collect();
    let mut notices = Vec::new();
    let author_path = s.active_occurrence_path.clone().unwrap_or_else(|| s.doc.first_path(comp));
    solidcraft_doc::EmbossFace::component_frame(&s.placed_document(), comp, &author_path)?;
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut made = Vec::new();
    for f in feats {
        let mut kind = f.kind.clone();
        kind.remap_refs(&sketches, &names);
        // A body made by an earlier copy is the one this copy acts on (#613).
        kind.remap_bodies(&bodies);
        if let FeatureKind::Sketch { plane, .. } = &mut kind
            && let Some(anchor) = super::sketch::face::anchor_mut(plane)
        {
            let internal = bodies.contains_key(&anchor.target.body);
            if !internal && translate.is_some_and(|translate| translate.len() > 1e-12) {
                return Err(bad(cmd, "translated paste retains an external exact face attachment; explicitly convert it to a fixed plane first"));
            }
            if !internal {
                notices.push(format!("pasted sketch retains external exact face attachment to {}", anchor.target.body));
            }
            anchor.remap_target(&sketches, &bodies)?;
            if internal {
                anchor.component = comp;
                anchor.path = author_path.clone();
            }
            anchor.author_path = author_path.clone();
            let target = anchor.target.clone();
            if let FeatureKind::Sketch { sketch, .. } = &mut kind {
                for link in &mut sketch.links {
                    if link.auto_face {
                        if let solidcraft_sketch::LinkSource::Face { body, .. } = &mut link.source
                            && let Some(mapped) = bodies.get(body)
                        {
                            *body = mapped.clone();
                        }
                        link.face_name = Some(target.name.clone());
                    }
                }
            }
        }
        let outputs = if f.body_names.is_empty() {
            s.model
                .result(f.id)
                .map(|result| result.state.bodies.iter().filter(|body| body.feature == f.id).map(|body| body.name.clone()).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            f.body_names.clone()
        };
        let copied_names: Vec<_> = outputs
            .iter()
            .map(|original| {
                let base = format!("{original} (copy)");
                let mut name = base.clone();
                let mut suffix = 2usize;
                while !used.insert(name.clone()) {
                    name = format!("{base} {suffix}");
                    suffix += 1;
                }
                bodies.insert(original.clone(), name.clone());
                name
            })
            .collect();
        let doc = s.doc_mut();
        if let FeatureKind::Sketch { sketch, .. } = &mut kind {
            // Each dimension of the copy gets its own parameter, with the same expression.
            for c in &mut sketch.constraints {
                if let Some(pn) = c.param.clone() {
                    let (e, u) = doc.param(&pn).map(|x| (x.expr.clone(), x.unit.clone())).unwrap_or_else(|| (pn.clone(), "mm".into()));
                    c.param = Some(doc.new_model_param(&e, &u));
                }
            }
        }
        if let Some(t) = translate {
            if let FeatureKind::Sketch { plane, .. } | FeatureKind::ConstructionPlane { plane } = &mut kind {
                explicit_planes(plane);
            }
            let mut m = solidcraft_doc::IDENTITY;
            m[3][0] = t.x;
            m[3][1] = t.y;
            m[3][2] = t.z;
            kind.transform_geometry(&m);
        }
        let id = doc.add_feature(kind, None)?;
        if let Some(nf) = doc.feature_mut(id) {
            nf.component = comp;
            nf.body_names = copied_names;
        }
        let new_name = doc.feature(id).map(|x| x.name.clone()).unwrap_or_default();
        sketches.insert(f.id, id);
        names.insert(f.name.clone(), new_name.clone());
        made.push(json!({"id": id, "name": new_name, "from": f.name}));
    }
    s.refresh();
    let errors: Vec<Value> = made
        .iter()
        .filter_map(|m| m["id"].as_u64())
        .filter_map(|id| s.model.result(id).and_then(|r| r.error.clone()).map(|e| json!({"feature": id, "error": e})))
        .collect();
    Ok(json!({"pasted": made, "errors": errors, "notices":notices}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn run(s: &mut Session, id: &str, p: serde_json::Value) -> serde_json::Value {
        match s.execute(id, &p) {
            Ok(v) => v,
            Err(e) => panic!("{id} {p}: {e}"),
        }
    }

    fn volume(s: &mut Session) -> f64 {
        run(s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap_or(f64::NAN)
    }

    #[test]
    fn copy_paste_sketch_and_extrude() {
        let mut s = Session::default();
        run(&mut s, "parameters.change", json!({"name": "w", "expression": "10 mm"}));
        run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
        run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [10, 10]}));
        run(&mut s, "sketch.constraint.coincident", json!({"a": "p1", "b": "origin"}));
        run(&mut s, "sketch.dimension", json!({"entities": ["l1"], "value": "w"}));
        run(&mut s, "sketch.dimension", json!({"entities": ["l2"], "value": 10}));
        run(&mut s, "sketch.finish", json!({}));
        run(&mut s, "solid.extrude", json!({"sketch": "S", "distance": 5}));
        assert!((volume(&mut s) - 500.0).abs() < 1e-6);
        run(&mut s, "timeline.copy", json!({"features": ["S", "Extrude1"]}));
        let r = run(&mut s, "timeline.paste", json!({"translate": [30, 0, 0]}));
        assert_eq!(r["errors"].as_array().map(Vec::len), Some(0), "{r}");
        assert_eq!(run(&mut s, "inspect.measure", json!({}))["body_count"], 2);
        assert!((volume(&mut s) - 1000.0).abs() < 1e-6);
        // The copy's dimensions are its own, still driven by the user parameter.
        run(&mut s, "parameters.change", json!({"name": "w", "expression": "20 mm"}));
        assert!((volume(&mut s) - 2000.0).abs() < 1e-6);
        let pasted_extrude = r["pasted"][1]["name"].as_str().unwrap_or_default().to_string();
        let d = s.doc.find_feature(&pasted_extrude).and_then(|f| f.param_names.first().cloned()).unwrap_or_default();
        run(&mut s, "parameters.change", json!({"name": d, "expression": "10"}));
        assert!((volume(&mut s) - 3000.0).abs() < 1e-6, "only the copy got thicker");
        assert!(s.execute("timeline.paste", &json!({"translate": "x"})).is_err());
        let mut t = Session::default();
        assert!(t.execute("timeline.paste", &json!({})).is_err());
    }

    #[test]
    fn paste_at_a_marker_inside_a_timeline_folder_joins_the_folder() {
        let mut s = Session::default();
        for (name, body, x) in [("First", "FirstBody", 0), ("Second", "SecondBody", 20), ("Source", "SourceBody", 40)] {
            run(
                &mut s,
                "solid.box",
                json!({"length": 10, "width": 10, "height": 10, "corner": [x, 0, 0], "operation": "new_body", "name": name, "body_name": body}),
            );
        }
        let group = run(&mut s, "timeline.group", json!({"name": "Pair", "features": ["First", "Second"]}))["group"].as_u64().unwrap();
        run(&mut s, "timeline.copy", json!({"features": ["Source"]}));
        run(&mut s, "timeline.roll_to", json!({"position": 1}));

        let pasted = run(&mut s, "timeline.paste", json!({}))["pasted"][0]["id"].as_u64().unwrap();
        let folder = s.doc.timeline_groups.iter().find(|folder| folder.id == group).unwrap();
        let first = s.doc.find_feature("First").unwrap().id;
        let second = s.doc.find_feature("Second").unwrap().id;
        assert_eq!(folder.features, vec![first, pasted, second]);
        assert!(solidcraft_doc::Document::from_json(&s.doc.to_json()).is_ok(), "pasting inside a folder must leave a reopenable design");
    }

    #[test]
    fn pasted_fillet_rounds_the_copied_body_it_names() {
        let mut s = Session::default();
        run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
        run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [40, 20]}));
        run(&mut s, "sketch.finish", json!({}));
        run(&mut s, "solid.extrude", json!({"sketch": "S", "distance": 10, "operation": "new", "body_name": "Block", "name": "Block"}));
        run(&mut s, "solid.fillet", json!({"body": "Block", "radius": 2, "edges": [[20, 0, 10], [20, 20, 10]], "name": "Round"}));
        let one = volume(&mut s);
        assert!(one < 8000.0 - 1.0, "the fillet removes material: {one}");
        run(&mut s, "timeline.copy", json!({"features": ["S", "Block", "Round"]}));
        let r = run(&mut s, "timeline.paste", json!({"translate": [0, 40, 0]}));
        assert_eq!(r["errors"].as_array().map(Vec::len), Some(0), "{r}");
        let round = r["pasted"][2]["name"].as_str().unwrap_or_default().to_string();
        let def = run(&mut s, "timeline.inspect", json!({"feature": round}));
        assert_eq!(def["definition"]["body"], "Block (copy)", "{def}");
        let m = run(&mut s, "inspect.measure", json!({}));
        assert_eq!(m["body_count"], 2, "{m}");
        assert!((volume(&mut s) - 2.0 * one).abs() < 1e-6, "both blocks are rounded: {m}");
    }

    /// One body's measures (bounding box, volume).
    fn measures(s: &mut Session, body: &str) -> serde_json::Value {
        run(s, "inspect.measure", json!({"bodies": [body]}))["bodies"][0].clone()
    }

    /// Pasting with a translate moves an extrude's `to` and `from` picks with it: the pasted posts
    /// run to the pasted roof and from the pasted shelf, not to and from the originals.
    #[test]
    fn a_pasted_extrude_runs_to_and_from_the_pasted_faces() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 2, "corner": [0, 0, 20], "name": "Roof", "body_name": "Roof"}));
        run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 2, "corner": [0, 0, 10], "name": "Shelf", "body_name": "Shelf"}));
        run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
        run(&mut s, "sketch.rectangle.two_point", json!({"p0": [5, 5], "p1": [10, 10]}));
        run(&mut s, "sketch.finish", json!({}));
        run(&mut s, "solid.extrude", json!({"sketch": "S", "distance": 5, "to": [7, 7, 20], "operation": "new", "name": "Up", "body_name": "Up"}));
        run(&mut s, "sketch.create", json!({"plane": "XY", "name": "T"}));
        run(&mut s, "sketch.rectangle.two_point", json!({"p0": [25, 25], "p1": [30, 30]}));
        run(&mut s, "sketch.finish", json!({}));
        run(
            &mut s,
            "solid.extrude",
            json!({"sketch": "T", "distance": 5, "from": [27, 27, 12], "operation": "new", "name": "On", "body_name": "On"}),
        );
        run(&mut s, "timeline.copy", json!({"features": ["Roof", "Shelf", "S", "Up", "T", "On"]}));
        let r = run(&mut s, "timeline.paste", json!({"translate": [100, 0, 10]}));
        assert_eq!(r["errors"].as_array().map(Vec::len), Some(0), "{r}");
        let up = measures(&mut s, "Up (copy)");
        assert!((up["bbox"]["min"][2].as_f64().unwrap_or(0.0) - 10.0).abs() < 1e-6, "{up}");
        assert!((up["bbox"]["max"][2].as_f64().unwrap_or(0.0) - 30.0).abs() < 1e-6, "it ends on the pasted roof: {up}");
        let on = measures(&mut s, "On (copy)");
        assert!((on["bbox"]["min"][2].as_f64().unwrap_or(0.0) - 22.0).abs() < 1e-6, "it starts on the pasted shelf: {on}");
        assert!((on["bbox"]["max"][2].as_f64().unwrap_or(0.0) - 27.0).abs() < 1e-6, "{on}");
    }

    /// Pasting with a translate moves a hole's `to` pick with it: the pasted through hole still goes
    /// through, rather than stopping at the original's depth.
    #[test]
    fn a_pasted_hole_drills_to_the_moved_point() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 10, "name": "Block", "body_name": "Block"}));
        run(&mut s, "solid.hole", json!({"position": [20, 20, 10], "direction": [0, 0, -1], "diameter": 10, "to": [30, 30, -2], "name": "Drill"}));
        run(&mut s, "timeline.copy", json!({"features": ["Block", "Drill"]}));
        let r = run(&mut s, "timeline.paste", json!({"translate": [60, 0, -6]}));
        assert_eq!(r["errors"].as_array().map(Vec::len), Some(0), "{r}");
        let (block, copy) = (measures(&mut s, "Block"), measures(&mut s, "Block (copy)"));
        let v = |m: &serde_json::Value| m["volume_mm3"].as_f64().unwrap_or(f64::NAN);
        assert!((v(&block) - (16000.0 - std::f64::consts::PI * 25.0 * 10.0)).abs() < 1.0, "a through hole: {block}");
        assert!((v(&copy) - v(&block)).abs() < 1e-6, "the pasted hole goes through too: {copy}");
    }

    /// Pasting with a translate moves a revolve's `to` pick with it: the pasted turn ends at the same
    /// angle about its moved axis.
    #[test]
    fn a_pasted_revolve_turns_to_the_moved_point() {
        let mut s = Session::default();
        run(&mut s, "sketch.create", json!({"plane": "XZ", "name": "S"}));
        run(&mut s, "sketch.line", json!({"points": [[10, 0], [15, 0], [15, 5], [10, 5]], "closed": true}));
        run(&mut s, "sketch.finish", json!({}));
        run(&mut s, "solid.revolve", json!({"sketch": "S", "axis": "y", "to": [0, 12, 2], "name": "Turn", "body_name": "Arc"}));
        run(&mut s, "timeline.copy", json!({"features": ["S", "Turn"]}));
        let r = run(&mut s, "timeline.paste", json!({"translate": [50, 0, 0]}));
        assert_eq!(r["errors"].as_array().map(Vec::len), Some(0), "{r}");
        let (arc, copy) = (measures(&mut s, "Arc"), measures(&mut s, "Arc (copy)"));
        let v = |m: &serde_json::Value| m["volume_mm3"].as_f64().unwrap_or(f64::NAN);
        // A quarter turn of a 5 × 5 section whose centre is 12.5 from the axis.
        let quarter = 25.0 * std::f64::consts::FRAC_PI_2 * 12.5;
        assert!((v(&arc) - quarter).abs() < quarter * 1e-3, "{arc}");
        assert!((v(&copy) - v(&arc)).abs() < 1e-6, "the pasted arc turns a quarter too: {copy}");
    }

    /// A pasted coil that joins a body made by an earlier copy joins that copy, not the original.
    #[test]
    fn a_pasted_coil_joins_the_pasted_body() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 30, "width": 30, "height": 5, "name": "Base", "body_name": "Plate"}));
        run(
            &mut s,
            "solid.coil",
            json!({"diameter": 16, "pitch": 6, "revolutions": 3, "section_size": 2, "base": [15, 15, 5], "operation": "join", "targets": ["Plate"], "name": "Spring"}),
        );
        run(&mut s, "timeline.copy", json!({"features": ["Base", "Spring"]}));
        let r = run(&mut s, "timeline.paste", json!({"translate": [50, 0, 0]}));
        assert_eq!(r["errors"].as_array().map(Vec::len), Some(0), "{r}");
        let spring = r["pasted"][1]["name"].as_str().unwrap_or_default().to_string();
        let def = run(&mut s, "timeline.inspect", json!({"feature": spring}));
        assert_eq!(def["definition"]["targets"], json!(["Plate (copy)"]), "{def}");
        let st = s.world_state();
        let v = |name: &str| solidcraft_kernel::measure(&st.body(name).unwrap().body).unwrap().volume;
        assert!((v("Plate") - v("Plate (copy)")).abs() < 1e-6, "each plate carries one spring: {} vs {}", v("Plate"), v("Plate (copy)"));
    }
}
