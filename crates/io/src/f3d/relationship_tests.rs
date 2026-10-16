use super::*;

pub(super) fn u32(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_le_bytes());
}
pub(super) fn u64(b: &mut Vec<u8>, n: u64) {
    b.extend(n.to_le_bytes());
}
pub(super) fn string(b: &mut Vec<u8>, s: &str) {
    u32(b, s.len() as u32);
    b.extend(s.as_bytes());
}
pub(super) fn wide(b: &mut Vec<u8>, s: &str) {
    u32(b, s.encode_utf16().count() as u32);
    for c in s.encode_utf16() {
        b.extend(c.to_le_bytes());
    }
}
pub(super) fn reference(b: &mut Vec<u8>, id: u64) {
    b.push(1);
    u64(b, id);
    b.extend([0; 2]);
}
pub(super) fn refs(b: &mut Vec<u8>, ids: &[u64]) {
    u32(b, ids.len() as u32);
    for id in ids {
        reference(b, *id);
    }
}

/// Hand-authored records; synthetic identities, values and presentation fields.
pub(super) fn fixture(construction: bool, curve_tag: u64) -> Design {
    let mut d = Design {
        recovery: None,
        archive: None,
        path: "test".into(),
        build: 1,
        layout: 8,
        classes: vec![],
        roots: vec![],
        records: vec![],
        metadata: vec![],
        bulk: vec![],
    };
    let mut add = |id, guid: &str, version, primary: Vec<u8>, secondary: Option<Vec<u8>>| {
        let mut write = |bytes: Vec<u8>| {
            let at = d.bulk.len();
            string(&mut d.bulk, "8");
            u64(&mut d.bulk, id);
            d.bulk.extend(bytes);
            at
        };
        let offset = write(primary);
        let secondary_offset = secondary.map(write);
        d.records.push(Record { id, offset, secondary_offset });
        if let Some(class) = d.classes.iter_mut().find(|c| c.id == guid) {
            class.objects.push(id);
        } else {
            d.classes.push(Class { id: guid.into(), version, parent: String::new(), group: String::new(), objects: vec![id] });
        }
    };
    let mut b = vec![0; 4];
    b.extend([0, 1]);
    u32(&mut b, 2);
    for (key, value) in [("crv_primary_id", curve_tag), ("crv_secondary_id", 2)] {
        string(&mut b, key);
        string(&mut b, "IntrinsicMetaTypeuint64");
        u64(&mut b, value);
    }
    for x in [1_f64, 2., 0., 0., 0., 1., 1., 0., 0., 3., 0., std::f64::consts::TAU] {
        b.extend(x.to_le_bytes());
    }
    reference(&mut b, 41);
    b.extend([0; 9]);
    b.push(construction as u8);
    b.extend([0; 34]);
    reference(&mut b, 70);
    add(31, "F0130424-8B7E-4092-93C9-1CA807482534", 0, b, None);
    for id in [41, 42] {
        add(id, "C2CEDAE7-1716-47C1-B7B1-07B70081D0FB", 11, vec![], None);
    }
    let mut b = vec![0; 4];
    b.extend([0, 1]);
    u32(&mut b, 0);
    wide(&mut b, "original");
    u64(&mut b, 0);
    b.push(1);
    refs(&mut b, &[55]);
    b.push(1);
    refs(&mut b, &[56]);
    b.extend([0; 17]);
    reference(&mut b, 8);
    b.extend([0; 4]);
    add(70, "44A64366-4BD3-4B24-881A-F94C206E8F2D", 18, b, None);
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 2);
    reference(&mut b, 41);
    u32(&mut b, 0);
    reference(&mut b, 42);
    u32(&mut b, 0);
    b.push(1);
    u32(&mut b, 0);
    reference(&mut b, 70);
    u64(&mut b, 1);
    refs(&mut b, &[42, 41]);
    b.push(0);
    add(55, "60403D47-0C49-49B0-BDE8-1679608164A2", 3, b, None);
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 2);
    reference(&mut b, 0);
    u32(&mut b, 4);
    reference(&mut b, 31);
    u32(&mut b, 0);
    b.push(1);
    u32(&mut b, 0);
    b.extend([0; 244]);
    wide(&mut b, "Ø");
    wide(&mut b, "60 mm");
    b.extend([0; 24]);
    u64(&mut b, 0x10000);
    reference(&mut b, 61);
    refs(&mut b, &[31]);
    b.push(0);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 70);
    secondary.extend([0; 18]);
    reference(&mut secondary, 70);
    add(56, "664AD49F-207F-4CD5-8555-DD269E9F09EA", 0, b, Some(secondary));
    let mut b = vec![0; 20];
    u32(&mut b, 7);
    reference(&mut b, 61);
    wide(&mut b, "60 mm");
    b.extend([0; 10]);
    for s in ["Diameter", "", "mm", "d7"] {
        wide(&mut b, s);
    }
    b.extend(6_f64.to_le_bytes());
    b.extend([0; 12]);
    add(62, "7A6A3D31-BE74-4E19-9115-20642944C2E4", 9, b, None);
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 1);
    reference(&mut b, 8);
    b.extend([0; 5]);
    b.extend(6_f64.to_le_bytes());
    reference(&mut b, 62);
    add(61, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0, b, None);
    add(9, "DD405BC2-D673-44F0-8833-5CB2A1C186C7", 8, vec![], None);
    add(8, "8DA771B7-52ED-42FC-94E2-2353FE141373", 5, vec![], None);
    let mut b = vec![0; 6];
    reference(&mut b, 101);
    u32(&mut b, 1);
    wide(&mut b, "synthetic-source");
    wide(&mut b, "70");
    b.extend([0; 20]);
    reference(&mut b, 9);
    add(100, "4BD53E5A-0B3E-45E5-AE8B-02044306485A", 0, b, None);
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    for n in [1, 1, 1, 3] {
        u32(&mut b, n);
    }
    u64(&mut b, 17);
    u64(&mut b, 2);
    u32(&mut b, 0);
    u32(&mut b, 1);
    u32(&mut b, 1);
    u64(&mut b, 0);
    u32(&mut b, 1);
    b.push(0);
    add(101, "0D57BD2F-D09B-43FC-AD57-1E89A118C453", 1, b, None);
    let mut b = vec![0; 6];
    refs(&mut b, &[100]);
    b.extend([0; 20]);
    reference(&mut b, 9);
    add(102, "2CA5A1CD-C99B-4C9B-91AF-57989148E841", 1, b, None);
    d
}

#[test]
fn selected_circle_and_driving_dimension_follow_explicit_references() {
    for construction in [false, true] {
        let d = fixture(construction, 17);
        let c = d.sketch_circles().unwrap();
        assert_eq!(c[0].construction, construction);
        assert_eq!(c[0].profile_id, Some((17, 2)));
        assert_eq!(d.selected_circle_profiles(9, &[102]).unwrap(), [CircleProfile { id: 100, feature: 9, geometry: 70, circle: 31 }]);
        assert_eq!(
            d.sketch_constraints().unwrap(),
            [SketchConstraints {
                geometry: 70,
                feature: 8,
                constraints: vec![
                    SketchConstraint::Coincident { id: 55, points: [41, 42] },
                    SketchConstraint::Diameter { id: 56, circle: 31, binding: 61 }
                ]
            }]
        );
    }
    // A selected native profile tag is not a curve's object id or its index.
    assert!(fixture(false, 31).circle_profiles().is_err());
    let d = fixture(false, 17);
    assert!(d.selected_circle_profiles(999, &[102]).is_err());
    assert!(d.selected_circle_profiles(9, &[102, 102]).is_err());
}

#[test]
fn unsupported_constraints_and_damaged_relationships_fail() {
    let mut d = fixture(false, 17);
    let rec = d.records.iter().find(|r| r.id == 55).unwrap();
    let at = rec.offset + Records::new(&d).primary(55).unwrap().len() - 35;
    d.bulk[at..at + 8].copy_from_slice(&0xdead_u64.to_le_bytes());
    assert!(d.sketch_constraints().unwrap_err().to_string().contains("constraint kind"));
    let mut d = fixture(false, 17);
    d.classes.iter_mut().find(|c| c.objects.contains(&55)).unwrap().version = 999;
    assert!(d.sketch_constraints().is_err());
    for (id, secondary) in [(31, false), (55, false), (56, false), (56, true), (62, false), (101, false)] {
        let d = fixture(false, 17);
        let records = Records::new(&d);
        let bytes = if secondary { records.secondary(id) } else { records.primary(id) }.unwrap();
        for keep in 0..bytes.len() {
            let mut d = fixture(false, 17);
            let rec = d.records.iter().find(|r| r.id == id).unwrap();
            let offset = if secondary { rec.secondary_offset.unwrap() } else { rec.offset };
            let removed = bytes.len() - keep;
            d.bulk.drain(offset + keep..offset + bytes.len());
            for r in &mut d.records {
                if r.offset > offset {
                    r.offset -= removed;
                }
                if let Some(at) = &mut r.secondary_offset
                    && *at > offset
                {
                    *at -= removed;
                }
            }
            let failed = if id == 31 || id == 101 { d.circle_profiles().is_err() } else { d.sketch_constraints().is_err() };
            assert!(failed, "record {id}, secondary {secondary}, prefix {keep}");
        }
    }
}

#[test]
fn shared_entity_connection_slots_preserve_explicit_relationships() {
    let mut d = fixture(false, 17);
    let expected = d.sketch_constraints().unwrap();
    for id in [55, 56] {
        let record = d.records.iter().find(|r| r.id == id).unwrap();
        let at = record.offset + super::history::payload(Records::new(&d).primary(id).unwrap(), id).unwrap().at;
        for (offset, slot) in [(20, 17_u32), (35, u32::MAX)] {
            d.bulk[at + offset..at + offset + 4].copy_from_slice(&slot.to_le_bytes());
        }
    }
    assert_eq!(d.sketch_constraints().unwrap(), expected);
}
