//! ZIP method 93 using a pure-Rust decoder. Header limits follow RFC 8878 §3.1.1.1.
use super::*;

pub(super) fn read(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    let i = zip.index_for_name(name).ok_or_else(|| invalid(&format!("missing stream {name}")))?;
    let mut raw = zip.by_index_raw(i).map_err(|e| invalid(&e.to_string()))?;
    let size = raw.size();
    let crc = raw.crc32();
    if size > MAX_STREAM as u64 {
        return Err(invalid("design stream exceeds 128 MiB"));
    }
    let mut bytes = Vec::new();
    if raw.compression() == zip::CompressionMethod::ZSTD {
        let mut compressed = Vec::new();
        raw.read_to_end(&mut compressed).map_err(|e| invalid(&e.to_string()))?;
        check_window(&compressed)?;
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(compressed.as_slice()).map_err(|e| invalid(&e.to_string()))?;
        (&mut decoder).take(size + 1).read_to_end(&mut bytes).map_err(|e| invalid(&e.to_string()))?;
        if !decoder.into_inner().is_empty() {
            return Err(invalid("unsupported trailing Zstandard frames"));
        }
    } else {
        drop(raw);
        zip.by_index(i).map_err(|e| invalid(&e.to_string()))?.take(size + 1).read_to_end(&mut bytes).map_err(|e| invalid(&e.to_string()))?;
    }
    if bytes.len() as u64 != size || crc32fast::hash(&bytes) != crc {
        return Err(invalid("stream size or ZIP checksum mismatch"));
    }
    Ok(bytes)
}

// Validate before constructing the decoder, which allocates its history window.
fn check_window(bytes: &[u8]) -> Result<()> {
    let mut r = Reader { bytes, at: 0 };
    if r.u32()? != 0xfd2fb528 {
        return Err(invalid("unsupported Zstandard frame"));
    }
    let byte = |r: &mut Reader<'_>| -> Result<u8> { r.take(1)?.first().copied().ok_or_else(|| invalid("truncated Zstandard header")) };
    let flags = byte(&mut r)?;
    let single = flags & 0x20 != 0;
    let window = if single {
        0
    } else {
        let w = byte(&mut r)?;
        let base = 1u64 << (10 + (w >> 3));
        base + (base / 8) * u64::from(w & 7)
    };
    let dict_len = match flags & 3 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    };
    r.take(dict_len)?;
    let size_len = match flags >> 6 {
        0 => usize::from(single),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let mut size = 0u64;
    for (i, b) in r.take(size_len)?.iter().enumerate() {
        size |= u64::from(*b) << (i * 8);
    }
    if size_len == 2 {
        size += 256;
    }
    if window > MAX_STREAM as u64 || size > MAX_STREAM as u64 {
        return Err(invalid("Zstandard window or content exceeds 128 MiB"));
    }
    Ok(())
}
