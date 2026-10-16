//! A rectangular extrusion cap with one circular cut: every projected boundary
//! retains its creating operation and a live, named edge of the modified body.
use super::body_projections::{Context, Edge};
use super::history::{finish, payload, references, require_class, zeros};
use super::*;

const IDENTITY: &str = "7ACC2A03-0261-4879-A14A-A93D661A5BDC";
const TRACKER: &str = "4B31DA1B-FCD6-4527-BC3E-BE1FBAF867FE";
const CIRCLE: &str = "B0B3FA59-C374-444F-B424-24AA5DE41645";

pub(super) struct Projection {
    pub body: u64,
    pub end: bool,
    base: u64,
    cut: u64,
    circle_tag: u64,
    edge: Option<Edge>,
}

struct Face {
    base_operation: u64,
    end: bool,
    seam: i32,
    cut_operation: u64,
    cut_side: u64,
    circle_end: bool,
}

fn integers(r: &mut Reader<'_>, expected: &[i32]) -> Result<()> {
    for n in expected {
        if r.u32()? != *n as u32 {
            return Err(invalid("unsupported cut-face incidence tree"));
        }
    }
    Ok(())
}

fn orientation(r: &mut Reader<'_>) -> Result<bool> {
    let mut values = [0; 7];
    for value in &mut values {
        *value = r.u32()?;
    }
    match values {
        [2, 2, 2, 2, 1, 0, 1] => Ok(true),
        [2, 1, 1, 1, 2, 1, 2] => Ok(false),
        _ => Err(invalid("unsupported cut-circle orientation")),
    }
}

fn terms(r: &mut Reader<'_>, count: usize, kind: &str) -> Result<Vec<(i32, u64)>> {
    zeros(r, 6)?;
    integers(r, &[1, 3])?;
    if r.count(24)? != count {
        return Err(invalid("unsupported cut-face term count"));
    }
    let mut terms = Vec::new();
    let mut seen = BTreeSet::new();
    for _ in 0..count {
        integers(r, &[1])?;
        let label = r.string()?.parse::<i32>().map_err(|_| invalid("invalid cut-face label"))?;
        zeros(r, 4)?;
        integers(r, &[1])?;
        let operation = r.u64()?;
        if operation == 0 || !seen.insert((label, operation)) {
            return Err(invalid("duplicate or empty cut-face term"));
        }
        terms.push((label, operation));
    }
    zeros(r, 4)?;
    if r.string()? != kind {
        return Err(invalid("incorrect cut-face recipe type"));
    }
    zeros(r, 4)?;
    Ok(terms)
}

fn face(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64) -> Result<Face> {
    require_class(classes, id, IDENTITY, 3)?;
    let mut r = payload(records.primary(id)?, id)?;
    let terms = terms(&mut r, 7, "bounded_face_recipe_data")?;
    let [(cap, base), (seam, cut), (side, side_owner), outer @ ..] = terms.as_slice() else {
        return Err(invalid("missing cut-face terms"));
    };
    let sides: BTreeSet<_> = outer.iter().map(|(label, _)| *label).collect();
    if !matches!(cap, 1 | 2)
        || *seam >= 0
        || *side < 3
        || base == cut
        || side_owner != cut
        || sides != BTreeSet::from([3, 4, 5, 6])
        || outer.iter().any(|(_, operation)| operation != base)
    {
        return Err(invalid("unsupported cut-face ownership or boundary roles"));
    }
    // The closed circle uses cap term 0, side term 2 and virtual seam term 1.
    integers(&mut r, &[-1, 5, 1, -1, 2, 0, -1, 2, -1, 1, -1, 2, 2, -1, 0, -1, 1, 1, -1, 0, 0, -1, 1, 0])?;
    let circle_end = orientation(&mut r)?;
    integers(&mut r, &[-1])?;
    super::loft_faces::quadrilateral_edges(&mut r, 2)?;
    finish(&r)?;
    Ok(Face { base_operation: *base, end: *cap == 2, seam: *seam, cut_operation: *cut, cut_side: *side as u64, circle_end })
}

fn circle_identity(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, face: &Face) -> Result<()> {
    require_class(classes, id, IDENTITY, 3)?;
    let mut r = payload(records.primary(id)?, id)?;
    let terms = terms(&mut r, 3, "edge_recipe_data")?;
    if terms != [(face.seam, face.cut_operation), (if face.end { 2 } else { 1 }, face.base_operation), (face.cut_side as i32, face.cut_operation)] {
        return Err(invalid("projected cut circle disagrees with its face"));
    }
    integers(&mut r, &[-1, 2, 1, -1, 2, -1, 1, -1, 2, 2, -1, 1, -1, 1, 0, -1, 0, 0, -1, 1, 0])?;
    if orientation(&mut r)? != face.circle_end {
        return Err(invalid("cut-circle orientation disagrees with its face"));
    }
    integers(&mut r, &[-1])?;
    finish(&r)
}

fn cached_circle(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, face: &Face) -> Result<()> {
    use solidcraft_geom::Vec3;
    require_class(classes, id, CIRCLE, 0)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    let mut values = [0.; 12];
    for value in &mut values {
        *value = f64::from_bits(r.u64()?);
        if !value.is_finite() {
            return Err(invalid("non-finite cut-circle cache"));
        }
    }
    let normal = Vec3::new(values[3], values[4], values[5]);
    let axis = Vec3::new(values[6], values[7], values[8]);
    if (normal.len() - 1.).abs() > 1e-9
        || (axis.len() - 1.).abs() > 1e-9
        || normal.dot(axis).abs() > 1e-9
        || values[9] <= 0.
        || ((values[11] - values[10]).abs() - std::f64::consts::TAU).abs() > 1e-9
    {
        return Err(invalid("unsupported cut-circle cache geometry"));
    }
    zeros(&mut r, 1)?;
    require_class(classes, r.reference()?, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1)?;
    if r.take(2)? != [0, 1] {
        return Err(invalid("unsupported cut-circle cache tracking"));
    }
    let tracker = r.reference()?;
    finish(&r)?;
    let identity = super::projections::tracking_identity(records, classes, tracker, TRACKER, 1)?;
    circle_identity(records, classes, identity, face)
}

pub(super) fn decode(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    selector: u64,
    owner: u64,
    mappings: &[(u64, u64)],
) -> Result<Vec<super::projections::Projection>> {
    if mappings.len() != 5 {
        return Err(invalid("cut-face projection requires four outer edges and one hole"));
    }
    let version = super::loft_faces::selector_version(classes, selector)?;
    let mut r = payload(records.primary(selector)?, selector)?;
    zeros(&mut r, if version == 2 { 7 } else { 8 })?;
    let face = face(records, classes, r.reference()?)?;
    integers(&mut r, &[1])?;
    r.wide_string()?;
    r.wide_string()?;
    integers(&mut r, &[2, 0])?;
    let cached = references(&mut r)?;
    if cached.len() != 5 || cached.iter().collect::<BTreeSet<_>>().len() != 5 {
        return Err(invalid("incomplete or repeated cut-face boundary"));
    }
    let handle = super::loft_faces::selector_tail(&mut r, records, owner)?;
    let (body, base) = super::body_projections::body_owner(records, classes, handle)?;
    let operations = records.design.operation_owners()?;
    if operations.get(&face.base_operation) != Some(&base) {
        return Err(invalid("cut-face cap disagrees with its creating extrusion"));
    }
    let cut = *operations.get(&face.cut_operation).filter(|id| **id != base).ok_or_else(|| invalid("missing cut-face cutting operation"))?;
    let circle_tag = super::side_roles::circle_tag(records, classes, cut, face.cut_side)?;
    let mut lines = Vec::new();
    let mut circle_count = 0;
    let mut sides = BTreeSet::new();
    for id in cached {
        if classes.get(&id).is_some_and(|c| c.id == CIRCLE) {
            cached_circle(records, classes, id, &face)?;
            circle_count += 1;
        } else {
            let (recipe, line) = super::body_projections::cached_edge(records, classes, id)?;
            if recipe.operation != face.base_operation || recipe.end != face.end || !sides.insert(recipe.side) {
                return Err(invalid("cut-face outer edge disagrees with its cap"));
            }
            lines.push(Edge { body, feature: base, recipe, line: Some(line) });
        }
    }
    if circle_count != 1 || sides != BTreeSet::from([Some(3), Some(4), Some(5), Some(6)]) {
        return Err(invalid("cut-face cache does not contain its complete boundary"));
    }
    let mut projections = Vec::new();
    let mut circle_mapped = false;
    for (tracking, target) in mappings {
        let identity = super::projections::tracking_identity(records, classes, *tracking, TRACKER, 1)?;
        let edge = match super::body_projections::identity(records, classes, identity, true) {
            Ok(recipe) => {
                let index = lines.iter().position(|e| e.recipe == recipe).ok_or_else(|| invalid("mapped cut-face edge is missing or repeated"))?;
                Some(lines.remove(index))
            }
            Err(_) => {
                circle_identity(records, classes, identity, &face)?;
                if circle_mapped {
                    return Err(invalid("repeated cut-face circle mapping"));
                }
                circle_mapped = true;
                None
            }
        };
        projections.push(super::projections::Projection {
            target: *target,
            source: super::projections::Source::CutBody(Projection { body, end: face.end, base, cut, circle_tag, edge }),
        });
    }
    if !lines.is_empty() || !circle_mapped {
        return Err(invalid("incomplete cut-face projection mappings"));
    }
    Ok(projections)
}

impl Projection {
    pub fn attach(
        &self,
        context: &Context<'_>,
        target: u64,
        geometry: u64,
        plane: solidcraft_geom::Plane,
        sketch: &mut solidcraft_doc::sketch::Sketch,
    ) -> Result<super::projections::Entities> {
        use solidcraft_doc::{Direction, FeatureKind, Operation, ProfileSel, expr::Kind};
        let base = context.features.get(&self.base).and_then(|id| context.doc.feature(*id)).ok_or_else(|| invalid("projection precedes its base"))?;
        let cut = context.features.get(&self.cut).and_then(|id| context.doc.feature(*id)).ok_or_else(|| invalid("projection precedes its cut"))?;
        let [body] = base.body_names.as_slice() else { return Err(invalid("cut-face projection requires one base body")) };
        let FeatureKind::Extrude {
            sketch: source_sketch,
            profiles: ProfileSel::WholeBoundaries { profiles },
            extent,
            operation: Operation::Cut,
            targets,
            ..
        } = &cut.kind
        else {
            return Err(invalid("projected inner boundary requires an extrusion cut"));
        };
        let [profile] = profiles.as_slice() else { return Err(invalid("projected cut requires one profile")) };
        let [curve] = profile.outer.as_slice() else { return Err(invalid("projected cut requires one circle")) };
        // Boolean operations retain the body's creating feature. Check the
        // intervening timeline as well, so that identity cannot conceal another
        // operation which changed the selected cap.
        let changed = context
            .doc
            .features
            .iter()
            .skip_while(|f| f.id != base.id)
            .skip(1)
            .any(|f| f.id != cut.id && !matches!(f.kind, FeatureKind::Sketch { .. }));
        if targets.as_slice() != std::slice::from_ref(body)
            || !profile.holes.is_empty()
            || base.suppressed
            || cut.suppressed
            || changed
            || context.model.body(body).is_none_or(|b| b.feature != base.id)
            || extent.direction != Direction::Positive
            || extent.distance2.is_some()
            || extent.start_offset.is_some()
            || extent.to.is_some()
            || extent.to_plane.is_some()
            || extent.from.is_some()
            || extent.through_all
            || extent.taper.as_ref().map(|s| context.doc.eval(s, Kind::Angle)).transpose()?.is_some_and(|a| a.abs() > 1e-12)
        {
            return Err(invalid("unsupported projected cut or changed body history"));
        }
        let native_id = curve.strip_prefix('c').and_then(|id| id.parse::<u64>().ok()).ok_or_else(|| invalid("missing native cut circle identity"))?;
        let native = context.native.circles.get(&native_id).ok_or_else(|| invalid("cut boundary is not a circle"))?;
        let source = context.profiles.get(&self.cut).ok_or_else(|| invalid("missing cut profile"))?;
        let source_plane = context.model.sketch(source_sketch.0).ok_or_else(|| invalid("missing cut sketch"))?.plane;
        if native.sketch != source.geometry
            || native.profile_id != Some((self.circle_tag, 0))
            || source_plane.normal().cross(plane.normal()).len() > 1e-9
        {
            return Err(invalid("cut-circle label disagrees with its selected profile"));
        }
        if let Some(edge) = &self.edge {
            let projection = super::projections::Projection { target, source: super::projections::Source::Body(edge.clone()) };
            return context.attach(&projection, geometry, plane, sketch);
        }
        let cap = format!("F{}:{}", base.id, if self.end { "top" } else { "start" });
        let side = format!("F{}:side:{curve}", cut.id);
        let body_state = context.model.body(body).ok_or_else(|| invalid("missing cut body"))?;
        let selected: Vec<_> = solidcraft_doc::naming::edge_names(body_state)
            .into_iter()
            .filter(|name| {
                let faces: BTreeSet<_> = name.split('|').map(solidcraft_doc::naming::strip_piece).collect();
                faces == BTreeSet::from([cap.as_str(), side.as_str()])
            })
            .collect();
        context.attach_circle(target, geometry, plane, sketch, (body, &selected, self.end))
    }
}

#[cfg(test)]
mod tests {
    use super::super::import_tests::Fixture;
    use super::super::relationship_tests::{string, u32, u64};
    use super::*;

    fn ints(bytes: &mut Vec<u8>, values: &[i32]) {
        for n in values {
            u32(bytes, *n as u32);
        }
    }

    // Original field-authored identities: no exported records are embedded.
    fn identity(terms: &[(i32, u64)], kind: &str) -> Vec<u8> {
        let mut bytes = vec![0; 6];
        ints(&mut bytes, &[1, 3, terms.len() as i32]);
        for (label, operation) in terms {
            u32(&mut bytes, 1);
            string(&mut bytes, &label.to_string());
            ints(&mut bytes, &[0, 1]);
            u64(&mut bytes, *operation);
        }
        u32(&mut bytes, 0);
        string(&mut bytes, kind);
        u32(&mut bytes, 0);
        bytes
    }

    fn face_record(cap: i32, base: u64, cut: u64, side: i32) -> Vec<u8> {
        let mut bytes = identity(&[(cap, base), (-83, cut), (side, cut), (3, base), (4, base), (5, base), (6, base)], "bounded_face_recipe_data");
        ints(&mut bytes, &[-1, 5, 1, -1, 2, 0, -1, 2, -1, 1, -1, 2, 2, -1, 0, -1, 1, 1, -1, 0, 0, -1, 1, 0]);
        ints(&mut bytes, &[2, 2, 2, 2, 1, 0, 1, -1]);
        for (a, before, after) in [(3, 6, 4), (4, 3, 5), (5, 4, 6), (6, 5, 3)] {
            ints(&mut bytes, &[-1, -1, 2, 0, -1, a, -1, 2]);
            for neighbor in [before, after] {
                ints(&mut bytes, &[-1, 3, 0, -1, a, -1, neighbor, -1, 0, 0]);
            }
            ints(&mut bytes, &[-1]);
        }
        bytes
    }

    fn decode_face(bytes: Vec<u8>) -> Result<Face> {
        let mut fixture = Fixture(BTreeMap::new());
        fixture.put(91, IDENTITY, 3, bytes, None);
        let design = fixture.design();
        let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        face(&Records::new(&design), &classes, 91)
    }

    #[test]
    fn cut_face_requires_distinct_owners_and_a_complete_incidence_tree() {
        let bytes = face_record(2, 17, 23, 7);
        let face = decode_face(bytes.clone()).unwrap();
        assert_eq!((face.base_operation, face.cut_operation, face.cut_side, face.seam), (17, 23, 7, -83));
        assert!(face.end && face.circle_end);
        assert!(!decode_face(face_record(1, 17, 23, 7)).unwrap().end);
        for bad in [face_record(3, 17, 23, 7), face_record(2, 17, 17, 7), face_record(2, 0, 23, 7), face_record(2, 17, 23, 2)] {
            assert!(decode_face(bad).is_err());
        }
        for end in 0..bytes.len() {
            assert!(decode_face(bytes[..end].to_vec()).is_err(), "accepted truncated face at {end}");
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_face(trailing).is_err());
    }

    #[test]
    #[ignore = "requires local controlled cut-face projection archives"]
    fn native_cut_face_projections_reopen_with_analytic_volume() {
        let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
        let mut failures = Vec::new();
        for face in ["top", "bottom"] {
            for (variant, radius, height) in [("baseline", 3., 10.), ("diameter8", 4., 10.), ("height15", 4., 15.)] {
                let name = format!("cut-face-projection-{face}-{variant}");
                let bytes = std::fs::read(root.join(format!("{name}.f3d"))).unwrap();
                let mut doc = match super::super::import_f3d(&bytes, &name) {
                    Ok(doc) => doc,
                    Err(error) => {
                        failures.push(format!("{name}: {error}"));
                        continue;
                    }
                };
                assert_eq!(doc.features.len(), 6, "{name}");
                let expected = (600. - std::f64::consts::PI * radius * radius) * (height + 5.);
                super::super::import_tests::volume(&doc, expected);
                doc.change_param("d1", "19 mm", None, None).unwrap();
                doc.change_param("d3", "10 mm", None, None).unwrap();
                let edited = (600. - std::f64::consts::PI * 25.) * 24.;
                super::super::import_tests::volume(&doc, edited);
                let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
                assert_eq!(doc, reopened, "{name}");
                super::super::import_tests::volume(&reopened, edited);
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
