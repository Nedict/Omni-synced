//! Reconstruct offset dependencies only after checking their complete native geometry.
use super::constraints::SketchOffset;
use super::*;
use solidcraft_doc::sketch::{CurveKind, OffsetChain, Sketch};

pub(super) fn metadata(native: &projections::Geometry<'_>, constraints: &BTreeMap<u64, SketchConstraints>) -> Result<()> {
    let mut relationships: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    let mut outputs = BTreeSet::new();
    let mut centers = BTreeSet::new();
    for (geometry, constraints) in constraints {
        for constraint in &constraints.constraints {
            let SketchConstraint::OffsetCurves(offset) = constraint else { continue };
            for (group, curves, generated) in [(offset.source_group, &offset.sources, false), (offset.target_group, &offset.outputs, true)] {
                for id in curves {
                    let owner = native.lines.get(id).map(|c| c.sketch).or_else(|| native.circles.get(id).map(|c| c.sketch));
                    if owner != Some(*geometry) || (generated && !outputs.insert(*id)) {
                        return Err(invalid("unsupported or conflicting offset curve ownership"));
                    }
                    relationships.entry(*id).or_default().insert(group);
                    if generated && let Some(c) = native.circles.get(id) {
                        centers.insert(c.center_point);
                    }
                }
            }
        }
    }
    let check = |id: u64, m: &EntityMetadata, point: bool| {
        let actual: BTreeSet<_> = m.relationships.iter().copied().collect();
        let expected = relationships.get(&id).cloned().unwrap_or_default();
        if m.specialized
            || m.point_prefix != [0; 3]
            || !m.definition.is_empty()
            || !m.legacy.is_empty()
            || actual != expected
            || actual.len() != m.relationships.len()
            || (m.point_auxiliary && !(point && centers.contains(&id)))
        {
            return Err(invalid("sketch entity relationship reconstruction is not supported yet"));
        }
        Ok(())
    };
    for p in native.points.values() {
        check(p.id, &p.metadata, true)?;
    }
    for c in native.circles.values() {
        check(c.id, &c.metadata, false)?;
    }
    for a in native.arcs.values() {
        check(a.circle.id, &a.circle.metadata, false)?;
    }
    for l in native.lines.values() {
        check(l.id, &l.metadata, false)?;
    }
    Ok(())
}

pub(super) fn attach(sketch: &mut Sketch, offset: &SketchOffset, parameter: &Parameter) -> Result<()> {
    if (parameter.value - offset.distance).abs() > 1e-10 || !parameter.value.is_finite() {
        return Err(invalid("offset distance disagrees with its parameter"));
    }
    let sources: Vec<_> = offset.sources.iter().map(|id| format!("c{id}")).collect();
    let outputs: Vec<_> = offset.outputs.iter().map(|id| format!("c{id}")).collect();
    let kind = |id: &str| sketch.curve_index(id).and_then(|i| sketch.curves.get(i)).map(|c| &c.kind).ok_or_else(|| invalid("missing offset curve"));
    let single_circle = if let ([source], [output]) = (sources.as_slice(), outputs.as_slice()) {
        if matches!(kind(source)?, CurveKind::Circle { .. }) {
            if !matches!(kind(output)?, CurveKind::Circle { .. }) {
                return Err(invalid("offset circle changed type"));
            }
            true
        } else {
            false
        }
    } else {
        false
    };
    let closed = if single_circle {
        true
    } else {
        let mut endpoints = Vec::new();
        for (source, output) in sources.iter().zip(&outputs) {
            let (CurveKind::Line { a, b }, CurveKind::Line { .. }) = (kind(source)?, kind(output)?) else {
                return Err(invalid("unsupported offset chain geometry"));
            };
            endpoints.push((*a, *b));
        }
        if endpoints.windows(2).any(|w| w[0].1 != w[1].0) {
            return Err(invalid("disconnected offset source chain"));
        }
        endpoints.first().zip(endpoints.last()).is_some_and(|(a, b)| a.0 == b.1)
    };
    // Native open offsets retain independent endpoint extents. SolidCraft's
    // analytic chain currently extends both endpoints with its source.
    if !closed {
        return Err(invalid("open offset endpoint reconstruction is not supported yet"));
    }
    let original = sketch.clone();
    sketch.offsets.push(OffsetChain {
        sources: sources.into_iter().map(|id| (id, true)).collect(),
        outputs: outputs.clone(),
        distance: -offset.distance * 10.,
        distance_expression: Some(format!("-({})", parameter.name)),
        closed,
        topology: None,
    });
    sketch.refresh_offsets().map_err(|e| invalid(&e.to_string()))?;
    for id in outputs {
        let index = sketch.curve_index(&id).ok_or_else(|| invalid("missing regenerated offset"))?;
        let actual = sketch.curves.get(index).ok_or_else(|| invalid("missing regenerated offset"))?;
        let expected = original.curves.get(index).ok_or_else(|| invalid("missing native offset"))?;
        for pi in actual.kind.point_ids() {
            let a = sketch.point(pi).ok_or_else(|| invalid("missing offset point"))?;
            let b = original.point(pi).ok_or_else(|| invalid("missing native offset point"))?;
            if a.dist(b) > 1e-7 {
                return Err(invalid("offset geometry disagrees with its source"));
            }
        }
        if let (CurveKind::Circle { r: a, .. }, CurveKind::Circle { r: b, .. }) = (&actual.kind, &expected.kind)
            && (a - b).abs() > 1e-7
        {
            return Err(invalid("offset radius disagrees with its source"));
        }
    }
    Ok(())
}
