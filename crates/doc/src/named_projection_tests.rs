use crate::{Document, ModelBody, ModelState};
use solidcraft_geom::{Plane, Vec3};
use solidcraft_sketch::{LinkKind, LinkSource, Sketch};
use std::collections::BTreeMap;
use std::sync::Arc;

fn named_box(name: &str, width: f64, height: f64) -> ModelBody {
    let body = solidcraft_kernel::box_solid(Vec3::ZERO, Vec3::new(width, width, height)).unwrap();
    let mut names = vec!["unrelated".to_string(); body.faces(0.01).unwrap().len()];
    let top = body.faces(0.01).unwrap().into_iter().find(|face| face.plane_normal.is_some_and(|normal| normal.z > 0.99)).unwrap().index;
    names[top] = "shared:top".into();
    let mut result = ModelBody::new(name.into(), body, 1);
    result.names = crate::naming::NameCell::known(Arc::new(names));
    result
}

#[test]
fn missing_named_edge_does_not_repick_the_saved_position() {
    let mut doc = Document::default();
    let mut state = ModelState::default();
    state.bodies.push(named_box("Source", 20., 10.));
    let source = LinkSource::Edge { body: "Source".into(), at: Vec3::new(10., 0., 10.) };
    let r = crate::project::resolve(&doc, &BTreeMap::new(), &state, &Plane::XY, LinkKind::Project, &source).unwrap();
    let mut sketch = Sketch::default();
    sketch.add_link(LinkKind::Project, source, &r.geom).unwrap();
    sketch.links[0].edge_name = Some("F42:top|F42:side:c1".into());
    let saved = sketch.clone();
    let warnings = crate::project::refresh_links(&doc, &BTreeMap::new(), &state, &Plane::XY, &mut sketch);
    assert!(!warnings.is_empty());
    assert!(sketch.links[0].lost);
    assert_eq!(sketch.points, saved.points);
    assert_eq!(sketch.curves, saved.curves);
    let id = doc.add_feature(crate::FeatureKind::Sketch { plane: crate::PlaneRef::Origin { name: "XY".into() }, sketch }, None).unwrap();
    let mut feature = doc.feature(id).unwrap().clone();
    feature
        .remap_clone(&BTreeMap::from([(42, 90)]), &BTreeMap::new(), &BTreeMap::from([("Source".into(), "Copy".into())]), &BTreeMap::new())
        .unwrap();
    let crate::FeatureKind::Sketch { sketch, .. } = feature.kind else { panic!("sketch") };
    assert_eq!(sketch.links[0].edge_name.as_deref(), Some("F90:top|F90:side:c1"));
    assert!(matches!(&sketch.links[0].source,LinkSource::Edge {body,..} if body=="Copy"));
}

#[test]
fn named_manual_projection_keeps_its_body_and_face_at_shared_boundaries() {
    let doc = Document::default();
    let mut initial = ModelState::default();
    initial.bodies.extend([named_box("Source", 20., 10.), named_box("Control", 5., 10.)]);
    let plane = Plane::XY.offset(20.);
    let source = LinkSource::Face { body: "Source".into(), at: Vec3::new(1., 1., 10.) };
    let resolved = crate::project::resolve(&doc, &BTreeMap::new(), &initial, &plane, LinkKind::Project, &source).unwrap();
    let mut sketch = Sketch::default();
    let link = sketch.add_link(LinkKind::Project, source, &resolved.geom).unwrap();
    sketch.links[0].face_name = Some("shared:top".into());
    sketch.links[0].source = LinkSource::Face { body: "Source".into(), at: Vec3::new(0., 0., 10.) };
    let mut current = ModelState::default();
    current.bodies.extend([named_box("Source", 20., 20.), named_box("Control", 5., 10.)]);
    assert!(crate::project::refresh_links(&doc, &BTreeMap::new(), &current, &plane, &mut sketch).is_empty());
    assert!(!sketch.links[0].lost);
    assert!(sketch.points.iter().any(|point| point.pos.x > 19.));
    assert!(sketch.points.iter().any(|point| point.pos.y > 19.));
    assert!(matches!(&sketch.links[0].source,LinkSource::Face{body,at} if body=="Source" && (at.z-20.).abs()<1e-6));
    // The closest named point can sit on the wall/top seam: retain its actual
    // face index rather than re-picking the adjacent vertical wall.
    let index = crate::naming::face_on_body(current.body("Source").unwrap(), "shared:top", Vec3::new(0., 0., 10.)).unwrap().0;
    assert!(current.body("Source").unwrap().body.faces(0.01).unwrap()[index].plane_normal.unwrap().z > 0.99);
    let saved = sketch.clone();
    let mut missing = ModelState::default();
    missing.bodies.push(named_box("Control", 5., 10.));
    let warnings = crate::project::refresh_links(&doc, &BTreeMap::new(), &missing, &plane, &mut sketch);
    assert!(sketch.links[0].lost);
    assert!(!warnings.is_empty());
    assert_eq!(sketch.curves, saved.curves);
    assert_eq!(sketch.points, saved.points);
    assert_eq!(sketch.links[0].id, link);
    assert!(matches!(&sketch.links[0].source,LinkSource::Face{body,..} if body=="Source"));
}
