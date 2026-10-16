//! Offset generators verified by independent source and distance edits.
use super::*;

#[derive(Debug, PartialEq)]
pub struct SketchOffset {
    pub id: u64,
    pub dimension: u64,
    pub relation: u64,
    pub binding: u64,
    pub distance: f64,
    pub source_group: u64,
    pub target_group: u64,
    pub sources: Vec<u64>,
    pub outputs: Vec<u64>,
}

pub(super) fn ordered(mut offsets: Vec<SketchOffset>) -> Result<Vec<SketchOffset>> {
    if offsets.len() > 512 || offsets.iter().map(|o| o.sources.len()).sum::<usize>() > 10_000 {
        return Err(invalid("too many offset dependencies"));
    }
    let mut owners = BTreeMap::new();
    for o in &offsets {
        for output in &o.outputs {
            if owners.insert(*output, o.id).is_some() {
                return Err(invalid("multiple offset producers"));
            }
        }
    }
    let mut done = BTreeSet::new();
    let mut result = Vec::new();
    while !offsets.is_empty() {
        let index = offsets
            .iter()
            .position(|o| o.sources.iter().all(|s| owners.get(s).is_none_or(|id| done.contains(id))))
            .ok_or_else(|| invalid("cyclic offset dependencies"))?;
        let offset = offsets.remove(index);
        done.insert(offset.id);
        result.push(offset);
    }
    Ok(result)
}

fn group(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, ids: [u64; 3], source: bool, layout: u32) -> Result<Vec<u64>> {
    let [id, other, owner] = ids;
    require_class(classes, id, "04e598ff-2979-5e41-8bc0-5c435c810909", 0)?;
    let mut r = payload(records.primary(id)?, id)?;
    empty(&mut r)?;
    one(&mut r)?;
    let mut curves = Vec::new();
    for _ in 0..r.count(15)? {
        // Connection slots change after dimension edits; they are not directions.
        curves.push(operand(&mut r)?);
    }
    if curves.is_empty() || curves.len() > 10_000 || curves.iter().copied().collect::<BTreeSet<_>>().len() != curves.len() {
        return Err(invalid("invalid offset curve group"));
    }
    if !attributes::read(&mut r)?.is_empty() || r.reference()? != other || r.u32()? != if source { 1 } else { 2 } {
        return Err(invalid("inconsistent offset group roles"));
    }
    finish(&r)?;
    let mut r = payload(records.secondary(id)?, id)?;
    empty(&mut r)?;
    if references(&mut r)? != curves || r.u64()? != 0 {
        return Err(invalid("inconsistent offset group curves"));
    }
    if matches!(layout, 4 | 5) {
        zeros(&mut r, 4)?;
    }
    if r.reference()? != owner {
        return Err(invalid("inconsistent offset group owner or curves"));
    }
    finish(&r)?;
    Ok(curves)
}

pub(super) fn decode(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    graph: &SketchConstraintGraph,
    id: u64,
    layout: u32,
) -> Result<SketchOffset> {
    require_class(classes, id, "afc07c10-99ac-5304-ae1b-dbee335b0e43", 1)?;
    let mut r = payload(records.primary(id)?, id)?;
    empty(&mut r)?;
    one(&mut r)?;
    if r.u32()? != 5 {
        return Err(invalid("unsupported offset generator arity"));
    }
    let mut operands = BTreeSet::new();
    for _ in 0..5 {
        if !operands.insert(operand(&mut r)?) {
            return Err(invalid("duplicate offset operand"));
        }
    }
    attributes::read(&mut r)?;
    let operation = r.reference()?;
    require_class(classes, operation, "B7F34D7B-DAC4-41A5-B67D-D672537C3BFA", 2)?;
    if !matches!(r.take(2)?, [0 | 1, 0]) {
        return Err(invalid("unsupported offset topology flags"));
    }
    let anchor = f64::from_bits(r.u64()?);
    if !anchor.is_finite() {
        return Err(invalid("non-finite offset anchor"));
    }
    let target_group = r.reference()?;
    let relation = r.reference()?;
    let dimension = r.reference()?;
    if r.reference()? != graph.geometry {
        return Err(invalid("offset belongs to another sketch"));
    }
    let source_group = r.reference()?;
    let first = r.reference()?;
    let distance = f64::from_bits(r.u64()?);
    finish(&r)?;
    if !distance.is_finite() || distance == 0. || operands != BTreeSet::from([first, source_group, target_group, dimension, relation]) {
        return Err(invalid("inconsistent offset generator inputs"));
    }
    if !graph.relations.contains(&relation) || !graph.dimensions.contains(&dimension) {
        return Err(invalid("offset relationships are outside their sketch"));
    }
    let sources = group(records, classes, [source_group, target_group, id], true, layout)?;
    let outputs = group(records, classes, [target_group, source_group, id], false, layout)?;
    let source_set: BTreeSet<_> = sources.iter().copied().collect();
    let output_set: BTreeSet<_> = outputs.iter().copied().collect();
    if sources.len() != outputs.len() || !source_set.contains(&first) || !source_set.is_disjoint(&output_set) {
        return Err(invalid("inconsistent offset source and output groups"));
    }
    require_class(classes, relation, "D3BD153B-EB8A-405E-9D29-69EE0C3D227C", 0)?;
    let mut r = payload(records.primary(relation)?, relation)?;
    empty(&mut r)?;
    one(&mut r)?;
    if r.u32()? as usize != sources.len() + outputs.len() {
        return Err(invalid("inconsistent offset relationship arity"));
    }
    let mut related = BTreeSet::new();
    for _ in 0..sources.len() + outputs.len() {
        if !related.insert(operand(&mut r)?) {
            return Err(invalid("duplicate offset relationship operand"));
        }
    }
    attributes::read(&mut r)?;
    if r.reference()? != graph.geometry || r.u64()? != 0x20_0000_0000 {
        return Err(invalid("invalid offset relationship kind or owner"));
    }
    let paired = references(&mut r)?;
    zeros(&mut r, 1)?;
    finish(&r)?;
    let expected: BTreeSet<_> = source_set.union(&output_set).copied().collect();
    if related != expected || paired.iter().copied().collect::<BTreeSet<_>>() != expected || paired.len() != expected.len() {
        return Err(invalid("inconsistent offset relationship curves"));
    }
    // The relation's references switch from interleaved to grouped after edits.
    // Keep the separate source/output group ordering and validate the resulting
    // geometric correspondence before attaching an editable association.
    require_class(classes, dimension, "8C780195-72C0-4a56-A911-E43AB14357F2", 3)?;
    let mut r = payload(records.primary(dimension)?, dimension)?;
    empty(&mut r)?;
    one(&mut r)?;
    if r.u32()? != 2 || operand(&mut r)? != 0 {
        return Err(invalid("invalid offset dimension operands"));
    }
    let dimension_curve = operand(&mut r)?;
    if !source_set.contains(&dimension_curve) {
        return Err(invalid("offset dimension refers outside its source group"));
    }
    attributes::read(&mut r)?;
    r.take(220)?;
    r.wide_string()?;
    r.wide_string()?;
    r.take(44)?;
    if r.u64()? != 0x10_0000_0000 {
        return Err(invalid("invalid offset dimension kind"));
    }
    let binding = r.reference()?;
    if references(&mut r)? != [dimension_curve] {
        return Err(invalid("invalid offset dimension curve"));
    }
    zeros(&mut r, 1)?;
    finish(&r)?;
    dimension_owner(records, dimension, graph.geometry)?;
    Ok(SketchOffset { id, dimension, relation, binding, distance, source_group, target_group, sources, outputs })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(id: u64, source: u64, output: u64) -> SketchOffset {
        SketchOffset {
            id,
            dimension: 0,
            relation: 0,
            binding: 0,
            distance: 1.,
            source_group: 0,
            target_group: 0,
            sources: vec![source],
            outputs: vec![output],
        }
    }

    #[test]
    fn dependencies_order_generators_and_reject_cycles_or_duplicate_producers() {
        let sorted = ordered(vec![offset(2, 11, 12), offset(1, 10, 11)]).unwrap();
        assert_eq!(sorted.iter().map(|o| o.id).collect::<Vec<_>>(), [1, 2]);
        assert!(ordered(vec![offset(1, 12, 11), offset(2, 11, 12)]).is_err());
        assert!(ordered(vec![offset(1, 10, 11), offset(2, 12, 11)]).is_err());
        assert!(ordered(vec![offset(1, 10, 10)]).is_err());
    }

    #[test]
    fn legacy_offset_groups_preserve_owners_and_reject_nonzero_extra_fields() {
        use super::super::super::offset_history_tests::fixture;
        let decode_fixture = |f: super::super::super::import_tests::Fixture, layout| {
            let design = f.design();
            let records = Records::new(&design);
            let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            let graphs = design.sketch_constraint_graphs().unwrap();
            decode(&records, &classes, &graphs[0], 300, layout)
        };
        let expected = decode_fixture(fixture(), 8).unwrap();
        for layout in [4, 5] {
            for malformed in [false, true] {
                let mut f = fixture();
                for id in [301, 302] {
                    let bytes = f.0.get_mut(&id).unwrap().3.as_mut().unwrap();
                    let at = bytes.len() - 11;
                    bytes.splice(at..at, [u8::from(malformed), 0, 0, 0]);
                }
                let result = decode_fixture(f, layout);
                if malformed {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), expected);
                }
            }
        }
    }
}
