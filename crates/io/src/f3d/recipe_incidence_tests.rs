use super::*;
use crate::f3d::import_tests::Fixture;
use crate::f3d::relationship_tests::{string, u32, u64};

fn ints(bytes: &mut Vec<u8>, values: &[i32]) {
    for value in values {
        u32(bytes, *value as u32);
    }
}

fn authored() -> Vec<u8> {
    let mut bytes = vec![0; 6];
    ints(&mut bytes, &[1, 3, 6]);
    for term in [vec![(9001, 2), (9002, 1)], vec![(9002, 9)], vec![(9002, 11)], vec![(9002, 13)], vec![(9002, 15)], vec![(9001, -7)]] {
        u32(&mut bytes, term.len() as u32);
        for (op, label) in term {
            string(&mut bytes, &label.to_string());
            ints(&mut bytes, &[0, 1]);
            u64(&mut bytes, op);
        }
    }
    u32(&mut bytes, 0);
    string(&mut bytes, "bounded_face_recipe_data");
    ints(&mut bytes, &[0, -1, 4]);
    for side in 1..=4 {
        ints(&mut bytes, &[if side == 1 { 5 } else { -1 }, -1, 2, 0, -1, side, -1, 2]);
        for neighbor in [if side == 1 { 4 } else { side - 1 }, if side == 4 { 1 } else { side + 1 }] {
            ints(&mut bytes, &[-1, 3, 0, -1, side, -1, neighbor, -1]);
            let curved = side == 1 || neighbor == 1;
            if curved {
                ints(&mut bytes, &[1, 5, -1, 0, 0, -1, 1]);
                // Original opaque endpoint signatures, stable across the two
                // incident edge recipes despite different face term order.
                ints(&mut bytes, &[side.max(neighbor), 3, 1, 1, 2, 2, 1, 3]);
            } else {
                ints(&mut bytes, &[0, 0]);
            }
        }
        ints(&mut bytes, &[-1]);
    }
    bytes
}

fn parse(bytes: Vec<u8>) -> Result<Identity> {
    parse_kind(bytes, true)
}

fn parse_kind(bytes: Vec<u8>, bounded: bool) -> Result<Identity> {
    let mut fixture = Fixture(BTreeMap::new());
    fixture.put(82, "7ACC2A03-0261-4879-A14A-A93D661A5BDC", 3, bytes, None);
    let design = fixture.design();
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    identity(&Records::new(&design), &classes, 82, bounded)
}

#[test]
fn tracking_qualifiers_require_explicit_retirement_and_remain_recorded() {
    let mut bytes = closed(false);
    let mut qualified = Vec::new();
    ints(&mut qualified, &[2, 8101, -9003, 0]);
    bytes.splice(31..43, qualified);
    let parse = |bytes: Vec<u8>, retired: BTreeSet<u64>| {
        let mut fixture = Fixture(BTreeMap::new());
        fixture.put(82, "7ACC2A03-0261-4879-A14A-A93D661A5BDC", 3, bytes, None);
        let design = fixture.design();
        let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        tracked_identity(&Records::new(&design), &classes, 82, &retired)
    };
    let decoded = parse(bytes.clone(), BTreeSet::from([9003])).unwrap();
    assert_eq!(decoded.retired_modifiers, BTreeSet::from([(8101, 5, 9003)]));
    assert_eq!(decoded.edges, parse_kind(closed(false), false).unwrap().edges);
    assert!(parse_kind(bytes.clone(), false).is_err(), "current selectors cannot silently ignore qualifiers");
    assert!(parse(bytes.clone(), BTreeSet::new()).is_err());
    assert!(parse(bytes.clone(), BTreeSet::from([9004])).is_err());
    for modifier in [0_i32, 9003, i32::MIN, -8101] {
        let mut bad = bytes.clone();
        bad[39..43].copy_from_slice(&modifier.to_le_bytes());
        assert!(parse(bad, BTreeSet::from([9003, 8101, 2147483648])).is_err());
    }
    for keep in 0..bytes.len() {
        assert!(parse(bytes[..keep].to_vec(), BTreeSet::from([9003])).is_err(), "truncated at {keep}");
    }
}

fn closed(seam: bool) -> Vec<u8> {
    let mut bytes = vec![0; 6];
    ints(&mut bytes, &[1, 3, if seam { 3 } else { 2 }]);
    for (op, label) in [(8101, 5), (8202, 1), (8101, -3)].into_iter().take(if seam { 3 } else { 2 }) {
        ints(&mut bytes, &[1]);
        string(&mut bytes, &label.to_string());
        ints(&mut bytes, &[0, 1]);
        u64(&mut bytes, op);
    }
    ints(&mut bytes, &[0]);
    string(&mut bytes, "edge_recipe_data");
    ints(&mut bytes, &[if seam { 2 } else { -1 }, -1, 2, 0, -1, 1, -1, 1, -1, 2, 0, -1, 1, -1]);
    if seam {
        ints(&mut bytes, &[1, 2, -1, 0, 0, -1]);
    } else {
        ints(&mut bytes, &[0]);
    }
    ints(&mut bytes, &[2, 9, 2, 3, 4, 5, 6, 7, 8, 10, 2, 3, 4, 5, 6, 7, 8, -1]);
    bytes
}

#[test]
fn closed_edge_retains_its_single_seam_vertex_and_intersection_signatures() {
    for seam in [false, true] {
        let bytes = closed(seam);
        let decoded = parse_kind(bytes.clone(), false).unwrap();
        let edge = &decoded.edges[0];
        assert_eq!(edge.ends.len(), 1);
        assert_eq!(edge.ends[0].faces, edge.faces.iter().cloned().collect());
        assert_eq!(edge.ends[0].seams.len(), usize::from(seam));
        assert_eq!(edge.ends[0].signatures, [[9, 2, 3, 4, 5, 6, 7, 8], [10, 2, 3, 4, 5, 6, 7, 8]]);
        assert!(cycle(&vec![edge.clone(); 3]).is_err(), "closed carriers cannot masquerade as a multi-edge loop");
        for keep in 0..bytes.len() {
            assert!(parse_kind(bytes[..keep].to_vec(), false).is_err(), "closed={seam}, truncated at {keep}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(parse_kind(trailing, false).is_err());
        let header = bytes.windows(16).position(|w| w == b"edge_recipe_data").unwrap() + 16;
        for (offset, value) in [(28, 0_u32), (28, 3), (36, 3), (44, 0), (52, 0), (0, 0)] {
            let mut invalid = bytes.clone();
            invalid[header + offset..header + offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(parse_kind(invalid, false).is_err(), "invalid closed edge field {offset}");
        }
    }
}

#[test]
fn single_edge_bounded_faces_require_the_complete_closed_carrier() {
    for seam in [false, true] {
        let original = closed(seam);
        let at = original.windows(16).position(|w| w == b"edge_recipe_data").unwrap() - 4;
        let mut bytes = original[..at].to_vec();
        string(&mut bytes, "bounded_face_recipe_data");
        ints(&mut bytes, &[0, -1, 1]);
        bytes.extend(&original[at + 20..]);
        let decoded = parse(bytes.clone()).unwrap();
        assert_eq!(decoded.edges, parse_kind(original, false).unwrap().edges);
        assert_eq!(decoded.edges[0].ends.len(), 1);
        for keep in 0..bytes.len() {
            assert!(parse(bytes[..keep].to_vec()).is_err());
        }
        let header = at + 4 + "bounded_face_recipe_data".len();
        for count in [0_u32, 2, 129] {
            let mut bad = bytes.clone();
            bad[header + 8..header + 12].copy_from_slice(&count.to_le_bytes());
            assert!(parse(bad).is_err());
        }
    }
}

#[test]
#[ignore = "requires the local Style 5 archive"]
fn native_joined_fillet_edges_preserve_open_and_closed_incidence() {
    let bytes = std::fs::read(std::env::var("SOLIDCRAFT_COMPOSITE_ARCHIVE").unwrap()).unwrap();
    let designs = read_f3d(&bytes).unwrap();
    let design = &designs[0];
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    for (id, endpoints) in [(2713, 2), (2717, 2), (2721, 1), (2725, 1), (2729, 2), (1135, 1)] {
        let decoded = identity(&Records::new(design), &classes, id, false).unwrap();
        assert_eq!(decoded.edges.len(), 1);
        assert_eq!(decoded.edges[0].ends.len(), endpoints);
        println!("IDENTITY {id}: {:?}", decoded.edges);
    }
    let mut doc = import_f3d_with_preference(&bytes, "Incidence", ImportPreference::Editability).unwrap().document;
    let end = doc.features.iter().position(|f| f.name == "Extrude6").unwrap();
    doc.features.truncate(end + 1);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let history = design.history().unwrap();
    let features = history.iter().filter_map(|native| doc.find_feature(&native.name).map(|f| (native.id, f.id))).collect();
    let operations = design.operation_owners().unwrap();
    let all_profiles = design.sketch_profiles().unwrap();
    let profiles = all_profiles.iter().map(|p| (p.feature, p)).collect();
    let lines = design.sketch_lines().unwrap().into_iter().map(|line| (line.id, line)).collect();
    let circles = design.sketch_circles().unwrap().into_iter().map(|circle| (circle.id, circle)).collect();
    let records = Records::new(design);
    let state = model.state();
    let context = super::super::composite_blends::Context {
        records: &records,
        classes: &classes,
        operations: &operations,
        features: &features,
        profiles: &profiles,
        lines: &lines,
        circles: &circles,
        doc: &doc,
        state: &state,
        model: &model,
        before: None,
        depth: 0,
    };
    let mut names = BTreeSet::new();
    for (selector, count) in [(2710, 1), (2714, 1), (2718, 2), (2722, 2), (2726, 1)] {
        let selected = context.resolve(selector, 1278).unwrap_or_else(|e| panic!("selector {selector}: {e}"));
        assert_eq!(selected.len(), count);
        for edge in selected {
            println!("SELECTED {selector}: {}", edge.name);
            assert!(names.insert(edge.name));
        }
    }
    assert_eq!(names.len(), 7, "all five native selections, including both split closed rings");
}

#[test]
fn finite_boundary_preserves_merged_faces_and_shared_intersection_identity() {
    let decoded = parse(authored()).unwrap();
    assert_eq!(decoded.edges.len(), 4);
    assert_eq!(decoded.terms[0], BTreeSet::from([(9001, 2), (9002, 1)]));
    assert_eq!(decoded.edges[0].ends[1], decoded.edges[1].ends[0]);
    let mut edges = decoded.edges.clone();
    edges[1].ends[0].signatures[0][0] += 1;
    assert!(cycle(&edges).is_err(), "different intersection branches must not be welded");
    edges = decoded.edges;
    edges.push(edges[0].clone());
    assert!(cycle(&edges).is_err(), "duplicate edges are not a closed boundary");
}

#[test]
fn boundary_generations_preserve_a_complete_incidence_bijection() {
    let original = parse(authored()).unwrap().edges;
    let mut later = original.clone();
    for edge in &mut later {
        edge.faces.swap(0, 1);
        edge.ends.reverse();
        for vertex in &mut edge.ends {
            for signature in &mut vertex.signatures {
                signature[0] += 100;
            }
        }
    }
    later.reverse();
    assert!(same_boundary(&original, &later).unwrap(), "counter changes and storage order do not change the closed graph");

    let mut broken = later.clone();
    let vertex = broken.iter_mut().flat_map(|e| &mut e.ends).find(|v| !v.signatures.is_empty()).unwrap();
    vertex.signatures[0][0] += 1;
    assert!(same_boundary(&original, &broken).is_err(), "signatures must still agree at shared endpoints within each generation");
    let mut duplicate = later.clone();
    duplicate.push(later[0].clone());
    assert!(same_boundary(&original, &duplicate).is_err());
    assert!(same_boundary(&original, &later[..later.len() - 1]).is_err());

    // Another producer with the same combinatorial shape is not this boundary.
    let rename = |term: &Term| term.iter().map(|(op, role)| (op + 1, *role)).collect::<Term>();
    for edge in &mut later {
        edge.faces = edge.faces.each_ref().map(rename);
        for vertex in &mut edge.ends {
            vertex.faces = vertex.faces.iter().map(rename).collect();
            vertex.seams = vertex.seams.iter().map(rename).collect();
        }
    }
    assert!(!same_boundary(&original, &later).unwrap());
}

#[test]
fn truncated_oversized_and_invalid_incidence_records_are_refused() {
    let bytes = authored();
    for end in 0..bytes.len() {
        assert!(parse(bytes[..end].to_vec()).is_err(), "truncated at {end}");
    }
    let mut extra = bytes.clone();
    extra.extend([0; 4]);
    assert!(parse(extra).is_err());
    for offset in [6, 10, 14, 18] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(bad).is_err());
    }
    let mut bad = parse(bytes).unwrap().edges;
    bad[0].ends[0] = bad[0].ends[1].clone();
    assert!(cycle(&bad).is_err());
}

#[test]
fn asymmetric_incidence_retains_endpoint_cycles_without_affecting_unoriented_topology() {
    let face = |label| Term::from([(9001, label), (9002, label)]);
    let faces = [face(2), face(9)];
    for reverse in [false, true] {
        let mut a = vec![face(2), face(9), face(11)];
        let mut b = vec![face(9), face(2), face(13)];
        if reverse {
            a.reverse();
            b.reverse();
        }
        let expected = if reverse { &faces[0] } else { &faces[1] };
        for i in 0..3 {
            for j in 0..3 {
                let mut a = a.clone();
                a.rotate_left(i);
                let mut b = b.clone();
                b.rotate_left(j);
                assert_eq!(primary_face(&faces, &[a.clone(), b.clone()]).as_ref(), Some(expected));
                b.reverse();
                assert!(primary_face(&faces, &[a.clone(), b]).is_none());
                a.push(face(15));
                assert!(primary_face(&faces, &[a, vec![face(9), face(2), face(13)]]).is_none());
            }
        }
    }
    assert!(primary_face(&faces, &[]).is_none());
    assert!(primary_face(&faces, &[faces.to_vec()]).is_none());
}
