//! Analytic corners for a join tool grown into an existing cylindrical wall.
use solidcraft_geom::{Seg2, Vec2};

pub(crate) fn concave_line_corner(prev: &Seg2, cur: &Seg2, g0: f64, g1: f64, corner: Vec2) -> Option<Vec2> {
    let (line, arc, growth) = match (prev, cur) {
        (Seg2::Line { .. }, Seg2::Arc { .. }) if g0 == 0.0 => (prev, cur, g1),
        (Seg2::Arc { .. }, Seg2::Line { .. }) if g1 == 0.0 => (cur, prev, g0),
        _ => return None,
    };
    let (Seg2::Line { a, b }, Seg2::Arc { center, radius, sweep, .. }) = (*line, *arc) else { return None };
    if !growth.is_finite() || growth <= 0.0 || sweep >= 0.0 || radius <= 2.0 * growth {
        return None;
    }
    let direction = (b - a).normalized()?;
    let foot = a + direction * (center - a).dot(direction);
    let r = radius - growth;
    let square = r * r - (foot - center).dot(foot - center);
    if !square.is_finite() || square <= 0.0 {
        return None;
    }
    let offset = direction * square.sqrt();
    let p = foot + offset;
    let q = foot - offset;
    let (near, far) = if p.dist(corner) < q.dist(corner) { (p, q) } else { (q, p) };
    // A nearly tangent or ambiguous crossing can move a corner far beyond the
    // sampled material. Leave those profiles to the ordinary Boolean path.
    (near.dist(corner) <= growth * 8.0 && far.dist(corner) > growth * 8.0).then_some(near)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_cylindrical_wall_join_preserves_volume_and_edits() {
        use crate::{Document, FeatureKind, Model, PlaneRef};
        use serde_json::json;
        use solidcraft_geom::{Plane, Vec3};
        use solidcraft_sketch::Sketch;
        let mut doc = Document::new("Partial cylindrical wall join");
        doc.set_param("wing_depth", "10 mm", Some("mm"), None).unwrap();
        let circle = |radius| {
            let mut sketch = Sketch::new();
            sketch.add_circle(Vec2::ZERO, radius, None, None).unwrap();
            sketch
        };
        let add_sketch = |doc: &mut Document, sketch, height| {
            doc.add_feature(
                FeatureKind::Sketch { plane: PlaneRef::Custom { plane: Plane::new(Vec3::new(0., 0., height), Vec3::X, Vec3::Y).unwrap() }, sketch },
                None,
            )
            .unwrap()
        };
        let add_extrude = |doc: &mut Document, sketch, distance, operation, profiles| {
            let feature: FeatureKind = serde_json::from_value(json!({
                "type":"extrude", "sketch":sketch, "profiles":profiles,
                "extent":{"distance":distance, "direction":"positive", "convention":"legacy_per_side"},
                "operation":operation
            }))
            .unwrap();
            doc.add_feature(feature, None).unwrap()
        };
        let base = add_sketch(&mut doc, circle(14.0), 0.0);
        add_extrude(&mut doc, base, "9", "new_body", json!({"type":"all"}));
        let neck = add_sketch(&mut doc, circle(7.0), 9.0);
        add_extrude(&mut doc, neck, "14", "join", json!({"type":"all"}));
        let mut sketch = circle(7.0);
        let corners = [Vec2::new(-1.875, -25.0), Vec2::new(1.875, -25.0), Vec2::new(1.875, 25.0), Vec2::new(-1.875, 25.0)];
        let ids: Vec<_> = corners.iter().map(|p| sketch.add_point(*p, None).unwrap()).collect();
        for i in 0..4 {
            sketch.add_line_pts(ids[i], ids[(i + 1) % 4], None).unwrap();
        }
        let wing = add_sketch(&mut doc, sketch, 23.0);
        add_extrude(&mut doc, wing, "-wing_depth", "join", json!({"type":"points", "points":[[0,-20]]}));
        let half_width = 1.875f64;
        let wing_area = 2.0 * half_width * 25.0 - (half_width * (49.0 - half_width * half_width).sqrt() + 49.0 * (half_width / 7.0).asin());
        let base_volume = std::f64::consts::PI * (14.0 * 14.0 * 9.0 + 7.0 * 7.0 * 14.0);
        let mut model = Model::new();
        for depth in [10.0, 8.0, 12.0] {
            doc.set_param("wing_depth", &format!("{depth} mm"), None, None).unwrap();
            model.evaluate(&doc);
            assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
            assert_eq!(model.state().bodies.len(), 1);
            let volume = solidcraft_kernel::measure(&model.state().bodies[0].body).unwrap().volume;
            let expected = base_volume + wing_area * depth;
            assert!((volume - expected).abs() < expected * 0.003, "{depth}: {volume} != {expected}");
        }
    }

    #[test]
    fn concave_join_corners_stay_on_both_exact_carriers() {
        for x in [-1.875f64, 1.875] {
            for sign in [-1.0, 1.0] {
                let corner = Vec2::new(x, sign * (49.0 - x * x).sqrt());
                let line = Seg2::Line { a: Vec2::new(x, -25.0), b: Vec2::new(x, 25.0) };
                let arc = Seg2::Arc { center: Vec2::ZERO, radius: 7.0, start: 0.0, sweep: -1.0 };
                for (prev, cur, g0, g1) in [(&line, &arc, 0.0, 0.2), (&arc, &line, 0.2, 0.0)] {
                    let point = concave_line_corner(prev, cur, g0, g1, corner).unwrap();
                    assert!((point.x - x).abs() < 1e-12);
                    assert!((point.len() - 6.8).abs() < 1e-12);
                    assert_eq!(point.y.signum(), sign);
                }
                assert!(concave_line_corner(&line, &arc, 0.1, 0.2, corner).is_none());
            }
        }
    }

    #[test]
    fn ambiguous_or_missing_intersections_and_non_concave_arcs_are_refused() {
        let line = Seg2::Line { a: Vec2::new(7.0, -1.0), b: Vec2::new(7.0, 1.0) };
        let arc = |sweep| Seg2::Arc { center: Vec2::ZERO, radius: 7.0, start: 0.0, sweep };
        for growth in [0.0, -0.1, 0.2, 4.0, f64::NAN] {
            assert!(concave_line_corner(&line, &arc(-1.0), 0.0, growth, Vec2::new(7.0, 0.0)).is_none());
        }
        assert!(concave_line_corner(&line, &arc(1.0), 0.0, 0.2, Vec2::new(7.0, 0.0)).is_none());
    }
}
