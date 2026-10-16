use crate::{Session, doc};
use serde_json::json;

fn model() -> Session {
    let mut s = Session::default();
    for (command, params) in [
        ("component.create", json!({"name":"Joined part"})),
        ("parameters.add", json!({"name":"height","expression":"10 mm","unit":"mm"})),
        ("sketch.create", json!({"plane":"XY","name":"Source"})),
        ("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[10,20]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance":"5 mm","name":"First"})),
        ("solid.extrude", json!({"sketch":"Source","distance":"height","operation":"join","name":"Joined"})),
        ("construct.plane.offset", json!({"base":"YZ","offset":"40 mm","name":"Upper plane"})),
        ("sketch.create", json!({"plane":"Upper plane","name":"Upper"})),
        ("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[20,30]})),
        ("sketch.finish", json!({})),
        ("solid.loft", json!({"sections":[{"face":[10,10,5]},{"sketch":"Upper"}],"name":"Transition"})),
    ] {
        s.execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
    }
    s
}

fn measure(s: &mut Session, want: f64) {
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
    let got = s.execute("inspect.measure", &json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((got - want).abs() < want * 1e-3, "{got} != {want}");
}

#[test]
fn joined_side_loft_keeps_contributions_through_edit_reopen_undo_and_copy() {
    let mut s = model();
    measure(&mut s, 14000.);
    let original = s.doc.clone();
    let loft = s.doc.find_feature("Transition").unwrap();
    let component = loft.component;
    let doc::FeatureKind::Loft { sections, .. } = &loft.kind else { panic!("loft") };
    assert!(
        matches!(&sections[0].face.as_ref().unwrap().target.certificate, Some(doc::config::FaceCertificate::PlanarUnion { sources }) if sources.len() == 2)
    );
    s.execute("parameters.change", &json!({"name":"height","expression":"15 mm"})).unwrap();
    measure(&mut s, 16500.);
    measure(&mut Session::new(doc::Document::from_json(&s.doc.to_json()).unwrap()), 16500.);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, original);
    measure(&mut s, 14000.);
    s.execute("component.paste_new", &json!({"component":component,"translate":[60,0,0]})).unwrap();
    measure(&mut s, 28000.);
    measure(&mut Session::new(doc::Document::from_json(&s.doc.to_json()).unwrap()), 28000.);
}

#[test]
fn missing_or_changed_join_contribution_cannot_retarget_by_plane() {
    let s = model();
    for variant in 0..3 {
        let mut document = (*s.doc).clone();
        let id = document.find_feature("Joined").unwrap().id;
        let feature = document.feature_mut(id).unwrap();
        match variant {
            0 => feature.suppressed = true,
            1 => {
                let doc::FeatureKind::Extrude { extent, .. } = &mut feature.kind else { panic!("extrude") };
                extent.taper = Some("5 deg".into());
            }
            _ => {
                let doc::FeatureKind::Extrude { operation, .. } = &mut feature.kind else { panic!("extrude") };
                *operation = doc::Operation::NewBody;
            }
        }
        let changed = Session::new(document);
        let loft = changed.doc.find_feature("Transition").unwrap().id;
        assert!(changed.model.results.iter().any(|r| r.id == loft && r.error.is_some()), "variant {variant}: {:?}", changed.model.results);
    }
}

#[test]
fn nested_oversized_and_forged_union_certificates_are_rejected() {
    let s = model();
    let original: serde_json::Value = serde_json::from_str(&s.doc.to_json()).unwrap();
    let index = s.doc.features.iter().position(|f| f.name == "Transition").unwrap();
    for variant in 0..5 {
        let mut value = original.clone();
        let certificate = &mut value["features"][index]["sections"][0]["face"]["target"]["certificate"];
        let leaf = certificate["sources"][0].clone();
        match variant {
            0 => certificate["sources"] = json!(vec![leaf; 65]),
            1 => certificate["sources"][0]["source"] = certificate.clone(),
            2 => certificate["sources"][0]["producer"] = json!(0),
            3 => certificate["sources"][0]["source"]["unexpected"] = json!(true),
            _ => certificate["sources"][1] = leaf,
        }
        assert!(doc::Document::from_json(&value.to_string()).is_err(), "variant {variant}");
    }
}
