use super::import_tests::{Fixture, common, fixture, volume};
use super::relationship_tests::{reference, refs, string, u32, u64, wide};
use solidcraft_doc::FeatureKind;
use std::f64::consts::{PI, TAU};

// Original field-authored geometry; no native exported byte sequences.
fn attributes(b: &mut Vec<u8>, tag: u64) {
    b.extend([0; 4]);
    b.extend([0, 1]);
    u32(b, 2);
    for (key, value) in [("crv_primary_id", tag), ("crv_secondary_id", 2)] {
        string(b, key);
        string(b, "IntrinsicMetaTypeuint64");
        u64(b, value);
    }
}
fn round(f: &mut Fixture, id: u64, tag: u64, radius: f64, sweep: f64, axis: f64) {
    let mut b = vec![];
    attributes(&mut b, tag);
    for x in [0., 0., 0., 0., 0., 1., axis.cos(), axis.sin(), 0., radius, 0., sweep] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 41);
    if sweep != TAU {
        reference(&mut b, 72);
        reference(&mut b, 71);
    }
    b.extend(vec![0; if sweep == TAU { 44 } else { 42 }]);
    reference(&mut b, 70);
    f.put(id, "F0130424-8B7E-4092-93C9-1CA807482534", 0, b, None);
}
pub(super) fn geometry(f: &mut Fixture, entities: &[u64], dimensions: &[u64]) {
    let mut b = vec![0; 4];
    b.extend([0, 1]);
    u32(&mut b, 0);
    wide(&mut b, "original-round-profile");
    u64(&mut b, 0);
    b.push(1);
    refs(&mut b, &[55]);
    b.push(1);
    refs(&mut b, dimensions);
    b.extend([0; 17]);
    reference(&mut b, 8);
    b.extend([0; 4]);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 6);
    secondary.extend([0; 17]);
    wide(&mut secondary, "Round profile");
    secondary.push(0);
    refs(&mut secondary, entities);
    reference(&mut secondary, 3);
    f.put(70, "44A64366-4BD3-4B24-881A-F94C206E8F2D", 18, b, Some(secondary));
}
fn tracking(f: &mut Fixture, loops: &[(Vec<(u64, bool)>, bool)]) {
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    u32(&mut b, 1);
    u32(&mut b, loops.len() as u32);
    for (curves, outer) in loops {
        u32(&mut b, curves.len() as u32);
        for (tag, reversed) in curves {
            u32(&mut b, 3);
            u64(&mut b, *tag);
            u64(&mut b, 2);
            u32(&mut b, u32::from(*reversed));
            u32(&mut b, 1);
            u32(&mut b, 1);
            u64(&mut b, 0);
        }
        b.push(u8::from(*outer));
    }
    u32(&mut b, 0);
    f.put(101, "0D57BD2F-D09B-43FC-AD57-1E89A118C453", 1, b, None);
}
pub(super) fn ring() -> Fixture {
    let mut f = fixture(false);
    round(&mut f, 35, 18, 1., TAU, 0.);
    geometry(&mut f, &[42, 41, 31, 35, 56], &[56]);
    // Inner loop first deliberately proves registration order is not outer identity.
    tracking(&mut f, &[(vec![(18, true)], false), (vec![(17, false)], true)]);
    f
}
pub(super) fn arc(sweep: f64, axis: f64) -> Fixture {
    let mut f = fixture(false);
    for id in [56, 61, 62] {
        f.0.remove(&id);
    }
    let mut b = vec![0; 7];
    refs(&mut b, &[]);
    u32(&mut b, 1);
    b.push(0);
    reference(&mut b, 70);
    reference(&mut b, 6);
    b.extend([0; 12]);
    common(&mut b, "Sketch", false);
    f.put(8, "8DA771B7-52ED-42FC-94E2-2353FE141373", 5, b, None);
    let start = [3. * axis.cos(), 3. * axis.sin(), 0.];
    let end = [3. * (axis + sweep).cos(), 3. * (axis + sweep).sin(), 0.];
    for (id, connection, position) in [(71, 73, start), (72, 74, end)] {
        let mut b = vec![0; 4];
        b.extend([0, 1]);
        u32(&mut b, 0);
        reference(&mut b, connection);
        b.extend([0; 8]);
        for n in position {
            b.extend(n.to_le_bytes());
        }
        b.extend([0; 30]);
        refs(&mut b, &[connection]);
        reference(&mut b, 70);
        f.put(id, "C2CEDAE7-1716-47C1-B7B1-07B70081D0FB", 11, b, None);
        f.put(connection, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0, vec![], None);
    }
    round(&mut f, 31, 17, 3., sweep, axis);
    let delta = [start[0] - end[0], start[1] - end[1], 0.];
    let length = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
    let mut b = vec![];
    attributes(&mut b, 18);
    for n in end.into_iter().chain(delta).chain(delta.map(|n| n / length)).chain([0., 0., 1.]) {
        b.extend(n.to_le_bytes());
    }
    reference(&mut b, 71);
    reference(&mut b, 72);
    b.extend([0; 42]);
    reference(&mut b, 70);
    f.put(35, "DCA267ED-D615-4934-B64F-AD805E8003E2", 2, b, None);
    geometry(&mut f, &[42, 41, 71, 72, 31, 35], &[]);
    tracking(&mut f, &[(vec![(17, false), (18, false)], true)]);
    f
}

#[test]
fn holes_preserve_selected_boundaries_across_edits_and_reopening() {
    let d = ring().design();
    let p = d.sketch_profiles().unwrap();
    assert!(!p[0].regions[0][0].outer);
    assert!(p[0].regions[0][0].reversed[0]);
    let mut doc = d.to_document("Original annulus").unwrap();
    volume(&doc, 800. * PI * 17.);
    doc.change_param("d7", "80 mm", None, None).unwrap();
    doc.change_param("height", "23 mm", None, None).unwrap();
    volume(&doc, 1500. * PI * 23.);
    let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
    assert_eq!(doc, reopened);
    volume(&reopened, 1500. * PI * 23.);
    // Removing an explicitly selected hole must not silently turn it into a disk.
    let FeatureKind::Sketch { sketch, .. } = &mut doc.features[0].kind else { panic!("sketch") };
    sketch.curves.iter_mut().find(|c| c.id == "c35").unwrap().construction = true;
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().any(|r| r.error.is_some()));
}

#[test]
fn arcs_preserve_major_and_rotated_sweeps_and_shared_chord_points() {
    for (sweep, axis) in [(PI / 2., 0.), (PI, 0.), (PI * 1.5, 0.), (PI, PI)] {
        let d = arc(sweep, axis).design();
        assert!(d.sketch_circles().unwrap().is_empty());
        assert_eq!(d.sketch_arcs().unwrap().len(), 1);
        let mut doc = d.to_document("Original arc segment").unwrap();
        let area = 900. * (sweep - sweep.sin()) / 2.;
        volume(&doc, area * 17.);
        doc.change_param("height", "23 mm", None, None).unwrap();
        volume(&doc, area * 23.);
        let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, area * 23.);
    }
}

#[test]
fn rotated_arc_roundoff_preserves_independent_endpoints_but_rejects_real_disagreement() {
    let mut f = arc(PI, PI / 3.);
    round(&mut f, 31, 17, 3., PI - 2.1e-8, PI / 3.);
    let mut prefix = Vec::new();
    attributes(&mut prefix, 17);
    let normal_z = prefix.len() + 5 * 8;
    f.0.get_mut(&31).unwrap().2[normal_z..normal_z + 8].copy_from_slice(&(1. + f64::EPSILON).to_le_bytes());
    volume(&f.design().to_document("Rounded semicircle").unwrap(), 450. * PI * 17.);
    let mut f = arc(PI, PI / 3.);
    round(&mut f, 31, 17, 3., PI - 1e-3, PI / 3.);
    assert!(f.design().to_document("Disagreeing endpoints").unwrap_err().to_string().contains("geometry disagree"));
}

#[test]
fn malformed_arc_and_hole_records_are_rejected() {
    for id in [31, 101] {
        let length = arc(PI, 0.).0[&id].2.len();
        for keep in 0..length {
            let mut f = arc(PI, 0.);
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(f.design().to_document("Truncated").is_err(), "{id}/{keep}");
        }
    }
    for loops in [
        vec![(vec![(17, false)], false)],
        vec![(vec![(17, false)], true), (vec![(18, true)], true)],
        vec![(vec![(17, false)], true), (vec![(17, true)], false)],
    ] {
        let mut f = ring();
        tracking(&mut f, &loops);
        assert!(f.design().sketch_profiles().is_err());
    }
}

#[test]
fn missing_selected_curve_reports_its_source_instead_of_choosing_another_region() {
    let mut f = ring();
    tracking(&mut f, &[(vec![(999, false)], true)]);
    let error = f.design().to_document("Missing selection").unwrap_err().to_string();
    assert!(error.contains("profile 100, tracking 101, region 1"), "{error}");
    assert!(error.contains("missing selected profile curve (999, 2) in sketch geometry 70, piece 1/1"), "{error}");
}

#[test]
fn diameter_graph_slots_do_not_change_dimension_semantics() {
    for slot in [0_u32, 4, 5, 6, u32::MAX] {
        let mut f = fixture(false);
        for at in [20, 35] {
            f.0.get_mut(&56).unwrap().2[at..at + 4].copy_from_slice(&slot.to_le_bytes());
        }
        let mut doc = f.design().to_document("Diameter connection slots").unwrap();
        doc.change_param("d7", "40 mm", None, None).unwrap();
        volume(&doc, 400. * PI * 17.);
    }
}

#[test]
fn fixed_arc_support_preserves_free_endpoint_motion() {
    use solidcraft_doc::sketch::{ConstraintKind, CurveKind};
    let mut f = arc((1_f64 / 3.).acos(), 0.);
    let b = &mut f.0.get_mut(&31).unwrap().2;
    let n = b.len();
    b[n - 51] = 1; // Original field-authored support fixation.
    f.0.get_mut(&71).unwrap().2[24] = 1; // Start point independently fixed.
    let mut doc = f.design().to_document("Fixed arc support").unwrap();
    let FeatureKind::Sketch { sketch, .. } = &mut doc.features[0].kind else { panic!("sketch") };
    let ci = sketch.curve_index("c31").unwrap();
    let CurveKind::Arc { c, a, b } = sketch.curves[ci].kind else { panic!("arc") };
    assert!(!sketch.curves[ci].fixed);
    assert!(sketch.points[a].fixed);
    assert!(!sketch.points[b].fixed);
    let dimension = sketch.add_constraint(ConstraintKind::DistanceX { p: c, q: b, value: 10. }, None).unwrap();
    for x in [10_f64, 20., 5.] {
        let FeatureKind::Sketch { sketch, .. } = &mut doc.features[0].kind else { panic!("sketch") };
        sketch.constraints.iter_mut().find(|c| c.id == dimension).unwrap().kind = ConstraintKind::DistanceX { p: c, q: b, value: x };
        let angle = (x / 30.).acos();
        volume(&doc, 450. * (angle - angle.sin()) * 17.);
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        let state = model.state();
        let solved = &state.sketches[0].sketch;
        for (index, expected) in [(c, (0., 0.)), (a, (30., 0.)), (b, (x, (900. - x * x).sqrt()))] {
            let p = solved.points[index].pos;
            assert!((p.x - expected.0).abs() < 1e-5);
            assert!((p.y - expected.1).abs() < 1e-5);
        }
        volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), 450. * (angle - angle.sin()) * 17.);
    }
}

fn multiple_regions(duplicate: bool) -> Fixture {
    let mut f = ring();
    let mut b = vec![];
    attributes(&mut b, 18);
    for x in [8_f64, 0., 0., 0., 0., 1., 1., 0., 0., 1., 0., TAU] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 75);
    b.extend([0; 44]);
    reference(&mut b, 70);
    f.put(35, "F0130424-8B7E-4092-93C9-1CA807482534", 0, b, None);
    let mut b = vec![0; 6];
    reference(&mut b, 76);
    b.extend([0, 0, 0, 1, 1, 0, 0, 0]);
    for x in [8_f64, 0., 0.] {
        b.extend(x.to_le_bytes());
    }
    b.extend([0; 30]);
    refs(&mut b, &[76]);
    reference(&mut b, 70);
    f.put(75, "C2CEDAE7-1716-47C1-B7B1-07B70081D0FB", 11, b, None);
    f.put(76, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0, vec![], None);
    geometry(&mut f, &[42, 41, 31, 35, 75, 56], &[56]);
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    u32(&mut b, 2);
    for tag in [17, if duplicate { 17 } else { 18 }] {
        u32(&mut b, 1);
        u32(&mut b, 1);
        u32(&mut b, 3);
        u64(&mut b, tag);
        u64(&mut b, 2);
        u32(&mut b, 0);
        u32(&mut b, 1);
        u32(&mut b, 1);
        u64(&mut b, 0);
        b.push(1);
        u32(&mut b, 0);
    }
    f.0.get_mut(&101).unwrap().2 = b;
    f
}

#[test]
fn multiple_profile_regions_keep_separate_bodies_and_editable_dimensions() {
    let mut doc = multiple_regions(false).design().to_document("Two original disks").unwrap();
    assert_eq!(doc.features[1].body_names, ["Body1", "Body2"]);
    volume(&doc, 1000. * PI * 17.);
    doc.change_param("d7", "80 mm", None, None).unwrap();
    volume(&doc, 1700. * PI * 17.);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert_eq!(model.state().bodies.len(), 2);
    volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), 1700. * PI * 17.);
}

#[test]
fn multiple_profile_regions_reject_duplicates_and_truncation() {
    assert!(multiple_regions(true).design().sketch_profiles().is_err());
    let length = multiple_regions(false).0[&101].2.len();
    for keep in 0..length {
        let mut f = multiple_regions(false);
        f.0.get_mut(&101).unwrap().2.truncate(keep);
        assert!(f.design().sketch_profiles().is_err(), "prefix {keep}");
    }
}

fn partial_circles(first: (u32, bool), second: (u32, bool)) -> Fixture {
    let mut f = multiple_regions(false);
    let mut b = vec![];
    attributes(&mut b, 18);
    for x in [4_f64, 0., 0., 0., 0., 1., 1., 0., 0., 3., 0., TAU] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 75);
    b.extend([0; 44]);
    reference(&mut b, 70);
    f.put(35, "F0130424-8B7E-4092-93C9-1CA807482534", 0, b, None);
    f.0.get_mut(&75).unwrap().2[25..33].copy_from_slice(&4_f64.to_le_bytes());
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    for n in [1, 1, 2] {
        u32(&mut b, n);
    }
    for (tag, (piece, reversed)) in [(17, first), (18, second)] {
        u32(&mut b, 3);
        u64(&mut b, tag);
        u64(&mut b, 2);
        u32(&mut b, u32::from(reversed));
        u32(&mut b, piece);
        u32(&mut b, 2);
        u64(&mut b, 0);
    }
    b.push(1);
    u32(&mut b, 0);
    f.0.get_mut(&101).unwrap().2 = b;
    f
}

fn overlap(r: f64, s: f64, d: f64) -> f64 {
    r * r * ((d * d + r * r - s * s) / (2. * d * r)).acos() + s * s * ((d * d + s * s - r * r) / (2. * d * s)).acos()
        - ((-d + r + s) * (d + r - s) * (d - r + s) * (d + r + s)).sqrt() / 2.
}

fn extra_profile_circle(f: &mut Fixture, center_x: f64, radius: f64) {
    let mut b = vec![];
    attributes(&mut b, 19);
    for x in [center_x, 0., 0., 0., 0., 1., 1., 0., 0., radius, 0., TAU] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 77);
    b.extend([0; 44]);
    reference(&mut b, 70);
    f.put(37, "F0130424-8B7E-4092-93C9-1CA807482534", 0, b, None);
    let mut b = vec![0; 6];
    reference(&mut b, 78);
    b.extend([0, 0, 0, 1, 1, 0, 0, 0]);
    for x in [center_x, 0., 0.] {
        b.extend(x.to_le_bytes());
    }
    b.extend([0; 30]);
    refs(&mut b, &[78]);
    reference(&mut b, 70);
    f.put(77, "C2CEDAE7-1716-47C1-B7B1-07B70081D0FB", 11, b, None);
    f.put(78, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0, vec![], None);
    let entities: Vec<_> = [42, 41, 31, 35, 37, 75, 77, 56].into_iter().filter(|id| f.0.contains_key(id)).collect();
    geometry(f, &entities, &[56]);
}

#[test]
fn boundary_scoped_piece_counts_translate_without_changing_geometry_or_editability() {
    use solidcraft_doc::{Model, ProfileSel, profile_selection_indices};
    for phase in [0., PI / 3., PI] {
        let mut f = partial_circles((if phase == 0. { 1 } else { 2 }, false), (2, false));
        round(&mut f, 31, 17, 3., TAU, phase);
        // This circle intersects the unused side of the left circle, adding two
        // global partitions without changing any selected lens boundary.
        extra_profile_circle(&mut f, -3., 0.8);
        let mut doc = f.design().to_document("Boundary-scoped lens").unwrap();
        volume(&doc, overlap(30., 30., 40.) * 17.);
        let FeatureKind::Extrude { profiles: ProfileSel::PieceBoundaries { profiles, circle_seams }, .. } = &doc.features[1].kind else {
            panic!("piece selection")
        };
        assert_eq!(profiles[0].outer.iter().find(|p| p.curve == "c31").unwrap().count, 4);
        let mut native = profiles.clone();
        let left = native[0].outer.iter_mut().find(|p| p.curve == "c31").unwrap();
        left.count = 2;
        left.piece = if phase == 0. { 1 } else { 2 };
        let mut model = Model::new();
        model.evaluate(&doc);
        let state = model.state();
        let solved = state.sketches.first().unwrap();
        let native = ProfileSel::PieceBoundaries { profiles: native, circle_seams: circle_seams.clone() };
        assert!(profile_selection_indices(solved, &native).is_err());
        assert!(super::profile_selection::resolve(native.clone(), solved).is_ok());
        let mut unrelated = solved.as_ref().clone();
        let points: Vec<_> = [(100., 0.), (110., 10.), (120., 10.), (130., 0.)]
            .into_iter()
            .map(|(x, y)| unrelated.sketch.add_point(solidcraft_geom::Vec2::new(x, y), None).unwrap())
            .collect();
        unrelated
            .sketch
            .add_curve(solidcraft_doc::sketch::CurveKind::Spline { pts: points.clone(), control: true, degree: 3, periodic: false }, None)
            .unwrap();
        unrelated.sketch.add_line_pts(points[3], points[0], None).unwrap();
        unrelated.profiles = solidcraft_doc::sketch::find_profiles(&unrelated.sketch);
        assert!(unrelated.profiles.iter().any(|p| p.piece_boundaries().is_none()));
        assert!(super::profile_selection::resolve(native.clone(), &unrelated).is_ok());
        let mut ambiguous = solved.as_ref().clone();
        ambiguous.profiles.extend(solved.profiles.clone());
        assert!(super::profile_selection::resolve(native, &ambiguous).is_err());
        doc.change_param("d7", "70 mm", None, None).unwrap();
        doc.change_param("height", "23 mm", None, None).unwrap();
        volume(&doc, overlap(35., 30., 40.) * 23.);
        let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, overlap(35., 30., 40.) * 23.);
    }
}

#[test]
fn scoped_profile_translation_preserves_filled_area_after_subdivision() {
    for (x, radius) in [(2., 0.2), (2., 2.)] {
        let mut f = partial_circles((1, false), (2, false));
        extra_profile_circle(&mut f, x, radius);
        let mut doc = f.design().to_document("Subdivided lens").unwrap();
        volume(&doc, overlap(30., 30., 40.) * 17.);
        let FeatureKind::Extrude { profiles: solidcraft_doc::ProfileSel::PieceBoundaries { profiles, .. }, .. } = &doc.features[1].kind else {
            panic!("piece selection")
        };
        assert!(profiles.len() > 1);
        doc.change_param("d7", "70 mm", None, None).unwrap();
        doc.change_param("height", "23 mm", None, None).unwrap();
        volume(&doc, overlap(35., 30., 40.) * 23.);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, overlap(35., 30., 40.) * 23.);
    }
}

#[test]
fn scoped_profile_translation_rejects_invalid_identity() {
    for (at, value) in [(53, 3_u32), (57, 3_u32), (49, 1_u32)] {
        let mut f = partial_circles((1, false), (2, false));
        extra_profile_circle(&mut f, -3., 0.8);
        f.0.get_mut(&101).unwrap().2[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(f.design().to_document("Invalid scoped identity").is_err(), "{at}/{value}");
    }
}

#[test]
fn refined_whole_boundaries_preserve_explicit_holes() {
    let mut f = ring();
    // The third circle divides the annulus. It is not a selected hole; the
    // original concentric inner circle must still subtract its complete disk.
    extra_profile_circle(&mut f, 1., 0.8);
    let mut doc = f.design().to_document("Divided annulus").unwrap();
    volume(&doc, (30. * 30. - 10. * 10.) * PI * 17.);
    doc.change_param("d7", "80 mm", None, None).unwrap();
    volume(&doc, (40. * 40. - 10. * 10.) * PI * 17.);
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(doc, reopened);
    volume(&reopened, (40. * 40. - 10. * 10.) * PI * 17.);
}

#[test]
#[ignore = "requires local before/after native profile subdivision controls"]
fn native_profile_subdivisions_retain_selected_union() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    for kind in ["line", "hole"] {
        for stage in ["before", "after", "reselected"] {
            let name = format!("profile-remap-lens-{kind}-{stage}");
            let bytes = std::fs::read(root.join(format!("{name}.f3d"))).unwrap();
            let mut doc = super::import_f3d(&bytes, &name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(doc.features.len(), 2, "{name}");
            assert!(doc.import_warnings.is_empty(), "{name}");
            volume(&doc, overlap(20., 20., 30.) * 10.);
            let feature = doc.features.iter().find(|f| matches!(f.kind, FeatureKind::Extrude { .. })).unwrap();
            let height = feature.param_names.iter().zip(feature.kind.inputs()).find(|(_, (label, _, _))| *label == "Distance").unwrap().0.clone();
            doc.change_param(&height, "15 mm", None, None).unwrap();
            volume(&doc, overlap(20., 20., 30.) * 15.);
            let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            assert_eq!(doc, reopened);
            volume(&reopened, overlap(20., 20., 30.) * 15.);
        }
    }
}

#[test]
fn tangent_contacts_split_selected_pieces_without_changing_the_region() {
    use solidcraft_doc::{Model, ProfileSel};
    for phase in [0., PI / 3., PI] {
        let mut f = partial_circles((if phase == 0. { 1 } else { 2 }, false), (2, false));
        round(&mut f, 31, 17, 3., TAU, phase);
        // The added circle is outside the lens, tangent to its boundary at x=30.
        // It splits the selected left-circle span but removes no lens area.
        extra_profile_circle(&mut f, 3.8, 0.8);
        let mut doc = f.design().to_document("Tangent contact on selected arc").unwrap();
        let FeatureKind::Extrude { profiles: ProfileSel::PieceBoundaries { profiles, .. }, .. } = &doc.features[1].kind else {
            panic!("piece selection")
        };
        let left: Vec<_> = profiles[0].outer.iter().filter(|p| p.curve == "c31").collect();
        assert_eq!(left.len(), 2);
        assert!(left.iter().all(|p| p.count == 3));
        volume(&doc, overlap(30., 30., 40.) * 17.);
        doc.change_param("height", "23 mm", None, None).unwrap();
        volume(&doc, overlap(30., 30., 40.) * 23.);
        volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), overlap(30., 30., 40.) * 23.);
        // Moving the contact into the lens creates a real subdivision. A saved
        // full-graph selection must report that topology change after an edit.
        doc.change_param("d7", "70 mm", None, None).unwrap();
        let mut model = Model::new();
        model.evaluate(&doc);
        assert!(model.results.iter().any(|r| r.error.is_some()));
    }
}

#[test]
#[ignore = "requires the local native Crescent and Narrow archives"]
fn native_scoped_profiles_preserve_analytic_cut_volumes_and_parameter_edits() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs");
    for (name, diameter, inset, caps) in [("Crescent", 7.5, 1.5, 1.), ("Narrow", 8.5, 1., 2.)] {
        let bytes = std::fs::read(root.join(format!("{name}.f3d"))).unwrap();
        let mut doc = super::import_f3d(&bytes, name).unwrap();
        assert_eq!(doc.features.len(), 4);
        assert!(doc.import_warnings.is_empty());
        // Exact circular-segment integral, independently of imported topology
        // and native body conversion. Stock is (InsertWidth - 0.25 mm)^2.
        let expected = |diameter: f64, inset: f64, width: f64, depth: f64| {
            let r = diameter / 2.;
            let d = r - inset;
            let cap = r * r * (d / r).acos() - d * (r * r - d * d).sqrt();
            ((width - 0.25).powi(2) - (PI * r * r - caps * cap)) * depth
        };
        for (diameter, inset, width, depth) in [(diameter, inset, 15., 10.), (diameter + 0.5, inset + 0.25, 16., 12.)] {
            for (parameter, value) in [("Diameter", diameter), ("Inset", inset), ("InsertWidth", width), ("InsertDepth", depth)] {
                doc.change_param(parameter, &format!("{value} mm"), None, None).unwrap();
            }
            volume(&doc, expected(diameter, inset, width, depth));
            let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            assert_eq!(doc, reopened);
            volume(&reopened, expected(diameter, inset, width, depth));
        }
    }
}

#[test]
#[ignore = "requires the local native Style 3 and Style 4 archives"]
fn native_tangent_profile_prefixes_preserve_volume_edits_and_reopening() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs");
    for (name, diameter, width, knob_height) in [("Style 3", 35., 10., 12.5), ("Style 4", 50., 17.5, 10.)] {
        let bytes = std::fs::read(root.join(format!("{name}.f3d"))).unwrap();
        let outcome = super::import_f3d_with_preference(&bytes, name, super::ImportPreference::Editability).unwrap();
        assert!(!outcome.warnings.iter().any(|w| w.contains("Omitted feature 541")));
        let mut doc = outcome.document;
        assert_eq!(doc.features.iter().take(4).map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Sketch1", "Extrude1", "Sketch3", "Extrude3"]);
        // Later unsupported features remain covered by the complete corpus gate.
        // This native prefix isolates the repaired circular strip joined to a cylinder.
        doc.features.truncate(4);
        let expected = |diameter: f64, width: f64, height: f64, knob_height: f64| {
            let r = diameter / 2.;
            let a = width / 2.;
            let strip = 2. * (a * (r * r - a * a).sqrt() + r * r * (a / r).asin());
            PI * r * r * height + strip * knob_height
        };
        for (diameter, width, height, knob_height) in [(diameter, width, 10., knob_height), (diameter + 2., width + 1., 11., knob_height + 2.)] {
            for (parameter, value) in [("Diameter", diameter), ("KnobWidth", width), ("Height", height), ("KnobHeight", knob_height)] {
                doc.change_param(parameter, &format!("{value} mm"), None, None).unwrap();
            }
            volume(&doc, expected(diameter, width, height, knob_height));
            let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            assert_eq!(doc, reopened);
            volume(&reopened, expected(diameter, width, height, knob_height));
        }
    }
}

#[test]
#[ignore = "requires the local native Style 7 archive"]
fn native_style7_retains_the_rectangular_grip_extrusion_and_its_dimensions() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs");
    let bytes = std::fs::read(root.join("Style 7.f3d")).unwrap();
    let outcome = super::import_f3d_with_preference(&bytes, "Style 7", super::ImportPreference::Editability).unwrap();
    assert!(!outcome.warnings.is_empty(), "later unsupported features must remain reported");
    let mut doc = outcome.document;
    assert_eq!(doc.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Sketch1", "Extrude1", "Sketch7", "Extrude3"]);
    // The recovered features are a cylinder and a separate rectangular grip.
    // From the recovered feature's own inputs: extrusion depth=Diameter/2+GripHeight,
    // sketch height=Height-2*Chamfer and sketch width=GripWidth.
    for (diameter, width, height, grip) in [(35_f64, 2., 15., 0.5), (35., 4., 15., 1.), (40., 4., 20., 2.)] {
        for (parameter, value) in [("Diameter", diameter), ("GripWidth", width), ("Height", height), ("GripHeight", grip)] {
            doc.change_param(parameter, &format!("{value} mm"), None, None).unwrap();
        }
        let expected = PI * (diameter / 2.).powi(2) * height + width * (diameter / 2. + grip) * (height - 2.);
        volume(&doc, expected);
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        assert_eq!(model.state().bodies.len(), 2);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, expected);
    }
}

#[test]
fn partial_circles_distinguish_lens_and_crescents_through_edits() {
    for (first, second, side) in [((1, false), (2, false), 0), ((2, false), (2, true), 1), ((1, true), (1, false), 2)] {
        let mut doc = partial_circles(first, second).design().to_document("Original split circles").unwrap();
        let area = |r: f64| {
            let lens = overlap(r, 30., 40.);
            match side {
                0 => lens,
                1 => PI * r * r - lens,
                _ => PI * 900. - lens,
            }
        };
        volume(&doc, area(30.) * 17.);
        doc.change_param("d7", "70 mm", None, None).unwrap();
        volume(&doc, area(35.) * 17.);
        volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), area(35.) * 17.);
        doc.change_param("d7", "10 mm", None, None).unwrap();
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        assert!(model.results.last().unwrap().error.is_some());
        assert!(model.state().bodies.is_empty());
    }
}

#[test]
fn profile_piece_records_reject_invalid_counts_and_truncation() {
    for (piece, count) in [(0, 2), (3, 2), (1, 0), (1, 50_001)] {
        let mut f = partial_circles((1, false), (2, false));
        f.0.get_mut(&101).unwrap().2[53..57].copy_from_slice(&u32::to_le_bytes(piece));
        f.0.get_mut(&101).unwrap().2[57..61].copy_from_slice(&u32::to_le_bytes(count));
        assert!(f.design().sketch_profiles().is_err());
    }
    let length = partial_circles((1, false), (2, false)).0[&101].2.len();
    for keep in 0..length {
        let mut f = partial_circles((1, false), (2, false));
        f.0.get_mut(&101).unwrap().2.truncate(keep);
        assert!(f.design().sketch_profiles().is_err(), "prefix {keep}");
    }
    assert!(partial_circles((1, false), (2, false)).design().circle_profiles().is_err());
}

#[test]
fn rotated_circle_seams_survive_import_edit_and_reopening() {
    for phase in [PI / 3., PI / 2., PI] {
        // Rotating only the left source axis moves its lens span to piece two.
        let mut f = partial_circles((2, false), (2, false));
        round(&mut f, 31, 17, 3., TAU, phase);
        let mut doc = f.design().to_document("Rotated source circle").unwrap();
        volume(&doc, overlap(30., 30., 40.) * 17.);
        doc.change_param("d7", "70 mm", None, None).unwrap();
        doc.change_param("height", "23 mm", None, None).unwrap();
        volume(&doc, overlap(35., 30., 40.) * 23.);
        let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, overlap(35., 30., 40.) * 23.);
    }
}

// A member may use its own source-curve partition counts. The combined outer
// boundary is stored separately and must agree with the union, not just its area.
type GroupLoop = (Vec<(u64, u32, u32, bool)>, bool);
fn grouped_tracking(f: &mut Fixture, parent: &[GroupLoop], members: &[Vec<GroupLoop>]) {
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    u32(&mut b, 1);
    let loops = |b: &mut Vec<u8>, loops: &[GroupLoop]| {
        u32(b, loops.len() as u32);
        for (pieces, outer) in loops {
            u32(b, pieces.len() as u32);
            for (tag, piece, count, reversed) in pieces {
                u32(b, 3);
                u64(b, *tag);
                u64(b, 2);
                u32(b, u32::from(*reversed));
                u32(b, *piece);
                u32(b, *count);
                u64(b, 0);
            }
            b.push(u8::from(*outer));
        }
    };
    loops(&mut b, parent);
    u32(&mut b, members.len() as u32);
    for member in members {
        loops(&mut b, member);
    }
    f.0.get_mut(&101).unwrap().2 = b;
}

fn circle_group() -> (Vec<GroupLoop>, Vec<Vec<GroupLoop>>) {
    let parent = vec![(vec![(17, 2, 2, false), (18, 1, 2, false)], true)];
    let members = vec![
        vec![(vec![(17, 1, 2, false), (18, 2, 2, false)], true)],
        vec![(vec![(17, 2, 2, false), (18, 2, 2, true)], true)],
        vec![(vec![(17, 1, 2, true), (18, 1, 2, false)], true)],
    ];
    (parent, members)
}

#[test]
fn grouped_profiles_import_as_an_editable_union_with_one_body() {
    let (parent, mut members) = circle_group();
    // Source registration order is not the union or body order.
    members.reverse();
    let mut f = partial_circles((1, false), (2, false));
    grouped_tracking(&mut f, &parent, &members);
    let native = f.design();
    assert_eq!(native.sketch_profiles().unwrap()[0].region_members[0].len(), 3);
    let mut doc = native.to_document("Original overlapping disk union").unwrap();
    assert_eq!(doc.features[1].body_names, ["Body1"]);
    for (radius, height) in [(30., 17.), (35., 23.)] {
        doc.change_param("d7", &format!("{} mm", radius * 2.), None, None).unwrap();
        doc.change_param("height", &format!("{height} mm"), None, None).unwrap();
        let expected = (PI * (radius * radius + 900.) - overlap(radius, 30., 40.)) * height;
        volume(&doc, expected);
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        let state = model.state();
        assert_eq!(state.bodies.len(), 1);
        let bounds = state.bodies[0].mesh().bounds();
        assert!((bounds.min.x + radius).abs() < 1e-5);
        assert!((bounds.max.x - 70.).abs() < 1e-5);
        assert!((bounds.max.z - height).abs() < 1e-5);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, expected);
    }
    let FeatureKind::Sketch { sketch, .. } = &mut doc.features[0].kind else { panic!("sketch") };
    sketch.curves.iter_mut().find(|c| c.id == "c35").unwrap().construction = true;
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.last().unwrap().error.is_some());
    assert!(model.state().bodies.is_empty());
}

#[test]
fn grouped_profiles_preserve_holes_and_only_fill_explicitly_selected_holes() {
    let outer = (vec![(17, 1, 1, false)], true);
    let hole = (vec![(18, 1, 1, true)], false);
    for fill in [false, true] {
        let mut f = ring();
        let parent = if fill { vec![outer.clone()] } else { vec![outer.clone(), hole.clone()] };
        let mut members = vec![vec![outer.clone(), hole.clone()]];
        if fill {
            members.push(vec![(vec![(18, 1, 1, false)], true)]);
        }
        grouped_tracking(&mut f, &parent, &members);
        let mut doc = f.design().to_document("Original grouped annulus").unwrap();
        let area = if fill { 900. } else { 800. };
        volume(&doc, area * PI * 17.);
        doc.change_param("height", "21 mm", None, None).unwrap();
        volume(&crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap(), area * PI * 21.);
    }
}

#[test]
fn grouped_profiles_reject_missing_duplicate_reversed_or_wrong_members() {
    for case in 0..6 {
        let (mut parent, mut members) = circle_group();
        match case {
            0 => {
                members.pop();
            }
            1 => members.push(members[0].clone()),
            2 => parent[0].0[0].3 = true,
            3 => parent = members[0].clone(),
            4 => members[0][0].0[0].2 = 3,
            5 => {
                members.remove(0);
            }
            _ => panic!("invalid authored group case"),
        }
        let mut f = partial_circles((1, false), (2, false));
        grouped_tracking(&mut f, &parent, &members);
        assert!(f.design().to_document("Invalid profile group").is_err(), "case {case}");
    }
    let mut f = ring();
    // Same curves and directions, but claiming the hole is the outer boundary.
    let wrong = vec![(vec![(17, 1, 1, false)], false), (vec![(18, 1, 1, true)], true)];
    let correct = vec![(vec![(17, 1, 1, false)], true), (vec![(18, 1, 1, true)], false)];
    grouped_tracking(&mut f, &wrong, &[correct]);
    assert!(f.design().to_document("Wrong group loop roles").is_err());
}

#[test]
fn grouped_profile_records_enforce_member_budgets_and_complete_payloads() {
    let (parent, members) = circle_group();
    let mut f = partial_circles((1, false), (2, false));
    grouped_tracking(&mut f, &parent, &members);
    let original = f.0[&101].2.clone();
    for keep in 0..original.len() {
        let mut f = partial_circles((1, false), (2, false));
        f.0.get_mut(&101).unwrap().2 = original[..keep].to_vec();
        assert!(f.design().sketch_profiles().is_err(), "prefix {keep}");
    }
    let mut f = partial_circles((1, false), (2, false));
    grouped_tracking(&mut f, &parent, &vec![members[0].clone(); 129]);
    assert!(f.design().sketch_profiles().is_err());
}

#[test]
#[ignore = "requires local native profile-group controls"]
fn native_grouped_profiles_match_analytic_union_lens_and_crescent_volumes() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    let lens = overlap(20., 20., 30.);
    for (name, area) in [("all", 800. * PI - lens), ("lens", lens), ("sides", 800. * PI - 2. * lens)] {
        let bytes = std::fs::read(root.join(format!("profile-group-{name}.f3d"))).unwrap();
        let mut doc = super::import_f3d(&bytes, name).unwrap();
        assert_eq!(doc.features.len(), 2);
        assert!(doc.import_warnings.is_empty());
        volume(&doc, area * 10.);
        let feature = doc.features.iter().find(|f| matches!(f.kind, FeatureKind::Extrude { .. })).unwrap();
        let height = feature.param_names.iter().zip(feature.kind.inputs()).find(|(_, (label, _, _))| *label == "Distance").unwrap().0.clone();
        doc.change_param(&height, "15 mm", None, None).unwrap();
        volume(&doc, area * 15.);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, area * 15.);
    }
}

#[test]
#[ignore = "requires local native clamp archive"]
fn native_clamp_grouped_profile_prefix() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap());
    let bytes = std::fs::read(root.join("clamp/Modular Clamp V1 - Parametrized.f3d")).unwrap();
    let mut designs = super::read_f3d(&bytes).unwrap();
    let native = designs.first_mut().unwrap();
    native.recovery = Some(Default::default());
    let doc = native.to_document("Clamp group prefix").unwrap();
    let profiles = native.sketch_profiles().unwrap();
    let profile = profiles.iter().find(|p| p.feature == 457).unwrap();
    let mut prefix = doc.clone();
    prefix.features.truncate(1);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&prefix);
    let state = model.state();
    let solved = &state.sketches[0];
    let circles = native.sketch_circles().unwrap().into_iter().map(|c| (c.id, c)).collect();
    let result = super::profile_groups::resolve(profile, &circles, solved);
    result.unwrap();
    assert!(doc.features.iter().any(|f| f.name == "Extrude1"));
}

#[test]
fn coincident_native_circle_references_preserve_dependencies_and_edits() {
    for phase in [0., PI / 3.] {
        let part = if phase == 0. { 1 } else { 2 };
        let mut f = partial_circles((part, false), (2, false));
        extra_profile_circle(&mut f, 0., 3.);
        round(&mut f, 31, 17, 3., TAU, phase);
        round(&mut f, 37, 19, 3., TAU, phase);
        let boundary = vec![(vec![(17, part, 2, false), (19, part, 2, false), (18, 2, 2, false)], true)];
        grouped_tracking(&mut f, &boundary, std::slice::from_ref(&boundary));
        let mut doc = f.design().to_document("Coincident circle dependencies").unwrap();
        let FeatureKind::Extrude { profiles: solidcraft_doc::ProfileSel::PieceBoundaries { profiles, .. }, .. } = &doc.features[1].kind else {
            panic!("piece boundaries")
        };
        let curves: Vec<_> = profiles[0].outer.iter().map(|p| p.curve.as_str()).collect();
        assert_eq!(curves, ["c31", "c35", "c37"]);
        volume(&doc, overlap(30., 30., 40.) * 17.);
        doc.change_param("height", "23 mm", None, None).unwrap();
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        volume(&reopened, overlap(30., 30., 40.) * 23.);
        // A model edit which separates the circles invalidates the selection.
        // Both source references survived translation and container serialization.
        let FeatureKind::Sketch { sketch, .. } = &mut doc.features[0].kind else { panic!("sketch") };
        let alias = sketch.curves.iter_mut().find(|c| c.id == "c37").unwrap();
        let solidcraft_doc::sketch::CurveKind::Circle { r, .. } = &mut alias.kind else { panic!("circle") };
        *r = 31.;
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(&doc);
        assert!(model.results.last().unwrap().error.is_some());
        assert!(model.state().bodies.is_empty());
    }
}

#[test]
#[ignore = "requires the local native Round archive"]
fn native_round_grip_profiles_remain_editable() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs");
    let bytes = std::fs::read(root.join("Round.f3d")).unwrap();
    let outcome = super::import_f3d_with_preference(&bytes, "Round", super::ImportPreference::Editability).unwrap();
    let mut doc = outcome.document;
    assert_eq!(
        doc.features.iter().take(6).map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["Sketch1", "Extrude1", "Sketch2", "Extrude2", "Sketch3", "Extrude3"]
    );
    doc.features.truncate(6);
    let check = |doc: &solidcraft_doc::Document, diameter: f64, grip: f64, depth: f64| {
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(doc);
        let errors: Vec<_> = model.results.iter().filter_map(|r| r.error.as_ref().map(|error| (r.id, error))).collect();
        assert!(errors.is_empty(), "diameter={diameter}, grip={grip}: {errors:?}");
        let state = model.state();
        assert_eq!(state.bodies.len(), 2);
        let feature = doc.features.iter().find(|f| f.name == "Sketch3").unwrap();
        let sketch = &state.sketch(feature.id).unwrap().sketch;
        let point = |id: &str| sketch.points.iter().find(|p| p.id == id).unwrap().pos;
        let left = point("p6924");
        let right = point("p6926");
        assert!((right.x - left.x - grip).abs() < 1e-7);
        assert!((right.y - left.y).abs() < 1e-7);
        let r = diameter / 2.;
        assert!(left.x.abs() < r && right.x.abs() < r);
        // Integral of the lower circular boundary up to the rectangle's top:
        // independent of selected profile regions or tessellated body geometry.
        let integral = |x: f64| 0.5 * (x * (r * r - x * x).sqrt() + r * r * (x / r).asin());
        let area = (right.x - left.x) * left.y + integral(right.x) - integral(left.x);
        assert!(area > 0.);
        for (name, expected) in [("Body1", (14.75 * 14.75 - PI * r * r) * 10.), ("Body2", area * depth)] {
            let body = state.body(name).unwrap();
            let measured = solidcraft_kernel::measure(&body.body).unwrap().volume;
            assert!((measured - expected).abs() < expected * 1e-3, "{name}: {measured} != {expected}");
        }
    };
    for (diameter, grip) in [(6., 0.5), (6., 0.6)] {
        doc.change_param("Diameter", &format!("{diameter} mm"), None, None).unwrap();
        doc.change_param("GripLength", &format!("{grip} mm"), None, None).unwrap();
        check(&doc, diameter, grip, 10.);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        check(&reopened, diameter, grip, 10.);
    }
    doc.change_param("d16", "-12 mm", None, None).unwrap();
    check(&doc, 6., 0.6, 12.);
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(doc, reopened);
    check(&reopened, 6., 0.6, 12.);
    doc.change_param("d13", "Diameter + 0.2 mm", None, None).unwrap();
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.last().unwrap().error.is_some(), "separating source circles must invalidate their joint boundary");
}

#[test]
#[ignore = "requires the local native Round archive"]
fn native_round_grip_fillet_and_pattern_preserve_radius_depth_and_source_edits() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs");
    let bytes = std::fs::read(root.join("Round.f3d")).unwrap();
    let outcome = super::import_f3d_with_preference(&bytes, "Round", super::ImportPreference::Editability).unwrap();
    let mut doc = outcome.document;
    assert_eq!(doc.features.get(6).map(|f| f.name.as_str()), Some("Fillet1"));
    assert!(matches!(&doc.features[6].kind, FeatureKind::Fillet { .. }));
    assert_eq!(doc.features[6].edge_names.len(), 2);
    assert_eq!(doc.features.get(7).map(|f| f.name.as_str()), Some("C-Pattern1"));
    doc.features.truncate(8);
    let check = |doc: &solidcraft_doc::Document, width: f64, radius: f64, depth: f64, count: usize| {
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(doc);
        for result in &model.results {
            assert!(result.error.is_none() && result.warning.is_none(), "{result:?}");
        }
        let state = model.state();
        assert_eq!(state.bodies.len(), count + 1);
        let sketch = &state.sketch(doc.features[4].id).unwrap().sketch;
        let point = |id: &str| sketch.points.iter().find(|p| p.id == id).unwrap().pos;
        let (left, right) = (point("p6924"), point("p6926"));
        assert!((right.x - left.x - width).abs() < 1e-7);
        let r: f64 = 3.;
        let integral = |x: f64| 0.5 * (x * (r * r - x * x).sqrt() + r * r * (x / r).asin());
        let area = (right.x - left.x) * left.y + integral(right.x) - integral(left.x);
        let removed = 2. * radius * radius * (1. - PI / 4.) * depth;
        for (name, expected) in [("Body1", (14.75 * 14.75 - PI * r * r) * 10.), ("Body2", area * depth - removed)] {
            let measured = solidcraft_kernel::measure(&state.body(name).unwrap().body).unwrap().volume;
            assert!((measured - expected).abs() < expected * 1e-3, "{name}: {measured} != {expected}");
        }
        let seed = solidcraft_kernel::measure(&state.body("Body2").unwrap().body).unwrap();
        for (i, body) in state.bodies.iter().skip(1).enumerate() {
            let measured = solidcraft_kernel::measure(&body.body).unwrap();
            assert!((measured.volume - seed.volume).abs() < 1e-6);
            let angle = 2. * PI * i as f64 / count as f64;
            let p = seed.centroid;
            let expected = solidcraft_geom::Vec3::new(p.x * angle.cos() - p.y * angle.sin(), p.x * angle.sin() + p.y * angle.cos(), p.z);
            assert!(measured.centroid.dist(expected) < 1e-5);
        }
    };
    check(&doc, 0.5, 0.25, 10., 10);
    doc.change_param("GripQuantity", "7", None, None).unwrap();
    for (name, value, width, radius, depth) in
        [("d30", "0.1 mm", 0.5, 0.1, 10.), ("d16", "-12 mm", 0.5, 0.1, 12.), ("GripLength", "0.6 mm", 0.6, 0.1, 12.)]
    {
        doc.change_param(name, value, None, None).unwrap();
        check(&doc, width, radius, depth, 7);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        check(&reopened, width, radius, depth, 7);
    }
}

#[test]
#[ignore = "requires the local native Round archive"]
fn native_round_complete_combine_preserves_editable_history() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs");
    let bytes = std::fs::read(root.join("Round.f3d")).unwrap();
    let mut doc = super::import_f3d(&bytes, "Round").unwrap();
    assert_eq!(doc.features.len(), 9);
    assert!(matches!(doc.features.last().unwrap().kind, FeatureKind::Combine { .. }));
    let check = |doc: &solidcraft_doc::Document, width: f64, radius: f64, depth: f64| {
        let mut model = solidcraft_doc::Model::new();
        model.evaluate(doc);
        for result in &model.results {
            assert!(result.error.is_none(), "width={width} radius={radius} depth={depth}: feature {}: {:?}", result.name, result.error);
        }
        let state = model.state();
        assert_eq!(state.bodies.len(), 1);
        let sketch = &state.sketch(doc.features[4].id).unwrap().sketch;
        let point = |id: &str| sketch.points.iter().find(|p| p.id == id).unwrap().pos;
        let (left, right) = (point("p6924"), point("p6926"));
        assert!((right.x - left.x - width).abs() < 1e-7);
        let r: f64 = 3.;
        let integral = |x: f64| 0.5 * (x * (r * r - x * x).sqrt() + r * r * (x / r).asin());
        let area = (right.x - left.x) * left.y + integral(right.x) - integral(left.x);
        let grip = (area - 2. * radius * radius * (1. - PI / 4.)) * depth;
        let expected = (14.75 * 14.75 - PI * r * r) * 10. + 10. * grip;
        let volume = solidcraft_kernel::measure(&state.bodies[0].body).unwrap().volume;
        assert!((volume - expected).abs() < expected * 1e-3, "combined volume {volume} != {expected}");
        volume
    };
    check(&doc, 0.5, 0.25, 10.);
    for (name, value, width, radius, depth) in
        [("d30", "0.1 mm", 0.5, 0.1, 10.), ("d16", "-12 mm", 0.5, 0.1, 12.), ("GripLength", "0.6 mm", 0.6, 0.1, 12.)]
    {
        doc.change_param(name, value, None, None).unwrap();
        if let Some(path) = solidcraft_geom::env::var_os("F3D_FAILED_COMBINE") {
            std::fs::write(path, crate::write_design(&doc)).unwrap();
        }
        let before = check(&doc, width, radius, depth);
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        assert!((check(&reopened, width, radius, depth) - before).abs() < before * 1e-6);
    }
}
