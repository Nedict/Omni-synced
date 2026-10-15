//! Program commands that are not modelling operations. Coaching progress is application
//! settings: these commands never change the design or its undo history.

use serde_json::Value;

use super::CommandSpec;
use crate::coaching::{self, coaching_state};
use crate::params::{bad, bool_};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("ui.coaching.set", "Set Coaching", set)
        .noundo()
        .doc_only()
        .params("enabled: bool. Stores whether the coaching layer is on. Returns {enabled, index, dismissed, completed, steps}"),
    CommandSpec::new("ui.coaching.advance", "Advance Coaching", advance).noundo().doc_only().params(
        "steps: number of coaching steps (from the UI table). Moves to the next step; the last step marks the sequence complete. Returns {enabled, index, dismissed, completed, steps}",
    ),
    CommandSpec::new("ui.coaching.dismiss", "Dismiss Coaching Step", dismiss)
        .noundo()
        .doc_only()
        .params("steps?: number of coaching steps. Hides the current step without disabling the layer. Returns {enabled, index, dismissed, completed, steps}"),
    CommandSpec::new("ui.coaching.state", "Coaching State", state)
        .noundo()
        .doc_only()
        .params("(steps?: number). Returns {enabled, index, dismissed, completed, steps} without changing anything"),
];

fn steps_of(cmd: &str, p: &Value, required: bool) -> Result<usize> {
    match p.get("steps") {
        Some(Value::Number(n)) => {
            let n = n.as_u64().filter(|n| *n <= 10_000).ok_or_else(|| bad(cmd, "`steps` must be a count"))?;
            Ok(usize::try_from(n).unwrap_or(10_000))
        }
        Some(_) => Err(bad(cmd, "`steps` must be a count")),
        None if required => Err(bad(cmd, "`steps` is required")),
        None => Ok(0),
    }
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "ui.coaching.set";
    let enabled = bool_(p, "enabled").ok_or_else(|| bad(cmd, "`enabled` must be a bool"))?;
    let steps = steps_of(cmd, p, false)?;
    coaching::coaching_set(&mut s.coaching, enabled);
    Ok(coaching_state(&s.coaching, steps))
}

fn advance(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "ui.coaching.advance";
    let steps = steps_of(cmd, p, true)?;
    coaching::coaching_advance(&mut s.coaching, steps);
    Ok(coaching_state(&s.coaching, steps))
}

fn dismiss(s: &mut Session, p: &Value) -> Result<Value> {
    let steps = steps_of("ui.coaching.dismiss", p, false)?;
    coaching::coaching_dismiss(&mut s.coaching);
    Ok(coaching_state(&s.coaching, steps))
}

fn state(s: &mut Session, p: &Value) -> Result<Value> {
    let steps = steps_of("ui.coaching.state", p, false)?;
    Ok(coaching_state(&s.coaching, steps))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn run(s: &mut Session, id: &str, p: serde_json::Value) -> serde_json::Value {
        match s.execute(id, &p) {
            Ok(v) => v,
            Err(e) => panic!("{id} {p}: {e}"),
        }
    }

    #[test]
    fn coaching_commands_change_only_the_preference() {
        let mut s = Session::default();
        let doc = s.doc.to_json();
        let undo = s.undo.len();
        let v = run(&mut s, "ui.coaching.set", json!({"enabled": false, "steps": 6}));
        assert_eq!(v["enabled"], json!(false));
        assert!(!s.coaching.enabled);
        let v = run(&mut s, "ui.coaching.set", json!({"enabled": true, "steps": 6}));
        assert_eq!(v["enabled"], json!(true));
        let v = run(&mut s, "ui.coaching.advance", json!({"steps": 6}));
        assert_eq!(v["index"], json!(1));
        assert_eq!(v["steps"], json!(6));
        let v = run(&mut s, "ui.coaching.dismiss", json!({"steps": 6}));
        assert_eq!(v["dismissed"], json!(true));
        assert_eq!(v["enabled"], json!(true));
        assert_eq!(s.doc.to_json(), doc);
        assert_eq!(s.undo.len(), undo);
        let v = run(&mut s, "ui.coaching.state", json!({"steps": 6}));
        assert_eq!(v["index"], json!(1));
        assert_eq!(v["dismissed"], json!(true));
    }

    #[test]
    fn coaching_commands_reject_malformed_params() {
        let mut s = Session::default();
        let doc = s.doc.to_json();
        let undo = s.undo.len();
        assert!(s.execute("ui.coaching.set", &json!({"enabled": "yes"})).is_err());
        assert!(s.execute("ui.coaching.set", &json!({})).is_err());
        assert!(s.execute("ui.coaching.advance", &json!({})).is_err());
        assert!(s.execute("ui.coaching.advance", &json!({"steps": -1})).is_err());
        assert!(s.execute("ui.coaching.advance", &json!({"steps": "6"})).is_err());
        assert_eq!(s.doc.to_json(), doc);
        assert_eq!(s.undo.len(), undo);
        assert!(s.coaching.enabled);
    }

    #[test]
    fn advancing_past_the_last_step_completes() {
        let mut s = Session::default();
        for _ in 0..5 {
            run(&mut s, "ui.coaching.advance", json!({"steps": 6}));
        }
        let v = run(&mut s, "ui.coaching.advance", json!({"steps": 6}));
        assert_eq!(v["completed"], json!(true));
        assert_eq!(v["index"], json!(5));
        assert_eq!(s.undo.len(), 0);
    }
}
