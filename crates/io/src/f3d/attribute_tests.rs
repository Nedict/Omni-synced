use super::import_tests::{Fixture, fixture, volume};
use super::polygon_tests::rectangle;

fn omit(f: &mut Fixture, id: u64, at: usize) {
    let b = &mut f.0.get_mut(&id).unwrap().2;
    assert_eq!(&b[at..at + 5], &[1, 0, 0, 0, 0]);
    b[at] = 0;
    b.drain(at + 1..at + 5);
}

#[test]
fn absent_metadata_keeps_complete_circle_and_polygon_history() {
    for polygon in [false, true] {
        let mut f = if polygon { rectangle() } else { fixture(false) };
        let expected = if polygon { rectangle() } else { fixture(false) }.design().to_document("Original optional metadata").unwrap();
        for id in [41, 42, 70] {
            omit(&mut f, id, 5);
        }
        if polygon {
            for id in [71, 72] {
                omit(&mut f, id, 5);
            }
            for id in [55, 58, 59, 60] {
                omit(&mut f, id, 24);
            }
            for id in [56, 57] {
                omit(&mut f, id, 54);
            }
        } else {
            for id in [55, 56] {
                omit(&mut f, id, 39);
            }
        }
        let mut doc = f.design().to_document("Original optional metadata").unwrap();
        assert_eq!(doc, expected);
        if polygon {
            doc.change_param("width", "45 mm", None, None).unwrap();
            volume(&doc, 45. * 20. * 17.);
        } else {
            doc.change_param("d7", "40 mm", None, None).unwrap();
            volume(&doc, 400. * std::f64::consts::PI * 17.);
        }
        assert_eq!(crate::read_design(&crate::write_design(&doc)).unwrap(), doc);
    }
}

#[test]
fn native_centerlines_stay_out_of_profiles_and_keep_parameter_edits() {
    use super::relationship_tests::{reference, refs, wide};
    let mut f = rectangle();
    let mut b = vec![0; 4];
    b.extend([0, 0]); // absent optional attributes
    let delta = [3_f64, 2., 0.];
    let len = 13_f64.sqrt();
    for n in [0_f64; 3].into_iter().chain(delta).chain(delta.map(|x| x / len)).chain([0., 0., 1.]) {
        b.extend(n.to_le_bytes());
    }
    reference(&mut b, 71);
    reference(&mut b, 42);
    b.extend([0; 7]);
    b.push(2); // centerline, distinct from construction
    b.extend([0; 34]);
    reference(&mut b, 70);
    f.put(81, "DCA267ED-D615-4934-B64F-AD805E8003E2", 2, b, None);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 6);
    secondary.extend([0; 17]);
    wide(&mut secondary, "Rectangle with diagonal axis");
    secondary.push(0);
    refs(&mut secondary, &[42, 41, 71, 72, 31, 32, 33, 34, 81, 56, 57]);
    reference(&mut secondary, 3);
    f.0.get_mut(&70).unwrap().3 = Some(secondary);
    let mut doc = f.design().to_document("Centerline prism").unwrap();
    let solidcraft_doc::FeatureKind::Sketch { sketch, .. } = &doc.features[0].kind else { panic!("sketch") };
    let axis = sketch.curves.iter().find(|c| c.id == "c81").unwrap();
    assert!(axis.centerline && !axis.construction);
    volume(&doc, 30. * 20. * 17.);
    doc.change_param("width", "45 mm", None, None).unwrap();
    volume(&doc, 45. * 20. * 17.);
    assert_eq!(crate::read_design(&crate::write_design(&doc)).unwrap(), doc);
}

#[test]
fn linked_geometry_is_inspectable_but_never_flattened_on_import() {
    for (polygon, id, from_end) in [(true, 31, 49), (false, 31, 49), (false, 41, 82)] {
        let mut f = if polygon { rectangle() } else { fixture(false) };
        let b = &mut f.0.get_mut(&id).unwrap().2;
        // Curves: reference flag 49 bytes from end. Points: linked flag
        // after fixation/origin and before coordinates (82 bytes from end).
        let at = b.len() - from_end;
        assert_eq!(b[at], 0);
        b[at] = 1;
        let d = f.design();
        if id == 41 {
            assert!(d.sketch_points().unwrap().iter().find(|p| p.id == id).unwrap().linked);
        } else if polygon {
            assert!(d.sketch_lines().unwrap()[0].reference);
        } else {
            assert!(d.sketch_circles().unwrap()[0].reference);
        }
        assert!(d.to_document("Keep links").unwrap_err().to_string().contains("linked sketch source"));
    }
}
