//! Seam analysis view state and the shared command implementation.

use serde_json::{Value, json};
use solidcraft_geom::Vec3;
use solidcraft_kernel::{Body, ContinuityOptions, EdgeContinuity};

use crate::params::{bad, str_, vec3};
use crate::{Result, Session};

#[derive(Clone, Debug)]
pub struct ContinuityBody {
    pub name: String,
    /// The geometry evaluated, used to suppress stale diagnostics after edits/moves.
    pub body: Body,
    pub edges: Vec<EdgeContinuity>,
}

#[derive(Clone, Debug)]
pub struct ContinuityAnalysis {
    pub options: ContinuityOptions,
    pub bodies: Vec<ContinuityBody>,
}

pub(crate) fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "inspect.continuity";
    if let Some(clear) = p.get("clear") {
        let clear = clear.as_bool().ok_or_else(|| bad(cmd, "`clear` must be a boolean"))?;
        if clear {
            s.continuity = None;
            s.revision += 1;
            return Ok(json!({"analysis": null}));
        }
    }
    let samples = match p.get("samples") {
        None => 33,
        Some(v) => v.as_u64().filter(|n| (3..=257).contains(n)).ok_or_else(|| bad(cmd, "`samples` must be an integer from 3 to 257"))? as usize,
    };
    let tolerance = |key: &str, default: f64| -> Result<f64> {
        match p.get(key) {
            None => Ok(default),
            Some(v) => v.as_f64().filter(|n| n.is_finite() && *n >= 0.).ok_or_else(|| bad(cmd, format!("`{key}` must be finite and non-negative"))),
        }
    };
    let options = ContinuityOptions {
        samples,
        gap_tolerance_mm: tolerance("gap_tolerance_mm", 1e-5)?,
        angle_tolerance_deg: tolerance("angle_tolerance_deg", 0.01)?,
        curvature_tolerance_per_mm: tolerance("curvature_tolerance_per_mm", 1e-5)?,
    };
    if options.angle_tolerance_deg > 180. {
        return Err(bad(cmd, "`angle_tolerance_deg` must be <= 180"));
    }
    let body_name = match p.get("body") {
        None => None,
        Some(_) => Some(str_(p, "body").ok_or_else(|| bad(cmd, "`body` must be a name"))?),
    };
    let world = s.world_state();
    let bodies: Vec<_> = world.bodies.iter().filter(|b| body_name.map_or_else(|| s.body_visible(&b.name), |name| b.name == name)).collect();
    if bodies.is_empty() {
        return Err(bad(cmd, "no matching visible B-rep body"));
    }
    if bodies.len() > 100 {
        return Err(bad(cmd, "select a body: at most 100 bodies can be analyzed at once"));
    }
    let picks = match p.get("edges") {
        None => None,
        Some(v) => {
            let array = v.as_array().ok_or_else(|| bad(cmd, "`edges` must be an array"))?;
            if array.is_empty() || array.len() > 1000 || bodies.len() != 1 {
                return Err(bad(cmd, "`edges` requires one body and 1..1000 picks"));
            }
            Some(array)
        }
    };
    let mut work = 0usize;
    let mut analyzed = Vec::new();
    let mut reply = Vec::new();
    for b in bodies {
        let edge_count = b.body.edge_count();
        let mut indices = Vec::new();
        if let Some(picks) = picks {
            for pick in picks {
                let index = if let Some(index) = pick.get("index") {
                    let index =
                        index.as_u64().and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(cmd, "edge index must be a non-negative integer"))?;
                    if index >= edge_count {
                        return Err(bad(cmd, "edge index out of range"));
                    }
                    index
                } else {
                    let at: Vec3 = vec3(pick).filter(|p| p.is_finite()).ok_or_else(|| bad(cmd, "edge pick must be a finite [x,y,z] or {index}"))?;
                    let (index, distance) = b.body.nearest_edge(at, 0.001)?.ok_or_else(|| bad(cmd, "body has no edges"))?;
                    if distance > 0.01 {
                        return Err(bad(cmd, "edge pick is more than 0.01 mm from an edge"));
                    }
                    index
                };
                if !indices.contains(&index) {
                    indices.push(index);
                }
            }
        }
        work = work.saturating_add(if indices.is_empty() { edge_count } else { indices.len() }.saturating_mul(samples));
        if work > 100_000 {
            return Err(bad(cmd, "analysis exceeds 100000 sample stations; select fewer edges/bodies or samples"));
        }
        let edges = solidcraft_kernel::edge_continuity(&b.body, &indices, options)?;
        reply.push(json!({"name": b.name, "edges": edges}));
        analyzed.push(ContinuityBody { name: b.name.clone(), body: b.body.clone(), edges });
    }
    // Publish only after the entire request succeeds; a failed request retains the old view.
    s.continuity = Some(ContinuityAnalysis { options, bodies: analyzed });
    s.revision += 1;
    Ok(json!({
        "analysis": "continuity", "sampled": true,
        "sampling": "uniform edge parameter, including endpoints; not a whole-edge certificate",
        "curvature_metric": "world-space second fundamental form difference, Frobenius norm; G2 verdict also requires G0/G1",
        "options": options, "bodies": reply
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_session() -> Session {
        let mut s = Session::default();
        s.execute("solid.box", &json!({"length": 10, "width": 20, "height": 30, "body_name": "Box"})).unwrap();
        s
    }

    #[test]
    fn diagnostics_are_view_state_and_have_units() {
        let mut s = box_session();
        let before = s.doc.clone();
        let undo = s.undo.len();
        let result = s.execute("inspect.continuity", &json!({"body": "Box"})).unwrap();
        assert_eq!(result["sampled"], true);
        let edges = result["bodies"][0]["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 12);
        for edge in edges {
            assert_eq!(edge["sampled_continuity"], 0);
            assert!((edge["normal_angle_deg"]["value"].as_f64().unwrap() - 90.).abs() < 1e-8);
        }
        assert_eq!(s.undo.len(), undo);
        assert!(std::sync::Arc::ptr_eq(&before, &s.doc));
        assert_eq!(crate::view::continuity_lines(&s).len(), 12);
        s.execute("inspect.continuity", &json!({"clear": true})).unwrap();
        assert!(s.continuity.is_none());
    }

    #[test]
    fn bad_requests_do_not_replace_previous_analysis() {
        let mut s = box_session();
        s.execute("inspect.continuity", &json!({})).unwrap();
        for params in [
            json!({"samples": 0}),
            json!({"samples": 3.5}),
            json!({"samples": 258}),
            json!({"samples": "33"}),
            json!({"body": "missing"}),
            json!({"body": 1}),
            json!({"edges": []}),
            json!({"edges": [[0,0]]}),
            json!({"edges": [{"index": -1}]}),
            json!({"edges": [{"index": 12}]}),
            json!({"gap_tolerance_mm": -1}),
            json!({"angle_tolerance_deg": 181}),
            json!({"clear": "yes"}),
            json!({"edges": [[100,100,100]]}),
        ] {
            assert!(s.execute("inspect.continuity", &params).is_err(), "{params}");
            assert!(s.continuity.is_some());
        }
    }

    #[test]
    fn edge_selection_is_deduplicated_and_geometry_changes_hide_old_lines() {
        let mut s = box_session();
        let result = s.execute("inspect.continuity", &json!({"body": "Box", "edges": [{"index":0},{"index":0}], "samples": 5})).unwrap();
        assert_eq!(result["bodies"][0]["edges"].as_array().unwrap().len(), 1);
        assert_eq!(result["bodies"][0]["edges"][0]["samples_evaluated"], 5);
        s.execute("solid.fillet", &json!({"body": "Box", "edges": [[5,0,0]], "radius":1})).unwrap();
        assert!(crate::view::continuity_lines(&s).is_empty());
    }
}
