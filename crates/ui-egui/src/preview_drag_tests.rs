//! A drag must keep showing completed geometry even when the next distance is already queued.
use super::*;
use serde_json::json;

fn colors() -> Colors {
    Colors { body: [1, 1, 1, 255], added: [2, 2, 2, 255], removed: [3, 3, 3, 100], edge: [0, 0, 0, 255] }
}

fn fixture() -> Session {
    let mut session = Session::new(solidcraft_engine::doc::Document::new("Preview drag"));
    session.execute("sketch.create", &json!({"plane":"XY", "name":"Profile"})).unwrap();
    session.execute("sketch.rectangle.two_point", &json!({"p0":[0,0], "p1":[10,10]})).unwrap();
    session.execute("sketch.finish", &json!({})).unwrap();
    session
}

fn commands(distance: i32) -> Vec<(String, Value)> {
    vec![("solid.extrude".into(), json!({"sketch":"Profile", "distance":distance, "operation":"new"}))]
}

#[test]
fn extrude_drag_displays_completed_geometry_before_the_mouse_stops() {
    let session = fixture();
    let original = serde_json::to_value(&*session.doc).unwrap();
    let undo = session.undo.len();
    let mut pv = PreviewState::default();
    for (finished, wanted) in [(10, 20), (20, 30), (30, 30)] {
        let old = commands(finished);
        let new = commands(wanted);
        let old_key = job_key(&old, session.revision, colors());
        let key = job_key(&new, session.revision, colors());
        let context = stream_key(&new, session.revision, colors());
        assert!(context.is_some());
        let (tx, rx) = std::sync::mpsc::channel();
        pv.running = Some((old_key, rx));
        pv.running_stream_key = stream_key(&old, session.revision, colors());
        tx.send(Msg::Done(compute(&session, &old, colors()))).unwrap();
        pv.poll(key, context);
        assert!(pv.active && pv.error.is_none());
        assert!((pv.bounds.max.z - f64::from(finished)).abs() < 1e-8, "completed distance must reach the viewport");
        assert_eq!(pv.shown_key, Some(old_key));
        assert_eq!(pv.shown_key == Some(key), finished == wanted, "only the exact final distance satisfies the request");
        assert!(pv.running.is_none());
    }
    assert_eq!(serde_json::to_value(&*session.doc).unwrap(), original);
    assert_eq!(session.undo.len(), undo);
}

#[test]
fn extrude_tool_streams_while_its_boolean_is_still_running() {
    let session = fixture();
    let old = commands(10);
    let new = commands(20);
    let old_key = job_key(&old, session.revision, colors());
    let key = job_key(&new, session.revision, colors());
    let (tx, rx) = std::sync::mpsc::channel();
    let mut pv =
        PreviewState { running: Some((old_key, rx)), running_stream_key: stream_key(&old, session.revision, colors()), ..Default::default() };
    tx.send(Msg::Partial(compute(&session, &old, colors()).unwrap())).unwrap();
    pv.poll(key, stream_key(&new, session.revision, colors()));
    assert!(pv.active && pv.running.is_some(), "the tool must show before boolean validation completes");
    assert_eq!(pv.shown_key, Some(old_key));
    // A late boolean failure validates only its old distance, not the newer one.
    tx.send(Msg::Done(Ok(Built { error: Some("old boolean failed".into()), ..compute(&session, &old, colors()).unwrap() }))).unwrap();
    pv.poll(key, stream_key(&new, session.revision, colors()));
    assert!(pv.error.is_none());
    assert_eq!(pv.bounds.max.z, 10.0);
    assert!(pv.running.is_none());
}

#[test]
fn distance_stream_keeps_feature_selection_and_extent_context() {
    let old = commands(10);
    let key = stream_key(&old, 7, colors());
    assert!(key.is_some());
    assert_eq!(key, stream_key(&commands(20), 7, colors()));
    assert_ne!(key, stream_key(&old, 8, colors()));
    let mut recolored = colors();
    recolored.added = [40, 50, 60, 255];
    assert_ne!(key, stream_key(&old, 7, recolored));
    for (field, value) in [
        ("sketch", json!("Other")),
        ("profiles", json!([1])),
        ("face", json!([0, 0, 5])),
        ("operation", json!("cut")),
        ("direction", json!("negative")),
        ("distance2", json!(5)),
        ("to", json!([0, 0, 50])),
        ("taper", json!(5)),
        ("start_offset", json!(3)),
        ("participants", json!(["Other"])),
    ] {
        let mut changed = old.clone();
        changed[0].1[field] = value;
        assert_ne!(key, stream_key(&changed, 7, colors()), "changed {field} cannot reuse the old result");
    }
    for field in ["distance2", "to_offset"] {
        let mut from = old.clone();
        from[0].1[field] = json!(5);
        let mut to = from.clone();
        to[0].1[field] = json!(9);
        assert_eq!(stream_key(&from, 7, colors()), stream_key(&to, 7, colors()));
    }
    let edit =
        |feature, distance| vec![("timeline.redefine".into(), json!({"feature":feature,"command":"solid.extrude","params":commands(distance)[0].1}))];
    assert_eq!(stream_key(&edit(2, 10), 7, colors()), stream_key(&edit(2, 20), 7, colors()));
    assert_ne!(stream_key(&edit(2, 10), 7, colors()), stream_key(&edit(3, 20), 7, colors()));
    assert_ne!(key, stream_key(&edit(2, 10), 7, colors()));
    let offset = |distance| vec![("solid.offset_face".into(), json!({"faces":[[5,5,10]],"distance":distance}))];
    assert!(stream_key(&offset(1), 7, colors()).is_some());
    assert_eq!(stream_key(&offset(1), 7, colors()), stream_key(&offset(5), 7, colors()));
    assert!(stream_key(&[], 7, colors()).is_none());
    assert!(stream_key(&[("solid.fillet".into(), json!({"radius":5}))], 7, colors()).is_none());
    let mut mixed = old;
    mixed.push(("select.clear".into(), json!({})));
    assert!(stream_key(&mixed, 7, colors()).is_none());
}

#[test]
fn old_partial_geometry_is_rejected_after_the_extrude_setup_changes() {
    let session = fixture();
    let old = commands(10);
    let mut new = commands(20);
    new[0].1["operation"] = json!("cut");
    let (tx, rx) = std::sync::mpsc::channel();
    let mut pv =
        PreviewState { running: Some((job_key(&old, 7, colors()), rx)), running_stream_key: stream_key(&old, 7, colors()), ..Default::default() };
    tx.send(Msg::Partial(compute(&session, &old, colors()).unwrap())).unwrap();
    pv.poll(job_key(&new, 7, colors()), stream_key(&new, 7, colors()));
    assert!(!pv.active);
    tx.send(Msg::Done(compute(&session, &old, colors()))).unwrap();
    pv.poll(job_key(&new, 7, colors()), stream_key(&new, 7, colors()));
    assert!(!pv.active);
}
