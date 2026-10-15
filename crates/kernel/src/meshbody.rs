//! Mesh bodies: triangles without a B-rep (imported 3MF/STL). They render, measure, move and
//! export; B-rep features refuse them.

use std::collections::HashMap;
use std::sync::Arc;

use solidcraft_geom::{Mesh, Vec3};

use crate::body::Body;
use crate::{KernelError, Result};

/// Most triangles in one mesh body.
pub const MAX_TRIANGLES: usize = 20_000_000;

/// Indexed triangles with shared vertices, wound counter-clockwise seen from outside.
#[derive(Clone, Debug, Default)]
pub struct TriMesh {
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

impl TriMesh {
    /// Display/measure mesh: corners split per triangle so normals stay flat.
    pub(crate) fn to_mesh(&self) -> Mesh {
        let mut out = Mesh::default();
        out.positions.reserve(self.triangles.len() * 3);
        for t in &self.triangles {
            let (Some(a), Some(b), Some(c)) =
                (self.positions.get(t[0] as usize), self.positions.get(t[1] as usize), self.positions.get(t[2] as usize))
            else {
                continue;
            };
            let n = (*b - *a).cross(*c - *a).normalized().unwrap_or(Vec3::Z);
            let base = u32::try_from(out.positions.len()).unwrap_or(u32::MAX);
            out.positions.extend([*a, *b, *c]);
            out.normals.extend([n, n, n]);
            out.triangles.push([base, base.saturating_add(1), base.saturating_add(2)]);
            out.tri_face.push(0);
        }
        out
    }

    /// Distinct undirected triangle sides.
    pub(crate) fn edge_count(&self) -> usize {
        let mut set = std::collections::HashSet::new();
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                set.insert((a.min(b), a.max(b)));
            }
        }
        set.len()
    }

    /// Signed volume ×6.
    fn volume6(&self) -> f64 {
        self.triangles
            .iter()
            .filter_map(|t| Some((self.positions.get(t[0] as usize)?, self.positions.get(t[1] as usize)?, self.positions.get(t[2] as usize)?)))
            .map(|(a, b, c)| a.dot(b.cross(*c)))
            .sum()
    }
}

/// A mesh body from triangles (`positions` in mm). Coincident vertices are merged, degenerate
/// triangles dropped, and a closed mesh with inward winding is turned outward.
pub fn mesh_body(positions: &[Vec3], triangles: &[[u32; 3]]) -> Result<Body> {
    if triangles.len() > MAX_TRIANGLES {
        return Err(KernelError::Invalid(format!("mesh with {} triangles (limit {MAX_TRIANGLES})", triangles.len())));
    }
    if positions.iter().any(|p| !(p.is_finite() && p.x.abs() < 1e9 && p.y.abs() < 1e9 && p.z.abs() < 1e9)) {
        return Err(KernelError::Invalid("mesh with non-finite or huge coordinates".into()));
    }
    // Merge vertices that are exactly equal (bit pattern of the coordinates).
    let mut map: HashMap<[u64; 3], u32> = HashMap::new();
    let mut remap: Vec<u32> = Vec::with_capacity(positions.len());
    let mut out = TriMesh::default();
    for p in positions {
        let key = [(p.x + 0.0).to_bits(), (p.y + 0.0).to_bits(), (p.z + 0.0).to_bits()];
        let next = u32::try_from(out.positions.len()).map_err(|_| KernelError::Invalid("too many vertices".into()))?;
        let i = *map.entry(key).or_insert_with(|| {
            out.positions.push(*p);
            next
        });
        remap.push(i);
    }
    for t in triangles {
        let (Some(&a), Some(&b), Some(&c)) = (remap.get(t[0] as usize), remap.get(t[1] as usize), remap.get(t[2] as usize)) else {
            return Err(KernelError::Invalid("mesh triangle refers to a missing vertex".into()));
        };
        if a == b || b == c || c == a {
            continue;
        }
        out.triangles.push([a, b, c]);
    }
    if out.triangles.is_empty() {
        return Err(KernelError::Invalid("mesh has no triangles".into()));
    }
    unify_winding(&mut out);
    if out.volume6() < 0.0 && is_closed(&out) {
        for t in &mut out.triangles {
            t.swap(1, 2);
        }
    }
    Ok(Body {
        solid: Arc::new(crate::body::Solid::new_unchecked(Vec::new())),
        coarse_missing: None,
        mesh: Some(Arc::new(out)),
        color: None,
        paint: None,
        splits: None,
        face_sources: None,
        history: None,
        regularization_protected: false,
    })
}

/// Wind every connected piece one way: a triangle that runs a side in the same direction as
/// its neighbour is turned over (files with stray inverted facets). Sides shared by more than
/// two triangles are left alone.
fn unify_winding(m: &mut TriMesh) {
    let mut sides: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, t) in m.triangles.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            sides.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }
    let runs = |t: &[u32; 3], a: u32, b: u32| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])].contains(&(a, b));
    let mut done = vec![false; m.triangles.len()];
    for start in 0..m.triangles.len() {
        if done[start] {
            continue;
        }
        done[start] = true;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let Some(t) = m.triangles.get(i).copied() else { continue };
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let Some(list) = sides.get(&(a.min(b), a.max(b))) else { continue };
                if list.len() != 2 {
                    continue;
                }
                for &j in list {
                    if j == i || done[j] {
                        continue;
                    }
                    done[j] = true;
                    if let Some(u) = m.triangles.get_mut(j)
                        && runs(u, a, b)
                    {
                        u.swap(1, 2);
                    }
                    stack.push(j);
                }
            }
        }
    }
}

/// Every triangle side is shared by exactly two triangles in opposite directions.
pub(crate) fn is_closed(m: &TriMesh) -> bool {
    let mut count: HashMap<(u32, u32), i32> = HashMap::new();
    for t in &m.triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *count.entry((a, b)).or_default() += 1;
        }
    }
    count.iter().all(|(&(a, b), &n)| n == 1 && count.get(&(b, a)) == Some(&1))
}

impl Body {
    /// Shared-vertex triangles of a mesh body.
    pub fn triangle_mesh(&self) -> Option<(&[Vec3], &[[u32; 3]])> {
        self.mesh.as_ref().map(|m| (m.positions.as_slice(), m.triangles.as_slice()))
    }
    /// Does a mesh body enclose a volume (every side shared by two triangles)?
    pub fn is_closed_mesh(&self) -> bool {
        self.mesh.as_ref().is_some_and(|m| is_closed(m))
    }

    /// A mesh body moved by a column-major affine matrix (reflections keep outward winding).
    pub(crate) fn mesh_transformed(&self, m: &[[f64; 4]; 4]) -> Option<Body> {
        let tm = self.mesh.as_ref()?;
        let f = |p: Vec3| {
            Vec3::new(
                m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0],
                m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1],
                m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2],
            )
        };
        let det = m[0][0] * (m[1][1] * m[2][2] - m[2][1] * m[1][2]) - m[1][0] * (m[0][1] * m[2][2] - m[2][1] * m[0][2])
            + m[2][0] * (m[0][1] * m[1][2] - m[1][1] * m[0][2]);
        let mut out = TriMesh { positions: tm.positions.iter().map(|p| f(*p)).collect(), triangles: tm.triangles.clone() };
        if det < 0.0 {
            for t in &mut out.triangles {
                t.swap(1, 2);
            }
        }
        Some(Body {
            solid: self.solid.clone(),
            coarse_missing: None,
            mesh: Some(Arc::new(out)),
            color: self.color,
            paint: self.paint.clone(),
            splits: self.splits.clone(),
            face_sources: None,
            history: None,
            regularization_protected: self.regularization_protected,
        })
    }
}

/// Keep the part of a mesh on the same side of a plane as `keep`. Triangles that cross the
/// plane are clipped. The result is a mesh body (open, if the cut removed a cap).
pub fn mesh_plane_cut(body: &Body, origin: Vec3, normal: Vec3, keep: Vec3) -> Result<Body> {
    let n = normal.normalized().ok_or_else(|| KernelError::Invalid("the cut plane needs a direction".into()))?;
    let (positions, triangles) = body.triangle_mesh().ok_or_else(|| KernelError::Invalid("select a mesh body".into()))?;
    let side = |p: Vec3| (p - origin).dot(n);
    let keep_pos = side(keep) >= 0.0;
    let mut out_pos = Vec::new();
    let mut out_tri = Vec::new();
    let push = |out_pos: &mut Vec<Vec3>, out_tri: &mut Vec<[u32; 3]>, poly: &[Vec3]| {
        if poly.len() < 3 {
            return;
        }
        let base = u32::try_from(out_pos.len()).unwrap_or(u32::MAX);
        out_pos.extend(poly.iter().copied());
        for k in 1..poly.len() - 1 {
            out_tri.push([base, base.saturating_add(k as u32), base.saturating_add(k as u32 + 1)]);
        }
    };
    for t in triangles {
        let pts: Vec<Vec3> = t.iter().filter_map(|i| positions.get(*i as usize).copied()).collect();
        let [a, b, c] = pts.as_slice() else { continue };
        let tri = [*a, *b, *c];
        let inside = |p: Vec3| (side(p) >= -1e-8) == keep_pos;
        let inn: Vec<Vec3> = tri.into_iter().filter(|p| inside(*p)).collect();
        if inn.len() == 3 {
            push(&mut out_pos, &mut out_tri, &tri);
            continue;
        }
        if inn.is_empty() {
            continue;
        }
        // Walk the boundary and insert intersections.
        let mut poly = Vec::new();
        for i in 0..3 {
            let (p, q) = (tri[i], tri[(i + 1) % 3]);
            let (ip, iq) = (inside(p), inside(q));
            if ip {
                poly.push(p);
            }
            if ip != iq {
                let (sp, sq) = (side(p), side(q));
                let den = sp - sq;
                if den.abs() > 1e-12 {
                    let t = sp / den;
                    if (0.0..=1.0).contains(&t) {
                        poly.push(p + (q - p) * t);
                    }
                }
            }
        }
        push(&mut out_pos, &mut out_tri, &poly);
    }
    if out_tri.is_empty() {
        return Err(KernelError::Invalid("the plane cut removed the whole mesh".into()));
    }
    let mut cut = mesh_body(&out_pos, &out_tri)?;
    cut.color = body.color();
    Ok(cut)
}

/// Drop duplicate and degenerate triangles. A boundary of exactly three edges is filled, which
/// closes a single triangular hole.
pub fn mesh_repair(body: &Body) -> Result<Body> {
    let (positions, triangles) = body.triangle_mesh().ok_or_else(|| KernelError::Invalid("select a mesh body".into()))?;
    let mut tris: Vec<[u32; 3]> = triangles
        .iter()
        .copied()
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0])
        .map(|t| {
            let mut s = t;
            if s[0] > s[1] {
                s.swap(0, 1);
            }
            if s[1] > s[2] {
                s.swap(1, 2);
            }
            if s[0] > s[1] {
                s.swap(0, 1);
            }
            s
        })
        .collect();
    tris.sort();
    tris.dedup();
    let mut edges: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
    for t in &tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let k = (a.min(b), a.max(b));
            *edges.entry(k).or_default() += 1;
        }
    }
    let boundary: Vec<(u32, u32)> = edges.into_iter().filter(|(_, n)| *n == 1).map(|(k, _)| k).collect();
    if boundary.len() == 3 {
        let mut ids = Vec::new();
        for (a, b) in &boundary {
            if !ids.contains(a) {
                ids.push(*a);
            }
            if !ids.contains(b) {
                ids.push(*b);
            }
        }
        if ids.len() == 3 {
            tris.push([ids[0], ids[1], ids[2]]);
        }
    }
    let mut fixed = mesh_body(positions, &tris)?;
    fixed.color = body.color();
    Ok(fixed)
}

/// Collapse nearby vertices so about `ratio` of the triangles remain (0…1).
pub fn mesh_reduce(body: &Body, ratio: f64) -> Result<Body> {
    if !(ratio.is_finite() && ratio > 0.0 && ratio <= 1.0) {
        return Err(KernelError::Invalid("reduce ratio must be within 0…1".into()));
    }
    let (positions, triangles) = body.triangle_mesh().ok_or_else(|| KernelError::Invalid("select a mesh body".into()))?;
    let cell = (body.size() * (1.0 - ratio).max(0.01)).max(1e-4);
    let key = |p: Vec3| {
        (
            (p.x / cell).round() as i64,
            (p.y / cell).round() as i64,
            (p.z / cell).round() as i64,
        )
    };
    let mut map: std::collections::HashMap<(i64, i64, i64), u32> = std::collections::HashMap::new();
    let mut remap = vec![0u32; positions.len()];
    let mut out_pos = Vec::new();
    for (i, p) in positions.iter().enumerate() {
        let k = key(*p);
        let next = u32::try_from(out_pos.len()).unwrap_or(u32::MAX);
        let id = *map.entry(k).or_insert_with(|| {
            out_pos.push(*p);
            next
        });
        if let Some(slot) = remap.get_mut(i) {
            *slot = id;
        }
    }
    let tris: Vec<[u32; 3]> = triangles
        .iter()
        .filter_map(|t| {
            let (a, b, c) = (*remap.get(t[0] as usize)?, *remap.get(t[1] as usize)?, *remap.get(t[2] as usize)?);
            (a != b && b != c && c != a).then_some([a, b, c])
        })
        .collect();
    if tris.is_empty() {
        return Err(KernelError::Invalid("reducing the mesh removed every triangle".into()));
    }
    let mut reduced = mesh_body(&out_pos, &tris)?;
    reduced.color = body.color();
    Ok(reduced)
}

/// Group triangles by which way their normal points (the six axis directions).
pub fn mesh_face_groups(body: &Body) -> Result<Vec<(String, usize)>> {
    let (positions, triangles) = body.triangle_mesh().ok_or_else(|| KernelError::Invalid("select a mesh body".into()))?;
    let mut counts = [0usize; 6];
    for t in triangles {
        let (Some(a), Some(b), Some(c)) =
            (positions.get(t[0] as usize), positions.get(t[1] as usize), positions.get(t[2] as usize))
        else {
            continue;
        };
        let n = (*b - *a).cross(*c - *a);
        let (ax, i) = [n.x.abs(), n.y.abs(), n.z.abs()].into_iter().enumerate().max_by(|u, v| u.1.total_cmp(&v.1)).unwrap_or((2, 0.0));
        let _ = i;
        let sign = match ax {
            0 => n.x >= 0.0,
            1 => n.y >= 0.0,
            _ => n.z >= 0.0,
        };
        let slot = ax * 2 + usize::from(!sign);
        if let Some(c) = counts.get_mut(slot) {
            *c += 1;
        }
    }
    let names = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];
    Ok(names.into_iter().zip(counts).filter(|(_, n)| *n > 0).map(|(n, c)| (n.to_string(), c)).collect())
}

/// One planar B-rep face per triangle. The mesh must be a closed triangle solid small enough to sew.
pub fn mesh_faceted(body: &Body) -> Result<Body> {
    let (positions, triangles) = body.triangle_mesh().ok_or_else(|| KernelError::Invalid("select a mesh body".into()))?;
    if triangles.len() > 4000 {
        return Err(KernelError::Invalid("a faceted conversion is limited to 4000 triangles".into()));
    }
    let mut faces = Vec::new();
    for t in triangles {
        let (Some(a), Some(b), Some(c)) =
            (positions.get(t[0] as usize).copied(), positions.get(t[1] as usize).copied(), positions.get(t[2] as usize).copied())
        else {
            continue;
        };
        if (b - a).cross(c - a).len() < 1e-8 {
            continue;
        }
        faces.push(crate::FaceSpec {
            loops: vec![vec![
                crate::EdgeSpec::Line { a, b },
                crate::EdgeSpec::Line { a: b, b: c },
                crate::EdgeSpec::Line { a: c, b: a },
            ]],
            surface: crate::SurfSpec::Plane,
            probe: None,
        });
    }
    if faces.is_empty() {
        return Err(KernelError::Invalid("the mesh has no triangles to convert".into()));
    }
    let mut solid = crate::sew(&faces)?;
    solid = solid.with_color(body.color());
    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_geom::Vec3;

    #[test]
    fn a_plane_cut_keeps_one_side_and_groups_point_along_axes() {
        let box_mesh = crate::box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap().tessellate(0.05).unwrap();
        let body = mesh_body(&box_mesh.positions, &box_mesh.triangles).unwrap();
        let before = body.triangle_mesh().unwrap().1.len();
        let cut = mesh_plane_cut(&body, Vec3::new(5.0, 0.0, 0.0), Vec3::X, Vec3::new(8.0, 5.0, 5.0)).unwrap();
        let (pos, tris) = cut.triangle_mesh().unwrap();
        assert!(!tris.is_empty() && pos.iter().all(|p| p.x >= 4.9), "kept the +X side, {} tris", tris.len());
        let groups = mesh_face_groups(&body).unwrap();
        assert!(groups.iter().any(|(n, c)| n == "+Z" && *c > 0));
        assert!(mesh_repair(&body).is_ok());
        let reduced = mesh_reduce(&body, 0.2).unwrap();
        assert!(reduced.triangle_mesh().unwrap().1.len() <= before);
    }
}
