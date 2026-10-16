use super::*;
use crate::f3d::import_tests::{fixture, volume};
use crate::f3d::relationship_tests::{reference, refs, u32, u64, wide};
use std::f64::consts::{PI, TAU};
use std::io::Write;

// Original records and analytic geometry; no exported source bytes.
fn snapshot() -> Vec<NativeRecord> {
    use Token::{Double as D, False as F, Long as L, Position as P, Ref as R, Str as S, Vector3 as V};
    let rows = vec![
        ("asmheader", vec![R(-1), L(-1), S("original".into())]),
        ("body", vec![R(-1), L(-1), R(-1), R(2), R(-1), R(-1)]),
        ("lump", vec![R(-1), L(-1), R(-1), R(-1), R(3), R(1)]),
        ("shell", vec![R(-1), L(-1), R(-1), R(-1), R(-1), R(4), R(-1), R(2)]),
        ("face", vec![R(-1), L(-1), R(-1), R(-1), R(5), R(3), R(-1), R(9), F, F]),
        ("loop", vec![R(-1), L(-1), R(-1), R(-1), R(6), R(4)]),
        ("coedge", vec![R(8), L(-1), R(-1), R(6), R(6), R(-1), R(7), F, R(5), L(0), R(-1)]),
        ("edge", vec![R(-1), L(-1), R(-1), R(10), D(0.), R(10), D(TAU), R(6), R(11), F, S("unknown".into())]),
        ("ATTRIB_CUSTOM-attrib", vec![R(-1), L(-1), R(-1), R(-1), R(6), S("sketch_attrib_def".into()), L(1), L(1), L(3), S("17 2 0 0 1 1".into())]),
        ("plane-surface", vec![R(-1), L(-1), R(-1), P([0.; 3]), V([0., 0., 1.]), V([1., 0., 0.]), F, F, F, F, F]),
        ("vertex", vec![R(-1), L(-1), R(-1), R(7), L(0), R(12)]),
        ("ellipse-curve", vec![R(-1), L(-1), R(-1), P([0.; 3]), V([0., 0., 1.]), V([3., 0., 0.]), D(1.), F, F]),
        ("point", vec![R(-1), L(-1), R(-1), P([3., 0., 0.])]),
        ("End-of-ASM-data", vec![]),
    ];
    rows.into_iter()
        .enumerate()
        .map(|(index, (name, tokens))| NativeRecord { index, name: name.into(), tokens: tokens.into(), offset: 0, len: 0 })
        .collect()
}

fn binary(records: &[NativeRecord]) -> Vec<u8> {
    fn text(out: &mut Vec<u8>, tag: u8, value: &str) {
        out.extend([tag, value.len() as u8]);
        out.extend(value.as_bytes());
    }
    let mut out = b"ASM BinaryFile4".to_vec();
    for n in [22900, 0, 2, 2] {
        u32(&mut out, n);
    }
    for s in ["Original", "1", "2000-01-01"] {
        text(&mut out, 7, s);
    }
    for n in [10_f64, 1e-7, 1e-10] {
        out.push(6);
        out.extend(n.to_le_bytes());
    }
    for record in records {
        let names: Vec<_> = record.name.split('-').collect();
        for (i, name) in names.iter().enumerate() {
            text(&mut out, if i + 1 == names.len() { 13 } else { 14 }, name);
        }
        for t in record.tokens.iter() {
            match t {
                Token::Ref(n) => {
                    out.push(12);
                    out.extend((*n as i32).to_le_bytes());
                }
                Token::Long(n) => {
                    out.push(4);
                    out.extend((*n as i32).to_le_bytes());
                }
                Token::Double(n) => {
                    out.push(6);
                    out.extend(n.to_le_bytes());
                }
                Token::Str(s) => text(&mut out, 7, s),
                Token::False => out.push(11),
                Token::True => out.push(10),
                Token::Position(v) | Token::Vector3(v) => {
                    out.push(if matches!(t, Token::Position(_)) { 19 } else { 20 });
                    for n in v {
                        out.extend(n.to_le_bytes());
                    }
                }
                _ => panic!("unsupported authored test token"),
            }
        }
        out.push(17);
    }
    out
}

fn archived_snapshot(records: &[NativeRecord]) -> Design {
    let mut f = fixture(false);
    let mut b = vec![0; 6];
    reference(&mut b, 101);
    u32(&mut b, 1);
    wide(&mut b, "original-asset");
    wide(&mut b, "70");
    u32(&mut b, 1);
    u32(&mut b, 0);
    refs(&mut b, &[150]);
    reference(&mut b, 9);
    f.put(100, "4BD53E5A-0B3E-45E5-AE8B-02044306485A", 0, b, None);
    let mut b = vec![0; 6];
    reference(&mut b, 100);
    for n in [1, 1, 1, 3] {
        u32(&mut b, n);
    }
    u64(&mut b, 999);
    u64(&mut b, 2);
    u32(&mut b, 0);
    u32(&mut b, 1);
    u32(&mut b, 1);
    u64(&mut b, 0);
    b.push(1);
    u32(&mut b, 0);
    f.put(101, "0D57BD2F-D09B-43FC-AD57-1E89A118C453", 1, b, None);
    let mut b = vec![0; 6];
    u64(&mut b, 501);
    wide(&mut b, "original-asset");
    wide(&mut b, "original-context");
    u32(&mut b, 2);
    b.extend([0; 5]);
    f.put(150, HANDLE, 1, b, None);
    let mut b = vec![0; 6];
    reference(&mut b, 152);
    u32(&mut b, 0);
    reference(&mut b, 153);
    b.push(0);
    wide(&mut b, "BREP.original.smb");
    f.put(151, MAP, 1, b, None);
    let mut b = vec![0; 6];
    u32(&mut b, 1);
    u64(&mut b, 123456);
    u64(&mut b, 502);
    b.extend([0; 2]);
    u32(&mut b, 1);
    u64(&mut b, 501);
    f.put(152, LIST, 0, b, None);
    f.put(153, CONTAINER, 4, vec![], None);
    // An unused map may have no snapshot blob. It must not prevent resolving
    // a different, explicitly selected identity.
    let mut b = vec![0; 6];
    reference(&mut b, 155);
    u32(&mut b, 0);
    reference(&mut b, 153);
    b.push(0);
    wide(&mut b, "");
    f.put(154, MAP, 1, b, None);
    let mut b = vec![0; 6];
    u32(&mut b, 0);
    u64(&mut b, 1);
    b.extend([0; 2]);
    u32(&mut b, 0);
    f.put(155, LIST, 0, b, None);
    let mut design = f.design();
    design.path = "original/Design1".into();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("original/Breps.BlobParts/BREP.original.smb", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(&binary(records)).unwrap();
    design.archive = Some(zip.finish().unwrap().into_inner().into());
    design
}

#[test]
fn stale_boundary_recovers_named_circle_and_preserves_edits_and_reopening() {
    let design = archived_snapshot(&snapshot());
    let profiles = design.sketch_profiles().unwrap();
    assert_eq!(profiles[0].regions[0][0].curves, [31]);
    assert_eq!(profiles[0].regions[0][0].pieces, [(1, 1)]);
    let mut doc = design.to_document("Recovered editable selection").unwrap();
    volume(&doc, 900. * PI * 17.);
    doc.change_param("d7", "80 mm", None, None).unwrap();
    doc.change_param("height", "23 mm", None, None).unwrap();
    volume(&doc, 1600. * PI * 23.);
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(doc, reopened);
    volume(&reopened, 1600. * PI * 23.);
}

#[test]
fn snapshots_reject_wrong_sources_cycles_and_ambiguous_tags() {
    for (record, slot, token) in [
        (8, 9, Token::Str("999 2 0 0 1 1".into())),
        (8, 9, Token::Str("17 2 0 0 2 1".into())),
        (8, 9, Token::Str("17 2 0 0 1 1 0".into())),
        (8, 2, Token::Ref(8)),
        (8, 4, Token::Ref(7)),
        (6, 3, Token::Ref(7)),
        (6, 4, Token::Ref(7)),
        (6, 8, Token::Ref(1)),
        (4, 3, Token::Ref(4)),
    ] {
        let mut rows = snapshot();
        let mut tokens = rows[record].tokens.to_vec();
        tokens[slot] = token;
        rows[record].tokens = tokens.into();
        assert!(archived_snapshot(&rows).to_document("Invalid snapshot").is_err(), "{record}/{slot}");
    }
    let rows = snapshot();
    let wrong = BTreeMap::from([((71, (17, 2)), 31)]);
    assert!(selected_loop(&rows, 1, 70, &wrong).is_err());
    for end in 0..rows.len() - 1 {
        assert!(archived_snapshot(&rows[..end]).sketch_profiles().is_err());
    }
}

#[test]
#[ignore = "requires local Eye archive"]
fn native_eye_resolves_current_circle_from_snapshot() {
    let path = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap()).join("knobs/Eye.f3d");
    let d = read_f3d(&std::fs::read(path).unwrap()).unwrap();
    let profiles = d[0].sketch_profiles().unwrap();
    let p = profiles.iter().find(|p| p.feature == 380).unwrap();
    assert_eq!(p.regions[0][0].curves, [347]);
    assert_eq!(p.regions[0][0].pieces, [(1, 1)]);
}
