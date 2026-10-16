//! Finite edge/face incidence graphs. Names and operation tags identify faces;
//! endpoint signatures distinguish intersections of the same incident surfaces.
use super::history::{finish, payload, require_class, zeros};
use super::*;

pub(super) type Term = BTreeSet<(u64, i32)>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Vertex {
    pub faces: BTreeSet<Term>,
    pub seams: BTreeSet<Term>,
    pub(super) signatures: Vec<[u32; 8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Edge {
    pub faces: [Term; 2],
    /// A closed carrier has one seam vertex; an open span has two endpoints.
    pub ends: Vec<Vertex>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub terms: Vec<Term>,
    pub edges: Vec<Edge>,
    /// Retained qualifiers from old tracking records, never live face owners.
    pub retired_modifiers: BTreeSet<(u64, i32, u64)>,
}

fn integer(r: &mut Reader<'_>, expected: i32) -> Result<()> {
    if r.u32()? != expected as u32 {
        return Err(invalid("unsupported incidence framing"));
    }
    Ok(())
}

fn term(r: &mut Reader<'_>, terms: &[Term]) -> Result<Term> {
    let index = r.u32()? as usize;
    integer(r, -1)?;
    terms.get(index).cloned().ok_or_else(|| invalid("incidence references an absent face term"))
}

fn vertex(r: &mut Reader<'_>, terms: &[Term], incident: &[Term; 2], closed: bool) -> Result<(Vertex, Vec<Term>)> {
    integer(r, -1)?;
    let count = r.count(8)?;
    if (closed && count != 2) || (!closed && !(3..=64).contains(&count)) {
        return Err(invalid("unsupported finite-edge vertex degree"));
    }
    let mut faces = BTreeSet::new();
    let mut cycle = Vec::new();
    for _ in 0..count {
        let face = term(r, terms)?;
        cycle.push(face.clone());
        if face.iter().any(|(_, label)| *label <= 0) || !faces.insert(face) {
            return Err(invalid("duplicate or virtual incident face"));
        }
    }
    if incident.iter().any(|face| !faces.contains(face)) {
        return Err(invalid("edge endpoints disagree with incident faces"));
    }
    let count = r.count(20)?;
    if count > 1 {
        return Err(invalid("unsupported vertex seam count"));
    }
    let mut seams = BTreeSet::new();
    for _ in 0..count {
        let seam = term(r, terms)?;
        if seam.len() != 1 || seam.iter().any(|(_, label)| *label >= 0) || !seams.insert(seam) {
            return Err(invalid("invalid vertex seam"));
        }
        integer(r, 0)?;
        integer(r, 0)?;
        integer(r, -1)?;
    }
    let count = r.count(32)?;
    if count > 64 {
        return Err(invalid("vertex signature work limit exceeded"));
    }
    let mut signatures = Vec::new();
    for _ in 0..count {
        let mut signature = [0; 8];
        for value in &mut signature {
            *value = r.u32()?;
        }
        // These values are opaque intersection identities, not geometric picks
        // or allocation sizes. Retain them exactly for endpoint agreement.
        signatures.push(signature);
    }
    Ok((Vertex { faces, seams, signatures }, cycle))
}

// A three-face endpoint gives an oriented cycle around a finite edge. The
// opposite endpoint must reverse that cycle. Ordinary topology remains usable
// without this extra evidence; asymmetric chamfers require it explicitly.
fn primary_face(faces: &[Term; 2], cycles: &[Vec<Term>]) -> Option<Term> {
    let [a, b] = cycles else { return None };
    let forward = |cycle: &[Term]| {
        if cycle.len() != 3 {
            return None;
        }
        let at = cycle.iter().position(|f| f == &faces[0])?;
        Some(cycle.get((at + 1) % 3) == Some(&faces[1]))
    };
    let first = forward(a)?;
    if first == forward(b)? {
        return None;
    }
    faces.get(usize::from(first)).cloned()
}

fn edge(r: &mut Reader<'_>, terms: &[Term]) -> Result<(Edge, Option<Term>)> {
    let seam = r.u32()?;
    if seam != u32::MAX && terms.get(seam as usize).is_none_or(|t| t.len() != 1 || t.iter().any(|(_, label)| *label >= 0)) {
        return Err(invalid("invalid circular-edge seam term"));
    }
    integer(r, -1)?;
    integer(r, 2)?;
    let faces = [term(r, terms)?, term(r, terms)?];
    if faces[0] == faces[1] || faces.iter().any(|t| t.iter().any(|(_, label)| *label <= 0)) {
        return Err(invalid("invalid finite-edge face pair"));
    }
    let count = r.count(24)?;
    if !matches!(count, 1 | 2) {
        return Err(invalid("unsupported edge endpoint count"));
    }
    let (ends, cycles): (Vec<_>, Vec<_>) = (0..count).map(|_| vertex(r, terms, &faces, count == 1)).collect::<Result<Vec<_>>>()?.into_iter().unzip();
    if (count == 2 && ends.first() == ends.get(1))
        || (seam != u32::MAX && ends.iter().any(|v| terms.get(seam as usize).is_none_or(|t| !v.seams.contains(t))))
    {
        return Err(invalid("invalid finite-edge endpoints"));
    }
    integer(r, -1)?;
    let primary = primary_face(&faces, &cycles);
    Ok((Edge { faces, ends }, primary))
}

pub(super) fn identity(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, bounded: bool) -> Result<Identity> {
    read_identity(records, classes, id, bounded, &BTreeSet::new()).map(|(identity, _)| identity)
}

pub(super) fn tracked_identity(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64, retired: &BTreeSet<u64>) -> Result<Identity> {
    read_identity(records, classes, id, false, retired).map(|(identity, _)| identity)
}

pub(super) fn identity_with_primary(records: &Records<'_>, classes: &BTreeMap<u64, &Class>, id: u64) -> Result<(Identity, Term)> {
    let (identity, primary) = read_identity(records, classes, id, false, &BTreeSet::new())?;
    let [Some(primary)] = primary.as_slice() else {
        return Err(invalid("asymmetric joined chamfer needs oppositely oriented three-face endpoints"));
    };
    Ok((identity, primary.clone()))
}

fn read_identity(
    records: &Records<'_>,
    classes: &BTreeMap<u64, &Class>,
    id: u64,
    bounded: bool,
    retired: &BTreeSet<u64>,
) -> Result<(Identity, Vec<Option<Term>>)> {
    require_class(classes, id, "7ACC2A03-0261-4879-A14A-A93D661A5BDC", 3)?;
    let mut r = payload(records.primary(id)?, id)?;
    zeros(&mut r, 6)?;
    integer(&mut r, 1)?;
    integer(&mut r, 3)?;
    let count = r.count(24)?;
    if !(2..=64).contains(&count) {
        return Err(invalid("unsupported incidence term count"));
    }
    let mut terms = Vec::new();
    let mut seen = BTreeSet::new();
    let mut retired_modifiers = BTreeSet::new();
    for _ in 0..count {
        let count = r.count(24)?;
        if !(1..=64).contains(&count) {
            return Err(invalid("invalid incidence contributor count"));
        }
        let mut choices = Term::new();
        for _ in 0..count {
            let label = r.string()?;
            let label = label.parse::<i32>().ok().filter(|n| *n != 0 && n.to_string() == label).ok_or_else(|| invalid("invalid incidence label"))?;
            zeros(&mut r, 4)?;
            let operation = match r.u32()? {
                1 => r.u64()?,
                2 => {
                    let operation = u64::from(r.u32()?);
                    let modifier = (r.u32()? as i32)
                        .checked_neg()
                        .filter(|n| *n > 0)
                        .map(|n| n as u64)
                        .ok_or_else(|| invalid("unsupported tracked operation qualifier"))?;
                    zeros(&mut r, 4)?;
                    if !retired.contains(&modifier) || modifier == operation || !retired_modifiers.insert((operation, label, modifier)) {
                        return Err(invalid("tracked operation qualifier is not uniquely retired"));
                    }
                    operation
                }
                _ => return Err(invalid("unsupported incidence operation count")),
            };
            if operation == 0 || !choices.insert((operation, label)) {
                return Err(invalid("duplicate or missing incidence contributor"));
            }
        }
        if !seen.insert(choices.clone()) {
            return Err(invalid("duplicate incidence face term"));
        }
        terms.push(choices);
    }
    zeros(&mut r, 4)?;
    if r.string()? != if bounded { "bounded_face_recipe_data" } else { "edge_recipe_data" } {
        return Err(invalid("incorrect incidence recipe kind"));
    }
    let count = if bounded {
        integer(&mut r, 0)?;
        integer(&mut r, -1)?;
        let count = r.count(64)?;
        if count != 1 && !(3..=128).contains(&count) {
            return Err(invalid("unsupported bounded-face edge count"));
        }
        count
    } else {
        1
    };
    let (edges, primary): (Vec<_>, Vec<_>) = (0..count).map(|_| edge(&mut r, &terms)).collect::<Result<Vec<_>>>()?.into_iter().unzip();
    finish(&r)?;
    if bounded {
        let cap = terms.first().ok_or_else(|| invalid("missing bounded face"))?;
        if edges.iter().any(|edge| !edge.faces.contains(cap)) {
            return Err(invalid("bounded edges disagree with selected face"));
        }
        if let [edge] = edges.as_slice() {
            if edge.ends.len() != 1 {
                return Err(invalid("single-edge bounded face requires a closed carrier"));
            }
        } else {
            cycle(&edges)?;
        }
    }
    Ok((Identity { terms, edges, retired_modifiers }, primary))
}

/// Every endpoint is shared by exactly two edges in one connected closed loop.
pub(super) fn cycle(edges: &[Edge]) -> Result<()> {
    if !(3..=128).contains(&edges.len()) {
        return Err(invalid("invalid projected boundary size"));
    }
    let mut incidence = BTreeMap::<&Vertex, Vec<usize>>::new();
    for (i, edge) in edges.iter().enumerate() {
        if edge.ends.len() != 2 {
            return Err(invalid("a multi-edge projected boundary requires open spans"));
        }
        for end in &edge.ends {
            incidence.entry(end).or_default().push(i);
        }
    }
    if incidence.values().any(|edges| edges.len() != 2 || edges.first() == edges.get(1)) {
        return Err(invalid("projected boundary has missing or ambiguous endpoint incidence"));
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![0];
    while let Some(i) = pending.pop() {
        if visited.insert(i) {
            for end in &edges.get(i).ok_or_else(|| invalid("missing projected boundary edge"))?.ends {
                pending.extend(incidence.get(end).into_iter().flatten().copied().filter(|j| !visited.contains(j)));
            }
        }
    }
    if visited.len() != edges.len() {
        return Err(invalid("projected boundary is disconnected"));
    }
    Ok(())
}

/// Compare complete boundary graphs from different evaluation generations.
/// Signatures still define exact vertex identity within each graph. Across
/// generations a cyclic bijection preserves every incident face and seam;
/// callers must additionally validate the regenerated geometric boundary.
pub(super) fn same_boundary(a: &[Edge], b: &[Edge]) -> Result<bool> {
    cycle(a)?;
    cycle(b)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    fn walk(edges: &[Edge], first: usize, reverse: bool) -> Result<Vec<(&Edge, &Vertex)>> {
        let mut index = first;
        let mut vertex = edges.get(first).and_then(|e| e.ends.get(usize::from(reverse))).ok_or_else(|| invalid("missing boundary start"))?;
        let mut out = Vec::new();
        for _ in 0..edges.len() {
            let edge = edges.get(index).ok_or_else(|| invalid("missing boundary edge"))?;
            out.push((edge, vertex));
            vertex = edge.ends.iter().find(|v| *v != vertex).ok_or_else(|| invalid("collapsed boundary edge"))?;
            index = edges
                .iter()
                .enumerate()
                .find(|(i, e)| *i != index && e.ends.contains(vertex))
                .map(|(i, _)| i)
                .ok_or_else(|| invalid("open boundary graph"))?;
        }
        Ok(out)
    }
    let ordered = walk(a, 0, false)?;
    for reverse in [false, true] {
        let candidate = walk(b, 0, reverse)?;
        for start in 0..b.len() {
            if ordered
                .iter()
                .zip(candidate.iter().cycle().skip(start))
                .all(|((a, av), (b, bv))| a.faces.iter().all(|f| b.faces.contains(f)) && av.faces == bv.faces && av.seams == bv.seams)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "recipe_incidence_tests.rs"]
mod tests;
