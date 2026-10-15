//! A small form sheet: a bicubic patch (a flat grid or an open cylindrical grid) whose control
//! points can be pulled, returned as a surface body.

use solidcraft_geom::Vec3;
use solidcraft_geom::bspline::NurbsSurface;
use solidcraft_geom::tspline::TSpline;

use crate::{KernelError, Result};

/// `shape` is `box` (a flat grid) or `cylinder` (an open half-cylinder). `pulls` are
/// `(index, delta)` on the 4×4 control grid, index 0 at the origin corner.
pub fn form_sheet(shape: &str, length: f64, width: f64, pulls: &[(u32, Vec3)]) -> Result<crate::Body> {
    if !(length.is_finite() && width.is_finite() && length > 1e-6 && width > 1e-6 && length < 1e6 && width < 1e6) {
        return Err(KernelError::Invalid("form size must be positive".into()));
    }
    let mut pts = Vec::with_capacity(16);
    for j in 0..4 {
        for i in 0..4 {
            let (u, v) = (i as f64 / 3.0, j as f64 / 3.0);
            let p = if shape == "cylinder" {
                let ang = u * std::f64::consts::PI;
                Vec3::new((length * 0.5) * ang.cos(), width * v, (length * 0.5) * ang.sin())
            } else if shape == "box" {
                Vec3::new(length * u, width * v, 0.0)
            } else {
                return Err(KernelError::Invalid("form shape must be box or cylinder".into()));
            };
            pts.push(p);
        }
    }
    for (index, delta) in pulls {
        if !delta.is_finite() {
            return Err(KernelError::Invalid("a form pull must be finite".into()));
        }
        let Some(p) = pts.get_mut(*index as usize) else {
            return Err(KernelError::Invalid("a form pull names a control point that is not on the 4×4 grid".into()));
        };
        *p += *delta;
    }
    let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let weights = vec![1.0; pts.len()];
    let nurbs = NurbsSurface::new([3, 3], [knots.clone(), knots], &pts, &weights)
        .map_err(|e| KernelError::Invalid(format!("form surface: {e}")))?;
    let ts = TSpline::from_nurbs(&nurbs).map_err(|e| KernelError::Invalid(format!("form surface: {e}")))?;
    crate::tspline_sheet(&ts)
}
