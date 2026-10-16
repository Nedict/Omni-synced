# F3D reference evaluation — 2026-10-13

The user corpus contains 17 native designs: four standalone F3D files and thirteen
F3D members of the two supplied ZIP packs. The small controlled feature tests
are not a substitute for opening these complete designs.

## Current strict-history increment — 2026-10-16

Eight of the seventeen designs now pass strict reconstruction, regeneration and
container reopening: Minimal, Crescent, Eye, Narrow, Round, Square, Style 5 and Style 6.
Style 5 retains fourteen features without warnings. Explicit retirement and
agreeing current edge trackers resolve its final projected circle; its 10 x 10 mm
square pocket follows the saved depth parameter and passes analytic removal/edit
checks. Style 6 retains all eight features without warnings, resolving its paired
chamfer output labels through exact input trackers and its cut-cap normal on the
retained solid. Nine other designs still fail the strict gate.

This gate is not a full source-fidelity proof. Style 5's regenerated volume is
6208.832 mm³, whereas native-body recovery measures about 6868.534 mm³ with
conversion-loss warnings. The final pocket removes the expected 700 mm³ and a
7-to-5 mm depth edit restores 200 mm³. The remaining shape discrepancy is unresolved.
The branch is rebased onto `3707a67c`, including upstream's typed feature fields,
mesh collision optimization, shell saddle corners and trim acceleration. The last
commit updates generated documentation only; all pending code hashes are unchanged.
All 177 original saved-history recipes, the legacy-offset case and 34 recovery
open/save/reopen cases pass on this integration. Recovery counts, display volumes
and warnings are unchanged. The focused prism and cap tests and kernel/I/O
all-target Clippy pass. HardEdgeBasin opens and prepares its display in **1.957
seconds** (20-second regression budget). The preceding full CI passed all 327 I/O
tests and WebAssembly but exposed geometry regressions. The branch-specific
connected-fillet and shell-volume regressions are now fixed; a separate clean
upstream run reproduces the seed-20 Boolean failure. That test also fails on the
final branch after both fixes, so the required CI gate remains blocked. No PR has
been opened yet.
Style 2's complete symmetric edge group now resolves, and tangent continuation
excludes smooth internal seams. Fillet4 still fails at a rolling-ball section
constrained to a support-surface seam. Its limiting-radius shoulder now consumes its
zero-area walls, covered by independent analytic volume and edit checks. The
other 33 recovery records are unchanged. The first strict failure for `style-1`
is now Sketch7's legacy entity metadata; complete reconstruction is still unfinished.
Face-only and mixed face/edge fillet containers decode, but live face-boundary
regeneration remains unfinished. The adapter still hits the assembly guard.
[Detailed evidence and integration status](f3d-upstream-recheck.md).

## References

| Project | Revision reviewed | Use and limitation |
|---|---|---|
| [cadmpeg](https://github.com/cadmpeg/cadmpeg) (Apache-2.0) | `8c19a6021ef70fe5134c9d4d08fb631725510c16`, reports version 0.6.1 | Executed the F3D decoder on all 17 designs. Public format notes and typed native records provide useful evidence. Neutral feature definitions still contain unresolved selections; successful decoding does not prove editable regeneration. |
| [ezf3d](https://github.com/AlexSabaka/ezf3d) (MIT) | `97fe90ab8c39c3978fa6354f4a21c5ebdbc38c6e` | Reviewed documentation and dependency metadata. Useful independent archive, parameter, timeline and B-rep reader. Its own roadmap leaves general feature-graph transpilation unfinished. Not executed in this evaluation. |
| [f3z-to-solidcraft](https://github.com/queball1999/f3z-to-solidcraft) (MIT) | `391ef19e5d85ef124c9c21836cf94dd75db263fa` | Reviewed its documented translation approach. It explicitly infers some dimensions and selections, skips unsupported operations, and reports that no design fully matches yet. Not executed; those fallbacks are not suitable as a silent history-preserving import path. |

The initial evaluation added no runtime dependency. The subsequent explicit
recovery modes now link cadmpeg 0.6.1 on desktop for native body conversion.
Native archives, full decoded models, fidelity sidecars and measurements stay
outside the repository.

## Measured decoder coverage

Built cadmpeg with only its `f3d` feature and used `dump --report` with its default
desktop resource limits. Sixteen files produced geometry. Gridfinity stopped at
the 16,000,000 collection-item limit while framing SAB tokens. Limits were not
disabled.

| Designs | Decoder result |
|---|---|
| Minimal; Crescent, Eye, Narrow, Square, Style 6 | Geometry decoded, no decode loss reported. |
| Adapter, R36S, modular clamp; Round, Style 2, Style 3, Style 4, Style 5, Style 7, style-1 | Geometry decoded, design-intent warnings remain. These include native-only feature definitions, unresolved profile/face/edge/body selections, and unresolved dimensions or parameter ownership. |
| Gridfinity | Resource-limit refusal; no decoded model. |

Additional `check` runs on the decoded CADIR passed for the adapter, Crescent,
Eye, Narrow, Square and Style 6. Minimal reported an ACT document-root link error.
R36S reported thirteen errors involving pcurves, procedural support and an ASM
history chain. These checks validate the decoded representation; they do not
compare a regenerated SolidCraft history with the source design.

For example, cadmpeg emits 47 neutral feature entries for the adapter's 49 native
timeline entries and 52 for R36S's 53. Entry-count differences and native operands
must be resolved before using the neutral graph as an authoritative editable
timeline. Do not treat the decoder's exit code as a fidelity verdict.

## Changes supported by the evidence

The public WorkPoint grammar identifies `boundingboxIndex` as unused for a
two-edge intersection. SolidCraft previously required its bytes to be zero,
rejecting the adapter's valid Point1 record. The reader now consumes that field
without interpreting it as a reference, supports point-data v2/v3 and the verified
legacy/modern geometry envelopes, and still checks ownership, source references,
method and cached position. Both adapter point definitions match their unchanged
modern resave. Modern native point imports still pass source-edit, volume,
position, reopen and undo checks.

Several knob sketches also have the v17 member list immediately followed by the
final parent reference. The reader previously interpreted that reference as an
extension-table count. It now accepts the exact final-reference envelope while
retaining and validating extension metadata when present.

Import errors now identify the failing timeline object, class and version;
reference failures also include the record offset or expected/actual class.

## Complete-import acceptance gate

`apps/solidcraft-cli/tests/f3d_corpus.rs` attempts every archive instead of stopping
at the first failure. It requires nonempty history, successful regeneration,
positive finite solid volume, lossless history container round-trip and stable
reopened volume. Minimal also has its independent analytic cylinder check. The
gate is opt-in because the user files are not committed and compatibility remains
incomplete. A failed import is a failed gate, not a passing expected-rejection test.

```sh
SOLIDCRAFT_F3D_CORPUS=/path/to/test_files \
SOLIDCRAFT_F3D_ADDITIONAL_CORPUS=/path/to/extracted_f3d_members \
cargo test --profile ci -p solidcraft-cli --test f3d_corpus -- --ignored --nocapture
```

The initial result is **1/17 complete imports (Minimal)**. The construction-point
fix advances the adapter to its chamfer blocker; it does not make that complete
design importable. General editable F3D support remains unfinished.

Validation of this change: 130 focused F3D tests passed, along with the native
point import/edit controls and original/resaved adapter point comparison. The
full quality gate found a byte-offset mistake in the new authored test fixture's
variable-length name; that fixture was corrected and the focused suite rerun.
All other full-gate tests passed, including the saved-history fixtures and the
opening-performance guard (1.03 seconds). Final formatting and clippy checks
passed; WASM, assets, licences and layering passed in the full gate.

The initial investigation checked upstream at `be55a49b` and used the F3D
branch's integrated baseline (`0f0bb098`). The later rebase results are recorded
below; those supersede the initial integration status.

The most useful next integration target is cadmpeg's native, versioned record
and historical-topology decoding. Retain SolidCraft's proven expression,
constraint and feature reconstruction where it is more complete. Any shared
decoder must first satisfy bounded-input handling and browser/WASM builds, and
each mapped feature needs regeneration and edit checks. Native-only definitions,
ambiguous selections and missing timeline entries remain explicit errors in
strict mode and are listed as omissions in user-selected recovery modes.

## Explicit recovery modes

The desktop open dialog now offers editability or correctness as a preference.
Editability reconstructs supported history and omits failed features and their
dependants. If no solid can be rebuilt, it attempts native bodies while retaining
recovered sketches. Correctness tries native bodies first, then partial history.
Neither option certifies the final geometry. Import reports persist on save.

All 17 supplied archives opened and survived save/reopen in both modes in the
opt-in recovery test. Sixteen recovered solids; Gridfinity produced an empty
document with an explicit report. Tests verify stable finite display-mesh volume
on reopen and independently check Minimal's analytic cylinder volume in both
modes. This is recovery coverage, not complete editable-history coverage.

The native adapter raises the decoder's collection budget from 16M to 64M while
retaining byte, recursion and work budgets. Gridfinity then reaches cadmpeg's
4-billion work-unit budget (one requested operation accounts for about 24 billion
units). These are software limits, not detected RAM exhaustion. Cold display
meshing of the modular clamp remains slow. Full-history reconstruction and
fidelity comparisons against Fusion remain unfinished.

## Upstream rebase validation — 2026-10-15

The isolated `feat/f3d-import-rebased` branch was rebased onto upstream
`e4ef6cfe` (0.11.0), then onto `9935133b` after rendering and preview changes
landed during validation. Integration preserves upstream's file-load scopes, new
feature geometry traversal, offset parameters, and loft section parameters.
Unpublished development file versions use explicit `solidcraft-f3d-dev/N`
tags so they cannot reinterpret upstream's numbered formats.

All 34 archive/preference combinations passed opening, display-mesh checks,
save and reopen on both `e4ef6cfe` and final upstream base `9935133b`.
Sixteen archives produced solids; Gridfinity
opened an empty document with diagnostics. Minimal passed the independent
analytic volume checks. The final corpus run took 102.58 seconds. An earlier
run took 786 seconds under concurrent compilation and geometry tests; these
wall times are not a controlled performance comparison.

The final workspace all-targets check, formatting, legacy offset edit/reopen
test and HardEdgeBasin opening guard passed. The latter opened and prepared
14 bodies in 2.32 seconds. Focused migration, clipboard, offset and file-open
dialog tests passed before the final rendering/preview rebase. The full quality
gate is not green. Three
kernel failures also reproduce on an unmodified `e4ef6cfe` snapshot:
`a_blend_along_a_boss_ends_on_the_box_side_beside_it`,
`arc_top_edge_sizes_never_panic`, and
`fillet_the_three_edges_at_the_arcs_front_corner`.

The initial rebase exposed two branch regressions, now fixed and checked:

- Circular section winding follows neighbouring planes, rather than flipping
  when a bend crosses 90 degrees from its first plane. Five engine tests pass,
  including both hollow-elbow suites, short bore overruns and other loft cuts.
  Opposing section frames still retain their independent analytic-volume checks.
- Explicit uniform interpolation uses a separate kernel path. New commands and
  upstream files retain chord-length spacing; older development files migrate
  to explicit uniform spacing. The three-circle uniform fixture again produces
  its independently expected 45,401.669 mm³ rather than 50,118.105 mm³. All 130
  authored histories pass replay, geometry, edit, save/reopen and undo checks;
  the separate legacy offset fixture also passes. Expected geometry was not changed.

The seam-boundary robustness case and robustness golden `sample_0` also fail
on clean upstream. Broad robustness
golden reruns were stopped after this failure was established; they are not
reported as passing. The PR gate remains closed. Recovery coverage alone does
not establish that the feature is ready to merge.

## Legacy adapter chamfer groups — 2026-10-15

The original adapter's three chamfer groups have a 25-byte tail after their
group index. Its unchanged modern resave has a 26-byte tail, adding a true flag
before suppression. The group class remains version 2 while the parent feature
changes from version 2 to 3. The reader now accepts the exact legacy envelope
for version-2 parents and continues validating suppression, reserved bytes,
reference classes, owner, group order and feature/container agreement.

All three original group definitions match the modern resave's selectors,
dimension bindings and suppression. The 135 focused F3D tests pass, including
authored legacy records, every truncation of their tails, and wrong-owner
rejection. The native comparison is an opt-in test; source archives remain
outside the repository. This removes a decoding blocker, not the adapter's
remaining feature reconstruction and topology limitations.

## Gridfinity budget probe — 2026-10-15

A separate process tested a 32-billion work-unit allowance, retaining the
64-million collection limit, 2-GiB retained-byte limit and a 180-second timeout.
The decoder advanced past work admission but stopped after 12.7 seconds at
the retained-byte budget while collecting historical surface planes. No STEP
output was produced and application limits were not changed. Raising the work
allowance alone therefore does not make this file recoverable; historical
topology materialization needs further work. This budget accounting does not
measure the process's peak resident memory.

## Latest upstream opening check — 2026-10-15

The branch is now based on upstream `2e7f41710bee953102108dea7748565204c97973`,
including the four commits after `9935133b`. The rebase's only conflict was the
engine test-module list; both upstream face-name tests and the feature's fillet
tests were retained. The pending loft, chamfer, fixture and locale fixes were
preserved and reapplied.

The rebuilt corpus gate again passes all **34 recovery-mode open/save/reopen
checks**, with solids from 16 archives and an explicitly empty Gridfinity
document in each mode. The combined strict/recovery run took 204.65 seconds.
Strict editable-history opening remains **1/17**, with Minimal passing its
independent analytic volume check. The first strict failures are:

| Files | First unsupported record |
|---|---|
| Modular clamp | Extrusion 457, version 8: unsupported extent layout. |
| Crescent, Eye, Narrow, Round, Square, Styles 2–6 and style-1 | Sketch projections use version 1 where this sketch envelope currently requires version 3; legacy multi-entity projection decoding is incomplete. |
| Style 7 | Timeline object 23341, class `11F1A5CE-2B57-4476-8480-6994621493C9`, version 0. |
| Gridfinity | Chamfer 16766, version 2: missing selection reference at record byte 40. Native-body fallback still reaches its software resource budget. |
| R36S Grip | Timeline object 1160, class `D7015B81-AB4D-4C89-BED7-AA4411E1964A`, version 4. |
| Adapter | Fillet 18835, version 2: selection-container layout differs from the supported envelope, failing at record byte 27. The preceding legacy chamfers now decode. |

All 130 authored history fixtures and the separate legacy-offset test pass
replay, geometry checks, edits, save/reopen and undo on this base (178.57 seconds).
The HardEdgeBasin opening guard passes in 8.99 seconds under concurrent load,
with 14 bodies and 14,898 triangles. Three file-input/failed-open safety tests,
formatting, asset attribution and dependency layering also pass.

The preceding full quality run on `9935133b` found missing Japanese/Chinese
inspection labels (now added), sandbox-only socket failures (their exact
unsandboxed reruns passed), and a 90-second action-matrix timeout. Its remaining
engine/kernel test processes were stopped when upstream advanced; that run is
incomplete, not green. No full quality-gate success or PR readiness is claimed.
The latest import results still do not meet the requested condition for opening
a PR.

## Legacy projections and geometry — 2026-10-15

Projection records now select their own versioned envelope independently of the
sketch version. Supported whole-face projections preserve all four rectangular
edges, or the single circular edge, with live source identities. Mapping order
is matched by construction identity rather than array position. Authored tests
exercise dimension edits, save/reopen, analytic volume, missing and duplicate
relationships, conflicting owners, and truncated records. Native probes decode
the previously rejected simple cap projections in nine knob archives.

Single-root component version 18 is accepted alongside version 23, retaining
ownership checks. Version-17 geometry also accepts the exact terminal-owner
form without an intervening relationship list. Native point, line and circle
records in nine affected knob archives pass this decoding check.

The resulting corpus run on `2e7f4171` completed in 132.36 seconds. Strict
editable-history coverage increased to **2/17**: Square now opens and reopens
with four features and volume 1,925.625 mm³, alongside Minimal. All **34 recovery
open/save/reopen checks** still pass, with 16 archives producing solids and
Gridfinity empty with diagnostics. Correctness-mode results are unchanged.
Editability mode recovers more knob geometry; its intermediate shapes must not
be treated as verified final shapes. The first remaining strict failures are:

| Files | First unsupported record |
|---|---|
| Modular clamp | Extrusion 457, version 8: unsupported extent layout. |
| Crescent, Eye, Narrow, Styles 3–6 and style-1 | Sketch container relationships, including body-supported sketch attachments. |
| Round, Style 7 | Timeline class `11F1A5CE-2B57-4476-8480-6994621493C9`, version 0. |
| Style 2 | Fillet 3963, version 2: unsupported history flags at byte 38. |
| Gridfinity | Chamfer 16766, version 2: missing selection reference at byte 40; native fallback also exceeds its software budget. |
| R36S Grip | Timeline object 1160, class `D7015B81-AB4D-4C89-BED7-AA4411E1964A`, version 4. |
| Adapter | Fillet 18835, version 2: missing selection-container reference at byte 27. |

The focused F3D suite passed 138 tests before the root/terminal-owner additions;
the subsequent root test and six legacy-geometry tests also passed. Native
projection and entity probes, IO all-target Clippy, and 16 locale tests passed.
These checks do not constitute a green full CI run.

## Final upstream recheck — 2026-10-15

Upstream advanced during validation. The isolated branch was rebased again onto
`16c49c3e860a954e9f27309927d6848bcba9c607`, including parameter cursor navigation
and Untrim. Applying the saved work produced only a generated feature-summary
conflict; upstream's completed Untrim status was retained. Source changes merged
without conflicts, and formatting and diff checks pass.

The rebuilt native corpus took 146.63 seconds: **34/34 recovery checks pass** and
**2/17 strict imports pass**. Every recovery result's feature count, body count,
volume, representation and warning count matches the preceding run. The strict
failures remain those listed above. All 130 authored histories plus the separate
legacy-offset case pass (131.71 seconds). HardEdgeBasin opens and prepares display
geometry in 3.37 seconds, retaining 14 bodies and 14,898 triangles under the
20-second regression deadline. This is not a controlled speed comparison.

No PR has been opened: the requested all-success condition is not met. Recovery
coverage, particularly an empty Gridfinity document, is not full import fidelity.

## Live sketch attachments and directed projections — 2026-10-15

Body-supported sketches now retain certified face attachments. Plane-definition
versions 8 and 15 validate the support selector, body ownership, feature ownership,
cached placement, and complete record envelope. Direct face recipes name their
producer and cap/side role without requiring a bounded edge loop. They remain
distinct from whole-boundary projection recipes. Sketch frames retain in-plane
translation and rotation, follow changed extrusion heights, and report a lost
attachment when their producer disappears. Invalid supports omit the sketch in
recovery mode instead of silently converting it to a fixed plane.

Native checks validate the first body-supported sketch in eight knob archives
and the modern adapter resave. Authored tests cover height/width edits, volume,
save/reopen, rotated/translated frames, source suppression, contradictory body
references, and all truncations of the plane, selector and face-recipe records.

A separate regression exposed reversed projected-line parameter directions:
the kernel's edge traversal could become the imported line's start/end order.
The importer now preserves native endpoint order, including after source edits
and reopening. This fixes a real selection-identity defect, but does not resolve
all remaining native partial-profile failures.

Validation passes **144 focused F3D tests**, the opt-in native attachment probe,
and IO all-target Clippy. The final corpus run took 94.66 seconds and again passes
**34/34 recovery checks**, with the same feature/body counts and volumes. More
constraints and attachments survive: Crescent and Narrow each now have one
omission warning rather than four. Strict coverage remains **2/17**.

Crescent and Narrow advance to their second extrusion. A local diagnostic shows
that Crescent selects the same directed curves as one regenerated profile, but
its source records describe two pieces per curve where the regenerated sketch
has three. The piece-count contract is still unresolved; the importer does not
discard that validation or guess a region. Eye still has a missing selected
curve; Styles 3/4 reach failed prerequisite regeneration, Styles 5/6 reach
unsupported fillet edge identities, and style-1 reaches profile-record flags.
The other previously listed first failures remain.

After validation, the branch was rebased without conflicts onto upstream
`d73cd8a177c792889c6f1b3f6ae27b0c389878b3`. Its two new commits affect action-matrix
test infrastructure and robustness goldens, not the importer or its runtime
dependencies. No additional full-suite success or PR readiness is claimed.

## Boundary-scoped profile partitions — 2026-10-15

Crescent and Narrow store piece counts relative to their named boundary curves.
Other sketch curves add partitions outside those selected boundaries: Crescent's
circle changes from two pieces to three, and Narrow's from four to six. Removing
those other curves from a temporary profile graph reproduces the native directed
piece identities exactly. The importer now resolves that graph and translates
the selection to the full sketch only when source identities, directions, outer
and hole boundaries, and analytic segments match a unique complete region.
The full sketch and its constraints remain intact. Saved selections use the
ordinary full-sketch piece contract; count validation is not weakened.

Authored fixtures cover unused-side intersections, rotated circle seams,
diameter/height edits and save/reopen. Negative checks cover wrong counts,
directions, missing holes, divided regions and ambiguous matches. Native opt-in
tests independently integrate circular segments to check Crescent and Narrow's
cut volumes, then change Diameter, Inset, InsertWidth and InsertDepth and repeat
the volume and container-reopening checks. Both native files retain all four
features without import warnings.

On upstream `d73cd8a1`, strict corpus acceptance increases to **4/17**: Crescent,
Narrow, Square and Minimal. All **34 recovery checks** pass in the same 93.49-second
run. Only Crescent/Narrow editability results change; all other recovery feature
counts, body counts, volumes, representations and warning counts are unchanged.
Gridfinity still opens empty with diagnostics. The focused F3D suite passes
**146 tests (8 opt-in tests ignored)**; the new native volume/edit test and IO
all-target Clippy also pass. Full CI and complete import fidelity are not claimed.

Remaining strict failures include Eye's missing selected curves, Styles 3/4's
selected boundaries that do not yet map to a complete sketch region, unsupported
fillet selections, extrusion extents and timeline records. Eye's missing curve
identities `(129, 0)` and `(127, 0)` belong to profile 514, owned by active timeline
feature 380; they cannot be dismissed as an unused profile. Its later profile 921
also has an unhandled tracking tail. These require further native-history work.
No PR has been opened because the requested all-success condition remains unmet.

The branch was subsequently rebased without conflicts onto upstream `ed22d663`
(Smart Extrude direction and cut detection). Importer, document and kernel files
match their pre-rebase state. The rebuilt corpus again passes 34 recovery checks
and four strict imports in 103.84 seconds, with identical reported outcomes.
Upstream's `extrude_auto_operation` regression test passes, as do formatting and
diff checks. The saved work was reapplied and retained; no full-CI success or
complete-import claim is made.

## Tangent contacts on selected boundaries — 2026-10-15

Styles 3/4 select a circular strip bounded by two arcs and two lines. Additional
curves touch the selected arcs at their poles, splitting each arc into two
full-graph pieces without changing the directed boundary. The translation now
compares contiguous analytic runs, including cyclic joins and full-circle seams.
Curve lineage, directions, holes and unique-region checks remain mandatory.
Every original segment is checked against the final carrier to prevent tolerance
drift; different carriers, missing holes and interior region divisions remain
explicit errors.

An authored native-record fixture exercises a tangent contact, three circle-seam
orientations, height edits, save/reopen and rejection after an edit moves the
contact inside the selected region. Geometry tests cover major arcs, line splits
across the loop seam, reordered holes, reversed boundaries and displaced holes.
Native Style 3/4 **prefix** tests check the repaired cylinder-and-strip join
against independent analytic volumes, edit Diameter, KnobWidth, Height and
KnobHeight, and save/reopen. These are feature-level checks, not full-file passes.

The focused suite passes **148 tests (9 opt-in tests ignored)**; both the new
native-prefix test and the existing Crescent/Narrow native edit test pass, as
does IO all-target Clippy. The full corpus run on `ed22d663` takes 98.50 seconds:
**34/34 recovery checks pass and strict imports remain 4/17**. Editability mode
retains an additional extrusion in Style 2, Style 3, Style 4 and style-1, increasing
each from three features to four and removing one omission warning. Other
recovery outcomes are unchanged. Style 3 now reaches an unsupported projected
body recipe; Style 4 reaches an unsupported fillet edge identity. Gridfinity
still produces no recovered geometry. The full-import/PR gate remains unmet.

Upstream advanced to `b1695873` (Ruled Surface and UI fixes). The rebase and saved
work restoration completed without conflicts. Workspace all-targets checking,
formatting and diff checks pass. The rebuilt corpus completed in 98.65 seconds,
again with 34 recovery checks and four strict imports passing; every reported
outcome matches the pre-rebase run. These checks do not replace full CI.

The next Style 3 projection is record 3401, selector 3405, identity 3408. Its
bounded-face recipe has four terms; one term includes both labels 3 and 5 for
operation 308 (Extrude3, feature 541), alongside label 3 for operation 301
(Extrude1, feature 216). The existing simple face decoder cannot represent that
multi-label alternative. The later Sketch2 projection also references operation
303 without a current owner mapping. Current native inspection confirms that
these sketches and Eye's missing-curve extrusion are unsuppressed, so omission
as inactive history is not a valid fix.

## Symmetric extrusion distance and legacy converted parameters — 2026-10-15

The clamp's first extrusion uses symmetric distance, with identical references
for both distances and both taper inputs. The reader now accepts that older
envelope and the current envelope with an empty opposite-distance slot. It checks
the repeated identities, extent markers, reserved fields and length convention;
independent two-sided distances remain unsupported.

Both full-length and per-side distance modes are verified with controlled local
exports, using the public [symmetric extent API](https://help.autodesk.com/cloudhelp/ENU/Fusion-360-API/files/fusion_ExtrudeFeature_setSymmetricExtent.htm).
SolidCraft's symmetric distance is per side. A native full-length parameter stays
at its original value in Parameters and drives the feature through `length / 2`,
so dependent expressions retain their units and meaning. Per-side inputs bind
directly to the feature. Nonzero symmetric draft still requires kernel support;
it is not flattened into an untapered shape.

Original authored-record tests check both envelopes, both length conventions,
centered bounds, independent cylinder volumes, negative distance, dependent
diameter edits, container reopening, invalid bindings and truncation. Four local
native zero-draft controls also pass import/edit/reopen checks. Two original
SolidCraft designs, `symmetric-extrude-half` and `symmetric-extrude-full`, join the
saved-history suite; the suite now validates 132 recipes and their editable files.
A headless front-view render confirms the cylinder spans both sides of the
sketch plane. Native exports and rendered evidence remain outside the repository.

The clamp also contains a v6 converted dimension (`d157`) with no binding. Its
expression is `( Clamp_Width - Chamfer_Small * 2 ) / 2`. Local API inspection
identifies it as a user parameter, and the unchanged v9 resave retains the same
conversion flag, value and expression. The reader preserves this unbound legacy
form; a legacy conversion retaining a live binding remains rejected. Parameter
errors now include their record ID and schema version. The resave introduces
unrelated thread-parameter flags that remain unsupported; the comparison test
does not claim full resaved-file compatibility.

Validation: 151 focused F3D tests pass (11 opt-in tests ignored), both native
control tests pass, all 132 saved histories pass their edit/reopen/undo checks,
and IO all-target Clippy passes. Upstream remains `b1695873`. The strict corpus
still passes 4/17, but the clamp advances past extrusion 457 to unsupported
timeline feature 621 (`584D9526-EF22-4F6F-9FE1-5FCF1F3CE575` v2). Full CI and
complete import fidelity are not claimed, and the PR gate remains unmet.

The complete corpus run takes 108.72 seconds: all 34 recovery checks pass. Its
feature counts, body counts, volumes and representations are unchanged; the
clamp's editability warning count drops from 240 to 239. It still needs native
bodies for its solid geometry. Gridfinity still opens empty with diagnostics.


## Rebase and opening verification on `bd8db8ea` — 2026-10-15

Rebased the isolated feature branch onto upstream `bd8db8ea`, incorporating the
loft-hole validation, surface-extension and touchpad-navigation changes. The
loft conflict retains face-backed sections and upstream's rejection of sections
with holes. Removed the earlier redundant sketch-region rejection so upstream's
new diagnostic and regression tests are preserved. All pending feature work and
original fixtures were restored from explicit saved stash identities.

The rebuilt native corpus takes **114.32 seconds**. Complete editable-history
acceptance remains **4/17** (Minimal, Square, Crescent and Narrow). Both recovery
preferences pass all **34 open/save/reopen checks**, with exactly the same feature
counts, body counts, volumes, representation choices and warning counts as before
the rebase. Sixteen designs produce geometry; Gridfinity produces an empty
document with diagnostics in both modes. Recovery success does not imply complete
history or correct final geometry.

| Design | First strict-import failure |
|---|---|
| Modular clamp | Unsupported timeline object 621, class `584D9526-EF22-4F6F-9FE1-5FCF1F3CE575`, version 2. |
| Eye | Missing selected profile curve. |
| Round, Style 7 | Unsupported timeline class `11F1A5CE-2B57-4476-8480-6994621493C9`, version 0 (objects 575 and 23341). |
| Style 2 | Fillet 3963, version 2: unsupported flags at byte 38. |
| Style 3 | Unsupported projected body-edge / loft-face selection term counts. |
| Styles 4, 5, 6 | Unsupported fillet edge identity. |
| style-1 | Unsupported profile tracking flags at byte 205; grouped profile selections remain unfinished. |
| Gridfinity | Chamfer 16766, version 2: missing reference at byte 40; native fallback still exceeds the software resource budget. |
| R36S Grip | Unsupported timeline object 1160, class `D7015B81-AB4D-4C89-BED7-AA4411E1964A`, version 4. |
| Adapter | Fillet 18835, version 2: missing reference at byte 27. |

HardEdgeBasin's cold open plus display preparation passes in **1.96 seconds**,
with 14 bodies and 14,898 triangles. The regression also verifies the five known
fillet failures, model bounds, source-file preservation and mesh reuse on refresh;
it does not count a fast failed model as a successful opening.

The first rebase onto `41f0c3d0` passed all 19 engine loft tests, including the two
new upstream hole regressions, and the browser offline suite. The subsequent
upstream changes do not alter the resolved loft code. Formatting and diff checks
pass on `bd8db8ea`.

The earlier full CI run was stopped after it was superseded by the rebases; it
must not be reported as green. Its socket-dependent tests passed separately with
loopback access, and its viewport-theme failure passed a targeted retry. Three
kernel failures reproduced against `bd8db8ea`:

- `blend::tests::a_blend_along_a_boss_ends_on_the_box_side_beside_it`: blend rejection for a non-straight edge.
- `issue_851_tests::arc_top_edge_sizes_never_panic`: an unmeshed face at radius 10.
- `issue_851_tests::fillet_the_three_edges_at_the_arcs_front_corner`: face boundary does not lie on its surface.

These failures and the incomplete native imports remain unresolved. No PR is
opened under the user's condition that the checks succeed first.


## Grouped profile boundary verification — 2026-10-15

The tracking decoder now retains the selected member-region lists instead of
rejecting their count as nonzero padding. Import resolves each member to a unique
analytic region and checks its merged geometry against the separately stored
combined boundary. Missing/duplicate members, reversed directions, wrong holes,
truncated payloads and excessive counts reject. Persistent saved selections use
the full sketch's curve-piece identities and remain editable.

The native two-circle union, lens and separated-crescent controls pass independent
circle-intersection volume checks, height edits and container reopening. The
focused F3D suite passes 155 tests. All **133 original saved histories** pass replay,
geometry checks, edits, save/reopen and undo; the new grouped-profile-union example
was rendered and visually checked. I/O all-target Clippy, the three default F3D
inspection tests and asset attribution checks pass.

The native corpus takes **99.20 seconds**: all **34 recovery checks** still pass,
and all feature/body counts, volumes and representation choices are unchanged.
Complete history remains **4/17**. Fewer profile records are omitted: editability
warning counts fall from 239 to 232 for the clamp, 9 to 8 for style-1, 45 to 43 for
Gridfinity and 81 to 76 for the adapter. This is improved decoding, not proof of
additional complete geometry. Style-1's first strict failure advances from profile
tracking flags to unsupported fillet edge identity.

A new opt-in clamp prefix acceptance test still fails at the first member of
profile 846. A local API comparison confirms that the reconstructed source-line
coordinates agree before/after timeline rollback and recomputation, while the
current selected boundaries differ from the stored profile tracking. Persistent
selection remapping remains to be decoded; the mismatched cached selection is
not accepted or substituted with a nearby profile. This evidence and the three
previously reproduced kernel failures prevent a complete-import or green-CI claim.
Upstream remains `bd8db8ea`; no PR is opened.

## Retained selections after subdivisions (2026-10-15)

The branch is now based on upstream `406293fd`, rechecked after validation.
Controlled native before/after exports preserve identical tracking records when
a selected lens is split by a line or an interior circle. The reconstruction now
refines named source spans into exact full-graph spans, selects their union and
checks the merged analytic boundary. Explicit holes remain holes. Compound hole
loops permit only opposite shared spans; all selections still require exact
regenerated identities. No sampled point or area ranking picks a region.

Verification passes 157 F3D unit tests, the 24-test profile group (including four
opt-in native groups), four document identity tests and six sketch identity tests.
A separate native Style 7 test verifies the additionally recovered `Extrude3`:
its rectangular grip has volume `GripWidth × (Diameter/2 + GripHeight) ×
(Height − 2×Chamfer)`. Diameter, width, height and grip-depth edits and reopening
pass against this formula plus the independent base-cylinder volume. All 134
original saved histories and the legacy offset check pass. The new
`subdivided-profile-lens` example was rendered and visually checked: one filled
extrusion without an interior void. Sketch/document/I/O Clippy, formatting and
asset attribution checks pass. HardEdgeBasin opens with display in 1.957 seconds,
with 14 bodies and 14,898 triangles.

The final corpus run takes 98.72 seconds. All 34 recovery-mode open/save/reopen
checks pass; complete history remains 4/17 and Gridfinity still has no recovered
bodies. Style 7 editability recovery improves from three features/one body to
four features/two bodies, with mesh volume increasing by 468 mm³ and warnings
decreasing from 13 to 12. All other recorded results remain unchanged. This is
additional editable recovery, not a claim that Style 7's final shape is complete.

The clamp prefix still rejects profile 846, region 1, member 1. Reselecting its
current profiles writes a different tracking record, but its per-curve piece
scopes remain unexplained. That work, later unsupported feature families, the
empty Gridfinity result and the previously reported kernel failures remain open.
Full CI has not been rerun and no new commit, push or PR is made.

## Circular patterns and latest upstream verification (2026-10-15)

The isolated branch is rebased onto upstream `af2f09fa`, including the generated
feature walks, imported-thread display fixes and explicit symmetric extrusion
conventions. Native full-length distances now retain their parameter expression
with `total_distance`; half-length extents explicitly retain `legacy_per_side`.
The walk integration preserves
grouped fillet/chamfer parameter order, live To Object plane references, loft
sections backed by faces and legacy offset-expression dependencies.

Native circular patterns now retain editable count/angle expressions, certified
origin axes, body/feature selection and symmetric placement. Retired operation
slots are accepted only when the pattern's instance map certifies their ownership.
Round recovers four editable features instead of two, with five warnings instead
of seven. Its next strict blocker is a projection mapping count. Style 7's next
blocker is historical topology ownership; neither file is a complete import.

On the rebased code, all 136 original saved histories pass volume, edit, undo and
save/reopen checks. HardEdgeBasin opens and prepares display in 1.947 seconds,
retaining 14 bodies and 14,898 triangles, the expected remaining feature errors,
warm-cache reuse and unchanged source bytes. All 34 recovery open/save/reopen
checks pass with identical feature/body counts, mesh volumes, representations and
warning counts to the pre-rebase circular-pattern run. Gridfinity remains empty
in both recovery modes; the recovery smoke test explicitly permits that known
limitation and must not be mistaken for a successful geometry import.

Complete editable import remains **4/17**: Minimal, Crescent, Narrow and Square.
The other 13 fail the complete-import acceptance gate. In the original test folder,
Gridfinity first fails on chamfer 16766's missing reference at byte 40, R36S on
unsupported feature 1160 (`D7015B81` version 4), and the adapter on fillet 18835's
missing reference at byte 27. These failures still prevent the requested PR.

Focused validation also passes 162 F3D unit tests, nine feature-walk tests,
29 format tests, seven upstream extrusion-convention tests and all three
symmetric-extent tests, including local native files. Document/I/O/engine/UI
Clippy with all targets and warnings denied, formatting and diff checks pass.
Full CI was not rerun because complete-import acceptance still fails.

## Recheck after upstream 6eaf7458

The isolated feature branch rebased cleanly onto `6eaf7458`, preserving pending
work. Complete editable import and container reopening remain **6/17**: Minimal,
Crescent, Eye, Narrow, Round and Square. Both recovery preferences recover geometry
from **16/17** archives and reopen it; Gridfinity still returns an empty warning
document. This known empty exception is permitted by the recovery harness and
does not count as geometric success. Source-shape fidelity remains unverified.

All 151 original saved histories pass replay, parameter edits, undo and reopening,
along with the legacy offset compatibility case (105.79 seconds total).
HardEdgeBasin opens and prepares its display in 2.161 seconds, with 14 bodies and
14,898 triangles; its 20-second regression gate, expected feature limitations,
source preservation and warm refresh checks pass. These runs overlapped other
validation and are not isolated performance measurements.

Verified circular cap fillets and equal-distance chamfers now preserve complete
source-circle selections. Twenty-four native controls cover both caps and changes
to height, diameter and blend size, including suppression and restoration.
Deleted historical operation owners no longer block unrelated live selections.
Style 7 now reaches a later compound blend identity; Style 5 reaches asymmetric
circular chamfer orientation. Styles 1/4 still need compound blends, Styles 3/6
need compound projections, and Clamp, Style 2, Gridfinity, R36S and the adapter
still encounter unsupported feature records. The strict acceptance gate fails
on these eleven files, so the requested PR has not been opened. Full workspace CI
has not passed.

## Asymmetric circular chamfer follow-up

Style 5 now retains its third feature, Chamfer3, in editability recovery. Strict
import proceeds to Extrude5 and reports a missing selected profile: its container
1115 actually references body-face selector 1118, with bounded face recipe 1121.
That face-based extrusion remains unsupported; this is not a complete import.
The last full corpus acceptance result remains 6/17; only Style 5 was rechecked
after this decoder extension.

Seventy-two local controls validate two distances and distance/angle on circular
caps, including flipped sides, rotations and negative travel. Direct contact
checks distinguish cap setback from wall setback even when volume tolerances
would not. I/O tests pass (263 passed, 28 ignored), and all 153 original saved
histories pass edit, undo and reopening checks (129.57 seconds). Kernel regression
coverage also checks face history, incomplete loops, excessive dimensions and
invalid/duplicate picks. I/O and kernel clippy passed; a later selection-count
guard was additionally covered by the targeted kernel regression. Asset attribution
and whitespace checks pass. Full workspace CI has not passed and no PR was opened.

## Requested rebase onto release 0.12.0

The branch is now based on `c36a6f9f`, verified after testing. Rebase and pending
work restoration completed without conflicts. Complete editable import remains
6/17; both recovery preferences recover geometry from 16/17 files and reopen it.
Gridfinity remains empty. Style 5 now preserves its chamfer and stops at the
face-based extrusion. HardEdgeBasin passes its opening gate at 3.313 seconds,
with 14 bodies and 14,898 triangles. No PR was opened because the all-files
success condition is unmet. See [the recheck report](f3d-upstream-recheck.md)
for exact failures, preservation evidence and validation limits.

## Face-based extrusion follow-up

Native extrusions can now select a verified circular body cap. The imported
history includes a linked helper sketch on a persistent face anchor, followed
by the ordinary editable extrusion. Two-term trimmed-cap recipes retain both
the original extrusion operation and the chamfer boundary operation; import
requires the chamfer to select that producer's complete circular cap edge.
Unbounded cap attachments also resolve certified caps created by face extrusions
without requiring a nonexistent native sketch-profile record.

Style 5 now imports Extrude5 in editability recovery: five SolidCraft features
(the original sketch, extrusion, chamfer, linked helper sketch and new extrusion),
one body and zero evaluation errors. Its next failure is Sketch4's projected
boundary, whose side term carries two operation values. This is still an
incomplete design. The strict corpus remains 6/17, with the same six passing
files as the upstream recheck.

All **78 local face-extrusion controls pass** analytic volume and centroid
checks, named-parameter edits and reopening after the circular-cap cut fix.
The 12 previously failing chamfer-cap cuts now use exact boundary topology when
the tool and opening share a complete circular cap on a convex coaxial body.
Tool padding outside the body is clipped to the opening plane, with composed
face history; validity, meshing and Boolean result checks still apply. Off-axis,
through-body and unsupported boundary cases use the existing Boolean paths.
This fixes the controlled cuts, but does not establish complete import of the
supplied corpus; compound projected boundaries remain unsupported.

Portable validation passes **265 I/O tests** (28 opt-in tests ignored), I/O
all-target Clippy with warnings denied, asset attribution and whitespace checks.
All **155 original saved histories**, plus the legacy offset case, pass replay,
editing, undo and reopening (111.35 seconds). New original
`face-extrude-chamfer-top` and `face-extrude-chamfer-bottom` designs check source
height/diameter, both chamfer setbacks, extension distance and suppression against
cylinder/frustum formulas. The bottom-cap extension was rendered and visually
checked. No full-CI success, new commit, push or PR is claimed.

## Requested upstream recheck at 8b97b9ed

Rebased onto upstream `8b97b9ed`, retaining both sides of the test-module conflict
and restoring all pending work. The native circular-cap cut fix passes all 78
face-extrusion controls after integration. All 157 original saved histories and
the legacy offset case pass; HardEdgeBasin opens and prepares its display in
2.105 seconds. UI all-target compilation and formatting pass.

Complete editable import remains 6/17. Both recovery preferences complete all
17 open/save/reopen checks, but only 16 contain geometry; Gridfinity remains
empty with warnings. Style 5 now retains five features and reaches its next
unsupported compound projection. Full workspace CI has not passed. No PR was
opened because the requested all-files success condition remains unmet. See
[the recheck report](f3d-upstream-recheck.md) for the current failure table.

## Solved-record recovery on upstream 2bfec256

Both recovery preferences now open geometry from all 17 supplied archives and
pass all 34 save/reopen checks (174.30 s). Gridfinity's complete-history topology
binding exceeded cadmpeg's software work budget. A single bounded retry now omits
only the token-framed historical BREP tails, preserving solved records, body maps
and transforms. The original archive and strict editable import are unchanged.

Gridfinity recovers six bodies; several shells are open, so volume and mass are
not reliable and full final-shape fidelity is not claimed. Geometry warnings are
shown before long native-record omission lists. The corpus test no longer allows
an empty Gridfinity document. Complete editable import remains 6/17.

All 273 I/O unit tests pass (28 opt-in tests ignored), including the four new
fallback/framing regressions. I/O and CLI all-target Clippy pass with warnings
denied. Upstream remains `2bfec256`, already included in the branch. Full workspace
CI is still unverified; no PR has been opened. See [the recheck report](f3d-upstream-recheck.md).

## Partial cylindrical-wall union

Style 5's Extrude6 checkpoint now regenerates successfully: 6963.300 mm³ versus
an analytic 6963.977 mm³. A concave circular tool boundary can grow into existing
material at non-tangent straight neighbours using exact line–circle crossings.
The original `partial-cylinder-wall-join` design checks the circular-segment wing
volume, depth edits, suppression and saved-history behavior. Style 5 recovery
now retains seven features and one body without regeneration errors, but still
omits Sketch5's unsupported composite cap and later dependent/unsupported features.

## Mixed cap support and upstream e382a773

The composite support plane now resolves by each native contributor's logical
start/end role and exact Boolean-history aliases. All contributors must identify
the same unique current planar face at their nominal extrusion endpoint. A saved
attachment then follows that resulting face through edits. The new original
`partial-cylinder-cap-pocket` recipe checks a π mm³ pocket removal, base-height
edits, wing/pocket suppression, undo and reopening. There are 165 original recipes.

Style 5 still retains seven features: Sketch5 now fails at its projected outline,
not its support plane. Its face-boundary record names later fillets, while its
four curve trackers retain only the earlier neck and wing operations. Investigation
must distinguish that later boundary description from the authored projection
trackers; no later-feature dependency is silently substituted.

The branch was rebased without conflicts onto `e382a773`; all 175 pending files
were preserved. On that base, 275 I/O tests, targeted all-target Clippy, UI
compilation and the new saved recipe pass. HardEdgeBasin opens and prepares its
display in 2.905 s. The corpus remains 6/17 complete imports and 34/34 recovery
open/save/reopen checks, with geometry in all 17 files. Full workspace CI remains
unverified; no PR was opened. See [the recheck report](f3d-upstream-recheck.md).

The subsequent UI-only `fd422a3f` update was also rebased without conflicts;
import and geometry code remain identical to the tested `e382a773` revision.
