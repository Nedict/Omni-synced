use super::import_tests::{Fixture, volume};
use super::relationship_tests::{reference, refs, u32, u64, wide};

pub(super) fn fixture() -> Fixture {
    let mut f = super::round_profile_tests::ring();
    for (curve, group) in [(31, 301), (35, 302)] {
        let bytes = &mut f.0.get_mut(&curve).unwrap().2;
        let at = bytes.len() - 41;
        let mut b = Vec::new();
        refs(&mut b, &[group]);
        bytes.splice(at..at + 4, b);
    }
    for (id, other, curve, source) in [(301, 302, 31, true), (302, 301, 35, false)] {
        let mut b = vec![0; 4];
        b.push(1);
        u32(&mut b, 1);
        reference(&mut b, curve);
        u32(&mut b, u32::from(source));
        b.push(0);
        reference(&mut b, other);
        u32(&mut b, if source { 1 } else { 2 });
        let mut secondary = vec![0; 4];
        refs(&mut secondary, &[curve]);
        u64(&mut secondary, 0);
        reference(&mut secondary, 300);
        f.put(id, "04e598ff-2979-5e41-8bc0-5c435c810909", 0, b, Some(secondary));
    }
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 5);
    for id in [31, 301, 302, 303, 304] {
        reference(&mut b, id);
        u32(&mut b, 0);
    }
    b.push(0);
    reference(&mut b, 305);
    b.extend([1, 0]);
    b.extend(0.5_f64.to_le_bytes());
    for id in [302, 304, 303, 70, 301, 31] {
        reference(&mut b, id);
    }
    b.extend((-2_f64).to_le_bytes());
    f.put(300, "afc07c10-99ac-5304-ae1b-dbee335b0e43", 1, b, None);
    f.put(305, "B7F34D7B-DAC4-41A5-B67D-D672537C3BFA", 2, vec![], None);
    super::geometric_tests::relation(&mut f, 304, 0x20_0000_0000, &[31, 35]);
    f.0.get_mut(&304).unwrap().0 = "D3BD153B-EB8A-405E-9D29-69EE0C3D227C".into();
    f.0.get_mut(&304).unwrap().1 = 0;
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 2);
    for id in [0, 31] {
        reference(&mut b, id);
        u32(&mut b, 0);
    }
    b.push(0);
    b.extend([0; 220]);
    wide(&mut b, "");
    wide(&mut b, "");
    b.extend([0; 44]);
    u64(&mut b, 0x10_0000_0000);
    reference(&mut b, 306);
    refs(&mut b, &[31]);
    b.push(0);
    let mut secondary = vec![0; 4];
    reference(&mut secondary, 70);
    secondary.extend([0; 18]);
    reference(&mut secondary, 70);
    f.put(303, "8C780195-72C0-4a56-A911-E43AB14357F2", 3, b, Some(secondary));
    let mut b = vec![0; 20];
    u32(&mut b, 0);
    reference(&mut b, 306);
    wide(&mut b, "-20 mm");
    b.extend([0; 10]);
    for s in ["Offset", "", "mm", "offset"] {
        wide(&mut b, s);
    }
    b.extend((-2_f64).to_le_bytes());
    b.extend([0; 12]);
    f.put(307, "7A6A3D31-BE74-4E19-9115-20642944C2E4", 9, b, None);
    let mut b = vec![0; 4];
    b.push(1);
    refs(&mut b, &[8]);
    b.extend([0; 5]);
    b.extend((-2_f64).to_le_bytes());
    reference(&mut b, 307);
    f.put(306, "D91D429C-1BB5-4640-B49F-69AF14F463E9", 0, b, None);
    let mut b = vec![0; 6];
    wide(&mut b, "");
    u64(&mut b, 0);
    b.push(1);
    refs(&mut b, &[55, 304]);
    b.push(1);
    refs(&mut b, &[56, 303]);
    b.extend([0; 11]);
    refs(&mut b, &[300]);
    b.extend([0; 2]);
    reference(&mut b, 8);
    b.extend([0; 4]);
    f.0.get_mut(&70).unwrap().2 = b;
    f
}

#[test]
fn offset_records_keep_source_and_distance_edits_through_reopening() {
    let mut f = fixture();
    f.0.get_mut(&301).unwrap().2[20..24].copy_from_slice(&123_u32.to_le_bytes());
    let mut doc = f.design().to_document("Editable offset ring").unwrap();
    for (diameter, offset) in [(60., -20.), (80., -15.), (50., -5.)] {
        doc.change_param("d7", &format!("{diameter} mm"), None, None).unwrap();
        doc.change_param("offset", &format!("{offset} mm"), None, None).unwrap();
        let radius = diameter / 2.;
        let want = std::f64::consts::PI * (radius * radius - (radius + offset) * (radius + offset)) * 17.;
        volume(&doc, want);
        volume(&crate::read_design(&crate::write_design(&doc)).unwrap(), want);
    }
}

#[test]
fn malformed_offset_records_never_flatten_into_independent_geometry() {
    for id in [300, 301, 302, 303, 304] {
        for secondary in [false, true] {
            let original = fixture();
            let Some(bytes) = (if secondary { original.0[&id].3.as_ref() } else { Some(&original.0[&id].2) }) else { continue };
            for keep in 0..bytes.len() {
                let mut f = fixture();
                let r = f.0.get_mut(&id).unwrap();
                if secondary {
                    r.3.as_mut().unwrap().truncate(keep);
                } else {
                    r.2.truncate(keep);
                }
                assert!(f.design().to_document("Invalid offset").is_err(), "record {id}, secondary {secondary}, prefix {keep}");
            }
        }
    }
    let mut f = fixture();
    let b = &mut f.0.get_mut(&300).unwrap().2;
    let n = b.len();
    b[n - 8..].copy_from_slice(&(-1_f64).to_le_bytes());
    assert!(f.design().to_document("Inconsistent offset value").is_err());
    let mut f = fixture();
    let b = &mut f.0.get_mut(&304).unwrap().2;
    let n = b.len();
    b[n - 12..n - 1].copy_from_slice(&{
        let mut v = Vec::new();
        reference(&mut v, 31);
        v
    });
    assert!(f.design().to_document("Repeated output").is_err());
}
