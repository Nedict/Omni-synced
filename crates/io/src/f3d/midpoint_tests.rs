use super::geometric_tests::relation;
use super::import_tests::{Fixture, volume};
use super::polygon_tests::rectangle;
use super::relationship_tests::{reference, refs, u64, wide};
use solidcraft_doc::sketch::{CurveKind, Sketch};
use solidcraft_geom::Vec2;

fn fixture() -> Fixture {
    let mut f = rectangle();
    for (id, connection, y) in [(301, 311, -1_f64), (302, 312, 1.)] {
        let mut b = vec![0; 6];
        reference(&mut b, connection);
        b.extend([0; 8]);
        for n in [1.5_f64, y, 0.] {
            b.extend(n.to_le_bytes());
        }
        b.extend([0; 30]);
        refs(&mut b, &[connection]);
        reference(&mut b, 70);
        f.put(id, "C2CEDAE7-1716-47C1-B7B1-07B70081D0FB", 11, b, None);
        f.put(connection, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0, vec![], None);
    }
    let mut b = vec![0; 6];
    for n in [1.5_f64, -1., 0., 0., 2., 0., 0., 1., 0., 0., 0., 1.] {
        b.extend(n.to_le_bytes());
    }
    reference(&mut b, 302);
    reference(&mut b, 301);
    let mut tail = [0; 42];
    tail[7] = 1; // Original construction line, excluded from profiles.
    b.extend(tail);
    reference(&mut b, 70);
    f.put(303, "DCA267ED-D615-4934-B64F-AD805E8003E2", 2, b, None);
    relation(&mut f, 304, 0x1000, &[31, 303]);
    let mut b = vec![0; 6];
    wide(&mut b, "Original midpoint relationship");
    u64(&mut b, 0);
    b.push(1);
    refs(&mut b, &[55, 58, 59, 60, 304]);
    b.push(1);
    refs(&mut b, &[56, 57]);
    b.extend([0; 17]);
    reference(&mut b, 8);
    b.extend([0; 4]);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 6);
    secondary.extend([0; 17]);
    wide(&mut secondary, "Rectangle and reference line");
    secondary.push(0);
    refs(&mut secondary, &[42, 41, 71, 72, 31, 32, 33, 34, 56, 57, 301, 302, 303]);
    reference(&mut secondary, 3);
    f.put(70, "44A64366-4BD3-4B24-881A-F94C206E8F2D", 18, b, Some(secondary));
    f
}

#[test]
fn line_midpoint_records_follow_dimension_edits_and_reopening() {
    for operands in [[31, 303], [303, 31]] {
        let mut f = fixture();
        relation(&mut f, 304, 0x1000, &operands);
        let mut doc = f.design().to_document("Common line centers").unwrap();
        for width in [30., 45., 20.] {
            doc.change_param("width", &format!("{width} mm"), None, None).unwrap();
            volume(&doc, width * 20. * 17.);
            let mut model = solidcraft_doc::Model::new();
            model.evaluate(&doc);
            let state = model.state();
            let sk = &state.sketches[0].sketch;
            let CurveKind::Line { a, b } = sk.curves[sk.curve_index("c303").unwrap()].kind else { panic!("line") };
            let center = (sk.points[a].pos + sk.points[b].pos) * 0.5;
            assert!((center - Vec2::new(width / 2., 0.)).len() < 1e-6);
            assert!(!sk.points.iter().find(|p| p.id == "midpoint304").unwrap().fixed);
            volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), width * 20. * 17.);
        }
    }
    let n = fixture().0[&304].2.len();
    for keep in 0..n {
        let mut f = fixture();
        f.0.get_mut(&304).unwrap().2.truncate(keep);
        assert!(f.design().to_document("Truncated midpoint").is_err());
    }
    for operands in [[31, 31], [31, 999], [31, 70]] {
        let mut f = fixture();
        relation(&mut f, 304, 0x1000, &operands);
        assert!(f.design().to_document("Invalid midpoint").is_err());
    }
}

#[test]
fn common_line_midpoints_allow_rotation_and_follow_moving_centers() {
    let mut sk = Sketch::new();
    let outer = sk.add_line(Vec2::new(-5., 0.), Vec2::new(5., 0.), None, None, None).unwrap();
    let inner = sk.add_line(Vec2::new(-2., -3.), Vec2::new(2., 3.), None, None, None).unwrap();
    super::import::line_midpoints(&mut sk, [outer, inner], 1).unwrap();
    let CurveKind::Line { a: oa, b: ob } = sk.curves[outer].kind else { panic!("line") };
    let CurveKind::Line { a, b } = sk.curves[inner].kind else { panic!("line") };
    for p in [oa, ob, a] {
        sk.points[p].fixed = true;
    }
    sk.points[a].pos = Vec2::new(4., 1.);
    assert!(solidcraft_doc::sketch::solve(&mut sk).max_residual < 1e-6);
    assert!((sk.points[b].pos - Vec2::new(-4., -1.)).len() < 1e-6);
    for p in [oa, ob] {
        sk.points[p].pos += Vec2::new(10., 7.);
    }
    assert!(solidcraft_doc::sketch::solve(&mut sk).max_residual < 1e-6);
    assert!((sk.points[b].pos - Vec2::new(16., 13.)).len() < 1e-6);
    let mut inconsistent = sk.clone();
    inconsistent.points[b].pos += Vec2::X;
    assert!(super::import::line_midpoints(&mut inconsistent, [outer, inner], 2).is_err());
    assert!(super::import::line_midpoints(&mut sk, [outer, usize::MAX], 2).is_err());
}
