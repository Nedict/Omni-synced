//! Optional metadata blocks shared by verified sketch entity schemas.
use super::*;

pub(super) fn read(r: &mut Reader<'_>) -> Result<BTreeMap<String, u64>> {
    let mut attributes = BTreeMap::new();
    match r.take(1)? {
        [0] => return Ok(attributes),
        [1] => {}
        _ => return Err(invalid("unsupported attribute presence flag")),
    }
    for _ in 0..r.count(16)? {
        let key = r.string()?;
        if r.string()? != "IntrinsicMetaTypeuint64" {
            return Err(invalid("unsupported sketch attribute type"));
        }
        if attributes.insert(key, r.u64()?).is_some() {
            return Err(invalid("duplicate sketch attribute"));
        }
    }
    Ok(attributes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::relationship_tests::{string, u32, u64};

    fn decode(bytes: &[u8]) -> Result<BTreeMap<String, u64>> {
        let mut r = Reader { bytes, at: 0 };
        let result = read(&mut r)?;
        super::super::history::finish(&r)?;
        Ok(result)
    }

    #[test]
    fn optional_attributes_are_bounded_and_unambiguous() {
        assert_eq!(decode(&[0]).unwrap(), decode(&[1, 0, 0, 0, 0]).unwrap());
        assert!(decode(&[2]).is_err());
        let mut b = vec![1];
        u32(&mut b, 1);
        string(&mut b, "original Δ");
        string(&mut b, "IntrinsicMetaTypeuint64");
        u64(&mut b, 17);
        assert_eq!(decode(&b).unwrap().get("original Δ"), Some(&17));
        for keep in 0..b.len() {
            assert!(decode(&b[..keep]).is_err(), "prefix {keep}");
        }
        let entry = b[5..].to_vec();
        b[1..5].copy_from_slice(&2u32.to_le_bytes());
        b.extend(entry);
        assert!(decode(&b).is_err());
        let mut bad = vec![1];
        u32(&mut bad, u32::MAX);
        assert!(decode(&bad).is_err());
        let mut bad = vec![1];
        u32(&mut bad, 1);
        string(&mut bad, "key");
        string(&mut bad, "UnknownType");
        u64(&mut bad, 1);
        assert!(decode(&bad).is_err());
    }
}
