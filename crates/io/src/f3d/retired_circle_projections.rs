//! A circular cap whose old bounded-face selector retains a retired operation.
//! The current cached boundary and mapped entity must independently name the
//! same live extrusion edge. Neither the retired operation nor cached geometry
//! can supply a replacement body or a guessed edge.
use super::body_projections::{Edge, Recipe};
use super::history::{finish, payload, references, require_class, zeros};
use super::*;

const IDENTITY: &str = "7ACC2A03-0261-4879-A14A-A93D661A5BDC";
const TRACKER: &str = "4B31DA1B-FCD6-4527-BC3E-BE1FBAF867FE";

fn integers(r: &mut Reader<'_>, values: &[i32]) -> Result<()> {
    for value in values {
        if r.u32()? != *value as u32 {
            return Err(invalid("unsupported retired-cap incidence"));
        }
    }
    Ok(())
}

fn terms(r: &mut Reader<'_>, labels: &[&str], kind: &str) -> Result<Vec<u64>> {
    zeros(r, 6)?;
    integers(r, &[1, 3])?;
    if r.u32()? as usize != labels.len() {
        return Err(invalid("unsupported retired-cap term count"));
    }
    let mut operations = Vec::new();
    for label in labels {
        integers(r, &[1])?;
        if r.string()? != *label {
            return Err(invalid("unsupported retired-cap face role"));
        }
        zeros(r, 4)?;
        integers(r, &[1])?;
        let operation = r.u64()?;
        if operation == 0 {
            return Err(invalid("empty retired-cap operation"));
        }
        operations.push(operation);
    }
    zeros(r, 4)?;
    if r.string()? != kind {
        return Err(invalid("incorrect retired-cap recipe kind"));
    }
    Ok(operations)
}

fn cap(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64) -> Result<(u64, u64)> {
    require_class(classes, id, IDENTITY, 3)?;
    let mut r = payload(records.primary(id)?, id)?;
    let operations = terms(&mut r, &["1", "2"], "bounded_face_recipe_data")?;
    let [base, retired] = operations.as_slice() else { return Err(invalid("missing retired-cap operations")) };
    if base == retired {
        return Err(invalid("retired cap requires distinct operations"));
    }
    integers(&mut r, &[0, -1, 1, -1, -1, 2, 0, -1, 1, -1, 1, -1, 2, 1, -1, 0, -1, 0, 1, 0, 2, 1, 1, 1, 1, 0, 1, -1])?;
    finish(&r)?;
    Ok((*base, *retired))
}

fn tracked_edge(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, tracker: u64, operation: u64) -> Result<()> {
    let id = super::projections::tracking_identity(records, classes, tracker, TRACKER, 1)?;
    require_class(classes, id, IDENTITY, 3)?;
    let mut r = payload(records.primary(id)?, id)?;
    let operations = terms(&mut r, &["-102", "3", "1"], "edge_recipe_data")?;
    if operations.iter().any(|tag| *tag != operation) {
        return Err(invalid("retired-cap boundary disagrees with its live producer"));
    }
    // This tracked origin edge has the closed-loop orientation of the retired
    // bounded selector. Keep it distinct from the ordinary extrusion-edge form.
    integers(&mut r, &[0, -1, 2, 1, -1, 2, -1, 1, -1, 2, 1, -1, 2, -1, 1, 0, -1, 0, 0, -1, 1, 0, 2, 1, 1, 1, 1, 0, 1, -1])?;
    finish(&r)
}

fn cached_circle(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, operation: u64) -> Result<()> {
    use solidcraft_geom::Vec3;
    require_class(classes, id, "B0B3FA59-C374-444F-B424-24AA5DE41645", 0)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    let mut values = [0.; 12];
    for value in &mut values {
        *value = f64::from_bits(r.u64()?);
        if !value.is_finite() {
            return Err(invalid("non-finite retired-cap circle"));
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
        return Err(invalid("retired-cap boundary is not a complete circle"));
    }
    zeros(&mut r, 1)?;
    require_class(classes, r.reference()?, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1)?;
    if r.take(2)? != [0, 1] {
        return Err(invalid("unsupported retired-cap circle tracking"));
    }
    let tracker = r.reference()?;
    finish(&r)?;
    tracked_edge(records, classes, tracker, operation)
}

pub(super) fn decode(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    selector: u64,
    owner: u64,
    mappings: &[(u64, u64)],
) -> Result<Vec<super::projections::Projection>> {
    let [(tracking, target)] = mappings else { return Err(invalid("retired circular cap requires one mapped edge")) };
    require_class(classes, *target, "F0130424-8B7E-4092-93C9-1CA807482534", 0)?;
    let version = super::loft_faces::selector_version(classes, selector)?;
    let mut r = payload(records.primary(selector)?, selector)?;
    zeros(&mut r, if version == 2 { 7 } else { 8 })?;
    let (operation, retired) = cap(records, classes, r.reference()?)?;
    integers(&mut r, &[1])?;
    r.wide_string()?;
    r.wide_string()?;
    integers(&mut r, &[2, 0])?;
    let cached = references(&mut r)?;
    let [cached] = cached.as_slice() else { return Err(invalid("retired circular cap requires one cached boundary")) };
    let handle = super::loft_faces::selector_tail(&mut r, records, owner)?;
    let (body, feature) = super::body_projections::body_owner(records, classes, handle)?;
    let (owners, retired_operations) = records.design.operation_index()?;
    if owners.get(&operation) != Some(&feature) || owners.contains_key(&retired) || !retired_operations.contains(&retired) {
        return Err(invalid("retired circular cap has unresolved or live boundary ownership"));
    }
    cached_circle(records, classes, *cached, operation)?;
    tracked_edge(records, classes, *tracking, operation)?;
    // Attachment resolves the current, named origin edge of this exact producer
    // and checks the native target circle against its regenerated geometry.
    Ok(vec![super::projections::Projection {
        target: *target,
        source: super::projections::Source::Body(Edge { body, feature, recipe: Recipe { operation, end: false, side: None }, line: None }),
    }])
}

#[cfg(test)]
mod tests;
