//! A small form sheet: a bicubic patch (a flat grid or an open cylindrical grid) whose control
//! points can be pulled, returned as a surface body.

use solidcraft_geom::Vec3;
use solidcraft_geom::bspline::NurbsSurface;
use solidcraft_geom::tspline::TSpline;

use crate::{KernelError, Result};

/// How a 4×4 form cage is edited before it becomes a sheet.
#[derive(Clone, Debug, Default)]
pub struct FormBuild {
    /// `(index, delta)` on the 4×4 grid, index 0 at the origin corner.
    pub pulls: Vec<(u32, Vec3)>,
    /// Control-point indices whose weights are raised so the edge stays sharper.
    pub creases: Vec<u32>,
    /// Grid edges (0..24) whose endpoints are moved apart.
    pub separate_edges: Vec<u32>,
    /// Midpoint-refinement passes, at most 6. Each pass turns an n×n cage into (2n−1)×(2n−1).
    pub subdiv: u32,
    /// `yz`, `xz` or `xy`.
    pub symmetry: Option<String>,
}

const MAX_CAGE_POINTS: usize = 20_000;

/// Control points after `level` midpoint refinements of the 4×4 cage.
pub fn cage_points(level: u32) -> Result<usize> {
    if level > 6 {
        return Err(KernelError::Invalid("subdivision level is at most 6".into()));
    }
    let mut n = 4usize;
    for _ in 0..level {
        n = n.saturating_mul(2).saturating_sub(1);
    }
    let count = n.saturating_mul(n);
    if count > MAX_CAGE_POINTS {
        return Err(KernelError::Invalid("the form cage is larger than 20000 points".into()));
    }
    Ok(count)
}

/// `shape` is `box` or `plane` (a flat grid), `cylinder` (an open half-cylinder), `sphere`,
/// `quadball` or `torus`. `length` and `width` are the cage size in millimetres.
pub fn form_sheet(shape: &str, length: f64, width: f64, build: &FormBuild) -> Result<crate::Body> {
    if !(length.is_finite() && width.is_finite() && length > 1e-6 && width > 1e-6 && length < 1e6 && width < 1e6) {
        return Err(KernelError::Invalid("form size must be positive".into()));
    }
    if build.subdiv > 6 {
        return Err(KernelError::Invalid("subdivision level is at most 6".into()));
    }
    let mut pts = Vec::with_capacity(16);
    for j in 0..4 {
        for i in 0..4 {
            let (u, v) = (i as f64 / 3.0, j as f64 / 3.0);
            pts.push(cage_point(shape, length, width, u, v)?);
        }
    }
    for (index, delta) in &build.pulls {
        if !delta.is_finite() {
            return Err(KernelError::Invalid("a form pull must be finite".into()));
        }
        let Some(p) = pts.get_mut(*index as usize) else {
            return Err(KernelError::Invalid("a form pull names a control point that is not on the 4×4 grid".into()));
        };
        *p += *delta;
    }
    for edge in &build.separate_edges {
        let (a, b) = edge_ends(*edge)?;
        let (Some(pa), Some(pb)) = (pts.get(a).copied(), pts.get(b).copied()) else {
            return Err(KernelError::Invalid("a form edge is outside the 4×4 grid".into()));
        };
        let mid = (pa + pb) * 0.5;
        if let Some(p) = pts.get_mut(a) {
            *p += (*p - mid) * 0.15;
        }
        if let Some(p) = pts.get_mut(b) {
            *p += (*p - mid) * 0.15;
        }
    }
    if let Some(plane) = build.symmetry.as_deref() {
        symmetrize(&mut pts, plane)?;
    }
    let mut weights = vec![1.0; pts.len()];
    for index in &build.creases {
        let Some(w) = weights.get_mut(*index as usize) else {
            return Err(KernelError::Invalid("a crease names a control point that is not on the 4×4 grid".into()));
        };
        *w = 5.0;
    }
    for _ in 0..build.subdiv {
        (pts, weights) = refine(&pts, &weights)?;
    }
    let n = (pts.len() as f64).sqrt() as usize;
    if n * n != pts.len() {
        return Err(KernelError::Invalid("the form cage is not a square grid".into()));
    }
    let knots = clamped_knots(n)?;
    let nurbs = NurbsSurface::new([3, 3], [knots.clone(), knots], &pts, &weights).map_err(|e| KernelError::Invalid(format!("form surface: {e}")))?;
    let ts = TSpline::from_nurbs(&nurbs).map_err(|e| KernelError::Invalid(format!("form surface: {e}")))?;
    crate::tspline_sheet(&ts)
}

fn cage_point(shape: &str, length: f64, width: f64, u: f64, v: f64) -> Result<Vec3> {
    let p = match shape {
        "box" | "plane" => Vec3::new(length * u, width * v, 0.0),
        "cylinder" => {
            let ang = u * std::f64::consts::PI;
            Vec3::new((length * 0.5) * ang.cos(), width * v, (length * 0.5) * ang.sin())
        }
        "sphere" | "quadball" => {
            let r = length * 0.5;
            let theta = if shape == "quadball" { (u - 0.5) * std::f64::consts::PI } else { u * std::f64::consts::TAU };
            let phi = v * std::f64::consts::PI;
            Vec3::new(r * phi.sin() * theta.cos(), r * phi.sin() * theta.sin(), r * phi.cos())
        }
        "torus" => {
            let major = length * 0.5;
            let minor = (width * 0.25).min(major * 0.5);
            let theta = u * std::f64::consts::TAU;
            let phi = v * std::f64::consts::TAU;
            Vec3::new((major + minor * phi.cos()) * theta.cos(), (major + minor * phi.cos()) * theta.sin(), minor * phi.sin())
        }
        _ => return Err(KernelError::Invalid("form shape must be box, plane, cylinder, sphere, quadball or torus".into())),
    };
    Ok(p)
}

/// Endpoints of one of the 24 edges of a 4×4 grid.
pub fn edge_ends(edge: u32) -> Result<(usize, usize)> {
    if edge < 12 {
        let j = (edge / 3) as usize;
        let i = (edge % 3) as usize;
        return Ok((j * 4 + i, j * 4 + i + 1));
    }
    if edge < 24 {
        let e = edge - 12;
        let col = (e / 3) as usize;
        let row = (e % 3) as usize;
        return Ok((row * 4 + col, (row + 1) * 4 + col));
    }
    Err(KernelError::Invalid("a form edge is 0…23".into()))
}

fn clamped_knots(n: usize) -> Result<Vec<f64>> {
    if !(4..=400).contains(&n) {
        return Err(KernelError::Invalid("form cage size".into()));
    }
    let interior = n - 4;
    let mut knots = Vec::with_capacity(n + 4);
    knots.extend(std::iter::repeat_n(0.0, 4));
    let steps = interior as f64 + 1.0;
    for i in 1..=interior {
        knots.push(i as f64 / steps);
    }
    knots.extend(std::iter::repeat_n(1.0, 4));
    Ok(knots)
}

/// One midpoint pass: even lattice sites keep the old points, odd sites are edge and face centres.
fn refine(pts: &[Vec3], weights: &[f64]) -> Result<(Vec<Vec3>, Vec<f64>)> {
    let n = (pts.len() as f64).sqrt() as usize;
    if n < 2 || n * n != pts.len() || weights.len() != pts.len() {
        return Err(KernelError::Invalid("the form cage is not a square grid".into()));
    }
    let m = n.saturating_mul(2).saturating_sub(1);
    if m.saturating_mul(m) > MAX_CAGE_POINTS {
        return Err(KernelError::Invalid("the form cage is larger than 20000 points".into()));
    }
    let mut out = vec![Vec3::ZERO; m * m];
    let mut wout = vec![1.0; m * m];
    let at = |i: usize, j: usize| pts[j * n + i];
    let wt = |i: usize, j: usize| weights[j * n + i];
    for j in 0..n {
        for i in 0..n {
            out[(j * 2) * m + i * 2] = at(i, j);
            wout[(j * 2) * m + i * 2] = wt(i, j);
        }
    }
    for j in 0..n {
        for i in 0..n - 1 {
            out[(j * 2) * m + i * 2 + 1] = (at(i, j) + at(i + 1, j)) * 0.5;
            wout[(j * 2) * m + i * 2 + 1] = (wt(i, j) + wt(i + 1, j)) * 0.5;
        }
    }
    for j in 0..n - 1 {
        for i in 0..n {
            out[(j * 2 + 1) * m + i * 2] = (at(i, j) + at(i, j + 1)) * 0.5;
            wout[(j * 2 + 1) * m + i * 2] = (wt(i, j) + wt(i, j + 1)) * 0.5;
        }
    }
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            out[(j * 2 + 1) * m + i * 2 + 1] = (at(i, j) + at(i + 1, j) + at(i, j + 1) + at(i + 1, j + 1)) * 0.25;
            wout[(j * 2 + 1) * m + i * 2 + 1] = (wt(i, j) + wt(i + 1, j) + wt(i, j + 1) + wt(i + 1, j + 1)) * 0.25;
        }
    }
    Ok((out, wout))
}

fn symmetrize(pts: &mut [Vec3], plane: &str) -> Result<()> {
    match plane {
        "yz" => {
            for j in 0..4 {
                for i in 0..2 {
                    let a = j * 4 + i;
                    let b = j * 4 + (3 - i);
                    let avg = (pts[a] + Vec3::new(-pts[b].x, pts[b].y, pts[b].z)) * 0.5;
                    pts[a] = avg;
                    pts[b] = Vec3::new(-avg.x, avg.y, avg.z);
                }
            }
        }
        "xz" => {
            for i in 0..4 {
                for j in 0..2 {
                    let a = j * 4 + i;
                    let b = (3 - j) * 4 + i;
                    let avg = (pts[a] + Vec3::new(pts[b].x, -pts[b].y, pts[b].z)) * 0.5;
                    pts[a] = avg;
                    pts[b] = Vec3::new(avg.x, -avg.y, avg.z);
                }
            }
        }
        "xy" => {
            for p in pts.iter_mut() {
                p.z = 0.0;
            }
        }
        _ => return Err(KernelError::Invalid("symmetry plane must be yz, xz or xy".into())),
    }
    Ok(())
}

/// A ruled sheet through two polylines, sampled onto the same 4×4 sheet.
pub fn ruled_sheet(a: &[Vec3], b: &[Vec3]) -> Result<crate::Body> {
    if a.len() < 2 || b.len() < 2 {
        return Err(KernelError::Invalid("a ruled surface needs two curves".into()));
    }
    let sample = |poly: &[Vec3]| -> Vec<Vec3> {
        (0..4)
            .map(|i| {
                let t = i as f64 / 3.0 * (poly.len() - 1) as f64;
                let i0 = (t.floor() as usize).min(poly.len() - 1);
                let i1 = (i0 + 1).min(poly.len() - 1);
                let f = t - i0 as f64;
                poly[i0] * (1.0 - f) + poly[i1] * f
            })
            .collect()
    };
    let (sa, sb) = (sample(a), sample(b));
    let mut pts = Vec::with_capacity(16);
    for row in 0..4 {
        let t = row as f64 / 3.0;
        for col in 0..4 {
            pts.push(sa[col] * (1.0 - t) + sb[col] * t);
        }
    }
    let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let weights = vec![1.0; pts.len()];
    let nurbs = NurbsSurface::new([3, 3], [knots.clone(), knots], &pts, &weights).map_err(|e| KernelError::Invalid(format!("ruled surface: {e}")))?;
    let ts = TSpline::from_nurbs(&nurbs).map_err(|e| KernelError::Invalid(format!("ruled surface: {e}")))?;
    crate::tspline_sheet(&ts)
}
