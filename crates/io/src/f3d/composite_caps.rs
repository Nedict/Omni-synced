//! Resolve a native composite cap through exact Boolean-history aliases.
//! Equal planes alone do not identify a face: every named producer must resolve
//! to the same current face, and its logical cap must lie on that plane.
use super::*;
use solidcraft_doc::config::{ConfigFaceTarget, FaceCertificate};
use solidcraft_doc::{Direction, Document, FaceAnchor, FeatureKind, ModelState, Operation};
use solidcraft_geom::Vec3;

pub(crate) fn anchor(doc: &Document, state: &ModelState, creator: u64, roles: &BTreeMap<u64, bool>) -> Result<FaceAnchor> {
    if !(2..=64).contains(&roles.len()) {
        return Err(invalid("composite cap requires two to 64 unique producers"));
    }
    cap_anchor(doc, state, creator, roles)
}

/// One or more explicitly named caps on a body assembled from extrusions.
pub(crate) fn cap_anchor(doc: &Document, state: &ModelState, creator: u64, roles: &BTreeMap<u64, bool>) -> Result<FaceAnchor> {
    let indices = cap_indices(doc, state, creator, roles)?;
    let mut found = indices.into_iter();
    let index = found.next().ok_or_else(|| invalid("missing composite cap face"))?;
    if found.next().is_some() {
        return Err(invalid("composite cap contributors do not name one face"));
    }
    let feature = doc.feature(creator).ok_or_else(|| invalid("missing composite cap body creator"))?;
    let [name] = feature.body_names.as_slice() else { return Err(invalid("composite cap requires one created body")) };
    let body = state.body(name).ok_or_else(|| invalid("composite cap body no longer exists"))?;
    let plane = body.body.planar_face(index).ok_or_else(|| invalid("composite cap is not planar"))?;
    // Capture the resulting face's persistent history name. Later edits follow
    // that face's current boundary; this never freezes the imported plane.
    let anchor = doc.capture_face_anchor(body, index, &plane, &[], 0, &[])?;
    doc.resolve_face_anchor(&anchor, 0, state)?;
    Ok(anchor)
}

/// Exact pieces of a logical cap. A later join can split one cap into several
/// faces; edge incidence can disambiguate those pieces even when a face anchor
/// cannot. Every returned piece must carry every declared producer's lineage.
pub(crate) fn cap_indices(doc: &Document, state: &ModelState, creator: u64, roles: &BTreeMap<u64, bool>) -> Result<BTreeSet<usize>> {
    if roles.is_empty() || roles.len() > 64 {
        return Err(invalid("invalid joined cap contributor count"));
    }
    let feature = doc.feature(creator).filter(|f| !f.suppressed).ok_or_else(|| invalid("missing composite cap body creator"))?;
    let [name] = feature.body_names.as_slice() else { return Err(invalid("composite cap requires one created body")) };
    let body = state.body(name).ok_or_else(|| invalid("composite cap body no longer exists"))?;
    let mut common: Option<BTreeSet<usize>> = None;
    for (producer, end) in roles {
        let feature = doc.feature(*producer).filter(|f| !f.suppressed).ok_or_else(|| invalid("missing composite cap producer"))?;
        let FeatureKind::Extrude { sketch, extent, operation, targets, .. } = &feature.kind else {
            return Err(invalid("composite cap requires extrusion contributors"));
        };
        if extent.from.is_some()
            || extent.start_offset.is_some()
            || (*producer == creator && *operation != Operation::NewBody)
            || (*producer != creator && (!matches!(operation, Operation::Join | Operation::Cut) || targets.as_slice() != [name.clone()]))
        {
            return Err(invalid("composite cap contributor has another target or unsupported extent"));
        }
        let high = doc.extrusion_cap_high(*producer, *end)?;
        let distance =
            doc.eval(&extent.distance, solidcraft_doc::expr::Kind::Length)? * if extent.direction == Direction::Negative { -1.0 } else { 1.0 };
        let sketch = state.sketch(sketch.0).ok_or_else(|| invalid("missing composite cap sketch"))?;
        let expected = sketch.plane.offset(if *end { distance } else { 0.0 });
        // A cut exposes the back of its tool cap: its persistent producer and
        // nominal plane are unchanged, but the retained solid's normal reverses.
        let normal = expected.normal() * if high { 1.0 } else { -1.0 } * if *operation == Operation::Cut { -1.0 } else { 1.0 };
        let tolerance = 1e-9 * (1.0 + body.body.size() + expected.origin.len());
        let mut indices = BTreeSet::new();
        // Coplanar regularization can extend a start cap above or below its
        // sketch before the Boolean returns it to the nominal plane. Each
        // candidate is an exact cap alias of this producer, never a face index
        // or a nearest-plane search among unrelated faces.
        for role in ["start", "top", "bottom"] {
            let target = ConfigFaceTarget {
                body: body.definition_name().into(),
                origin_feature: *producer,
                name: format!("F{producer}:{role}"),
                point: Vec3::ZERO,
                certificate: Some(FaceCertificate::Lineage),
            };
            if let Ok(candidates) = doc.configuration_face_indices(body, &target) {
                for index in candidates {
                    if body.body.planar_face(index).is_some_and(|plane| {
                        plane.normal().dot(normal) > 1.0 - 1e-9 && (plane.origin - expected.origin).dot(normal).abs() <= tolerance
                    }) {
                        indices.insert(index);
                    }
                }
            }
        }
        if indices.is_empty() {
            return Err(invalid("composite cap contributor has no exact nominal face"));
        }
        common = Some(match common {
            Some(previous) => previous.intersection(&indices).copied().collect(),
            None => indices,
        });
    }
    common.filter(|indices| !indices.is_empty()).ok_or_else(|| invalid("composite cap contributors have no common face"))
}

#[cfg(test)]
#[path = "composite_caps_tests.rs"]
mod tests;
