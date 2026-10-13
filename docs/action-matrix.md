# Action matrix

The action matrix tries every ordinary action on every face, edge, vertex and body of the final
bodies of a corpus of real designs, through the same commands and parameters the desktop sends,
and records what works. [`tests/action_matrix/baseline.json`](../tests/action_matrix/baseline.json)
is a ratchet: an action that passed there and fails now is a regression and fails the test.
The test is [`apps/solidcraft-cli/tests/action_matrix.rs`](../apps/solidcraft-cli/tests/action_matrix.rs).

## Running it

- **Every CI run** (pull requests too) checks a fixed sample, one action in 40 by a stable hash of
  its name, on every design except the seven slowest to build (`action_matrix_sample`, under a
  minute).
- **Nightly** (and by hand, Run workflow), the "Action matrix" job runs the whole matrix in four
  parts (`action_matrix_full_1` to `_4`).
- **Locally**: `cargo xtask action-matrix` runs it whole (about 22 minutes with six designs at a time on a 16-core machine). Add `--bless` to
  rewrite the baseline after a fix (newly passing actions are listed when you don't), and
  `--results FILE` to keep every outcome with its message and the command that was sent.
  `ACTION_MATRIX_FILTER=oracle/23` runs only the designs whose name contains it;
  `ACTION_MATRIX_JOBS` sets how many designs run at once.

Each design runs in its own process, so an action that hangs (90 s without a result) or aborts is
recorded as `timeout` or `crash` and the rest of the design still runs. None did in this run.

Pieces that share a name (`#2`, `~3`) are numbered by where they lie, and the numbers of pieces
lying almost level can swap between runs (#569). The ratchet therefore counts passes per family of
pieces (the name without its numbers) and samples whole families, so a swap is not a regression.

Each open cause below has a small reproduction among the ignored tests at the end of
`action_matrix.rs` (`cargo test --profile ci -p solidcraft-cli --test action_matrix -- --ignored`
lists which still fail; skip `action_matrix_` to leave out the matrix itself).

## The corpus

- `corpus/*`: the old design files of [`apps/solidcraft-cli/tests/corpus`](../apps/solidcraft-cli/tests/corpus/).
- `examples/*`, `regressions/*`: the scripts in [`examples/`](../examples/).
- `oracle/*`: the 76 reference recipes, rebuilt (`solidcraft-cli recipe`).
- `demo/enclosure_steps`: the electronics enclosure built over MCP;
  `demo/pi-case`: the Raspberry Pi 4 case of the 0.6 README demo, built through the desktop UI
  and saved by `examples/demo/enclosure_ui_mcp.py --story pi --save`.
- `typical/*`: [`tests/action_matrix/designs`](../tests/action_matrix/designs/), parts made the
  ordinary way: a stacked extrude, a filleted and shelled box, a revolved knob, a plate with a
  counterbored hole patterned 3 × 2 and a chamfered outline, a sheet metal L bracket with holes, a
  join and a cut of three bodies, a loft on a plate, and a sweep.

## The actions

Each one runs on a scratch copy of the design. It passes when the command works, the timeline
has no new errors, and any sketch it makes is attached to its face (healthy, not a legacy face
sketch found again by a point). A face pick is what the desktop sends: the world body, the face
index, the occurrence path and a point inside the face; never `fixed`.

| Entity | Action | Command and parameters |
|---|---|---|
| planar face | `face.sketch` | `sketch.create` on the face |
| | `face.offset_plane` | `construct.plane.offset`, 10% of the body's size |
| | `face.angle_plane` | `construct.plane.perpendicular` through the face's longest straight edge |
| | `face.midplane` | `construct.plane.midplane` with the nearest opposite parallel face |
| | `face.press_pull`, `face.extrude` | `solid.press_pull` / `solid.extrude` (`face`, join), 5% of the size (0.5 to 5 mm) |
| | `face.hole` | `solid.hole` well inside the face (0.8 × the way to its nearest edge, at most 6 mm), through all |
| | `face.draft_neutral` | `solid.draft` of the walls square to the face, 2°, the face's plane neutral |
| | `face.shell` | `solid.shell` removing the face, 1% of the size (0.1 to 0.5 mm) |
| cylinder, cone, torus, sphere | `curved.axis` | `construct.axis.through_cylinder` (not spheres) |
| | `curved.tangent_plane` | `construct.plane.tangent` |
| edge | `edge.fillet`, `edge.chamfer` | the edge alone, 10% of the shortest edge at its ends (not tangent edges, where there is nothing to blend) |
| | `edge.axis` | `construct.axis.edge` (straight edges) |
| | `edge.normal_plane` | `construct.plane.along_path`, halfway |
| | `edge.project` | a sketch on the origin plane it shows longest in, `sketch.project` |
| vertex | `vertex.point` | `construct.point.vertex` |
| | `vertex.plane_three_points` | `construct.plane.three_points` with two neighbours along its edges |
| body | `body.move`, `body.mirror`, `body.pattern` | `solid.move` 10 mm, `solid.mirror` about YZ, `solid.pattern.rectangular` 2 × |

Sketching on a cylinder's cap is covered by `face.sketch` on the planar cap. Bodies are capped at
120 faces, 240 edges and 120 vertices each. Entities are named by their persistent names
(`F2:top`, edges `face|face`, vertices by the faces around them).

**The edit round**: one pass in six also changes a driving parameter (the first user parameter,
else a feature's, by 10%, one that the design itself survives) and checks the new feature still
resolves; it is recorded as `<action>+edit`.

## Results

**30185 of 35199 actions pass (85.8%)** on 103 designs; the edit round passes 1363 of 1382.

### By category

| Category | Actions | Pass | Rate |
|---|---:|---:|---:|
| body | 348 | 347 | 99.7% |
| curved | 1848 | 1815 | 98.2% |
| edge | 18645 | 17351 | 93.1% |
| face | 9221 | 5571 | 60.4% |
| vertex | 5137 | 5101 | 99.3% |

### By action

| Action | Actions | Pass | Rate | Edit round |
|---|---:|---:|---:|---:|
| `body.mirror` | 116 | 116 | 100.0% | 1 / 1 |
| `body.move` | 116 | 116 | 100.0% | 3 / 3 |
| `body.pattern` | 116 | 115 | 99.1% | 8 / 8 |
| `curved.axis` | 906 | 885 | 97.7% | 37 / 39 |
| `curved.tangent_plane` | 942 | 930 | 98.7% | 44 / 44 |
| `edge.axis` | 2875 | 2824 | 98.2% | 139 / 139 |
| `edge.chamfer` | 3441 | 2839 | 82.5% | 121 / 122 |
| `edge.fillet` | 3441 | 2872 | 83.5% | 126 / 126 |
| `edge.normal_plane` | 4444 | 4389 | 98.8% | 189 / 191 |
| `edge.project` | 4444 | 4427 | 99.6% | 224 / 224 |
| `face.angle_plane` | 992 | 968 | 97.6% | 52 / 52 |
| `face.draft_neutral` | 883 | 234 | 26.5% | 8 / 8 |
| `face.extrude` | 1111 | 454 | 40.9% | 25 / 26 |
| `face.hole` | 1111 | 706 | 63.5% | 40 / 43 |
| `face.midplane` | 680 | 668 | 98.2% | 24 / 26 |
| `face.offset_plane` | 1111 | 574 | 51.7% | 26 / 26 |
| `face.press_pull` | 1111 | 454 | 40.9% | 25 / 26 |
| `face.shell` | 1111 | 941 | 84.7% | 45 / 47 |
| `face.sketch` | 1111 | 572 | 51.5% | 27 / 27 |
| `vertex.plane_three_points` | 2031 | 1995 | 98.2% | 79 / 84 |
| `vertex.point` | 3106 | 3106 | 100.0% | 120 / 120 |

### By design

| Design | Actions | Pass | Rate |
|---|---:|---:|---:|
| corpus/components-arm | 589 | 350 | 59.4% |
| corpus/issue-489-stacked-join | 242 | 226 | 93.4% |
| corpus/joints-hinge | 340 | 254 | 74.7% |
| corpus/m0-box-fillet-hole | 169 | 157 | 92.9% |
| corpus/m0-revolve-primitives | 342 | 298 | 87.1% |
| corpus/modern-path-pattern | 298 | 292 | 98.0% |
| corpus/modern-plastic | 829 | 632 | 76.2% |
| corpus/modern-sheet-metal | 393 | 272 | 69.2% |
| corpus/naming-split-pieces | 284 | 223 | 78.5% |
| corpus/split-body-pieces | 302 | 278 | 92.1% |
| demo/enclosure_steps | 1069 | 989 | 92.5% |
| demo/pi-case | 1779 | 1498 | 84.2% |
| examples/bracket | 201 | 190 | 94.5% |
| examples/sensor_enclosure | 2018 | 1855 | 91.9% |
| oracle/01-box | 130 | 130 | 100.0% |
| oracle/02-cylinder | 40 | 40 | 100.0% |
| oracle/03-box-through-hole | 154 | 150 | 97.4% |
| oracle/04-revolve-vase | 100 | 78 | 78.0% |
| oracle/05-taper-extrude | 94 | 86 | 91.5% |
| oracle/06-symmetric-extrude | 130 | 130 | 100.0% |
| oracle/07-fillet-all-edges | 248 | 248 | 100.0% |
| oracle/08-chamfer | 205 | 187 | 91.2% |
| oracle/09-shell | 273 | 264 | 96.7% |
| oracle/10-rect-pattern-holes | 418 | 414 | 99.0% |
| oracle/11-circular-pattern-flange | 208 | 200 | 96.2% |
| oracle/12-mirror-feature | 192 | 180 | 93.8% |
| oracle/13-combine-join | 230 | 201 | 87.4% |
| oracle/14-combine-cut | 159 | 145 | 91.2% |
| oracle/15-combine-intersect | 107 | 77 | 72.0% |
| oracle/16-sweep | 63 | 55 | 87.3% |
| oracle/17-loft | 89 | 63 | 70.8% |
| oracle/18-hole-features | 285 | 261 | 91.6% |
| oracle/19-l-bracket-constrained | 186 | 186 | 100.0% |
| oracle/20-slot-constrained | 105 | 99 | 94.3% |
| oracle/21-draft | 122 | 98 | 80.3% |
| oracle/22-split-body | 78 | 54 | 69.2% |
| oracle/23-bracket-multifeature | 449 | 379 | 84.4% |
| oracle/24-user-parameters | 154 | 150 | 97.4% |
| oracle/25-sketch-triangle | 101 | 99 | 98.0% |
| oracle/26-sketch-belt | 128 | 122 | 95.3% |
| oracle/27-sketch-trapezoid | 177 | 171 | 96.6% |
| oracle/28-blind-pocket-fillets | 360 | 329 | 91.4% |
| oracle/29-two-sided-and-partial-revolve | 313 | 255 | 81.5% |
| oracle/30-fillet-after-shell | 420 | 405 | 96.4% |
| oracle/31-fillet-l-all-edges | 374 | 370 | 98.9% |
| oracle/32-boss-plate-fillets | 173 | 166 | 96.0% |
| oracle/33-fillet-tangent-chain | 183 | 165 | 90.2% |
| oracle/34-chamfer-then-fillet | 311 | 254 | 81.7% |
| oracle/35-fillet-curved-curved | 72 | 69 | 95.8% |
| oracle/36-corner-blend-two-radii | 241 | 235 | 97.5% |
| oracle/37-pyramid-apex-blend | 168 | 143 | 85.1% |
| oracle/38-coincident-cylinders | 40 | 40 | 100.0% |
| oracle/39-tee-pipe | 118 | 117 | 99.2% |
| oracle/40-box-sphere-torus | 225 | 189 | 84.0% |
| oracle/41-tube-radial-holes | 496 | 494 | 99.6% |
| oracle/42-overlapping-patterns | 1092 | 924 | 84.6% |
| oracle/43-cut-splits-body | 260 | 252 | 96.9% |
| oracle/44-shell-l-bracket | 395 | 389 | 98.5% |
| oracle/45-shell-boss-plate | 334 | 321 | 96.1% |
| oracle/46-shell-two-open-faces | 277 | 268 | 96.8% |
| oracle/47-draft-curved-pocket | 220 | 178 | 80.9% |
| oracle/48-pulley | 297 | 273 | 91.9% |
| oracle/49-helical-sweep | 35 | 17 | 48.6% |
| oracle/50-sweep-tight-bend | 76 | 60 | 78.9% |
| oracle/51-loft-rect-circle-offset | 89 | 63 | 70.8% |
| oracle/52-circular-pattern-fillet | 266 | 225 | 84.6% |
| oracle/53-path-pattern | 298 | 292 | 98.0% |
| oracle/54-mirror-body-fillets | 238 | 208 | 87.4% |
| oracle/55-tapped-hole-cosmetic | 178 | 166 | 93.3% |
| oracle/56-holes-on-curved-face | 142 | 122 | 85.9% |
| oracle/57-hole-pattern-sketch-points | 484 | 453 | 93.6% |
| oracle/58-phone-stand | 634 | 551 | 86.9% |
| oracle/59-enclosure-base | 1028 | 941 | 91.5% |
| oracle/60-pipe-flange | 377 | 355 | 94.2% |
| oracle/61-shaft-keyway | 309 | 230 | 74.4% |
| oracle/62-heat-sink | 1780 | 1523 | 85.6% |
| oracle/63-hinge-knuckle | 431 | 351 | 81.4% |
| oracle/64-sketch-tangent-arc-chain | 172 | 141 | 82.0% |
| oracle/65-sketch-gear-tooth | 169 | 143 | 84.6% |
| oracle/66-sketch-linkage | 200 | 195 | 97.5% |
| oracle/67-sm-base-flange | 130 | 104 | 80.0% |
| oracle/68-sm-edge-flange | 262 | 186 | 71.0% |
| oracle/69-sm-tray | 918 | 598 | 65.1% |
| oracle/70-sm-flange-45 | 260 | 184 | 70.8% |
| oracle/71-sm-custom-rule | 393 | 277 | 70.5% |
| oracle/72-sm-contour-flange | 393 | 272 | 69.2% |
| oracle/73-sm-hem | 396 | 273 | 68.9% |
| oracle/74-sm-unfold-refold | 618 | 438 | 70.9% |
| oracle/75-sm-bracket-holes | 478 | 322 | 67.4% |
| oracle/76-sm-thickness-variant | 262 | 186 | 71.0% |
| regressions/join_struts_on_bore_wall | 543 | 423 | 77.9% |
| regressions/loft_cut_beside_round_hole | 385 | 304 | 79.0% |
| regressions/oblique_cut_of_cylinder | 304 | 268 | 88.2% |
| regressions/oblique_cut_of_cylinder_hang | 304 | 269 | 88.5% |
| regressions/shallow_loft_cut_from_top_face | 306 | 263 | 85.9% |
| typical/filleted-shelled-box | 401 | 385 | 96.0% |
| typical/holes-pattern-chamfer | 535 | 479 | 89.5% |
| typical/loft | 217 | 179 | 82.5% |
| typical/multi-body-join-cut | 308 | 283 | 91.9% |
| typical/revolved-part | 160 | 152 | 95.0% |
| typical/sheet-metal-bracket | 310 | 229 | 73.9% |
| typical/stacked-extrude | 282 | 274 | 97.2% |
| typical/sweep | 279 | 184 | 65.9% |

## Causes

Every failure, grouped by cause (5033 in all). Faces that can't take an attached sketch fail four
actions each (`face.sketch`, `face.offset_plane`, and `face.press_pull` and `face.extrude`, which
make a legacy face sketch there).

| Failures | Issue | Cause | Actions | Designs | Example |
|---:|---|---|---|---:|---|
| 982 | #556 | Fillet or chamfer of one edge whose ends meet curved, slanted or several faces | `edge.chamfer` 494, `edge.fillet` 488 | 46 | corpus/components-arm `ArmBody (Arm:2)/F4:+x\|F4:+y` |
| 680 | #495 | Sheet metal thickness (rim) faces have no attachment | `face.extrude` 200, `face.press_pull` 200, `face.offset_plane` 140, `face.sketch` 140 | 12 | corpus/modern-sheet-metal `Body1/F2:face:0` |
| 649 | #390 | Draft refuses walls whose neighbours aren't square to the pull (rounds, other walls), or corners that change after the draft | `face.draft_neutral` 649 | 65 | corpus/components-arm `ArmBody (Arm:2)/F4:+x` |
| 384 | #494 | Revolve, Offset Face, Boss and the other plastic features drop the body's face sources | `face.offset_plane` 100, `face.sketch` 100, `face.extrude` 92, `face.press_pull` 92 | 11 | corpus/m0-revolve-primitives `Body1/F2:end` |
| 380 | #493 | Cut walls, hole faces and chamfer faces have no source | `face.extrude` 95, `face.offset_plane` 95, `face.press_pull` 95, `face.sketch` 95 | 17 | corpus/split-body-pieces `Body1 (1)/F4:side:l1` |
| 336 | #557 | Pattern and mirror copies share or lose their faces' sources | `face.extrude` 84, `face.offset_plane` 84, `face.press_pull` 84, `face.sketch` 84 | 6 | demo/pi-case `Body1/F6:top` |
| 285 | #558 | Picks on a moved or pasted component miss (construction geometry, Hole, Press Pull, Project) | `edge.normal_plane` 55, `edge.axis` 51, `vertex.plane_three_points` 36, `face.extrude` 34 | 2 | corpus/components-arm `ArmBody (Arm:2)/F4:+x` |
| 239 | #559 | A through-all Hole along a wall or beside curved faces: the cut fails | `face.hole` 239 | 50 | corpus/m0-revolve-primitives `Body1/F2:end` |
| 150 | #560 | Hole on a sheet metal face is refused | `face.hole` 150 | 12 | corpus/modern-sheet-metal `Body1/F2:face:0` |
| 146 | #561 | Loft, Sweep and Coil faces have no source | `face.offset_plane` 38, `face.sketch` 38, `face.extrude` 35, `face.press_pull` 35 | 10 | oracle/16-sweep `Tube/F3:end` |
| 120 | #431 | Fillet or chamfer along a curved edge: the ball doesn't fit along the edge | `edge.chamfer` 70, `edge.fillet` 50 | 16 | corpus/modern-sheet-metal `Body1/F2:face:14\|F2:face:16` |
| 100 | #492 | A face split into pieces by a later feature | `face.extrude` 25, `face.offset_plane` 25, `face.press_pull` 25, `face.sketch` 25 | 5 | corpus/issue-489-stacked-join `Body1/F2:top#[F2:side:l1,F4:side:l5]` |
| 98 | #491 | A face that a Join merged from two features' faces | `face.sketch` 27, `face.offset_plane` 25, `face.extrude` 23, `face.press_pull` 23 | 12 | corpus/issue-489-stacked-join `Body1/F2:side:l2` |
| 86 |  | Shell's other refusals: a removed face meeting a round as large as the wall, freeform (loft) faces, walls thinner than twice the shell (expected, not filed) | `face.shell` 86 | 22 | corpus/modern-plastic `Body1/F3:face:15` |
| 84 | #566 | Shell of swept and sheet metal bodies: a corner whose faces don't meet the same way | `face.shell` 84 | 12 | oracle/30-fillet-after-shell `Box/F3:inner:F2:start` |
| 82 | #562 | Press Pull beside rounds and curved neighbours: the join fails | `face.extrude` 41, `face.press_pull` 41 | 13 | demo/enclosure_steps `Enclosure/F2:start` |
| 69 |  | Blend larger than the faces around it (the 10% radius doesn't fit; expected, not filed) | `edge.chamfer` 38, `edge.fillet` 31 | 11 | corpus/modern-plastic `Body1/F2:inner:F1:+x\|F3:face:19` |
| 56 | #563 | Draft and tapered Extrude drop the faces' sources | `face.extrude` 14, `face.offset_plane` 14, `face.press_pull` 14, `face.sketch` 14 | 3 | oracle/05-taper-extrude `Frustum/F2:start` |
| 48 | #564 | The rim a Shell leaves has no source | `face.extrude` 12, `face.offset_plane` 12, `face.press_pull` 12, `face.sketch` 12 | 8 | demo/pi-case `Body1/F2:top` |
| 18 | #567 | Axis or tangent plane on a drill tip: no face at NaN | `curved.axis` 9, `curved.tangent_plane` 9 | 3 | oracle/18-hole-features `Plate/F5:face:15` |
| 12 | #568 | Construction geometry on model edges, faces and vertices is lost when a parameter moves it | `vertex.plane_three_points+edit` 5, `face.midplane+edit` 2, `curved.axis+edit` 2, `edge.normal_plane+edit` 2 | 4 | demo/enclosure_steps `Enclosure/v[F2:top & F4:inner:F2:side:l4 & F4:inner:F3:blend:F2:side:l` |
| 12 | #565 | Split Body and Combine Intersect drop the faces' sources | `face.offset_plane` 4, `face.sketch` 4, `face.extrude` 2, `face.press_pull` 2 | 1 | oracle/22-split-body `Cyl (1)/F2:bottom` |
| 8 |  | Axis through a face the kernel sees as a cylinder that the command doesn't (sensor enclosure slot ends; not filed yet) | `curved.axis` 8 | 1 | examples/sensor_enclosure `Lid/F20:side:ls#1` |
| 7 |  | Other failures in the edit round: the hole cut and blend sizes above, and an extrude from the M0 box's top face whose profile isn't found again (not filed yet) | `face.hole+edit` 3, `face.extrude+edit` 1, `face.press_pull+edit` 1, `edge.chamfer+edit` 1 | 3 | corpus/m0-box-fillet-hole `Body1/F2:top` |
| 2 |  | A helix edge projected along its axis is a point (expected) | `edge.project` 2 | 1 | oracle/50-sweep-tight-bend `Bend/F3:side:c1#1\|F3:side:c1#2~2` |

Where the face history stands (`crates/kernel/src/history.rs`, [face attachments](face-attachments.md)):
a face takes an attached sketch only when the body carries an exact source certificate for it.
Extrude caps and sides, boxes, shells' inner walls, STEP faces and sheet metal panels have one,
and booleans, fillets and shells pass untouched faces' sources through. Everything that makes or
reshapes faces without recording one (cut walls, holes, chamfers, revolves, lofts, sweeps, drafts,
tapers, splits, intersects, shell rims, pattern and mirror copies, sheet metal rims) leaves faces
that refuse a sketch, and press-pull and extrude-from-face then fall back to a legacy face sketch
that is found again by a point. Together they are the largest group of failures.
