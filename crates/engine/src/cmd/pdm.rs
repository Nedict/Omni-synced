//! Local PDM commands enqueue work for the host; no command accepts a credential.
use super::CommandSpec;
use crate::{
    EngineError, Result, Session,
    pdm::{Reference, Task, valid_document},
};
use serde_json::{Value, json};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("pdm.panel", "Local PDM", status).icon("library").noundo().doc_only().params("none; programmatic calls return status, only interactive start opens the panel"),
    CommandSpec::new("pdm.status", "Local PDM Status", status).icon("library").noundo().doc_only().params("none; host-only connection/job/version state, never credentials"),
    CommandSpec::new("pdm.connect", "Connect Local PDM", connect).icon("library").noundo().doc_only().params("port: local TCP port; credential comes from the native host, never params"),
    CommandSpec::new("pdm.disconnect", "Disconnect Local PDM", disconnect).icon("library").noundo().doc_only().params("none; refused while an operation is running"),
    CommandSpec::new("pdm.refresh", "Refresh PDM Versions", refresh).icon("library").noundo().doc_only().params("none; list exact document versions"),
    CommandSpec::new("pdm.check_in", "Check In Saved Design", check_in).icon("save").noundo().doc_only().params("document: lowercase identifier; parent?: exact expected latest version (omit for creation); dependencies?: [{document,version}]; active design must already be saved and unchanged"),
    CommandSpec::new("pdm.retrieve", "Retrieve PDM Working Copy", retrieve).icon("open").noundo().doc_only().params("document, version; open?: bool (default true); create a NEW private working copy and optionally open a NEW tab; never replace current design"),
];
fn err(message: impl Into<String>) -> EngineError {
    EngineError::Other(message.into())
}
/// Validate the entire supported parameter shape BEFORE the engine records a command.
/// In particular, accidental token/credential fields must never enter the crash journal.
pub(crate) fn validate_params(id: &str, p: &Value) -> Result<()> {
    let keys: &[&str] = match id {
        "pdm.connect" => &["port"],
        "pdm.check_in" => &["document", "parent", "dependencies"],
        "pdm.retrieve" => &["document", "version", "open"],
        _ => &[],
    };
    if !p.is_null() && !p.is_object() {
        return Err(err("Invalid local PDM parameters; credentials belong only to the native host"));
    }
    if p.as_object().is_some_and(|p| p.keys().any(|key| !keys.contains(&key.as_str()))) {
        return Err(err("Unsupported local PDM parameter; credentials belong only to the native host"));
    }
    match id {
        "pdm.connect" => {
            if p.get("port").and_then(Value::as_u64).filter(|v| *v > 0 && *v <= u16::MAX as u64).is_none() {
                return Err(err("A valid local PDM port is required"));
            }
        }
        "pdm.check_in" => {
            document(p)?;
            if p.get("parent").is_some_and(|v| !v.is_null() && v.as_u64().filter(|v| *v > 0).is_none()) {
                return Err(err("Expected parent must be a positive version"));
            }
            if let Some(value) = p.get("dependencies") {
                let dependencies: Vec<Reference> = serde_json::from_value(value.clone()).map_err(|_| err("Invalid PDM dependencies"))?;
                if dependencies.len() > 256 {
                    return Err(err("Too many PDM dependencies"));
                }
                for reference in dependencies {
                    reference.validate().map_err(err)?;
                }
            }
        }
        "pdm.retrieve" => {
            document(p)?;
            if p.get("version").and_then(Value::as_u64).filter(|v| *v > 0).is_none() {
                return Err(err("A positive PDM version is required"));
            }
            if p.get("open").is_some_and(|v| !v.is_boolean()) {
                return Err(err("open must be boolean"));
            }
        }
        _ => {}
    }
    Ok(())
}
fn enqueue(s: &Session, task: Task) -> Result<Value> {
    Ok(json!({"queued":true,"request":s.pdm.enqueue(task).map_err(err)?}))
}
fn status(s: &mut Session, _: &Value) -> Result<Value> {
    serde_json::to_value(s.pdm.status().map_err(err)?).map_err(|_| err("Local PDM status unavailable"))
}
fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    let port = p
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p > 0)
        .ok_or_else(|| err("A valid local PDM port is required"))?;
    enqueue(s, Task::Connect { port })
}
fn disconnect(s: &mut Session, _: &Value) -> Result<Value> {
    enqueue(s, Task::Disconnect)
}
fn refresh(s: &mut Session, _: &Value) -> Result<Value> {
    enqueue(s, Task::Refresh)
}
fn document(p: &Value) -> Result<String> {
    let document = p
        .get("document")
        .and_then(Value::as_str)
        .filter(|id| valid_document(id))
        .ok_or_else(|| err("A valid PDM document identifier is required"))?;
    Ok(document.into())
}
fn check_in(s: &mut Session, p: &Value) -> Result<Value> {
    if s.active_sketch.is_some() || !s.pending_moves.is_empty() {
        return Err(err("Finish active edits before checking in the saved design"));
    }
    if s.is_dirty() {
        return Err(err("Save the active design before checking it in"));
    }
    let path = s.path.clone().filter(|p| solidcraft_io::is_design_path(p)).ok_or_else(|| err("Save the active design as a SolidCraft file first"))?;
    let document = document(p)?;
    let parent = match p.get("parent").filter(|v| !v.is_null()) {
        Some(value) => Some(value.as_u64().filter(|v| *v > 0).ok_or_else(|| err("Expected parent must be a positive version"))?),
        None => None,
    };
    let dependencies: Vec<Reference> = match p.get("dependencies") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| err("Invalid PDM dependencies"))?,
        None => Vec::new(),
    };
    if dependencies.len() > 256 {
        return Err(err("Too many PDM dependencies"));
    }
    for dependency in &dependencies {
        dependency.validate().map_err(err)?;
    }
    enqueue(s, Task::CheckIn { document, parent, path, dependencies, saved: s.doc.clone(), visibility: Box::new(s.visibility.clone()) })
}
fn retrieve(s: &mut Session, p: &Value) -> Result<Value> {
    let reference = Reference {
        document: document(p)?,
        version: p.get("version").and_then(Value::as_u64).filter(|v| *v > 0).ok_or_else(|| err("A positive PDM version is required"))?,
    };
    let open = match p.get("open") {
        Some(v) => v.as_bool().ok_or_else(|| err("open must be boolean"))?,
        None => true,
    };
    enqueue(s, Task::Retrieve { reference, open })
}
