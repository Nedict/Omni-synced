//! Observed circle and arc geometry. Coordinates are sketch-local centimetres;
//! placement and constraints are decoded separately from curve geometry.
use super::*;

#[derive(Debug, PartialEq)]
pub struct SketchCircle {
    pub id: u64,
    pub sketch: u64,
    pub center_point: u64,
    pub center: [f64; 3],
    pub normal: [f64; 3],
    pub x_axis: [f64; 3],
    pub radius: f64,
    pub construction: bool,
    pub fixed: bool,
    pub reference: bool,
    pub metadata: EntityMetadata,
    pub profile_id: Option<(u64, u64)>,
}

#[derive(Debug, PartialEq)]
pub struct SketchArc {
    pub circle: SketchCircle,
    pub start_point: u64,
    pub end_point: u64,
    pub start_angle: f64,
    pub end_angle: f64,
}

type CircularRecord = (SketchCircle, Option<(u64, u64, f64, f64)>);

impl Design {
    pub fn sketch_circles(&self) -> Result<Vec<SketchCircle>> {
        Ok(self.circular_records()?.into_iter().filter_map(|(circle, arc)| arc.is_none().then_some(circle)).collect())
    }

    pub fn sketch_arcs(&self) -> Result<Vec<SketchArc>> {
        Ok(self
            .circular_records()?
            .into_iter()
            .filter_map(|(circle, arc)| {
                arc.map(|(start_point, end_point, start_angle, end_angle)| SketchArc { circle, start_point, end_point, start_angle, end_angle })
            })
            .collect())
    }

    fn circular_records(&self) -> Result<Vec<CircularRecord>> {
        let Some(class) = self.classes.iter().find(|c| c.id == "F0130424-8B7E-4092-93C9-1CA807482534") else {
            return Ok(Vec::new());
        };
        if class.version != 0 {
            return Err(invalid("unsupported circle class version"));
        }
        let schema = entity::Schema::read(self)?;
        let records = Records::new(self);
        let classes: BTreeMap<_, _> = self.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        self.recover_records(
            "circle or arc",
            class.objects.iter().map(|id| {
                let (c, arc) = decode(records.primary(*id)?, schema)?;
                schema.require_owner(&classes, c.sketch)?;
                entity::require_point(&classes, c.center_point)?;
                c.metadata.validate(&classes)?;
                if c.id != *id {
                    return Err(invalid("inconsistent circle references"));
                }
                if let Some((a, b, _, _)) = arc {
                    if a == b || a == c.center_point || b == c.center_point {
                        return Err(invalid("inconsistent arc endpoint references"));
                    }
                    entity::require_point(&classes, a)?;
                    entity::require_point(&classes, b)?;
                }
                Ok((c, arc))
            }),
        )
    }
}

fn decode(bytes: &[u8], schema: entity::Schema) -> Result<CircularRecord> {
    let mut r = Reader { bytes, at: 0 };
    r.string()?;
    let id = r.u64()?;
    if !r.string()?.is_empty() || r.take(1)? != [0] {
        return Err(invalid("unsupported circle attributes"));
    }
    let attributes = attributes::read(&mut r)?;
    let mut number = || -> Result<f64> {
        let x = f64::from_bits(r.u64()?);
        if !x.is_finite() {
            return Err(invalid("non-finite circle geometry"));
        }
        Ok(x)
    };
    let center = [number()?, number()?, number()?];
    let normal = [number()?, number()?, number()?];
    let x_axis = [number()?, number()?, number()?];
    let radius = number()?;
    let start = number()?;
    let end = number()?;
    let dot = |a: &[f64; 3], b: &[f64; 3]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    if radius <= 0.0
        || !(end - start).is_finite()
        || end - start <= 1e-12
        || end - start > std::f64::consts::TAU + 1e-12
        || (dot(&normal, &normal) - 1.0).abs() > 1e-10
        || (dot(&x_axis, &x_axis) - 1.0).abs() > 1e-10
        || dot(&normal, &x_axis).abs() > 1e-10
    {
        return Err(invalid("unsupported circular geometry"));
    }
    let center_point = r.reference()?;
    let arc = if (end - start - std::f64::consts::TAU).abs() > 1e-12 {
        let end_point = r.reference()?;
        let start_point = r.reference()?;
        Some((start_point, end_point, start, end))
    } else {
        None
    };
    r.take(if arc.is_some() { 2 } else { 4 })?;
    let fixed = points::fixed(&mut r)?;
    let reference = points::boolean(&mut r)?;
    r.take(if schema.curve == 9 { 2 } else { 1 })?;
    let construction = match r.take(1)? {
        [0] => false,
        [1] => true,
        _ => return Err(invalid("unsupported circle construction flag")),
    };
    let mut metadata = EntityMetadata::default();
    let sketch = schema.curve_tail(&mut r, &mut metadata)?;
    if r.at != bytes.len() {
        return Err(invalid("unsupported circle payload tail"));
    }
    let profile_id = match (attributes.get("crv_primary_id"), attributes.get("crv_secondary_id")) {
        (Some(a), Some(b)) => Some((*a, *b)),
        (None, None) => None,
        _ => return Err(invalid("incomplete circle profile identity")),
    };
    Ok((SketchCircle { id, sketch, center_point, center, normal, x_axis, radius, construction, fixed, reference, profile_id, metadata }, arc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> Result<CircularRecord> {
        super::decode(bytes, entity::Schema { geometry: 18, curve: 9 })
    }

    fn circle(radius: f64) -> Vec<u8> {
        // Original synthetic data, deliberately not a copy of a Fusion record.
        let mut b = Vec::new();
        b.extend(1u32.to_le_bytes());
        b.push(b'7');
        b.extend(81u64.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend([0, 1]);
        b.extend(0u32.to_le_bytes());
        for x in [1.5, -2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, radius, 0.0, std::f64::consts::TAU] {
            b.extend(x.to_le_bytes());
        }
        b.push(1);
        b.extend(82u64.to_le_bytes());
        b.extend([0; 2]);
        b.extend([0; 44]);
        b.push(1);
        b.extend(83u64.to_le_bytes());
        b.extend([0; 2]);
        b
    }

    #[test]
    fn full_circle_keeps_local_coordinates_and_rejects_bad_payloads() {
        let b = circle(3.0);
        let (c, arc) = decode(&b).unwrap();
        assert!(arc.is_none());
        assert_eq!(c.center, [1.5, -2.0, 0.0]);
        assert_eq!((c.id, c.center_point, c.sketch), (81, 82, 83));
        assert_eq!(c.radius, 3.0);
        for n in 0..b.len() {
            assert!(decode(&b[..n]).is_err(), "prefix {n}");
        }
        for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(decode(&circle(radius)).is_err());
        }
        let mut bad = b.clone();
        bad[23..31].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode(&bad).is_err());
        let mut bad = b.clone();
        bad[47..55].copy_from_slice(&2f64.to_le_bytes());
        assert!(decode(&bad).is_err());
        let mut bad = b;
        bad.push(0);
        assert!(decode(&bad).is_err());
    }

    #[test]
    fn absent_circle_attributes_preserve_geometry() {
        let mut b = circle(3.);
        let expected = decode(&b).unwrap();
        b[18] = 0;
        b.drain(19..23);
        assert_eq!(decode(&b).unwrap(), expected);
    }
}
