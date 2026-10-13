//! Display tessellation: each face's triangulation in its own parameters, with every edge
//! sampled once so neighbouring faces share their boundary points exactly, handed over as a
//! [`TessPlan`] for the CPU or GPU evaluator.
//!
//! Per face, cheapest first:
//! - planar faces: a constrained Delaunay triangulation of the edge nodes alone (a plane needs
//!   no interior points);
//! - faces whose boundary runs along their parameter box (cylinder halves, sphere and torus
//!   patches, fillets): a structured grid zipped to the edge nodes;
//! - anything else (trimmed faces, holes): constrained Delaunay with interior seeds and
//!   Ruppert refinement.
//!
//! A face the planner cannot handle (its surface has no evaluable form, a boundary that does
//! not project or crosses itself) fails the whole plan, and the caller meshes the body the
//! old way.

pub(crate) mod cdt;
pub(crate) mod edges;
pub(crate) mod grid;
pub(crate) mod surface;
pub(crate) mod uv;

use std::collections::HashMap;

use solidcraft_geom::Vec3;
use solidcraft_geom::tess::{EvalSurface, FacePlan, TessPlan};
use truck_modeling as mt;

use crate::body::{Solid, edge_order};
use crate::{KernelError, Result};

/// How the faces of a plan were meshed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanStats {
    pub planar: usize,
    pub structured: usize,
    pub delaunay: usize,
}

/// Most lines a curved face's grid gets in one direction.
const MAX_LINES: usize = 1024;
/// Most points Ruppert refinement may add to one face, beyond its seeds.
const MAX_REFINE: usize = 20_000;

fn unsupported(what: impl std::fmt::Display) -> KernelError {
    KernelError::Failed(format!("not supported yet: tessellation: {what}"))
}

/// Plan the display tessellation of `solid` with chord tolerance `tol`.
pub(crate) fn plan(solid: &Solid, tol: f64) -> Result<(TessPlan, PlanStats)> {
    let faces: Vec<&mt::Face> = solid.face_iter().collect();
    let (edges, edge_faces) = edge_order(faces.iter().copied());
    let mut nodes: HashMap<_, Vec<Vec3>> = HashMap::with_capacity(edges.len());
    for e in &edges {
        nodes.entry(e.id()).or_insert_with(|| edges::sample_edge(e, tol));
    }
    let oriented = |e: &mt::Edge| -> Option<Vec<Vec3>> {
        let mut pts = nodes.get(&e.id())?.clone();
        if !e.orientation() {
            pts.reverse();
        }
        Some(pts)
    };
    let mut plan = TessPlan::default();
    let mut stats = PlanStats::default();
    for (fi, face) in faces.iter().enumerate() {
        let surface = face.surface();
        let eval = surface::eval_surface(&surface).ok_or_else(|| unsupported(format!("face {fi}: surface not evaluable")))?;
        // Boundary nodes, loop by loop (each edge's last node is the next edge's first).
        let mut loops3: Vec<Vec<Vec3>> = Vec::new();
        for wire in face.absolute_boundaries() {
            let mut pts = Vec::new();
            for e in wire.edge_iter() {
                let mut ep = oriented(e).ok_or_else(|| unsupported("edge without nodes"))?;
                ep.pop();
                pts.extend(ep);
            }
            if pts.len() >= 2 {
                loops3.push(pts);
            }
        }
        let fp = plan_face(fi, &eval, &loops3, tol, &mut stats)?;
        let fp = FacePlan {
            surface: Some(u32::try_from(plan.surfaces.len()).map_err(|_| unsupported("too many faces"))?),
            flip: !face.orientation(),
            triangles: if face.orientation() { fp.triangles } else { fp.triangles.into_iter().map(|[a, b, c]| [a, c, b]).collect() },
            ..fp
        };
        plan.surfaces.push(eval);
        plan.faces.push(fp);
    }
    plan.edges = edges.iter().filter_map(oriented).collect();
    plan.edge_faces = edge_faces.into_iter().map(|fs| fs.into_iter().map(|f| u32::try_from(f).unwrap_or(u32::MAX)).collect()).collect();
    Ok((plan, stats))
}

/// One face: boundary loops in 3D (shared edge nodes) to parameters and triangles.
fn plan_face(fi: usize, s: &EvalSurface, loops3: &[Vec<Vec3>], tol: f64, stats: &mut PlanStats) -> Result<FacePlan> {
    let dom = uv::domain(s);
    let accept = (tol * 10.0).max(1e-6);
    let fail = |why: &str| unsupported(format!("face {fi}: {why}"));
    // Project every loop, unwrapping periodic parameters along it and splitting poles.
    let mut fixed: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<[f64; 2]> = Vec::new();
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for pts in loops3 {
        let mut luv: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
        let mut prev: Option<[f64; 2]> = None;
        for p in pts {
            let mut q = uv::project(s, &dom, *p, prev, accept).ok_or_else(|| fail("boundary does not lie on the surface"))?;
            if let Some(pr) = prev {
                for d in 0..2 {
                    q[d] = uv::unwrap(q[d], pr[d], dom.period[d]);
                }
            }
            luv.push(q);
            prev = Some(q);
        }
        // A loop that winds once around a periodic direction does not close in (u, v).
        if let (Some(first), Some(last)) = (luv.first(), luv.last()) {
            for d in 0..2 {
                if let Some(per) = dom.period[d]
                    && (uv::unwrap(first[d], last[d], Some(per)) - first[d]).abs() > per * 0.25
                {
                    return Err(fail("boundary winds around the surface"));
                }
            }
        }
        let start = uvs.len();
        let n = luv.len();
        for (k, (q, p)) in luv.iter().zip(pts).enumerate() {
            // At a pole the parameter along the degenerate direction is free: one copy with
            // the previous node's value, one with the next node's.
            let (prev, next) = (luv.get((k + n - 1) % n).copied(), luv.get((k + 1) % n).copied());
            let pole = (0..2).find(|&d| uv::degenerate(s, *q, d));
            match (pole, prev, next) {
                (Some(d), Some(a), Some(b)) => {
                    let mut qa = *q;
                    let mut qb = *q;
                    qa[d] = a[d];
                    qb[d] = b[d];
                    uvs.push(qa);
                    fixed.push(*p);
                    if (qa[d] - qb[d]).abs() > 1e-12 {
                        uvs.push(qb);
                        fixed.push(*p);
                    }
                }
                _ => {
                    uvs.push(*q);
                    fixed.push(*p);
                }
            }
        }
        ranges.push(start..uvs.len());
    }
    if uvs.len() < 3 {
        return Err(fail("degenerate boundary"));
    }
    let planar = matches!(s, EvalSurface::Plane { .. });
    let (extra, triangles) = if planar {
        stats.planar += 1;
        cdt::triangulate(&uvs, &ranges, cdt::Fill { seeds: &[], refine: None }).ok_or_else(|| fail("boundary crosses itself"))?
    } else {
        let mut bbox = [(f64::MAX, f64::MIN); 2];
        for p in &uvs {
            for d in 0..2 {
                bbox[d] = (bbox[d].0.min(p[d]), bbox[d].1.max(p[d]));
            }
        }
        let mut us = uv::divide(s, 0, bbox[0], bbox[1], tol, MAX_LINES);
        let mut vs = uv::divide(s, 1, bbox[1], bbox[0], tol, MAX_LINES);
        thin(&mut us, &mut vs, grid::MAX_FACE_GRID);
        let structured = match (ranges.len(), grid::quad(&uvs)) {
            (1, Some(q)) => {
                let lines = |d: usize, div: &Vec<f64>| grid::aligned_lines(&uvs, &q, d).unwrap_or_else(|| grid::interior(div.clone(), bbox[d]));
                grid::structured(&uvs, &q, &lines(0, &us), &lines(1, &vs))
            }
            _ => None,
        };
        match structured {
            Some(r) => {
                stats.structured += 1;
                r
            }
            None => {
                stats.delaunay += 1;
                let polys: Vec<&[[f64; 2]]> = ranges.iter().filter_map(|r| uvs.get(r.clone())).collect();
                let seeds: Vec<[f64; 2]> = vs
                    .iter()
                    .filter(|v| **v > bbox[1].0 && **v < bbox[1].1)
                    .flat_map(|&v| us.iter().filter(|u| **u > bbox[0].0 && **u < bbox[0].1).map(move |&u| [u, v]))
                    .filter(|p| cdt::inside(*p, &polys))
                    .collect();
                let eval = |u: f64, v: f64| s.derivs(u, v).map(|(p, _, _)| p);
                // One unit per tolerance cell in each direction: a straight direction (one
                // cell) keeps long triangles, Ruppert shapes them in that metric.
                let cells = |lines: &Vec<f64>, (a, b): (f64, f64)| (lines.len().saturating_sub(1).max(1) as f64) / (b - a).max(f64::MIN_POSITIVE);
                let sag = cdt::Sag { eval: &eval, tol, scale: [cells(&us, bbox[0]), cells(&vs, bbox[1])] };
                let fill = cdt::Fill { seeds: &seeds, refine: Some((sag, MAX_REFINE)) };
                cdt::triangulate(&uvs, &ranges, fill).ok_or_else(|| fail("boundary crosses itself"))?
            }
        }
    };
    if !closed_boundary(&ranges, &triangles) {
        return Err(fail("triangulation does not follow the boundary"));
    }
    let mut all = uvs;
    all.extend(extra);
    Ok(FacePlan { face: u32::try_from(fi).unwrap_or(u32::MAX), surface: None, uv: all, fixed, normals: Vec::new(), flip: false, triangles })
}

/// Every boundary segment (consecutive nodes of a loop, skipping pole copies of one node) is
/// the side of exactly one triangle: the face covers its boundary with nothing missing or
/// doubled, so it meets its neighbours exactly.
fn closed_boundary(ranges: &[std::ops::Range<usize>], triangles: &[[u32; 3]]) -> bool {
    let mut sides: HashMap<(u32, u32), u32> = HashMap::with_capacity(triangles.len() * 3);
    for t in triangles {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            *sides.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    ranges.iter().all(|r| {
        let n = r.len();
        (0..n).all(|k| {
            let (a, b) = ((r.start + k) as u32, (r.start + (k + 1) % n) as u32);
            sides.get(&(a.min(b), a.max(b))).copied() == Some(1)
        })
    })
}

/// Keep at most `cap` grid points by taking every k-th line in both directions (ends kept).
fn thin(us: &mut Vec<f64>, vs: &mut Vec<f64>, cap: usize) {
    let total = us.len().saturating_mul(vs.len());
    if total <= cap {
        return;
    }
    let f = (total as f64 / cap.max(4) as f64).sqrt();
    let pick = |d: &mut Vec<f64>| {
        let n = ((d.len() as f64 / f).ceil() as usize).max(2);
        if d.len() > n {
            let len = d.len();
            *d = (0..n).filter_map(|i| d.get(i * (len - 1) / (n - 1)).copied()).collect();
        }
    };
    pick(us);
    pick(vs);
}

#[cfg(test)]
mod tests;
