//! Bounded in-memory geometry recovery after a historical topology budget refusal.
//! Keep all body maps, transforms and solved records byte-for-byte. Only the
//! token-framed history tail of a binary BREP carrier is omitted. Never write
//! this reduced archive over the user's source or use it for editable import.
use super::super::*;
use cadmpeg_asm::asm_header;
use cadmpeg_core::decode::{DecodeArena, DecodeContext, DecodePolicy};
use std::io::{Seek, SeekFrom, Write};

struct BoundedArchive(Cursor<Vec<u8>>);

impl Write for BoundedArchive {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.position().checked_add(bytes.len() as u64).is_none_or(|end| end > MAX_F3D_BYTES as u64) {
            return Err(std::io::Error::other("reduced F3D archive exceeds 256 MiB"));
        }
        self.0.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Seek for BoundedArchive {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(position)
    }
}

pub(super) fn without_history(bytes: &[u8], policy: &DecodePolicy) -> Result<Vec<u8>> {
    if bytes.len() > MAX_F3D_BYTES {
        return Err(invalid("archive exceeds 256 MiB"));
    }
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| invalid(&e.to_string()))?;
    if input.len() > 10_000 {
        return Err(invalid("too many archive entries"));
    }
    let mut names = BTreeSet::new();
    let mut expanded = 0u64;
    for i in 0..input.len() {
        let entry = input.by_index_raw(i).map_err(|e| invalid(&e.to_string()))?;
        let name = entry.name();
        if name.len() > 4096
            || name.starts_with('/')
            || name.contains(['\\', ':'])
            || name.split('/').any(|part| part == "..")
            || !names.insert(name.to_owned())
        {
            return Err(invalid("unsafe or duplicate archive path"));
        }
        expanded = expanded.checked_add(entry.size()).ok_or_else(|| invalid("expanded size overflow"))?;
        if entry.size() > MAX_STREAM as u64 || expanded > MAX_EXPANDED {
            return Err(invalid("geometry recovery archive exceeds expansion limits"));
        }
    }
    if !names.contains("Manifest.dat") {
        return Err(invalid("missing archive manifest"));
    }
    let arena = DecodeArena::new();
    let (ctx, _) = DecodeContext::from_root_bytes(bytes, &arena, policy).map_err(|e| invalid(&e.to_string()))?;
    let mut output = zip::ZipWriter::new(BoundedArchive(Cursor::new(Vec::new())));
    let mut trimmed = false;
    for i in 0..input.len() {
        let name = input.by_index_raw(i).map_err(|e| invalid(&e.to_string()))?.name().to_owned();
        let mut data = archive::read(&mut input, &name)?;
        let carrier = name
            .rsplit_once("/Breps.BlobParts/")
            .is_some_and(|(_, leaf)| leaf.starts_with("BREP.") && !leaf.contains('/') && (leaf.ends_with(".smb") || leaf.ends_with(".smbh")));
        if carrier
            && asm_header::has_asm_magic(&data)
            && let Some(end) = asm_header::solved_record_limit(&ctx, &data).map_err(|e| invalid(&e.to_string()))?
        {
            let start = asm_header::record_stream_start(&data).ok_or_else(|| invalid("missing solved BREP record stream"))?;
            if end <= start || end >= data.len() {
                return Err(invalid("invalid solved BREP boundary"));
            }
            data.truncate(end);
            trimmed = true;
        }
        output
            .start_file(
                name,
                zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(1)),
            )
            .map_err(|e| invalid(&e.to_string()))?;
        output.write_all(&data).map_err(|e| invalid(&e.to_string()))?;
    }
    if !trimmed {
        return Err(invalid("no framed BREP history partition to omit"));
    }
    Ok(output.finish().map_err(|e| invalid(&e.to_string()))?.0.into_inner())
}

#[cfg(test)]
#[path = "solved_archive/tests.rs"]
mod tests;
