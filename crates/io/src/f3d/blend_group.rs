//! Resolve a whole selected edge graph when individual symmetric references
//! are ambiguous. Every admissible chain must be selected, and the native
//! endpoint graph must embed bijectively. Coordinates only join kernel seams.
use super::*;

#[derive(Clone, Copy)]
struct Choice {
    chain: usize,
    ends: [usize; 2],
}

struct Matching {
    native: Vec<[usize; 2]>,
    choices: Vec<Vec<Choice>>,
    order: Vec<usize>,
    used: Vec<bool>,
    forward: Vec<Option<usize>>,
    reverse: Vec<Option<usize>>,
    budget: usize,
}

impl Matching {
    fn search(&mut self, depth: usize) -> Result<bool> {
        if depth == self.order.len() {
            return Ok(self.used.iter().all(|used| *used) && self.forward.iter().all(Option::is_some) && self.reverse.iter().all(Option::is_some));
        }
        if self.budget == 0 {
            return Err(invalid("blend group correspondence work limit exceeded"));
        }
        self.budget -= 1;
        let index = *self.order.get(depth).ok_or_else(|| invalid("missing native group edge"))?;
        let native = *self.native.get(index).ok_or_else(|| invalid("missing group endpoints"))?;
        let choices = self.choices.get(index).cloned().ok_or_else(|| invalid("missing group choices"))?;
        for choice in choices {
            if self.used.get(choice.chain).copied().unwrap_or(true) {
                continue;
            }
            let compatible = native.iter().zip(choice.ends).all(|(a, b)| {
                self.forward.get(*a).is_some_and(|slot| slot.is_none_or(|old| old == b))
                    && self.reverse.get(b).is_some_and(|slot| slot.is_none_or(|old| old == *a))
            });
            if !compatible {
                continue;
            }
            let saved = (self.forward.clone(), self.reverse.clone());
            for (a, b) in native.iter().zip(choice.ends) {
                *self.forward.get_mut(*a).ok_or_else(|| invalid("missing native vertex"))? = Some(b);
                *self.reverse.get_mut(b).ok_or_else(|| invalid("missing reconstructed vertex"))? = Some(*a);
            }
            *self.used.get_mut(choice.chain).ok_or_else(|| invalid("missing reconstructed chain"))? = true;
            if self.search(depth + 1)? {
                return Ok(true);
            }
            *self.used.get_mut(choice.chain).ok_or_else(|| invalid("missing reconstructed chain"))? = false;
            (self.forward, self.reverse) = saved;
        }
        Ok(false)
    }
}

fn node(nodes: &mut Vec<Vec3>, point: Vec3, tolerance: f64) -> Result<usize> {
    if !point.is_finite() {
        return Err(invalid("nonfinite group endpoint location"));
    }
    let matches = nodes.iter().enumerate().filter_map(|(i, p)| (p.dist(point) <= tolerance).then_some(i)).collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] if nodes.len() < 128 => {
            nodes.push(point);
            Ok(nodes.len() - 1)
        }
        _ => Err(invalid("ambiguous or excessive group endpoint locations")),
    }
}

pub(super) fn resolve(body: &ModelBody, inputs: &[(recipe_incidence::Edge, Vec<edges::Candidate>)]) -> Result<Vec<Selection>> {
    if !(2..=64).contains(&inputs.len()) {
        return Err(invalid("invalid composite blend group size"));
    }
    let tolerance = 1e-7 * (1. + body.body.size());
    if !tolerance.is_finite() || tolerance <= 0. {
        return Err(invalid("invalid group correspondence scale"));
    }
    let mut vertices = BTreeMap::new();
    let mut native = Vec::new();
    let mut nodes = Vec::new();
    let mut chains = Vec::<BTreeSet<usize>>::new();
    let mut selections = Vec::new();
    let mut choices = Vec::new();
    let mut seen = BTreeSet::new();
    for (edge, candidates) in inputs {
        let [a, b] = edge.ends.as_slice() else {
            return Err(invalid("blend group requires finite native edges"));
        };
        if a == b {
            return Err(invalid("blend group repeats a native endpoint"));
        }
        if !seen.insert((edge.faces.iter().cloned().collect::<BTreeSet<_>>(), edge.ends.iter().cloned().collect::<BTreeSet<_>>())) {
            return Err(invalid("blend group repeats a complete native edge identity"));
        }
        let mut pair = [0; 2];
        for (slot, vertex) in pair.iter_mut().zip([a, b]) {
            let count = vertices.len();
            *slot = *vertices.entry(vertex).or_insert(count);
        }
        native.push(pair);
        let mut options = Vec::new();
        for candidate in candidates {
            let chain = if let Some(index) = chains.iter().position(|spans| spans == &candidate.spans) {
                index
            } else {
                if chains.len() >= inputs.len() || chains.iter().any(|spans| !spans.is_disjoint(&candidate.spans)) {
                    return Err(invalid("blend group leaves ambiguous or overlapping chains"));
                }
                chains.push(candidate.spans.clone());
                selections.extend(candidate.selection.iter().cloned());
                chains.len() - 1
            };
            let ends = [node(&mut nodes, candidate.ends[0], tolerance)?, node(&mut nodes, candidate.ends[1], tolerance)?];
            if ends[0] == ends[1] {
                return Err(invalid("blend group has a closed finite chain"));
            }
            for order in [[0, 1], [1, 0]] {
                if candidate.end_faces[order[0]] == a.faces && candidate.end_faces[order[1]] == b.faces {
                    options.push(Choice { chain, ends: [ends[order[0]], ends[order[1]]] });
                }
            }
        }
        if options.is_empty() {
            return Err(invalid("native group edge has no matching endpoints"));
        }
        choices.push(options);
    }
    if chains.len() != inputs.len() || nodes.len() != vertices.len() {
        return Err(invalid("blend group does not cover the complete candidate graph"));
    }
    let mut order: Vec<_> = (0..inputs.len()).collect();
    order.sort_by_key(|i| choices.get(*i).map_or(usize::MAX, Vec::len));
    let mut matching = Matching {
        native,
        choices,
        order,
        used: vec![false; chains.len()],
        forward: vec![None; vertices.len()],
        reverse: vec![None; nodes.len()],
        budget: 4096,
    };
    if !matching.search(0)? {
        return Err(invalid("blend group endpoint graphs disagree"));
    }
    let mut names = BTreeSet::new();
    if selections.iter().any(|s| !names.insert(&s.name)) {
        return Err(invalid("blend group repeats a persistent edge name"));
    }
    Ok(selections)
}
