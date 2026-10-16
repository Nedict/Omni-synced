//! Sketch-local point coordinates and fixation verified with independent API edits.
use super::history::{empty, finish, payload, references, require_class, zeros};
use super::*;

#[derive(Debug, PartialEq)]
pub struct SketchPoint {
    pub id: u64,
    pub sketch: u64,
    pub position: [f64; 3],
    pub fixed: bool,
    pub origin: bool,
    pub linked: bool,
    pub metadata: EntityMetadata,
}

pub(super) fn fixed(r: &mut Reader<'_>) -> Result<bool> {
    match r.take(2)? {
        // The second flag is derived constraint state. A dimensioned, coincident
        // circle is constrained but remains editable: never turn that into Fix.
        [0, 0 | 1] => Ok(false),
        [1, 0 | 1] => Ok(true),
        _ => Err(invalid("unsupported fixation flags")),
    }
}

pub(super) fn boolean(r: &mut Reader<'_>) -> Result<bool> {
    match r.take(1)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(invalid("unsupported sketch boolean flag")),
    }
}

impl Design {
    pub fn sketch_points(&self) -> Result<Vec<SketchPoint>> {
        let schema = entity::Schema::read(self)?;
        let point_classes: Vec<_> = self.classes.iter().filter(|c| matches!(c.id.as_str(), entity::POINT | entity::SPECIAL_POINT)).collect();
        for class in &point_classes {
            if !matches!((class.id.as_str(), class.version), (entity::POINT, 11) | (entity::SPECIAL_POINT, 0))
                || (class.id == entity::SPECIAL_POINT && class.parent != entity::POINT)
            {
                return Err(invalid("unsupported sketch point version or parent"));
            }
        }
        let records = Records::new(self);
        let classes: BTreeMap<_, _> = self.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        self.recover_records(
            "sketch point",
            point_classes.iter().flat_map(|class| class.objects.iter().map(move |id| (class, id))).map(|(class, id)| {
                let mut r = payload(records.primary(*id)?, *id)?;
                empty(&mut r)?;
                zeros(&mut r, 1)?;
                attributes::read(&mut r)?;
                let mut metadata = EntityMetadata { specialized: class.id == entity::SPECIAL_POINT, ..Default::default() };
                if metadata.specialized {
                    finish(&r)?;
                    r = payload(records.secondary(*id)?, *id)?;
                    empty(&mut r)?;
                }
                let connections = r.reference()?;
                require_class(&classes, connections, "362B7EC3-0F09-47C8-A3BE-DC066715CDAE", 0)?;
                metadata.point_prefix = match r.take(3)? {
                    [0, middle @ (0 | 1), 0] => [0, *middle, 0],
                    _ => return Err(invalid("unsupported point prefix flags")),
                };
                let fixed = fixed(&mut r)?;
                let origin = match r.take(1)? {
                    [0] => false,
                    [1] => true,
                    _ => return Err(invalid("unsupported point origin flag")),
                };
                let linked = boolean(&mut r)?;
                metadata.point_auxiliary = boolean(&mut r)?;
                let mut number = || -> Result<f64> {
                    let n = f64::from_bits(r.u64()?);
                    if !n.is_finite() {
                        return Err(invalid("non-finite sketch point"));
                    }
                    Ok(n)
                };
                let position = [number()?, number()?, number()?];
                if origin && (!fixed || position != [0.; 3]) {
                    return Err(invalid("inconsistent sketch origin"));
                }
                r.take(30)?; // presentation fields
                if references(&mut r)? != [connections] {
                    return Err(invalid("unsupported point connection list"));
                }
                let sketch = schema.owner(&mut r, &mut metadata)?;
                schema.require_owner(&classes, sketch)?;
                metadata.validate(&classes)?;
                finish(&r)?;
                Ok(SketchPoint { id: *id, sketch, position, fixed, origin, linked, metadata })
            }),
        )
    }
}
