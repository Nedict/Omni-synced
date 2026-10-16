use super::import_tests::{Fixture, fixture, volume};
use super::relationship_tests::{reference, refs, u32, u64, wide};
use super::*;
use solidcraft_doc::sketch::{ConstraintKind, Sketch};
use solidcraft_geom::Vec2;

fn relation(f: &mut Fixture, operands: &[u64], ordered: &[u64]) {
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, operands.len() as u32);
    for (slot, id) in operands.iter().enumerate() {
        reference(&mut b, *id);
        u32(&mut b, slot as u32 + 7);
    }
    b.push(0);
    reference(&mut b, 70);
    u64(&mut b, 0x400);
    refs(&mut b, ordered);
    b.push(0);
    f.put(306, "60403D47-0C49-49B0-BDE8-1679608164A2", 3, b, None);
}

fn circles() -> Fixture {
    let mut f = fixture(false);
    for (id, connection, position, fixed) in [(301, 201, [8_f64, 0., 0.], false), (302, 202, [4., -5., 0.], true), (303, 203, [4., 5., 0.], true)] {
        let mut b = vec![0; 6];
        reference(&mut b, connection);
        b.extend([0; 3]);
        b.extend([u8::from(fixed), 1, 0, 0, 0]);
        for x in position {
            b.extend(x.to_le_bytes());
        }
        b.extend([0; 30]);
        refs(&mut b, &[connection]);
        reference(&mut b, 70);
        f.put(id, entity::POINT, 11, b, None);
        f.put(connection, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0, vec![], None);
    }
    let mut b = vec![0; 6];
    for x in [4_f64, -5., 0., 0., 10., 0., 0., 1., 0., 0., 0., 1.] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 303);
    reference(&mut b, 302);
    b.extend([0, 0, 1, 1, 0, 0, 0, 1]);
    b.extend([0; 34]);
    reference(&mut b, 70);
    f.put(304, entity::LINE, 2, b, None);
    let mut b = vec![0; 6];
    for x in [8_f64, 0., 0., 0., 0., 1., 1., 0., 0., 3., 0., std::f64::consts::TAU] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 301);
    b.extend([0; 44]);
    reference(&mut b, 70);
    f.put(305, "F0130424-8B7E-4092-93C9-1CA807482534", 0, b, None);
    relation(&mut f, &[304, 31, 305], &[31, 305, 304]);
    let mut primary = vec![0; 6];
    wide(&mut primary, "Original symmetry");
    u64(&mut primary, 0);
    primary.push(1);
    refs(&mut primary, &[55, 306]);
    primary.push(1);
    refs(&mut primary, &[56]);
    primary.extend([0; 17]);
    reference(&mut primary, 8);
    primary.extend([0; 4]);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 6);
    secondary.extend([0; 17]);
    wide(&mut secondary, "Symmetric circles");
    secondary.push(0);
    refs(&mut secondary, &[42, 41, 31, 56, 301, 302, 303, 304, 305]);
    reference(&mut secondary, 3);
    f.put(70, "44A64366-4BD3-4B24-881A-F94C206E8F2D", 18, primary, Some(secondary));
    f
}

#[test]
fn native_circle_symmetry_keeps_equal_radius_during_parameter_edits() {
    let mut doc = circles().design().to_document("Original circle symmetry").unwrap();
    volume(&doc, 900. * std::f64::consts::PI * 17.);
    let sketch = doc.sketch(doc.features[0].id).unwrap();
    assert!(sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Symmetric { .. })));
    assert!(sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Equal { .. })));
    doc.change_param("d7", "40 mm", None, None).unwrap();
    volume(&doc, 400. * std::f64::consts::PI * 17.);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    let state = model.state();
    let sketch = &state.sketches[0].sketch;
    assert!((sketch.radius(sketch.curve_index("c305").unwrap()).unwrap() - 20.).abs() < 1e-6);
    assert_eq!(crate::read_design(&crate::write_design(&doc)).unwrap(), doc);
    let size = circles().0[&306].2.len();
    for n in 0..size {
        let mut f = circles();
        f.0.get_mut(&306).unwrap().2.truncate(n);
        assert!(f.design().to_document("Truncated symmetry").is_err(), "{n}");
    }
    for ordered in [vec![31, 304, 305], vec![31, 31, 304], vec![31, 305, 999], vec![31, 305]] {
        let mut f = circles();
        relation(&mut f, &[304, 31, 305], &ordered);
        assert!(f.design().sketch_constraints().is_err());
    }
}

#[test]
fn symmetry_roles_are_independent_of_graph_registration_order() {
    for operands in [[304, 31, 305], [304, 305, 31], [31, 304, 305], [31, 305, 304], [305, 304, 31], [305, 31, 304]] {
        for entities in [[31, 305], [305, 31]] {
            let mut f = circles();
            relation(&mut f, &operands, &[entities[0], entities[1], 304]);
            let design = f.design();
            assert!(
                design.sketch_constraints().unwrap()[0]
                    .constraints
                    .iter()
                    .any(|c| { matches!(c, SketchConstraint::Symmetry { axis: 304, entities: pair, .. } if *pair == entities) })
            );
            let mut doc = design.to_document("Ordered symmetry roles").unwrap();
            doc.change_param("d7", "40 mm", None, None).unwrap();
            volume(&doc, 400. * std::f64::consts::PI * 17.);
            let mut model = solidcraft_doc::Model::new();
            model.evaluate(&doc);
            let state = model.state();
            let sketch = &state.sketches[0].sketch;
            assert!((sketch.radius(sketch.curve_index("c305").unwrap()).unwrap() - 20.).abs() < 1e-6);
            assert_eq!(crate::read_design(&crate::write_design(&doc)).unwrap(), doc);
        }
    }
}

#[test]
fn line_symmetry_uses_reflected_endpoint_incidence_in_both_orientations() {
    for reverse in [false, true] {
        let mut s = Sketch::new();
        let axis = s.add_line(Vec2::new(0., -5.), Vec2::new(0., 5.), None, None, None).unwrap();
        let a = s.add_line(Vec2::new(-2., 0.), Vec2::new(-3., 2.), None, None, None).unwrap();
        let ends = if reverse { [Vec2::new(3., 2.), Vec2::new(2., 0.)] } else { [Vec2::new(2., 0.), Vec2::new(3., 2.)] };
        let b = s.add_line(ends[0], ends[1], None, None, None).unwrap();
        let curves = BTreeMap::from([(10, axis), (11, a), (12, b)]);
        let constraints = super::import::symmetry(&s, &BTreeMap::new(), &curves, 10, [11, 12]).unwrap();
        assert_eq!(constraints.len(), 2);
        for c in constraints {
            s.add_constraint(c, None).unwrap();
        }
        assert!(solidcraft_doc::sketch::solve(&mut s).max_residual < 1e-6);
    }
}

#[test]
fn legacy_constraint_graph_versions_preserve_references_and_reject_damaged_lists() {
    for version in [16, 17, 18] {
        let mut f = fixture(false);
        let mut b = vec![0; 6];
        if version == 18 {
            wide(&mut b, "Original graph");
        }
        u64(&mut b, 0);
        b.push(1);
        refs(&mut b, &[55]);
        if version >= 17 {
            b.push(1);
        }
        refs(&mut b, &[56]);
        b.extend([0; 11]);
        refs(&mut b, &[6]);
        reference(&mut b, 6);
        b.push(1);
        reference(&mut b, 8);
        b.push(0);
        if version < 18 {
            wide(&mut b, "Original external identity");
        }
        b.extend([0, 1, 1]);
        let n = b.len();
        f.0.get_mut(&70).unwrap().1 = version;
        f.0.get_mut(&70).unwrap().2 = b.clone();
        let mut d = f.design();
        d.layout = if version == 16 {
            4
        } else if version == 17 {
            5
        } else {
            8
        };
        let g = d.sketch_constraint_graphs().unwrap().remove(0);
        assert_eq!((g.geometry, g.feature), (70, 8));
        assert_eq!(g.relations, [55]);
        assert_eq!(g.dimensions, [56]);
        assert_eq!(g.auxiliary, [6]);
        assert_eq!(g.support, Some(6));
        assert!(d.sketch_constraints().is_err());
        for keep in 0..n {
            let mut f = fixture(false);
            f.0.get_mut(&70).unwrap().1 = version;
            f.0.get_mut(&70).unwrap().2 = b[..keep].to_vec();
            assert!(f.design().sketch_constraint_graphs().is_err(), "{version}/{keep}");
        }
    }
}
