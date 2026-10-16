//! Map native contributor labels to exact reconstructed face families. A native
//! cylinder or chamfer can be split at the kernel's parameter seam.
use super::*;
use solidcraft_doc::{PlaneRef, ProfileSel};
use solidcraft_geom::Seg2;

pub(super) fn tracked_names(
    feature: &solidcraft_doc::Feature,
    source: &ModelBody,
    edge: &recipe_incidence::Edge,
    indices: &BTreeMap<Term, BTreeSet<usize>>,
) -> Result<Vec<String>> {
    let selected = edges::resolve(source, edge, indices)?;
    selected
        .into_iter()
        .map(|selection| {
            if !feature.edge_names.contains(&selection.name) {
                return Err(invalid("fillet output tracker is not one of its selected inputs"));
            }
            Ok(format!("F{}:blend:{}", feature.id, selection.name))
        })
        .collect()
}

pub(super) fn fillet_family(body: &ModelBody, feature: &solidcraft_doc::Feature, selected: &BTreeSet<usize>) -> Result<BTreeSet<usize>> {
    // Adjacent rounds on the same analytic carrier can represent one native
    // face. Every added piece must have verified lineage from this producer.
    let mut allowed = BTreeSet::new();
    for name in &feature.edge_names {
        allowed.extend(solidcraft_doc::naming::face_lineage_indices(body, &format!("F{}:blend:{name}", feature.id)).map_err(|e| invalid(&e))?);
    }
    if selected.is_empty() || !selected.is_subset(&allowed) {
        return Err(invalid("fillet face selection is not owned by its producer"));
    }
    let expanded = solidcraft_kernel::guard("tracked fillet face family", || {
        Ok(solidcraft_kernel::with_same_surface(&body.body, &selected.iter().copied().collect::<Vec<_>>()))
    })?;
    if expanded.iter().any(|face| !allowed.contains(face)) {
        return Err(invalid("fillet face carrier crosses another feature's lineage"));
    }
    Ok(expanded.into_iter().collect())
}

impl Context<'_> {
    fn imported(&self, op: u64) -> Result<(u64, u64)> {
        let native = *self.operations.get(&op).ok_or_else(|| invalid("missing blend face operation"))?;
        let id = *self.features.get(&native).ok_or_else(|| invalid("blend face precedes its producer"))?;
        if let Some(before) = self.before {
            let position = |id| self.doc.features.iter().position(|f| f.id == id);
            if !matches!((position(id), position(before)), (Some(a), Some(b)) if a < b) {
                return Err(invalid("blend face tracking refers to a later feature"));
            }
        }
        if self.doc.feature(id).is_none_or(|f| f.suppressed) {
            return Err(invalid("blend face producer is unavailable"));
        }
        Ok((native, id))
    }

    fn named_faces(&self, body: &ModelBody, id: u64, names: &[String]) -> Result<BTreeSet<usize>> {
        let mut indices = BTreeSet::new();
        for name in names {
            if !name.starts_with(&format!("F{id}:")) {
                return Err(invalid("blend face belongs to another producer"));
            }
            let found = solidcraft_doc::naming::face_lineage_indices(body, name).map_err(|e| invalid(&e))?;
            if found.is_empty() {
                return Err(invalid("blend face lineage is no longer present"));
            }
            indices.extend(found);
        }
        Ok(indices)
    }

    fn chamfer_edges(&self, native: u64, id: u64, label: i32, body: &str) -> Result<&[String]> {
        let class = self.classes.get(&native).ok_or_else(|| invalid("missing chamfer face owner"))?;
        if class.id != super::super::chamfers::CLASS || !matches!(class.version, 2 | 3) {
            return Err(invalid("unsupported blend face producer"));
        }
        let sets =
            super::super::chamfers::inputs(&mut payload(self.records.primary(native)?, native)?, self.records, self.classes, native, class.version)?;
        let [set] = sets.as_slice() else { return Err(invalid("chamfer face requires one output group")) };
        if set.suppressed || label <= 0 || set.output_labels.as_slice() != [label as u64] {
            return Err(invalid("chamfer face is not the group's sole output label"));
        }
        let feature = self.doc.feature(id).ok_or_else(|| invalid("missing imported chamfer"))?;
        if !matches!(&feature.kind, FeatureKind::Chamfer { body: Some(target), groups, .. } if target == body && groups.is_empty())
            || feature.edge_names.is_empty()
            || feature.edge_names.len() > 128
        {
            return Err(invalid("chamfer face has a different body or ambiguous input groups"));
        }
        Ok(&feature.edge_names)
    }

    fn face_profile_curves(&self, native: u64, id: u64, sketch: u64, label: u64, body: &str) -> Result<Vec<String>> {
        let Some(FeatureKind::Extrude { profiles: ProfileSel::All, .. }) = self.doc.feature(id).map(|f| &f.kind) else {
            return Err(invalid("circular face extrusion requires its whole linked profile"));
        };
        let Some(FeatureKind::Sketch { plane: PlaneRef::Face { anchor: Some(anchor), .. }, sketch: linked }) =
            self.doc.feature(sketch).map(|f| &f.kind)
        else {
            return Err(invalid("missing live face-extrusion profile"));
        };
        let [link] = linked.links.as_slice() else { return Err(invalid("ambiguous face-extrusion profile links")) };
        if !link.auto_face || link.lost || link.face_name.as_ref() != Some(&anchor.target.name) || anchor.target.body != body {
            return Err(invalid("face-extrusion profile does not follow its source face"));
        }
        let tag = super::super::side_roles::circle_tag(self.records, self.classes, native, label)?;
        let table = super::super::side_roles::label_table(self.records, self.classes, native)?;
        let tracker = *table.get(&tag).ok_or_else(|| invalid("circular face side has no owned source tracker"))?;
        let tracked = super::super::projections::tracking_identity(self.records, self.classes, tracker, "4B31DA1B-FCD6-4527-BC3E-BE1FBAF867FE", 1)?;
        let identity = recipe_incidence::identity(self.records, self.classes, tracked, false)?;
        let [edge] = identity.edges.as_slice() else { return Err(invalid("missing face-profile carrier")) };
        if edge.ends.len() != 1 || identity.terms.len() != 2 {
            return Err(invalid("face-profile carrier is not one closed trimmed cap boundary"));
        }
        let Some(FaceCertificate::ExtrudeTravelCap { end, .. }) = &anchor.target.certificate else {
            return Err(invalid("face-profile carrier lacks a certified extrusion cap"));
        };
        let mut parent = None;
        let mut modifier = None;
        for term in &edge.faces {
            let contributors: Vec<_> = term.iter().copied().collect();
            let [(op, role)] = contributors.as_slice() else {
                return Err(invalid("ambiguous face-profile source contributor"));
            };
            let (source_native, source_id) = self.imported(*op)?;
            if source_id == anchor.target.origin_feature && *role == if *end { 2 } else { 1 } {
                parent = Some((source_native, source_id));
            } else {
                modifier = Some((source_native, source_id, *role));
            }
        }
        let (parent_native, parent_id) = parent.ok_or_else(|| invalid("face side tracker disagrees with its cap producer"))?;
        let (modifier_native, modifier_id, role) = modifier.ok_or_else(|| invalid("face side tracker lacks its cap modifier"))?;
        let source = self.profiles.get(&parent_native).ok_or_else(|| invalid("missing original circular cap profile"))?;
        let [loops] = source.regions.as_slice() else { return Err(invalid("ambiguous circular source profile")) };
        let [boundary] = loops.as_slice() else { return Err(invalid("circular source has holes")) };
        let [circle] = boundary.curves.as_slice() else { return Err(invalid("source cap is not one circle")) };
        if boundary.pieces.as_slice() != [(1, 1)] || !self.circles.contains_key(circle) {
            return Err(invalid("source cap requires a complete circle"));
        }
        let Some(FeatureKind::Extrude { extent, .. }) = self.doc.feature(parent_id).map(|f| &f.kind) else {
            return Err(invalid("source cap producer is not an extrusion"));
        };
        let cap = if !end {
            "start"
        } else if self.doc.eval(&extent.distance, solidcraft_doc::expr::Kind::Length)? < 0. {
            "bottom"
        } else {
            "top"
        };
        let expected = BTreeSet::from([format!("F{parent_id}:{cap}"), format!("F{parent_id}:side:c{circle}")]);
        if self
            .chamfer_edges(modifier_native, modifier_id, role, body)?
            .iter()
            .any(|name| solidcraft_doc::naming::strip_edge_piece(name).split('|').map(str::to_owned).collect::<BTreeSet<_>>() != expected)
        {
            return Err(invalid("face side tracker refers to another chamfer boundary"));
        }
        let solved = self.state.sketch(sketch).ok_or_else(|| invalid("missing solved face profile"))?;
        let [profile] = solved.profiles.as_slice() else { return Err(invalid("face side requires one reconstructed circular profile")) };
        let segments = &profile.region.outer.segs;
        let Some(Seg2::Arc { center, radius, .. }) = segments.first() else { return Err(invalid("face side profile is not circular")) };
        if !profile.region.holes.is_empty() || !(1..=64).contains(&segments.len()) || profile.outer_curves.len() > 64 {
            return Err(invalid("unsupported face side profile boundary"));
        }
        let mut sweep = 0.;
        for segment in segments {
            let Seg2::Arc { center: c, radius: r, sweep: angle, .. } = segment else {
                return Err(invalid("face side profile contains noncircular segments"));
            };
            if c.dist(*center) > 1e-7 || (*r - radius).abs() > 1e-7 || !angle.is_finite() || *angle <= 0. {
                return Err(invalid("face side profile contains different circular carriers"));
            }
            sweep += angle;
        }
        if (sweep - std::f64::consts::TAU).abs() > 1e-7 {
            return Err(invalid("face side profile is not a complete circle"));
        }
        Ok(profile.outer_curves.clone())
    }

    fn tracked_blend_faces(&self, native: u64, id: u64, label: i32, creator: u64, body: &ModelBody) -> Result<BTreeSet<usize>> {
        if self.depth >= 16 || label <= 0 {
            return Err(invalid("invalid or excessively nested fillet face tracking"));
        }
        let feature = self.doc.feature(id).ok_or_else(|| invalid("missing fillet face producer"))?;
        let chamfer = matches!(&feature.kind, FeatureKind::Chamfer { body: Some(target), .. } if target == &body.name);
        if chamfer {
            let class = self.classes.get(&native).ok_or_else(|| invalid("missing tracked chamfer owner"))?;
            if class.id != super::super::chamfers::CLASS || !matches!(class.version, 2 | 3) {
                return Err(invalid("unsupported tracked chamfer owner"));
            }
            let sets = super::super::chamfers::inputs(
                &mut payload(self.records.primary(native)?, native)?,
                self.records,
                self.classes,
                native,
                class.version,
            )?;
            if sets.iter().filter(|set| !set.suppressed && set.output_labels.contains(&(label as u64))).count() != 1 {
                return Err(invalid("tracked chamfer face has no unique active output group"));
            }
        } else if !matches!(&feature.kind, FeatureKind::Fillet { body: Some(target), style: solidcraft_doc::FilletStyle::Constant, .. } if target == &body.name)
        {
            return Err(invalid("fillet face has another body or unsupported radius groups"));
        }
        let table = super::super::side_roles::label_table(self.records, self.classes, native)?;
        let tracker = *table.get(&(label as u64)).ok_or_else(|| invalid("missing fillet output face label"))?;
        let tracked = super::super::projections::tracking_identity(self.records, self.classes, tracker, "4B31DA1B-FCD6-4527-BC3E-BE1FBAF867FE", 1)?;
        let (owners, mut retired) = self.records.design.operation_index()?;
        retired.retain(|operation| !owners.contains_key(operation));
        let identity = recipe_incidence::tracked_identity(self.records, self.classes, tracked, &retired)?;
        let [edge] = identity.edges.as_slice() else { return Err(invalid("fillet output requires one tracked source edge")) };
        if !identity.retired_modifiers.is_empty() {
            if chamfer {
                return Err(invalid("retired chamfer output qualifiers are not supported"));
            }
            // Historical qualifiers can only disappear when the producer's
            // explicit current selection independently contains the exact same
            // complete edge/endpoint graph. The retired map alone is insufficient.
            let class = self.classes.get(&native).ok_or_else(|| invalid("missing fillet tracking owner"))?;
            let sets =
                super::super::fillets::inputs(&mut payload(self.records.primary(native)?, native)?, self.records, self.classes, class.version)?;
            let mut matches = 0;
            for set in sets {
                if set.face_container.is_some() {
                    return Err(invalid("retired fillet face selection is not yet certified"));
                }
                let container = set.container.ok_or_else(|| invalid("missing fillet tracking selection"))?;
                for selector in super::super::loft_faces::objects(self.records, self.classes, container, native)? {
                    let (_, current) = decode(self.records, self.classes, selector, native)?;
                    if current.edges == identity.edges {
                        matches += 1;
                    }
                }
            }
            if matches != 1 {
                return Err(invalid("retired fillet tracker has no unique current input certificate"));
            }
        }
        if self.model.result(id).is_none_or(|result| result.error.is_some() || result.skipped || result.stale) {
            return Err(invalid("fillet face producer has no valid evaluated history"));
        }
        let before = self.model.state_before(id);
        let source_body = before.body(&body.name).ok_or_else(|| invalid("fillet source body is unavailable"))?;
        let source = Context { state: &before, before: Some(id), depth: self.depth + 1, ..*self };
        let mut indices = BTreeMap::new();
        for term in &identity.terms {
            if term.iter().all(|(_, label)| *label < 0) {
                for (operation, _) in term {
                    source.producer(*operation, creator, &body.name)?;
                    if !edge.ends.iter().any(|v| v.seams.contains(term) && v.faces.iter().any(|f| f.iter().any(|(owner, _)| owner == operation))) {
                        return Err(invalid("tracked blend seam is not incident to its producer"));
                    }
                }
                continue;
            }
            indices.insert(term.clone(), source.face_indices(term, creator, source_body)?);
        }
        let names = tracked_names(feature, source_body, edge, &indices)?;
        let selected = self.named_faces(body, id, &names)?;
        fillet_family(body, feature, &selected)
    }

    pub(super) fn face_indices(&self, term: &Term, creator: u64, body: &ModelBody) -> Result<BTreeSet<usize>> {
        let mut roles = BTreeMap::new();
        for (op, label) in term {
            let (_, id) = self.imported(*op)?;
            if matches!(label, 1 | 2) && matches!(self.doc.feature(id).map(|f| &f.kind), Some(FeatureKind::Extrude { .. })) {
                self.producer(*op, creator, &body.name)?;
                if roles.insert(id, *label == 2).is_some() {
                    return Err(invalid("duplicate blend cap producer"));
                }
            }
        }
        if roles.len() == term.len() {
            return super::super::loft_faces::joined_cap_indices(self.doc, self.state, creator, &roles).or_else(|error| {
                let mut roles = roles.iter();
                let Some((&producer, &end)) = roles.next() else { return Err(error) };
                if roles.next().is_some() {
                    return Err(error);
                }
                // Trimming can replace a history alias while retaining the
                // exact extrusion certificate. That certificate, never plane
                // proximity, identifies a single surviving cap.
                let anchor = super::super::loft_faces::Face::cap(0, 0, end, false).anchor(self.doc, self.state, creator, &[(producer, None)])?;
                Ok(self.doc.configuration_face_indices(body, &anchor.target)?.into_iter().collect())
            });
        }
        let contributors: Vec<_> = term.iter().copied().collect();
        let [(op, label)] = contributors.as_slice() else { return self.cylindrical_family(term, creator, body) };
        let (native, id) = self.imported(*op)?;
        if matches!(self.doc.feature(id).map(|f| &f.kind), Some(FeatureKind::Fillet { .. })) {
            return self.tracked_blend_faces(native, id, *label, creator, body);
        }
        if matches!(self.doc.feature(id).map(|f| &f.kind), Some(FeatureKind::Chamfer { .. })) {
            // A sole output label identifies the whole group. Multiple outputs
            // require the explicit output-to-input tracker, never group order.
            if let Ok(edges) = self.chamfer_edges(native, id, *label, &body.name) {
                let names = edges.iter().map(|edge| format!("F{id}:blend:{edge}")).collect::<Vec<_>>();
                return self.named_faces(body, id, &names);
            }
            return self.tracked_blend_faces(native, id, *label, creator, body);
        }
        if *label <= 2 {
            return Err(invalid("invalid blend side label"));
        }
        let (_, _, sketch) = self.producer(*op, creator, &body.name)?;
        let curves = if let Some(profile) = self.profiles.get(&native) {
            let solved = self.state.sketch(sketch).ok_or_else(|| invalid("missing blend source sketch"))?;
            let source = || super::super::blend_sides::Source { profile, lines: self.lines, circles: self.circles, solved };
            let curve = super::super::blend_sides::circle(self.records, self.classes, native, *label as u64, source())
                .or_else(|_| super::super::blend_sides::curve(self.records, self.classes, native, *label as u64, source()))?;
            vec![format!("c{curve}")]
        } else {
            self.face_profile_curves(native, id, sketch, *label as u64, &body.name)?
        };
        self.named_faces(body, id, &curves.iter().map(|curve| format!("F{id}:side:{curve}")).collect::<Vec<_>>())
    }

    fn cylindrical_family(&self, term: &Term, creator: u64, body: &ModelBody) -> Result<BTreeSet<usize>> {
        use solidcraft_doc::sketch::CurveKind;
        if !(2..=64).contains(&term.len()) {
            return Err(invalid("invalid composite cylindrical contributor count"));
        }
        let tolerance = 1e-7 * (1. + body.body.size());
        if !tolerance.is_finite() || tolerance <= 0. {
            return Err(invalid("invalid composite cylindrical scale"));
        }
        let mut carrier: Option<(Vec3, Vec3, f64)> = None;
        let mut faces = BTreeSet::new();
        for (op, label) in term {
            if *label <= 2 {
                return Err(invalid("composite cylindrical contributor is not a side"));
            }
            let (native, id, sketch) = self.producer(*op, creator, &body.name)?;
            let profile = self.profiles.get(&native).ok_or_else(|| invalid("missing composite cylindrical profile"))?;
            let solved = self.state.sketch(sketch).ok_or_else(|| invalid("missing composite cylindrical sketch"))?;
            let source = || super::super::blend_sides::Source { profile, lines: self.lines, circles: self.circles, solved };
            let curve = super::super::blend_sides::circle(self.records, self.classes, native, *label as u64, source())
                .or_else(|_| super::super::blend_sides::curve(self.records, self.classes, native, *label as u64, source()))?;
            let name = format!("c{curve}");
            let index = solved.sketch.curve_index(&name).ok_or_else(|| invalid("missing composite cylindrical curve"))?;
            let Some(CurveKind::Circle { c, r }) = solved.sketch.curves.get(index).map(|curve| &curve.kind) else {
                return Err(invalid("composite side requires circular source curves"));
            };
            let center = solved.sketch.point(*c).ok_or_else(|| invalid("missing composite circle center"))?;
            let origin = solved.plane.to_world(center);
            let axis = solved.plane.normal();
            if !origin.is_finite() || !axis.is_finite() || !r.is_finite() || *r <= tolerance {
                return Err(invalid("invalid composite cylindrical carrier"));
            }
            if let Some((point, direction, radius)) = carrier {
                if direction.cross(axis).len() > 1e-9 || (point - origin).cross(axis).len() > tolerance || (radius - r).abs() > tolerance {
                    return Err(invalid("composite side contributors have different cylindrical carriers"));
                }
            } else {
                carrier = Some((origin, axis, *r));
            }
            // Geometry certifies an explicitly owned family; it never searches
            // unrelated faces for a matching cylinder or discards a contributor.
            let selected = solidcraft_doc::naming::face_family_indices(body, &format!("F{id}:side:{name}")).map_err(|e| invalid(&e))?;
            solidcraft_kernel::guard("composite cylindrical face certificate", || {
                for index in &selected {
                    let Some(solidcraft_kernel::FaceSurface::Cylinder { origin: point, axis: direction, radius }) =
                        solidcraft_kernel::face_surface(&body.body, *index)
                    else {
                        return Err(solidcraft_kernel::KernelError::Invalid("composite side lineage is not cylindrical".into()));
                    };
                    if !point.is_finite()
                        || !direction.is_finite()
                        || !radius.is_finite()
                        || radius <= tolerance
                        || direction.cross(axis).len() > 1e-7
                        || (point - origin).cross(axis).len() > tolerance
                        || (radius - r).abs() > tolerance
                    {
                        return Err(solidcraft_kernel::KernelError::Invalid("composite side lineage disagrees with its source cylinder".into()));
                    }
                }
                Ok(())
            })?;
            faces.extend(selected);
        }
        Ok(faces)
    }
}
