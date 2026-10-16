//! Point, line and driving-dimension relationships observed in controlled sketches.
//! Other relationships sharing the generic constraint class fail explicitly.
use super::history::{empty, finish, payload, references, require_class, zeros};
use super::*;
mod graph;
mod offsets;
pub use graph::SketchConstraintGraph;
pub use offsets::SketchOffset;

#[derive(Debug, PartialEq)]
pub struct SketchConstraints {
    pub geometry: u64,
    pub feature: u64,
    pub constraints: Vec<SketchConstraint>,
}

#[derive(Debug, PartialEq)]
pub enum SketchConstraint {
    OffsetCurves(SketchOffset),
    Coincident { id: u64, points: [u64; 2] },
    Horizontal { id: u64, line: u64 },
    Vertical { id: u64, line: u64 },
    Linear { id: u64, points: [u64; 2], orientation: LinearOrientation, binding: u64 },
    Offset { id: u64, line: u64, entity: u64, binding: u64 },
    Angle { id: u64, lines: [u64; 2], quadrants: [u32; 2], binding: u64, cached_radians: f64 },
    Diameter { id: u64, circle: u64, binding: u64 },
    Radius { id: u64, circle: u64, binding: u64 },
    AlignedPoints { id: u64, points: [u64; 2], vertical: bool },
    PointCurve { id: u64, point: u64, curve: u64, midpoint: bool },
    LineMidpoints { id: u64, lines: [u64; 2] },
    Curves { id: u64, curves: [u64; 2], relation: CurveRelation },
    Symmetry { id: u64, axis: u64, entities: [u64; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinearOrientation {
    Horizontal,
    Vertical,
    Aligned,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveRelation {
    Collinear,
    Concentric,
    Parallel,
    Perpendicular,
    Tangent,
    Equal,
}

fn one(r: &mut Reader<'_>) -> Result<()> {
    if r.take(1)? != [1] {
        return Err(invalid("unsupported constraint flags"));
    }
    Ok(())
}

fn operand(r: &mut Reader<'_>) -> Result<u64> {
    let id = r.reference()?;
    // Native graph connection slot, not geometry or dimension orientation.
    // Reordering dimensions changes these slots without changing the constraint.
    // They are not used as indexes: semantics come from explicit entity references,
    // the kind flag, owner and parameter binding, all checked below.
    r.u32()?;
    Ok(id)
}

impl Design {
    pub fn sketch_constraints(&self) -> Result<Vec<SketchConstraints>> {
        let graphs = self.sketch_constraint_graphs()?;
        let records = Records::new(self);
        let classes: BTreeMap<_, _> = self.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let parameters = self.parameters()?;
        let bindings: BTreeMap<_, _> = parameters.iter().filter_map(|p| p.binding.zip(p.owner)).collect();
        let circles: BTreeMap<_, _> =
            self.sketch_circles()?.into_iter().chain(self.sketch_arcs()?.into_iter().map(|a| a.circle)).map(|c| (c.id, c.sketch)).collect();
        let lines: BTreeMap<_, _> = self.sketch_lines()?.into_iter().map(|c| (c.id, c.sketch)).collect();
        let arcs: BTreeSet<_> = self.sketch_arcs()?.into_iter().map(|a| a.circle.id).collect();
        self.recover_records(
            "sketch constraints",
            graphs.into_iter().map(|graph| {
                let geometry = &graph.geometry;
                // Constraint semantics do not depend on the support face. The
                // importer separately validates and preserves its live attachment.
                if graph.support.is_some_and(|id| {
                    !classes.get(&id).is_some_and(|c| c.id == "D3937028-C20C-4E65-B010-94AD418A5C20" && matches!(c.version, 18 | 19))
                }) {
                    return Err(invalid("unsupported sketch support body"));
                }
                let offsets = offsets::ordered(
                    graph.auxiliary.iter().map(|id| offsets::decode(&records, &classes, &graph, *id, self.layout)).collect::<Result<Vec<_>>>()?,
                )?;
                let mut offset_records = BTreeSet::new();
                for offset in &offsets {
                    if bindings.get(&offset.binding) != Some(&graph.feature)
                        || !offset_records.insert(offset.relation)
                        || !offset_records.insert(offset.dimension)
                    {
                        return Err(invalid("conflicting offset relationships or parameter owner"));
                    }
                }
                let feature = graph.feature;
                let feature_class = classes.get(&feature).ok_or_else(|| invalid("missing sketch feature"))?;
                if feature_class.id != "8DA771B7-52ED-42FC-94E2-2353FE141373" || !matches!(feature_class.version, 3..=5) {
                    return Err(invalid("unsupported sketch feature owner"));
                }
                let relations = graph.relations;
                let dimensions = graph.dimensions;
                let mut seen = BTreeSet::new();
                let mut constraints: Vec<_> = offsets.into_iter().map(SketchConstraint::OffsetCurves).collect();
                let point = |id: &u64| entity::require_point(&classes, *id).is_ok();
                let line = |id: &u64| lines.get(id) == Some(geometry);
                let round = |id: &u64| circles.get(id) == Some(geometry);
                for id in relations {
                    if offset_records.contains(&id) {
                        continue;
                    }
                    if !seen.insert(id) {
                        return Err(invalid("duplicate sketch constraint"));
                    }
                    require_class(&classes, id, "60403D47-0C49-49B0-BDE8-1679608164A2", 3)?;
                    let mut r = payload(records.primary(id)?, id)?;
                    empty(&mut r)?;
                    one(&mut r)?;
                    let count = r.u32()?;
                    if !(1..=3).contains(&count) {
                        return Err(invalid("unsupported geometric constraint arity"));
                    }
                    let mut operands = Vec::new();
                    for _ in 0..count {
                        operands.push(operand(&mut r)?);
                    }
                    attributes::read(&mut r)?;
                    if r.reference()? != *geometry {
                        return Err(invalid("inconsistent coincidence owner"));
                    }
                    // The class is shared by geometric relationships; operand
                    // count and point types alone do not establish coincidence.
                    let kind = r.u64()?;
                    let back = references(&mut r)?;
                    let operands_set: BTreeSet<_> = operands.iter().copied().collect();
                    if back.len() != operands.len()
                        || operands_set.len() != operands.len()
                        || back.iter().copied().collect::<BTreeSet<_>>() != operands_set
                    {
                        return Err(invalid("inconsistent coincidence references"));
                    }
                    zeros(&mut r, 1)?;
                    finish(&r)?;
                    // Symmetry's ordered references are entity one, entity two,
                    // axis. Graph operands follow registration order instead;
                    // an axis created last must not become a mirrored entity.
                    let semantic = if kind == 0x400 { back.as_slice() } else { operands.as_slice() };
                    constraints.push(match (kind, semantic) {
                        (1, [a, b]) if point(a) && point(b) => SketchConstraint::Coincident { id, points: [*a, *b] },
                        (0x400, [a, b, axis])
                            if line(axis)
                                && ((point(a) && point(b))
                                    || (line(a) && line(b))
                                    || (round(a) && round(b) && !arcs.contains(a) && !arcs.contains(b))) =>
                        {
                            SketchConstraint::Symmetry { id, axis: *axis, entities: [*a, *b] }
                        }
                        (1 | 0x1000, [c, p]) if point(p) && (line(c) || (kind == 1 && round(c))) => {
                            SketchConstraint::PointCurve { id, point: *p, curve: *c, midpoint: kind == 0x1000 }
                        }
                        (1 | 0x1000, [p, c]) if point(p) && (line(c) || (kind == 1 && round(c))) => {
                            SketchConstraint::PointCurve { id, point: *p, curve: *c, midpoint: kind == 0x1000 }
                        }
                        (0x1000, [a, b]) if line(a) && line(b) => SketchConstraint::LineMidpoints { id, lines: [*a, *b] },
                        (64 | 128, [a, b]) if point(a) && point(b) => SketchConstraint::AlignedPoints { id, points: [*a, *b], vertical: kind == 128 },
                        (2 | 16 | 32, [a, b]) if line(a) && line(b) => SketchConstraint::Curves {
                            id,
                            curves: [*a, *b],
                            relation: if kind == 2 {
                                CurveRelation::Collinear
                            } else if kind == 16 {
                                CurveRelation::Parallel
                            } else {
                                CurveRelation::Perpendicular
                            },
                        },
                        (4, [a, b]) if round(a) && round(b) => SketchConstraint::Curves { id, curves: [*a, *b], relation: CurveRelation::Concentric },
                        (0x800, [a, b]) if (line(a) && line(b)) || (round(a) && round(b)) => {
                            SketchConstraint::Curves { id, curves: [*a, *b], relation: CurveRelation::Equal }
                        }
                        (0x100, [a, b]) if (line(a) && round(b)) || (round(a) && (line(b) || round(b))) => {
                            SketchConstraint::Curves { id, curves: [*a, *b], relation: CurveRelation::Tangent }
                        }
                        (64 | 128, [line]) if lines.get(line) == Some(geometry) => {
                            if kind == 64 {
                                SketchConstraint::Horizontal { id, line: *line }
                            } else {
                                SketchConstraint::Vertical { id, line: *line }
                            }
                        }
                        _ => return Err(invalid("unsupported geometric constraint kind")),
                    });
                }
                for id in dimensions {
                    if offset_records.contains(&id) {
                        continue;
                    }
                    if !seen.insert(id) {
                        return Err(invalid("duplicate sketch dimension"));
                    }
                    if let Some(class) = classes.get(&id).filter(|c| c.id == "6EEAFDE5-59C4-424D-8EBE-77CDBC562017") {
                        if class.version > 1 {
                            return Err(invalid("unsupported angular dimension version"));
                        }
                        let mut r = payload(records.primary(id)?, id)?;
                        empty(&mut r)?;
                        one(&mut r)?;
                        if r.u32()? != 3 || operand(&mut r)? != 0 {
                            return Err(invalid("unsupported angular dimension operands"));
                        }
                        let a = operand(&mut r)?;
                        let b = operand(&mut r)?;
                        if a == b || !line(&a) || !line(&b) {
                            return Err(invalid("angular dimension requires two distinct sketch lines"));
                        }
                        attributes::read(&mut r)?;
                        let cached_radians = f64::from_bits(r.u64()?);
                        let quadrant = r.u32()?;
                        zeros(&mut r, 2)?;
                        let reference_quadrant = r.u32()?;
                        if !cached_radians.is_finite()
                            || !(0. ..=std::f64::consts::PI).contains(&cached_radians)
                            || quadrant > 3
                            || reference_quadrant > 3
                            || quadrant % 2 != reference_quadrant % 2
                        {
                            return Err(invalid("unsupported angular dimension value or quadrants"));
                        }
                        r.take(219)?; // dimension presentation geometry
                        r.wide_string()?;
                        r.wide_string()?;
                        r.take(24)?; // annotation position
                        if r.u64()? != 0x8000 {
                            return Err(invalid("unsupported angular dimension flags"));
                        }
                        let binding = r.reference()?;
                        let back = references(&mut r)?;
                        if bindings.get(&binding) != Some(&feature) || (back != [a, b] && back != [b, a]) {
                            return Err(invalid("inconsistent angular dimension binding or lines"));
                        }
                        zeros(&mut r, 1)?;
                        finish(&r)?;
                        dimension_owner(&records, id, *geometry)?;
                        constraints.push(SketchConstraint::Angle {
                            id,
                            lines: [a, b],
                            quadrants: [quadrant, reference_quadrant],
                            binding,
                            cached_radians,
                        });
                        continue;
                    }
                    if classes.get(&id).is_some_and(|c| c.id == "8C780195-72C0-4a56-A911-E43AB14357F2") {
                        require_class(&classes, id, "8C780195-72C0-4a56-A911-E43AB14357F2", 3)?;
                        let mut r = payload(records.primary(id)?, id)?;
                        empty(&mut r)?;
                        one(&mut r)?;
                        if r.u32()? != 3 || operand(&mut r)? != 0 {
                            return Err(invalid("unsupported linear dimension operands"));
                        }
                        let a = operand(&mut r)?;
                        let b = operand(&mut r)?;
                        if a == b {
                            return Err(invalid("duplicate dimension points"));
                        }
                        attributes::read(&mut r)?;
                        r.take(220)?;
                        r.wide_string()?; // display prefix, including expression marker
                        r.wide_string()?;
                        r.take(44)?;
                        let orientation = match r.u64()? {
                            0x400000 => LinearOrientation::Horizontal,
                            0x800000 => LinearOrientation::Vertical,
                            0x4000 => LinearOrientation::Aligned,
                            _ => return Err(invalid("unsupported linear dimension orientation or flags")),
                        };
                        let binding = r.reference()?;
                        let back = references(&mut r)?;
                        let matches_points = back == [a, b] || back == [b, a];
                        if bindings.get(&binding) != Some(&feature) || !matches_points {
                            return Err(invalid("inconsistent linear dimension binding or points"));
                        }
                        zeros(&mut r, 1)?;
                        finish(&r)?;
                        dimension_owner(&records, id, *geometry)?;
                        constraints.push(if point(&a) && point(&b) {
                            SketchConstraint::Linear { id, points: [a, b], orientation, binding }
                        } else if orientation == LinearOrientation::Aligned && line(&a) && (point(&b) || line(&b)) {
                            SketchConstraint::Offset { id, line: a, entity: b, binding }
                        } else if orientation == LinearOrientation::Aligned && point(&a) && line(&b) {
                            SketchConstraint::Offset { id, line: b, entity: a, binding }
                        } else {
                            return Err(invalid("unsupported dimension operand types"));
                        });
                        continue;
                    }
                    let radial = classes.get(&id).is_some_and(|c| c.id == "8EDAC24D-35D4-4591-B8E7-B3EA081C9DF0");
                    require_class(
                        &classes,
                        id,
                        if radial { "8EDAC24D-35D4-4591-B8E7-B3EA081C9DF0" } else { "664AD49F-207F-4CD5-8555-DD269E9F09EA" },
                        0,
                    )?;
                    let mut r = payload(records.primary(id)?, id)?;
                    empty(&mut r)?;
                    one(&mut r)?;
                    if r.u32()? != 2 || operand(&mut r)? != 0 {
                        return Err(invalid("unsupported circular dimension operands"));
                    }
                    let circle = operand(&mut r)?;
                    require_class(&classes, circle, "F0130424-8B7E-4092-93C9-1CA807482534", 0)?;
                    if circles.get(&circle) != Some(geometry) {
                        return Err(invalid("dimensioned curve belongs to another sketch"));
                    }
                    attributes::read(&mut r)?;
                    r.take(244)?; // presentation geometry and layout
                    r.wide_string()?; // prefix may include the expression marker (fx:)
                    r.wide_string()?; // displayed value; the binding carries the expression
                    r.take(24)?;
                    if r.u64()? != if radial { 0x20000 } else { 0x10000 } {
                        return Err(invalid("circular dimension flags disagree with its class"));
                    }
                    let binding = r.reference()?;
                    if bindings.get(&binding) != Some(&feature) || references(&mut r)? != [circle] {
                        return Err(invalid("inconsistent circular dimension binding or curve"));
                    }
                    zeros(&mut r, 1)?;
                    finish(&r)?;
                    dimension_owner(&records, id, *geometry)?;
                    constraints.push(if radial {
                        SketchConstraint::Radius { id, circle, binding }
                    } else {
                        SketchConstraint::Diameter { id, circle, binding }
                    });
                }
                Ok(SketchConstraints { geometry: *geometry, feature, constraints })
            }),
        )
    }
}

fn dimension_owner(records: &Records<'_>, id: u64, geometry: u64) -> Result<()> {
    let mut r = payload(records.secondary(id)?, id)?;
    empty(&mut r)?;
    if r.reference()? != geometry {
        return Err(invalid("inconsistent dimension owner"));
    }
    r.take(18)?;
    if r.reference()? != geometry {
        return Err(invalid("inconsistent dimension owner back-reference"));
    }
    finish(&r)
}
