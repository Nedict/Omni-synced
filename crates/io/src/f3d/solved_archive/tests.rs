use super::*;

fn text(bytes: &mut Vec<u8>, tag: u8, value: &[u8]) {
    bytes.extend([tag, u8::try_from(value.len()).unwrap()]);
    bytes.extend(value);
}

fn identifier(bytes: &mut Vec<u8>, name: &str) {
    let parts = name.split('-').collect::<Vec<_>>();
    for (i, part) in parts.iter().enumerate() {
        text(bytes, if i + 1 == parts.len() { 13 } else { 14 }, part.as_bytes());
    }
}

// Original token-framing records with arbitrary metadata. No native asset bytes.
fn stream(wide: bool, legacy: bool) -> (Vec<u8>, usize) {
    let mut bytes = if wide { b"ASM BinaryFile8".to_vec() } else { b"ASM BinaryFile4".to_vec() };
    bytes.resize(if wide { 47 } else { 31 }, 0);
    bytes[15..19].copy_from_slice(&22900u32.to_le_bytes());
    bytes[if wide { 31 } else { 23 }] = 1;
    bytes[if wide { 39 } else { 27 }] = 3;
    for value in ["Original", "1", "2000-01-01"] {
        text(&mut bytes, 7, value.as_bytes());
    }
    for value in [10f64, 1e-7, 1e-10] {
        bytes.push(6);
        bytes.extend(value.to_le_bytes());
    }
    identifier(&mut bytes, "asmheader");
    let mut embedded = Vec::new();
    identifier(&mut embedded, "Begin-of-ASM-History-Data");
    embedded.push(17);
    text(&mut bytes, 7, &embedded);
    bytes.push(17);
    let end = bytes.len();
    identifier(&mut bytes, if legacy { "delta_state" } else { "Begin-of-ASM-History-Data" });
    bytes.push(17);
    identifier(&mut bytes, "End-of-ASM-History-Data");
    bytes.push(17);
    (bytes, end)
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        output.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        output.write_all(data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn only_framed_history_is_removed_and_all_body_mapping_bytes_survive() {
    for wide in [false, true] {
        for legacy in [false, true] {
            let (carrier, end) = stream(wide, legacy);
            let snapshot = b"independent solved body and transform";
            let entries = [
                ("Manifest.dat", b"original manifest".as_slice()),
                ("asset/Design1/BulkStream.dat", b"body mapping with stable identities".as_slice()),
                ("asset/Breps.BlobParts/BREP.original.smbh", carrier.as_slice()),
                ("asset/Breps.BlobParts/BREP.snapshot.smb", snapshot.as_slice()),
                ("asset/unrelated.dat", carrier.as_slice()),
            ];
            let original = zip(&entries);
            let before = original.clone();
            let reduced = without_history(&original, &DecodePolicy::service()).unwrap();
            assert_eq!(original, before, "the source must stay byte-identical");
            let mut archive = zip::ZipArchive::new(Cursor::new(reduced.as_slice())).unwrap();
            assert_eq!(archive.len(), entries.len());
            for (name, data) in entries {
                let expected = if name.ends_with(".smbh") { &data[..end] } else { data };
                assert_eq!(super::super::super::archive::read(&mut archive, name).unwrap(), expected, "{wide}/{legacy}/{name}");
            }
        }
    }
}

#[test]
fn missing_partitions_malformed_tokens_and_unsafe_archives_are_refused() {
    let (carrier, end) = stream(false, false);
    let archive = |data: &[u8], name| zip(&[("Manifest.dat", b"original"), (name, data)]);
    let path = "asset/Breps.BlobParts/BREP.original.smb";
    for keep in [0, 15, 31, end - 1, end] {
        assert!(without_history(&archive(&carrier[..keep], path), &DecodePolicy::service()).is_err());
    }
    for name in ["/asset/BREP.smb", "../asset/BREP.smb", "asset\\BREP.smb", "asset:BREP.smb"] {
        assert!(without_history(&archive(&carrier, name), &DecodePolicy::service()).is_err());
    }
    assert!(without_history(&zip(&[(path, &carrier)]), &DecodePolicy::service()).is_err());
    let mut limited = DecodePolicy::service();
    limited.limits.max_work_units = 1;
    assert!(without_history(&archive(&carrier, path), &limited).is_err(), "the framing pass must obey its work budget");
}

#[test]
fn reduced_archive_output_is_bounded_before_allocation() {
    let mut output = BoundedArchive(Cursor::new(Vec::new()));
    output.seek(SeekFrom::Start(MAX_F3D_BYTES as u64)).unwrap();
    assert!(output.write_all(&[1]).is_err());
    assert!(output.0.get_ref().is_empty());
    output.seek(SeekFrom::Start(u64::MAX)).unwrap();
    assert!(output.write_all(&[1]).is_err());
    assert!(output.0.get_ref().is_empty());
}
