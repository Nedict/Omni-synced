//! Exact planar extrusion endpoints, without positional recovery of lost targets.
use crate::{DocError, Document, Extent, ModelState, PlaneRef, Result};
use solidcraft_geom::Plane;

impl Extent {
    pub(crate) fn has_to(&self) -> bool {
        self.to.is_some() || self.to_plane.is_some()
    }

    pub(crate) fn target_height(
        &self,
        doc: &Document,
        vals: &std::collections::BTreeMap<String, crate::expr::Value>,
        author: u64,
        state: &ModelState,
        source: &Plane,
    ) -> Result<Option<f64>> {
        let Some(target) = self.to_plane.as_deref() else { return Ok(None) };
        if self.to.is_some() || self.through_all || self.distance2.is_some() {
            return Err(DocError::Invalid("ambiguous planar extrusion endpoint".into()));
        }
        let target = match target {
            PlaneRef::Face { anchor: Some(anchor), .. } => doc.resolve_face_anchor(anchor, author, state)?,
            PlaneRef::Construction { name } => {
                let target = state
                    .construct
                    .iter()
                    .find(|c| &c.name == name)
                    .ok_or_else(|| DocError::Unknown(format!("extrusion endpoint plane `{name}` must be active and precede the extrusion")))?;
                let crate::construct::ConstructGeom::Plane(plane) = target.geom else {
                    return Err(DocError::Invalid("extrusion endpoint is not a plane".into()));
                };
                plane
            }
            PlaneRef::Origin { .. } | PlaneRef::Custom { .. } => doc.resolve_plane(vals, target, 0)?,
            _ => return Err(DocError::Invalid("extrusion endpoint requires a construction plane or exact face attachment".into())),
        };
        if target.normal().dot(source.normal()).abs() < 1. - 1e-9 {
            return Err(DocError::Invalid("not supported yet: a nonparallel exact extrusion endpoint".into()));
        }
        Ok(Some((target.origin - source.origin).dot(source.normal())))
    }
}

pub(crate) fn validate_json(value: &serde_json::Value) -> Result<()> {
    use serde_json::Value;
    let Some(features) = value.get("features").and_then(Value::as_array) else { return Ok(()) };
    for feature in features {
        if feature.get("type").and_then(Value::as_str) != Some("extrude") {
            continue;
        }
        let Some(extent) = feature.get("extent") else { continue };
        let Some(target) = extent.get("to_plane").filter(|v| !v.is_null()) else { continue };
        if extent.get("to").is_some_and(|v| !v.is_null())
            || extent.get("distance2").is_some_and(|v| !v.is_null())
            || extent.get("through_all").is_some_and(|v| v != &Value::Bool(false))
        {
            return Err(DocError::Invalid("ambiguous planar extrusion endpoint".into()));
        }
        match target.get("type").and_then(Value::as_str) {
            Some("face") => crate::face_anchor::validate_face_anchor_json(
                target.get("anchor").ok_or_else(|| DocError::Invalid("extrusion endpoint needs an exact face anchor".into()))?,
            )?,
            Some("construction" | "origin" | "custom") => {}
            _ => return Err(DocError::Invalid("unsupported planar extrusion endpoint reference".into())),
        }
    }
    Ok(())
}
