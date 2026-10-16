//! Finite sketch lines, verified with rectangle edits and isolated flag controls.
use super::history::{empty, finish, payload, zeros};
use super::*;
use solidcraft_geom::Vec3;

#[derive(Debug, PartialEq)]
pub struct SketchLine {
    pub id: u64,
    pub sketch: u64,
    pub start_point: u64,
    pub end_point: u64,
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub fixed: bool,
    pub construction: bool,
    pub centerline: bool,
    pub reference: bool,
    pub metadata: EntityMetadata,
    pub profile_id: Option<(u64, u64)>,
}

impl Design {
    pub fn sketch_lines(&self) -> Result<Vec<SketchLine>> {
        let schema = entity::Schema::read(self)?;
        let line_classes: Vec<_> = self.classes.iter().filter(|c| matches!(c.id.as_str(), entity::LINE | entity::SPECIAL_LINE)).collect();
        for class in &line_classes {
            if !matches!((class.id.as_str(), class.version), (entity::LINE, 2) | (entity::SPECIAL_LINE, 0))
                || (class.id == entity::SPECIAL_LINE && class.parent != entity::LINE)
            {
                return Err(invalid("unsupported sketch line version or parent"));
            }
        }
        let records = Records::new(self);
        let classes: BTreeMap<_, _> = self.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        self.recover_records(
            "sketch line",
            line_classes.iter().flat_map(|class| class.objects.iter().map(move |id| (class, id))).map(|(class, id)| {
                let mut r = payload(records.primary(*id)?, *id)?;
                empty(&mut r)?;
                zeros(&mut r, 1)?;
                let attributes = attributes::read(&mut r)?;
                let mut metadata = EntityMetadata { specialized: class.id == entity::SPECIAL_LINE, ..Default::default() };
                let endpoints = if metadata.specialized {
                    let end = r.reference()?;
                    match r.bytes.get(r.at) {
                        Some(0) => {
                            r.take(1)?;
                        }
                        Some(1) => metadata.definition.push(r.reference()?),
                        _ => return Err(invalid("unsupported specialized line definition")),
                    }
                    let start = r.reference()?;
                    finish(&r)?;
                    r = payload(records.secondary(*id)?, *id)?;
                    empty(&mut r)?;
                    Some((start, end))
                } else {
                    None
                };
                let mut number = || -> Result<f64> {
                    let n = f64::from_bits(r.u64()?);
                    if !n.is_finite() {
                        return Err(invalid("non-finite line geometry"));
                    }
                    Ok(n)
                };
                let start = Vec3::new(number()?, number()?, number()?);
                let delta = Vec3::new(number()?, number()?, number()?);
                let direction = Vec3::new(number()?, number()?, number()?);
                for _ in 0..3 {
                    number()?;
                } // auxiliary frame; endpoints establish the line
                let length = delta.len();
                // The support-line axis may oppose the finite segment's endpoint
                // order. Both orientations occur with identical API endpoints.
                let parallel_error = (delta - direction * length).len().min((delta + direction * length).len());
                if !length.is_finite() || length < 1e-12 || (direction.len() - 1.).abs() > 1e-9 || parallel_error > length * 1e-9 {
                    return Err(invalid("inconsistent finite line direction"));
                }
                let end = start + delta;
                if !end.is_finite() {
                    return Err(invalid("non-finite line endpoint"));
                }
                let end_point = r.reference()?;
                let start_point = r.reference()?;
                if start_point == end_point {
                    return Err(invalid("coincident line endpoints"));
                }
                for p in [start_point, end_point] {
                    entity::require_point(&classes, p)?;
                }
                if endpoints.is_some_and(|pair| pair != (start_point, end_point)) {
                    return Err(invalid("specialized line endpoint references disagree"));
                }
                r.take(2)?;
                let fixed = points::fixed(&mut r)?;
                let reference = points::boolean(&mut r)?;
                r.take(if schema.curve == 9 { 2 } else { 1 })?;
                let (construction, centerline) = match r.take(1)? {
                    [0] => (false, false),
                    [1] => (true, false),
                    [2] => (false, true),
                    _ => return Err(invalid("unsupported line construction flag")),
                };
                let sketch = schema.curve_tail(&mut r, &mut metadata)?;
                schema.require_owner(&classes, sketch)?;
                metadata.validate(&classes)?;
                finish(&r)?;
                let profile_id = match (attributes.get("crv_primary_id"), attributes.get("crv_secondary_id")) {
                    (Some(a), Some(b)) => Some((*a, *b)),
                    (None, None) => None,
                    _ => return Err(invalid("incomplete line profile identity")),
                };
                Ok(SketchLine {
                    id: *id,
                    sketch,
                    start_point,
                    end_point,
                    start: [start.x, start.y, start.z],
                    end: [end.x, end.y, end.z],
                    fixed,
                    construction,
                    centerline,
                    reference,
                    profile_id,
                    metadata,
                })
            }),
        )
    }
}
