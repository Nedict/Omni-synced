//! Original circle/strip intersection with analytic tangent rounds. No native
//! archive or commercial geometry is needed by this regression.
use super::*;
use solidcraft_doc::{FeatureKind, Model};
use solidcraft_geom::Vec3;
use std::f64::consts::{PI, TAU};

#[derive(Default)]
struct Outline {
    points: BTreeMap<u64, SketchPoint>,
    lines: BTreeMap<u64, SketchLine>,
    arcs: BTreeMap<u64, SketchArc>,
    targets: Vec<u64>,
}

impl Outline {
    fn point(&mut self, value: Vec2) -> u64 {
        let value = [value.x / 10., value.y / 10., 0.];
        if let Some((id, _)) = self.points.iter().find(|(_, p)| p.position.iter().zip(value).all(|(a, b)| (*a - b).abs() < 1e-12)) {
            return *id;
        }
        let id = 1000 + self.points.len() as u64;
        self.points
            .insert(id, SketchPoint { id, sketch: 500, position: value, fixed: false, origin: false, linked: true, metadata: Default::default() });
        id
    }

    fn line(&mut self, a: Vec2, b: Vec2) {
        let id = 2000 + self.targets.len() as u64;
        let start_point = self.point(a);
        let end_point = self.point(b);
        self.lines.insert(
            id,
            SketchLine {
                id,
                sketch: 500,
                start_point,
                end_point,
                start: [a.x / 10., a.y / 10., 0.],
                end: [b.x / 10., b.y / 10., 0.],
                fixed: false,
                construction: false,
                centerline: false,
                reference: true,
                metadata: Default::default(),
                profile_id: None,
            },
        );
        self.targets.push(id);
    }

    fn arc(&mut self, center: Vec2, radius: f64, start_angle: f64, end_angle: f64) {
        let id = 2000 + self.targets.len() as u64;
        let center_point = self.point(center);
        let at = |angle: f64| center + Vec2::new(angle.cos(), angle.sin()) * radius;
        let start_point = self.point(at(start_angle));
        let end_point = self.point(at(end_angle));
        self.arcs.insert(
            id,
            SketchArc {
                circle: SketchCircle {
                    id,
                    sketch: 500,
                    center_point,
                    center: [center.x / 10., center.y / 10., 0.],
                    normal: [0., 0., 1.],
                    x_axis: [1., 0., 0.],
                    radius: radius / 10.,
                    construction: false,
                    fixed: false,
                    reference: true,
                    metadata: Default::default(),
                    profile_id: None,
                },
                start_point,
                end_point,
                start_angle,
                end_angle,
            },
        );
        self.targets.push(id);
    }
}

fn outline() -> Outline {
    // R=10, strip half-width=3, corner radius=1. Each corner centre lies
    // on the radius-9 inset circle, two millimetres from its centreline.
    let y = 77_f64.sqrt();
    let angle = y.atan2(2.);
    let mut out = Outline::default();
    out.line(Vec2::new(3., -y), Vec2::new(3., y));
    out.arc(Vec2::new(2., y), 1., 0., angle);
    out.arc(Vec2::ZERO, 10., angle, PI - angle);
    out.arc(Vec2::new(-2., y), 1., PI - angle, PI);
    out.line(Vec2::new(-3., y), Vec2::new(-3., -y));
    out.arc(Vec2::new(-2., -y), 1., PI, PI + angle);
    out.arc(Vec2::ZERO, 10., PI + angle, TAU - angle);
    out.arc(Vec2::new(2., -y), 1., -angle, 0.);
    out
}

#[test]
fn unrounded_joined_cap_projection_requires_unchanged_evaluated_provenance() {
    let mut doc =
        crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/split-cylinder-grip-corner-fillets.solidcraft")).unwrap();
    doc.features.truncate(4);
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let anchor = super::super::loft_faces::Face::cap(0, 0, true, false).anchor(&doc, &state, 2, &[(4, None)]).unwrap();
    let plane = Plane::new(Vec3::new(0., 0., 20.), Vec3::X, Vec3::Y).unwrap();
    let support = super::super::planes::attach_sketch(&doc, &state, anchor, plane).unwrap();
    let mut data = Outline::default();
    let y = 91_f64.sqrt();
    let angle = y.atan2(3.);
    data.line(Vec2::new(3., -y), Vec2::new(3., y));
    data.arc(Vec2::ZERO, 10., angle, PI - angle);
    data.line(Vec2::new(-3., y), Vec2::new(-3., -y));
    data.arc(Vec2::ZERO, 10., PI + angle, TAU - angle);
    let circles = BTreeMap::new();
    let native = super::super::projections::Geometry { points: &data.points, lines: &data.lines, circles: &circles, arcs: &data.arcs };
    let features = BTreeMap::from([(102, 2), (104, 4)]);
    let operations = BTreeMap::from([(9002, 102), (9004, 104)]);
    let profiles = BTreeMap::new();
    let context = Context { native: &native, profiles: &profiles, doc: &doc, model: &state, features: &features, operations: &operations };
    let projection = Projection {
        body: 99,
        creator: 102,
        roles: BTreeMap::from([(9004, true)]),
        boundary_operations: BTreeSet::from([9002, 9004]),
        targets: data.targets,
    };
    let mut sketch = Sketch::new();
    assert_eq!(projection.attach(&context, &model, 500, plane, &support, &mut sketch).unwrap().curves.len(), 4);
    let mut stale = model.clone();
    stale.results.iter_mut().find(|r| r.id == 4).unwrap().stale = true;
    assert!(projection.attach(&context, &stale, 500, plane, &support, &mut Sketch::new()).is_err());
    let kind: FeatureKind = serde_json::from_value(serde_json::json!({"type":"sketch","plane":support,"sketch":sketch})).unwrap();
    let id = doc.add_feature(kind, Some("Live joined cap")).unwrap();
    for (diameter, height, grip) in [(20_f64, 8., 12.), (22., 9., 14.)] {
        for (name, value) in [("diameter", diameter), ("height", height), ("grip_height", grip)] {
            doc.change_param(name, &format!("{value} mm"), None, None).unwrap();
        }
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        model.evaluate(&reopened);
        assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
        let state = model.state();
        let projected = state.sketch(id).unwrap();
        assert!(projected.sketch.links.iter().all(|l| !l.lost));
        assert_eq!(projected.sketch.curves.len(), 4);
        assert!((projected.plane.origin.z - height - grip).abs() < 1e-7);
        let r = diameter / 2.;
        for curve in &projected.sketch.curves {
            if let CurveKind::Arc { c, a, .. } = curve.kind {
                assert!((projected.sketch.point(c).unwrap().dist(projected.sketch.point(a).unwrap()) - r).abs() < 1e-7);
            }
        }
        let strip = 2. * (3. * (r * r - 9.).sqrt() + r * r * (3. / r).asin());
        let expected = PI * r * r * height + strip * grip;
        let measured = solidcraft_kernel::measure(&state.body("Knob").unwrap().body).unwrap().volume;
        assert!((measured - expected).abs() < expected * 1e-3, "{measured} != {expected}");
    }
}

#[test]
fn rounded_cap_projection_is_live_and_rejects_incomplete_or_unrelated_geometry() {
    let mut doc =
        crate::read_design(include_bytes!("../../../../apps/solidcraft-cli/tests/history/split-cylinder-grip-corner-fillets.solidcraft")).unwrap();
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let anchor = super::super::loft_faces::Face::cap(0, 0, true, false).anchor(&doc, &state, 2, &[(4, None)]).unwrap();
    let plane = Plane::new(Vec3::new(0., 0., 20.), Vec3::X, Vec3::Y).unwrap();
    let support = super::super::planes::attach_sketch(&doc, &state, anchor, plane).unwrap();
    let data = outline();
    let empty = BTreeMap::new();
    let native = super::super::projections::Geometry { points: &data.points, lines: &data.lines, circles: &empty, arcs: &data.arcs };
    let features = BTreeMap::from([(102, 2), (104, 4), (105, 5)]);
    let operations = BTreeMap::from([(9002, 102), (9004, 104), (9005, 105)]);
    let profiles = BTreeMap::new();
    let context = Context { native: &native, profiles: &profiles, doc: &doc, model: &state, features: &features, operations: &operations };
    let mut projection = Projection {
        body: 99,
        creator: 102,
        roles: BTreeMap::from([(9004, true)]),
        boundary_operations: BTreeSet::from([9002, 9004, 9005]),
        targets: data.targets.clone(),
    };
    let mut sketch = Sketch::new();
    let linked = projection.attach(&context, &model, 500, plane, &support, &mut sketch).unwrap();
    assert_eq!(linked.curves.len(), 8);
    assert_eq!(sketch.links.len(), 1);
    assert!(sketch.links[0].auto_face && sketch.links[0].join_arcs);
    projection.targets.pop();
    assert!(
        projection.attach(&context, &model, 500, plane, &support, &mut Sketch::new()).is_err(),
        "a missing span must not become a partial live boundary"
    );
    projection.targets = data.targets;
    projection.boundary_operations.remove(&9005);
    assert!(
        projection.attach(&context, &model, 500, plane, &support, &mut Sketch::new()).is_err(),
        "a rounded boundary requires its modifier provenance"
    );
    projection.boundary_operations.insert(9005);
    projection.roles = BTreeMap::from([(9002, true)]);
    assert!(
        projection.attach(&context, &model, 500, plane, &support, &mut Sketch::new()).is_err(),
        "another cap on the same body cannot replace the selected cap"
    );
    projection.roles = BTreeMap::from([(9004, true)]);
    let mut shifted = outline();
    for point in shifted.points.values_mut() {
        point.position[0] += 0.01;
    }
    for line in shifted.lines.values_mut() {
        line.start[0] += 0.01;
        line.end[0] += 0.01;
    }
    for arc in shifted.arcs.values_mut() {
        arc.circle.center[0] += 0.01;
    }
    let shifted_native = super::super::projections::Geometry { points: &shifted.points, lines: &shifted.lines, circles: &empty, arcs: &shifted.arcs };
    let shifted_context = Context { native: &shifted_native, ..context };
    assert!(
        projection.attach(&shifted_context, &model, 500, plane, &support, &mut Sketch::new()).is_err(),
        "matching topology cannot excuse different geometric boundaries"
    );

    let kind: FeatureKind = serde_json::from_value(serde_json::json!({ "type": "sketch", "plane": support, "sketch": sketch })).unwrap();
    let id = doc.add_feature(kind, Some("Live rounded cap")).unwrap();
    for (name, value, volume, height) in [("round_radius", "0.5 mm", 3930.0794945661146, 20.), ("height", "10 mm", 4554.1335339305715, 22.)] {
        let mut edited = doc.clone();
        edited.change_param(name, value, None, None).unwrap();
        let reopened = crate::read_design(&crate::write_design_container(&edited).unwrap()).unwrap();
        assert_eq!(edited, reopened);
        model.evaluate(&reopened);
        assert!(model.results.iter().all(|r| r.error.is_none()), "{name}: {:?}", model.results);
        let state = model.state();
        let projected = state.sketch(id).unwrap();
        assert!(projected.sketch.links.iter().all(|l| !l.lost));
        assert_eq!(projected.sketch.curves.len(), 8);
        assert!((projected.plane.origin.z - height).abs() < 1e-7);
        let measured = solidcraft_kernel::measure(&state.body("Knob").unwrap().body).unwrap().volume;
        assert!((measured - volume).abs() < 0.15, "{name}: {measured} != {volume}");
    }
}
