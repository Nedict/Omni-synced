use serde_json::json;

use crate::{Session, product_scene};

#[test]
fn named_opaque_finishes_reach_the_scene_and_survive_undo_and_reopen_without_changing_mass() {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"body_name":"Specimen","length":10,"width":10,"height":10})).unwrap();
    s.execute("material.assign", &json!({"bodies":["Specimen"],"material":"Steel"})).unwrap();
    let mass = s.execute("component.properties", &json!({})).unwrap();
    let library = s.execute("appearance.library", &json!({})).unwrap();
    let entries = library["appearances"].as_array().unwrap();
    // Exact pre-finish compatibility fixture. Identify legacy rows by their
    // names/values, not by missing IDs: independent future packs may coexist.
    let legacy: [(&str, [u8; 3], f64); 20] = [
        ("Aluminum - Satin", [204, 208, 214], 1.0),
        ("Aluminum - Anodized Black", [44, 46, 50], 1.0),
        ("Aluminum - Anodized Blue", [52, 92, 160], 1.0),
        ("Steel - Satin", [158, 164, 172], 1.0),
        ("Stainless Steel - Brushed", [186, 191, 198], 1.0),
        ("Cast Iron", [104, 104, 110], 1.0),
        ("Brass - Polished", [208, 172, 88], 1.0),
        ("Copper", [204, 122, 80], 1.0),
        ("Titanium", [150, 148, 158], 1.0),
        ("ABS - White", [236, 236, 230], 1.0),
        ("ABS - Black", [36, 36, 38], 1.0),
        ("ABS - Red", [196, 40, 40], 1.0),
        ("ABS - Blue", [40, 90, 186], 1.0),
        ("ABS - Yellow", [236, 196, 40], 1.0),
        ("ABS - Green", [52, 150, 80], 1.0),
        ("Rubber - Black", [28, 28, 28], 1.0),
        ("Glass - Clear", [190, 220, 228], 0.25),
        ("Glass - Smoked", [90, 96, 104], 0.45),
        ("Wood - Oak", [190, 146, 92], 1.0),
        ("Paint - Gloss White", [246, 246, 244], 1.0),
    ];
    assert_eq!(solidcraft_doc::appearance::LIBRARY, legacy);
    for (name, color, opacity) in legacy {
        let matching: Vec<_> = entries.iter().filter(|entry| entry["name"] == name).collect();
        assert_eq!(matching.len(), 1, "legacy row must occur exactly once: {name}");
        let row = matching[0];
        assert_eq!(row["color"], format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]));
        assert_eq!(row["opacity"], opacity);
        assert!(row.get("pbr").is_none() && row.get("id").is_none(), "legacy scalar meaning changed: {name}");
    }
    assert_eq!(
        solidcraft_doc::appearance::OPAQUE_FINISHES.map(|(name, ..)| name),
        ["Steel - Satin PBR", "Aluminum - Satin PBR", "Copper - Clean PBR", "ABS - Molded PBR", "PLA - Smooth PBR", "Wood - Oak Uniform PBR Preview"]
    );
    for (name, ..) in solidcraft_doc::appearance::OPAQUE_FINISHES {
        let expected = solidcraft_doc::appearance::library(name).unwrap();
        let matching: Vec<_> = entries.iter().filter(|entry| entry["name"] == name).collect();
        assert_eq!(matching.len(), 1, "finish row must occur exactly once: {name}");
        assert_eq!(matching[0]["color"], expected.hex());
        assert_eq!(matching[0]["opacity"], 1.0);
        assert_eq!(matching[0]["pbr"], json!(expected.pbr.unwrap()));
        assert!(!legacy.iter().any(|(legacy_name, ..)| legacy_name == &name));
        assert!(entries.iter().any(|e| e["name"] == name && e["pbr"] == json!(expected.pbr.unwrap())));
        let before = s.doc.appearances.clone();
        s.execute("appearance.assign", &json!({"bodies":["Specimen"],"appearance":name.to_lowercase()})).unwrap();
        assert_eq!(s.doc.appearances.bodies["Specimen"], expected);
        assert_eq!(product_scene::extract(&s).unwrap().meshes[0].surface, expected.opaque_surface().unwrap());
        assert_eq!(s.execute("component.properties", &json!({})).unwrap(), mass);
        let saved = s.doc.to_json();
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.appearances, before);
        s.execute("edit.redo", &json!({})).unwrap();
        let mut reopened = Session::new(solidcraft_doc::Document::from_json(&saved).unwrap());
        assert_eq!(reopened.doc.appearances.bodies["Specimen"], expected);
        assert_eq!(reopened.execute("component.properties", &json!({})).unwrap(), mass);
        assert!(solidcraft_io::catalog::Catalog::from_document(&reopened.doc, "user.fixture", "Original", Default::default()).is_err());
    }
}

#[test]
fn named_opaque_finish_refuses_transparency_atomically_and_allows_explicit_scalar_override() {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"body_name":"Specimen","length":10,"width":10,"height":10})).unwrap();
    let doc = s.doc.clone();
    let history = (s.undo.len(), s.redo.len(), s.revision);
    for opacity in [json!(0.5), json!(2), json!("invalid"), json!(null)] {
        assert!(s.execute("appearance.assign", &json!({"bodies":["Specimen"],"appearance":"Steel - Satin PBR","opacity":opacity})).is_err());
        assert_eq!(*s.doc, *doc);
        assert_eq!((s.undo.len(), s.redo.len(), s.revision), history);
    }
    s.execute("appearance.assign", &json!({"bodies":["Specimen"],"appearance":"Steel - Satin PBR","pbr":null,"opacity":0.5})).unwrap();
    assert!(s.doc.appearances.bodies["Specimen"].pbr.is_none());
    assert_eq!(s.doc.appearances.bodies["Specimen"].opacity, 0.5);
    for (name, ..) in solidcraft_doc::appearance::LIBRARY {
        assert!(solidcraft_doc::appearance::library(name).unwrap().pbr.is_none());
    }
}
