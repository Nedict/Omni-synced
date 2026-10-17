//! Booleans on a body the faceted route made (#1910). The route flattens the faces where two
//! bodies curve into each other, so a pair of cylinders of six and four faces comes back as a
//! body of some seventeen hundred: every face a facet, and every facet's mesh a hair off its
//! own plane. Two things followed from that, and together they made a later boolean stop
//! returning at all:
//!
//! - the exact polygon fallback refuses such a body, and it refuses it in the same words it
//!   refuses a body whose faces genuinely curve, so every later boolean read it as a curved
//!   contact and faceted the body again - hundreds of times the facets, each pass heavier;
//! - the retries underneath a boolean (a nudged tool, faces pushed apart) each run every route
//!   above them again, so one operation on such a body ran the whole ladder seven times over,
//!   eighty-four B-rep booleans, on a body of seventeen hundred faces instead of four.
//!
//! Now a body already faceted is not faceted again, and the retries that cannot help it are
//! not run on it: the same operation costs twelve B-rep booleans instead of eighty-four.

use solidcraft_geom::Vec3;

use crate::*;

const R: f64 = 25.6;

/// The #1865 pair: a bored cylinder and the one standing on it, overlapping along the axis they
/// share. The bore leaves no prism along that axis, so every exact route refuses them and the
/// faceted one answers - which is what leaves the body this file is about.
fn bored_and_upper() -> (Body, Body) {
    let lower = cylinder(Vec3::ZERO, Vec3::Z, R, 30.0).unwrap();
    let upper = cylinder(Vec3::new(0.0, 0.0, 22.0), Vec3::Z, R, 24.0).unwrap();
    let drill = cylinder(Vec3::new(-40.0, 0.0, 10.0), Vec3::X, 4.0, 80.0).unwrap();
    let bored = boolean(&lower, &drill, BoolOp::Cut).unwrap().unwrap();
    (bored, upper)
}

/// The body the faceted route leaves: all facets, each a face of its own.
fn faceted_body() -> Body {
    let (bored, upper) = bored_and_upper();
    let body = boolean(&bored, &upper, BoolOp::Union).unwrap().unwrap();
    assert!(body.face_count() > 500, "the faceted route should leave a body of facets, got {} faces", body.face_count());
    body
}

#[test]
fn a_body_the_faceted_route_made_is_not_faceted_again() {
    // A third cylinder standing on the faceted body: another contact no exact route takes, so
    // the gate that chooses the faceted route is asked about this pair too.
    let faceted = faceted_body();
    let third = cylinder(Vec3::new(0.0, 0.0, 40.0), Vec3::Z, R, 20.0).unwrap();

    let t = std::time::Instant::now();
    let (r, how) = capture_boolean_diagnostics(|| boolean(&faceted, &third, BoolOp::Union));
    let took = t.elapsed();

    // Faceting it again answers nothing - it is already flat where it curved - and comes to
    // hundreds of times the facets it was made from, so each pass costs more than the last.
    // The route must not be asked at all here: not taken, and not even tried and refused.
    assert!(!how.events.iter().any(|e| e.strategy == BooleanStrategy::FacetedFallback), "a body the faceted route made was faceted again: {how:?}");
    if let Err(e) = &r {
        assert!(!e.to_string().contains("faceted"), "the operation spent itself faceting an already faceted body: {e}");
    }
    // Whether this pair can be answered at all is a separate question (today it cannot, and it
    // says so). That it answers at all is this one: what the fix replaces is a boolean that
    // returned neither a result nor an error for as long as anyone waited. The bound is loose
    // on purpose, and says nothing about how long the operation ought to take: it is there so
    // that a boolean which stops returning fails this test rather than hanging the suite.
    assert!(took.as_secs() < 60, "the boolean took {took:?}");
    if let Ok(Some(body)) = &r {
        assert!(body.validity().is_empty(), "{:?}", body.validity());
    }
}

#[test]
fn the_faceted_route_is_asked_by_the_operation_not_by_its_retries() {
    // The operation asks the last resort on its own behalf, once. The retries nested under it
    // do not ask again: their tool differs from the operation's by a screw motion of two parts
    // in a million, which decides nothing the facets decide, and asking cost seven times over.
    let (bored, upper) = bored_and_upper();
    let (r, how) = capture_boolean_diagnostics(|| boolean(&bored, &upper, BoolOp::Union));
    r.unwrap().expect("a result");
    let faceted = how.events.iter().filter(|e| e.strategy == BooleanStrategy::FacetedFallback).count();
    assert_eq!(faceted, 1, "the faceted route answered {faceted} times for one operation: {how:?}");
    // And it is still reported as what it is: a result that is flat where the faces curve.
    assert!(how.warning().is_some(), "a result that is not the exact one is warned about");
}

#[test]
fn facets_that_cut_each_other_without_end_are_refused_not_run() {
    // The polygon boolean's own budget. Two stacks of thin plates crossing at a shallow angle:
    // every plate's plane cuts a share of every other, which is how a faceted body's facets
    // behave, and the pieces multiply far past anything the polygon count and the tree's depth
    // bound between them. Whatever the budget decides here it decides: it does not run on.
    let mut a = box_solid(Vec3::new(-50.0, -50.0, -0.05), Vec3::new(50.0, 50.0, 0.05)).unwrap();
    for i in 1..20 {
        let z = i as f64 * 0.37;
        let plate = box_solid(Vec3::new(-50.0, -50.0, z - 0.05), Vec3::new(50.0, 50.0, z + 0.05)).unwrap();
        a = boolean(&a, &plate, BoolOp::Union).unwrap().unwrap();
    }
    let b = crate::transform(&a, Vec3::new(0.01, 0.0, 0.19), Vec3::ZERO, Vec3::X, 0.004).unwrap();
    let t = std::time::Instant::now();
    let r = crate::polybool::planar_boolean(&a, &b, BoolOp::Union);
    let took = t.elapsed();
    // A result, or a refusal in words. Never an answer that does not come.
    assert!(took.as_secs() < 180, "the polygon boolean took {took:?}");
    if let Err(e) = &r {
        assert!(!e.to_string().is_empty(), "an error with no words");
    }
}
