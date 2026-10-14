//! Golden record of what the walk over a feature's expressions reports, for every variant (and
//! every fillet style, pattern, hole and construction definition): `inputs` (label, expression,
//! kind, in order: the order names the `dN` parameters) and `expressions` (every expression,
//! placement points included). Each expression field holds its own name, so a moved, missing or
//! relabelled input shows in the diff. The records live in `feature_walk_tests/*.txt`; to record
//! an intended change, run the tests with `FEATURE_WALK_BLESS=1`.

use serde_json::{Value, json};

use crate::FeatureKind;

fn plane(chain: &str) -> Value {
    json!({"type": "at_angle", "angle": format!("{chain}.angle"), "axis_origin": [0, 0, 0], "axis_dir": [1, 0, 0],
        "base": {"type": "offset", "distance": format!("{chain}.offset"), "base": {"type": "origin", "name": "XY"}}})
}

fn geo_plane(chain: &str) -> Value {
    json!({"type": "plane", "plane": plane(chain)})
}

fn along(chain: &str) -> Value {
    json!({"type": "along_path", "path": {"type": "edge", "at": [0, 0, 0]}, "t": format!("{chain}.t")})
}

fn fixtures() -> Vec<(&'static str, Value)> {
    let o = [0, 0, 0];
    let sk = serde_json::to_value(solidcraft_sketch::Sketch::new()).unwrap();
    let extent = |n: &str| {
        json!({"distance": format!("{n}.distance"), "distance2": format!("{n}.distance2"), "start_offset": format!("{n}.start_offset"),
            "taper": format!("{n}.taper"), "to_offset": format!("{n}.to_offset")})
    };
    let fillet = |style: Value| json!({"type": "fillet", "edges": [o], "radius": "fillet.radius", "style": style});
    let pattern = |p: Value| json!({"type": "pattern", "features": ["F"], "pattern": p});
    let hole =
        |h: Value| json!({"type": "hole", "position": o, "direction": [0, 0, 1], "diameter": "hole.diameter", "depth": "hole.depth", "hole": h});
    vec![
        (
            "body_transfer",
            json!({"type": "body_transfer", "bodies": ["B"], "from": 0, "to": 1, "transform": [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]}),
        ),
        ("sketch", json!({"type": "sketch", "plane": plane("sketch"), "sketch": sk.clone()})),
        ("sketch_along_path", json!({"type": "sketch", "plane": along("sketch"), "sketch": sk})),
        ("extrude", json!({"type": "extrude", "sketch": 1, "profiles": {"type": "all"}, "extent": extent("extrude")})),
        (
            "revolve",
            json!({"type": "revolve", "sketch": 1, "profiles": {"type": "all"}, "axis": {"type": "world", "axis": "X"}, "angle": "revolve.angle", "angle2": "revolve.angle2"}),
        ),
        ("fillet_constant", fillet(json!({"type": "constant"}))),
        ("fillet_chord", fillet(json!({"type": "chord"}))),
        ("fillet_curvature", fillet(json!({"type": "curvature"}))),
        ("fillet_variable", fillet(json!({"type": "variable", "radius2": "fillet.radius2", "start": o, "mid": [[0.5, "fillet.mid"]]}))),
        ("fillet_spline", fillet(json!({"type": "spline", "start": o, "knots": [0.0, 1.0], "radii": ["fillet.r1", "fillet.r2", "fillet.r3"]}))),
        (
            "chamfer",
            json!({"type": "chamfer", "edges": [o], "distance": "chamfer.distance", "distance2": "chamfer.distance2", "angle": "chamfer.angle"}),
        ),
        ("box", json!({"type": "box", "corner": ["box.x", "box.y", "box.z"], "length": "box.length", "width": "box.width", "height": "box.height"})),
        ("cylinder", json!({"type": "cylinder", "base": ["cyl.x", "cyl.y", "cyl.z"], "radius": "cyl.radius", "height": "cyl.height"})),
        ("sphere", json!({"type": "sphere", "center": ["sph.x", "sph.y", "sph.z"], "radius": "sph.radius"})),
        ("torus", json!({"type": "torus", "center": ["tor.x", "tor.y", "tor.z"], "major": "tor.major", "minor": "tor.minor"})),
        ("combine", json!({"type": "combine", "target": "A", "tools": ["B"], "operation": "join"})),
        (
            "pattern_rectangular",
            pattern(
                json!({"type": "rectangular", "dir1": [1, 0, 0], "count1": "rect.count1", "spacing1": "rect.spacing1", "dir2": [0, 1, 0], "count2": "rect.count2", "spacing2": "rect.spacing2"}),
            ),
        ),
        ("pattern_circular", pattern(json!({"type": "circular", "origin": o, "axis": [0, 0, 1], "count": "circ.count", "angle": "circ.angle"}))),
        ("pattern_path", pattern(json!({"type": "path", "path_sketch": 1, "path": ["l1"], "count": "path.count", "spacing": "path.spacing"}))),
        ("mirror", json!({"type": "mirror", "features": ["F"], "plane": plane("mirror")})),
        ("hole_simple", hole(json!({"type": "simple"}))),
        ("hole_drilled", hole(json!({"type": "drilled", "tip_angle": "hole.tip_angle"}))),
        ("hole_counterbore", hole(json!({"type": "counterbore", "cb_diameter": "hole.cb_diameter", "cb_depth": "hole.cb_depth"}))),
        ("hole_countersink", hole(json!({"type": "countersink", "cs_diameter": "hole.cs_diameter", "cs_angle": "hole.cs_angle"}))),
        ("thread", json!({"type": "thread", "face": o, "length": "thread.length"})),
        ("loft", json!({"type": "loft", "sections": []})),
        ("sweep", json!({"type": "sweep", "sketch": 1, "profiles": {"type": "all"}, "path_sketch": 2, "path": ["l1"]})),
        ("shell", json!({"type": "shell", "faces": [o], "thickness": "shell.thickness"})),
        ("draft", json!({"type": "draft", "faces": [o], "angle": "draft.angle", "neutral": plane("draft_neutral"), "pull": [0, 0, 1]})),
        ("construction_plane", json!({"type": "construction_plane", "plane": plane("cplane")})),
        ("split", json!({"type": "split", "body": "B", "plane": plane("split")})),
        (
            "construction_axis",
            json!({"type": "construction_axis", "def": {"type": "two_planes", "a": geo_plane("axis_a"), "b": geo_plane("axis_b")}}),
        ),
        (
            "construction_point_planes",
            json!({"type": "construction_point", "def": {"type": "three_planes", "a": geo_plane("pt_a"), "b": geo_plane("pt_b"), "c": geo_plane("pt_c")}}),
        ),
        (
            "construction_point_along",
            json!({"type": "construction_point", "def": {"type": "along_path", "path": {"type": "edge", "at": o}, "t": "pt.t"}}),
        ),
        ("split_face", json!({"type": "split_face", "faces": [o], "plane": plane("split_face")})),
        ("scale", json!({"type": "scale", "bodies": ["B"], "factor": "scale.factor", "factors": ["scale.x", "scale.y", "scale.z"]})),
        ("offset_face", json!({"type": "offset_face", "faces": [o], "distance": "offset_face.distance"})),
        ("delete_face", json!({"type": "delete_face", "faces": [o]})),
        ("bounding_solid", json!({"type": "bounding_solid", "bodies": ["B"], "margin": "bounding.margin"})),
        ("pipe", json!({"type": "pipe", "path_sketch": 1, "path": ["l1"], "diameter": "pipe.diameter", "wall": "pipe.wall"})),
        ("emboss", json!({"type": "emboss", "sketch": 1, "profiles": {"type": "all"}, "depth": "emboss.depth"})),
        ("rib", json!({"type": "rib", "sketch": 1, "curves": ["l1"], "thickness": "rib.thickness", "depth": "rib.depth"})),
        ("replace_face", json!({"type": "replace_face", "faces": [o], "target": plane("replace_face")})),
        ("align", json!({"type": "align", "bodies": ["B"], "from": o, "to": o})),
        (
            "coil",
            json!({"type": "coil", "base": ["coil.x", "coil.y", "coil.z"], "diameter": "coil.diameter", "pitch": "coil.pitch", "turns": "coil.turns", "section_size": "coil.section_size", "start_angle": "coil.start_angle"}),
        ),
        ("sheet_base", json!({"type": "sheet_base", "sketch": 1, "profiles": {"type": "all"}})),
        ("sheet_contour", json!({"type": "sheet_contour", "sketch": 1, "curves": ["l1"], "distance": "contour.distance"})),
        (
            "sheet_flange",
            json!({"type": "sheet_flange", "edges": [{"point": o, "angle": "flange.e1_angle", "height": "flange.e1_height"}, {"point": o, "angle": null, "height": "flange.e2_height"}], "height": "flange.height", "angle": "flange.angle", "radius": "flange.radius"}),
        ),
        ("sheet_hem", json!({"type": "sheet_hem", "edges": [o], "length": "hem.length", "gap": "hem.gap"})),
        (
            "sheet_fold",
            json!({"type": "sheet_fold", "a": o, "b": [1, 0, 0], "angle": "fold.angle", "radius": "fold.radius", "position": "centerline"}),
        ),
        ("sheet_unfold", json!({"type": "sheet_unfold"})),
        ("sheet_convert", json!({"type": "sheet_convert", "body": "B", "face": o})),
        (
            "boss",
            json!({"type": "boss", "position": o, "diameter": "boss.diameter", "height": "boss.height", "hole_diameter": "boss.hole_diameter", "hole_depth": "boss.hole_depth",
            "draft": "boss.draft", "fillet": "boss.fillet", "ribs": "boss.ribs", "rib_thickness": "boss.rib_thickness", "rib_length": "boss.rib_length", "rib_offset": "boss.rib_offset"}),
        ),
        ("lip", json!({"type": "lip", "face": o, "width": "lip.width", "height": "lip.height", "gap": "lip.gap"})),
        (
            "snap_fit",
            json!({"type": "snap_fit", "position": o, "hook": [1, 0, 0], "length": "snap.length", "thickness": "snap.thickness", "width": "snap.width", "catch_depth": "snap.catch_depth", "catch_length": "snap.catch_length"}),
        ),
        (
            "rest",
            json!({"type": "rest", "position": o, "width": "rest.width", "length": "rest.length", "height": "rest.height", "draft": "rest.draft", "thickness": "rest.thickness"}),
        ),
        ("boundary_fill", json!({"type": "boundary_fill", "tools": ["B"], "cells": [o]})),
        ("remove", json!({"type": "remove", "bodies": ["B"]})),
        ("patch", json!({"type": "patch"})),
        ("stitch", json!({"type": "stitch", "bodies": ["B"], "tolerance": "stitch.tolerance"})),
        ("thicken", json!({"type": "thicken", "bodies": ["B"], "thickness": "thicken.thickness", "operation": "new_body"})),
        ("surface_extrude", json!({"type": "surface_extrude", "sketch": 1, "extent": extent("surface_extrude")})),
        (
            "surface_revolve",
            json!({"type": "surface_revolve", "sketch": 1, "axis": {"type": "world", "axis": "X"}, "angle": "srev.angle", "angle2": "srev.angle2"}),
        ),
        ("surface_sweep", json!({"type": "surface_sweep", "sketch": 1, "path_sketch": 2, "path": ["l1"]})),
        ("surface_loft", json!({"type": "surface_loft", "sections": []})),
        ("surface_offset", json!({"type": "surface_offset", "faces": [o], "distance": "soff.distance", "tolerance": "soff.tolerance"})),
        ("unstitch", json!({"type": "unstitch", "body": "B"})),
        ("reverse_normal", json!({"type": "reverse_normal", "body": "B"})),
        ("surface_extend", json!({"type": "surface_extend", "body": "B", "edges": [o], "distance": "extend.distance"})),
        ("surface_trim", json!({"type": "surface_trim", "body": "B", "plane": plane("trim"), "keep": o})),
        ("move", json!({"type": "move", "bodies": ["B"], "translate": ["move.x", "move.y", "move.z"], "angle": "move.angle"})),
        ("import", json!({"type": "import", "file": "a.step", "step": ""})),
        ("mesh_convert", json!({"type": "mesh_convert", "body": "B", "tolerance": "mesh_convert.tolerance"})),
        ("mesh_import", json!({"type": "mesh_import", "file": "a.stl", "meshes": []})),
    ]
}

/// Per fixture: its inputs, one per line (`label = expression (kind)`), then every expression.
fn record() -> String {
    let mut out = String::new();
    for (name, v) in fixtures() {
        let kind: FeatureKind = serde_json::from_value(v).unwrap_or_else(|e| panic!("{name}: {e}"));
        out += &format!("{name}\n");
        let inputs = kind.inputs();
        if !inputs.is_empty() {
            out += "  inputs:\n";
            for (label, e, k) in inputs {
                out += &format!("    {label} = {e} ({k:?})\n");
            }
        }
        let expressions = kind.expressions();
        if !expressions.is_empty() {
            out += &format!("  expressions: {}\n", expressions.join(", "));
        }
    }
    out
}

/// Compares `got` with the golden file `name` in `feature_walk_tests/`, line by line. With
/// `FEATURE_WALK_BLESS` set it rewrites the file instead, for an intended change (say why in the
/// pull request).
fn check_golden(name: &str, want: &str, got: &str) {
    if std::env::var_os("FEATURE_WALK_BLESS").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/feature_walk_tests").join(name);
        std::fs::write(&path, got).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        return;
    }
    for (line, (want, got)) in want.lines().zip(got.lines()).enumerate() {
        assert_eq!(want, got, "{name} line {}; rerun with FEATURE_WALK_BLESS=1 if the change is intended", line + 1);
    }
    assert_eq!(want.lines().count(), got.lines().count(), "{name}: the record has a different number of lines");
}

/// Recorded from the hand-written `inputs_mut` and `expressions` the walk replaced.
const GOLDEN_EXPRS: &str = include_str!("feature_walk_tests/golden_exprs.txt");

#[test]
fn inputs_and_expressions_match_the_golden_record() {
    check_golden("golden_exprs.txt", GOLDEN_EXPRS, &record());
}

#[test]
fn the_record_covers_every_variant() {
    let src = include_str!("document.rs");
    let body = src.split("pub enum FeatureKind {").nth(1).and_then(|s| s.split("\n}\n").next()).unwrap();
    let names: Vec<String> = fixtures().into_iter().map(|(_, v)| v["type"].as_str().unwrap().replace('_', "")).collect();
    for line in body.lines().filter(|l| l.starts_with("    ") && !l.starts_with("     ")) {
        let t = line.trim();
        if t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            let variant = t.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_lowercase();
            assert!(names.contains(&variant), "FeatureKind::{t} has no fixture in feature_walk_tests");
        }
    }
}
