//! Native topology-operation ownership, verified with top/bottom edge and
//! overlapping independent-body controls. Tags are not bulk record identities.
use super::history::{empty, finish, payload, require_class, zeros};
use super::*;

const CLASS: &str = "5D89B935-3F04-4CAB-81F5-E7B309E7E13F";

impl Design {
    /// Map kernel operation tags to native feature records. Populated native
    /// indexes must agree; no ownership is inferred from geometry or order.
    pub fn operation_owners(&self) -> Result<BTreeMap<u64, u64>> {
        self.operation_index().map(|(owners, _)| owners)
    }

    pub(super) fn operation_index(&self) -> Result<(BTreeMap<u64, u64>, BTreeSet<u64>)> {
        let class = self.classes.iter().find(|c| c.id == CLASS).ok_or_else(|| invalid("missing topology operation map"))?;
        if !matches!(class.version, 0 | 1) || class.objects.len() != 1 {
            return Err(invalid("unsupported topology operation map version or count"));
        }
        let id = class.objects.first().copied().ok_or_else(|| invalid("missing operation map identity"))?;
        let records = Records::new(self);
        let classes: BTreeMap<_, _> = self.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        let mut r = payload(records.primary(id)?, id)?;
        empty(&mut r)?;
        zeros(&mut r, 2)?;
        let mut features = BTreeSet::new();
        let mut owners = BTreeMap::new();
        let mut pattern_instances = BTreeMap::new();
        if self.classes.iter().any(|c| c.id == super::circular_patterns::CLASS) {
            for entry in self.timeline()?.into_iter().filter(|e| e.class == super::circular_patterns::CLASS) {
                let HistoryKind::CircularPattern(pattern) = entry.feature?.kind else { return Err(invalid("missing circular-pattern ownership")) };
                for (tag, instance) in pattern.operation_instances {
                    if pattern_instances.insert(tag, instance).is_some() {
                        return Err(invalid("duplicate circular-pattern operation owner"));
                    }
                }
            }
        }
        let forward_count = if class.version == 1 { r.count(12)? } else { 0 };
        for _ in 0..forward_count {
            let feature = r.u64()?;
            if !features.insert(feature) {
                return Err(invalid("missing or duplicate topology operation feature"));
            }
            for _ in 0..r.count(8)? {
                let tag = r.u64()?;
                if tag == 0 || feature == 0 || owners.insert(tag, feature).is_some() {
                    return Err(invalid("duplicate or empty topology operation tag"));
                }
            }
        }
        let mut reverse = BTreeMap::new();
        for _ in 0..r.count(16)? {
            let tag = r.u64()?;
            let feature = r.u64()?;
            if tag == 0 || feature == 0 || reverse.insert(tag, feature).is_some() {
                return Err(invalid("invalid reverse topology operation ownership"));
            }
        }
        // Version 0 has only the authoritative tag-to-feature table. An
        // unchanged archive saved by newer Fusion adds an empty forward index.
        // When that optional index is populated, both directions must agree.
        if forward_count != 0 && reverse != owners {
            return Err(invalid("inconsistent topology operation ownership"));
        }
        for (tag, instance) in &pattern_instances {
            if let Some(instance) = instance
                && reverse.get(tag) != Some(instance)
            {
                return Err(invalid("circular-pattern operation disagrees with its current instance"));
            }
        }
        let mut retired = BTreeSet::new();
        for _ in 0..r.count(8)? {
            let tag = r.u64()?;
            // Recreating an instance can reuse its stable operation slot even
            // while that slot remains in the retired list. Only the certified
            // pattern's current instance relationship can authorize that overlap.
            let pattern_slot = pattern_instances.get(&tag).is_some_and(|instance| match instance {
                Some(instance) => reverse.get(&tag) == Some(instance),
                None => reverse.get(&tag).is_none_or(|owner| !classes.contains_key(owner)),
            });
            let absent_owner = reverse.get(&tag).is_some_and(|owner| !classes.contains_key(owner));
            if tag == 0 || (reverse.contains_key(&tag) && !pattern_slot && !absent_owner) || !retired.insert(tag) {
                return Err(invalid("invalid retired topology operation tag"));
            }
        }
        let owner = r.reference()?;
        require_class(&classes, owner, "C3423E04-7E85-4B44-8E31-A5FA9C100052", 2)?;
        finish(&r)?;
        // These tables also retain operations of deleted features and former
        // pattern instances. Validate both indexes before removing their
        // unavailable owners. A selector that still needs one cannot resolve it
        // from this map; it must not inherit another feature's operation.
        reverse.retain(|_, owner| classes.contains_key(owner));
        Ok((reverse, retired))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, fixture};
    use crate::f3d::relationship_tests::{reference, u32, u64};

    fn mapped() -> Fixture {
        let mut f = fixture(false);
        let mut b = vec![0; 6];
        u32(&mut b, 2);
        for (feature, tags) in [(9, vec![8001, 8002]), (8, vec![8000])] {
            u64(&mut b, feature);
            u32(&mut b, tags.len() as u32);
            for tag in tags {
                u64(&mut b, tag);
            }
        }
        u32(&mut b, 3);
        for (tag, feature) in [(8000, 8), (8001, 9), (8002, 9)] {
            u64(&mut b, tag);
            u64(&mut b, feature);
        }
        u32(&mut b, 0);
        reference(&mut b, 4);
        f.put(180, CLASS, 1, b, None);
        f
    }

    #[test]
    fn operation_tags_are_owned_by_features_not_records_with_similar_ids() {
        assert_eq!(mapped().design().operation_owners().unwrap(), BTreeMap::from([(8000, 8), (8001, 9), (8002, 9)]));
    }

    #[test]
    fn absent_historical_owners_are_not_live_selection_candidates() {
        for version in [0, 1] {
            for retired in [false, true] {
                let mut f = mapped();
                let mut b = vec![0; 6];
                if version == 1 {
                    u32(&mut b, 2);
                    for (feature, tag) in [(9, 8001), (999, 8999)] {
                        u64(&mut b, feature);
                        u32(&mut b, 1);
                        u64(&mut b, tag);
                    }
                }
                u32(&mut b, 2);
                for (tag, feature) in [(8001, 9), (8999, 999)] {
                    u64(&mut b, tag);
                    u64(&mut b, feature);
                }
                u32(&mut b, u32::from(retired));
                if retired {
                    u64(&mut b, 8999);
                }
                reference(&mut b, 4);
                f.put(180, CLASS, version, b, None);
                assert_eq!(Fixture(f.0.clone()).design().operation_owners().unwrap(), BTreeMap::from([(8001, 9)]));
                // Reintroducing an actual owner keeps its live operation unless
                // the retired index contradicts that relationship.
                f.put(999, "original-feature", 0, vec![], None);
                let result = f.design().operation_owners();
                if retired {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().get(&8999), Some(&999));
                }
            }
        }
    }

    #[test]
    #[ignore = "requires local Style 7 archive"]
    fn native_style7_ignores_only_unavailable_operation_owners() {
        let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap());
        let designs = read_f3d(&std::fs::read(root.join("knobs/Style 7.f3d")).unwrap()).unwrap();
        let owners = designs[0].operation_owners().unwrap();
        assert_eq!(owners.get(&301), Some(&216));
        assert_eq!(owners.get(&450), Some(&76352));
        assert!(!owners.contains_key(&330));
        assert!(!owners.contains_key(&423));
    }

    fn retired(version: u32, forward: bool, tags: &[u64]) -> Fixture {
        let mut f = mapped();
        let record = f.0.get_mut(&180).unwrap();
        record.1 = version;
        if version == 0 {
            record.2.drain(6..58);
        } else if !forward {
            record.2[6..10].copy_from_slice(&0_u32.to_le_bytes());
            record.2.drain(10..58);
        }
        let mut list = Vec::new();
        u32(&mut list, tags.len() as u32);
        for tag in tags {
            u64(&mut list, *tag);
        }
        let at = record.2.len() - 15;
        record.2.splice(at..at + 4, list);
        f
    }

    #[test]
    fn legacy_maps_optional_indexes_and_retired_operations_preserve_live_ownership() {
        for (version, forward) in [(0, false), (1, false), (1, true)] {
            assert_eq!(retired(version, forward, &[8999, 9000]).design().operation_owners().unwrap(), mapped().design().operation_owners().unwrap());
            for tags in [vec![0], vec![8001], vec![8999, 8999]] {
                assert!(retired(version, forward, &tags).design().operation_owners().is_err(), "{version}/{forward}/{tags:?}");
            }
            for keep in 0..retired(version, forward, &[8999]).0[&180].2.len() {
                let mut f = retired(version, forward, &[8999]);
                f.0.get_mut(&180).unwrap().2.truncate(keep);
                assert!(f.design().operation_owners().is_err(), "{version}/{forward}/{keep}");
            }
        }
    }

    #[test]
    fn damaged_operation_maps_fail_without_partial_ownership() {
        let n = mapped().0[&180].2.len();
        for keep in 0..n {
            let mut f = mapped();
            f.0.get_mut(&180).unwrap().2.truncate(keep);
            assert!(f.design().operation_owners().is_err(), "prefix {keep}");
        }
        // Missing/duplicate feature, duplicate/zero tag, reverse disagreement,
        // duplicate reverse tag and wrong owner class.
        for (at, value) in [(10, 999), (38, 9), (30, 8001), (22, 0), (70, 9), (78, 8000), (115, 9)] {
            let mut f = mapped();
            f.0.get_mut(&180).unwrap().2[at..at + 8].copy_from_slice(&u64::to_le_bytes(value));
            assert!(f.design().operation_owners().is_err(), "offset {at}");
        }
        let mut f = mapped();
        f.0.get_mut(&180).unwrap().1 = 0;
        assert!(f.design().operation_owners().is_err());
    }
}
