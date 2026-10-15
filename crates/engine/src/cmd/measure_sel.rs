//! Measuring picked geometry (`inspect.measure` with `items`): one item gives its length, area,
//! volume or position; two give the minimum distance between them (with the two closest
//! points) and, where both have a direction (planar faces, straight edges), the angle.

use serde_json::{Value, json};
use solidcraft_geom::{Mesh, Vec3};

use crate::params::bad;
use crate::{Result, Sel, Session};

/// Geometry of a picked item, as points, polylines or triangles.
enum Geo {
    Points(Vec<Vec3>),
    /// Separate connected polylines; retain segments for exact point-to-edge distances.
    Lines(Vec<Vec<Vec3>>),
    /// The unbounded centre axis of a cylindrical face.
    Axis {
        origin: Vec3,
        direction: Vec3,
    },
    Tris(Vec<[Vec3; 3]>),
}

/// Caps that keep a measurement interactive on big meshes.
const MAX_POINTS: usize = 4000;
const MAX_TRIS: usize = 20000;

fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let n = (b - a).cross(c - a);
    if let Some(nn) = n.normalized() {
        let q = p - nn * (p - a).dot(nn);
        if [(a, b), (b, c), (c, a)].iter().all(|(u, v)| (*v - *u).cross(q - *u).dot(n) >= -1e-12) {
            return q;
        }
    }
    [closest_on_segment(p, a, b), closest_on_segment(p, b, c), closest_on_segment(p, c, a)]
        .into_iter()
        .min_by(|x, y| x.dist(p).total_cmp(&y.dist(p)))
        .unwrap_or(a)
}

fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let d = b - a;
    let l2 = d.dot(d);
    let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    a + d * t
}

fn polyline_len(pts: &[Vec3]) -> f64 {
    pts.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// A polyline with extra points so no gap is longer than 1/64 of it.
fn densify(pts: &[Vec3]) -> Vec<Vec3> {
    let step = (polyline_len(pts) / 64.0).max(1e-9);
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let n = ((w[0].dist(w[1]) / step).ceil() as usize).clamp(1, 64);
        for k in 0..n {
            out.push(w[0].lerp(w[1], k as f64 / n as f64));
        }
    }
    out.extend(pts.last().copied());
    out
}

/// The mesh edge a selection means: by index if it still passes through the point, otherwise
/// the nearest.
fn edge_index(m: &Mesh, index: usize, point: Vec3) -> Option<usize> {
    let near = |e: &Vec<Vec3>| e.windows(2).map(|w| point.dist_to_segment(w[0], w[1])).fold(f64::INFINITY, f64::min);
    let tol = m.bounds().diagonal().max(1.0) * 1e-6;
    if m.edges.get(index).is_some_and(|e| near(e) < tol) {
        return Some(index);
    }
    m.edges.iter().enumerate().min_by(|(_, a), (_, b)| near(a).total_cmp(&near(b))).map(|(i, _)| i)
}

fn face_tris(m: &Mesh, f: usize) -> Vec<[Vec3; 3]> {
    m.triangles.iter().zip(&m.tri_face).filter(|(_, tf)| **tf as usize == f).filter_map(|(t, _)| m.tri(t)).collect()
}

/// Geometry, direction (planar face normal, straight edge direction) and a description.
fn describe(s: &Session, sel: &Sel) -> Option<(Geo, Option<Vec3>, Value)> {
    let st = s.world_state();
    Some(match sel {
        Sel::Vertex { point, .. } => (Geo::Points(vec![*point]), None, json!({"type": "vertex", "position": point})),
        Sel::SketchPoint { id } => {
            let (sketch, id) = sketch_id(s, id)?;
            return describe_sketch(s, sketch, id, true);
        }
        Sel::ExternalSketchPoint { sketch, id } => return describe_sketch(s, *sketch, id, true),
        Sel::SketchCurve { id } => {
            let (sketch, id) = sketch_id(s, id)?;
            return describe_sketch(s, sketch, id, false);
        }
        Sel::ExternalSketchCurve { sketch, id } => return describe_sketch(s, *sketch, id, false),
        Sel::Edge { body, index, point } => {
            let m = st.body(body)?.mesh();
            let index = edge_index(&m, *index, *point)?;
            let pts = m.edges.get(index)?;
            let rim = m.circular_rim(index);
            if rim.len() > 1 {
                let pieces: Vec<&Vec<Vec3>> = rim.iter().filter_map(|&i| m.edges.get(i)).collect();
                let len: f64 = pieces.iter().map(|p| polyline_len(p)).sum();
                let lines = pieces.into_iter().cloned().collect();
                return Some((Geo::Lines(lines), None, json!({"type": "edge", "length_mm": len})));
            }
            let (a, b) = (*pts.first()?, *pts.last()?);
            let straight = (b - a).normalized().filter(|d| pts.iter().all(|p| (*p - a).cross(*d).len() < 1e-6 * (1.0 + a.dist(b))));
            let len = polyline_len(pts);
            (Geo::Lines(vec![pts.clone()]), straight, json!({"type": "edge", "length_mm": len}))
        }
        Sel::Face { body, index, .. } => {
            let m = st.body(body)?.mesh();
            let tris = m.surface_patch(*index).into_iter().flat_map(|f| face_tris(&m, f)).collect::<Vec<_>>();
            let area: f64 = tris.iter().map(|[a, b, c]| (*b - *a).cross(*c - *a).len() * 0.5).sum();
            let normals: Vec<Vec3> = tris.iter().filter_map(|[a, b, c]| (*b - *a).cross(*c - *a).normalized()).collect();
            let planar = normals.first().copied().filter(|n0| normals.iter().all(|n| n.dot(*n0) > 1.0 - 1e-6));
            (Geo::Tris(tris), planar, json!({"type": "face", "area_mm2": area, "planar": planar.is_some()}))
        }
        Sel::Body { name } => {
            let m = st.body(name)?.mesh();
            let tris: Vec<[Vec3; 3]> = m.triangles.iter().filter_map(|t| m.tri(t)).collect();
            let v = m.measure().volume;
            (Geo::Tris(tris), None, json!({"type": "body", "name": name, "volume_mm3": v}))
        }
        _ => return None,
    })
}

/// Qualified ids also work after Finish Sketch, without guessing between repeated local ids.
fn sketch_id<'a>(s: &Session, id: &'a str) -> Option<(u64, &'a str)> {
    match id.split_once(':') {
        Some((sketch, id)) => Some((sketch.parse().ok()?, id)),
        None => Some((s.active_sketch?, id)),
    }
}

fn describe_sketch(s: &Session, sketch: u64, id: &str, point: bool) -> Option<(Geo, Option<Vec3>, Value)> {
    let st = s.world_state();
    let ss = st.sketch(sketch)?;
    if point {
        let position = ss.plane.to_world(ss.sketch.point(ss.sketch.resolve_point(id)?)?);
        return Some((Geo::Points(vec![position]), None, json!({"type": "vertex", "sketch": sketch, "id": id, "position": position})));
    }
    let pts: Vec<Vec3> = if let Some(ci) = ss.sketch.curve_index(id) {
        ss.sketch.polyline(ci).into_iter().map(|p| ss.plane.to_world(p)).collect()
    } else {
        ss.sketch.wires.get(ss.sketch.wire_index(id)?)?.pts.clone()
    };
    let (a, b) = (*pts.first()?, *pts.last()?);
    let straight = (b - a).normalized().filter(|d| pts.iter().all(|p| (*p - a).cross(*d).len() < 1e-6 * (1.0 + a.dist(b))));
    let len = polyline_len(&pts);
    Some((Geo::Lines(vec![pts]), straight, json!({"type": "edge", "sketch": sketch, "id": id, "length_mm": len})))
}

fn sample<T: Copy>(v: &[T], max: usize) -> Vec<T> {
    let step = v.len().div_ceil(max).max(1);
    v.iter().step_by(step).copied().collect()
}

/// Closest points of an infinite axis and a finite segment. Parallel segments use their
/// midpoint for a stable, readable dimension; all other feet are clamped to the edge.
fn axis_segment(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3) -> (Vec3, Vec3) {
    let edge = b - a;
    let projected = edge - direction * edge.dot(direction);
    let den = projected.len2();
    let t = if den > edge.len2() * 1e-20 { ((origin - a).dot(projected) / den).clamp(0.0, 1.0) } else { 0.5 };
    let q = a + edge * t;
    (origin + direction * (q - origin).dot(direction), q)
}

fn axis_closest(origin: Vec3, direction: Vec3, other: &Geo) -> Option<(Vec3, Vec3)> {
    let best = |points: Vec<(Vec3, Vec3)>| points.into_iter().min_by(|a, b| a.0.dist(a.1).total_cmp(&b.0.dist(b.1)));
    match other {
        Geo::Points(points) => best(sample(points, MAX_POINTS).into_iter().map(|q| (origin + direction * (q - origin).dot(direction), q)).collect()),
        Geo::Lines(lines) => lines
            .iter()
            .flat_map(|line| line.windows(2).map(|w| axis_segment(origin, direction, w[0], w[1])))
            .min_by(|a, b| a.0.dist(a.1).total_cmp(&b.0.dist(b.1))),
        Geo::Axis { origin: q, direction: v } => {
            let n = direction.cross(*v);
            let den = n.len2();
            if den > 1e-20 {
                let w = *q - origin;
                Some((origin + direction * (w.cross(*v).dot(n) / den), *q + *v * (w.cross(direction).dot(n) / den)))
            } else {
                let midpoint = (origin + *q) * 0.5;
                let p = origin + direction * (midpoint - origin).dot(direction);
                Some((p, *q + *v * (p - *q).dot(*v)))
            }
        }
        Geo::Tris(tris) => {
            let mut candidates = Vec::new();
            for [a, b, c] in sample(tris, MAX_TRIS) {
                if let Some(n) = (b - a).cross(c - a).normalized() {
                    let d = n.dot(direction);
                    if d.abs() > 1e-12 {
                        let p = origin + direction * (n.dot(a - origin) / d);
                        if p.dist(closest_on_triangle(p, a, b, c)) < 1e-9 * (1.0 + a.dist(b).max(a.dist(c))) {
                            return Some((p, p));
                        }
                    }
                }
                candidates.extend([
                    axis_segment(origin, direction, a, b),
                    axis_segment(origin, direction, b, c),
                    axis_segment(origin, direction, c, a),
                ]);
            }
            best(candidates)
        }
    }
}

/// Closest pair of points between two pieces of geometry.
fn closest(a: &Geo, b: &Geo) -> Option<(Vec3, Vec3)> {
    if let Geo::Axis { origin, direction } = a {
        return axis_closest(*origin, *direction, b);
    }
    if matches!(b, Geo::Axis { .. }) {
        return closest(b, a).map(|(p, q)| (q, p));
    }
    match (a, b) {
        (Geo::Points(points), Geo::Lines(lines)) => {
            return sample(points, MAX_POINTS)
                .into_iter()
                .flat_map(|p| lines.iter().flat_map(move |line| line.windows(2).map(move |w| (p, closest_on_segment(p, w[0], w[1])))))
                .min_by(|a, b| a.0.dist(a.1).total_cmp(&b.0.dist(b.1)));
        }
        (Geo::Lines(_), Geo::Points(_)) => return closest(b, a).map(|(p, q)| (q, p)),
        // Other combinations keep the existing bounded sampling of curves.
        (Geo::Lines(lines), _) => {
            let points = Geo::Points(lines.iter().flat_map(|line| densify(line)).collect());
            return closest(&points, b);
        }
        (_, Geo::Lines(lines)) => {
            let points = Geo::Points(lines.iter().flat_map(|line| densify(line)).collect());
            return closest(a, &points);
        }
        _ => {}
    }
    let pts_to_tris = |pts: &[Vec3], tris: &[[Vec3; 3]]| -> Option<(Vec3, Vec3)> {
        let pts = sample(pts, MAX_POINTS);
        let tris = sample(tris, MAX_TRIS);
        let mut best: Option<(f64, Vec3, Vec3)> = None;
        for p in &pts {
            for [x, y, z] in &tris {
                let q = closest_on_triangle(*p, *x, *y, *z);
                let d = p.dist(q);
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, *p, q));
                }
            }
        }
        best.map(|(_, p, q)| (p, q))
    };
    let verts = |tris: &[[Vec3; 3]]| -> Vec<Vec3> { tris.iter().flat_map(|t| t.iter().copied()).collect() };
    match (a, b) {
        (Geo::Points(p), Geo::Points(q)) => {
            let (p, q) = (sample(p, MAX_POINTS), sample(q, MAX_POINTS));
            let mut best: Option<(f64, Vec3, Vec3)> = None;
            for x in &p {
                for y in &q {
                    let d = x.dist(*y);
                    if best.is_none_or(|(bd, _, _)| d < bd) {
                        best = Some((d, *x, *y));
                    }
                }
            }
            best.map(|(_, x, y)| (x, y))
        }
        (Geo::Points(p), Geo::Tris(t)) => pts_to_tris(p, t),
        (Geo::Tris(t), Geo::Points(p)) => pts_to_tris(p, t).map(|(x, y)| (y, x)),
        (Geo::Tris(t1), Geo::Tris(t2)) => {
            let ab = pts_to_tris(&verts(t1), t2);
            let ba = pts_to_tris(&verts(t2), t1).map(|(x, y)| (y, x));
            [ab, ba].into_iter().flatten().min_by(|x, y| x.0.dist(x.1).total_cmp(&y.0.dist(y.1)))
        }
        _ => None,
    }
}

/// `inspect.measure` with `items` (one or two selections).
pub(crate) fn measure_items(s: &Session, items: &Value) -> Result<Value> {
    measure_items_with_centers(s, items, &Value::Null)
}

pub(crate) fn measure_items_with_centers(s: &Session, items: &Value, centers: &Value) -> Result<Value> {
    let cmd = "inspect.measure";
    let list = items.as_array().ok_or_else(|| bad(cmd, "`items` must be a list of selections"))?;
    if list.is_empty() || list.len() > 1000 {
        return Err(bad(cmd, "measure one or two items"));
    }
    let references = if centers.is_null() {
        Vec::new()
    } else {
        let refs = centers.as_array().ok_or_else(|| bad(cmd, "`centers` must be a list of booleans, one per item"))?;
        if refs.len() != list.len() || refs.iter().any(|v| !v.is_boolean()) {
            return Err(bad(cmd, "`centers` must have one boolean per item"));
        }
        refs.iter().map(|v| v.as_bool().unwrap_or(false)).collect::<Vec<_>>()
    };
    let st = s.world_state();
    let mut sels = Vec::new();
    let mut use_centers = Vec::new();
    for (item_index, value) in list.iter().enumerate() {
        let mut sel = serde_json::from_value::<Sel>(value.clone()).map_err(|e| bad(cmd, e.to_string()))?;
        // Two kernel halves of one visible item are one measurement target.
        match &mut sel {
            Sel::Edge { body, index, point } => {
                if let Some(b) = st.body(body) {
                    let m = b.mesh();
                    let resolved = edge_index(&m, *index, *point).unwrap_or(*index);
                    *index = m.circular_rim(resolved).first().copied().unwrap_or(resolved);
                }
            }
            Sel::Face { body, index, .. } => {
                if let Some(b) = st.body(body) {
                    *index = b.mesh().surface_patch(*index).first().copied().unwrap_or(*index);
                }
            }
            _ => {}
        }
        let use_center = references.get(item_index).copied().unwrap_or(false);
        let duplicate = sels.iter().zip(&use_centers).any(|(x, reference)| {
            *reference == use_center
                && match (x, &sel) {
                    (Sel::Edge { body: a, index: i, .. }, Sel::Edge { body: b, index: j, .. })
                    | (Sel::Face { body: a, index: i, .. }, Sel::Face { body: b, index: j, .. }) => a == b && i == j,
                    _ => false,
                }
        });
        if !duplicate {
            sels.push(sel);
            use_centers.push(use_center);
        }
        if sels.len() > 2 {
            return Err(bad(cmd, "measure one or two items"));
        }
    }
    let mut described = Vec::new();
    for (x, use_center) in sels.iter().zip(&use_centers) {
        let (mut geo, mut direction, mut description) =
            describe(s, x).ok_or_else(|| bad(cmd, "an item can't be measured (pick sketch points or curves, vertices, edges, faces or bodies)"))?;
        if let Some(center) = super::measure_center::of(s, x) {
            description["center"] = center.json();
            if *use_center {
                geo = match center.axis {
                    Some(axis) => Geo::Axis { origin: center.position, direction: axis },
                    None => Geo::Points(vec![center.position]),
                };
                direction = center.axis;
                description["reference"] = json!(if center.axis.is_some() { "axis" } else { "center" });
            }
        } else if *use_center {
            return Err(bad(cmd, "this item has no circular centre or cylindrical centre axis"));
        }
        described.push((geo, direction, description));
    }
    let mut out = json!({"items": described.iter().map(|d| d.2.clone()).collect::<Vec<_>>()});
    if let [(ga, da, _), (gb, db, _)] = described.as_slice() {
        if let Some((p, q)) = closest(ga, gb) {
            out["distance_mm"] = json!(p.dist(q));
            out["from"] = json!(p);
            out["to"] = json!(q);
            out["delta"] = json!(q - p);
        }
        if let (Some(a), Some(b)) = (da, db) {
            let both_faces = matches!(sels.as_slice(), [Sel::Face { .. }, Sel::Face { .. }]) && !use_centers.iter().any(|v| *v);
            let c = a.dot(*b).clamp(-1.0, 1.0);
            let c = if both_faces { c } else { c.abs() };
            out["angle_deg"] = json!(c.acos().to_degrees());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod measurement_tests {
    use super::*;
    use solidcraft_sketch::CurveKind;

    fn draw(s: &mut Session, command: &str, params: Value) -> Value {
        s.execute(command, &params).unwrap()
    }

    fn cylinder_face(s: &Session, body: &str) -> Sel {
        let st = s.world_state();
        let b = st.body(body).unwrap();
        let index = (0..b.body.face_count())
            .find(|i| matches!(solidcraft_kernel::face_surface(&b.body, *i), Some(solidcraft_kernel::FaceSurface::Cylinder { .. })))
            .unwrap();
        Sel::Face { body: body.into(), index, point: solidcraft_doc::naming::face_spot(b, index).unwrap().point }
    }

    fn low_rim(s: &Session, body: &str, z: f64) -> Sel {
        let st = s.world_state();
        let mesh = st.body(body).unwrap().mesh();
        let (index, edge) = mesh.edges.iter().enumerate().find(|(_, e)| e.len() >= 3 && e.iter().all(|p| (p.z - z).abs() < 1e-8)).unwrap();
        Sel::Edge { body: body.into(), index, point: edge[0] }
    }

    #[test]
    fn cylinder_measurement_is_pitch_between_axes_at_different_heights_and_directions() {
        for axis in [Vec3::Z, Vec3::new(1.0, 2.0, 3.0)] {
            let mut s = Session::default();
            let axis = axis.normalized().unwrap();
            let offset = if axis == Vec3::Z { Vec3::new(20.0, 0.0, 0.0) } else { Vec3::new(20.0, -10.0, 0.0) };
            draw(&mut s, "solid.cylinder", json!({"radius":5,"height":20,"axis":axis,"body_name":"A"}));
            draw(
                &mut s,
                "solid.cylinder",
                json!({"radius":3,"height":30,"base":offset+axis*7.0,"axis":-axis,"body_name":"B","operation":"new_body"}),
            );
            let items = json!([cylinder_face(&s, "A"), cylinder_face(&s, "B")]);
            let raw = draw(&mut s, "inspect.measure", json!({"items":items}));
            assert!(raw["distance_mm"].as_f64().unwrap() < offset.len() - 7.9, "{raw}");
            let r = draw(&mut s, "inspect.measure", json!({"items":items,"centers":[true,true]}));
            assert!((r["distance_mm"].as_f64().unwrap() - offset.len()).abs() < 1e-7, "{r}");
            assert_eq!(r["items"][0]["reference"], "axis");
            assert_eq!(r["items"][1]["reference"], "axis");
        }
    }

    #[test]
    fn measurement_of_two_bores_can_switch_from_surface_gap_to_hole_pitch() {
        let mut s = Session::default();
        draw(&mut s, "solid.box", json!({"length":35,"width":20,"height":8,"corner":[-10,-10,0],"body_name":"Plate"}));
        for x in [0.0, 10.9] {
            draw(&mut s, "solid.cylinder", json!({"base":[x,0,-1],"radius":4,"height":10,"operation":"cut","targets":["Plate"]}));
        }
        let st = s.world_state();
        let b = st.body("Plate").unwrap();
        let items: Vec<_> = [0.0, 10.9]
            .into_iter()
            .map(|x| {
                let index = (0..b.body.face_count())
                    .find(|i| {
                        matches!(solidcraft_kernel::face_surface(&b.body,*i),
                Some(solidcraft_kernel::FaceSurface::Cylinder {origin,..}) if (origin.x-x).abs()<1e-6)
                    })
                    .unwrap();
                Sel::Face { body: "Plate".into(), index, point: solidcraft_doc::naming::face_spot(b, index).unwrap().point }
            })
            .collect();
        let raw = s.measure_items(&items);
        assert!((raw["distance_mm"].as_f64().unwrap() - 2.9).abs() < 0.02, "{raw}");
        let pitch = s.measure_items_with_centers(&items, &[true, true]);
        assert!((pitch["distance_mm"].as_f64().unwrap() - 10.9).abs() < 1e-8, "{pitch}");
    }

    #[test]
    fn cylinder_axis_and_rim_centre_measure_to_an_arbitrary_finite_body_edge() {
        let mut s = Session::default();
        draw(&mut s, "solid.cylinder", json!({"base":[12.3,9.7,20],"radius":4,"height":10,"body_name":"A"}));
        draw(&mut s, "solid.box", json!({"length":2,"width":30,"height":5,"corner":[0,0,5],"body_name":"Box","operation":"new_body"}));
        let st = s.world_state();
        let mesh = st.body("Box").unwrap().mesh();
        let (index, edge) =
            mesh.edges.iter().enumerate().find(|(_, edge)| edge.iter().all(|p| p.x.abs() < 1e-8 && (p.z - 5.0).abs() < 1e-8)).unwrap();
        let target = Sel::Edge { body: "Box".into(), index, point: edge[0] };
        let axis = cylinder_face(&s, "A");
        let rim = low_rim(&s, "A", 20.0);
        for (source, expected) in [(axis, 12.3), (rim, (12.3_f64.powi(2) + 15.0_f64.powi(2)).sqrt())] {
            for reverse in [false, true] {
                let items = if reverse { json!([target, source]) } else { json!([source, target]) };
                let centers = if reverse { json!([false, true]) } else { json!([true, false]) };
                let r = draw(&mut s, "inspect.measure", json!({"items":items,"centers":centers}));
                assert!((r["distance_mm"].as_f64().unwrap() - expected).abs() < 1e-8, "{r}");
                let foot: Vec3 = serde_json::from_value(r[if reverse { "from" } else { "to" }].clone()).unwrap();
                assert!(foot.dist(Vec3::new(0.0, 9.7, 5.0)) < 1e-8, "{r}");
            }
        }
    }

    #[test]
    fn circular_edges_give_true_3d_centre_distance_and_mixed_references_keep_the_other_rim() {
        let mut s = Session::default();
        draw(&mut s, "solid.cylinder", json!({"radius":5,"height":10,"body_name":"A"}));
        draw(&mut s, "solid.cylinder", json!({"base":[20,0,7],"radius":3,"height":10,"body_name":"B","operation":"new_body"}));
        let items = json!([low_rim(&s, "A", 0.0), low_rim(&s, "B", 7.0)]);
        let r = draw(&mut s, "inspect.measure", json!({"items":items,"centers":[true,true]}));
        assert!((r["distance_mm"].as_f64().unwrap() - 449.0_f64.sqrt()).abs() < 1e-8, "{r}");
        let r = draw(&mut s, "inspect.measure", json!({"items":items,"centers":[true,false]}));
        assert!((r["distance_mm"].as_f64().unwrap() - 338.0_f64.sqrt()).abs() < 0.02, "{r}");
        assert_eq!(r["items"][0]["reference"], "center");
        assert!(r["items"][1].get("reference").is_none());
    }

    #[test]
    fn measurement_axes_handle_skew_intersection_and_edge_endpoints() {
        let axis = Geo::Axis { origin: Vec3::ZERO, direction: Vec3::Z };
        for (other, expected) in [
            (Geo::Axis { origin: Vec3::new(0., 3., 4.), direction: Vec3::X }, 3.),
            (Geo::Axis { origin: Vec3::new(0., 0., 4.), direction: Vec3::X }, 0.),
            (Geo::Lines(vec![vec![Vec3::new(3., 7., 8.), Vec3::new(3., 5., 8.)]]), 34.0_f64.sqrt()),
        ] {
            let (p, q) = closest(&axis, &other).unwrap();
            assert!((p.dist(q) - expected).abs() < 1e-10, "{p:?}, {q:?}");
        }
    }

    #[test]
    fn measurement_centre_requests_reject_nonround_items_and_invalid_flags() {
        let mut s = Session::default();
        draw(&mut s, "solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Box"}));
        let item = json!({"type":"face","body":"Box","index":0,"point":[0,0,0]});
        for centers in [json!([true]), json!([false, false]), json!(["axis"]), json!(true)] {
            assert!(s.execute("inspect.measure", &json!({"items":[item],"centers":centers})).is_err());
        }
        let r = draw(&mut s, "inspect.measure", json!({"items":[item]}));
        assert!(r["items"][0].get("center").is_none(), "a square is not a circular face: {r}");
    }

    fn centre_scene(command: &str, params: Value, plane: &str) -> (Session, u64, String, String) {
        let mut s = Session::default();
        draw(&mut s, "sketch.create", json!({"plane": plane}));
        let sketch = s.active_sketch.unwrap();
        draw(&mut s, command, params);
        let line = draw(&mut s, "sketch.line", json!({"points": [[0, -1], [0, 31]]}))["curves"][0].as_str().unwrap().to_string();
        let st = s.world_state();
        let sk = &st.sketch(sketch).unwrap().sketch;
        let centre = sk
            .curves
            .iter()
            .find_map(|curve| match curve.kind {
                CurveKind::Circle { c, .. } | CurveKind::Arc { c, .. } => Some(sk.points[c].id.clone()),
                _ => None,
            })
            .unwrap();
        (s, sketch, centre, line)
    }

    #[test]
    fn circle_and_both_arc_centres_measure_to_the_exact_foot_on_a_sketch_edge() {
        for plane in ["XY", "XZ", "YZ"] {
            for (cmd, params) in [
                ("sketch.circle.center", json!({"center": [12.3, 9.7], "radius": 4})),
                ("sketch.arc.center_point", json!({"center": [12.3, 9.7], "start": [16.3, 9.7], "end": [12.3, 13.7]})),
                ("sketch.arc.three_point", json!({"start": [16.3, 9.7], "end": [8.3, 9.7], "through": [12.3, 13.7]})),
            ] {
                let (mut s, sketch, centre, line) = centre_scene(cmd, params, plane);
                for finished in [false, true] {
                    if finished {
                        draw(&mut s, "sketch.finish", json!({}));
                    }
                    let point = json!({"type": "sketch_point", "id": format!("{sketch}:{centre}")});
                    let edge = json!({"type": "external_sketch_curve", "sketch": sketch, "id": line});
                    for reversed in [false, true] {
                        let items = if reversed { json!([edge, point]) } else { json!([point, edge]) };
                        let r = draw(&mut s, "inspect.measure", json!({"items": items}));
                        assert!((r["distance_mm"].as_f64().unwrap() - 12.3).abs() < 1e-8, "{cmd}, {plane}, finished={finished}: {r}");
                        let st = s.world_state();
                        let expected = st.sketch(sketch).unwrap().plane.to_world(solidcraft_geom::Vec2::new(0.0, 9.7));
                        let foot: Vec3 = serde_json::from_value(r[if reversed { "from" } else { "to" }].clone()).unwrap();
                        assert!(foot.dist(expected) < 1e-8, "{r}");
                    }
                }
            }
        }
    }

    #[test]
    fn arc_centre_measures_to_a_body_edge_and_stops_at_its_endpoint() {
        let (mut s, sketch, centre, _) =
            centre_scene("sketch.arc.center_point", json!({"center": [12.3, 9.7], "start": [16.3, 9.7], "end": [12.3, 13.7]}), "XY");
        draw(&mut s, "sketch.finish", json!({}));
        draw(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 20}));
        let st = s.world_state();
        let body = st.bodies.first().unwrap();
        let mesh = body.mesh();
        let (index, pts) = mesh.edges.iter().enumerate().find(|(_, pts)| pts.iter().all(|p| p.x.abs() < 1e-8 && p.z.abs() < 1e-8)).unwrap();
        let point = json!({"type": "external_sketch_point", "sketch": sketch, "id": centre});
        let edge = json!({"type": "edge", "body": body.name, "index": index, "point": pts[0]});
        let r = draw(&mut s, "inspect.measure", json!({"items": [point, edge]}));
        assert!((r["distance_mm"].as_f64().unwrap() - 12.3).abs() < 1e-8, "{r}");

        let r = closest(&Geo::Points(vec![Vec3::new(3.0, 7.0, 0.0)]), &Geo::Lines(vec![vec![Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0)]])).unwrap();
        assert_eq!(r.1, Vec3::new(0.0, 5.0, 0.0));
    }

    #[test]
    fn explicit_sketch_ids_do_not_resolve_against_another_active_sketch() {
        let (mut s, sketch, centre, line) = centre_scene("sketch.circle.center", json!({"center": [12.3, 9.7], "radius": 4}), "XY");
        draw(&mut s, "sketch.finish", json!({}));
        draw(&mut s, "sketch.create", json!({"plane": "XY"}));
        draw(&mut s, "sketch.circle.center", json!({"center": [80, 80], "radius": 4}));
        let items = json!([
            {"type": "external_sketch_point", "sketch": sketch, "id": centre},
            {"type": "sketch_curve", "id": format!("{sketch}:{line}")}
        ]);
        let r = draw(&mut s, "inspect.measure", json!({"items": items}));
        assert!((r["distance_mm"].as_f64().unwrap() - 12.3).abs() < 1e-8, "{r}");
        assert!(s.execute("inspect.measure", &json!({"items": [{"type": "external_sketch_point", "sketch": sketch, "id": "missing"}]})).is_err());
    }
}
