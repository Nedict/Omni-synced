//! Validate cached curve envelopes without using their coordinates as selections.
//! The actual edge is chosen by the owned face and endpoint incidence graph.
use super::*;

const SPLINE: &str = "2319971D-8E07-441A-8F16-11C8F6F862D4";

fn number(r: &mut Reader<'_>) -> Result<f64> {
    let value = f64::from_bits(r.u64()?);
    if !value.is_finite() {
        return Err(invalid("nonfinite blend cache value"));
    }
    Ok(value)
}

fn array(r: &mut Reader<'_>, width: usize) -> Result<usize> {
    let count = r.count(width)?;
    if count > 4096 || r.u32()? as usize != count || r.u32()? != 8 {
        return Err(invalid("invalid blend cache array"));
    }
    Ok(count)
}

fn spline(records: &Records<'_>, id: u64) -> Result<()> {
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 8)?;
    let degree = r.u32()? as usize;
    if !(1..=16).contains(&degree) || number(&mut r)? <= 0. {
        return Err(invalid("unsupported blend cache spline degree or tolerance"));
    }
    let count = array(&mut r, 8)?;
    let mut knots = Vec::with_capacity(count);
    for _ in 0..count {
        knots.push(number(&mut r)?);
    }
    if knots.windows(2).any(|pair| pair.first() > pair.get(1)) {
        return Err(invalid("unordered blend cache knots"));
    }
    let weights = array(&mut r, 8)?;
    for _ in 0..weights {
        if number(&mut r)? <= 0. {
            return Err(invalid("invalid blend cache weight"));
        }
    }
    let points = array(&mut r, 24)?;
    if points <= degree || points + degree + 1 != count || (weights != 0 && weights != points) {
        return Err(invalid("inconsistent blend cache spline arrays"));
    }
    for _ in 0..points * 3 {
        number(&mut r)?;
    }
    if number(&mut r)? <= 0. || r.take(2)? != [1, 1] {
        return Err(invalid("unsupported blend cache spline interval"));
    }
    let start = number(&mut r)?;
    let end = number(&mut r)?;
    if start >= end || knots.get(degree).is_none_or(|k| start < *k) || knots.get(points).is_none_or(|k| end > *k) {
        return Err(invalid("blend cache interval is outside its spline domain"));
    }
    if r.take(5)? != [0, 0, 0, 1, 0] {
        return Err(invalid("unsupported blend cache spline tail"));
    }
    finish(&r)
}

pub(super) fn validate(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64) -> Result<()> {
    match classes.get(&id).map(|c| (c.id.as_str(), c.version)) {
        Some(("50AEE8B4-9456-4DD7-847D-6F6C909BBB9D" | "B0B3FA59-C374-444F-B424-24AA5DE41645", 0)) => Ok(()),
        Some((SPLINE, 2)) => spline(records, id),
        _ => Err(invalid("unsupported blend cache carrier")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f3d::import_tests::Fixture;
    use crate::f3d::relationship_tests::u32;

    fn authored() -> Vec<u8> {
        let mut b = vec![0; 8];
        u32(&mut b, 3);
        b.extend(1e-9_f64.to_le_bytes());
        for n in [8, 8, 8] {
            u32(&mut b, n);
        }
        for knot in [0_f64, 0., 0., 0., 1., 1., 1., 1.] {
            b.extend(knot.to_le_bytes());
        }
        for n in [0, 0, 8, 4, 4, 8] {
            u32(&mut b, n);
        }
        for value in [0_f64, 0., 0., 1., 2., 0., 2., 2., 0., 3., 0., 0.] {
            b.extend(value.to_le_bytes());
        }
        b.extend(1e-12_f64.to_le_bytes());
        b.extend([1, 1]);
        b.extend(0_f64.to_le_bytes());
        b.extend(1_f64.to_le_bytes());
        b.extend([0, 0, 0, 1, 0]);
        b
    }

    fn check(b: Vec<u8>) -> Result<()> {
        let mut fixture = Fixture(BTreeMap::new());
        fixture.put(401, SPLINE, 2, b, None);
        let design = fixture.design();
        let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        validate(&Records::new(&design), &classes, 401)
    }

    #[test]
    fn spline_cache_requires_bounded_consistent_finite_arrays_and_domain() {
        let b = authored();
        check(b.clone()).unwrap();
        for keep in 0..b.len() {
            assert!(check(b[..keep].to_vec()).is_err(), "truncated at {keep}");
        }
        let mut trailing = b.clone();
        trailing.push(0);
        assert!(check(trailing).is_err());
        for (offset, value) in [(8, 0), (8, 17), (20, u32::MAX), (24, 7), (28, 4), (108, 5)] {
            let mut bad = b.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(check(bad).is_err(), "integer at {offset}");
        }
        for (offset, value) in [(12, 0.), (32, f64::NAN), (40, -1.), (120, f64::INFINITY), (226, -1.), (226, 2.), (234, 0.)] {
            let mut bad = b.clone();
            bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert!(check(bad).is_err(), "number at {offset}");
        }
    }
}
