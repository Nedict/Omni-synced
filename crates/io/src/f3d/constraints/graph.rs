//! The versioned sketch relationship container, separate from constraint semantics.
use super::*;

#[derive(Debug)]
pub struct SketchConstraintGraph {
    pub geometry: u64,
    pub feature: u64,
    pub relations: Vec<u64>,
    pub dimensions: Vec<u64>,
    pub auxiliary: Vec<u64>,
    pub support: Option<u64>,
    pub external_identity: String,
}

impl Design {
    pub fn sketch_constraint_graphs(&self) -> Result<Vec<SketchConstraintGraph>> {
        if !matches!(self.layout, 4 | 5 | 8) {
            return Err(invalid("unsupported constraint metadata layout"));
        }
        let Some(class) = self.classes.iter().find(|c| c.id == "44A64366-4BD3-4B24-881A-F94C206E8F2D") else { return Ok(vec![]) };
        if !matches!(class.version, 16..=18) {
            return Err(invalid("unsupported sketch geometry version"));
        }
        let records = Records::new(self);
        let classes: BTreeMap<_, _> = self.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        self.recover_records(
            "constraint graph",
            class.objects.iter().map(|geometry| {
                let mut r = payload(records.primary(*geometry)?, *geometry)?;
                empty(&mut r)?;
                zeros(&mut r, 1)?;
                attributes::read(&mut r)?;
                if class.version == 18 {
                    r.wide_string()?;
                }
                r.u64()?;
                one(&mut r)?;
                let relations = references(&mut r)?;
                if class.version >= 17 {
                    one(&mut r)?;
                }
                let dimensions = references(&mut r)?;
                r.take(11)?; // inherited evaluation/presentation fields
                let auxiliary = references(&mut r)?;
                let support = match r.bytes.get(r.at) {
                    Some(0) => {
                        r.take(1)?;
                        None
                    }
                    Some(1) => Some(r.reference()?),
                    _ => return Err(invalid("unsupported sketch support reference")),
                };
                r.take(1)?;
                let feature = r.reference()?;
                r.take(1)?;
                let external_identity = if class.version < 18 { r.wide_string()? } else { String::new() };
                r.take(3)?;
                finish(&r)?;
                let mut seen = BTreeSet::new();
                for id in relations.iter().chain(&dimensions) {
                    if !seen.insert(*id) || !classes.contains_key(id) {
                        return Err(invalid("duplicate or missing sketch relationship"));
                    }
                }
                for id in auxiliary.iter().chain(support.iter()).chain(std::iter::once(&feature)) {
                    if !classes.contains_key(id) {
                        return Err(invalid("missing sketch container reference"));
                    }
                }
                Ok(SketchConstraintGraph { geometry: *geometry, feature, relations, dimensions, auxiliary, support, external_identity })
            }),
        )
    }
}
