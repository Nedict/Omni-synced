use super::import_tests::{Fixture, common, volume};
use super::polygon_tests::rectangle;
use super::relationship_tests::{reference, refs, u32, u64, wide};
use solidcraft_doc::sketch::{ConstraintKind, Sketch};
use solidcraft_geom::Vec2;

fn angle_record(quadrants: [u32; 2]) -> Vec<u8> {
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 3);
    for (id, slot) in [(0, 6), (31, 17), (32, 0)] {
        reference(&mut b, id);
        u32(&mut b, slot);
    }
    b.push(0);
    b.extend(std::f64::consts::FRAC_PI_2.to_le_bytes());
    u32(&mut b, quadrants[0]);
    b.extend([0; 2]);
    u32(&mut b, quadrants[1]);
    b.extend([0; 219]);
    wide(&mut b, "");
    wide(&mut b, "90 deg");
    b.extend([0; 24]);
    u64(&mut b, 0x8000);
    reference(&mut b, 301);
    refs(&mut b, &[32, 31]);
    b.push(0);
    b
}

fn prism(quadrants: [u32; 2], version: u32) -> Fixture {
    let mut f = rectangle();
    // Keep the side length, allowing the original rectangle to shear into a
    // parallelogram when the new angle changes.
    let b = &mut f.0.get_mut(&57).unwrap().2;
    let n = b.len();
    b[n - 46..n - 38].copy_from_slice(&0x4000_u64.to_le_bytes());
    super::geometric_tests::relation(&mut f, 59, 16, &[32, 34]);
    f.0.remove(&60);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 70);
    secondary.extend([0; 18]);
    reference(&mut secondary, 70);
    f.put(300, "6EEAFDE5-59C4-424D-8EBE-77CDBC562017", version, angle_record(quadrants), Some(secondary));
    let mut b = vec![0; 20];
    u32(&mut b, 0);
    reference(&mut b, 301);
    wide(&mut b, "90 deg");
    b.extend([0; 10]);
    for s in ["Angular Dimension-3", "", "deg", "angle"] {
        wide(&mut b, s);
    }
    b.extend(std::f64::consts::FRAC_PI_2.to_le_bytes());
    b.extend([0; 12]);
    f.put(302, "7A6A3D31-BE74-4E19-9115-20642944C2E4", 9, b, None);
    let mut b = vec![0; 4];
    b.push(1);
    refs(&mut b, &[8]);
    b.extend([0; 5]);
    b.extend(std::f64::consts::FRAC_PI_2.to_le_bytes());
    reference(&mut b, 302);
    f.put(301, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0, b, None);
    let mut b = vec![0; 7];
    refs(&mut b, &[61, 75, 301]);
    u32(&mut b, 1);
    b.push(0);
    reference(&mut b, 70);
    reference(&mut b, 6);
    b.extend([0; 12]);
    common(&mut b, "Sketch", false);
    f.put(8, "8DA771B7-52ED-42FC-94E2-2353FE141373", 5, b, None);
    let mut b = vec![0; 6];
    wide(&mut b, "Original angular prism");
    u64(&mut b, 0);
    b.push(1);
    refs(&mut b, &[55, 58, 59]);
    b.push(1);
    refs(&mut b, &[56, 57, 300]);
    b.extend([0; 17]);
    reference(&mut b, 8);
    b.extend([0; 4]);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 6);
    secondary.extend([0; 17]);
    wide(&mut secondary, "Angular parallelogram");
    secondary.push(0);
    refs(&mut secondary, &[42, 41, 71, 72, 31, 32, 33, 34, 56, 57, 300]);
    reference(&mut secondary, 3);
    f.put(70, "44A64366-4BD3-4B24-881A-F94C206E8F2D", 18, b, Some(secondary));
    f
}

#[test]
fn angular_quadrants_keep_distinct_edit_directions_at_ninety_degrees() {
    for (quadrants, version) in [([0, 0], 1), ([1, 1], 1), ([2, 2], 1), ([3, 3], 1), ([1, 3], 0)] {
        let mut doc = prism(quadrants, version).design().to_document("Original angular prism").unwrap();
        volume(&doc, 30. * 20. * 17.);
        doc.change_param("angle", "60 deg", None, None).unwrap();
        volume(&doc, 30. * 20. * 17. * 3_f64.sqrt() / 2.);
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        let state = model.state();
        let sketch = &state.sketches[0].sketch;
        let x = sketch.points.iter().find(|p| p.id == "p71").unwrap().pos.x;
        assert!((x - if quadrants[0] % 2 == 0 { 40. } else { 20. }).abs() < 1e-6);
        let mut reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        reopened.change_param("angle", "120 deg", None, None).unwrap();
        volume(&reopened, 30. * 20. * 17. * 3_f64.sqrt() / 2.);
    }
}

#[test]
fn angular_records_reject_corrupt_values_quadrants_owners_and_truncation() {
    let n = angle_record([0, 0]).len();
    for keep in 0..n {
        let mut f = prism([0, 0], 1);
        f.0.get_mut(&300).unwrap().2.truncate(keep);
        assert!(f.design().to_document("Truncated angle").is_err(), "{keep}");
    }
    for quadrants in [[0, 1], [4, 4], [u32::MAX, 0]] {
        assert!(prism(quadrants, 1).design().sketch_constraints().is_err());
    }
    assert!(prism([0, 0], 2).design().sketch_constraints().is_err());
    for value in [f64::NAN, f64::INFINITY, -1., 7., 1.] {
        let mut f = prism([0, 0], 1);
        f.0.get_mut(&300).unwrap().2[55..63].copy_from_slice(&value.to_le_bytes());
        assert!(f.design().to_document("Bad angle value").is_err());
    }
    for (secondary, offset) in [(false, n - 37), (true, 5)] {
        let mut f = prism([0, 0], 1);
        let record = f.0.get_mut(&300).unwrap();
        let data = if secondary { record.3.as_mut().unwrap() } else { &mut record.2 };
        data[offset..offset + 8].copy_from_slice(&999_u64.to_le_bytes());
        assert!(f.design().sketch_constraints().is_err());
    }
}

#[test]
fn angular_direction_mapping_preserves_clockwise_and_reversed_lines() {
    for (theta, quadrant) in [(60_f64, 0), (-60., 0), (120., 1), (-120., 3), (180., 0), (180., 1)] {
        let mut s = Sketch::new();
        let a = s.add_line(Vec2::ZERO, Vec2::new(30., 0.), None, None, None).unwrap();
        let radians = theta.to_radians();
        let b = s.add_line(Vec2::ZERO, Vec2::new(radians.cos(), radians.sin()) * 20., None, None, None).unwrap();
        let value = if theta == 180. { if quadrant == 0 { std::f64::consts::PI } else { 0. } } else { std::f64::consts::PI / 3. };
        let kind = super::import::angle_constraint(&s, a, b, value, quadrant).unwrap();
        assert!(matches!(kind, ConstraintKind::Angle { flip, .. } if flip == (quadrant % 2 == 1)));
        s.add_constraint(kind, None).unwrap();
        assert!(solidcraft_doc::sketch::solve(&mut s).max_residual < 1e-6);
    }
}

#[test]
fn fixed_native_line_support_keeps_endpoint_length_editable() {
    let mut f = rectangle();
    let b = &mut f.0.get_mut(&31).unwrap().2;
    let n = b.len();
    b[n - 51] = 1;
    let mut doc = f.design().to_document("Fixed line support").unwrap();
    volume(&doc, 30. * 20. * 17.);
    let sketch = doc.sketch(doc.features[0].id).unwrap();
    let line = sketch.curve_index("c31").unwrap();
    assert!(!sketch.curves[line].fixed);
    assert!(sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::LineAngle { l, .. } if l == line)));
    doc.change_param("width", "40 mm", None, None).unwrap();
    volume(&doc, 40. * 20. * 17.);
    volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), 40. * 20. * 17.);
}
