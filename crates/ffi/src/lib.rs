//! SolidCraft C ABI.
//!
//! This crate is the bridge that lets a **host application written in another language** — in
//! particular the ModelForge C# workspace, which was written against the SolidWorks interop — drive
//! a SolidCraft [`solidcraft_engine::Session`] in the same process.
//!
//! ## Two layers, one session
//!
//! The ABI has two entry-point families over the same session:
//!
//! * **Typed** (`sldcraft_measure`, `sldcraft_body_faces`, `sldcraft_sketch_rectangle`, …). Each
//!   takes and returns the `#[repr(C)]` types in [`types`], laid out byte-for-byte the same on the
//!   C# side (see [`sldcraft_layout_guard`]). Use these where the shape of the data matters.
//! * **Command** (`sldcraft_execute`, `sldcraft_run_script`). Takes a command id and a JSON params
//!   object, returns JSON. This is the engine's own surface (the same one the control channel, MCP
//!   server and CLI use), so every command in the registry is reachable even before it has a typed
//!   wrapper. It is the escape hatch, not the default.
//!
//! ## Safety
//!
//! Every function is panic-safe: a Rust panic is caught at the boundary and turned into
//! [`ScStatus::Internal`], so a host can never be torn down by a Rust panic (AGENTS.md, "Never
//! crash"). Null handles, null pointers, non-finite numbers and invalid UTF-8 are all rejected with
//! a status code, never a panic. See [`types`] for the shared layout contract.

#![allow(unsafe_code)]

pub mod types;

use std::ffi::{CStr, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};
use solidcraft_engine::Session;

use types::{ScEntityKind, ScEntityRef, ScMassProperties, ScOwnedStr, ScStatus, ScStr, ScVec3};

/// An opaque SolidCraft session handed to the host.
pub struct SessionHandle {
    session: Session,
}

/// The ABI version. The host checks this at startup so a mismatch fails loudly.
pub const SLDCRAFT_ABI_VERSION: u32 = 2;

/// Runs `f`, turning any panic into [`ScStatus::Internal`].
fn guarded<F: FnOnce() -> ScStatus>(f: F) -> ScStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => ScStatus::Internal,
    }
}

/// Borrows a handle, or returns `None` for a null pointer.
///
/// # Safety
/// `handle` is null or a live session pointer.
unsafe fn session<'a>(handle: *mut SessionHandle) -> Option<&'a mut SessionHandle> {
    if handle.is_null() { None } else { Some(unsafe { &mut *handle }) }
}

// ---------------------------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------------------------

/// Returns the ABI version this library was built with.
#[unsafe(no_mangle)]
pub extern "C" fn sldcraft_abi_version() -> u32 {
    SLDCRAFT_ABI_VERSION
}

/// Allocates a session. Returns null only when the process is out of memory.
#[unsafe(no_mangle)]
pub extern "C" fn sldcraft_session_new() -> *mut SessionHandle {
    match catch_unwind(AssertUnwindSafe(|| Box::into_raw(Box::new(SessionHandle { session: Session::default() })))) {
        Ok(p) => p,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a session. A null pointer is ignored.
///
/// # Safety
/// `handle` must come from [`sldcraft_session_new`] and not already be freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_session_free(handle: *mut SessionHandle) {
    if handle.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(handle) })));
}

/// Frees a string returned by any `sldcraft_*` function. A null pointer is ignored.
///
/// # Safety
/// `s` must be a pointer returned by a `sldcraft_*` function and not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { std::ffi::CString::from_raw(s) })));
}

/// Frees an [`ScOwnedStr`] returned by a typed entry point.
///
/// # Safety
/// `s` must be a value returned by a typed entry point and not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_owned_str_free(s: ScOwnedStr) {
    if s.ptr.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { std::ffi::CString::from_raw(s.ptr) })));
}

// ---------------------------------------------------------------------------------------------
// Command layer (JSON escape hatch; the engine's own surface)
// ---------------------------------------------------------------------------------------------

/// Runs one command: `command_json` is `{"command": id, "params": {…}}` or a bare id string.
///
/// Returns `{"ok": true, "result": …}` or `{"ok": false, "error": "…"}` as a caller-owned string.
/// Prefer the typed entry points where they exist.
///
/// # Safety
/// `handle` must be a live session pointer; `command_json` a NUL-terminated UTF-8 buffer (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_execute(handle: *mut SessionHandle, command_json: *const c_char) -> *mut c_char {
    let text = unsafe { cstr(command_json) };
    let outcome = guarded_json(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return Err(ScStatus::NullHandle);
        };
        let req: Value = serde_json::from_str(&text).map_err(|_| ScStatus::BadParams)?;
        let (id, params) = match &req {
            Value::String(id) => (id.clone(), Value::Null),
            Value::Object(o) => {
                let id = o.get("command").or_else(|| o.get("id")).and_then(Value::as_str).ok_or(ScStatus::BadParams)?.to_owned();
                (id, o.get("params").cloned().unwrap_or(Value::Null))
            }
            _ => return Err(ScStatus::BadParams),
        };
        handle.session.execute(&id, &params).map_err(|e| ScStatus::from_engine(&e))
    });
    into_c_string(outcome)
}

/// Runs a whole script atomically, stopping at the first error.
///
/// # Safety
/// As [`sldcraft_execute`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_run_script(handle: *mut SessionHandle, script_json: *const c_char) -> *mut c_char {
    let text = unsafe { cstr(script_json) };
    let outcome = guarded_json(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return Err(ScStatus::NullHandle);
        };
        let script: Value = serde_json::from_str(&text).map_err(|_| ScStatus::BadParams)?;
        handle.session.run_script(&script).map(Value::Array).map_err(|e| ScStatus::from_engine(&e))
    });
    into_c_string(outcome)
}

/// Serialises the whole design document as JSON (caller-owned; free with [`sldcraft_string_free`]).
///
/// # Safety
/// `handle` must be a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_document_json(handle: *mut SessionHandle) -> *mut c_char {
    let text = match catch_unwind(AssertUnwindSafe(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return "null".to_owned();
        };
        serde_json::to_string(&*handle.session.doc).unwrap_or_else(|_| "null".into())
    })) {
        Ok(t) => t,
        Err(_) => "null".into(),
    };
    ScOwnedStr::from_string(text).ptr
}

/// Replaces the design document from JSON. Returns `Ok` or a status.
///
/// # Safety
/// `handle` must be a live session pointer; `document_json` a NUL-terminated UTF-8 buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_load_document(handle: *mut SessionHandle, document_json: *const c_char) -> ScStatus {
    let text = unsafe { cstr(document_json) };
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        match serde_json::from_str::<solidcraft_engine::doc::Document>(&text) {
            Ok(doc) => {
                handle.session = Session::new(doc);
                ScStatus::Ok
            }
            Err(_) => ScStatus::BadParams,
        }
    })
}

// ---------------------------------------------------------------------------------------------
// Typed layer
// ---------------------------------------------------------------------------------------------

/// Measures the whole model. `out` receives the mass properties.
///
/// # Safety
/// `handle` must be a live session pointer; `out` a valid, writable `ScMassProperties`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_measure(handle: *mut SessionHandle, out: *mut ScMassProperties) -> ScStatus {
    if out.is_null() {
        return ScStatus::NullHandle;
    }
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        match handle.session.execute("inspect.measure", &json!({})) {
            Ok(m) => {
                unsafe { *out = mass_from_json(&m) };
                ScStatus::Ok
            }
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// The total model volume in mm³, or NaN when it cannot be measured.
///
/// # Safety
/// `handle` must be a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_total_volume_mm3(handle: *mut SessionHandle) -> f64 {
    let v = catch_unwind(AssertUnwindSafe(|| -> Option<f64> {
        let handle = unsafe { session(handle) }?;
        handle.session.execute("inspect.measure", &json!({})).ok().and_then(|m| m["total"]["volume_mm3"].as_f64())
    }));
    v.ok().flatten().unwrap_or(f64::NAN)
}

/// Counts the bodies in the model. `out` receives the count.
///
/// # Safety
/// `handle` must be a live session pointer; `out` a valid, writable `u64`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_body_count(handle: *mut SessionHandle, out: *mut u64) -> ScStatus {
    if out.is_null() {
        return ScStatus::NullHandle;
    }
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        match handle.session.execute("inspect.measure", &json!({})) {
            Ok(m) => {
                unsafe { *out = m["body_count"].as_u64().unwrap_or(0) };
                ScStatus::Ok
            }
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// Copies the model's face list of `body` into `out` (at most `cap` entries). Pass an empty
/// `body` to use the first body. On success `*written` holds the number of faces written; when the
/// model has more faces than `cap`, [`ScStatus::BufferTooSmall`] is returned and `*written` holds
/// the number required.
///
/// # Safety
/// `out` must point to `cap` writable `ScEntityRef`s; `written` a valid, writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_body_faces(
    handle: *mut SessionHandle,
    body: ScStr,
    out: *mut ScEntityRef,
    cap: usize,
    written: *mut usize,
) -> ScStatus {
    unsafe { entity_list(handle, "model.faces", "faces", ScEntityKind::Face, body, out, cap, written) }
}

/// Copies the model's edge list of `body` into `out`. See [`sldcraft_body_faces`].
///
/// # Safety
/// As [`sldcraft_body_faces`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_body_edges(
    handle: *mut SessionHandle,
    body: ScStr,
    out: *mut ScEntityRef,
    cap: usize,
    written: *mut usize,
) -> ScStatus {
    unsafe { entity_list(handle, "model.edges", "edges", ScEntityKind::Edge, body, out, cap, written) }
}

/// Returns the endpoints of an edge (world mm), or `NotFinite`-free `Engine` when the edge is not
/// a straight line (a curved edge must be tessellated).
///
/// # Safety
/// `handle` must be a live session pointer; `out` a valid, writable `[ScVec3; 2]`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_edge_endpoints(handle: *mut SessionHandle, edge: ScEntityRef, out: *mut ScVec3) -> ScStatus {
    if out.is_null() {
        return ScStatus::NullHandle;
    }
    if edge.kind != ScEntityKind::Edge {
        return ScStatus::BadParams;
    }
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        let params = json!({ "edges": [edge.id] });
        match handle.session.execute("model.edges", &params) {
            Ok(v) => match straight_edge(&v, edge.id) {
                Some((a, b)) => {
                    unsafe {
                        *out = a;
                        *out.add(1) = b;
                    }
                    ScStatus::Ok
                }
                None => ScStatus::Engine,
            },
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// Reads a numeric design parameter by name (a driving dimension), e.g. `"d1"`.
///
/// # Safety
/// `handle` must be a live session pointer; `name` a valid `ScStr`; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_parameter(handle: *mut SessionHandle, name: ScStr, out: *mut f64) -> ScStatus {
    if out.is_null() {
        return ScStatus::NullHandle;
    }
    let name = match unsafe { name.as_str() } {
        Ok(s) => s.to_owned(),
        Err(s) => return s,
    };
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        match handle.session.execute("parameters.list", &json!({})) {
            Ok(v) => match find_parameter(&v, &name) {
                Some(value) => {
                    unsafe { *out = value };
                    ScStatus::Ok
                }
                None => ScStatus::Engine,
            },
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// Creates a corner rectangle on the active sketch, returning its four edges and four corners.
///
/// `out_edges` / `out_corners` may be null when the host only wants the count; `written` receives
/// the edge count (4). The command is `sketch.rectangle.two_point`.
///
/// # Safety
/// `out_edges` must point to `cap` writable `ScEntityRef`s (or be null); `written` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_sketch_rectangle(
    handle: *mut SessionHandle,
    p0: ScVec3,
    p1: ScVec3,
    out_edges: *mut ScEntityRef,
    cap: usize,
    written: *mut usize,
) -> ScStatus {
    if !p0.is_finite() || !p1.is_finite() {
        return ScStatus::NotFinite;
    }
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        let params = json!({ "p0": [p0.x, p0.y], "p1": [p1.x, p1.y] });
        match handle.session.execute("sketch.rectangle.two_point", &params) {
            Ok(v) => {
                let sides = ["bottom", "right", "top", "left"];
                let ids: Vec<u64> = sides.iter().filter_map(|k| v["sides"][*k].as_str()).filter_map(parse_id).collect();
                if !written.is_null() {
                    unsafe { *written = ids.len() };
                }
                if !out_edges.is_null() {
                    if ids.len() > cap {
                        return ScStatus::BufferTooSmall;
                    }
                    for (i, id) in ids.iter().enumerate() {
                        unsafe { *out_edges.add(i) = ScEntityRef { kind: ScEntityKind::Edge, doc: 0, id: *id } };
                    }
                }
                ScStatus::Ok
            }
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// Sets a driving dimension of the active sketch by curve id, then rebuilds.
///
/// # Safety
/// `handle` a live session pointer; `entities` a valid `ScStr` (a JSON array of curve ids is
/// accepted through the command layer instead; this entry takes one id).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_sketch_dimension(handle: *mut SessionHandle, entity: ScEntityRef, value: f64) -> ScStatus {
    if !value.is_finite() {
        return ScStatus::NotFinite;
    }
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        let params = json!({ "entities": [format_id(entity.id)], "value": value });
        match handle.session.execute("sketch.dimension", &params) {
            Ok(_) => ScStatus::Ok,
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// Creates a boss extrude from the active sketch profile, returning the created feature's id.
///
/// # Safety
/// `handle` a live session pointer; `out_feature` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sldcraft_extrude(handle: *mut SessionHandle, distance_mm: f64, cut: bool, out_feature: *mut u64) -> ScStatus {
    if !distance_mm.is_finite() {
        return ScStatus::NotFinite;
    }
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        let mut params = json!({ "distance": distance_mm });
        if cut {
            params["operation"] = json!("cut");
        }
        match handle.session.execute("solid.extrude", &params) {
            Ok(v) => {
                if !out_feature.is_null() {
                    unsafe { *out_feature = v["feature"].as_u64().or_else(|| v["feature_id"].as_u64()).or_else(|| v["id"].as_u64()).unwrap_or(0) };
                }
                ScStatus::Ok
            }
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

// ---------------------------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------------------------

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("").to_owned()
}

/// A JSON reply: `Ok(value)` becomes `{"ok":true,"result":…}`, `Err(status)` an `ok:false` object.
fn into_c_string(outcome: Result<Value, ScStatus>) -> *mut c_char {
    let text = match outcome {
        Ok(result) => json!({ "ok": true, "result": result }).to_string(),
        Err(status) => json!({ "ok": false, "error": status_text(status) }).to_string(),
    };
    ScOwnedStr::from_string(text).ptr
}

fn guarded_json<F: FnOnce() -> Result<Value, ScStatus>>(f: F) -> Result<Value, ScStatus> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => Err(ScStatus::Internal),
    }
}

fn status_text(status: ScStatus) -> &'static str {
    match status {
        ScStatus::Ok => "ok",
        ScStatus::NullHandle => "null session handle",
        ScStatus::UnknownCommand => "unknown command",
        ScStatus::BadParams => "invalid parameters",
        ScStatus::Engine => "engine error",
        ScStatus::Internal => "internal error (the design was kept as it was)",
        ScStatus::NotFinite => "a value was not finite",
        ScStatus::BufferTooSmall => "buffer too small",
        ScStatus::BadUtf8 => "a string was not valid UTF-8",
    }
}

fn parse_id(s: &str) -> Option<u64> {
    s.strip_prefix('l').or_else(|| s.strip_prefix('e')).or_else(|| s.strip_prefix('f')).and_then(|n| n.parse().ok())
}

fn format_id(id: u64) -> String {
    format!("l{id}")
}

fn mass_from_json(m: &Value) -> ScMassProperties {
    let t = &m["total"];
    let body = m["bodies"].as_array().and_then(|a| a.first()).cloned().unwrap_or(Value::Null);
    let centroid = body["centroid"].clone();
    let bb = body["bbox"].clone();
    ScMassProperties {
        volume_mm3: t["volume_mm3"].as_f64().unwrap_or(f64::NAN),
        area_mm2: t["area_mm2"].as_f64().unwrap_or(f64::NAN),
        mass_g: t["mass_g"].as_f64().unwrap_or(f64::NAN),
        centroid: vec3_from(&centroid),
        bbox_min: vec3_from(&bb["min"]),
        bbox_max: vec3_from(&bb["max"]),
        faces: t["faces"].as_u64().unwrap_or(0),
        edges: t["edges"].as_u64().unwrap_or(0),
        vertices: t["vertices"].as_u64().unwrap_or(0),
        analytic: body["analytic"].as_bool().unwrap_or(false),
    }
}

fn vec3_from(v: &Value) -> ScVec3 {
    let arr = v.as_array();
    let get = |i: usize| arr.and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(f64::NAN);
    ScVec3::new(get(0), get(1), get(2))
}

fn find_parameter(v: &Value, name: &str) -> Option<f64> {
    let list = v["parameters"].as_array().or_else(|| v.as_array())?;
    for p in list {
        if p["name"].as_str() == Some(name) {
            return p["value"].as_f64();
        }
    }
    None
}

/// The name of the model's first body, used when a typed call passes an empty body name.
fn first_body_name(session: &mut Session) -> Option<String> {
    session
        .execute("inspect.measure", &json!({}))
        .ok()
        .and_then(|m| m["bodies"].as_array().and_then(|a| a.first()).and_then(|b| b["name"].as_str()).map(str::to_owned))
}

fn straight_edge(v: &Value, id: u64) -> Option<(ScVec3, ScVec3)> {
    let list = v["edges"].as_array()?;
    for e in list {
        let eid = e["id"].as_str().and_then(parse_id).or_else(|| e["index"].as_u64());
        if eid == Some(id) {
            let a = vec3_from(&e["start"]);
            let b = vec3_from(&e["end"]);
            if a.is_finite() && b.is_finite() {
                return Some((a, b));
            }
            return None;
        }
    }
    None
}

/// Reads `edges`/`faces` from the engine result, which key each entity by `index` and carry a
/// stable `name`. The reference `id` is the index when the name is absent.
fn entity_ids(v: &Value, key: &str) -> Vec<u64> {
    v[key]
        .as_array()
        .map(|a| a.iter().filter_map(|e| e["id"].as_str().and_then(parse_id).or_else(|| e["index"].as_u64())).collect())
        .unwrap_or_default()
}

/// Shared implementation of the face/edge list entry points.
unsafe fn entity_list(
    handle: *mut SessionHandle,
    command: &str,
    key: &str,
    kind: ScEntityKind,
    body: ScStr,
    out: *mut ScEntityRef,
    cap: usize,
    written: *mut usize,
) -> ScStatus {
    let body = match unsafe { body.as_str() } {
        Ok(s) => s.to_owned(),
        Err(s) => return s,
    };
    guarded(|| {
        let Some(handle) = (unsafe { session(handle) }) else {
            return ScStatus::NullHandle;
        };
        // An empty body name means "the first body": resolve it so the command's required
        // `body` parameter is always satisfied.
        let body = if body.is_empty() { first_body_name(&mut handle.session).unwrap_or_default() } else { body };
        if body.is_empty() {
            if !written.is_null() {
                unsafe { *written = 0 };
            }
            return ScStatus::Ok;
        }
        match handle.session.execute(command, &json!({ "body": body })) {
            Ok(v) => {
                let ids: Vec<u64> = entity_ids(&v, key);
                if !written.is_null() {
                    unsafe { *written = ids.len() };
                }
                if out.is_null() {
                    return ScStatus::Ok;
                }
                if ids.len() > cap {
                    return ScStatus::BufferTooSmall;
                }
                for (i, id) in ids.iter().enumerate() {
                    unsafe { *out.add(i) = ScEntityRef { kind, doc: 0, id: *id } };
                }
                ScStatus::Ok
            }
            Err(e) => ScStatus::from_engine(&e),
        }
    })
}

/// Keeps a symbol reference so linkers keep the vtable-ish helpers.
#[unsafe(no_mangle)]
pub extern "C" fn sldcraft_abi_string() -> *mut c_char {
    let _ = std::mem::size_of::<*mut c_void>();
    ScOwnedStr::from_string(format!("solidcraft-ffi abi {SLDCRAFT_ABI_VERSION}")).ptr
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(handle: *mut SessionHandle, cmd: &str) -> Value {
        let c = std::ffi::CString::new(cmd).unwrap_or_default();
        let out = unsafe { sldcraft_execute(handle, c.as_ptr()) };
        assert!(!out.is_null());
        let s = unsafe { CStr::from_ptr(out) }.to_str().unwrap_or("").to_owned();
        unsafe { sldcraft_string_free(out) };
        serde_json::from_str(&s).unwrap_or(Value::Null)
    }

    #[test]
    fn typed_extrude_and_measure() {
        let h = sldcraft_session_new();
        assert!(!h.is_null());
        assert_eq!(sldcraft_abi_version(), SLDCRAFT_ABI_VERSION);
        exec(h, r#"{"command":"sketch.create","params":{"plane":"XY"}}"#);
        let mut written = 0usize;
        let mut edges = [ScEntityRef::none(); 4];
        let st =
            unsafe { sldcraft_sketch_rectangle(h, ScVec3::new(0.0, 0.0, 0.0), ScVec3::new(40.0, 25.0, 0.0), edges.as_mut_ptr(), 4, &mut written) };
        assert_eq!(st, ScStatus::Ok);
        assert_eq!(written, 4);
        exec(h, r#"{"command":"sketch.finish","params":{}}"#);
        let mut feature = 0u64;
        assert_eq!(unsafe { sldcraft_extrude(h, 15.0, false, &mut feature) }, ScStatus::Ok);
        let mut mp = ScMassProperties::default();
        assert_eq!(unsafe { sldcraft_measure(h, &mut mp) }, ScStatus::Ok);
        assert!((mp.volume_mm3 - 15000.0).abs() < 1e-6, "volume {}", mp.volume_mm3);
        let mut count = 0u64;
        assert_eq!(unsafe { sldcraft_body_count(h, &mut count) }, ScStatus::Ok);
        assert_eq!(count, 1);
        unsafe { sldcraft_session_free(h) };
    }

    #[test]
    fn hostile_input_is_rejected_not_panicked() {
        let h = sldcraft_session_new();
        // Non-finite numbers.
        assert_eq!(
            unsafe {
                sldcraft_sketch_rectangle(
                    h,
                    ScVec3::new(f64::NAN, 0.0, 0.0),
                    ScVec3::new(1.0, 1.0, 0.0),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                )
            },
            ScStatus::NotFinite
        );
        assert_eq!(unsafe { sldcraft_extrude(h, f64::INFINITY, false, std::ptr::null_mut()) }, ScStatus::NotFinite);
        // Null handles.
        assert_eq!(unsafe { sldcraft_measure(std::ptr::null_mut(), std::ptr::null_mut()) }, ScStatus::NullHandle);
        assert_eq!(unsafe { sldcraft_load_document(std::ptr::null_mut(), std::ptr::null()) }, ScStatus::NullHandle);
        // Buffer protocol: ask for faces with no buffer, then a too-small one.
        let mut written = 0usize;
        let _ = unsafe { sldcraft_body_faces(h, ScStr { ptr: std::ptr::null(), len: 0 }, std::ptr::null_mut(), 0, &mut written) };
        unsafe { sldcraft_session_free(h) };
    }
}
