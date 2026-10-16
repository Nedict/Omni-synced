use super::import_tests::{Fixture, fixture, volume};
use super::polygon_tests::rectangle;
use super::relationship_tests::{reference, refs, u32, u64};

pub(super) fn relation(f: &mut Fixture, id: u64, kind: u64, operands: &[u64]) {
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, operands.len() as u32);
    for operand in operands {
        reference(&mut b, *operand);
        u32(&mut b, 0);
    }
    b.push(1);
    u32(&mut b, 0);
    reference(&mut b, 70);
    u64(&mut b, kind);
    refs(&mut b, operands);
    b.push(0);
    f.put(id, "60403D47-0C49-49B0-BDE8-1679608164A2", 3, b, None);
}

#[test]
fn geometric_relationships_keep_a_prism_rectangular_after_dimension_edits() {
    for equal in [false, true] {
        let mut f = rectangle();
        relation(&mut f, 55, 0x40, &[42, 41]);
        if equal {
            relation(&mut f, 58, 0x800, &[31, 33]);
            relation(&mut f, 59, 0x80, &[41, 71]);
            relation(&mut f, 60, 0x80, &[42, 72]);
        } else {
            relation(&mut f, 58, 0x10, &[31, 33]);
            relation(&mut f, 59, 0x20, &[31, 32]);
            relation(&mut f, 60, 0x10, &[32, 34]);
        }
        let mut doc = f.design().to_document("Original constrained prism").unwrap();
        volume(&doc, 30. * 20. * 17.);
        doc.change_param("width", "45 mm", None, None).unwrap();
        doc.change_param("heightSketch", "25 mm", None, None).unwrap();
        volume(&doc, 45. * 25. * 17.);
        let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, 45. * 25. * 17.);
    }
}

#[test]
fn constraint_kind_arity_and_operand_classes_must_agree() {
    for (kind, operands) in [(4, vec![31, 33]), (0x10, vec![42, 41]), (0x20, vec![31]), (1, vec![31, 33]), (0x800, vec![31, 42])] {
        let mut f = rectangle();
        relation(&mut f, 55, kind, &operands);
        assert!(f.design().sketch_constraints().is_err());
    }
    // An arc midpoint is not a chord midpoint; that operand combination cannot
    // use SolidCraft's line-only midpoint constraint.
    let mut f = fixture(false);
    relation(&mut f, 55, 0x1000, &[41, 31]);
    assert!(f.design().sketch_constraints().is_err());
    let mut f = rectangle();
    relation(&mut f, 55, 0x10, &[31, 33]);
    let len = f.0[&55].2.len();
    for keep in 0..len {
        let mut f = rectangle();
        relation(&mut f, 55, 0x10, &[31, 33]);
        f.0.get_mut(&55).unwrap().2.truncate(keep);
        assert!(f.design().sketch_constraints().is_err(), "{keep}");
    }
}
