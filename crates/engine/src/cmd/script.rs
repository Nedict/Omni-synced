//! A design as a command script (#886). `file.export` with `format: script` writes `doc.open`
//! of everything but the timeline, then one `timeline.append` per feature, so
//! `solidcraft-cli eval` replays the design step by step and a repro is trimmed by deleting
//! steps from the end. `doc.open` of a script file builds its design without file access.

use std::sync::Arc;

use serde_json::{Value, json};
use solidcraft_doc::format::{FORMAT, format_string};
use solidcraft_doc::{Document, Feature};

use super::CommandSpec;
use crate::params::{bad, string_list};
use crate::{EngineError, Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("timeline.append", "Append Feature", append).params(
    "feature: a feature as a design file holds it (its id new to the design); format?: the design file format it is written in (default: this build's) → feature, name, error? (a feature that fails to build is still added)",
)];

fn append(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "timeline.append";
    let current = format_string(FORMAT);
    match p.get("format") {
        None => {}
        Some(Value::String(f)) if *f == current => {}
        Some(f) => return Err(bad(cmd, format!("the feature is written in format {f}; this build reads {current}: export the script again"))),
    }
    let feature: Feature = serde_json::from_value(p.get("feature").cloned().ok_or_else(|| bad(cmd, "`feature` is required"))?)
        .map_err(|e| bad(cmd, format!("`feature`: {e}")))?;
    let mut doc = (*s.doc).clone();
    let id = doc.insert_feature(feature)?;
    // The checks a design file gets when it opens: a script is as untrusted as a file.
    // ponytail: re-reads the whole design on every step (quadratic in the script); check the new
    // feature alone if long scripts get slow.
    let doc = Document::from_json(&doc.to_json()).map_err(|e| bad(cmd, format!("`feature`: {e}")))?;
    s.doc = Arc::new(doc);
    s.refresh();
    let name = s.doc.feature(id).map(|f| f.name.clone()).unwrap_or_default();
    let error = s.model.result(id).and_then(|r| r.error.clone());
    Ok(json!({"feature": id, "name": name, "error": error}))
}

/// The design as a script: what a design file holds, with the timeline as steps. Timeline
/// folders are left out (they only organise the timeline) and reported.
fn script(s: &Session) -> (Value, Vec<&'static str>) {
    let design = s.design();
    let mut base = (*design).clone();
    base.features.clear();
    base.marker = None;
    let mut left_out = Vec::new();
    if !base.timeline_groups.is_empty() {
        base.timeline_groups.clear();
        left_out.push("timeline folders");
    }
    let format = format_string(FORMAT);
    base.format = format.clone();
    let mut commands = vec![json!({"command": "doc.open", "params": {"design": base}})];
    for f in &design.features {
        commands.push(json!({"command": "timeline.append", "params": {"format": format, "feature": f}}));
    }
    if let Some(position) = design.marker {
        commands.push(json!({"command": "timeline.roll_to", "params": {"position": position}}));
    }
    let description = format!("{}: the design's timeline, one feature per step (solidcraft-cli eval replays it)", design.name);
    (json!({"description": description, "commands": commands}), left_out)
}

/// A command script file run on a new design (`doc.open` of a script), or `None` when `path`
/// holds no script (a JSON object with `commands` and no `format`). The script runs with no
/// file access, so opening someone's script can't read, write or delete files.
pub fn run_file(path: &str) -> Option<Result<Session>> {
    let text = solidcraft_io::vfs::read_to_string(path, solidcraft_io::MAX_DESIGN_BYTES).ok()?;
    let script: Value = serde_json::from_str(&text).ok()?;
    if script.get("format").is_some() || !script.get("commands").is_some_and(Value::is_array) {
        return None;
    }
    let mut fresh = Session::default();
    let ran = solidcraft_io::vfs::scoped(Some(solidcraft_io::vfs::PathScope::default()), || fresh.run_script(&script));
    Some(ran.map(|_| fresh))
}

/// `file.export` with `format: script`.
pub fn export(s: &Session, path: &str, p: &Value) -> Result<Value> {
    if !string_list(p, "bodies").is_empty() {
        return Err(bad("file.export", "a script holds the complete design; omit bodies"));
    }
    let (script, left_out) = script(s);
    let steps = script["commands"].as_array().map_or(0, Vec::len);
    let bytes = serde_json::to_vec_pretty(&script).map_err(|e| EngineError::Other(e.to_string()))?;
    solidcraft_io::vfs::write(path, &bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    Ok(json!({"path": path, "bytes": bytes.len(), "format": "Script", "steps": steps, "left_out": left_out}))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::Session;

    fn run(s: &mut Session, id: &str, p: Value) -> Value {
        s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
    }

    fn replay(script: &Value) -> Session {
        let mut s = Session::default();
        s.run_script(script).unwrap_or_else(|e| panic!("{e}"));
        s
    }

    fn volume(s: &mut Session) -> f64 {
        run(s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap_or(f64::NAN)
    }

    /// Parameters, a timeline folder, a configuration and a rolled-back marker.
    fn design() -> Session {
        let mut s = Session::default();
        run(&mut s, "parameters.change", json!({"name": "w", "expression": "20 mm", "comment": "width"}));
        run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
        run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [20, 10]}));
        run(&mut s, "sketch.dimension", json!({"entities": ["l1"], "value": "w"}));
        run(&mut s, "sketch.finish", json!({}));
        run(&mut s, "solid.extrude", json!({"sketch": "S", "distance": 5, "name": "Block"}));
        run(&mut s, "solid.fillet", json!({"body": "Body1", "radius": 1, "edges": [[10, 0, 5]], "name": "Round"}));
        run(&mut s, "solid.box", json!({"length": 4, "width": 4, "height": 4, "corner": [40, 0, 0], "operation": "new_body", "name": "Late"}));
        run(&mut s, "timeline.group", json!({"name": "Base", "features": ["S", "Block"]}));
        run(&mut s, "config.configure", json!({"params": ["w"], "features": ["Round"]}));
        run(&mut s, "timeline.roll_to", json!({"feature": "Round"}));
        s
    }

    fn script_of(s: &Session) -> Value {
        let (script, left_out) = super::script(s);
        assert_eq!(left_out, ["timeline folders"]);
        script
    }

    #[test]
    fn a_script_replays_the_design_it_came_from() {
        let mut s = design();
        let script = script_of(&s);
        let steps = script["commands"].as_array().map_or(0, Vec::len);
        assert_eq!(steps, 1 + 4 + 1, "doc.open, a step per feature, the marker: {script}");
        let mut r = replay(&script);
        let mut want = (*s.doc).clone();
        want.timeline_groups.clear();
        assert_eq!(*r.doc, want);
        assert_eq!(r.doc.marker, Some(3));
        assert!((volume(&mut r) - volume(&mut s)).abs() < 1e-9);
        // It is the design, not a copy of its results: parameters still drive it.
        run(&mut r, "parameters.change", json!({"name": "w", "expression": "30 mm"}));
        assert!(volume(&mut r) > 30.0 * 10.0 * 5.0 - 10.0, "{}", volume(&mut r));
    }

    #[test]
    fn a_script_cut_short_replays_the_start_of_the_timeline() {
        let s = design();
        let mut script = script_of(&s);
        let commands = script["commands"].as_array_mut().unwrap();
        commands.truncate(3); // doc.open, the sketch, the extrude
        let mut r = replay(&script);
        assert_eq!(r.doc.features.len(), 2);
        assert!((volume(&mut r) - 20.0 * 10.0 * 5.0).abs() < 1e-6, "{}", volume(&mut r));
    }

    #[test]
    fn appended_features_are_checked_as_a_design_file_is() {
        let s = design();
        let script = script_of(&s);
        let mut r = replay(&json!({"commands": [script["commands"][0].clone(), script["commands"][1].clone()]}));
        let sketch = script["commands"][1]["params"]["feature"].clone();
        let before = r.doc.clone();
        let mut zero = sketch.clone();
        zero["id"] = json!(0);
        let mut huge = sketch.clone();
        huge["id"] = json!(u64::MAX);
        let mut not_a_feature = sketch.clone();
        not_a_feature["type"] = json!("no_such_feature");
        let refused = [
            json!({"feature": sketch}),
            json!({"feature": zero}),
            json!({"feature": huge}),
            json!({"feature": not_a_feature}),
            json!({"feature": 5}),
            json!({}),
            json!({"feature": script["commands"][2]["params"]["feature"], "format": "solidcraft/1"}),
            json!({"feature": script["commands"][2]["params"]["feature"], "format": 14}),
        ];
        for p in refused {
            assert!(r.execute("timeline.append", &p).is_err(), "{p}");
            assert_eq!(r.doc, before, "a refused step leaves the design as it was: {p}");
        }
        assert!(r.execute("doc.open", &json!({"design": {"features": "x"}})).is_err());
        assert!(r.execute("doc.open", &json!({"design": 7})).is_err());
        assert_eq!(r.doc, before);
    }

    /// A script may give a feature the last id there is: features made after it are refused,
    /// never a crash (`next_id + 1` would overflow).
    #[test]
    fn an_id_at_the_end_of_the_range_refuses_later_features() {
        let s = design();
        let script = script_of(&s);
        let mut r = replay(&json!({"commands": [script["commands"][0].clone()]}));
        let mut last = script["commands"][1]["params"]["feature"].clone();
        last["id"] = json!(u64::MAX - 1);
        run(&mut r, "timeline.append", json!({"feature": last}));
        let before = r.doc.clone();
        let e = r.execute("solid.box", &json!({"length": 1, "width": 1, "height": 1}));
        assert!(matches!(e, Err(crate::EngineError::Other(_))), "{e:?}");
        assert_eq!(r.doc, before);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("solidcraft-script-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_script_file_opens_as_the_design_it_builds_unsaved() {
        let mut s = design();
        let dir = temp_dir("open");
        let file = dir.join("part.json").to_string_lossy().to_string();
        run(&mut s, "file.export", json!({"path": file, "format": "script"}));
        let mut r = Session::default();
        let out = run(&mut r, "doc.open", json!({"path": file}));
        assert_eq!(out["script"], true, "{out}");
        let mut want = (*s.doc).clone();
        want.timeline_groups.clear();
        want.name = "part".into(); // an unnamed design takes its file's name, as a design file's does
        assert_eq!(*r.doc, want);
        assert_eq!(r.path, None, "saving must not write a design over the script");
        assert!(r.is_dirty());
        assert!(r.undo.is_empty() && r.redo.is_empty());
        // Not a script: the design reader's own error, as before.
        let other = dir.join("other.json").to_string_lossy().to_string();
        std::fs::write(&other, r#"{"x": 1}"#).unwrap();
        assert!(r.execute("doc.open", &json!({"path": other})).is_err());
        assert_eq!(*r.doc, want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_opened_script_cannot_reach_files() {
        let dir = temp_dir("sandbox");
        let file = dir.join("evil.json").to_string_lossy().to_string();
        let target = dir.join("written.stl");
        let script = json!({"commands": [
            {"command": "solid.box", "params": {"length": 1, "width": 1, "height": 1}},
            {"command": "file.export", "params": {"path": target.to_string_lossy()}}
        ]});
        std::fs::write(&file, script.to_string()).unwrap();
        let mut r = Session::default();
        let before = r.doc.clone();
        let e = r.execute("doc.open", &json!({"path": file}));
        assert!(e.is_err(), "{e:?}");
        assert!(!target.exists(), "the script wrote a file");
        assert_eq!(r.doc, before, "a script that fails leaves the open design alone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_feature_that_fails_is_still_appended() {
        let s = design();
        let script = script_of(&s);
        let mut r = replay(&json!({"commands": [script["commands"][0].clone()]}));
        // The extrude without its sketch: added, and reported as failing.
        let out = run(&mut r, "timeline.append", script["commands"][2]["params"].clone());
        assert!(out["error"].is_string(), "{out}");
        assert_eq!(r.doc.features.len(), 1);
    }
}
