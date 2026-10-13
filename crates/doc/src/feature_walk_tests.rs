//! Golden record of what the walk over a feature's expressions reports, for every variant (and
//! every fillet style, pattern, hole and construction definition): `inputs` (label, expression,
//! kind, in order: the order names the `dN` parameters) and `expressions` (every expression,
//! placement points included). Each expression field holds its own name, so a moved, missing or
//! relabelled input shows in the diff.

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

fn record() -> String {
    let mut out = String::new();
    for (name, v) in fixtures() {
        let kind: FeatureKind = serde_json::from_value(v).unwrap_or_else(|e| panic!("{name}: {e}"));
        let inputs: Vec<String> = kind.inputs().into_iter().map(|(l, e, k)| format!("{l}={e}:{k:?}")).collect();
        out += &format!("{name}\n  inputs: {}\n  expressions: {}\n", inputs.join(", "), kind.expressions().join(", "));
    }
    out
}

/// Recorded from the hand-written `inputs_mut` and `expressions` the walk replaced.
const GOLDEN: &str = "\
body_transfer
  inputs: 
  expressions: 
sketch
  inputs: Angle=sketch.angle:Angle, Offset=sketch.offset:Length
  expressions: sketch.angle, sketch.offset
sketch_along_path
  inputs: Distance (0 to 1)=sketch.t:Unitless
  expressions: sketch.t
extrude
  inputs: Distance=extrude.distance:Length, Distance 2=extrude.distance2:Length, Start offset=extrude.start_offset:Length, Taper angle=extrude.taper:Angle, To offset=extrude.to_offset:Length
  expressions: extrude.distance, extrude.distance2, extrude.start_offset, extrude.taper, extrude.to_offset
revolve
  inputs: Angle=revolve.angle:Angle, Angle 2=revolve.angle2:Angle
  expressions: revolve.angle, revolve.angle2
fillet_constant
  inputs: Radius=fillet.radius:Length
  expressions: fillet.radius
fillet_chord
  inputs: Chord length=fillet.radius:Length
  expressions: fillet.radius
fillet_variable
  inputs: Start radius=fillet.radius:Length, End radius=fillet.radius2:Length, Mid radius=fillet.mid:Length
  expressions: fillet.radius, fillet.radius2, fillet.mid
fillet_spline
  inputs: Start radius=fillet.radius:Length, Control radius=fillet.r1:Length, Control radius=fillet.r2:Length, End radius=fillet.r3:Length
  expressions: fillet.radius, fillet.r1, fillet.r2, fillet.r3
chamfer
  inputs: Distance=chamfer.distance:Length, Distance 2=chamfer.distance2:Length, Angle=chamfer.angle:Angle
  expressions: chamfer.distance, chamfer.distance2, chamfer.angle
box
  inputs: Length=box.length:Length, Width=box.width:Length, Height=box.height:Length
  expressions: box.length, box.width, box.height, box.x, box.y, box.z
cylinder
  inputs: Radius=cyl.radius:Length, Height=cyl.height:Length
  expressions: cyl.radius, cyl.height, cyl.x, cyl.y, cyl.z
sphere
  inputs: Radius=sph.radius:Length
  expressions: sph.radius, sph.x, sph.y, sph.z
torus
  inputs: Major radius=tor.major:Length, Minor radius=tor.minor:Length
  expressions: tor.major, tor.minor, tor.x, tor.y, tor.z
combine
  inputs: 
  expressions: 
pattern_rectangular
  inputs: Quantity=rect.count1:Unitless, Spacing=rect.spacing1:Length, Quantity 2=rect.count2:Unitless, Spacing 2=rect.spacing2:Length
  expressions: rect.count1, rect.spacing1, rect.count2, rect.spacing2
pattern_circular
  inputs: Quantity=circ.count:Unitless, Total angle=circ.angle:Angle
  expressions: circ.count, circ.angle
pattern_path
  inputs: Quantity=path.count:Unitless, Distance=path.spacing:Length
  expressions: path.count, path.spacing
mirror
  inputs: Angle=mirror.angle:Angle, Offset=mirror.offset:Length
  expressions: mirror.angle, mirror.offset
hole_simple
  inputs: Diameter=hole.diameter:Length, Depth=hole.depth:Length
  expressions: hole.diameter, hole.depth
hole_drilled
  inputs: Diameter=hole.diameter:Length, Depth=hole.depth:Length, Tip angle=hole.tip_angle:Angle
  expressions: hole.diameter, hole.depth, hole.tip_angle
hole_counterbore
  inputs: Diameter=hole.diameter:Length, Depth=hole.depth:Length, Counterbore diameter=hole.cb_diameter:Length, Counterbore depth=hole.cb_depth:Length
  expressions: hole.diameter, hole.depth, hole.cb_diameter, hole.cb_depth
hole_countersink
  inputs: Diameter=hole.diameter:Length, Depth=hole.depth:Length, Countersink diameter=hole.cs_diameter:Length, Countersink angle=hole.cs_angle:Angle
  expressions: hole.diameter, hole.depth, hole.cs_diameter, hole.cs_angle
thread
  inputs: Length=thread.length:Length
  expressions: thread.length
loft
  inputs: 
  expressions: 
sweep
  inputs: 
  expressions: 
shell
  inputs: Thickness=shell.thickness:Length
  expressions: shell.thickness
draft
  inputs: Angle=draft.angle:Angle, Angle=draft_neutral.angle:Angle, Offset=draft_neutral.offset:Length
  expressions: draft.angle, draft_neutral.angle, draft_neutral.offset
construction_plane
  inputs: Angle=cplane.angle:Angle, Offset=cplane.offset:Length
  expressions: cplane.angle, cplane.offset
split
  inputs: Angle=split.angle:Angle, Offset=split.offset:Length
  expressions: split.angle, split.offset
construction_axis
  inputs: Angle=axis_a.angle:Angle, Offset=axis_a.offset:Length, Angle=axis_b.angle:Angle, Offset=axis_b.offset:Length
  expressions: axis_a.angle, axis_a.offset, axis_b.angle, axis_b.offset
construction_point_planes
  inputs: Angle=pt_a.angle:Angle, Offset=pt_a.offset:Length, Angle=pt_b.angle:Angle, Offset=pt_b.offset:Length, Angle=pt_c.angle:Angle, Offset=pt_c.offset:Length
  expressions: pt_a.angle, pt_a.offset, pt_b.angle, pt_b.offset, pt_c.angle, pt_c.offset
construction_point_along
  inputs: Distance (0 to 1)=pt.t:Unitless
  expressions: pt.t
split_face
  inputs: Angle=split_face.angle:Angle, Offset=split_face.offset:Length
  expressions: split_face.angle, split_face.offset
scale
  inputs: Scale factor=scale.factor:Unitless, X scale=scale.x:Unitless, Y scale=scale.y:Unitless, Z scale=scale.z:Unitless
  expressions: scale.factor, scale.x, scale.y, scale.z
offset_face
  inputs: Distance=offset_face.distance:Length
  expressions: offset_face.distance
delete_face
  inputs: 
  expressions: 
bounding_solid
  inputs: Margin=bounding.margin:Length
  expressions: bounding.margin
pipe
  inputs: Diameter=pipe.diameter:Length, Wall=pipe.wall:Length
  expressions: pipe.diameter, pipe.wall
emboss
  inputs: Depth=emboss.depth:Length
  expressions: emboss.depth
rib
  inputs: Thickness=rib.thickness:Length, Depth=rib.depth:Length
  expressions: rib.thickness, rib.depth
replace_face
  inputs: Angle=replace_face.angle:Angle, Offset=replace_face.offset:Length
  expressions: replace_face.angle, replace_face.offset
align
  inputs: 
  expressions: 
coil
  inputs: Diameter=coil.diameter:Length, Pitch=coil.pitch:Length, Revolutions=coil.turns:Unitless, Section size=coil.section_size:Length, Start angle=coil.start_angle:Angle
  expressions: coil.diameter, coil.pitch, coil.turns, coil.section_size, coil.start_angle, coil.x, coil.y, coil.z
sheet_base
  inputs: 
  expressions: 
sheet_contour
  inputs: Distance=contour.distance:Length
  expressions: contour.distance
sheet_flange
  inputs: Height=flange.height:Length, Angle=flange.angle:Angle, Bend radius=flange.radius:Length, Edge angle=flange.e1_angle:Angle, Edge height=flange.e1_height:Length, Edge height=flange.e2_height:Length
  expressions: flange.height, flange.angle, flange.radius, flange.e1_angle, flange.e1_height, flange.e2_height
sheet_hem
  inputs: Length=hem.length:Length, Gap=hem.gap:Length
  expressions: hem.length, hem.gap
sheet_fold
  inputs: Angle=fold.angle:Angle, Bend radius=fold.radius:Length
  expressions: fold.angle, fold.radius
sheet_unfold
  inputs: 
  expressions: 
sheet_convert
  inputs: 
  expressions: 
boss
  inputs: Diameter=boss.diameter:Length, Height=boss.height:Length, Hole diameter=boss.hole_diameter:Length, Hole depth=boss.hole_depth:Length, Draft angle=boss.draft:Angle, Root fillet=boss.fillet:Length, Ribs=boss.ribs:Unitless, Rib thickness=boss.rib_thickness:Length, Rib length=boss.rib_length:Length, Rib offset=boss.rib_offset:Length
  expressions: boss.diameter, boss.height, boss.hole_diameter, boss.hole_depth, boss.draft, boss.fillet, boss.ribs, boss.rib_thickness, boss.rib_length, boss.rib_offset
lip
  inputs: Width=lip.width:Length, Height=lip.height:Length, Gap=lip.gap:Length
  expressions: lip.width, lip.height, lip.gap
snap_fit
  inputs: Length=snap.length:Length, Thickness=snap.thickness:Length, Width=snap.width:Length, Catch depth=snap.catch_depth:Length, Catch length=snap.catch_length:Length
  expressions: snap.length, snap.thickness, snap.width, snap.catch_depth, snap.catch_length
rest
  inputs: Width=rest.width:Length, Height=rest.height:Length, Length=rest.length:Length, Draft angle=rest.draft:Angle, Wall thickness=rest.thickness:Length
  expressions: rest.width, rest.height, rest.length, rest.draft, rest.thickness
boundary_fill
  inputs: 
  expressions: 
remove
  inputs: 
  expressions: 
patch
  inputs: 
  expressions: 
stitch
  inputs: Tolerance=stitch.tolerance:Length
  expressions: stitch.tolerance
thicken
  inputs: Thickness=thicken.thickness:Length
  expressions: thicken.thickness
surface_extrude
  inputs: Distance=surface_extrude.distance:Length, Distance 2=surface_extrude.distance2:Length, Start offset=surface_extrude.start_offset:Length, Taper angle=surface_extrude.taper:Angle, To offset=surface_extrude.to_offset:Length
  expressions: surface_extrude.distance, surface_extrude.distance2, surface_extrude.start_offset, surface_extrude.taper, surface_extrude.to_offset
surface_revolve
  inputs: Angle=srev.angle:Angle, Angle 2=srev.angle2:Angle
  expressions: srev.angle, srev.angle2
surface_sweep
  inputs: 
  expressions: 
surface_loft
  inputs: 
  expressions: 
surface_offset
  inputs: Distance=soff.distance:Length, Tolerance=soff.tolerance:Length
  expressions: soff.distance, soff.tolerance
unstitch
  inputs: 
  expressions: 
reverse_normal
  inputs: 
  expressions: 
surface_extend
  inputs: Distance=extend.distance:Length
  expressions: extend.distance
surface_trim
  inputs: Angle=trim.angle:Angle, Offset=trim.offset:Length
  expressions: trim.angle, trim.offset
move
  inputs: X distance=move.x:Length, Y distance=move.y:Length, Z distance=move.z:Length, Angle=move.angle:Angle
  expressions: move.x, move.y, move.z, move.angle
import
  inputs: 
  expressions: 
mesh_convert
  inputs: Tolerance=mesh_convert.tolerance:Length
  expressions: mesh_convert.tolerance
mesh_import
  inputs: 
  expressions: 
";

#[test]
fn inputs_and_expressions_match_the_golden_record() {
    let got = record();
    for (line, (want, have)) in GOLDEN.lines().zip(got.lines()).enumerate() {
        assert_eq!(want, have, "line {}", line + 1);
    }
    assert_eq!(GOLDEN.lines().count(), got.lines().count(), "{got}");
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
