//! Issue #2844: a ring joined onto a plate with a grid of square holes kept its walls split
//! into full-height strips, one seam wherever its foot crossed a hole edge.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use solidcraft_geom::{Loop2, Plane, Region2, Vec2};

use crate::{Body, BoolOp, boolean, extrude, measure};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region2 {
    Region2 { outer: Loop2::polygon(&[Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]), holes: vec![] }
}

/// The 80 x 80 x 1.2 plate, with the 9 x 9 grid of 6.4 mm holes when `holes`.
fn plate(holes: bool) -> Body {
    let plate = extrude(&Plane::XY, &[rect(0.0, 0.0, 80.0, 80.0)], 0.0, 1.2).unwrap().remove(0);
    if !holes {
        return plate;
    }
    let grid: Vec<Region2> =
        (0..81).map(|k| (4.8 + 8.0 * (k % 9) as f64, 4.8 + 8.0 * (k / 9) as f64)).map(|(x, y)| rect(x, y, x + 6.4, y + 6.4)).collect();
    let tools = extrude(&Plane::XY, &grid, -0.1, 1.2).unwrap();
    tools.iter().fold(plate, |p, t| boolean(&p, t, BoolOp::Cut).unwrap().unwrap())
}

/// The ring (r 19.6 / 20.8 about (41.3, 40.7), off the hole grid) from `z0` to 21.2.
fn ring(z0: f64) -> Body {
    let c = Vec2::new(41.3, 40.7);
    let ring = Region2 { outer: Loop2::circle_from(c, 20.8, 0.0), holes: vec![Loop2::circle_from(c, 19.6, 0.0).reversed()] };
    extrude(&Plane::XY, &[ring], z0, 21.2).unwrap().remove(0)
}

fn volume(b: &Body) -> f64 {
    measure(b).unwrap().volume
}

/// Sunk into the plate, standing on it, and on a plain plate (the control): above the plate each
/// wall is one cylinder (its two halves) under a rim of two half circles, and the body holds the
/// material the operands make. Sunk, the wall's foot in each hole it crosses stays a face of its
/// own: pieces stacked up the axis don't merge.
#[test]
fn a_ring_on_a_perforated_plate_keeps_whole_walls() {
    let (plain, holed) = (plate(false), plate(true));
    for (base, z0, cylinders) in [(&plain, 0.6, 2), (&holed, 0.6, 12), (&holed, 1.2, 2)] {
        let ring = ring(z0);
        let joined = boolean(base, &ring, BoolOp::Union).unwrap().unwrap();
        let m = measure(&joined).unwrap();
        assert_eq!(m.merged.face_types.get("cylinder"), Some(&cylinders), "ring from z {z0}: {:?}", m.merged.face_types);
        let walls = joined
            .solid
            .face_iter()
            .filter(|f| !matches!(f.surface(), truck_modeling::Surface::Plane(_)))
            .filter(|f| f.edge_iter().flat_map(|e| crate::heal::edge_points(&e)).any(|p| (p.z - 21.2).abs() < 1e-9))
            .count();
        assert_eq!(walls, 4, "ring from z {z0}");
        // The top rims too: a half circle per edge, not a piece per strip.
        let rim: std::collections::HashSet<_> = joined
            .solid
            .edge_iter()
            .filter(|e| !matches!(e.curve(), truck_modeling::Curve::Line(_)))
            .filter(|e| crate::heal::edge_points(e).iter().all(|p| (p.z - 21.2).abs() < 1e-9))
            .map(|e| e.id())
            .collect();
        assert_eq!(rim.len(), 4, "ring from z {z0}");
        assert!(joined.validity().is_empty(), "{:?}", joined.validity());
        let common = boolean(base, &ring, BoolOp::Intersect).unwrap().map_or(0.0, |b| volume(&b));
        let want = volume(base) + volume(&ring) - common;
        assert!((m.volume - want).abs() < want * 1e-4, "ring from z {z0}: {} vs {want}", m.volume);
    }
}
