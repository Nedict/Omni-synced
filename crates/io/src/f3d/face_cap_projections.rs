//! Complete circular caps produced by an extrusion of a linked face profile.
//! Face and edge records must agree, including opaque legacy lineage tokens.
use super::body_projections::Context;
use super::history::{finish, payload, references, require_class, zeros};
use super::*;

const IDENTITY: &str = "7ACC2A03-0261-4879-A14A-A93D661A5BDC";
type Terms = Vec<Vec<(String, Vec<u32>)>>;
#[derive(PartialEq)]
struct Identity {
    terms: Terms,
    orientation: [u32; 8],
}

pub(super) struct Projection {
    pub body: u64,
    feature: u64,
    operation: u64,
    boundary: Vec<(String, Vec<u32>)>,
}

fn integers(r: &mut Reader<'_>, values: &[i32]) -> Result<()> {
    for value in values {
        if r.u32()? != *value as u32 {
            return Err(invalid("unsupported face-profile cap incidence"));
        }
    }
    Ok(())
}

fn identity(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, edge: bool) -> Result<Identity> {
    require_class(classes, id, IDENTITY, 3)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    integers(&mut r, &[1, 3, 3])?;
    let mut terms = Vec::new();
    for _ in 0..3 {
        let count = r.u32()?;
        if !(1..=2).contains(&count) {
            return Err(invalid("unsupported circular cap contributors"));
        }
        let mut term = Vec::new();
        for _ in 0..count {
            let label = r.string()?;
            zeros(&mut r, 4)?;
            let count = r.u32()?;
            if !(1..=2).contains(&count) {
                return Err(invalid("unsupported circular cap lineage length"));
            }
            let mut lineage = Vec::new();
            for _ in 0..count {
                lineage.push(r.u32()?);
            }
            zeros(&mut r, 4)?;
            if lineage.first().is_none_or(|n| *n == 0 || *n > i32::MAX as u32)
                || lineage.get(1).is_some_and(|n| *n <= i32::MAX as u32)
                || term.iter().any(|(other, _)| other == &label)
            {
                return Err(invalid("invalid circular cap lineage"));
            }
            term.push((label, lineage));
        }
        terms.push(term);
    }
    zeros(&mut r, 4)?;
    if r.string()? != if edge { "edge_recipe_data" } else { "bounded_face_recipe_data" } {
        return Err(invalid("incorrect face-profile cap recipe kind"));
    }
    zeros(&mut r, 4)?;
    if edge {
        integers(&mut r, &[-1, 2, 1, -1, 2, -1, 1, -1, 2, 1, -1, 2, -1, 1, 0, -1, 0, 0, -1, 1, 0])?;
    } else {
        integers(&mut r, &[-1, 1, 1, -1, 2, 2, -1, 0, -1, 1, -1, 2, 2, -1, 0, -1, 1, 1, -1, 0, 0, -1, 1, 0])?;
    }
    let mut orientation = [0; 8];
    for value in &mut orientation {
        *value = r.u32()?;
    }
    if !matches!(orientation, [2, 2, 2, 2, 1, 0, 1, u32::MAX] | [2, 1, 1, 1, 1, 0, 1, u32::MAX]) {
        return Err(invalid("unsupported face-profile cap orientation"));
    }
    finish(&r)?;
    if edge {
        terms.rotate_right(1);
    }
    Ok(Identity { terms, orientation })
}

pub(super) fn decode(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    id: u64,
    owner: u64,
    mappings: &[(u64, u64)],
) -> Result<Vec<super::projections::Projection>> {
    let version = super::loft_faces::selector_version(classes, id)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, if version == 2 { 7 } else { 8 })?;
    let selected = identity(records, classes, r.reference()?, false)?;
    let terms = &selected.terms;
    let [cap, seam, boundary] = terms.as_slice() else { return Err(invalid("missing cap terms")) };
    let [(label, lineage)] = cap.as_slice() else { return Err(invalid("ambiguous cap producer")) };
    let [operation] = lineage.as_slice() else { return Err(invalid("ambiguous cap operation")) };
    if label != "2" || seam.as_slice() != [("-1".to_string(), lineage.clone())] || boundary.iter().any(|(label, _)| label != "3" && label != "5") {
        return Err(invalid("unsupported face-profile cap roles"));
    }
    integers(&mut r, &[1])?;
    r.wide_string()?;
    r.wide_string()?;
    integers(&mut r, &[2, 0])?;
    let cached = references(&mut r)?;
    let [cached] = cached.as_slice() else { return Err(invalid("circular cap requires one cached boundary")) };
    require_class(classes, *cached, "B0B3FA59-C374-444F-B424-24AA5DE41645", 0)?;
    let handle = super::loft_faces::selector_tail(&mut r, records, owner)?;
    let (body, feature) = super::body_projections::body_owner(records, classes, handle)?;
    let [(tracking, target)] = mappings else { return Err(invalid("circular cap requires one curve mapping")) };
    let tracked = super::projections::tracking_identity(records, classes, *tracking, "4B31DA1B-FCD6-4527-BC3E-BE1FBAF867FE", 1)?;
    if identity(records, classes, tracked, true)? != selected {
        return Err(invalid("face-profile cap tracker disagrees with selector lineage"));
    }
    Ok(vec![super::projections::Projection {
        target: *target,
        source: super::projections::Source::FaceCap(Projection { body, feature, operation: u64::from(*operation), boundary: boundary.clone() }),
    }])
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
        use solidcraft_doc::{FeatureKind, ProfileSel};
        let producer = context.operations.get(&self.operation).ok_or_else(|| invalid("missing face-profile cap operation"))?;
        let id = *context.features.get(producer).ok_or_else(|| invalid("projection precedes its face extrusion"))?;
        let creator = *context.features.get(&self.feature).ok_or_else(|| invalid("missing projected body creator"))?;
        let feature = context.doc.feature(id).filter(|f| !f.suppressed).ok_or_else(|| invalid("missing face extrusion"))?;
        let FeatureKind::Extrude { sketch: source, profiles: ProfileSel::All, .. } = &feature.kind else {
            return Err(invalid("projected cap requires a face-profile extrusion"));
        };
        let Some(FeatureKind::Sketch { plane: solidcraft_doc::PlaneRef::Face { anchor: Some(parent), .. }, sketch: linked }) =
            context.doc.feature(source.0).map(|f| &f.kind)
        else {
            return Err(invalid("missing linked face profile"));
        };
        if !linked.links.iter().any(|link| link.auto_face) {
            return Err(invalid("face-profile projection requires a live complete cap"));
        }
        for (label, lineage) in &self.boundary {
            let operation = u64::from(*lineage.first().ok_or_else(|| invalid("empty boundary lineage"))?);
            let native = context.operations.get(&operation).ok_or_else(|| invalid("missing boundary operation"))?;
            let producer = context.features.get(native).ok_or_else(|| invalid("missing boundary producer"))?;
            let expected = if label == "5" { id } else { parent.target.origin_feature };
            if *producer != expected {
                return Err(invalid("cap boundary does not belong to the face extrusion or its source"));
            }
        }
        let face = super::loft_faces::Face::cap(self.feature, self.operation, true, false);
        let anchor = face.anchor(context.doc, context.model, creator, &[(id, None)])?;
        let body = context.model.body(&anchor.target.body).ok_or_else(|| invalid("missing projected cap body"))?;
        let indices = context.doc.configuration_face_indices(body, &anchor.target)?;
        let selected = solidcraft_doc::naming::edge_names(body)
            .into_iter()
            .enumerate()
            .filter(|(i, _)| body.mesh().edge_faces.get(*i).is_some_and(|adjacent| adjacent.iter().any(|index| indices.contains(&(*index as usize)))))
            .map(|(_, name)| name)
            .collect::<Vec<_>>();
        if indices.len() != 1 {
            return Err(invalid("ambiguous projected circular cap"));
        }
        context.attach_circle(target, geometry, plane, sketch, (&body.name, &selected, true))
    }
}

#[cfg(test)]
mod tests {
    use super::super::import_tests::Fixture;
    use super::super::relationship_tests::{string, u32};
    use super::*;

    fn recipe(edge: bool, token: Option<u32>) -> Vec<u8> {
        let mut b = vec![0; 6];
        for n in [1, 3, 3] {
            u32(&mut b, n);
        }
        for label in if edge { ["-1", "5", "2"] } else { ["2", "-1", "5"] } {
            u32(&mut b, 1);
            string(&mut b, label);
            u32(&mut b, 0);
            let extended = label == "5" && token.is_some();
            u32(&mut b, if extended { 2 } else { 1 });
            u32(&mut b, 8001);
            if extended {
                u32(&mut b, token.unwrap());
            }
            u32(&mut b, 0);
        }
        u32(&mut b, 0);
        string(&mut b, if edge { "edge_recipe_data" } else { "bounded_face_recipe_data" });
        u32(&mut b, 0);
        let tree = if edge {
            vec![-1, 2, 1, -1, 2, -1, 1, -1, 2, 1, -1, 2, -1, 1, 0, -1, 0, 0, -1, 1, 0]
        } else {
            vec![-1, 1, 1, -1, 2, 2, -1, 0, -1, 1, -1, 2, 2, -1, 0, -1, 1, 1, -1, 0, 0, -1, 1, 0]
        };
        for n in tree.into_iter().chain([2, 2, 2, 2, 1, 0, 1, -1]) {
            u32(&mut b, n as u32);
        }
        b
    }

    fn fixture(token: Option<u32>) -> Fixture {
        let mut f = super::super::body_projections::tests::body_circle(true);
        f.0.get_mut(&183).unwrap().2[7] = 0;
        f.0.get_mut(&185).unwrap().2 = recipe(false, token);
        f.0.get_mut(&188).unwrap().2 = recipe(true, token);
        f
    }

    fn check(f: Fixture) -> Result<Vec<super::super::projections::Projection>> {
        let d = f.design();
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        decode(&Records::new(&d), &classes, 183, 18, &[(184, 123)])
    }

    #[test]
    fn circular_face_profile_requires_identical_selector_and_tracker_lineage() {
        for token in [None, Some(u32::MAX - 9000)] {
            let projections = check(fixture(token)).unwrap();
            assert_eq!(projections.len(), 1);
            let super::super::projections::Source::FaceCap(cap) = &projections[0].source else { panic!("cap") };
            assert_eq!((cap.feature, cap.operation), (9, 8001));
            for record in [183, 184, 185, 188] {
                for keep in 0..fixture(token).0[&record].2.len() {
                    let mut f = fixture(token);
                    f.0.get_mut(&record).unwrap().2.truncate(keep);
                    assert!(check(f).is_err(), "record {record}, prefix {keep}");
                }
            }
        }
        let mut mismatched = fixture(Some(u32::MAX - 9000));
        mismatched.0.get_mut(&188).unwrap().2 = recipe(true, Some(u32::MAX - 9001));
        assert!(check(mismatched).is_err());
        assert!(check(fixture(Some(9000))).is_err());
        let mut broken_tree = fixture(None);
        let data = &mut broken_tree.0.get_mut(&185).unwrap().2;
        let n = data.len();
        data[n - 8..n - 4].copy_from_slice(&2_u32.to_le_bytes());
        assert!(check(broken_tree).is_err());
    }
}
