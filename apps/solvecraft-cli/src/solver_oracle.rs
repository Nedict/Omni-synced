//! Solver-behaviour oracle: build a sketch case (`<case>/case.json`) with engine commands,
//! apply its dimension edits one after another through the parameter commands, and compare
//! where every point lands with Fusion's measurements of the same edits (`<case>/measure.json`,
//! written by the local Fusion script; never committed).
//!
//! Each edit is classified: `same` (every point within [`SAME_TOL`]), `mirrored` (both solved,
//! and a constrained triangle turns the other way or an offset or a horizontal/vertical line
//! points the other way), `different` (both solved, another placement with the same
//! orientations), `we_refused` / `fusion_refused` (only one took the values), `both_refused`.
//! The orientations are the solver's own notion of a branch: the turn of three mutually
//! related points and the sign of a one-axis offset.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use solvecraft_engine::Session;

use crate::read_json;

/// Two placements are the same when every point is within this (mm).
pub const SAME_TOL: f64 = 1e-3;

/// A requested value counts as held within this (mm): the case values have six decimals.
const HELD_TOL: f64 = 1e-6;

/// Name of the sketch every case is built in.
const SKETCH: &str = "SolverOracle";

/// Caps on a case's size (case files are input, so hostile).
const MAX_ENTITIES: usize = 1000;
const MAX_EDITS: usize = 10_000;
/// Caps on the orientations compared (related point pairs, related triples).
const MAX_RELATIONS: usize = 20_000;
const MAX_TURNS: usize = 100_000;

type Pt = [f64; 2];
type Points = BTreeMap<String, Pt>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DimKind {
    Distance,
    Horizontal,
    Vertical,
    Radius,
}

#[derive(Debug)]
struct Dim {
    name: String,
    kind: DimKind,
    /// Two points, or the arc of a radius.
    refs: Vec<String>,
    value: f64,
}

#[derive(Debug)]
struct Arc {
    id: String,
    center: String,
    start: String,
    end: String,
}

impl DimKind {
    fn from_case(t: &str) -> Option<Self> {
        Some(match t {
            "distance" => DimKind::Distance,
            "distance_horizontal" => DimKind::Horizontal,
            "distance_vertical" => DimKind::Vertical,
            "radius" => DimKind::Radius,
            _ => return None,
        })
    }

    /// The `sketch.dimension` type.
    fn engine_type(self) -> &'static str {
        match self {
            DimKind::Distance => "distance",
            DimKind::Horizontal => "horizontal",
            DimKind::Vertical => "vertical",
            DimKind::Radius => "radius",
        }
    }

    /// The axis a one-axis offset measures along.
    fn axis(self) -> Option<Axis> {
        match self {
            DimKind::Horizontal => Some(Axis::X),
            DimKind::Vertical => Some(Axis::Y),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

#[derive(Debug)]
enum Constraint {
    /// A horizontal (X) or vertical (Y) line.
    Axis(String, Axis),
    Tangent(String, String),
    Equal(String, String),
    Coincident(String, String),
}

/// A solver-oracle case (see plan/fusion/solver-oracle/README.md for the format).
#[derive(Debug)]
pub struct Case {
    points: Vec<(String, Pt)>,
    fixed: Vec<String>,
    /// (id, start point, end point).
    lines: Vec<[String; 3]>,
    arcs: Vec<Arc>,
    constraints: Vec<Constraint>,
    dims: Vec<Dim>,
    /// Each edit's values in the order of `dims` (the order the case generator wrote them,
    /// which is the order Fusion applies them one by one).
    edits: Vec<Vec<(String, f64)>>,
}

fn text(v: &Value, k: &str, what: &str) -> Result<String, String> {
    v.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("{what}: `{k}` must be text"))
}

fn finite(v: &Value, what: &str) -> Result<f64, String> {
    v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| format!("{what}: not a finite number"))
}

fn point(v: &Value, what: &str) -> Result<Pt, String> {
    match v.as_array().map(Vec::as_slice) {
        Some([x, y]) => Ok([finite(x, what)?, finite(y, what)?]),
        _ => Err(format!("{what}: a point must be [x, y]")),
    }
}

/// `v[k]`, a list of at most `max` items (case files are hostile).
fn list<'a>(v: &'a Value, k: &str, max: usize) -> Result<&'a [Value], String> {
    let l = v.get(k).and_then(Value::as_array).ok_or_else(|| format!("case: `{k}` must be a list"))?;
    if l.len() > max { Err(format!("case: too many `{k}`")) } else { Ok(l) }
}

/// Parse `case.json`.
pub fn parse_case(v: &Value) -> Result<Case, String> {
    let pts = v.get("points").and_then(Value::as_object).ok_or("case: `points` must be an object")?;
    if pts.len() > MAX_ENTITIES {
        return Err("case: too many `points`".into());
    }
    let points = pts.iter().map(|(n, p)| Ok((n.clone(), point(p, &format!("point {n}"))?))).collect::<Result<Vec<_>, String>>()?;
    let known: BTreeSet<&str> = points.iter().map(|(n, _)| n.as_str()).collect();
    let pref = |n: String| if known.contains(n.as_str()) { Ok(n) } else { Err(format!("case: unknown point `{n}`")) };
    let entities = |k: &str| list(v, k, MAX_ENTITIES);
    let fixed = entities("fixed")?
        .iter()
        .map(|x| x.as_str().ok_or_else(|| "case: `fixed` lists point names".to_string()).and_then(|n| pref(n.to_string())))
        .collect::<Result<_, _>>()?;
    let lines = entities("lines")?
        .iter()
        .map(|l| match l.as_array().map(Vec::as_slice) {
            Some([id, a, b]) => {
                let s = |x: &Value| x.as_str().map(str::to_string).ok_or("case: a line is [id, a, b]".to_string());
                Ok([s(id)?, pref(s(a)?)?, pref(s(b)?)?])
            }
            _ => Err("case: a line is [id, a, b]".to_string()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arcs = entities("arcs")?
        .iter()
        .map(|a| {
            Ok(Arc {
                id: text(a, "id", "arc")?,
                center: pref(text(a, "center", "arc")?)?,
                start: pref(text(a, "start", "arc")?)?,
                end: pref(text(a, "end", "arc")?)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let constraints = entities("constraints")?
        .iter()
        .map(|c| {
            let two = |f: fn(String, String) -> Constraint| Ok(f(text(c, "a", "constraint")?, text(c, "b", "constraint")?));
            match c.get("type").and_then(Value::as_str) {
                Some("horizontal") => Ok(Constraint::Axis(text(c, "line", "constraint")?, Axis::X)),
                Some("vertical") => Ok(Constraint::Axis(text(c, "line", "constraint")?, Axis::Y)),
                Some("tangent") => two(Constraint::Tangent),
                Some("equal") => two(Constraint::Equal),
                Some("coincident") => two(Constraint::Coincident),
                other => Err(format!("case: constraint {other:?} is not supported yet")),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let dims = entities("dimensions")?
        .iter()
        .map(|d| {
            let name = text(d, "name", "dimension")?;
            let value = finite(d.get("value").unwrap_or(&Value::Null), &format!("dimension {name}"))?;
            let ty = d.get("type").and_then(Value::as_str);
            let kind = ty.and_then(DimKind::from_case).ok_or_else(|| format!("case: dimension type {ty:?} is not supported yet"))?;
            let refs = match kind {
                DimKind::Radius => vec![text(d, "arc", "dimension")?],
                _ => vec![pref(text(d, "a", "dimension")?)?, pref(text(d, "b", "dimension")?)?],
            };
            Ok(Dim { name, kind, refs, value })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let edits = list(v, "edits", MAX_EDITS)?
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let m = e.as_object().ok_or_else(|| format!("case: edit {i} must be an object"))?;
            if let Some(k) = m.keys().find(|k| !dims.iter().any(|d| &d.name == *k)) {
                return Err(format!("case: edit {i} names unknown dimension `{k}`"));
            }
            dims.iter().filter_map(|d| m.get(&d.name).map(|x| Ok((d.name.clone(), finite(x, &format!("edit {i} {}", d.name))?)))).collect()
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Case { points, fixed, lines, arcs, constraints, dims, edits })
}

fn exec(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    s.execute(id, &p).map_err(|e| format!("{id}: {e}"))
}

fn mm(v: f64) -> String {
    format!("{v:.6} mm")
}

/// Build the case's sketch with engine commands; dimensions get the case's names.
pub fn build(case: &Case) -> Result<Session, String> {
    let mut s = Session::default();
    exec(&mut s, "sketch.create", json!({"plane": "XY", "name": SKETCH}))?;
    // `connect: false`: the case says exactly which points are shared (as Fusion's API does).
    for (n, p) in &case.points {
        exec(&mut s, "sketch.point", json!({"point": p, "id": n, "connect": false}))?;
    }
    for [id, a, b] in &case.lines {
        exec(&mut s, "sketch.line", json!({"points": [a, b], "ids": [id], "connect": false}))?;
    }
    for a in &case.arcs {
        exec(&mut s, "sketch.arc.center_point", json!({"center": a.center, "start": a.start, "end": a.end, "id": a.id, "connect": false}))?;
    }
    if !case.fixed.is_empty() {
        exec(&mut s, "sketch.constraint.fix", json!({"entities": case.fixed, "fixed": true}))?;
    }
    for c in &case.constraints {
        let (id, p) = match c {
            Constraint::Axis(l, axis) => {
                ("sketch.constraint.horizontal_vertical", json!({"line": l, "mode": if *axis == Axis::X { "horizontal" } else { "vertical" }}))
            }
            Constraint::Tangent(a, b) => ("sketch.constraint.tangent", json!({"a": a, "b": b})),
            Constraint::Equal(a, b) => ("sketch.constraint.equal", json!({"a": a, "b": b})),
            Constraint::Coincident(a, b) => ("sketch.constraint.coincident", json!({"a": a, "b": b})),
        };
        exec(&mut s, id, p)?;
    }
    for d in &case.dims {
        let r = exec(&mut s, "sketch.dimension", json!({"entities": d.refs, "type": d.kind.engine_type(), "value": mm(d.value)}))?;
        let param = r.get("param").and_then(Value::as_str).ok_or_else(|| format!("dimension {}: no parameter", d.name))?.to_string();
        exec(&mut s, "parameters.rename", json!({"name": param, "new_name": d.name}))?;
    }
    exec(&mut s, "sketch.finish", json!({}))?;
    Ok(s)
}

/// One side's state after an edit: whether the values took, and where the points are.
#[derive(Clone, Debug)]
pub struct Snap {
    pub took: bool,
    pub points: Points,
    /// Why SolveCraft refused (command errors, unsolved sketch).
    pub errors: Vec<String>,
}

/// The sketch's solve status and the case's points, read with `sketch.inspect`.
fn inspect(s: &mut Session, case: &Case) -> Result<(bool, Points), String> {
    let v = exec(s, "sketch.inspect", json!({"sketch": SKETCH}))?;
    let solved = v.get("status").and_then(Value::as_str) == Some("solved");
    let mut all = Points::new();
    for p in v.get("points").and_then(Value::as_array).into_iter().flatten() {
        if let (Some(id), Ok(at)) = (p.get("id").and_then(Value::as_str), point(p.get("at").unwrap_or(&Value::Null), "inspect")) {
            all.insert(id.to_string(), at);
        }
    }
    let points = case
        .points
        .iter()
        .map(|(n, _)| all.get(n).map(|p| (n.clone(), *p)).ok_or_else(|| format!("point `{n}` is missing")))
        .collect::<Result<_, _>>()?;
    Ok((solved, points))
}

/// How an edit's values are applied: all together (one recompute), or one parameter change
/// after another in the case's order — whichever Fusion did for that edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Together,
    OneByOne,
}

/// A parameter command's refusal: its error, or the features it left failing.
fn param_cmd(s: &mut Session, id: &str, p: Value) -> Vec<String> {
    match exec(s, id, p) {
        Err(e) => vec![e],
        Ok(r) => r.get("errors").and_then(Value::as_array).into_iter().flatten().map(|e| format!("{id}: {e}")).collect(),
    }
}

/// Apply one edit; what the parameter commands reported against it (whether the edit took is
/// decided by the state after it, see [`replay`]).
fn apply(s: &mut Session, edit: &[(String, f64)], mode: Mode) -> Vec<String> {
    match mode {
        // The engine's batch change: every value set, then one recompute (all or nothing).
        Mode::Together => {
            let rows: Vec<Value> = edit.iter().map(|(n, v)| json!({"name": n, "expression": mm(*v)})).collect();
            param_cmd(s, "parameters.import", json!({"text": Value::Array(rows).to_string()}))
        }
        Mode::OneByOne => edit.iter().flat_map(|(n, v)| param_cmd(s, "parameters.change", json!({"name": n, "expression": mm(*v)}))).collect(),
    }
}

/// What dimension `d` measures on a placement.
fn dim_value(case: &Case, p: &Points, d: &Dim) -> Option<f64> {
    let at = |n: &str| p.get(n).copied();
    match (d.kind, d.refs.as_slice()) {
        (DimKind::Radius, [arc]) => {
            let a = case.arcs.iter().find(|a| &a.id == arc)?;
            let (o, s) = (at(&a.center)?, at(&a.start)?);
            Some((s[0] - o[0]).hypot(s[1] - o[1]))
        }
        (DimKind::Distance, [a, b]) => {
            let (a, b) = (at(a)?, at(b)?);
            Some((b[0] - a[0]).hypot(b[1] - a[1]))
        }
        (DimKind::Horizontal, [a, b]) => Some((at(b)?[0] - at(a)?[0]).abs()),
        (DimKind::Vertical, [a, b]) => Some((at(b)?[1] - at(a)?[1]).abs()),
        _ => None,
    }
}

/// The requested values a placement does not measure.
fn unheld(case: &Case, p: &Points, edit: &[(String, f64)]) -> Vec<String> {
    edit.iter()
        .filter_map(|(n, want)| {
            let got = case.dims.iter().find(|d| &d.name == n).and_then(|d| dim_value(case, p, d));
            match got {
                Some(g) if (g - want).abs() <= HELD_TOL => None,
                Some(g) => Some(format!("{n} measures {g:.6}, not {want:.6}")),
                None => Some(format!("{n} cannot be measured")),
            }
        })
        .collect()
}

/// Build the case and apply its edits in order; SolveCraft's state after the build and after
/// each edit. `modes` gives each edit's mode (missing: together).
///
/// An edit took when, after all its values are applied, the sketch solves and measures every
/// requested value (what happens in between, one value at a time, does not matter). An edit
/// that did not take is undone before the next one, as Fusion keeps the values it held.
pub fn replay(case: &Case, modes: &[Mode]) -> Result<(Snap, Vec<Snap>), String> {
    let mut s = build(case)?;
    // Checkpoints copy the undo history, which the replay never uses.
    s.undo.clear();
    let (solved, points) = inspect(&mut s, case)?;
    let initial = Snap { took: solved, errors: if solved { vec![] } else { vec!["the initial sketch does not solve".into()] }, points };
    let mut out = Vec::with_capacity(case.edits.len());
    for (i, edit) in case.edits.iter().enumerate() {
        let before = s.checkpoint();
        let mut errors = apply(&mut s, edit, modes.get(i).copied().unwrap_or(Mode::Together));
        // A sketch that cannot be read back after the edit is a refusal of that edit.
        let (solved, points) = inspect(&mut s, case).unwrap_or_else(|e| {
            errors.push(e);
            (false, Points::new())
        });
        let missing = if solved { unheld(case, &points, edit) } else { vec!["the sketch does not solve".into()] };
        let took = missing.is_empty();
        errors.extend(missing);
        if took {
            // What went wrong on the way (one value at a time) does not matter once it holds.
            errors.clear();
            s.drop_checkpoint(before);
        } else {
            s.restore_checkpoint(before).map_err(|e| format!("edit {i}: cannot undo the refused edit: {e}"))?;
        }
        out.push(Snap { took, points, errors });
    }
    Ok((initial, out))
}

/// Fusion's measurements: the initial state, and each edit's state and mode.
pub fn parse_measure(v: &Value, case: &Case) -> Result<(Snap, Vec<(Snap, Mode)>), String> {
    let snap = |e: &Value, took: bool| -> Result<Snap, String> {
        let pts = e.get("points").and_then(Value::as_object).ok_or("measure: `points` must be an object")?;
        let points = case
            .points
            .iter()
            .map(|(n, _)| pts.get(n).ok_or_else(|| format!("measure: point `{n}` is missing")).and_then(|p| point(p, n)).map(|p| (n.clone(), p)))
            .collect::<Result<_, _>>()?;
        Ok(Snap { took, points, errors: vec![] })
    };
    let initial = snap(v.get("initial").ok_or("measure: `initial` is missing")?, true)?;
    let edits = v.get("edits").and_then(Value::as_array).ok_or("measure: `edits` must be a list")?;
    if edits.len() != case.edits.len() {
        return Err(format!("measure: {} edits, the case has {}", edits.len(), case.edits.len()));
    }
    let edits = edits
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let took = e.get("took").and_then(Value::as_bool).ok_or_else(|| format!("measure: edit {i} has no `took`"))?;
            let mode = match e.get("mode").and_then(Value::as_str) {
                Some("together") => Mode::Together,
                Some("one_by_one") => Mode::OneByOne,
                other => return Err(format!("measure: edit {i} has mode {other:?} (together or one_by_one)")),
            };
            Ok((snap(e, took)?, mode))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((initial, edits))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Same,
    Mirrored,
    Different,
    WeRefused,
    FusionRefused,
    BothRefused,
}

impl Class {
    pub const ALL: [Class; 6] = [Class::Same, Class::Mirrored, Class::Different, Class::WeRefused, Class::FusionRefused, Class::BothRefused];

    pub fn name(self) -> &'static str {
        match self {
            Class::Same => "same",
            Class::Mirrored => "mirrored",
            Class::Different => "different",
            Class::WeRefused => "we_refused",
            Class::FusionRefused => "fusion_refused",
            Class::BothRefused => "both_refused",
        }
    }
}

/// The orientations a case's constraints leave open: the turn of every three mutually related
/// points (dimensioned, joined by a line or arc, or both fixed), and the direction of every
/// one-axis offset and horizontal or vertical line.
struct Orientations {
    turns: Vec<[String; 3]>,
    sides: Vec<(String, String, Axis)>,
}

impl Orientations {
    fn of(case: &Case) -> Result<Self, String> {
        // Each point's related points, both ways.
        let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut pairs: Vec<(&str, &str)> = case.lines.iter().map(|[_, a, b]| (a.as_str(), b.as_str())).collect();
        for a in &case.arcs {
            pairs.extend([(a.center.as_str(), a.start.as_str()), (a.center.as_str(), a.end.as_str()), (a.start.as_str(), a.end.as_str())]);
        }
        pairs.extend(case.dims.iter().filter_map(|d| match d.refs.as_slice() {
            [a, b] => Some((a.as_str(), b.as_str())),
            _ => None,
        }));
        // Every two fixed points are related: n² / 2 pairs, so count them before making them.
        let n = case.fixed.len();
        if pairs.len().saturating_add(n.saturating_mul(n.saturating_sub(1)) / 2) > MAX_RELATIONS {
            return Err(format!("too many related point pairs to compare orientations (at most {MAX_RELATIONS})"));
        }
        for (i, a) in case.fixed.iter().enumerate() {
            pairs.extend(case.fixed.iter().skip(i + 1).map(|b| (a.as_str(), b.as_str())));
        }
        for (a, b) in pairs.into_iter().filter(|(a, b)| a != b) {
            adj.entry(a).or_default().insert(b);
            adj.entry(b).or_default().insert(a);
        }
        // Triangles: a < b < c, each pair related (neighbours of a and b in common).
        let mut turns = Vec::new();
        for (a, na) in &adj {
            for b in na.range::<&str, _>((std::ops::Bound::Excluded(a), std::ops::Bound::Unbounded)) {
                let Some(nb) = adj.get(b) else { continue };
                for c in na.intersection(nb).filter(|c| *c > b) {
                    if turns.len() >= MAX_TURNS {
                        return Err(format!("too many related point triples to compare orientations (at most {MAX_TURNS})"));
                    }
                    turns.push([a.to_string(), b.to_string(), c.to_string()]);
                }
            }
        }
        let line = |id: &str| case.lines.iter().find(|l| l[0] == id).map(|l| (l[1].clone(), l[2].clone()));
        let mut sides: Vec<(String, String, Axis)> = case
            .constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::Axis(l, axis) => line(l).map(|(a, b)| (a, b, *axis)),
                _ => None,
            })
            .collect();
        for d in &case.dims {
            if let ([a, b], Some(axis)) = (d.refs.as_slice(), d.kind.axis()) {
                sides.push((a.clone(), b.clone(), axis));
            }
        }
        Ok(Orientations { turns, sides })
    }

    /// The orientations that differ between two placements (those too close to zero to tell
    /// on either side are not compared).
    fn flipped(&self, a: &Points, b: &Points) -> Vec<String> {
        let mut out = Vec::new();
        for t in &self.turns {
            if let (Some(x), Some(y)) = (turn(a, t), turn(b, t))
                && x != y
            {
                out.push(format!("turn {}", t.join(",")));
            }
        }
        for (p, q, axis) in &self.sides {
            if let (Some(x), Some(y)) = (side(a, p, q, *axis), side(b, p, q, *axis))
                && x != y
            {
                out.push(format!("{} {p}→{q}", if *axis == Axis::X { "x-side" } else { "y-side" }));
            }
        }
        out
    }
}

/// Relative size below which an orientation is too close to zero to tell.
const ON_MIRROR: f64 = 1e-9;

fn turn(p: &Points, t: &[String; 3]) -> Option<bool> {
    let (a, b, c) = (p.get(&t[0])?, p.get(&t[1])?, p.get(&t[2])?);
    let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
    let cross = u[0] * v[1] - u[1] * v[0];
    let scale = u[0].hypot(u[1]) * v[0].hypot(v[1]);
    (scale > 0.0 && cross.abs() > ON_MIRROR * scale).then_some(cross > 0.0)
}

fn side(p: &Points, a: &str, b: &str, axis: Axis) -> Option<bool> {
    let (a, b) = (p.get(a)?, p.get(b)?);
    let axis = if axis == Axis::X { 0 } else { 1 };
    let d = b[axis] - a[axis];
    (d.abs() > ON_MIRROR * a[axis].abs().max(b[axis].abs()).max(1.0)).then_some(d > 0.0)
}

/// The largest distance between the two placements' points, and which point.
fn deviation(a: &Points, b: &Points) -> (f64, String) {
    a.iter()
        .map(|(n, p)| (b.get(n).map_or(f64::INFINITY, |q| (p[0] - q[0]).hypot(p[1] - q[1])), n.clone()))
        .fold((0.0, String::new()), |m, x| if x.0 > m.0 || x.0.is_nan() { x } else { m })
}

/// Compare SolveCraft's state after an edit with Fusion's; the class and what differs.
fn classify(o: &Orientations, ours: &Snap, fusion: &Snap) -> (Class, String) {
    match (ours.took, fusion.took) {
        (false, false) => (Class::BothRefused, ours.errors.join("; ")),
        (false, true) => (Class::WeRefused, ours.errors.join("; ")),
        (true, false) => (Class::FusionRefused, String::new()),
        (true, true) => {
            let (dev, at) = deviation(&ours.points, &fusion.points);
            if dev <= SAME_TOL {
                return (Class::Same, String::new());
            }
            let flipped = o.flipped(&ours.points, &fusion.points);
            let worst = format!("{at} is {dev:.3} mm away");
            if flipped.is_empty() { (Class::Different, worst) } else { (Class::Mirrored, format!("{} ({worst})", flipped.join(", "))) }
        }
    }
}

/// Replay one case directory; compare with its measure.json when there is one.
fn check_case(dir: &str) -> Value {
    let name = std::path::Path::new(dir).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let mut out = compare_case(dir).unwrap_or_else(|e| json!({"error": e}));
    out["case"] = json!(name);
    out
}

fn compare_case(dir: &str) -> Result<Value, String> {
    let case = parse_case(&read_json(&format!("{dir}/case.json"))?)?;
    let mp = format!("{dir}/measure.json");
    let measure = if std::path::Path::new(&mp).is_file() { Some(parse_measure(&read_json(&mp)?, &case)?) } else { None };
    let modes: Vec<Mode> = measure.iter().flat_map(|(_, e)| e.iter().map(|(_, m)| *m)).collect();
    let (initial, ours) = replay(&case, &modes)?;
    let took = ours.iter().filter(|s| s.took).count();
    let Some((f_initial, fusion)) = measure else {
        return Ok(json!({"measured": false, "edits": ours.len(), "took": took, "initial_solved": initial.took}));
    };
    let o = Orientations::of(&case)?;
    let (init_class, init_detail) = classify(&o, &initial, &f_initial);
    let mut counts: BTreeMap<&str, usize> = Class::ALL.iter().map(|c| (c.name(), 0)).collect();
    let mut results = Vec::new();
    for (i, (us, (them, mode))) in ours.iter().zip(&fusion).enumerate() {
        let (class, detail) = classify(&o, us, them);
        *counts.entry(class.name()).or_default() += 1;
        let mut r = json!({"edit": i, "class": class.name(), "mode": if *mode == Mode::OneByOne { "one_by_one" } else { "together" }});
        if class != Class::Same {
            r["detail"] = json!(detail);
            r["ours"] = json!(us.points);
            r["fusion"] = json!(them.points);
        }
        results.push(r);
    }
    let mut out = json!({"measured": true, "edits": ours.len(), "took": took, "counts": counts, "results": results});
    if init_class != Class::Same {
        out["error"] = json!(format!("the initial sketch differs from Fusion's ({}: {init_detail})", init_class.name()));
    }
    Ok(out)
}

/// `solvecraft-cli solver-oracle <case-dir>... [--json]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let dirs: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if dirs.is_empty() {
        return Err("solver-oracle: give one or more case directories (each with case.json, and measure.json once measured)".into());
    }
    let results: Vec<Value> = dirs.iter().map(|d| check_case(d)).collect();
    if args.iter().any(|a| a == "--json") {
        println!("{}", crate::pretty(&json!(results)));
    } else {
        for r in &results {
            let case = r["case"].as_str().unwrap_or("");
            if let Some(e) = r["error"].as_str() {
                println!("ERROR {case}: {e}");
            }
            if r["measured"].as_bool() != Some(true) {
                println!("{case}: not measured; SolveCraft took {}/{} edits", r["took"], r["edits"]);
                continue;
            }
            let counts: Vec<String> = Class::ALL.iter().map(|c| format!("{} {}", c.name(), r["counts"][c.name()])).collect();
            println!("{case}: {} edits — {}", r["edits"], counts.join(", "));
            for e in r["results"].as_array().into_iter().flatten().filter(|e| e["class"] != "same") {
                println!("  edit {} {}: {}", e["edit"], e["class"].as_str().unwrap_or(""), e["detail"].as_str().unwrap_or(""));
            }
        }
    }
    let failed = results.iter().filter(|r| r.get("error").is_some()).count();
    if failed > 0 { Err(format!("{failed} solver-oracle case(s) could not be replayed")) } else { Ok(()) }
}

#[cfg(test)]
#[path = "solver_oracle_tests.rs"]
mod tests;
