//! Original wing geometry: a perimeter that returns to a face it also follows.
use serde_json::json;
use solidcraft_doc::{Document, FeatureKind, Model};
use solidcraft_geom::Vec3;

fn evaluate(model: &mut Model, doc: &Document) {
    model.evaluate(doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().filter_map(|r| r.error.as_ref()).collect::<Vec<_>>());
}

fn add_round(doc: &mut Document, model: &mut Model, name: &str, points: &[[f64; 3]], radius: f64) -> u64 {
    evaluate(model, doc);
    let state = model.state();
    let body = state.body("Block").unwrap();
    let names = solidcraft_doc::naming::edge_names(body);
    let selected: Vec<_> = points
        .iter()
        .map(|p| {
            let (i, distance) = body.body.nearest_edge(Vec3::from(*p), 1e-4).unwrap().unwrap();
            assert!(distance < 0.02);
            names[i].clone()
        })
        .collect();
    assert_eq!(selected.iter().collect::<std::collections::BTreeSet<_>>().len(), points.len());
    let id = doc.next_id;
    doc.next_id += 1;
    doc.features.push(
        serde_json::from_value(json!({
            "id":id,"name":name,"type":"fillet","body":"Block",
            "edges":points,"edge_names":selected,"radius":format!("{radius} mm")
        }))
        .unwrap(),
    );
    doc.name_feature_inputs(id);
    id
}

#[test]
fn pinched_perimeter_preserves_closed_geometry_edits_and_saved_history() {
    let mut doc =
        crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/partial-cylinder-touching-tip-fillets.solidcraft")).unwrap();
    let mut model = Model::new();
    add_round(
        &mut doc,
        &mut model,
        "Root and shoulder rounds",
        &[
            [1.875, -7.77717011515114, 54.],
            [-1.875, -7.77717011515114, 54.],
            [0., -8., 53.],
            [0., 8., 40.],
            [0., -8., 40.],
            [0., 10., 37.],
            [0., -10., 37.],
        ],
        1.,
    );
    evaluate(&mut model, &doc);
    let before = model.state().body("Block").unwrap().body.tessellate_with(1e-4, true).unwrap().measure().volume;
    let perimeter = add_round(
        &mut doc,
        &mut model,
        "Perimeter round",
        &[
            [2.101482879, -7.896426913, 52.557629203],
            [1.875, -8.665482291, 52.981726923],
            [1.875, -16.401259765, 53.],
            [1.875, -24.707106781, 54.707106781],
            [1.875, -24.707106781, 53.292893219],
            [1.875, -16.264222526, 55.],
            [2.062767138, -7.945111718, 55.],
            [-6.493737025, -4.662629459, 55.],
            [6.493737024, -4.662629456, 55.],
            [0., 8., 55.],
            [-2.062767138, -7.945111718, 55.],
            [-1.875, -16.264222526, 55.],
            [-1.875, -24.707106781, 53.292893219],
            [-1.875, -24.707106781, 54.707106781],
            [-1.875, -16.401259765, 53.],
            [-1.875, -8.665482291, 52.981726923],
            [-2.101482879, -7.896426913, 52.557629203],
        ],
        0.25,
    );
    let base = doc.clone();
    let mut baseline: Option<f64> = None;
    for (radius, height, reversed) in [(0.25, 40., false), (0.25, 40., true), (0.25, 42., false), (0.4, 40., false)] {
        let mut doc = base.clone();
        doc.change_param("height", &format!("{height} mm"), None, None).unwrap();
        let feature = doc.features.iter_mut().find(|f| f.id == perimeter).unwrap();
        if let FeatureKind::Fillet { radius: value, edges, .. } = &mut feature.kind {
            *value = format!("{radius} mm").into();
            if reversed {
                edges.reverse();
                feature.edge_names.reverse();
            }
        }
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        evaluate(&mut model, &reopened);
        let state = model.state();
        let body = &state.body("Block").unwrap().body;
        assert!(body.validity().is_empty(), "{:?}", body.validity());
        assert_eq!(body.crossed_face_loops().unwrap(), 0);
        let mesh = body.tessellate_with(1e-4, true).unwrap();
        let volume = mesh.measure().volume;
        let height_added = (height - 40.) * 100. * std::f64::consts::PI;
        let removed = before + height_added - volume;
        // Four straight, right-angle runs are each at least 14 mm long.
        let minimum = 4. * 14. * radius * radius * (1. - std::f64::consts::FRAC_PI_4);
        assert!(removed > minimum && removed < 100. * radius * radius, "r={radius}: removed {removed}");
        if radius == 0.25 {
            if let Some(expected) = baseline {
                let tolerance = if height == 40. { 0.002 } else { 0.02 };
                assert!((volume - height_added - expected).abs() < tolerance, "height={height}: volume delta {}", volume - height_added - expected);
            } else {
                baseline = Some(volume);
            }
        }
        // Independent circular sections on the four long wing edges.
        for x_sign in [-1., 1.] {
            for z_sign in [-1., 1.] {
                let normal = Vec3::new(x_sign, 0., z_sign).normalized().unwrap();
                // Include stations near each curved junction, where fitting
                // through a neighbour used to bend an otherwise exact strip.
                for y in [-10., -16., -23.8] {
                    let centre = Vec3::new(x_sign * (1.875 - radius), y, height + 14. + z_sign * (1. - radius));
                    assert!(mesh.contains(centre + normal * (radius - 0.02)));
                    assert!(!mesh.contains(centre + normal * (radius + 0.02)));
                }
            }
        }
        let signed_volume = |origin: Vec3| {
            mesh.triangles
                .iter()
                .map(|t| {
                    let a = mesh.positions[t[0] as usize] - origin;
                    let b = mesh.positions[t[1] as usize] - origin;
                    let c = mesh.positions[t[2] as usize] - origin;
                    a.dot(b.cross(c)) / 6.
                })
                .sum::<f64>()
        };
        let origin_difference = (signed_volume(Vec3::ZERO) - signed_volume(Vec3::new(100., -50., 80.))).abs();
        // B-rep closure is checked above. Its 0.0001 mm tessellation is a
        // volume proxy, checked here to ten parts per million.
        assert!(
            origin_difference < volume.abs() * 1e-5,
            "radius={radius}, height={height}, origin shift={origin_difference}, volume={volume}, removed={removed}"
        );
    }
}
