use super::import_tests::{Fixture, volume};
use super::polygon_tests::rectangle;
use super::relationship_tests::{reference, refs, u32, u64, wide};
use solidcraft_doc::sketch::ConstraintKind;

fn offset(entities: [u64; 2], flags: u64) -> Fixture {
    let mut f = rectangle();
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 3);
    for (id, slot) in [(0, 5), (entities[0], 17), (entities[1], 0)] {
        reference(&mut b, id);
        u32(&mut b, slot);
    }
    b.push(0);
    b.extend([0; 220]);
    wide(&mut b, "");
    wide(&mut b, "20 mm");
    b.extend([0; 44]);
    u64(&mut b, flags);
    reference(&mut b, 75);
    refs(&mut b, &[entities[1], entities[0]]);
    b.push(0);
    f.0.get_mut(&57).unwrap().2 = b;
    f
}

#[test]
fn offset_dimensions_reconstruct_editable_point_line_and_parallel_line_distances() {
    for entities in [[31, 71], [71, 31], [31, 33], [33, 31]] {
        let mut doc = offset(entities, 0x4000).design().to_document("Original offset prism").unwrap();
        volume(&doc, 30. * 20. * 17.);
        let sketch = doc.sketch(doc.features[0].id).unwrap();
        assert!(
            sketch
                .constraints
                .iter()
                .any(|c| c.param.as_deref() == Some("heightSketch") && matches!(c.kind, ConstraintKind::PointLineDistance { .. }))
        );
        doc.change_param("heightSketch", "width + 5 mm", None, None).unwrap();
        volume(&doc, 30. * 35. * 17.);
        let mut reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(reopened, doc);
        reopened.change_param("width", "40 mm", None, None).unwrap();
        volume(&reopened, 40. * 45. * 17.);
    }
}

#[test]
fn offset_dimension_records_reject_wrong_types_flags_and_every_truncation() {
    let n = offset([31, 33], 0x4000).0[&57].2.len();
    for keep in 0..n {
        let mut f = offset([31, 33], 0x4000);
        f.0.get_mut(&57).unwrap().2.truncate(keep);
        assert!(f.design().to_document("Truncated offset").is_err(), "{keep}");
    }
    for (entities, flags) in [([31, 33], 0x400000), ([31, 71], 0x800000), ([31, 31], 0x4000), ([31, 999], 0x4000), ([31, 70], 0x4000)] {
        assert!(offset(entities, flags).design().sketch_constraints().is_err());
    }
}
