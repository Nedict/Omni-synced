//! Native chamfer sets verified by controlled distance, angle, flip and chain edits.
use super::history::{empty, finish, payload, references, require_class, zeros};
use super::*;

pub(super) const CLASS: &str = "F757D611-217B-4B72-9C41-B617BDCE43DA";
const SET: &str = "0D383DB2-A6E5-48C7-82D5-ED3DD0C275C5";
const BINDING: &str = "D91D429C-1BB5-4640-B49F-69AF14F463E9";

#[derive(Debug, PartialEq)]
pub struct ChamferSet {
    /// Native output face labels explicitly assigned to this input set.
    pub output_labels: Vec<u64>,
    pub container: u64,
    pub distance: u64,
    pub distance2: Option<u64>,
    pub angle: Option<u64>,
    pub flip: bool,
    pub tangent_chain: bool,
    pub suppressed: bool,
}

fn flag(r: &mut Reader<'_>) -> Result<bool> {
    match r.take(1)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(invalid("invalid chamfer boolean")),
    }
}
fn optional(r: &mut Reader<'_>, classes: &BTreeMap<u64, &Class>) -> Result<Option<u64>> {
    if r.bytes.get(r.at) == Some(&0) {
        r.take(1)?;
        Ok(None)
    } else {
        let id = r.reference()?;
        require_class(classes, id, BINDING, 0)?;
        Ok(Some(id))
    }
}

pub(super) fn inputs(
    r: &mut Reader<'_>,
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    owner: u64,
    version: u32,
) -> Result<Vec<ChamferSet>> {
    empty(r)?;
    zeros(r, 1)?;
    if !super::attributes::read(r)?.is_empty() || r.u32()? != 1 {
        return Err(invalid("unsupported chamfer attributes or mode"));
    }
    if version == 3 {
        zeros(r, 1)?;
    }
    if !flag(r)? {
        return Err(invalid("unsupported chamfer edge table"));
    }
    // Output topology labels are not input edge selectors. Retain structural
    // validation without treating a label as an ordinal in regenerated geometry.
    let count = r.count(10)?;
    if count == 0 || count > 1000 {
        return Err(invalid("invalid chamfer label count"));
    }
    let mut labels = BTreeSet::new();
    let mut group_labels = Vec::new();
    let mut output_labels = Vec::new();
    for _ in 0..count {
        let label = r.u64()?;
        if label == 0 || !labels.insert(label) {
            return Err(invalid("duplicate or empty chamfer label"));
        }
        let [a, b] = r.take(2)? else {
            return Err(invalid("missing chamfer group label"));
        };
        let group = u16::from_le_bytes([*a, *b]);
        group_labels.push(usize::from(group));
        output_labels.push((label, usize::from(group)));
    }
    let helpers = references(r)?;
    if !(1..=64).contains(&helpers.len())
        || helpers.iter().collect::<BTreeSet<_>>().len() != helpers.len()
        || group_labels.iter().any(|g| *g == 0 || *g > helpers.len())
    {
        return Err(invalid("invalid chamfer edge-set table"));
    }
    let containers = references(r)?;
    let mut sets = Vec::new();
    for (index, helper) in helpers.into_iter().enumerate() {
        require_class(classes, helper, SET, 2)?;
        let mut h = payload(records.primary(helper)?, helper)?;
        zeros(&mut h, 6)?;
        let tangent_chain = flag(&mut h)?;
        let flip = flag(&mut h)?;
        let angle = optional(&mut h, classes)?;
        let distance = h.reference()?;
        require_class(classes, distance, BINDING, 0)?;
        let distance2 = optional(&mut h, classes)?;
        let mode = h.u32()?;
        if !matches!((mode, angle.is_some(), distance2.is_some()), (1, false, false) | (2, false, true) | (3, true, false)) || (mode == 1 && flip) {
            return Err(invalid("inconsistent chamfer mode and dimensions"));
        }
        let container = h.reference()?;
        require_class(classes, container, "2CA5A1CD-C99B-4C9B-91AF-57989148E841", 1)?;
        zeros(&mut h, 2)?;
        if h.u32()? as usize != index + 1 {
            return Err(invalid("invalid chamfer group order"));
        }
        // The legacy v2 feature's group tail has 25 bytes: suppression, a
        // reserved byte, the group reference, a reserved byte and the owner.
        // A modern resave adds a true flag before that tail (26 bytes), without
        // changing the group's class version. Accept only these exact envelopes.
        let legacy_tail = version == 2 && h.bytes.len().saturating_sub(h.at) == 25;
        if !legacy_tail && !flag(&mut h)? {
            return Err(invalid("invalid chamfer group flag"));
        }
        let suppressed = flag(&mut h)?;
        zeros(&mut h, 1)?;
        require_class(classes, h.reference()?, "6CCF41D5-40BE-48ED-A834-18F3EAED6C57", 1)?;
        zeros(&mut h, 1)?;
        if h.reference()? != owner {
            return Err(invalid("chamfer group belongs to another feature"));
        }
        finish(&h)?;
        let selected_labels = output_labels.iter().filter_map(|(label, group)| (*group == index + 1).then_some(*label)).collect();
        sets.push(ChamferSet { output_labels: selected_labels, container, distance, distance2, angle, flip, tangent_chain, suppressed });
    }
    if containers != sets.iter().map(|s| s.container).collect::<Vec<_>>() || containers.iter().collect::<BTreeSet<_>>().len() != containers.len() {
        return Err(invalid("chamfer feature and group selections disagree"));
    }
    Ok(sets)
}

// Cyclic incidence at the two endpoints distinguishes the primary setback face.
// A face-label sort or geometric edge-cache point loses this orientation.
pub(super) fn primary_face(edge: &super::fillets::Edge) -> Result<u32> {
    if edge.closed {
        return edge.primary_face.filter(|label| edge.faces.contains(label)).ok_or_else(|| invalid("missing circular chamfer primary face"));
    }
    let forward = |cycle: &[u32; 3]| -> Result<bool> {
        let zero = cycle.iter().position(|n| *n == 0).ok_or_else(|| invalid("missing chamfer incident face"))?;
        Ok(cycle.get((zero + 1) % 3) == Some(&1))
    };
    let [a, b] = &edge.endpoint_cycles;
    let first = forward(a)?;
    if first == forward(b)? {
        return Err(invalid("inconsistent chamfer endpoint orientation"));
    }
    edge.faces.get(usize::from(first)).copied().ok_or_else(|| invalid("missing primary chamfer face"))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::f3d::import_tests::{Fixture, common, fixture};
    use crate::f3d::relationship_tests::{reference, refs, u32, u64};

    #[test]
    fn endpoint_cycles_preserve_primary_face_through_rotations_and_reject_inconsistent_orientation() {
        for (cycle, expected) in [([0, 1, 2], 42), ([1, 2, 0], 42), ([2, 0, 1], 42), ([0, 2, 1], 19), ([2, 1, 0], 19), ([1, 0, 2], 19)] {
            let mut other = cycle.map(|n| if n == 2 { 3 } else { n });
            other.swap(0, 1);
            for rotation in 0..3 {
                let mut second = other;
                second.rotate_left(rotation);
                let mut edge = super::super::fillets::Edge {
                    feature: 9,
                    operation: 8001,
                    faces: [19, 42],
                    labels: vec![19, 42, 71, 95],
                    endpoint_cycles: [cycle, second],
                    closed: false,
                    primary_face: None,
                };
                assert_eq!(primary_face(&edge).unwrap(), expected);
                edge.endpoint_cycles[1].swap(0, 1);
                assert!(primary_face(&edge).is_err());
            }
        }
    }

    pub(in crate::f3d) fn authored(version: u32, mode: u32, flip: bool, chain: bool, suppressed: bool) -> Fixture {
        let mut f = fixture(false);
        let mut timeline = vec![0; 6];
        reference(&mut timeline, 4);
        refs(&mut timeline, &[8, 9, 19]);
        f.0.get_mut(&5).unwrap().2 = timeline;
        let mut feature = vec![0; 6];
        u32(&mut feature, 1);
        if version == 3 {
            feature.push(0);
        }
        feature.push(1);
        u32(&mut feature, 1);
        u64(&mut feature, 71);
        feature.extend([1, 0]);
        refs(&mut feature, &[900]);
        refs(&mut feature, &[901]);
        common(&mut feature, "Chamfer", suppressed);
        f.put(19, CLASS, version, feature, None);
        let mut set = vec![0; 6];
        set.extend([u8::from(chain), u8::from(flip)]);
        if mode == 3 {
            reference(&mut set, 904);
        } else {
            set.push(0);
        }
        reference(&mut set, 902);
        if mode == 2 {
            reference(&mut set, 903);
        } else {
            set.push(0);
        }
        u32(&mut set, mode);
        reference(&mut set, 901);
        set.extend([0; 2]);
        u32(&mut set, 1);
        set.extend([1, u8::from(suppressed), 0]);
        reference(&mut set, 905);
        set.push(0);
        reference(&mut set, 19);
        f.put(900, SET, 2, set, None);
        f.put(901, "2CA5A1CD-C99B-4C9B-91AF-57989148E841", 1, vec![], None);
        for id in [902, 903, 904] {
            f.put(id, BINDING, 0, vec![], None);
        }
        f.put(905, "6CCF41D5-40BE-48ED-A834-18F3EAED6C57", 1, vec![], None);
        f
    }

    #[test]
    fn chamfer_modes_keep_bindings_flip_chain_and_suppression() {
        for version in [2, 3] {
            for mode in 1..=3 {
                for flip in [false, true] {
                    for chain in [false, true] {
                        for suppressed in [false, true] {
                            let decoded = authored(version, mode, flip, chain, suppressed).design().history();
                            if mode == 1 && flip {
                                assert!(decoded.is_err());
                                continue;
                            }
                            let h = decoded.unwrap();
                            assert_eq!(
                                h.last().unwrap().kind,
                                HistoryKind::Chamfer {
                                    sets: vec![ChamferSet {
                                        output_labels: vec![71],
                                        container: 901,
                                        distance: 902,
                                        distance2: (mode == 2).then_some(903),
                                        angle: (mode == 3).then_some(904),
                                        flip,
                                        tangent_chain: chain,
                                        suppressed
                                    }]
                                }
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn legacy_group_tail_preserves_dimensions_and_suppression() {
        for mode in 1..=3 {
            for suppressed in [false, true] {
                let mut legacy = authored(2, mode, false, true, suppressed);
                let flag_at = if mode == 1 { 42 } else { 52 };
                legacy.0.get_mut(&900).unwrap().2.remove(flag_at);
                assert_eq!(
                    Fixture(legacy.0.clone()).design().history().unwrap(),
                    authored(2, mode, false, true, suppressed).design().history().unwrap()
                );
                let bytes = legacy.0[&900].2.clone();
                for keep in 0..bytes.len() {
                    legacy.0.get_mut(&900).unwrap().2 = bytes[..keep].to_vec();
                    assert!(Fixture(legacy.0.clone()).design().history().is_err(), "{mode}/{suppressed}/{keep}");
                }
                legacy.0.get_mut(&900).unwrap().2 = bytes;
                let owner_at = legacy.0[&900].2.len() - 10;
                legacy.0.get_mut(&900).unwrap().2[owner_at] = 99;
                assert!(Fixture(legacy.0.clone()).design().history().is_err(), "legacy owner must still match");

                let mut modern = authored(3, mode, false, true, suppressed);
                modern.0.get_mut(&900).unwrap().2.remove(flag_at);
                assert!(modern.design().history().is_err(), "v3 requires the modern tail");
            }
        }
    }

    #[test]
    fn output_face_labels_preserve_native_group_order() {
        let mut f = authored(3, 1, false, false, false);
        let bytes = &mut f.0.get_mut(&19).unwrap().2;
        let at = bytes.windows(8).position(|w| w == 71_u64.to_le_bytes()).unwrap();
        bytes[at - 4..at].copy_from_slice(&2_u32.to_le_bytes());
        let mut extra = 13_u64.to_le_bytes().to_vec();
        extra.extend([1, 0]);
        bytes.splice(at + 10..at + 10, extra);
        let history = f.design().history().unwrap();
        let HistoryKind::Chamfer { sets } = &history.last().unwrap().kind else { panic!("chamfer") };
        assert_eq!(sets[0].output_labels, [71, 13], "labels are references, not sorted output ordinals");
    }

    #[test]
    #[ignore = "requires local F3D corpus and modern adapter resave; archives are not committed"]
    fn original_adapter_chamfers_match_the_modern_resave() {
        let corpus = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS"));
        let probes = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").expect("set SOLIDCRAFT_F3D_PROBES"));
        let read = |path| {
            let designs = read_f3d(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(designs.len(), 1);
            designs[0]
                .timeline()
                .unwrap()
                .into_iter()
                .filter(|entry| entry.class == CLASS)
                .map(|entry| {
                    let feature = entry.feature.unwrap();
                    (feature.id, feature.suppressed, feature.kind)
                })
                .collect::<Vec<_>>()
        };
        let original = read(corpus.join("adapter-x20-makita.f3d"));
        assert_eq!(original.len(), 3);
        assert_eq!(original, read(probes.join("adapter-construction-resaved.f3d")));
    }

    #[test]
    #[ignore = "requires local asymmetric circular chamfer controls"]
    fn native_asymmetric_circular_caps_preserve_setbacks_and_edits() {
        use solidcraft_geom::Vec3;
        let dir = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
        for mode in ["two", "two-flipped", "angle", "angle-flipped"] {
            for top in [false, true] {
                let cap = if top { "top" } else { "bottom" };
                for (suffix, radius, height, first, second, suppressed) in [
                    ("baseline", 20., 40., 2., 3., false),
                    ("size4", 20., 40., 4., 3., false),
                    ("second", 20., 40., 2., 5., false),
                    ("diameter60", 30., 40., 2., 3., false),
                    ("height50", 20., 50., 2., 3., false),
                    ("suppressed", 20., 40., 2., 3., true),
                    ("restored", 20., 40., 2., 3., false),
                ] {
                    let name = format!("circular-asymmetric-{mode}-{cap}-{suffix}");
                    let native = read_f3d(&std::fs::read(dir.join(format!("{name}.f3d"))).unwrap()).unwrap();
                    let doc = native[0].to_document(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
                    let angle = if suffix == "second" { 60. } else { 30. };
                    let second = if mode.starts_with("angle") { first * f64::to_radians(angle).tan() } else { second };
                    let flip = !top ^ mode.ends_with("flipped");
                    let (radial, axial) = if flip { (second, first) } else { (first, second) };
                    check_circle(&doc, Vec3::Z, top, radius, height, radial, axial, suppressed);
                    if suffix == "baseline" {
                        for (parameter, value, radius, height, first, second) in [
                            ("d1", "60 mm", 30., 40., 2., second),
                            ("d2", "50 mm", 20., 50., 2., second),
                            ("d4", "4 mm", 20., 40., 4., if mode.starts_with("angle") { 2. * second } else { second }),
                            (
                                "d5",
                                if mode.starts_with("angle") { "60 deg" } else { "5 mm" },
                                20.,
                                40.,
                                2.,
                                if mode.starts_with("angle") { 2. * 60f64.to_radians().tan() } else { 5. },
                            ),
                        ] {
                            let mut edited = doc.clone();
                            edited.change_param(parameter, value, None, None).unwrap();
                            let (radial, axial) = if flip { (second, first) } else { (first, second) };
                            check_circle(&edited, Vec3::Z, top, radius, height, radial, axial, false);
                        }
                        let name = format!("circular-asymmetric-oriented-xz-{mode}-{cap}-baseline");
                        let native = read_f3d(&std::fs::read(dir.join(format!("{name}.f3d"))).unwrap()).unwrap();
                        let doc = native[0].to_document(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
                        check_circle(&doc, Vec3::Y, top, 20., 40., radial, axial, false);
                        let name = format!("circular-asymmetric-oriented-negative-{mode}-{cap}-baseline");
                        let native = read_f3d(&std::fs::read(dir.join(format!("{name}.f3d"))).unwrap()).unwrap();
                        let doc = native[0].to_document(&name).unwrap_or_else(|e| panic!("{name}: {e}"));
                        // Both verified negative-travel recipes designate the cap first.
                        let (radial, axial) = if mode.ends_with("flipped") { (second, first) } else { (first, second) };
                        check_circle(&doc, -Vec3::Z, top, 20., 40., radial, axial, false);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_circle(
        doc: &solidcraft_doc::Document,
        axis: solidcraft_geom::Vec3,
        top: bool,
        radius: f64,
        height: f64,
        radial: f64,
        axial: f64,
        suppressed: bool,
    ) {
        let reopened = crate::read_design(&crate::write_design_container(doc).unwrap()).unwrap();
        assert_eq!(*doc, reopened);
        for doc in [doc, &reopened] {
            let mut model = solidcraft_doc::Model::default();
            model.evaluate(doc);
            assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
            let state = model.state();
            assert_eq!(state.bodies.len(), 1);
            let body = &state.bodies[0];
            let mesh = body.mesh();
            let z = if top { height } else { 0. };
            let cap_radius = mesh.edges.iter().flatten().filter(|p| (p.dot(axis) - z).abs() < 1e-7).map(|p| (*p - axis * z).len()).fold(0., f64::max);
            assert!((cap_radius - (radius - if suppressed { 0. } else { radial })).abs() < 0.001, "cap radius {cap_radius}, {radial}");
            if !suppressed {
                let contact = z + if top { -axial } else { axial };
                assert!(
                    mesh.edges.iter().any(|e| e.len() > 2
                        && e.iter().all(|p| (p.dot(axis) - contact).abs() < 0.001 && ((*p - axis * contact).len() - radius).abs() < 0.001)),
                    "wall setback {axial}"
                );
            }
            let pi = std::f64::consts::PI;
            let stock = pi * radius * radius * height;
            let removed = if suppressed { 0. } else { pi * radial * axial * (radius - radial / 3.) };
            let moment = if suppressed { 0. } else { pi * radial * axial * axial * (radius / 3. - radial / 12.) };
            let moment = if top { height * removed - moment } else { moment };
            let measured = solidcraft_kernel::measure(&body.body).unwrap();
            assert!((measured.volume - (stock - removed)).abs() < stock * 2e-4);
            let center = axis * ((stock * height / 2. - moment) / (stock - removed));
            assert!(measured.centroid.dist(center) < 0.005);
        }
    }

    #[test]
    fn malformed_chamfers_do_not_drop_dimensions_or_relationships() {
        for mode in 1..=3 {
            for id in [19, 900] {
                for keep in 0..authored(3, mode, false, false, false).0[&id].2.len() {
                    let mut f = authored(3, mode, false, false, false);
                    f.0.get_mut(&id).unwrap().2.truncate(keep);
                    assert!(f.design().history().is_err(), "{mode}/{id}/{keep}");
                }
            }
            for offset in [6, 7] {
                let mut f = authored(3, mode, false, false, false);
                f.0.get_mut(&900).unwrap().2[offset] = 2;
                assert!(f.design().history().is_err());
            }
            let mut f = authored(3, mode, false, false, false);
            let b = &mut f.0.get_mut(&900).unwrap().2;
            let at = b.len() - 10;
            b[at] = 9;
            assert!(f.design().history().is_err(), "wrong group owner");
            let mut f = authored(3, mode, false, false, false);
            f.0.get_mut(&900).unwrap().2[if mode == 1 { 43 } else { 53 }] = 1;
            assert!(f.design().history().is_err(), "suppression mismatch");
            let mut f = authored(3, mode, false, false, false);
            f.0.get_mut(&902).unwrap().0 = SET.into();
            assert!(f.design().history().is_err(), "wrong dimension class");
        }
    }
}
