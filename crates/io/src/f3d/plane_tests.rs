//! Original field-authored records. No exported Fusion bytes are embedded.
use super::import_tests::{Fixture, common, fixture, volume};
use super::relationship_tests::{reference, refs, u32, wide};
use super::*;
use solidcraft_doc::{Document, FeatureKind, Model, PlaneRef};

#[test]
fn rotated_sketch_basis_retains_live_origin_plane_offset_dependencies() {
    use solidcraft_geom::{Plane, Vec3};
    for (name, origin) in [("XY", Plane::XY), ("XZ", Plane::XZ), ("YZ", Plane::YZ)] {
        let mut doc = Document::default();
        doc.set_param("height", "20 mm", Some("mm"), None).unwrap();
        doc.add_feature(
            FeatureKind::ConstructionPlane {
                plane: PlaneRef::Offset { base: Box::new(PlaneRef::Origin { name: name.into() }), distance: "height".into() },
            },
            Some("Support"),
        )
        .unwrap();
        let cached = origin.offset(20.);
        let sketch = Plane::new(cached.origin, -cached.y, cached.x).unwrap();
        let base = PlaneRef::Construction { name: "Support".into() };
        let attached = super::import::sketch_plane_reference(base.clone(), cached, sketch).unwrap();
        assert!(matches!(&attached, PlaneRef::AtAngle { base, .. } if matches!(base.as_ref(), PlaneRef::Construction { name } if name == "Support")));
        for height in [20., 35.] {
            doc.set_param("height", &format!("{height} mm"), Some("mm"), None).unwrap();
            let cold = crate::read_design(&crate::write_design(&doc)).unwrap();
            for doc in [&doc, &cold] {
                let (values, _) = doc.param_values();
                let plane = doc.resolve_plane(&values, &attached, 0).unwrap();
                assert!(plane.origin.dist(origin.normal() * height) < 1e-9);
                assert!((plane.x - sketch.x).len() < 1e-9 && (plane.y - sketch.y).len() < 1e-9);
            }
        }
        let displaced = Plane { origin: sketch.origin + cached.x, ..sketch };
        assert!(super::import::sketch_plane_reference(base.clone(), cached, displaced).is_err());
        let opposite = Plane { y: -sketch.y, ..sketch };
        assert!(super::import::sketch_plane_reference(base.clone(), cached, opposite).is_err());
        let nonfinite = Plane { origin: Vec3::new(f64::NAN, 0., 0.), ..sketch };
        assert!(super::import::sketch_plane_reference(base, cached, nonfinite).is_err());
    }
}

fn matrix(z: f64) -> [f64; 16] {
    [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., z, 0., 0., 0., 1.]
}

#[test]
fn legacy_origin_geometry_retains_exact_root_plane_identity() {
    for version in [0, 1] {
        let mut f = planes_fixture();
        let record = f.0.get_mut(&1000).unwrap();
        record.1 = version;
        if version == 0 {
            record.2.drain(119..124);
            record.2.drain(10..14);
            record.2.remove(record.2.len() - 12);
        }
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let records = Records::new(&d);
        assert_eq!(planes::origin(&records, &classes, 1000, 3).unwrap(), ("XY".into(), solidcraft_geom::Plane::XY));
        assert!(planes::origin(&records, &classes, 1000, 4).is_err());
    }
}

#[test]
#[ignore = "requires local original adapter and unchanged public API resave"]
fn original_adapter_origin_planes_match_modern_resave() {
    let corpus = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_CORPUS").unwrap());
    let probes = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    let mut original = None;
    for path in [corpus.join("adapter-x20-makita.f3d"), probes.join("corpus-adapter-x20-makita.f3d")] {
        let designs = read_f3d(&std::fs::read(path).unwrap()).unwrap();
        let d = &designs[0];
        let records = Records::new(d);
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let root = d.classes.iter().find(|c| c.id == "E03784ED-5E19-4E14-B9F2-3B07017018CD").unwrap().objects[0];
        let planes: Vec<_> = d
            .classes
            .iter()
            .filter(|c| c.id == planes::GEOMETRY)
            .flat_map(|c| &c.objects)
            .filter_map(|id| planes::origin(&records, &classes, *id, root).ok().map(|plane| (*id, plane)))
            .collect();
        assert_eq!(planes.len(), 3);
        for (_, (name, plane)) in &planes {
            assert_eq!(
                *plane,
                match name.as_str() {
                    "XY" => solidcraft_geom::Plane::XY,
                    "XZ" => solidcraft_geom::Plane::XZ,
                    "YZ" => solidcraft_geom::Plane::YZ,
                    _ => panic!("unexpected origin plane"),
                }
            );
        }
        if let Some(original) = &original {
            assert_eq!(&planes, original);
        } else {
            original = Some(planes);
        }
    }
}
fn definition(f: &mut Fixture, id: u64, owner: u64, base: u64, distance: Option<u64>, extra: Option<u64>, z: f64) {
    let mut b = vec![0; 40];
    b.extend([1, 0]);
    if let Some(extra) = extra {
        reference(&mut b, extra);
    } else {
        b.push(0);
    }
    b.extend([0; 8]);
    for n in matrix(z) {
        b.extend(n.to_le_bytes());
    }
    b.extend([0; 35]);
    u32(&mut b, 2);
    b.push(0);
    refs(&mut b, &distance.into_iter().collect::<Vec<_>>());
    refs(&mut b, &[id + 1]);
    b.extend([0; 2]);
    refs(&mut b, &[]);
    b.extend([0; 60]);
    reference(&mut b, owner);
    f.put(id, planes::DEFINITION, 15, b, None);
    let mut b = vec![0; 6];
    reference(&mut b, id + 2);
    u32(&mut b, 1);
    wide(&mut b, "original-one");
    wide(&mut b, "original-two");
    u32(&mut b, 2);
    b.extend([0; 4]);
    refs(&mut b, &[]);
    b.extend([0; 60]);
    reference(&mut b, owner);
    f.put(id + 1, "5A1BF548-241F-46FD-9FB5-E4B05126EB9D", 1, b, None);
    let mut b = vec![0; 6];
    b.extend(base.to_le_bytes());
    f.put(id + 2, "90055C05-546C-4EE7-B3C9-3DD922AD0C9C", 0, b, None);
}
fn parameter(f: &mut Fixture, id: u64, owner: u64, name: &str, label: &str, value: f64) {
    let mut b = vec![0; 20];
    u32(&mut b, 0);
    reference(&mut b, id + 1);
    wide(&mut b, &format!("{} mm", value * 10.));
    b.extend([0; 10]);
    for s in [label, "", "mm", name] {
        wide(&mut b, s);
    }
    b.extend(value.to_le_bytes());
    b.extend([0; 12]);
    f.put(id, "7A6A3D31-BE74-4E19-9115-20642944C2E4", 9, b, None);
    let mut b = vec![0; 4];
    b.push(1);
    refs(&mut b, &[owner]);
    b.extend([0; 5]);
    b.extend(value.to_le_bytes());
    reference(&mut b, id);
    f.put(id + 1, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0, b, None);
}
fn planes_fixture() -> Fixture {
    let mut f = fixture(false);
    let mut b = vec![0; 4];
    b.extend([0, 1]);
    u32(&mut b, 0);
    b.extend([0; 21]);
    for n in [0_f64, 0., 0., 1., 0., 0., 0., 1., 0.] {
        b.extend(n.to_le_bytes());
    }
    b.extend([0; 26]);
    wide(&mut b, "XY");
    b.extend([0; 37]);
    reference(&mut b, 1001);
    f.put(1000, planes::GEOMETRY, 1, b, None);
    let mut b = vec![0; 8];
    refs(&mut b, &[1000]);
    reference(&mut b, 3);
    f.put(1001, "2972A3B8-3627-49BC-8C3D-18707605EC44", 1, b, None);
    for (owner, base, offset, total, name) in [(2000, 1000, 2., 2., "parent"), (2100, 2001, 1., 3., "child")] {
        parameter(&mut f, owner + 20, owner, name, "AlongDistance", offset);
        parameter(&mut f, owner + 22, owner, &format!("internal{owner}"), "ExtraOffset", 0.);
        definition(&mut f, owner + 10, owner, base, Some(owner + 21), Some(owner + 23), total);
        let mut b = vec![0; 6];
        b.extend([1, 0, 0, 0, 0]);
        reference(&mut b, owner + 23);
        b.push(1);
        reference(&mut b, owner + 1);
        reference(&mut b, owner + 10);
        b.push(1);
        common(&mut b, name, false);
        f.put(owner, "D869265F-F339-4751-A066-91D30F81FF08", 4, b, None);
        f.put(owner + 1, planes::GEOMETRY, 1, vec![], None);
    }
    let mut b = vec![0; 6];
    reference(&mut b, 4);
    refs(&mut b, &[2000, 2100, 8, 9]);
    f.0.get_mut(&5).unwrap().2 = b;
    let mut b = vec![0; 6];
    b.push(0);
    for n in matrix(3.) {
        b.extend(n.to_le_bytes());
    }
    b.push(0);
    reference(&mut b, 3);
    f.0.get_mut(&6).unwrap().2 = b;
    let mut support = Vec::new();
    reference(&mut support, 3000);
    f.0.get_mut(&8).unwrap().2[38..49].copy_from_slice(&support);
    definition(&mut f, 3000, 8, 2101, None, None, 3.);
    f
}
fn bounds(doc: &Document, z: f64) {
    let mut model = Model::new();
    model.evaluate(doc);
    for result in &model.results {
        assert!(result.error.is_none(), "{result:?}");
    }
    let state = model.state();
    let sketch = state.sketch(doc.features[2].id).unwrap();
    assert!((sketch.plane.origin.z - z).abs() < 1e-9);
    volume(doc, std::f64::consts::PI * 30. * 30. * 17.);
}

#[test]
fn compact_identity_plane_transform_matches_explicit_matrix() {
    for compact in [false, true] {
        let mut f = planes_fixture();
        definition(&mut f, 3000, 8, 1000, None, None, 0.);
        let b = &mut f.0.get_mut(&3000).unwrap().2;
        if compact {
            b[50] = 1;
            b.drain(51..179);
        }
        let d = f.design();
        let records = Records::new(&d);
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let definition = planes::definition(&records, &classes, 3000, 8).unwrap();
        assert_eq!(definition.plane, solidcraft_geom::Plane::XY);
    }
    let mut f = planes_fixture();
    f.0.get_mut(&3000).unwrap().2[50] = 2;
    assert!(f.design().to_document("Unknown transform encoding").is_err());
}

fn absent_extra_offsets(legacy: bool) -> Fixture {
    let mut f = planes_fixture();
    for (owner, base, total, name) in [(2000, 1000, 2., "parent"), (2100, 2001, 3., "child")] {
        f.0.remove(&(owner + 22));
        f.0.remove(&(owner + 23));
        definition(&mut f, owner + 10, owner, base, Some(owner + 21), None, total);
        let mut b = vec![0; 4];
        b.extend([0, 1]);
        u32(&mut b, 0); // present but empty attributes
        u32(&mut b, 1);
        if !legacy {
            b.push(0);
        }
        b.extend([0, 1]); // absent extra offset, geometry follows
        reference(&mut b, owner + 1);
        reference(&mut b, owner + 10);
        b.push(0);
        common(&mut b, name, false);
        f.put(owner, "D869265F-F339-4751-A066-91D30F81FF08", if legacy { 2 } else { 4 }, b, None);
    }
    if legacy {
        // A class has one version for every instance, including sketch supports.
        for (guid, version, b, _) in f.0.values_mut() {
            if guid == planes::DEFINITION {
                *version = 8;
                b.drain(26..41);
                b.remove(203);
            }
        }
    }
    f
}

#[test]
fn legacy_and_resaved_planes_without_extra_offsets_preserve_parent_edits() {
    for legacy in [false, true] {
        let d = absent_extra_offsets(legacy).design();
        let planes = d.offset_planes().unwrap();
        assert_eq!(planes.len(), 2);
        assert!(planes.iter().all(|p| p.extra.is_none()));
        assert_eq!(planes.iter().map(|p| (p.feature, p.base, p.distance)).collect::<Vec<_>>(), [(2000, 1000, 2021), (2100, 2001, 2121)]);
        let mut doc = d.to_document("Original legacy offset planes").unwrap();
        bounds(&doc, 30.);
        doc.change_param("parent", "40 mm", None, None).unwrap();
        bounds(&doc, 50.);
        bounds(&crate::read_design(&crate::write_design(&doc)).unwrap(), 50.);
        for id in [2000, 2010] {
            for keep in 0..absent_extra_offsets(legacy).0[&id].2.len() {
                let mut f = absent_extra_offsets(legacy);
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(f.design().offset_planes().is_err(), "{legacy}/{id}/{keep}");
            }
        }
    }
}

#[test]
fn offset_plane_dependencies_follow_parent_edits_and_reopen() {
    let mut doc = planes_fixture().design().to_document("Offset planes").unwrap();
    assert_eq!(doc.features.len(), 4);
    assert_eq!(doc.features[0].param_names, ["parent"]);
    assert_eq!(doc.features[1].param_names, ["child"]);
    assert!(doc.all_param_exprs().iter().all(|(name, _, _)| !name.starts_with("internal")));
    assert!(matches!(&doc.features[2].kind, FeatureKind::Sketch { plane: PlaneRef::Construction { name }, .. } if name == "child1"));
    bounds(&doc, 30.);
    doc.change_param("parent", "-20 mm", None, None).unwrap();
    bounds(&doc, -10.);
    doc.change_param("child", "parent / -2", None, None).unwrap();
    bounds(&doc, -10.);
    doc.change_param("parent", "40 mm", None, None).unwrap();
    bounds(&doc, 20.);
    let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
    assert_eq!(doc, reopened);
    bounds(&reopened, 20.);
}
#[test]
fn malformed_plane_references_and_every_truncated_record_fail() {
    for id in [1000, 1001, 2000, 2010, 2011, 2012, 3000] {
        for keep in 0..planes_fixture().0[&id].2.len() {
            let mut f = planes_fixture();
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(f.design().to_document("Truncated").is_err(), "record {id}, prefix {keep}");
        }
    }
    for base in [2001_u64, 2101, 9999] {
        let mut f = planes_fixture();
        f.0.get_mut(&2012).unwrap().2[6..14].copy_from_slice(&base.to_le_bytes());
        assert!(f.design().to_document("Cyclic, forward or missing plane").is_err());
    }
    let mut f = planes_fixture();
    parameter(&mut f, 2022, 2000, "extra", "ExtraOffset", 1.);
    assert!(f.design().to_document("Unverified extra offset").is_err());
    let mut f = planes_fixture();
    let mut b = vec![0; 6];
    reference(&mut b, 4);
    refs(&mut b, &[8, 2000, 2100, 9]);
    f.0.get_mut(&5).unwrap().2 = b;
    assert!(f.design().to_document("Sketch precedes its support").is_err());
    let mut f = planes_fixture();
    f.0.get_mut(&2010).unwrap().2[61..69].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(f.design().to_document("Non-finite cached plane").is_err());
}
