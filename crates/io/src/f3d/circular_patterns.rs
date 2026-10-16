//! Circular-pattern relationships verified with independent count, angle,
//! symmetry, axis and source-kind controls. Retired operations are not instances.
use super::history::{empty, finish, payload, references, require_class, zeros};
use super::*;
use solidcraft_geom::Vec3;

pub(super) const CLASS: &str = "11F1A5CE-2B57-4476-8480-6994621493C9";
const HELPER: &str = "7493A99D-EDFC-4932-8B75-4120A8FA7875";
const AXIS: &str = "19031004-3AB6-4D13-ABBF-C51EE214688D";

#[derive(Debug, PartialEq)]
pub struct CircularPattern {
    pub axis: u64,
    pub angle: u64,
    pub count: u64,
    pub symmetric: bool,
    pub instances: usize,
    pub source_container: u64,
    /// Empty for body patterns; otherwise explicit feature identities.
    pub features: Vec<u64>,
    pub targets: Vec<u64>,
    pub outputs: Vec<u64>,
    /// Historical operation slots can outlive their instance objects.
    pub(super) operation_instances: BTreeMap<u64, Option<u64>>,
}

pub(super) fn inputs(
    r: &mut Reader<'_>,
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    owner: u64,
    body_version: u32,
) -> Result<CircularPattern> {
    empty(r)?;
    zeros(r, 1)?;
    if !super::attributes::read(r)?.is_empty() {
        return Err(invalid("unsupported circular-pattern attributes"));
    }
    let helper = r.reference()?;
    require_class(classes, helper, HELPER, 0)?;
    let sources = r.reference()?;
    require_class(classes, sources, "8DB90DAE-3FF5-4E01-BACC-00AAA3EBB3EF", 1)?;
    let roles = super::history::body_roles(r, classes, body_version)?;
    let outputs = roles.get(1).ok_or_else(|| invalid("missing circular-pattern outputs"))?.clone();
    if outputs.len() > 4096 || roles.iter().enumerate().any(|(i, ids)| i != 1 && !ids.is_empty()) {
        return Err(invalid("unsupported circular-pattern body roles"));
    }
    let mut unique = BTreeSet::new();
    for output in &outputs {
        if !unique.insert(*output) {
            return Err(invalid("duplicate circular-pattern output"));
        }
        require_class(classes, *output, "D26351F0-5940-4D23-AA20-2C35475A6D9E", 2)?;
        let mut o = payload(records.primary(*output)?, *output)?;
        zeros(&mut o, 6)?;
        if o.wide_string()?.is_empty() || !matches!(o.take(2)?, [0, 1] | [1, 1]) {
            return Err(invalid("unsupported circular-pattern output identity"));
        }
        let body = o.reference()?;
        if !classes.get(&body).is_some_and(|c| c.id == "D3937028-C20C-4E65-B010-94AD418A5C20" && matches!(c.version, 18 | 19))
            || o.reference()? != owner
        {
            return Err(invalid("circular-pattern output belongs to another feature"));
        }
        finish(&o)?;
    }
    let mut h = payload(records.primary(helper)?, helper)?;
    zeros(&mut h, 6)?;
    let symmetric = match h.take(1)? {
        [0] => false,
        [1] => true,
        _ => return Err(invalid("unsupported circular-pattern symmetry")),
    };
    let angle = h.reference()?;
    require_class(classes, angle, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0)?;
    let axis = axis_selector(records, classes, h.reference()?, owner)?;
    let handles = references(&mut h)?;
    if handles.len() > 64 || handles.iter().collect::<BTreeSet<_>>().len() != handles.len() {
        return Err(invalid("invalid circular-pattern targets"));
    }
    let targets = handles.iter().map(|id| super::loft_faces::producer(records, classes, *id)).collect::<Result<Vec<_>>>()?;
    let pattern_version = version(records, "3EF91258-FE47-45A5-957D-1295C3679496", 9);
    zeros(
        &mut h,
        match pattern_version {
            6 => 0,
            8 => 4,
            9 => 8,
            _ => return Err(invalid("unsupported inherited circular-pattern schema")),
        },
    )?;
    let compute = h.u32()?;
    let selection = h.u32()?;
    if compute != 2 || !matches!(selection, 1 | 2) || (pattern_version >= 8 && h.u32()? != u32::MAX) || h.take(1)? != [1] {
        return Err(invalid("unsupported circular-pattern selection or computation mode"));
    }
    let mut operations = BTreeMap::new();
    let mut tags = BTreeSet::new();
    let operation_count = h.count(16)?;
    if operation_count > 4096 {
        return Err(invalid("too many circular-pattern operation records"));
    }
    for _ in 0..operation_count {
        let index = h.u32()?;
        zeros(&mut h, 4)?;
        let tag = h.u64()?;
        if !(2..=4096).contains(&index) || tag == 0 || operations.insert(index, tag).is_some() || !tags.insert(tag) {
            return Err(invalid("invalid circular-pattern operation identity"));
        }
    }
    let instances = references(&mut h)?;
    if !(1..=4096).contains(&instances.len()) || instances.iter().collect::<BTreeSet<_>>().len() != instances.len() {
        return Err(invalid("invalid circular-pattern instance count"));
    }
    for (index, instance) in instances.iter().enumerate() {
        require_class(classes, *instance, "AD719C1A-11AF-4537-BB41-030A9091D36B", 0)?;
        let mut r = payload(records.primary(*instance)?, *instance)?;
        zeros(&mut r, 6)?;
        if r.u32()? != index as u32 + 1 || r.reference()? != helper || (index > 0 && !operations.contains_key(&(index as u32 + 1))) {
            return Err(invalid("inconsistent circular-pattern instance ownership"));
        }
        finish(&r)?;
    }
    let count = h.reference()?;
    require_class(classes, count, "A085449A-5144-4B2B-B455-7F7035A40559", 1)?;
    if h.reference()? != owner {
        return Err(invalid("circular-pattern helper belongs to another feature"));
    }
    let source_container = h.reference()?;
    zeros(&mut h, 4)?;
    finish(&h)?;
    // Parameters validates the reciprocal parameter link and its numeric value.
    // Validate the current instance count here as well, without using retired tags.
    let mut c = payload(records.primary(count)?, count)?;
    empty(&mut c)?;
    if c.take(5)? != [1, 1, 0, 0, 0] || c.reference()? != owner {
        return Err(invalid("inconsistent circular-pattern count owner"));
    }
    c.take(5)?;
    if c.take(1)? != [1] || c.u32()? as usize != instances.len() {
        return Err(invalid("circular-pattern count disagrees with its instances"));
    }
    let parameter = c.reference()?;
    if !classes.get(&parameter).is_some_and(|c| c.id == "7A6A3D31-BE74-4E19-9115-20642944C2E4" && matches!(c.version, 5 | 6 | 7 | 9)) {
        return Err(invalid("missing circular-pattern count parameter"));
    }
    super::mirrors::owner_tail(&mut c, owner)?;
    let objects = super::loft_faces::objects(records, classes, source_container, owner)?;
    if !(1..=64).contains(&objects.len()) || objects.iter().collect::<BTreeSet<_>>().len() != objects.len() {
        return Err(invalid("invalid circular-pattern sources"));
    }
    let mut s = payload(records.primary(sources)?, sources)?;
    zeros(&mut s, 6)?;
    cached_groups(&mut s, classes)?;
    if s.count(15)? != objects.len() {
        return Err(invalid("inconsistent circular-pattern source count"));
    }
    let mut features = Vec::new();
    let mut new_bodies = 0;
    for object in &objects {
        if selection == 2 {
            require_class(classes, *object, "9716F783-676D-42E0-93F9-EBF273E7C035", 1)?;
            new_bodies += 1;
        } else {
            let (feature, bodies) = super::mirrors::feature_source(records, classes, *object, owner)?;
            if features.contains(&feature) {
                return Err(invalid("duplicate circular-pattern feature"));
            }
            let mut input = payload(records.primary(feature)?, feature)?;
            let input = super::history::extrude_inputs(&mut input, classes)?;
            if !matches!(input.limit, super::history::ExtrudeLimit::Distance(_)) {
                return Err(invalid("To Object circular-pattern feature computation is not supported yet"));
            }
            match input.operation {
                solidcraft_doc::Operation::NewBody if bodies == [feature] => new_bodies += 1,
                solidcraft_doc::Operation::Join | solidcraft_doc::Operation::Cut if !targets.is_empty() && bodies == targets => {}
                _ => return Err(invalid("circular-pattern feature operation disagrees with its targets")),
            }
            features.push(feature);
        }
        if s.reference()? != *object {
            return Err(invalid("circular-pattern source relationships disagree"));
        }
        cached_groups(&mut s, classes)?;
    }
    if s.reference()? != owner {
        return Err(invalid("circular-pattern sources belong to another feature"));
    }
    super::mirrors::owner_tail(&mut s, owner)?;
    if outputs.len() != new_bodies * (instances.len() - 1) || (new_bodies == objects.len() && !targets.is_empty()) {
        return Err(invalid("circular-pattern outputs disagree with its sources or active instances"));
    }
    let operation_instances = operations.into_iter().map(|(index, tag)| (tag, instances.get(index as usize - 1).copied())).collect();
    Ok(CircularPattern {
        axis,
        angle,
        count,
        symmetric,
        instances: instances.len(),
        source_container,
        features,
        targets,
        outputs,
        operation_instances,
    })
}

pub(super) struct BodyOutput {
    pub index: Option<usize>,
    pub source_feature: u64,
    /// Additional operations must be certified as modifiers of this source
    /// before the pattern, against the reconstructed document timeline.
    pub contributors: BTreeSet<u64>,
}

/// Resolve a copy through its source, pattern and live instance operation groups.
/// Output handle registration order is unrelated to generated body order.
pub(super) fn body_output(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    owners: &BTreeMap<u64, u64>,
    feature: u64,
    handle: Option<u64>,
    operation: u64,
    label: u64,
) -> Result<BodyOutput> {
    let pattern = read_inputs(records, classes, feature)?;
    if !pattern.features.is_empty()
        || pattern.symmetric
        || handle.is_some_and(|handle| !pattern.outputs.contains(&handle))
        || owners.get(&operation) != Some(&feature)
    {
        return Err(invalid("unsupported patterned body selector"));
    }
    let (_, mut groups) = if let Some(handle) = handle {
        super::body_labels::output_groups(records, classes, feature, label, handle)?
    } else {
        super::body_labels::resolve_groups(records, classes, feature, label, None)?
    };
    if groups.len() < 3 || !groups.remove(&operation) {
        return Err(invalid("inconsistent patterned body operation groups"));
    }
    let mut source_index = None;
    let mut source_identity = None;
    for (index, object) in super::loft_faces::objects(records, classes, pattern.source_container, feature)?.into_iter().enumerate() {
        let source = super::loft_faces::target_object(records, classes, object, feature)?;
        let [source_operation] = source.operations.as_slice() else { return Err(invalid("unsupported pattern source contributors")) };
        if owners.get(&source_operation.operation) != Some(&source.feature) {
            return Err(invalid("pattern source operation belongs to another feature"));
        }
        super::body_labels::validate(records, classes, source.feature, source_operation.operation, u64::from(source_operation.label))?;
        if groups.contains(&source_operation.operation) {
            if source_index.replace(index).is_some() {
                return Err(invalid("ambiguous patterned body source"));
            }
            source_identity = Some((source.feature, source_operation.operation));
        }
    }
    let source_index = source_index.ok_or_else(|| invalid("missing patterned body source group"))?;
    let (source_feature, source_operation) = source_identity.ok_or_else(|| invalid("missing pattern source identity"))?;
    groups.remove(&source_operation);
    let mut instance_index = None;
    let mut retired = false;
    for (tag, instance) in &pattern.operation_instances {
        if !groups.contains(tag) {
            continue;
        }
        if instance_index.is_some() || retired {
            return Err(invalid("ambiguous patterned body instance"));
        }
        let Some(instance) = *instance else {
            if handle.is_some() || owners.contains_key(tag) {
                return Err(invalid("retired pattern instance retains a live handle or owner"));
            }
            retired = true;
            groups.remove(tag);
            continue;
        };
        if handle.is_none() || owners.get(tag) != Some(&instance) {
            return Err(invalid("ambiguous patterned body instance"));
        }
        let mut r = payload(records.primary(instance)?, instance)?;
        zeros(&mut r, 6)?;
        instance_index = Some(r.u32()? as usize);
        groups.remove(tag);
    }
    if retired {
        return Ok(BodyOutput { index: None, source_feature, contributors: groups });
    }
    let instance_index =
        instance_index.filter(|index| (2..=pattern.instances).contains(index)).ok_or_else(|| invalid("missing patterned body instance group"))?;
    let index = source_index * (pattern.instances - 1) + instance_index - 2;
    if index >= pattern.outputs.len() {
        return Err(invalid("patterned body index exceeds outputs"));
    }
    Ok(BodyOutput { index: Some(index), source_feature, contributors: groups })
}

pub(super) fn read_inputs(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, feature: u64) -> Result<CircularPattern> {
    require_class(classes, feature, CLASS, 0)?;
    let mut r = payload(records.primary(feature)?, feature)?;
    inputs(&mut r, records, classes, feature, version(records, "2EDC2536-D95B-4469-8ACA-DC07124D9C43", 2))
}

fn cached_groups(r: &mut Reader<'_>, classes: &BTreeMap<u64, &Class>) -> Result<()> {
    for group in references(r)? {
        require_class(classes, group, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1)?;
    }
    Ok(())
}

fn version(records: &Records<'_>, class: &str, modern: u32) -> u32 {
    records.design.classes.iter().find(|c| c.id == class).map(|c| c.version).unwrap_or(if records.design.layout == 8 { modern } else { 0 })
}

fn axis_selector(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, owner: u64) -> Result<u64> {
    let v = classes
        .get(&id)
        .filter(|c| c.id == "F2A7590D-6654-4674-B393-A2AEF4FEC48A" && matches!(c.version, 1 | 4))
        .map(|c| c.version)
        .ok_or_else(|| invalid("unsupported circular-pattern axis selector"))?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    if r.u32()? != 8 {
        return Err(invalid("unsupported circular-pattern axis method"));
    }
    r.take(64)?; // cached display extents; never choose an axis from coordinates
    if r.u32()? != 7 {
        return Err(invalid("unsupported circular-pattern axis kind"));
    }
    if v == 4 {
        zeros(&mut r, 1)?;
        for _ in 0..3 {
            if f64::from_bits(r.u64()?) != -1. {
                return Err(invalid("unsupported circular-pattern axis extent"));
            }
        }
    }
    let objects = references(&mut r)?;
    let [selector] = objects.as_slice() else { return Err(invalid("circular pattern requires one tracked axis")) };
    zeros(&mut r, 2)?;
    super::planes::evaluation_tail(&mut r, classes, owner, if v == 1 { 59 } else { 60 })?;
    let geometry = super::planes::tracked_entity(records, classes, *selector, owner)?;
    require_class(classes, geometry, AXIS, 0)?;
    Ok(geometry)
}

/// Only certified root axes are fixed in the document frame. Other construction
/// axes require their editable construction dependency, not cached geometry.
pub(super) fn origin_axis(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, root: u64) -> Result<Vec3> {
    require_class(classes, id, AXIS, 0)?;
    let mut r = payload(records.primary(id)?, id)?;
    empty(&mut r)?;
    zeros(&mut r, 1)?;
    super::attributes::read(&mut r)?;
    let mut vector = || -> Result<Vec3> { Ok(Vec3::new(f64::from_bits(r.u64()?), f64::from_bits(r.u64()?), f64::from_bits(r.u64()?))) };
    let (origin, direction) = (vector()?, vector()?);
    r.take(16)?;
    match version(records, "1E5F6CE7-7544-4DE1-8C7B-6BACF6DC8E40", 5) {
        3 => {}
        5 => {
            r.take(5)?;
        }
        _ => return Err(invalid("unsupported inherited construction-axis schema")),
    }
    if r.u32()? != u32::MAX || r.take(1)? != [1] {
        return Err(invalid("unsupported origin-axis identity"));
    }
    let expected = match r.wide_string()?.as_str() {
        "X" => Vec3::X,
        "Y" => Vec3::Y,
        "Z" => Vec3::Z,
        _ => return Err(invalid("unsupported root-axis name")),
    };
    if !origin.is_finite() || !direction.is_finite() || origin.len() > 1e-10 || (direction - expected).len() > 1e-10 {
        return Err(invalid("root-axis identity and geometry disagree"));
    }
    r.take(match version(records, "A0A15D26-1F3B-4120-A3F1-9CDDA189AB74", 2) {
        1 => 36,
        2 => 37,
        _ => return Err(invalid("unsupported inherited root-axis schema")),
    })?;
    let group = r.reference()?;
    finish(&r)?;
    require_class(classes, group, "2972A3B8-3627-49BC-8C3D-18707605EC44", 1)?;
    let mut r = payload(records.primary(group)?, group)?;
    zeros(&mut r, 8)?;
    let axes = references(&mut r)?;
    if axes.iter().filter(|axis| **axis == id).count() != 1 || r.reference()? != root {
        return Err(invalid("axis is not part of the root origin"));
    }
    finish(&r)?;
    Ok(expected)
}

#[cfg(test)]
#[path = "circular_pattern_tests.rs"]
mod tests;
