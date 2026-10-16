//! Shared section parameter for the verified three-circle interpolation mode.
use crate::{KernelError, Result};
use solidcraft_geom::{Loop2, Plane, Seg2, Vec3};

fn invalid() -> KernelError {
    KernelError::Invalid("circular chord loft requires three complete coaxial circles with a monotone axial span".into())
}

fn circle(plane: &Plane, lp: &Loop2) -> Result<(Vec3, f64)> {
    let Some(Seg2::Arc { center, radius, .. }) = lp.segs.first() else { return Err(invalid()) };
    if !radius.is_finite() || *radius <= 1e-7 || *radius > 1e9 || lp.segs.len() > 4096 {
        return Err(invalid());
    }
    let tolerance = 1e-8 * radius.max(1.0);
    let mut sweep = 0.0;
    for (i, segment) in lp.segs.iter().enumerate() {
        let Seg2::Arc { center: c, radius: r, start: a, sweep: s } = segment else { return Err(invalid()) };
        let next = lp.segs.get((i + 1) % lp.segs.len()).ok_or_else(invalid)?;
        if !c.is_finite()
            || !r.is_finite()
            || !a.is_finite()
            || !s.is_finite()
            || c.dist(*center) > tolerance
            || (r - radius).abs() > tolerance
            || s.abs() <= 1e-12
            || s.abs() > std::f64::consts::TAU + 1e-10
            || (sweep != 0.0 && sweep * s < 0.0)
            || segment.end().dist(next.start()) > tolerance
        {
            return Err(invalid());
        }
        sweep += s;
    }
    let world = plane.to_world(*center);
    if !world.is_finite()
        || world.len() > 1e9
        || (sweep.abs() - std::f64::consts::TAU).abs() > 1e-8
        || !plane.x.is_finite()
        || !plane.y.is_finite()
        || (plane.x.len() - 1.0).abs() > 1e-9
        || (plane.y.len() - 1.0).abs() > 1e-9
        || plane.x.dot(plane.y).abs() > 1e-9
    {
        return Err(invalid());
    }
    Ok((world, *radius))
}

pub(crate) fn middle_parameter(sections: &[&(Plane, Loop2)]) -> Result<f64> {
    let [a, b, c] = sections else { return Err(invalid()) };
    let (ca, ra) = circle(&a.0, &a.1)?;
    let (cb, rb) = circle(&b.0, &b.1)?;
    let (cc, rc) = circle(&c.0, &c.1)?;
    let axis = (cc - ca).normalized().ok_or_else(invalid)?;
    let z1 = (cb - ca).dot(axis);
    let z2 = (cc - ca).dot(axis);
    if sections.iter().any(|s| s.0.normal().cross(axis).len() > 1e-9) || (cb - ca - axis * z1).len() > 1e-7 || z1 <= 1e-7 || z2 - z1 <= 1e-7 {
        return Err(invalid());
    }
    let left = z1.hypot(rb - ra);
    let right = (z2 - z1).hypot(rc - rb);
    let t = left / (left + right);
    if !t.is_finite() || !(1e-6..=1.0 - 1e-6).contains(&t) {
        return Err(invalid());
    }
    let denominator = 2.0 * t * (1.0 - t);
    let control_z = (z1 - t * t * z2) / denominator;
    let control_r = (rb - (1.0 - t).powi(2) * ra - t * t * rc) / denominator;
    // Positive radial controls and positive endpoint axial derivatives prevent
    // an intermediate pinch or a folded/reversing side surface.
    if !control_z.is_finite() || !control_r.is_finite() || control_z <= 1e-7 || control_z >= z2 - 1e-7 || control_r <= 1e-7 || control_r > 1e9 {
        return Err(invalid());
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_geom::Vec2;

    fn sections() -> Vec<(Plane, Loop2)> {
        [(0., 7.), (23., 16.), (71., 11.)]
            .into_iter()
            .map(|(z, r)| (Plane { origin: Vec3::new(0., 0., z), ..Plane::XY }, Loop2::circle(Vec2::ZERO, r)))
            .collect()
    }

    #[test]
    fn chord_parameter_is_shared_and_reverses_with_sections() {
        let mut sections = sections();
        let t = middle_parameter(&sections.iter().collect::<Vec<_>>()).unwrap();
        let expected = 23_f64.hypot(9.) / (23_f64.hypot(9.) + 48_f64.hypot(5.));
        assert!((t - expected).abs() < 1e-14);
        sections.reverse();
        assert!((middle_parameter(&sections.iter().collect::<Vec<_>>()).unwrap() - (1. - t)).abs() < 1e-14);
    }

    #[test]
    fn circular_chord_volume_and_bounds_match_polynomial_integrals() {
        // Independent integral of pi * r(u)^2 * z'(u), with original dimensions.
        // The legacy uniform interpolant intentionally has a different volume.
        for (chord, expected) in [(true, 15954.222488679663), (false, 14451.8)] {
            for reversed in [false, true] {
                let mut sections = sections();
                if reversed {
                    sections.reverse();
                }
                let body = if chord { crate::loft_circular_chord(&sections) } else { crate::loft_uniform(&sections) }.unwrap();
                let m = crate::measure(&body).unwrap();
                let volume = expected * std::f64::consts::PI;
                assert!((m.volume - volume).abs() < volume * 1e-3, "{chord}/{reversed}: {} != {volume}", m.volume);
                assert!((m.bbox.min.z - 0.).abs() < 1e-7 && (m.bbox.max.z - 71.).abs() < 1e-7);
            }
        }
    }

    #[test]
    fn unsupported_or_degenerate_sections_do_not_get_a_ruled_fallback() {
        let baseline = sections();
        assert!(crate::loft_circular_chord(&baseline[..2]).is_err());
        for variant in 0..7 {
            let mut s = baseline.clone();
            match variant {
                0 => s[1].0.origin.x = 0.1,
                1 => s[1].0.origin.z = 0.,
                2 => s[1].1 = Loop2::polygon(&[Vec2::ZERO, Vec2::new(1., 0.), Vec2::new(0., 1.)]),
                3 => s[1].0.x = Vec3::ZERO,
                4 => s[1].1.segs.pop().map(|_| ()).unwrap(),
                5 => {
                    let segment = s[1].1.segs[0];
                    s[1].1.segs.push(segment);
                }
                _ => s[1].0.origin.z = f64::NAN,
            }
            assert!(crate::loft_circular_chord(&s).is_err(), "variant {variant}");
        }
    }
}
