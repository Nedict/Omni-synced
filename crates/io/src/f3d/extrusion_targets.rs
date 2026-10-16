//! Explicit native Boolean participants: body handles and operation labels own
//! the target, independently of how many bodies happen to be active.
use super::*;
use solidcraft_doc::{Document, FeatureKind, Operation};

pub(super) struct Context<'a> {
    pub records: &'a Records<'a>,
    pub classes: &'a BTreeMap<u64, &'a Class>,
    pub document: &'a Document,
    pub feature_ids: &'a BTreeMap<u64, u64>,
    pub owners: &'a BTreeMap<u64, u64>,
    pub active: &'a [String],
}

impl Context<'_> {
    pub fn single(&self, containers: &[u64], owner: u64) -> Result<String> {
        let [container] = containers else {
            return Err(invalid("extrusion requires one explicit target body"));
        };
        let target = super::loft_faces::target(self.records, self.classes, *container, owner)?;
        let source = self
            .feature_ids
            .get(&target.feature)
            .and_then(|id| self.document.feature(*id))
            .filter(|f| !f.suppressed)
            .ok_or_else(|| invalid("extrusion target precedes its producer or the producer is suppressed"))?;
        let [body] = source.body_names.as_slice() else {
            return Err(invalid("extrusion target must identify one body"));
        };
        if !self.active.contains(body) {
            return Err(invalid("extrusion target body is no longer active"));
        }
        let mut producers = BTreeSet::new();
        for (index, operation) in target.operations.iter().enumerate() {
            let producer = *self.owners.get(&operation.operation).ok_or_else(|| invalid("missing extrusion target operation owner"))?;
            let feature = self
                .feature_ids
                .get(&producer)
                .and_then(|id| self.document.feature(*id))
                .filter(|f| !f.suppressed)
                .ok_or_else(|| invalid("extrusion target operation follows its selector or is suppressed"))?;
            let compatible = match (&feature.kind, operation.flags) {
                (FeatureKind::Extrude { operation: Operation::Join | Operation::Cut, targets, .. }, 4)
                | (FeatureKind::Loft { operation: Operation::Join, targets, .. }, 128 | 129) => targets.as_slice() == [body.clone()],
                _ => false,
            };
            if !producers.insert(producer) || (index == 0 && producer != target.feature) || (index != 0 && !compatible) {
                return Err(invalid("extrusion target operations disagree with their body ownership"));
            }
            super::body_labels::validate(self.records, self.classes, producer, operation.operation, u64::from(operation.label))?;
        }
        Ok(body.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, fixture};
    use crate::f3d::relationship_tests::refs;

    fn authored() -> Fixture {
        let mut f = super::super::body_labels::tests::authored(false);
        let mut selectors = super::super::loft_faces::tests::authored(true);
        for id in [80, 81, 190, 191, 192] {
            f.0.insert(id, selectors.0.remove(&id).unwrap());
        }
        // The selector uses the original fixture's arbitrary whole-body label.
        f.0.get_mut(&191).unwrap().2[30..34].copy_from_slice(&208_u32.to_le_bytes());
        let mut output = Vec::new();
        refs(&mut output, &[80]);
        f.0.get_mut(&9).unwrap().2.splice(151..155, output);
        f
    }

    #[test]
    fn explicit_target_ignores_unrelated_bodies_but_requires_live_owned_history() {
        let mut doc = fixture(false).design().to_document("Explicit target").unwrap();
        let producer = doc.features[1].id;
        let body = doc.features[1].body_names[0].clone();
        let other = doc.add_feature(doc.features[1].kind.clone(), Some("Other solid")).unwrap();
        doc.feature_mut(other).unwrap().body_names = vec!["Unrelated".into()];
        let d = authored().design();
        let records = Records::new(&d);
        let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let ids = BTreeMap::from([(9, producer)]);
        let owners = BTreeMap::from([(8001, 9)]);
        let active = vec!["Unrelated".into(), body.clone()];
        let resolve = |doc: &Document, ids: &BTreeMap<u64, u64>, owners: &BTreeMap<u64, u64>, active: &[String], containers: &[u64], owner: u64| {
            Context { records: &records, classes: &classes, document: doc, feature_ids: ids, owners, active }.single(containers, owner)
        };
        assert_eq!(resolve(&doc, &ids, &owners, &active, &[192], 19).unwrap(), body);
        assert!(resolve(&doc, &ids, &owners, &active, &[192], 20).is_err(), "foreign selection owner");
        assert!(resolve(&doc, &ids, &owners, &active, &[192, 192], 19).is_err(), "duplicate target");
        assert!(resolve(&doc, &ids, &owners, &active[..1], &[192], 19).is_err(), "consumed body");
        assert!(resolve(&doc, &BTreeMap::new(), &owners, &active, &[192], 19).is_err(), "future producer");
        assert!(resolve(&doc, &ids, &BTreeMap::from([(8001, 99)]), &active, &[192], 19).is_err(), "foreign operation");
        doc.feature_mut(producer).unwrap().suppressed = true;
        assert!(resolve(&doc, &ids, &owners, &active, &[192], 19).is_err(), "suppressed producer");
    }

    #[test]
    fn explicit_target_rejects_label_substitution_and_truncated_references() {
        let doc = fixture(false).design().to_document("Explicit target").unwrap();
        let producer = doc.features[1].id;
        let ids = BTreeMap::from([(9, producer)]);
        let owners = BTreeMap::from([(8001, 9)]);
        let active = doc.features[1].body_names.clone();
        let check = |f: Fixture| {
            let d = f.design();
            let records = Records::new(&d);
            let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
            Context { records: &records, classes: &classes, document: &doc, feature_ids: &ids, owners: &owners, active: &active }.single(&[192], 19)
        };
        let mut wrong = authored();
        wrong.0.get_mut(&191).unwrap().2[30..34].copy_from_slice(&209_u32.to_le_bytes());
        assert!(check(wrong).is_err());
        for id in [80, 190, 191, 192, 901, 902] {
            for keep in 0..authored().0[&id].2.len() {
                let mut f = authored();
                f.0.get_mut(&id).unwrap().2.truncate(keep);
                assert!(check(f).is_err(), "record {id}, prefix {keep}");
            }
        }
    }
}
