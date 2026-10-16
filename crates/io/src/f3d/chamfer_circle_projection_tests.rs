use super::*;
use crate::f3d::import_tests::Fixture;
use crate::f3d::relationship_tests::{reference, string, u32, u64};
use solidcraft_doc::{Document, FeatureKind, Model, PlaneRef};
use solidcraft_geom::Plane;

const IDENTITY: &str = "7ACC2A03-0261-4879-A14A-A93D661A5BDC";
const TRACKER: &str = "4B31DA1B-FCD6-4527-BC3E-BE1FBAF867FE";
const LABELS: &str = "B7F34D7B-DAC4-41A5-B67D-D672537C3BFA";

fn ints(b: &mut Vec<u8>, values: &[i32]) {
    for value in values {
        u32(b, *value as u32);
    }
}

fn boundary(end: bool, bounded: bool) -> Vec<u8> {
    let mut b = vec![0; 6];
    ints(&mut b, &[1, 3, 2]);
    for (label, operation) in [(if end { "2" } else { "1" }, 8001), (if end { "71" } else { "13" }, 8002)] {
        u32(&mut b, 1);
        string(&mut b, label);
        ints(&mut b, &[0, 1]);
        u64(&mut b, operation);
    }
    u32(&mut b, 0);
    string(&mut b, if bounded { "bounded_face_recipe_data" } else { "edge_recipe_data" });
    if bounded {
        ints(&mut b, &[0, -1, 1]);
    }
    ints(&mut b, &[-1, -1, 2, 0, -1, 1, -1, 1, -1, 2, 0, -1, 1, -1, 0, 1, 91, 92, 93, 94, 95, 96, 97, 98, -1]);
    b
}

fn tracker(f: &mut Fixture, id: u64, identity: u64) {
    let mut b = vec![0; 6];
    reference(&mut b, identity);
    b.push(0);
    f.put(id, TRACKER, 1, b, None);
}

fn table(f: &mut Fixture, owner: u64, id: u64, entries: &[(u64, u64)]) {
    let b = &mut f.0.get_mut(&owner).unwrap().2;
    let at = b.len() - 59;
    let mut slot = Vec::new();
    reference(&mut slot, id);
    b[at..at + 11].copy_from_slice(&slot);
    let mut b = vec![0; 6];
    u32(&mut b, entries.len() as u32);
    for (label, tracker) in entries {
        reference(&mut b, *tracker);
        u64(&mut b, *label);
    }
    u64(&mut b, entries.iter().map(|(label, _)| *label).max().unwrap() + 1);
    f.put(id, LABELS, 2, b, Some(vec![0; 5]));
}

fn fixture(end: bool) -> Fixture {
    let mut f = super::super::body_projections::tests::body_circle(end);
    // This fixture describes a complete source carrier, not the relationship
    // fixture's independently numbered sketch partition.
    let original = f.0[&31].2.clone();
    let mut r = Reader { bytes: &original, at: 6 };
    for _ in 0..r.u32().unwrap() {
        let key = r.string().unwrap();
        r.string().unwrap();
        let at = r.at;
        r.u64().unwrap();
        if key == "crv_secondary_id" {
            f.0.get_mut(&31).unwrap().2[at..at + 8].fill(0);
        }
    }
    f.0.get_mut(&101).unwrap().2[41..49].fill(0);
    let chamfer = super::super::chamfers::tests::authored(3, 1, false, false, false);
    for id in [19, 900, 901, 902, 903, 904, 905] {
        f.0.insert(id, chamfer.0[&id].clone());
    }
    let b = &mut f.0.get_mut(&19).unwrap().2;
    let at = b.windows(8).position(|b| b == 71_u64.to_le_bytes()).unwrap();
    b[at - 4..at].copy_from_slice(&2_u32.to_le_bytes());
    let mut extra = 13_u64.to_le_bytes().to_vec();
    extra.extend([1, 0]);
    b.splice(at + 10..at + 10, extra);
    for (top, track, identity) in [(true, 601, 602), (false, 603, 604)] {
        f.put(identity, IDENTITY, 3, super::super::body_projections::tests::body_circle(top).0[&188].2.clone(), None);
        tracker(&mut f, track, identity);
    }
    // Label order deliberately differs from numeric order and input geometry.
    table(&mut f, 19, 600, &[(71, 601), (13, 603)]);
    let tag = Fixture(f.0.clone()).design().sketch_circles().unwrap().into_iter().find(|c| c.id == 31).unwrap().profile_id.unwrap().0;
    table(&mut f, 9, 610, &[(3, 611)]);
    let mut b = vec![0; 5];
    u32(&mut b, 1);
    b.push(0);
    reference(&mut b, 612);
    b.push(0);
    f.put(611, "335CC7BD-BBE5-40AA-9E8C-5EC99D427A3A", 0, b, None);
    let mut b = vec![0; 6];
    ints(&mut b, &[1, 0, 1, 3]);
    u64(&mut b, tag);
    u64(&mut b, 0);
    ints(&mut b, &[1, 1, 1, 0, 0, 0, 0]);
    f.put(612, "2DEE34BE-FA70-4D54-979C-164D86FDA786", 2, b, None);
    let mut map = vec![0; 6];
    u32(&mut map, 0);
    u32(&mut map, 2);
    for (op, owner) in [(8001, 9), (8002, 19)] {
        u64(&mut map, op);
        u64(&mut map, owner);
    }
    u32(&mut map, 0);
    reference(&mut map, 4);
    f.0.get_mut(&280).unwrap().2 = map;
    f.0.get_mut(&183).unwrap().2[7] = 0;
    f.0.get_mut(&185).unwrap().2 = boundary(end, true);
    f.0.get_mut(&188).unwrap().2 = boundary(end, false);
    f.put(288, IDENTITY, 3, boundary(end, false), None);
    tracker(&mut f, 284, 288);
    f.put(287, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1, vec![], None);
    let mut b = vec![0; 6];
    for value in [0_f64, 0., if end { 1.7 } else { 0. }, 0., 0., 1., 1., 0., 0., 2.8, 0., std::f64::consts::TAU] {
        b.extend(value.to_le_bytes());
    }
    b.push(0);
    reference(&mut b, 287);
    b.extend([0, 1]);
    reference(&mut b, 284);
    f.0.get_mut(&187).unwrap().2 = b;
    let b = &mut f.0.get_mut(&131).unwrap().2;
    let mut r = Reader { bytes: b, at: 6 };
    for _ in 0..r.u32().unwrap() {
        r.string().unwrap();
        r.string().unwrap();
        r.u64().unwrap();
    }
    let at = r.at + 9 * 8;
    b[at..at + 8].copy_from_slice(&2.8_f64.to_le_bytes());
    f
}

fn decode_fixture(f: Fixture) -> Result<Vec<super::super::projections::Projection>> {
    let d = f.design();
    let records = Records::new(&d);
    let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    decode(&records, &classes, 183, 18, &[(184, 131)])
}

fn source_document() -> Document {
    let mut doc = super::super::body_projections::tests::body_circle(false).design().to_document("Original paired circular chamfers").unwrap();
    doc.features.truncate(2);
    doc.next_id = 3;
    doc.set_param("bevel", "2 mm", Some("mm"), None).unwrap();
    let mut model = Model::new();
    model.evaluate(&doc);
    let state = model.state();
    let body = &state.bodies[0];
    let names = solidcraft_doc::naming::edge_names(body);
    let picks = names.iter().enumerate().filter(|(_, name)| name.contains("F2:top") || name.contains("F2:start")).collect::<Vec<_>>();
    let feature: solidcraft_doc::Feature = serde_json::from_value(serde_json::json!({
        "id":3,"name":"Both bevels","type":"chamfer","body":body.name,
        "edges":picks.iter().map(|(i,_)|body.mesh().edges[*i][1]).collect::<Vec<_>>(),
        "edge_names":picks.iter().map(|(_,name)|name.as_str()).collect::<Vec<_>>(),"distance":"bevel"
    }))
    .unwrap();
    let id = doc.add_feature(feature.kind, Some(&feature.name)).unwrap();
    doc.feature_mut(id).unwrap().edge_names = feature.edge_names;
    doc
}

fn attach(f: Fixture, doc: &Document, end: bool) -> Result<solidcraft_doc::sketch::Sketch> {
    let d = f.design();
    let records = Records::new(&d);
    let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    let projections = decode(&records, &classes, 183, 18, &[(184, 131)])?;
    let super::super::projections::Source::ChamferCap(cap) = &projections[0].source else { panic!("chamfer cap") };
    assert_eq!(cap.end, end);
    let mut model = Model::new();
    model.evaluate(doc);
    if model.results.iter().any(|r| r.error.is_some()) {
        return Err(invalid("invalid test dependency"));
    }
    let state = model.state();
    let points = d.sketch_points()?.into_iter().map(|p| (p.id, p)).collect();
    let lines = d.sketch_lines()?.into_iter().map(|p| (p.id, p)).collect();
    let circles = d.sketch_circles()?.into_iter().map(|p| (p.id, p)).collect();
    let arcs = BTreeMap::new();
    let native = super::super::projections::Geometry { points: &points, lines: &lines, circles: &circles, arcs: &arcs };
    let native_profiles = d.sketch_profiles()?;
    let profiles = native_profiles.iter().map(|p| (p.feature, p)).collect();
    let features = BTreeMap::from([(9, 2), (19, 3)]);
    let operations = BTreeMap::from([(8001, 9), (8002, 19)]);
    let context = super::super::body_projections::Context {
        native: &native,
        profiles: &profiles,
        doc,
        model: &state,
        features: &features,
        operations: &operations,
    };
    let topology = super::super::composite_blends::Context {
        records: &records,
        classes: &classes,
        operations: &operations,
        features: &features,
        profiles: &profiles,
        lines: &lines,
        circles: &circles,
        doc,
        state: &state,
        model: &model,
        before: None,
        depth: 0,
    };
    let mut sketch = solidcraft_doc::sketch::Sketch::new();
    cap.attach(&context, &topology, 131, 170, Plane::named("XY").unwrap(), &mut sketch)?;
    Ok(sketch)
}

#[test]
fn paired_chamfer_labels_create_live_circle_links_and_analytic_extrusions() {
    for end in [false, true] {
        let mut doc = source_document();
        let sketch = attach(fixture(end), &doc, end).unwrap();
        let linked = doc
            .add_feature(FeatureKind::Sketch { plane: PlaneRef::Custom { plane: Plane::named("XY").unwrap() }, sketch }, Some("Projected bevel"))
            .unwrap();
        let mut kind = doc.feature(2).unwrap().kind.clone();
        let FeatureKind::Extrude { sketch, profiles, extent, .. } = &mut kind else { panic!("extrude") };
        *sketch = (linked).into();
        *profiles = solidcraft_doc::ProfileSel::All;
        extent.distance = "7 mm".into();
        doc.add_feature(kind, Some("Projected extrusion")).unwrap();
        for (diameter, bevel) in [(60_f64, 2.), (80., 2.), (80., 4.)] {
            doc.change_param("d7", &format!("{diameter} mm"), None, None).unwrap();
            doc.change_param("bevel", &format!("{bevel} mm"), None, None).unwrap();
            let saved = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            assert_eq!(doc, saved);
            let mut model = Model::new();
            model.evaluate(&saved);
            assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
            let state = model.state();
            let projected = &state.sketch(linked).unwrap().sketch;
            assert!(projected.links.iter().all(|link| !link.lost && !link.edge_circle.is_empty()));
            let r = diameter / 2.;
            let expected = std::f64::consts::PI * (r * r * 17. - 2. * bevel * bevel * (r - bevel / 3.) + (r - bevel).powi(2) * 7.);
            super::super::import_tests::volume(&saved, expected);
            assert!(matches!(projected.curves[0].kind,solidcraft_doc::sketch::CurveKind::Circle{r:actual,..}if(actual-r+bevel).abs()<1e-6));
        }
    }
}

#[test]
fn chamfer_projection_requires_matching_closed_selector_cache_and_mapping() {
    for end in [false, true] {
        assert!(decode_fixture(fixture(end)).is_ok());
        for id in [183, 184, 185, 186, 187, 188, 280, 284, 288] {
            let bytes = fixture(end).0[&id].2.clone();
            for keep in 0..bytes.len() {
                let mut f = fixture(end);
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(decode_fixture(f).is_err(), "{end}/{id}/{keep}");
            }
        }
        for id in [185, 188, 288] {
            let mut f = fixture(end);
            f.0.get_mut(&id).unwrap().2 = boundary(!end, id == 185);
            assert!(decode_fixture(f).is_err());
        }
    }
}

#[test]
fn output_labels_cannot_switch_ends_or_refer_to_unselected_edges() {
    let doc = source_document();
    for end in [false, true] {
        let mut f = fixture(end);
        let a = f.0[&601].clone();
        let b = f.0[&603].clone();
        f.0.insert(601, b);
        f.0.insert(603, a);
        assert!(attach(f, &doc, end).is_err(), "an output label cannot be mapped to the opposite end");
        for id in [600, 601, 602, 603, 604, 610, 611, 612] {
            let mut f = fixture(end);
            f.0.get_mut(&id).unwrap().2.clear();
            if (end && [603, 604].contains(&id)) || (!end && [601, 602].contains(&id)) {
                continue;
            }
            assert!(attach(f, &doc, end).is_err(), "missing tracking record {id}");
        }
        let mut changed = doc.clone();
        changed.feature_mut(3).unwrap().edge_names.retain(|name| if end { !name.contains("F2:top") } else { !name.contains("F2:start") });
        assert!(attach(fixture(end), &changed, end).is_err());
    }
}
