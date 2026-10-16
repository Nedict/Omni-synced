use super::*;
use serde_json::json;

#[path = "cap_family_tests.rs"]
mod cap_family;

#[path = "perimeter_blend_tests.rs"]
mod perimeter;

fn original() -> Document {
    crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-wall-join.solidcraft")).unwrap()
}

fn selected(doc: &Document, state: &ModelState, top: bool) -> (recipe_incidence::Edge, BTreeMap<Term, usize>, Selection) {
    let body = state.body("Block").unwrap();
    let cap = if top { Term::from([(9001, 2), (9002, 1)]) } else { Term::from([(9002, 2)]) };
    let tip = Term::from([(9002, 3)]);
    let left = Term::from([(9002, 4)]);
    let right = Term::from([(9002, 6)]);
    let roles = if top { BTreeMap::from([(5, true), (7, false)]) } else { BTreeMap::from([(7, true)]) };
    let anchor = super::super::loft_faces::joined_cap_anchor(doc, state, 2, &roles).unwrap();
    let caps = doc.configuration_face_indices(body, &anchor.target).unwrap();
    assert_eq!(caps.len(), 1);
    let mut indices = BTreeMap::from([(cap.clone(), caps[0])]);
    for (term, curve) in [(&tip, "l1"), (&left, "l2"), (&right, "l4")] {
        let found = doc
            .configuration_face_indices(
                body,
                &ConfigFaceTarget {
                    body: "Block".into(),
                    origin_feature: 7,
                    name: format!("F7:side:{curve}"),
                    point: Vec3::ZERO,
                    certificate: Some(FaceCertificate::Lineage),
                },
            )
            .unwrap();
        assert_eq!(found.len(), 1);
        indices.insert(term.clone(), found[0]);
    }
    let end = |neighbor| recipe_incidence::Vertex {
        faces: BTreeSet::from([cap.clone(), tip.clone(), neighbor]),
        seams: BTreeSet::new(),
        signatures: vec![],
    };
    let edge = recipe_incidence::Edge { faces: [cap.clone(), tip.clone()], ends: vec![end(left), end(right)] };
    let result = resolve_edge(body, &edge, &indices).unwrap();
    (edge, indices, result)
}

#[test]
fn joined_cap_blend_resolves_named_endpoints_and_preserves_analytic_volume_on_edit() {
    let mut doc = original();
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let body = state.body("Block").unwrap();
    // Use the same chord tolerance for both bodies: the general measurement
    // adapts to the smallest radius, so introducing a fillet also changes the
    // approximation of the untouched cylinder and can mask this small removal.
    let stock = body.body.tessellate_with(1e-4, true).unwrap().measure().volume;
    let pi = std::f64::consts::PI;
    let half_width: f64 = 1.875;
    let overlap = half_width * (64. - half_width * half_width).sqrt() + 64. * (half_width / 8.).asin();
    let analytic_stock = pi * (100. * 37. + 100. + 80. + 64. + 64. * 15.) + 10. * (3.75 * 25. - overlap);
    assert!((stock - analytic_stock).abs() < 0.5);
    let (mut edge, indices, top) = selected(&doc, &state, true);
    let (_, _, bottom) = selected(&doc, &state, false);
    let extra = Term::from([(9003, 3)]);
    let mut collapsed = edge.clone();
    collapsed.ends[0].faces.insert(extra.clone());
    let mut aliases = indices.clone();
    aliases.insert(extra, indices[&edge.faces[0]]);
    assert!(resolve_edge(body, &collapsed, &aliases).is_err(), "distinct native faces must not silently collapse");
    edge.ends[1] = edge.ends[0].clone();
    assert!(resolve_edge(body, &edge, &indices).is_err(), "the opposite endpoint must be certified too");
    assert!((top.point.z - 55.).abs() < 1e-7 && (bottom.point.z - 45.).abs() < 1e-7);
    doc.features.push(
        serde_json::from_value(json!({
            "id":8,"name":"Wing tip blend","type":"fillet","edges":[top.point,bottom.point],
            "edge_names":[top.name,bottom.name],"param_names":["d9"],"radius":"1 mm","body":"Block"
        }))
        .unwrap(),
    );
    doc.next_id = 9;
    for radius in [1., 1.5] {
        if let FeatureKind::Fillet { radius: value, .. } = &mut doc.features.last_mut().unwrap().kind {
            *value = format!("{radius} mm").into();
        }
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        model.evaluate(&reopened);
        assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().filter_map(|r| r.error.as_ref()).collect::<Vec<_>>());
        let volume = model.state().body("Block").unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
        let removed = 2. * 3.75 * radius * radius * (1. - std::f64::consts::FRAC_PI_4);
        assert!((volume - (stock - removed)).abs() < 0.02, "r={radius}: {volume} != {}", stock - removed);
        assert!((volume - (analytic_stock - removed)).abs() < 0.5);
    }
}

#[test]
fn circular_and_intersection_edges_follow_split_face_families_through_source_edits() {
    let mut doc = original();
    let mut model = solidcraft_doc::Model::new();
    for diameter in [20., 24.] {
        doc.change_param("diameter", &format!("{diameter} mm"), None, None).unwrap();
        model.evaluate(&doc);
        assert!(model.results.iter().all(|r| r.error.is_none()));
        let state = model.state();
        let body = state.body("Block").unwrap();
        let positional_tolerance = 1e-7 * (1. + body.body.size());
        let names = solidcraft_doc::naming::face_names(body);
        let family = |prefix: &str| names.iter().enumerate().filter_map(|(i, name)| name.starts_with(prefix).then_some(i)).collect::<BTreeSet<_>>();
        let cylinder = Term::from([(9101, 5)]);
        let bevel = Term::from([(9202, 1)]);
        let cap = Term::from([(9101, 2), (9303, 1)]);
        let bottom = Term::from([(9303, 2)]);
        let left = Term::from([(9303, 4)]);
        let right = Term::from([(9303, 6)]);
        let indices = BTreeMap::from([
            (cylinder.clone(), family("F5:side:")),
            (bevel.clone(), family("F3:blend:")),
            (cap.clone(), family("F5:top")),
            (bottom.clone(), family("F7:bottom")),
            (left.clone(), family("F7:side:l2")),
            (right.clone(), family("F7:side:l4")),
        ]);
        assert!(indices.values().all(|v| !v.is_empty()));
        let vertex =
            |faces: Vec<Term>| recipe_incidence::Vertex { faces: faces.into_iter().collect(), seams: BTreeSet::new(), signatures: vec![[8; 8]] };
        let circle = recipe_incidence::Edge { faces: [cylinder.clone(), bevel.clone()], ends: vec![vertex(vec![cylinder.clone(), bevel.clone()])] };
        let selected = edges::resolve(body, &circle, &indices).unwrap();
        assert_eq!(selected.len(), 2, "the whole source circle spans both kernel seams");
        let edge_names = solidcraft_doc::naming::edge_names(body);
        let expected_radius = diameter / 2. - 2.;
        for selected in &selected {
            let i = edge_names.iter().position(|n| n == &selected.name).unwrap();
            let carrier = solidcraft_kernel::edge_circle(&body.body, i).unwrap();
            assert!((carrier.radius - expected_radius).abs() < 1e-7 && (carrier.center.z - 40.).abs() < 1e-7);
        }
        let vertical = recipe_incidence::Edge {
            faces: [cylinder.clone(), left.clone()],
            ends: vec![vertex(vec![cylinder.clone(), left.clone(), bottom.clone()]), vertex(vec![cylinder.clone(), left.clone(), cap])],
        };
        let selected = edges::resolve(body, &vertical, &indices).unwrap();
        assert_eq!(selected.len(), 1);
        let pick = selected[0].point;
        assert!((pick.z - 50.).abs() < 1e-7 && (pick.x.abs() - 1.875).abs() < 1e-7);
        // Boolean intersections are certified at the kernel's model-scale
        // tolerance, rather than exact arithmetic (about 0.5 nm here).
        assert!(
            (pick.y + (expected_radius * expected_radius - 1.875_f64.powi(2)).sqrt()).abs() < positional_tolerance,
            "r={expected_radius}: {pick:?}"
        );
        let root = recipe_incidence::Edge {
            faces: [cylinder.clone(), bottom.clone()],
            ends: vec![vertex(vec![cylinder.clone(), bottom.clone(), left]), vertex(vec![cylinder.clone(), bottom, right])],
        };
        let selected = edges::resolve(body, &root, &indices).unwrap();
        assert_eq!(selected.len(), 1);
        let i = edge_names.iter().position(|n| n == &selected[0].name).unwrap();
        let samples = &body.mesh().edges[i];
        // Test against the analytic carrier itself. A circle fitted to a short
        // Boolean intersection arc magnifies endpoint noise into radius error.
        assert!(
            samples
                .iter()
                .all(|p| ((p.x * p.x + p.y * p.y).sqrt() - expected_radius).abs() < positional_tolerance && (p.z - 45.).abs() < positional_tolerance)
        );
        let a = samples.first().unwrap();
        let b = samples.last().unwrap();
        let angle = (a.x * b.y - a.y * b.x).atan2(a.x * b.x + a.y * b.y).abs();
        assert!((angle - 2. * (1.875 / expected_radius).asin()).abs() < 2. * positional_tolerance / expected_radius);
        let mut incomplete = indices.clone();
        incomplete.get_mut(&cylinder).unwrap().pop_first();
        assert!(edges::resolve(body, &circle, &incomplete).is_err(), "half a recovered ring must not count as a closed selection");
        let mut collapsed = indices;
        collapsed.insert(bevel, collapsed[&cylinder].clone());
        assert!(edges::resolve(body, &circle, &collapsed).is_err(), "distinct native surfaces cannot share reconstructed faces");
    }
}

#[test]
fn original_chamfer_ring_blends_match_analytic_removed_or_added_volume() {
    let fixtures = [
        ("neck", 8., -1., include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-ring-neck-fillet.solidcraft").as_slice()),
        ("base", 10., 1., include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-ring-base-fillet.solidcraft").as_slice()),
    ];
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&original());
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let stock = model.state().body("Block").unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
    // Integrate pi*r(z)^2 over the circular fillet's axial interval. The cone
    // has dr/dz=-2/3. Its tangent circle is offset from the vertical wall by
    // the blend radius and from the corner by R*(sqrt(1+k^2)-1)/k axially.
    let k: f64 = 2. / 3.;
    let q = (1. + k * k).sqrt();
    for (name, radius, sign, bytes) in fixtures {
        for r in [1_f64, 1.5] {
            let c = r * (q - 1.) / k;
            let b = r * k / q - c;
            let a = radius - sign * r;
            let primitive = |t: f64| (a * a + r * r) * t - t.powi(3) / 3. + sign * a * (t * (r * r - t * t).sqrt() + r * r * (t / r).asin());
            let rounded = if sign < 0. { primitive(0.) - primitive(-r * k / q) } else { primitive(r * k / q) - primitive(0.) };
            let unrounded = radius * radius * (b + c) - sign * radius * k * b * b + k * k * b.powi(3) / 3.;
            let delta = std::f64::consts::PI * (rounded - unrounded);
            let mut doc = crate::read_design(bytes).unwrap();
            doc.change_param("round_radius", &format!("{r} mm"), None, None).unwrap();
            let doc = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            model.evaluate(&doc);
            assert!(model.results.iter().all(|r| r.error.is_none()));
            let actual = model.state().body("Block").unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
            assert!((actual - stock - delta).abs() < 0.02, "{name} R={r}: volume change {} != {delta}", actual - stock);
            assert!((actual - (16046.640042604115 + delta)).abs() < 0.5, "{name} R={r}: total volume {actual}");
        }
    }
}

#[test]
#[ignore = "requires the local Style 5 archive"]
fn native_composite_blend_retains_fillet_and_later_editable_features() {
    let bytes = std::fs::read(std::env::var("SOLIDCRAFT_COMPOSITE_ARCHIVE").unwrap()).unwrap();
    let outcome = import_f3d_with_preference(&bytes, "Joined blend", ImportPreference::Editability).unwrap();
    let doc = outcome.document;
    for feature in &doc.features {
        println!("FEATURE {} {}", feature.id, feature.name);
    }
    for warning in &doc.import_warnings {
        println!("WARNING {warning}");
    }
    assert!(doc.find_feature("Fillet3").is_some(), "composite wing fillet must import");
    assert_eq!(doc.find_feature("Fillet1").expect("combined corner and rings must import").edge_names.len(), 7);
    assert!(doc.find_feature("Sketch5").is_some() && doc.find_feature("Extrude7").is_some(), "retain later editable history");
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(doc, reopened);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&reopened);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    assert!(solidcraft_kernel::measure(&model.state().bodies[0].body).unwrap().volume > 0.);
    let fillet = doc.find_feature("Fillet3").unwrap().clone();
    let FeatureKind::Fillet { radius, body: Some(body_name), .. } = &fillet.kind else { panic!("named constant fillet") };
    let old_radius = doc.eval(radius, solidcraft_doc::expr::Kind::Length).unwrap();
    let stock = model.state().body(body_name).unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
    let mut unrounded = doc.clone();
    unrounded.features.iter_mut().find(|f| f.id == fillet.id).unwrap().suppressed = true;
    model.evaluate(&unrounded);
    let state = model.state();
    let body = state.body(body_name).unwrap();
    let names = solidcraft_doc::naming::edge_names(body);
    let mesh = body.mesh();
    let total_length: f64 = fillet
        .edge_names
        .iter()
        .map(|name| {
            let i = names.iter().position(|n| n == name).unwrap();
            mesh.edges[i].first().unwrap().dist(*mesh.edges[i].last().unwrap())
        })
        .sum();
    let new_radius = old_radius * 0.75;
    let mut edited = doc.clone();
    edited.change_param(&fillet.param_names[0], &format!("{new_radius} mm"), None, None).unwrap();
    let edited = crate::read_design(&crate::write_design_container(&edited).unwrap()).unwrap();
    model.evaluate(&edited);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let volume = model.state().body(body_name).unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
    let expected = stock + total_length * (old_radius * old_radius - new_radius * new_radius) * (1. - std::f64::consts::FRAC_PI_4);
    assert!((volume - expected).abs() < 0.02, "native radius edit {old_radius} -> {new_radius}, length={total_length}: {volume} != {expected}");
    let combined = doc.find_feature("Fillet1").unwrap();
    let FeatureKind::Fillet { radius, body: Some(body_name), .. } = &combined.kind else { panic!("combined constant fillet") };
    let old = doc.eval(radius, solidcraft_doc::expr::Kind::Length).unwrap();
    let mut edited = doc.clone();
    edited.change_param(&combined.param_names[0], &format!("{} mm", old * 0.75), None, None).unwrap();
    let edited = crate::read_design(&crate::write_design_container(&edited).unwrap()).unwrap();
    let FeatureKind::Fillet { radius: edited_radius, .. } = &edited.find_feature("Fillet1").unwrap().kind else { panic!("edited constant fillet") };
    assert!((edited.eval(edited_radius, solidcraft_doc::expr::Kind::Length).unwrap() - old * 0.75).abs() < 1e-10);
    model.evaluate(&edited);
    assert!(model.results.iter().all(|r| r.error.is_none()), "combined native radius edit must regenerate");
    let volume = model.state().body(body_name).unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
    assert!((volume - stock).abs() > 1e-6, "combined radius edit must change geometry");
}

fn authored_selector() -> crate::f3d::import_tests::Fixture {
    use crate::f3d::relationship_tests::{reference, refs, string, u32, u64, wide};
    let mut f = crate::f3d::import_tests::fixture(false);
    let mut outputs = Vec::new();
    refs(&mut outputs, &[80]);
    f.0.get_mut(&9).unwrap().2.splice(151..155, outputs);
    let mut handle = vec![0; 6];
    wide(&mut handle, "authored-body");
    handle.extend([0, 1]);
    reference(&mut handle, 81);
    reference(&mut handle, 9);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 4);
    f.put(80, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2, handle, Some(secondary));
    f.put(81, "D3937028-C20C-4E65-B010-94AD418A5C20", 19, vec![], None);
    let mut selector = vec![0; 8];
    reference(&mut selector, 302);
    u32(&mut selector, 1);
    wide(&mut selector, "authored-selected-body");
    wide(&mut selector, "authored-selected-owner");
    u32(&mut selector, 2);
    u32(&mut selector, 0);
    refs(&mut selector, &[303]);
    refs(&mut selector, &[80]);
    selector.extend([0; 56]);
    reference(&mut selector, 19);
    f.put(301, super::super::body_projections::SELECTOR, 3, selector, None);
    let mut recipe = vec![0; 6];
    for n in [1, 3, 4] {
        u32(&mut recipe, n);
    }
    for term in [vec![(8001, 2), (8002, 1)], vec![(8002, 19)], vec![(8002, 42)], vec![(8002, 65)]] {
        u32(&mut recipe, term.len() as u32);
        for (op, label) in term {
            string(&mut recipe, &label.to_string());
            u32(&mut recipe, 0);
            u32(&mut recipe, 1);
            u64(&mut recipe, op);
        }
    }
    u32(&mut recipe, 0);
    string(&mut recipe, "edge_recipe_data");
    for n in [-1_i32, -1, 2, 0, -1, 1, -1, 2, -1, 3, 1, -1, 0, -1, 2, -1, 0, 0, -1, 3, 1, -1, 3, -1, 0, -1, 0, 0, -1] {
        u32(&mut recipe, n as u32);
    }
    f.put(302, "7ACC2A03-0261-4879-A14A-A93D661A5BDC", 3, recipe, None);
    f.put(303, "50AEE8B4-9456-4DD7-847D-6F6C909BBB9D", 0, vec![], None);
    f
}

#[test]
fn joined_blend_recipe_preserves_contributors_and_requires_owned_complete_records() {
    let parse = |f: crate::f3d::import_tests::Fixture, owner| {
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        decode(&Records::new(&d), &classes, 301, owner)
    };
    let (creator, identity) = parse(authored_selector(), 19).unwrap();
    assert_eq!(creator, 9);
    assert_eq!(identity.terms[0], Term::from([(8001, 2), (8002, 1)]));
    assert_eq!(identity.edges.len(), 1);
    assert_eq!(identity.edges[0].ends[0].faces.len(), 3);
    assert!(parse(authored_selector(), 9).is_err());
    for id in [80, 301, 302] {
        for keep in 0..authored_selector().0[&id].2.len() {
            let mut f = authored_selector();
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(parse(f, 19).is_err(), "truncated {id}/{keep}");
        }
    }
    let mut f = authored_selector();
    f.0.get_mut(&303).unwrap().0 = "unsupported-cache".into();
    assert!(parse(f, 19).is_err(), "unknown cached carriers must be refused");
    let mut f = authored_selector();
    f.0.get_mut(&302).unwrap().2.extend([0; 4]);
    assert!(parse(f, 19).is_err());
}

// Green's theorem for the vertical root's cross-section: the straight wing
// wall, a circle of radius r tangent to that wall and the R=8 cylinder, and
// the original cylinder arc. Multiply the added area by the 10 mm wing depth.
fn wing_vertical_volume(r: f64) -> f64 {
    let (radius, w) = (8_f64, 1.875_f64);
    let p = (w, -(radius * radius - w * w).sqrt());
    let c = (w + r, -((radius + r).powi(2) - (w + r).powi(2)).sqrt());
    let a = (w, c.1);
    let b = (c.0 * radius / (radius + r), c.1 * radius / (radius + r));
    let cross = |a: (f64, f64), b: (f64, f64)| a.0 * b.1 - a.1 * b.0;
    let arc = (b.1 - c.1).atan2(b.0 - c.0) - std::f64::consts::PI;
    5. * (cross(p, a) + cross(c, (b.0 - a.0, b.1 - a.1)) + r * r * arc + radius * radius * (p.1.atan2(p.0) - b.1.atan2(b.0)))
}

// The lower root is a torus trimmed by the two wing side planes x=+/-w.
// At z=45-r+r*sin(t), its outside radius is R+r-r*cos(t). Integrate the
// difference of circular-strip areas over z, independently of the B-rep mesh.
fn wing_bottom_volume(r: f64) -> f64 {
    let (radius, w) = (8_f64, 1.875_f64);
    let area = |rho: f64| w * (rho * rho - w * w).sqrt() + rho * rho * (w / rho).asin();
    let f = |t: f64| (area(radius + r - r * t.cos()) - area(radius)) * r * t.cos();
    let n = 4096;
    let h = std::f64::consts::FRAC_PI_2 / f64::from(n);
    let sum: f64 = (1..n).map(|i| f(f64::from(i) * h) * if i % 2 == 0 { 2. } else { 4. }).sum();
    h / 3. * (f(0.) + f(std::f64::consts::FRAC_PI_2) + sum)
}

#[test]
fn original_partial_wing_fillets_add_material() {
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&original());
    let state = model.state();
    let body = &state.body("Block").unwrap().body;
    let stock = body.tessellate_with(1e-4, true).unwrap().measure().volume;
    let root = Vec3::new(1.875, -(64_f64 - 1.875_f64.powi(2)).sqrt(), 50.);
    for (name, point, reference) in
        [("vertical", root, wing_vertical_volume as fn(f64) -> f64), ("bottom", Vec3::new(0., -8., 45.), wing_bottom_volume as fn(f64) -> f64)]
    {
        for r in [1., 1.5] {
            let result = solidcraft_kernel::fillet(body, &[point], r).unwrap_or_else(|e| panic!("{name} R={r}: {e}"));
            assert!(result.validity().is_empty(), "{name}: {:?}", result.validity());
            assert_eq!(result.crossed_face_loops().unwrap(), 0);
            let volume = result.tessellate_with(1e-4, true).unwrap().measure().volume;
            let expected = reference(r);
            assert!((volume - stock - expected).abs() < 0.02, "{name} R={r}: added {} != {expected}", volume - stock);
        }
    }
    assert!(solidcraft_kernel::fillet(body, &[Vec3::new(0., -8., 45.)], 6.).is_err(), "the lower round must not cross the shoulder below it");
    assert!(solidcraft_kernel::fillet(body, &[root], 100.).is_err(), "the root round must not consume the wing tip");
}

// At a fixed radius rho, the wing's rounded footprint occupies an angle on
// either side of -Y. Up to the vertical round's planar contact this angle is
// the intersection of the cylinder and fillet circles; past it the wing is
// bounded by x=+/-w. Integrate the bottom torus below z=45 over that footprint.
fn wing_combined_volume(r: f64) -> f64 {
    let (radius, w) = (8_f64, 1.875_f64);
    let d = radius + r;
    let contact = (d * d - (w + r).powi(2) + w * w).sqrt();
    let alpha = ((w + r) / d).asin();
    let f = |t: f64| {
        let rho = d - r * t.cos();
        let theta = if rho < contact { alpha - ((rho * rho + d * d - r * r) / (2. * rho * d)).clamp(-1., 1.).acos() } else { (w / rho).asin() };
        2. * rho * theta * r * r * t.sin() * (1. - t.sin())
    };
    let n = 4096;
    let h = std::f64::consts::FRAC_PI_2 / f64::from(n);
    let sum: f64 = (1..n).map(|i| f(f64::from(i) * h) * if i % 2 == 0 { 2. } else { 4. }).sum();
    2. * wing_vertical_volume(r) + h / 3. * (f(0.) + f(std::f64::consts::FRAC_PI_2) + sum)
}

#[test]
fn original_combined_wing_fillets() {
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&original());
    let state = model.state();
    let body = &state.body("Block").unwrap().body;
    let stock = body.tessellate_with(2.5e-5, true).unwrap().measure().volume;
    let y = -(64_f64 - 1.875_f64.powi(2)).sqrt();
    let points = [Vec3::new(1.875, y, 50.), Vec3::new(-1.875, y, 50.), Vec3::new(0., -8., 45.)];
    for r in [1., 1.5] {
        for reversed in [false, true] {
            let mut selection = points.to_vec();
            if reversed {
                selection.reverse();
            }
            let result = solidcraft_kernel::fillet(body, &selection, r).unwrap();
            assert!(result.validity().is_empty());
            assert_eq!(result.crossed_face_loops().unwrap(), 0);
            let volume = result.tessellate_with(2.5e-5, true).unwrap().measure().volume;
            let expected = wing_combined_volume(r);
            println!("R={r}, reversed={reversed}: delta={}, expected={expected}", volume - stock);
            assert!((volume - stock - expected).abs() < 0.02, "R={r}, reversed={reversed}: added {} != {expected}", volume - stock);
            for x in [-2.05, 2.05] {
                let p = Vec3::new(x, -7.8, 44.8);
                assert!(!body.contains(p));
                assert!(result.contains(p), "the meeting fillets must fill the corner at {p:?}");
            }
        }
    }
    let mut all = points.to_vec();
    all.extend([Vec3::new(0., 8., 40.), Vec3::new(0., -8., 40.), Vec3::new(0., 10., 37.), Vec3::new(0., -10., 37.)]);
    let result = solidcraft_kernel::fillet(body, &all, 1.).unwrap();
    assert!(result.validity().is_empty());
    assert_eq!(result.crossed_face_loops().unwrap(), 0);
    let delta = result.tessellate_with(2.5e-5, true).unwrap().measure().volume - stock;
    let expected = wing_combined_volume(1.) + 0.44292588330146027 - 0.5494281346105299;
    assert!((delta - expected).abs() < 0.02, "all seven edges: added {delta} != {expected}");
}

#[test]
fn saved_combined_wing_radii_reopen_with_analytic_volumes() {
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&original());
    let stock = model.state().body("Block").unwrap().body.tessellate_with(2.5e-5, true).unwrap().measure().volume;
    for (bytes, shoulders) in [
        (include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-combined-corner-fillet.solidcraft").as_slice(), false),
        (include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-combined-all-fillet.solidcraft").as_slice(), true),
    ] {
        for (r, ring_delta) in [(1., 0.44292588330146027 - 0.5494281346105299), (1.5, 0.9986978471864474 - 1.2340986931155526)] {
            let mut doc = crate::read_design(bytes).unwrap();
            doc.change_param("round_radius", &format!("{r} mm"), None, None).unwrap();
            let doc = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            model.evaluate(&doc);
            assert!(model.results.iter().all(|r| r.error.is_none()));
            let actual = model.state().body("Block").unwrap().body.tessellate_with(2.5e-5, true).unwrap().measure().volume;
            let expected = wing_combined_volume(r) + if shoulders { ring_delta } else { 0. };
            assert!((actual - stock - expected).abs() < 0.02, "saved R={r}, shoulders={shoulders}: added {} != {expected}", actual - stock);
        }
    }
}

#[test]
fn tracked_fillet_faces_follow_selected_inputs_through_radius_and_height_edits() {
    for height in [40., 42.] {
        for radius in [0.75, 1.25] {
            let mut doc = original();
            doc.change_param("height", &format!("{height} mm"), None, None).unwrap();
            let mut model = solidcraft_doc::Model::new();
            model.evaluate(&doc);
            let state = model.state();
            let (top, top_indices, top_selection) = selected(&doc, &state, true);
            let (bottom, bottom_indices, bottom_selection) = selected(&doc, &state, false);
            doc.features.push(
                serde_json::from_value(json!({
                    "id":8,"name":"Tracked wing tips","type":"fillet","body":"Block",
                    "edges":[top_selection.point,bottom_selection.point],
                    "edge_names":[top_selection.name,bottom_selection.name],"radius":format!("{radius} mm")
                }))
                .unwrap(),
            );
            doc.next_id = 9;
            let doc = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            model.evaluate(&doc);
            assert!(model.results.iter().all(|r| r.error.is_none()));
            let after = model.state();
            let body = after.body("Block").unwrap();
            let feature = doc.feature(8).unwrap();
            let mut families = Vec::new();
            for (edge, indices) in [(top, top_indices), (bottom, bottom_indices)] {
                let indices = indices.into_iter().map(|(term, index)| (term, BTreeSet::from([index]))).collect();
                let names = faces::tracked_names(feature, state.body("Block").unwrap(), &edge, &indices).unwrap();
                assert_eq!(names.len(), 1);
                let family = solidcraft_doc::naming::face_lineage_indices(body, &names[0]).unwrap();
                assert_eq!(family.len(), 1, "each tracked input produces a distinct verified fillet face");
                families.push(family);
                let mut unrelated = feature.clone();
                unrelated.edge_names.clear();
                assert!(faces::tracked_names(&unrelated, state.body("Block").unwrap(), &edge, &indices).is_err());
            }
            assert_ne!(families[0], families[1]);
        }
    }
}

#[test]
#[ignore = "requires the local Style 5 archive"]
fn native_later_fillet_caches_and_source_face_labels_are_owned() {
    let bytes = std::fs::read(std::env::var("SOLIDCRAFT_COMPOSITE_ARCHIVE").unwrap()).unwrap();
    let designs = read_f3d(&bytes).unwrap();
    let design = &designs[0];
    let records = Records::new(design);
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    for selector in (2952..=3000).step_by(4) {
        let (_, identity) = decode(&records, &classes, selector, 2361).unwrap();
        assert_eq!(identity.edges.len(), 1);
    }
    let mut doc = import_f3d_with_preference(&bytes, "Fillet output tracking", ImportPreference::Editability).unwrap().document;
    if let Some(index) = doc.features.iter().position(|f| f.name == "Fillet6") {
        doc.features.truncate(index);
    }
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
    let state = model.state();
    let context = Context {
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
    let creator = doc.find_feature("Extrude1").unwrap();
    let body = state.body(&creator.body_names[0]).unwrap();
    for (operation, label) in [(321, 1), (321, 2), (321, 3), (323, 1), (323, 2)] {
        let faces = context.face_indices(&Term::from([(operation, label)]), creator.id, body).unwrap();
        assert!(!faces.is_empty());
        println!("FILLET FACE {operation}/{label}: {faces:?}");
    }
    let mut failures = Vec::new();
    let mut names = BTreeSet::new();
    for selector in (2952..=3000).step_by(4) {
        match context.resolve(selector, 2361) {
            Ok(selected) => {
                assert!(!selected.is_empty());
                println!("FILLET6 {selector}: {:?}", selected.iter().map(|s| &s.name).collect::<Vec<_>>());
                for edge in selected {
                    assert!(names.insert(edge.name), "native selectors must not reuse a kernel span");
                }
            }
            Err(error) => {
                let (_, identity) = decode(&records, &classes, selector, 2361).unwrap();
                for term in &identity.terms {
                    if term.iter().all(|(_, label)| *label > 0) {
                        println!("TERM {selector} {term:?}: {:?}", context.face_indices(term, creator.id, body));
                    }
                }
                failures.push(format!("{selector}: {error}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(names.len(), 17, "thirteen native edges cover seventeen distinct kernel spans");
}

#[test]
fn noncircular_blend_intersections_require_unique_faces_and_both_endpoints() {
    let doc = crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-combined-corner-fillet.solidcraft"))
        .unwrap();
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let body = state.body("Block").unwrap();
    let mesh = body.mesh();
    let term = |face: u32| Term::from([(9001, face as i32 + 1)]);
    let indices = mesh.edge_faces.iter().flatten().map(|f| (term(*f), BTreeSet::from([*f as usize]))).collect();
    let tolerance = 1e-7 * (1. + body.body.size());
    let at = |p: Vec3| -> BTreeSet<Term> {
        mesh.edges
            .iter()
            .zip(&mesh.edge_faces)
            .filter(|(points, _)| points.first().is_some_and(|v| v.dist(p) <= tolerance) || points.last().is_some_and(|v| v.dist(p) <= tolerance))
            .flat_map(|(_, faces)| faces.iter().map(|f| term(*f)))
            .collect()
    };
    let names = solidcraft_doc::naming::edge_names(body);
    let mut checked = 0;
    for (index, (points, faces)) in mesh.edges.iter().zip(&mesh.edge_faces).enumerate() {
        let [first, second] = faces.as_slice() else { continue };
        let start = *points.first().unwrap();
        let end = *points.last().unwrap();
        let length = start.dist(end);
        if length <= tolerance || solidcraft_kernel::edge_circle(&body.body, index).is_some() {
            continue;
        }
        let axis = (end - start) * (1. / length);
        if points.iter().all(|p| (*p - start).cross(axis).len() <= tolerance) {
            continue;
        }
        let a = at(start);
        let b = at(end);
        if a == b || a.len() < 3 || b.len() < 3 {
            continue;
        }
        let vertex = |faces| recipe_incidence::Vertex { faces, seams: BTreeSet::new(), signatures: vec![] };
        let mut edge = recipe_incidence::Edge { faces: [term(*first), term(*second)], ends: vec![vertex(a), vertex(b)] };
        let selected = edges::resolve(body, &edge, &indices).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, names[index]);
        edge.ends[1] = edge.ends[0].clone();
        assert!(edges::resolve(body, &edge, &indices).is_err());
        checked += 1;
    }
    assert!(checked >= 2, "fixture must exercise curved intersection edges, found {checked}");
}

#[test]
fn touching_tip_rounds_form_one_owned_face_family_only_at_the_meeting_radius() {
    let bytes = include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-tip-fillet.solidcraft");
    let mut model = solidcraft_doc::Model::new();
    for height in [40., 42.] {
        for radius in [0.75, 1.] {
            let mut doc = crate::read_design(bytes).unwrap();
            for (name, value) in [("height", height), ("wing_depth", 2.), ("tip_radius", radius)] {
                doc.change_param(name, &format!("{value} mm"), None, None).unwrap();
            }
            let doc = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            model.evaluate(&doc);
            assert!(model.results.iter().all(|r| r.error.is_none()));
            let feature = doc.find_feature("Wing tip blend").unwrap();
            let state = model.state();
            let body = state.body("Block").unwrap();
            let before = model.state_before(feature.id);
            let stock = before.body("Block").unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
            let actual = body.body.tessellate_with(1e-4, true).unwrap().measure().volume;
            let removed = 2. * 3.75 * radius * radius * (1. - std::f64::consts::FRAC_PI_4);
            assert!((stock - actual - removed).abs() < 0.02);
            let mut families = Vec::new();
            for name in &feature.edge_names {
                let selected =
                    solidcraft_doc::naming::face_lineage_indices(body, &format!("F{}:blend:{name}", feature.id)).unwrap().into_iter().collect();
                let family = faces::fillet_family(body, feature, &selected).unwrap();
                assert_eq!(family.len(), if radius == 1. { 2 } else { 1 });
                families.push(family);
                let mut foreign = feature.clone();
                foreign.edge_names.retain(|edge| edge == name);
                if radius == 1. {
                    assert!(faces::fillet_family(body, &foreign, &selected).is_err(), "cannot expand across unowned lineage");
                }
            }
            assert_eq!(families[0] == families[1], radius == 1.);
        }
    }
}

#[test]
#[ignore = "requires a local joined asymmetric chamfer archive"]
fn native_joined_chamfer_retains_primary_cap_and_cut_profiles() {
    let bytes = std::fs::read(std::env::var("SOLIDCRAFT_JOINED_CHAMFER_ARCHIVE").unwrap()).unwrap();
    let designs = read_f3d(&bytes).unwrap();
    let design = &designs[0];
    let mut doc = import_f3d_with_preference(&bytes, "Joined chamfer", ImportPreference::Editability).unwrap().document;
    let end = doc.features.iter().position(|f| f.name == "Extrude6").unwrap();
    doc.features.truncate(end + 1);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let history = design.history().unwrap();
    let native = history.iter().find(|f| f.name == "Chamfer2").unwrap();
    let HistoryKind::Chamfer { sets } = &native.kind else { panic!("chamfer") };
    let features = history.iter().filter_map(|native| doc.find_feature(&native.name).map(|f| (native.id, f.id))).collect();
    let operations = design.operation_owners().unwrap();
    let all_profiles = design.sketch_profiles().unwrap();
    let profiles = all_profiles.iter().map(|p| (p.feature, p)).collect();
    let lines = design.sketch_lines().unwrap().into_iter().map(|line| (line.id, line)).collect();
    let circles = design.sketch_circles().unwrap().into_iter().map(|circle| (circle.id, circle)).collect();
    let records = Records::new(design);
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    let state = model.state();
    assert!(state.bodies[0].body.history().is_some(), "the cut must retain history across internal wall stations");
    let context = Context {
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
    for set in sets {
        let selectors = super::super::loft_faces::objects(&records, &classes, set.container, native.id).unwrap();
        let group = context.resolve_group(&selectors, native.id).unwrap();
        assert_eq!(group.len(), 2, "both symmetric finite arcs must be covered together");
        for selector in selectors {
            let (creator, identity) = decode(&records, &classes, selector, native.id).unwrap();
            println!("SELECTOR {selector} IDENTITY {identity:?}");
            let mut r = payload(records.primary(selector).unwrap(), selector).unwrap();
            zeros(&mut r, if super::super::loft_faces::selector_version(&classes, selector).unwrap() == 2 { 7 } else { 8 }).unwrap();
            let id = r.reference().unwrap();
            let (_, primary) = recipe_incidence::identity_with_primary(&records, &classes, id).unwrap();
            assert_eq!(primary, Term::from([(308, 2)]));
            let body = &state.bodies[0];
            let caps = context.face_indices(&primary, features[&creator], body).unwrap();
            assert_eq!(caps.len(), 1);
            let plane = body.body.planar_face(*caps.first().unwrap()).unwrap();
            assert!((plane.origin.z - 22.5).abs() < 1e-7);
            let cut_native = history.iter().find(|f| f.name == "Extrude6").unwrap().id;
            let profile = profiles[&cut_native];
            assert_eq!(profile.regions.len(), 2);
            let FeatureKind::Extrude { sketch, .. } = &doc.feature(features[&cut_native]).unwrap().kind else { panic!("cut") };
            for (op, label) in identity.terms.iter().flatten().filter(|(op, _)| operations[op] == cut_native) {
                let curve = super::super::blend_sides::curve(
                    &records,
                    &classes,
                    cut_native,
                    *label as u64,
                    super::super::blend_sides::Source { profile, lines: &lines, circles: &circles, solved: state.sketch(sketch.0).unwrap() },
                )
                .unwrap();
                assert_eq!(curve, 3448);
                println!("CUT SIDE {op}/{label}: curve {curve}");

                let family = solidcraft_doc::naming::face_family_indices(body, &format!("F{}:side:c{curve}", features[&cut_native])).unwrap();
                assert!(!family.is_empty());
                println!("CUT FAMILY {family:?}");
            }
            println!("RESOLVED {:?}", context.resolve(selector, native.id).map(|s| s.into_iter().map(|s| (s.name, s.point)).collect::<Vec<_>>()));
        }
    }
}

#[test]
fn joined_asymmetric_chamfer_preserves_setbacks_flip_and_edits() {
    // This record is authored here, with arbitrary labels and two contributors
    // to the cap. Its cyclic incidence designates the cap as the primary face.
    let design = authored_selector().design();
    let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    let (_, identity, primary) = decode_oriented(&Records::new(&design), &classes, 301, 19, true).unwrap();
    assert_eq!(primary.as_ref(), identity.terms.first());

    let source = original();
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&source);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let body = state.body("Block").unwrap();
    let stock = body.body.tessellate_with(1e-4, true).unwrap().measure();
    for top in [false, true] {
        let (edge, indices, selected) = selected(&source, &state, top);
        let families = indices.iter().map(|(t, i)| (t.clone(), BTreeSet::from([*i]))).collect();
        for native_flip in [false, true] {
            let flip = asymmetric_flip(body, &edge, &families, &edge.faces[0], native_flip).unwrap();
            assert_eq!(flip, !top ^ native_flip);
            let mut doc = source.clone();
            doc.features.push(
                serde_json::from_value(json!({
                    "id":8,"name":"Asymmetric wing chamfer","type":"chamfer","edges":[selected.point],
                    "edge_names":[selected.name],"param_names":["d9","d10"],"distance":"1 mm","distance2":"2 mm","flip":flip,"body":"Block"
                }))
                .unwrap(),
            );
            doc.next_id = 9;
            for first in [1., 1.5] {
                if let FeatureKind::Chamfer { distance, .. } = &mut doc.features.last_mut().unwrap().kind {
                    *distance = format!("{first} mm").into();
                }
                let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
                assert_eq!(doc, reopened);
                model.evaluate(&reopened);
                assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
                let state = model.state();
                let result = state.body("Block").unwrap();
                let mesh = result.body.tessellate_with(1e-4, true).unwrap();
                let measured = mesh.measure();
                let (cap, wall) = if native_flip { (2., first) } else { (first, 2.) };
                let z = if top { 55. } else { 45. };
                let direction = if top { -1. } else { 1. };
                let removed = 3.75 * cap * wall / 2.;
                let centroid = Vec3::new(0., -25. + cap / 3., z + direction * wall / 3.);
                let expected = (stock.centroid * stock.volume - centroid * removed) / (stock.volume - removed);
                assert!((measured.volume - (stock.volume - removed)).abs() < 0.02);
                assert!(measured.centroid.dist(expected) < 1e-4, "setback orientation must preserve first moments");
                for (y, z) in [(-25. + cap, z), (-25., z + direction * wall)] {
                    assert!(
                        mesh.edges.iter().any(|e| e.len() >= 2 && e.iter().all(|p| (p.y - y).abs() < 1e-7 && (p.z - z).abs() < 1e-7)),
                        "missing setback at y={y}, z={z}"
                    );
                }
            }
        }
    }
}
