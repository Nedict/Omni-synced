# Latest upstream rebase and F3D opening checks

Upstream base: `650ba567` (2026-10-16, version 0.14.0). The feature commit
was rebased to `86c3e830`; pending improvements are restored and uncommitted.
All 135 untracked files match their preservation-stash blobs. No preserved path
is missing. Upstream G2 corners, curved radius laws and prism changes are retained
alongside the F3D tangent-chain, strip and pinched-endpoint fixes. Focused integration checks now pass; the full repository CI run is in progress.
Results in the earlier dated sections describe their own validated states.

The user allows unsupported features to be omitted, so both recovery preferences
form the opening acceptance gate. Full editable-history reconstruction remains
unfinished. The post-rebase corpus run passed **34/34 recovery cases** (17 archives in both
preferences), while strict history passed **8/17**. All recorded feature counts,
body counts, volumes, representations and warning counts match the preceding run.

The earlier full CI run failed at engine seed 20; that failure was also reproduced
on pristine `571122c4` and remained on the `07da7f80` integration. The same test still fails on this rebased branch
(99.43 seconds, Boolean Cut returned no result); a fresh pristine 0.14.0
comparison has not been run. No expectations were changed to hide that failure.
No PR has been opened yet.
Workspace: `/Users/chess/projects/cad/workspaces/f3d`, branch `feat/f3d-import-rebased`.

## Internal wall stations and upstream integration — 2026-10-16

The remaining Style 3 cut-history loss came from a horizontal wall-to-wall join
inside a slab, below its cap. The prism partition now includes certified internal
wall stations, retaining finite/work limits and a maximum of 64 levels. If the
partition budget is exceeded, geometry still uses the conservative cap partition;
unknown provenance is not promoted to exact history.

Before the upstream rebase, the extended original crescent-cut regression passed
for overlapping joins, both placements and changed radii. The native Style 3
prefix also passed with complete cut history and the expected source-face family.
The post-rebase native check also resolves both symmetric arcs together. The
remaining work includes grouped asymmetric face assignment and full-height
curved chamfers. The new station-bound test and native grouped-arc assertion pass.

Conflict review found that the pinched-contact path would reject a zero-radius
closed endpoint introduced upstream. The combined implementation retains both
cases; the regression checks the homogeneous point controls. Grouped fillets also
explicitly reject the unsupported G2 corner option instead of silently ignoring it.

The upstream endpoint-coincidence cleanup also conflicted with the existing
small-station regression. Splitting now uses sewing precision for endpoint
collapse; actual sewing removes a generated fragment only when both ends weld
to the same vertex. Whole authored lines and arcs retain their previous refusal.
All **51 prism tests** and upstream's repeated-text engraving test pass together;
the engraving takes **12.83 seconds** with no Boolean fallback (the first attempted
integration fell back and took 180.15 seconds). All **35 curved-blend tests** and
**3 grouped-fillet tests** also pass. In addition, **336 I/O tests**, all **178 saved
history recipes** plus legacy reopening, and all **11 WebAssembly crates** pass.
HardEdgeBasin opens and prepares its display in **2.531 seconds**, retaining
14 bodies and 14,898 triangles. Its two corner-radius error expectations were
updated solely for upstream's added `(site 1257)` diagnostic suffix.

The full CI run has passed formatting, asset attribution (92 files), sorted
catalogs, dependency layering and Clippy; its workspace test run is in progress.
The separate seed-20 check still fails, so no full CI pass is claimed.

Evidence before integration: `/private/tmp/f3d-internal-station-fix.log` and
`/private/tmp/f3d-internal-station-native.log`. Post-integration checks are recorded
in `/private/tmp/f3d-650ba567-final-evidence.json`; corpus comparison is in
`/private/tmp/f3d-650ba567-recovery-comparison.json`. Full CI output is
`/private/tmp/f3d-650ba567-ci.log`.

## Curved prism stations and cut history — 2026-10-16

A reduced original cylinder-and-grip cut exposed a kernel history failure:
curved joins between adjacent walls were treated as crossing every height,
blanking wall provenance and discarding the whole result history. The prism
reader now retains an exact station when positive rational control bounds prove
the entire curve lies at one height. Equal endpoints, negative weights and an
exhausted certificate budget do not suffice.

The original regression verifies two disconnected crescent cuts, changed radii,
a rotated/translated placement, complete face/edge origins, solid validity and
analytic volume within 0.02 mm³. At this earlier checkpoint the local Style 3 cut still lost history; the internal
station fix described above resolves that case. Chamfer2 remains unfinished.

Validation passes: **50 prism tests**, **336 I/O tests** (42 opt-in tests ignored),
the local native prefix check, all **178 saved-history recipes** plus the legacy
reopen test, workspace all-target Clippy with warnings denied, formatting and
all 11 WebAssembly crates. HardEdgeBasin opens and builds its display in
**2.2315 seconds**, retaining 14 bodies and 14,898 triangles.

The complete corpus passes **34/34 recovery cases**, with all recorded feature
counts, body counts, volumes, representations and warning counts unchanged.
Strict history remains **8/17** and is the only failure in the combined corpus
command (220.17 seconds of test execution). Full CI remains unpassed because of
the previously recorded engine seed-20 failure; no new commit, push or PR was made.

Evidence: `/private/tmp/f3d-curved-station-validation-results.json`,
`/private/tmp/f3d-curved-station-recovery-comparison.json`, and
`/private/tmp/f3d-curved-station-*-final.log`.

## Joined chamfer orientation and profile identities — 2026-10-16

Joined planar asymmetric chamfers now preserve the native primary setback face
through ordered endpoint cycles and the flip flag. Original tests cover both cap
sides, both flips, edited unequal distances, save/reopen, analytic removed volume,
centroid and actual setback edges. Cap certificates survive renamed history after
cuts. Side identities now accept uniquely certified regions in a multiple-region
profile and finite projected arcs, with negative ownership and interval tests.

The local Style 3 prefix check verifies both primary caps and both cut-profile
intervals. Complete Chamfer2 import still stops at the regenerated cut wall's
nonexact face history; curved asymmetric geometry remains unfinished too. The
prefix test explicitly does not claim complete chamfer reconstruction.

Final checks pass: **336 I/O tests** (42 opt-in tests ignored), workspace all-target
Clippy with warnings denied, the native prefix check, formatting and all 11
WebAssembly crates. The complete corpus again passes **34/34 recovery cases** with
all feature/body counts, volumes, representations and warning counts identical
to the previous run. Strict history remains **8/17** and is the sole failure in
the combined corpus command. Test execution took 216.23 seconds, 305.96 seconds
including compilation. No full CI pass is claimed; the previously recorded engine
seed-20 blocker remains unresolved. No new commit, push or PR was made. The final
fetch still shows `9d82b944`, and the branch has no open PR.

Evidence: `/private/tmp/f3d-joined-chamfer-validation-results.json`,
`/private/tmp/f3d-joined-chamfer-recovery-comparison.json`, and
`/private/tmp/f3d-joined-chamfer-{clippy,io,native,corpus,wasm}-final.log`.

## Joined cap projections — 2026-10-16

Style 3 now reconstructs Sketch6 and its Extrude6 cut: six editable features and
nine omission warnings, previously four features and eleven warnings. A single
joined cap without a fillet must match the successful producer snapshot exactly.
Its complete cyclic boundary and every mapped curve must also agree. Circular
projection pieces retain the kernel's exact carrier and directed sweep, including
short arcs represented by only two display points.

The native prefix check verifies two parameter sets and reopening. Fine-mesh
volumes are 13300.447746 and 16665.963536 mm³, within 0.05 mm³ of analytic values
13300.477070 and 16665.996565. Original tests also check stale-snapshot refusal,
source edits, plane movement, short and major arcs, reversed direction and
inconsistent samples. Style 1's rounded projection check continues to pass.

Before the test/config-only rebase, all 332 I/O tests (41 ignored), 19 projection
tests, 178 saved-history recipes plus legacy compatibility, workspace all-target
Clippy and 11 WebAssembly crates passed. The complete corpus again passed **34/34
recovery cases**, with only Style 3's editability summary changing. Strict history
remains **8/17**; Style 3 advances to its asymmetric Chamfer2 selection. The
production sources were byte-identical after rebase. The seed-20 full-CI blocker
is unchanged; updated upstream golden seeds have not yet been separately checked.

Evidence: `/private/tmp/f3d-joined-projection-validation-results.json`,
`/private/tmp/f3d-joined-projection-recovery-comparison.json` and
`/private/tmp/f3d-joined-projection-{doc,io,native,rounded,histories,corpus,wasm}-final.log`.

## Rounded cap projections — earlier 2026-10-16 checkpoint

Style 1's Sketch7 now reconstructs all eight projected outline curves after
Fillet1, and Extrude6 imports. The three boundary graphs preserve exact vertex
identities within each evaluation generation and a complete incidence bijection
between generations. The cap is resolved through its extrusion certificate, then
checked against the live sketch support and every mapped geometric curve.
Additional provenance must identify a decoded fillet on the same body. Ordinary
extrusion caps retain their stricter cache validation.

The native check passes save/reopen, changing fillet radius and moving the base
height by one millimetre, including the analytic cylinder-volume increment. An
original circle/strip grip test verifies tangent-derived outline geometry, live
links, analytic volumes and plane movement after edits and reopening. Negative
cases reject missing spans, another cap, absent modifier provenance and shifted
geometry. Field-authored graph tests retain exact intersection consistency and
reject missing or duplicate edges and unrelated producer identities.

The first I/O run caught an over-broad fallback accepting a malformed ordinary
rectangular projection. The additional-provenance requirement fixes it without
changing that regression's expectations. All 331 I/O tests now pass (40 opt-in
tests ignored), and all six regular incidence tests pass after bounding the graph
comparison to quadratic work. Workspace all-target Clippy with warnings denied
and formatting pass. The latest fetch still resolves upstream to `db1fd391`; no
additional integration was needed. The feature branch has no open PR.

The final retry passes all **34/34 recovery open/save/reopen cases**. Only Style 1
editability changes: seven features, one body, five omission warnings and a
display volume of 13816.532 mm³, compared with five features, seven warnings and
13863.528 mm³ before restoring the projected pointer recess. The other 33 recovery
summaries are identical. Strict history remains **8/17**; Style 1 now stops at
Sketch5's unsupported loft-cap boundary pieces, while Style 3 reaches the
single-cap projection's missing-fillet-provenance guard. The combined corpus
command exits 101 because the strict diagnostic fails; the recovery test passes.
Execution took 210.66 seconds (294.53 including compilation). All 11 WebAssembly
crates pass. No additional full CI run was made to reproduce the unchanged
Boolean robustness failure. There are 290 pending paths and no unmerged conflicts;
no new commit, push or PR was made.

Evidence: `/private/tmp/f3d-rounded-cap-native-3.log`,
`/private/tmp/f3d-rounded-projection-io-repaired.log`, and
`/private/tmp/f3d-rounded-projection-{clippy,incidence,corpus,wasm}-final.log`.
The comparison and check results are in
`/private/tmp/f3d-rounded-projection-recovery-comparison.json` and
`/private/tmp/f3d-rounded-projection-validation-results.json`.

## Cylindrical grip junctions — earlier 2026-10-16 checkpoint

A clipped cylindrical grip exposes a junction shared by two coincident cylindrical
wall pieces, a side wall and a shoulder. Sketch extrusion additionally splits the
cylindrical meridian into multiple height stations. The local fillet fallback now
joins the coincident cylinder pieces and follows an existing axial edge chain to
retain a valid periodic seam. It removes only intermediate vertices owned entirely
by that cylinder family, preserves provenance, and refuses deliberate split lines
or per-face paint. Face/edge counts bound the fallback. Existing periodic seams
may survive boundary rebuilding; a new repeated edge is still rejected.

At a convex grip's foot, the exposed shoulder grows as material is removed from
the grip. The extension check now distinguishes that case from the top cap, which
shrinks. Boundary-crossing checks and the normal fit/validity checks remain in use.

The original `split-cylinder-grip-corner-fillets` recipe and saved design cover
four vertical rounds, radius/diameter/base-height/grip-height edits, suppression,
undo and container replay. Volumes and centroids are integrated independently
from circular segments. The kernel regression checks three radii within 0.02 mm³,
a rotated/translated case, valid topology and uncrossed face loops. All 178 saved
histories plus the legacy-offset case pass after the latest integration (210.61
seconds of test execution, 296.94 seconds including compilation). Twenty related
curved-round tests, four connected-round tests and 32 prism tests pass. The first
prism filter selected zero tests; the corrected `prism::` filter provides coverage.

A public-safe rendering of the original grip was captured and inspected locally
at `/private/tmp/f3d-grip-corner-review.png`; it shows the four rounded vertical
corners and exposed shoulder without private document data. It is not committed.
The supplied-archive retry passes all 34 recovery cases. Only Style 1 changes:
editability retains five features rather than four, with seven warnings rather
than eight; its display volume changes from 13867.067 to 13863.528 mm³ as the round
is restored. Its strict import now advances past Fillet1 and stops at a projected
body selection. The other 33 recovery summaries are unchanged. Strict import is
still 8/17 overall; this does not certify full native fidelity.
Evidence: `/private/tmp/f3d-07da-{corpus-final.log,recovery-comparison.json}`.

HardEdgeBasin opens and prepares display in **2.2146 seconds**, retaining 14 bodies
and 14,898 triangles. The 20-second deadline, unchanged known geometry errors and
warm mesh reuse checks all pass. All 11 WebAssembly crates build. Assets, layering
and sorted catalogs pass. All 329 I/O tests pass (38 opt-in tests ignored), and
workspace all-target Clippy with warnings denied, formatting and whitespace checks
pass. The documentation-only final rebase did not change tested code. No further
full-suite rerun was made solely to reproduce the known failing CI regression.
The GitHub check found no open PR for this feature branch; none was created.

Current strict-import failures (recovery succeeds for each):

| Archive | First remaining failure |
| --- | --- |
| Modular Clamp | Unsupported timeline feature 621/class version 2 |
| Style 1 | Sketch5: unsupported loft-cap boundary pieces |
| Style 3 | Single-cap projection requires decoded boundary fillet provenance |
| Style 2 | Fillet4 rolling-ball geometry cannot regenerate |
| Style 4 | Unsupported fillet edge identity/selection mode |
| Style 7 | Unsupported body-recipe entry count |
| Gridfinity Storage Box Shelves | Missing object reference at byte 40 of timeline feature 16766 |
| R36S Grip | Unsupported timeline feature 1160/class version 4 |
| adapter-x20-makita | Component/assembly reconstruction unsupported |

The recovery comparison, per-check exit codes and timings are recorded in
`/private/tmp/f3d-07da-recovery-comparison.json` and
`/private/tmp/f3d-07da-validation-results.json`. The corpus command exits 101
because it deliberately runs the strict diagnostic alongside the passing recovery
gate; its final result is one passing recovery test and one failing strict test.

## Earlier integration evidence

Final focused validation on the code shared by `571122c4` and `3707a67c`:

- 28 prism tests pass, including short stations, cap provenance and filleted grips.
- Three cap-family tests pass; two opt-in native diagnostic tests remain ignored.
  The first filter selected zero tests; the corrected registered filter was run.
- 177 saved-history recipes and legacy offset pass (187.83 seconds).
- All 34 supplied recovery cases pass open/save/reopen (161.46 seconds).
- HardEdgeBasin opens and prepares display in **1.957 seconds**, below the
  20-second deadline, retaining 14 bodies and 14,898 triangles.
- Kernel/I/O all-target Clippy with warnings denied, workspace formatting and
  whitespace checks pass.

The final upstream commit changes only `docs/features.md`; unchanged code checks
were not repeated after that rebase. Evidence is in
`/private/tmp/f3d-571122c4-validation-results.json`,
`f3d-571122c4-recovery-comparison.json` and the corresponding `*-final.log` files.
Corrected cap coverage is in `f3d-571122c4-cap-families-corrected.log`.

The final upstream check found the collision optimization after the initial tests.
The second rebase had no conflicts. A content-hash comparison confirmed all 284
pending paths were restored unchanged. Preservation stash
`b690a03ea400dd3e0ae7f01a9919903d4b577515` is already applied; do not reapply it.

The subsequent rebase onto `571122c4` preserved all 285 pending paths. Nine files
combined upstream changes with the pending implementation; the others retained
their exact content hashes. Stash `9436f36ac535b75cbd7383b24af779514691ee73`
is also already applied. The temporary upstream comparison checkout was removed.

The full CI retry passed the I/O, document and saved-history suites, collision
equivalence, WebAssembly and the exclusive HardEdgeBasin opening regression
(2.00 seconds against a 20-second budget). It exposed a connected-fillet radius
regression and engine robustness failures. The UI agent discovery probe timed out
under load and passed a narrow rerun. The unfinished engine process was stopped;
the overall CI result is a failure, not a successful gate.

Connected earlier rounds now use their existing mitre reconstruction before the
new pinched-end rolling-ball path. All four issue1557 checks pass after this fix.
A separate shell/fillet/pattern comparison diverged first at shelling: clean
upstream gave 16045.974877 mm³ there, versus 16382.475817 mm³ before the fix.
The section splitter had discarded real short edges using the whole-body Boolean
tolerance. Its cleanup now uses only the sewing engine's minimum edge length
(1e-9 mm); a regression test preserves valid stations below the Boolean tolerance.
The complete shell/fillet/pattern check passes, including the unchanged final
16609.353673 mm³ volume assertion, topology validity and container reopening.
Neither prism direction ordering nor the added cap-history witness caused this
regression; those diagnostic changes were restored.

The seed-20 Boolean Cut failure also reproduces on a pristine `571122c4` CLI,
with the same command sequence and error. It is an upstream failure on this host,
not evidence of a regression introduced by F3D. The repository CI gate is still
not green; no expectations were changed to conceal the failure.
The final focused rerun of
`tests::robustness::a_boundary_found_across_a_seam_is_not_used` fails identically
after both branch-specific fixes (84.50 seconds; exit 101). Evidence:
`/private/tmp/f3d-3707a67c-seed20-final.log` and
`/private/tmp/f3d-seed20-upstream/results.jsonl`.
No further full-suite run was made merely to repeat this known failure. No commit,
fork push or PR was made because the repository requires the full CI gate before
committing; recovery success alone does not establish that gate.

## Complete legacy sketch membership — 2026-10-16

Style 1's version-17 sketches contain a second complete entity list, in a
different order, with every value equal to 1. A local controlled experiment
changed only those fixed-width values (one circle to 0 or 2, one dimension to 0,
and the entire table to 0). All five temporary imports retained identical sketch
geometry, construction/reference/link flags, dimensions, constraints, profile
counts, feature health and body volumes, both before and after changing a driving
diameter. Each copy was resaved locally and closed without saving the source
document. The temporary probe entry point was restored afterward.

This evidence supports the observed complete/default table, not arbitrary table
semantics. Reconstruction accepts only a one-to-one repetition of the sketch's
members with value 1. Duplicate, partial, foreign and unknown-state entries remain
errors. Original field-authored regression records exercise parameter edits,
analytic cylinder volume and saved-file replay; no native payload is committed.
Local evidence: `repro/f3d/legacy-sketch-metadata-controls/{patches,api-results,status}.json`.

All 27 selected legacy regressions pass (three opt-in native checks are ignored),
including the two new membership tests. The focused Style 1 retry passes both
recovery preferences and saved replay. Complete history now passes the metadata
check and stops at regeneration of `Fillet1`: a chain vertex has adjoining curved
faces unsupported by the kernel. This is progress in decoding, not another fully
editable archive. Its recovery counts remain four editable features/one body or
one native-body feature/one body, with explicit omission warnings. Evidence:
`/private/tmp/f3d-legacy-membership-{tests,native}.log`.

The final recovery recheck passes **34/34 cases** (161.38 seconds), retaining
the same feature/body counts, display volumes, representations and warning counts
as the preceding `3707a67c` integration. I/O all-target Clippy with warnings denied,
workspace formatting and whitespace checks pass. The final fetch still resolves
upstream to `3707a67c`. Full CI was not rerun to repeat its known upstream failure;
no commit, push or PR was made. Logs: `f3d-legacy-membership-recovery.log`,
`f3d-legacy-membership-recovery-comparison.json` and
`f3d-legacy-membership-clippy.log` under `/private/tmp`.

## Mesh collision integration — 2026-10-16

The new upstream collision candidate search does not change the import results:
all 34 recovery cases retain identical counts, display volumes, representations
and warning counts; strict history remains 8/17. The corpus run took 241.66 seconds.
All 177 original saved-history recipes and the legacy-offset case pass (335.06
seconds), and HardEdgeBasin's cold open plus display preparation passes in 6.114
seconds during the concurrent full-CI build (20-second regression budget).

Kernel/I/O Clippy and WebAssembly checks pass, as do full workspace formatting,
asset attribution, catalog ordering, layering and Clippy. The first full CI attempt
ran out of disk space while linking, before tests ran. Only reproducible build
artifacts in this isolated workspace were removed: `target/debug` and stale hashed
executables under `target/ci/deps` (about 18 GiB). CI is being retried with four
compiler jobs. The separate collision-equivalence build encountered the same disk
failure; its tests are included in the full CI retry.

Evidence: `/private/tmp/f3d-d29ca295-corpus.log`,
`f3d-d29ca295-recovery-comparison.json`, `f3d-d29ca295-history-performance.log`,
`f3d-d29ca295-clippy.log`, `f3d-d29ca295-wasm.log`,
`f3d-d29ca295-full-ci.log` and `f3d-d29ca295-full-ci-retry.log`.

## Typed feature integration and complete edge groups — 2026-10-16

Rebased onto upstream's feature-role types, sketch-pattern controls, intersection
curve parameterization and STEP cylinder changes. Conflicts were resolved while
retaining the F3D extent, grouped blend, loft and circular-pattern behavior.
Importer constructors, parameter traversal and test fixtures now use upstream's
expression, sketch, body, feature and position types. Grouped chamfer dialogs keep
editable strings locally and convert to document expressions through commands.
Upstream's generated license file is retained unchanged.

All 283 pending paths were preserved. Preservation stash
`790206f9c94270a0132e23894b6b1feeeac81378` is already applied; do not reapply it.

A complete finite-edge selection can now resolve symmetric references together:
every candidate chain must be selected, and the full native vertex identities must
map bijectively to reconstructed endpoints. The search is bounded and rejects
incomplete, duplicated, disconnected or overlapping selections. It does not use
cached geometry to guess which edge was intended.

At a limiting shoulder radius, the kernel now consumes a planar wall even when
its adjacent analytic cylinder is split at parameter seams. The original
`split-cylinder-consumed-wall` recipe checks independent analytic volume and
centroid, edits, suppression and reopening. Unit checks verify that genuine walls
remain below the limit and that an oversized round is refused.

Style 2's Fillet4 references now resolve. Tangent continuation previously added
smooth internal cylinder seams to the selected rim (16 edges instead of eight in
the local archive, 14 instead of eight in the original regression model). It now
rejects smooth seams as automatic continuations while retaining explicitly selected
seeds. The original symmetric-cap regression fails before this fix and passes after.

Fillet4 still fails in rolling-ball regeneration. Independent sections along the
selected edges solve, but the vertex section constrained to the cylinder seam does
not. The contacts cross the two support-surface seams at different stations; the
current algorithm requires a common section. No tolerance was loosened and no
surface boundary was discarded. The local opt-in complete-Fillet4 acceptance test
still fails, so this is not a successful Fillet4 import.

Validation on the final base:

- All 17 supplied designs open, save and reopen with geometry under both recovery
  preferences (34 cases). Recovery remains explicitly partial where necessary.
- Strict import still passes eight designs: Minimal, Crescent, Eye, Narrow,
  Round, Square, Style 5 and Style 6. The combined corpus test exits 101 because
  the strict gate fails; its separate recovery test passes.
- Recovery counts, representations and warnings match the previous base in all
  34 cases. Only Style 2's editability-mode mesh volume changes, from 31,044.191
  to 26,984.874 mm³ following the consumed-wall fix. Its five retained features
  still omit later operations. Correctness-mode native recovery is unchanged.
- All 327 I/O unit tests pass; 38 local opt-in tests are excluded.
- All 177 original saved-history recipes and the legacy-offset case pass replay,
  analytic geometry checks, parameter edits, undo and reopening (379.26 seconds
  while the corpus/build checks were also running).
- Both grouped fillet/chamfer dialog regressions pass, preserving selections,
  independent parameters and tangent-chain choices.
- HardEdgeBasin opens and prepares its display in **2.007 seconds**, with 14 bodies
  and 14,898 triangles. Source preservation, cached refresh and known-error checks pass.
- Workspace all-target Clippy with warnings denied, the I/O WebAssembly build,
  formatting, catalog ordering, asset attribution and dependency layering pass.

| Archive | First strict-import failure |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class `584D9526` v2, feature 621 |
| Style 2 | Fillet4 (3371): rolling-ball section fails at a support-surface seam |
| Style 3 | Sketch6 (3389): unsupported projected body selection |
| Style 4 | Chamfer3 (4386): unsupported joined-blend selection mode |
| Style 7 | C-Pattern2 (23341): unsupported body recipe entry count |
| style-1 | Sketch7 (3915): unsupported legacy sketch entity metadata |
| Gridfinity | Chamfer 16766: missing selection reference at byte 40 |
| R36S Grip | Unsupported timeline class `D7015B81` v4, feature 1160 |
| adapter-x20-makita | Component/assembly reconstruction is unsupported |

Gridfinity still reports open shells. Style 5's source-volume discrepancy remains
unresolved; its fine inspection and coarse display measurements each match their
previous values. Neither recovery nor the strict gate proves complete source fidelity.
Full workspace CI has not been rerun and its previous failures are not cleared.
No new feature commit or PR was created.

Follow-up validation after the tangent-seam fix:

- All three kernel tangent-chain tests, three original cap-family regressions,
  and four engine tangent-history tests pass. The latter cover edits, patterns,
  independent groups, save/reopen and undo.
- Style 2's first fillet still passes its local import, radius-edit and reopening
  test. The second-fillet acceptance test still fails at regeneration, after
  confirming all eight exact named edge selections and tangent continuations.
- The full corpus was rerun: 34/34 recovery cases pass, and the strict gate remains
  8/17 (201.65 seconds combined). Every recovery result has the same feature count,
  body count, displayed volume, representation and warning count as the preceding
  rebase run. The changed Style 2 warning identifies the rolling-ball failure.
- Kernel/I/O all-target Clippy with warnings denied, I/O WebAssembly, formatting
  and whitespace checks pass. Temporary geometry probes and debug output were
  removed. The earlier full-workspace CI limitation still applies.

Follow-up evidence: `/private/tmp/f3d-seam-corpus.log`,
`f3d-seam-recovery-comparison.json`, `f3d-tangent-seam-kernel2.log`,
`f3d-seam-cap-tests-final.log`, `f3d-seam-engine-tests.log`,
`f3d-seam-native-first.log`, `f3d-seam-native-second.log`,
`f3d-seam-clippy.log`, and `f3d-seam-wasm.log`.

Evidence: `/private/tmp/f3d-5e9285ca-corpus.log`,
`f3d-5e9285ca-recovery-comparison.json`, `f3d-5e9285ca-history-performance.log`,
`f3d-5e9285ca-io3.log`, `f3d-5e9285ca-ui-groups.json`,
`f3d-5e9285ca-clippy3.log`, and `f3d-5e9285ca-wasm.log`.
The sections below record earlier checkpoints.

## Split caps and composite cylinder families — 2026-10-16

Rebased without conflicts onto `716107f0`, then onto `2e5b7b11` after the final
upstream check found more merges. All 280 pending files were preserved. Only
`ROADMAP.md` and the Japanese/Chinese catalogs changed during the final integration,
retaining upstream's additions. Preservation stashes
`7ac5629ef96c174ae8845ebd714db7ca0ce1d964` and
`70c695abaf91c2b286b02614c42c6b0624049aa3` are already applied; do not reapply them.

Style 2's first shoulder fillet (Fillet3) now imports, regenerates, saves,
reopens and responds to a radius edit. Its cap is split into two faces by the
upper grip. A cap family can participate in edge incidence while face attachment
still requires one unambiguous face. The side profile can contain two distinct
arcs of the same circle, each with its own partition and endpoint-neighbor
certificate. A composite cylindrical face accepts only explicit producer
families whose solved circles and reconstructed surfaces share the same carrier.

Exact source-family ownership now survives ambiguous piece numbering in the
transient naming cache. It does not certify individual numbered pieces, add
geometric guesses, or change saved format. Whole-family resolution rejects a
candidate family if any of its pieces lacks a source certificate. Finite edges
may have identical logical face sets at both endpoints, but must still have
distinct complete native vertex identities and form one connected chain.

The original `split-cylinder-cap-fillet` example adds independent volume and
centroid expectations, parameter edits and suppression checks. The I/O test
compares rounded and unrounded volumes at identical chord tolerance, so the
small added material cannot be hidden by a change in cylinder tessellation.
Four focused tests pass, including the local Style 2 import/edit check and
synthetic repeated-arc and split-cap cases. Native archives remain local.

Validation on `716107f0`:

- 325 I/O tests pass; 37 local opt-in tests are excluded.
- Eleven naming tests and all 176 original saved-history recipes plus the legacy
  offset case pass. The new fixture includes analytic volume and centroid checks.
- All 34 recovery cases pass. Style 2 retains five editable features (previously
  four), including Fillet3; its next omission is Fillet4 (3371), where the named
  faces identify more than one finite edge chain. `style-1` reaches the same
  finite-chain limitation at Fillet1 (597). Other first failures are unchanged.
- Complete-history import still passes 8/17; the strict test exits 101.
- HardEdgeBasin opens and prepares its display in 2.156 seconds, with 14 bodies
  and 14,898 triangles, passing the source, cache and known-error checks.
- Workspace all-target Clippy, the I/O WebAssembly build, formatting, asset
  attribution and whitespace checks pass.
- The new original fixture was rendered and visually inspected: both concave
  shoulders are present and join the grip to the circular base. Local image:
  `/private/tmp/f3d-split-cylinder-cap.png`.

Final validation on `2e5b7b11`:

- Four focused native/synthetic importer tests, 58 joint/clipboard engine tests,
  42 update/chat/joint-drag UI tests and 20 localization tests pass.
- Repeating all supplied imports produces exactly the same 34 recovery records
  (feature/body counts, volume, representation and warning counts) and eight
  strict successes as `716107f0`. The strict test still exits 101.
- HardEdgeBasin opens and prepares its display in **2.157 seconds**, retaining
  14 bodies and 14,898 triangles and passing all cache/source/error assertions.
- Workspace all-target Clippy with warnings denied, the I/O WebAssembly build,
  catalog/module ordering, formatting and whitespace checks pass.
- The final upstream merge does not change the original recipes' operation paths;
  their complete 176-case suite was not repeated. The four focused importer tests
  do cover the final finite-value certificate guard.

Current first strict-import failures:

| Archive | First failure |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class `584D9526` v2, feature 621 |
| Style 2 | Fillet4 (3371): named faces select more than one finite edge chain |
| Style 3 | Sketch6 (3389): unsupported projected body selection |
| Style 4 | Chamfer3 (4386): unsupported joined-blend selection mode |
| Style 7 | C-Pattern2 (23341): unsupported body recipe entry count |
| style-1 | Fillet1 (597): named faces select more than one finite edge chain |
| Gridfinity | Chamfer 16766: missing selection reference at byte 40 |
| R36S Grip | Unsupported timeline class `D7015B81` v4, feature 1160 |
| adapter-x20-makita | Component/assembly reconstruction is unsupported |

Recovery remains partial: Gridfinity reports open shells, and Style 5's source
volume discrepancy is unresolved. Passing recovery is not a complete-fidelity claim.

Validation evidence for this increment is recorded in
`/private/tmp/f3d-716107f0-*.log`, `f3d-716107f0-focused.json` and the
`results.json` file. The final integration uses `/private/tmp/f3d-2e5b7b11-*.log`.
Full workspace CI has not been rerun; its prior failures are not cleared. No
new feature commit or PR was created.

## Face-selection records and upstream recheck — 2026-10-16

Rebased onto `904bf19e`, then `987f0be2`, and finally `ce96e625` as upstream
advanced during validation. All pending work was restored without merge conflicts.
The final preservation stash `924685cbae8058ec35725785bfdc7c76faa227ba` is already
applied; do not reapply it. The preceding `325f0ca115c35188028ce0eaec58e12e633f971e`
and `4c16a9cb53c7a03b1117cb10f1907149c244c87e` stashes are also already applied.

Integration retains upstream's input-relative fillet validation, including the
disconnected-round fallback. A stale `no_pixels` assignment prevented UI test
compilation on `904bf19e`; removing it restored compilation and the edge-picking
test passed. Upstream subsequently merged the identical fix in `5cfcf7fd`, so
the final branch carries that correction from upstream.

Fillet group records now distinguish optional edge and face selection containers.
This decodes Style 2's mixed Fillet7 inputs and the adapter's face-only fillet,
including its intentional null edge slot. Synthetic tests cover both helper
versions, malformed flags, truncation, wrong classes and container mismatches.
Local controls verify eight archives, the original and re-saved Style 2, and the
adapter record. Those native archives remain outside the repository.

Live face-boundary regeneration is still unfinished. Import explicitly reports
that limitation instead of dropping face inputs. Removing the old metadata
failures exposes earlier reconstruction blockers: Style 2's Fillet3 selection
and the adapter's component/assembly guard. Recovery counts and volumes are
unchanged by this increment.

Validation:

- On `904bf19e`, all 323 I/O unit tests pass, with 36 local opt-in tests excluded;
  both targeted native face-selection tests pass. Two upstream kernel tests and
  seven connected-fillet, loft/join and mirror engine regressions pass.
- On `987f0be2`, all 175 original saved-history recipes and the legacy-offset
  case pass replay, analytic checks, parameter edits, undo and reopening.
- Both preferences open, save and reopen all 17 supplied archives with geometry:
  34/34 recovery cases pass. Feature/body counts, measured volumes and warning
  counts match the preceding `63accec1` and `904bf19e` runs.
- Complete-history import passes 8/17: Minimal, Crescent, Eye, Narrow, Round,
  Square, Style 5 and Style 6. The strict test exits 101 for the nine failures below.
- HardEdgeBasin opens and displays in **2.114 seconds**, with 14 bodies and 14,898
  triangles. The source-preservation, warm-cache and known-error assertions pass.
- Seven upstream UI tests pass, covering infinite-line display, frame-rate
  accounting and edge picking. The initial construction-line engine filter
  matched zero tests; the corrected `infinite_tests` filter subsequently ran and
  passed all five cases on `ce96e625`.
- The final `ce96e625` surface-patch integration passes 12 geometry tests, 18
  kernel fill/bridge tests, 37 format/migration tests and three patch-dialog tests.
  Its changes do not alter F3D reconstruction or any of the 175 history recipes'
  operation paths; those complete suites were not repeated for this integration.
- Workspace all-target Clippy with warnings denied and the I/O WebAssembly build
  pass on `ce96e625`. Formatting, asset attribution and whitespace checks pass.

| Archive | First complete-history failure |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class `584D9526` v2, feature 621 |
| Style 2 | Fillet3 (3342): unresolved edge ownership; composite cap contributors do not name one face |
| Style 3 | Sketch6 (3389): unsupported projected body selection |
| Style 4 | Chamfer3 (4386): unsupported joined-blend selection mode |
| Style 7 | C-Pattern2 (23341): unsupported body recipe entry count |
| style-1 | Fillet1 (597): ambiguous blend boundary carrier |
| Gridfinity | Chamfer 16766: missing selection reference at byte 40 |
| R36S Grip | Unsupported timeline class `D7015B81` v4, feature 1160 |
| adapter-x20-makita | Component/assembly reconstruction is unsupported |

Recovery is not a full source-fidelity check. Gridfinity retains six bodies with
open-shell warnings; Style 5's unresolved volume discrepancy remains documented
below. Full workspace CI was not rerun, and its previous failures have not been
cleared. No new feature commit or PR was created; the remaining work is preserved
in the isolated workspace.

Evidence: `/private/tmp/f3d-face-input-*.log`,
`/private/tmp/f3d-987f0be2-*.log`, `/private/tmp/f3d-ce96e625-*.log`, their
`results.json` files, and `/private/tmp/f3d-ce96e625-infinite-engine.json`.
The sections below record earlier checkpoints, not the current failure list.

## Live chamfer projections and final upstream integration — 2026-10-16

Rebased first onto `b562ab51`, then onto `63accec1` after upstream advanced during
validation. Both rebases completed without conflicts. The final rebase preserved
all 274 pending files; only `crates/kernel/src/ops.rs` changed during integration,
retaining upstream's void-cut fallback alongside the F3D cut-cap lineage. The
final preservation stash `cb58dc8a2843e85d04155bf0ddcf1d8497189651` is already applied;
do not reapply it. The preceding `157070e9c4e2218bc731eb84fc6a4a1e05a508db` stash is
also already applied. Upstream's catalog ordering is retained, and the generated
command documentation is left to its main-branch workflow.

Style 6 now reconstructs all eight features, including its final fillet, without
recovery warnings. Its two chamfer-cap projections follow explicit output-label
and input trackers rather than ordinal labels or geometric repicking. A cut's
nominal cap keeps its producer identity and reverses its normal on the retained
solid. Synthetic tests reject mismatched trackers, swapped ends, truncated data,
unselected edges and wrong cut ownership, and verify live source edits and reopening.

Two new original saved designs, `paired-chamfer-projection-top.solidcraft` and
`paired-chamfer-projection-bottom.solidcraft`, exercise both ends of a beveled
cylinder. Analytic cylinder/frustum volume, first moments, bounds, parameter
edits, suppression, undo and reopening pass. The lower-end example was rendered
and visually checked; its image is local evidence, not a committed asset:
`/Users/chess/projects/cad/repro/f3d/paired-chamfer-projection-bottom-solidcraft.png`.

Final validation on `63accec1`:

- 321 I/O unit tests pass; 34 local-native opt-in tests are excluded from that count.
- All 175 original saved-history recipes and the legacy-offset case pass.
- 34/34 supplied-archive recovery cases pass: both preferences open, save and
  reopen all 17 files with geometry. Their feature/body counts, reported volumes
  and warning counts match the preceding `b562ab51` run.
- Complete-history import passes 8/17: Minimal, Crescent, Eye, Narrow, Round,
  Square, Style 5 and Style 6. The other nine first failures are listed below.
- HardEdgeBasin opens and displays in 2.015 seconds, with 14 bodies and 14,898
  triangles. Source-preservation, cache and known-feature-error assertions pass.
- Three new upstream kernel tests, four shell/STEP engine regressions, and nine
  discovery, dropdown and edge-picking UI tests pass.
- Workspace all-target Clippy with warnings denied, the I/O WebAssembly build,
  formatting, asset attribution and whitespace checks pass.

| Archive | First complete-history failure |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class `584D9526` v2, feature 621 |
| Style 2 | Unsupported history flags, feature 3963 at byte 38 |
| Style 3 | Unsupported projected composite-cap selection |
| Style 4 | Unsupported joined-blend selection mode |
| Style 7 | Unsupported body-recipe entry count |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Missing chamfer reference, feature 16766 at byte 40 |
| R36S Grip | Unsupported timeline class `D7015B81` v4, feature 1160 |
| adapter-x20-makita | Missing fillet reference, feature 18835 at byte 27 |

The corpus command exits 101 because the complete-history test fails; its separate
recovery test passes. Recovery does not establish source fidelity: Gridfinity
still has six bodies with open-shell warnings, and Style 5's overall shape
discrepancy remains unresolved. Full workspace CI was not repeated; its earlier
robustness failures are documented below, so a green full-CI gate is not claimed.
No follow-up commit, push or PR was made.

Final logs: `/private/tmp/f3d-63accec1-{upstream-kernel,upstream-engine,io,history,corpus,performance,upstream-ui,clippy,wasm,fmt,assets,whitespace}.log`.
Stage results: `/private/tmp/f3d-63accec1-results.json`.
The earlier `b562ab51` run passed I/O, saved histories, recovery and performance;
its remaining build checks were interrupted before integrating the new kernel fixes.

## Face-join and measurement integration — 2026-10-16

Rebased onto `34230570`, including face-attached join scoping, circle/arc-to-edge
measurement heights, rectangle constraints through sketch fillets, and Grok agent
discovery. The only rebase conflict was the extrusion command's parameter text;
it now describes both upstream's sketch-face join target and F3D's plane target.
All 269 pending files were restored. Of those, only `ROADMAP.md` and the extrusion
command changed during integration; the other 267 are byte-identical. Preservation
stash `ac356f47aab637adbea39e4355aa12a0c226951e` is already applied; do not reapply it.

Before this final rebase, the format migration passed 261 document tests (one
ignored), all 173 original saved-history recipes plus the legacy-offset case,
316 I/O tests (33 opt-in tests ignored), workspace all-target Clippy with warnings
denied, and the pending-format merge/fold integration script. The thirteen local
F3D files extracted from the user's ZIP packs match their ZIP members by SHA-256;
with four direct archives, the acceptance corpus contains seventeen files.

Post-rebase saved-history validation passes all 173 recipes plus the legacy-offset
case. The 20 measurement tests and three new face-join/rectangle-corner regressions
pass. Strict supplied-file import remains **7/17**, including Style 5's fourteen
features. Both preference modes open, save and reopen every supplied archive:
**34/34 recovery checks pass**, preserving recovered geometry and saved history.
Gridfinity retains six bodies with explicit open-shell warnings; this does not
establish watertight solids or final-source fidelity. Style 5's complete history
still needs independent final-shape verification as explained below.

HardEdgeBasin passes at **2.057 seconds** for opening plus display, with 14 bodies
and 14,898 triangles. Its source-preservation, warm-cache and known-feature-error
assertions pass within the 20-second deadline. Workspace all-target Clippy with
warnings denied, formatting, whitespace, and the I/O WebAssembly target pass.
The full CI suite was not repeated: the prior robustness failures remain open,
and the supplied-file complete-history acceptance test still exits 101.
A final remote check confirms upstream is still `34230570`.

| Archive | First complete-history failure after rebase |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class `584D9526` v2, feature 621 |
| Style 2 | Unsupported history flags, feature 3963 at byte 38 |
| Style 3 | Projected composite cap requires multiple contributors |
| Style 4 | Unsupported joined-blend selection mode |
| Style 6 | Unsupported live-chamfer cap projection |
| Style 7 | Unsupported body-recipe entry count |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Missing chamfer reference, feature 16766 at byte 40 |
| R36S Grip | Unsupported timeline class `D7015B81` v4, feature 1160 |
| adapter-x20-makita | Missing fillet reference, feature 18835 at byte 27 |

Logs and process results: `/private/tmp/f3d-34230570-{engine,join-corners,history,corpus,performance,clippy,wasm}.log`
and `/private/tmp/f3d-34230570-results.json`. The strict acceptance failure is not
waived by partial recovery. No PR has been opened.

## Pending-format integration — 2026-10-16

Rebased onto `9a07932f`, retaining the new Windows agent-discovery fix as well.
All 212 pending files were restored; only the document schema, format reader,
module list and container documentation changed during integration. The applied
stash is `bcaebd2307da848512d4bbc82f7f91e1ce7c705b`; do not reapply it.

The F3D extension now has a pending change file. The generated format number and
upstream's hand-written migration list are unchanged from upstream. Explicit
`solidcraft-f3d-dev/13` through `/30` files have a separate compatibility reader:
it preserves their naming/piece-order and omitted-loft interpolation semantics,
then upgrades them into the current pending format. Plain upstream numbers never
invoke these development rules. Unknown future versions remain errors.

100 known authored `.solidcraft` test fixtures received explicit development tags;
a JSON comparison verified that no other field changed. No user design was
rewritten. All tagged versions, ordinary upstream defaults and future-version
refusal are covered by compatibility tests. Broader validation passed as recorded
in the subsequent integration section above.

## Retired circular-cap history — 2026-10-16

Style 5 now imports all fourteen features through strict reconstruction, without
recovery warnings. Selector 305's bounded face names live operation 301 and
explicitly retired operation 303. Cached tracker 3345 and mapped tracker 494
independently carry identical current origin-edge identities. The decoder requires
that full incidence, retirement and producer ownership agree before attaching a
live named edge. It then checks the regenerated circle against the target sketch.

- **7/17 strict imports pass**, including Style 5 for the first time. The ten other
  first failures remain unchanged. The strict corpus process still exits 101.
- **316 I/O unit tests pass**, 33 opt-in tests ignored. Three new original tests
  cover circle/dependent-extrusion edits and reopening, record/identity mutations,
  truncation, explicit retirement and target-geometry mismatch.
- Style 5 saves/reopens with one body and 6208.832363 mm³ measured volume. Its
  final 10 x 10 x 7 mm pocket removes 700.002855 mm³. After reopening, changing
  InsertDepth to 5 mm restores 200.000000 mm³. The opt-in native analytic test passes.
- I/O all-target Clippy and the I/O WebAssembly target pass. Full workspace CI
  was not repeated; the earlier robustness failures still prevent a green gate.

**Fidelity remains unverified:** native-body recovery reports about 6868.534 mm³
for Style 5 and lists conversion losses, including procedural surfaces/curves and
normalized signed analytic surfaces. Neither recovery volume nor successful
history regeneration establishes the complete source shape. The analytic pocket
checks narrow the discrepancy; comparison of the rest of the shape remains open.
Fusion was inspected read-only but was showing a simulation panel; no document
or simulation was changed.

Evidence: `/private/tmp/f3d-retired-circle-{io,native,strict-corpus,clippy,wasm}.log`,
`/private/tmp/f3d-retired-circle-native-analytic.log`,
`/private/tmp/f3d-retired-circle-results.json`, and
`/private/tmp/f3d-style5-cut-results.json`. Local exported documents:
`/private/tmp/f3d-style5-retired-circle.solidcraft` and
`/private/tmp/f3d-style5-native-bodies.solidcraft`. They are not committed.

Upstream advanced to `9a07932f` during this increment. Its format generator
replaces manual version increments with pending change files, so the F3D version
30 extensions and existing development fixtures need deliberate integration.
At that checkpoint it was fetched but not yet rebased; the completed integration
is recorded above. No PR or feature follow-up commit was made.

## Final CI-only upstream update — 2026-10-16

Rebased once more onto `af771b9c`, which changes only CI package selection,
its Python self-tests and contributor documentation. The importer, document,
geometry and UI implementations are unchanged from the `4cbf9cf6` validation
below. All 210 pending files were restored byte-for-byte. The applied stash is
`d1980f16bed796810d556656a4f9a5e464e8978d`; do not reapply it.
The upstream flange UI regression and all eight CI package-selection self-tests
also pass. No corpus rerun is needed for this CI-only update. The publication
decision below remains unchanged. CI self-test log:
`/private/tmp/f3d-af771b9c-ci-selection.log`.

## Edge-flange relief integration — 2026-10-16

Rebased onto `4cbf9cf6` (per-feature edge-flange corner relief). Resolved the
format and container-documentation conflicts by preserving upstream's version-24
semantics alongside the F3D extensions written as version 30. All 210 pending
files were retained. The four changed pending files received upstream changes:
Japanese and Chinese locales, the document feature schema and format migrations.
The applied preservation stash is `8c0288315d36ccde529dfcf22e462258052dcd7b`;
do not reapply it. All 31 document-format tests pass, including upstream's new
relief save/reopen check and the F3D development-format compatibility checks.

The 13 native archives extracted from the two user ZIP packs were verified by
SHA-256 against their ZIP members. Together with the four direct F3D files,
the acceptance run covers all 17 supplied archives. Complete-history decoding
still passes six files; the eleven first failures remain unchanged.

- **34/34 recovery checks pass**: all 17 archives open, save and reopen in both
  preference modes with geometry retained and unchanged saved history/mesh volume.
  Style 5 retains twelve editable features and two omissions. Gridfinity retains
  six bodies with explicit incomplete-shell warnings; source fidelity and solid
  closure are not established by this recovery check.
- **HardEdgeBasin: 2.267 s** for opening and display, 14 bodies and 14,898
  triangles. Source preservation, warm-cache reuse, known feature limitations
  and the 20-second deadline all pass.
- All 31 format tests and ten feature-walking tests pass. Five corner-relief
  engine tests, two per-feature relief-inheritance tests and the flange UI test pass.

The corpus process exits 101 only because its separate complete-history gate
fails for eleven archives. The recovery gate passes. No new import failure was
observed from this rebase. Full workspace CI was not repeated: the prior run's
engine robustness failures remain, with upstream comparisons documented below.
No feature follow-up commit, push or PR has been made; the requested condition
that all checks succeed is not met.

Logs: `/private/tmp/f3d-4cbf9cf6-{format,feature-walk,corpus,performance,relief,flange,ui-relief}.log`;
command exit codes and durations: `/private/tmp/f3d-4cbf9cf6-results.json`.

## Pristine upstream comparison — 2026-10-16

The golden failures alone do not establish regressions introduced by this branch.
A separate archive of `c3616309` was built in
`/private/tmp/solidcraft-f3d-upstream-c3616309`. Git blob hashes verify that all
874 checked crate, vendor and Cargo files match upstream except the test-only
helper used to run individual seeds. No production code was changed there.

- **Seed 20 fails identically on pristine upstream:** same failed Cut, 20 faces,
  48 edges, 18198.527948 mm³ volume and 5253.768421 mm² area. The failure is not
  introduced by the F3D work.
- **Seed 71 differs from its golden identically on pristine upstream:** one
  body, 19 faces, 50 edges, 15749.928226 mm³ volume and 3980.574957 mm² area.
- **Seed 53 also differs from its golden on pristine upstream**, but it selects
  different edges for the first fillet after shelling. Upstream chooses
  `[-2.602280410795756, 7.250851179380608, 35]`, `[25, 20, 17.5]` and
  `[32.5, 37.5, 35]`; the feature build chooses three other edges. The generator
  samples the current edge array by index, so these are different command
  sequences. The new permanent regression
  `tests::robustness::shell_fillet_pattern_replays_fixed_selections` replays
  the successful upstream commands on this branch. It passes in 3.40 s,
  preserving one body, the 16609.353673 mm³ reference volume within 0.02 mm³,
  valid topology, and the same volume within 0.000001 mm³ after serialization
  and regeneration. This successful operation is not regressed; the seeded
  generator's different selections still need separate golden-test handling.

Neither preserving the old prism direction order nor restricting the additional
union regularization changed the quick failing cases. Restoring the old curved
blend implementation and the old station-splitting rule also reproduced the
three golden failures. All those diagnostic production edits were reverted;
the original F3D geometry implementation remains intact. Golden expectations
and tolerances have not been changed.

Evidence: `/private/tmp/f3d-pristine-upstream-verification.json`,
`/private/tmp/f3d-pristine-upstream-{20,53,71}.log`, and
`/private/tmp/f3d-pristine-upstream-53-trace.log`. The fixed-selection result is
`/private/tmp/f3d-fixed-shell-fillet-pattern-final.log`. Do not reuse the feature
target directory for future pristine checkouts: Cargo reused incompatible local
crate artifacts afterward. The affected feature crates were invalidated and
rebuilt before the passing fixed-selection run.

Native investigation also confirms that Style 5 Sketch2 (feature 289) has one
mapping, tracker 494 to circle 500, in mapping record 309. Selector 305 refers
to identity 308, with operation 301 owned by feature 216 and operation 303 explicitly
retired in the operation index. Its remaining source relationship is unresolved. This is a single-circle projection, not the multi-span composite
outline supported by `composite_projections.rs`; lowering its minimum count
would not establish the missing source identity. The raw mapping is saved in
`/private/tmp/f3d-style5-sketch2-mapping.json`.

## Final recheck and publication decision — 2026-10-16

The final rebase incorporates upstream's DXF view export (`c3616309`) without
conflicts. All 209 pending files remain present; only ROADMAP, the Japanese and
Chinese locales, the UI dialog dispatcher and the generated command list changed
during integration. The applied preservation stash is
`d8e5ba28d503a214e4d45a167584cd825cf12bc2`; do not reapply it. This upstream
commit does not change F3D import, document evaluation or the geometry kernel.
Its focused integration check passes all six DXF engine tests and the DXF UI
test; the I/O package compiled but this filter matched no I/O tests. Formatting
and `git diff --check` pass. The merged command list contains 378 implemented
commands and three planned commands. Log: `/private/tmp/f3d-c3616309-dxf.log`.

The latest complete corpus run, on `845f4913`, again passes **34/34 recovery
checks**: all 17 supplied archives open, save and reopen in both preferences.
Complete history remains supported for six archives. Style 5 retains twelve
editable features, omits Sketch2 and its dependent Extrude2, and preserves
6892.271 mm³ mesh volume after reopening. Gridfinity's six recovered bodies
still carry incomplete-shell warnings. Recovery is not proof of final-source
shape fidelity.

The same integration passes 312 I/O tests, nine file-loading UI tests, two native
CAD import engine tests, fifteen texture-sampling tests, workspace Clippy and all
eleven WebAssembly targets. HardEdgeBasin opens and prepares its display in
**3.907 s**, with 14 bodies and 14,898 triangles, within its 20-second regression
deadline. These timings were collected under concurrent load.

Full workspace CI is **not green**, so no PR has been opened. The engine suite
completed with 1363 passing tests, five failures and six ignored tests:

- Seed 20: a previously successful cut after a fillet now returns
  `boolean Cut: the operation failed: no result`. Both the focused seam test
  and its golden test fail.
- Seed 71: topology changes from 20 faces/52 edges to 19 faces/50 edges.
- Seed 53: a fillet becomes unsupported; later history changes, and the result
  changes from one body to four. The `all_1` shard took roughly forty minutes.
- The fillet beside a modelled thread took 53.293 s and exceeded its deadline
  during the full run. It passes when rerun alone in 4.13 s, so the full-run
  timing failure was not reproduced in isolation.

The focused seam test was stopped after a 180-second diagnostic timeout; its
original full-suite failure remains unresolved. The timeout's test child was
explicitly terminated. The isolated thread rerun invokes the already-built
test binary directly, avoiding build-lock time in the timing measurement.

These are unresolved geometry/robustness failures, distinct from the intentional
partial-history omissions. No golden expectations or tolerances were relaxed.
Logs: `/private/tmp/f3d-2b45635d-ci.log`,
`/private/tmp/f3d-845f4913-results.json`, and
`/private/tmp/f3d-845f4913-isolated-failures.json`.
The isolated thread result is in
`/private/tmp/f3d-845f4913-isolated-thread.json`.

## FreeCAD/Rhino and texture-sampling integration — 2026-10-16

Upstream advanced during validation. Rebased onto `845f4913`, retaining the
FreeCAD/Rhino and F3D import paths, their file-picker handling and command options.
Resolved dependency and notice conflicts by combining the native-only codecs
at cadmpeg 0.6.1 and regenerating the notices. The lockfile adds only upstream's
FreeCAD, Rhino, registry and SHA-1 packages; no existing package was removed or
changed to a different version. All 209 pending files were preserved; seven
received upstream integration changes. The already-applied recovery stash is
`c523e2194a9214dc0745377e6fa5a887ecfadff4`; do not reapply it.

The 0.13.1 full-workspace run passes all 632 kernel tests, all 173 original
editable-history recipes and their legacy compatibility case, 944 UI unit tests,
111 UI scenarios, 213 sketch tests, 305 I/O tests and 254 document tests. All
eleven WebAssembly targets compile, workspace Clippy passes, and formatting,
assets, licences and layering pass. The six socket-dependent test failures
were sandbox denials: the desktop control-server check, both CLI connection
tests and all 43 MCP tests pass when rerun with local socket access. The
web-offline check passes on retry after compilation and the UI suites finish;
its first attempt failed with a DevTools timeout under load.

The completed engine robustness results and latest integration checks are
summarized above. No new feature commit, push or PR has been made yet.

## Upstream 0.13.1 recheck — 2026-10-16

Rebased onto upstream 0.13.1. Resolved the document-format conflicts by retaining
upstream's version-23 motion-link semantics and the branch's version-30 F3D
extensions. Restored all 209 pending files; SHA-256 comparison found no missing
files and only eight files changed by upstream integration. The command-list
count conflict was reconciled against the built registry: 377 implemented and
three planned commands, with no missing or extra IDs. The preservation stash
`ec80e5b5482070ca1bd8e8e2b30439dac9125366` has already been applied; do not reapply it.

- **34/34 recovery checks pass:** all 17 archives open, save and reopen with
  geometry under both preferences. Gridfinity retains six bodies and explicit
  incomplete-shell warnings. These checks do not establish final-source fidelity.
- **6/17 complete editable imports pass:** Minimal, Crescent, Eye, Narrow,
  Round and Square. Strict acceptance still fails for the other eleven files.
- **Style 5 advances past Fillet6:** recovery retains twelve editable features
  instead of eleven, with two omissions instead of three. Opening took 23.28 s;
  save/reopen preserves the history and measured mesh volume of 6892.271 mm³.
  Sketch2's composite projected boundary and dependent Extrude2 remain unsupported.
- **HardEdgeBasin opens and prepares display in 2.124 s**, with 14 bodies and
  14,898 triangles. Source preservation, warm-cache reuse, the five pinned feature
  limitations and the 20-second deadline pass.
- Kernel, I/O, CLI and UI **all-target Clippy passes** with warnings denied.
- **305 I/O tests pass, 32 ignored** (42.02 s), including the formerly failing
  0.4 mm perimeter-volume check and the additional sections near curved junctions.
- **254 document tests pass, one ignored** (29.73 s), including document-format
  compatibility and the new upstream coupling tests.

The first remaining strict failures are:

| File | First failing area |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class/version, feature 621 |
| Style 2 | Unsupported history flags, feature 3963 at byte 38 |
| Style 3 | Projected composite cap requires multiple contributors |
| Style 4 | Unsupported joined blend selection mode |
| Style 5 | Sketch2: unsupported composite cap mapping count |
| Style 6 | Unsupported composite cap mapping count |
| Style 7 | Unsupported body recipe entry count |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Missing chamfer reference, feature 16766 at byte 40 |
| R36S Grip | Unsupported timeline class/version, feature 1160 |
| adapter-x20-makita | Missing fillet reference, feature 18835 at byte 27 |

The narrow-strip meshing repair cuts each supporting surface to its owned
contact interval. Resampling uses the computed ball centre, planar strips omit
curved-neighbour interpolation, and pinched endpoints validate against bounded
construction accuracy. The original 0.4 mm perimeter example was regenerated
and rendered: its cylinder, shoulder, thin wing and rounded rim remain intact.
The local image is `repro/f3d/compound-kernel/thin-rounded-wing-perimeter-04.png`
in the main CAD workspace; generated media is not committed.

Corpus execution took 337.57 s, excluding the 0.13.1 rebuild. Logs and exit codes
use `/private/tmp/f3d-2b45635d-`: `corpus.log`, `performance.log`, `io.log`,
`blends.log`, `document.log`, `clippy.log`, and `results.json`.
Complete-history acceptance remains red. The requested recovery feature allows
unsupported features to be omitted with warnings, so these strict failures are
documented limitations rather than its acceptance gate. Full workspace CI is
being checked separately before publication.

## Upstream rebase and opening checks — 2026-10-16

Rebased without conflicts first onto `73493b94` (timeline-edit validation,
perspective texture interpolation and circle/arc centre measurements), then onto
`6128a20c`, the Windows agent-chat fix that landed during validation. All 209
pending files were preserved byte-for-byte through each rebase, verified against
SHA-256 manifests. Recovery stashes are retained and have already been applied.
Temporary local kernel diagnostics were removed; the permanent regression remains.

The final upstream addition touches only `agent_chat/discover.rs` and
`agent_chat/process.rs`. The importer, geometry code and tests are identical to
the versions exercised in the following run, so that corpus was not repeated.

- **34/34 recovery checks pass:** all 17 archives open, save and reopen in both
  preference modes, retaining geometry and stable saved history/mesh volume.
  Gridfinity retains six bodies with explicit incomplete-shell warnings; this
  does not establish watertight solids or fidelity to the source's final shape.
- **6/17 complete editable-history imports pass:** Minimal, Crescent, Eye,
  Narrow, Round and Square. The strict acceptance test remains red.
- **HardEdgeBasin opens and prepares its display in 2.747 s**, retaining 14
  bodies and 14,898 triangles. The source-preservation, warm-cache and five
  known feature-error assertions pass within the 20-second deadline.
- **I/O: 304 passed, one failed, 32 ignored** (50.00 s). The failing original
  fixture is `pinched_perimeter_preserves_closed_geometry_edits_and_saved_history`.
  At a 0.4 mm perimeter radius, shifting the mesh-volume integration origin
  changes the result by 0.630342 mm³ on a 15,531.378889 mm³ body, exceeding the
  existing ten-parts-per-million check. It also failed before this rebase.
  Topological validity alone does not establish geometric closure here; the
  assertion remains enabled and its tolerance has not been relaxed.
- **All 17 targeted curved-blend kernel tests pass** (111.05 s), including the
  new analytic pinched-endpoint contact checks. Formatting and whitespace pass.

The first remaining complete-history failures are:

| File | First failing area |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class/version, feature 621 |
| Style 2 | Unsupported history flags, feature 3963 at byte 38 |
| Style 3 | Projected composite cap requires multiple contributors |
| Style 4 | Unsupported joined blend selection mode |
| Style 5 | Fillet6: rolling-ball contact does not fit at the tapered endpoint |
| Style 6 | Unsupported composite cap mapping count |
| Style 7 | Unsupported body recipe entry count |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Missing chamfer reference, feature 16766 at byte 40 |
| R36S Grip | Unsupported timeline class/version, feature 1160 |
| adapter-x20-makita | Missing fillet reference, feature 18835 at byte 27 |

Style 5 still recovers eleven editable features. The pending endpoint treatment
advances its kernel refusal from end-face classification to contact validation:
one contact misses by approximately 0.00000765 mm against a 0.00000440 mm
tolerance. This is not a successful Fillet6 import. The original 0.25 mm wing
case succeeds, but its 0.4 mm edit exposes the mesh-volume issue above.

The corpus took 426.48 s excluding compilation. Timings were affected by
concurrent tests in another workspace; for example, the correctness-mode clamp
open took 60.45 s versus 8.35 s in the earlier run. A stack sample showed active
body meshing, not a stalled dialog. These runs are not a controlled performance
comparison. The previously reported native-decoder resource refusal is a
software decoding budget, not a measurement of available hardware.

No PR, new feature commit or push: the ordinary I/O regression and strict
acceptance are failing. Full workspace CI was not run. Logs and exit codes are
in `/private/tmp/f3d-7349-results.json`, with `corpus.log`, `performance.log`,
`io.log` and `curved-blends.log` under the same prefix. The stack sample is
`/private/tmp/f3d-7349-corpus-sample.txt`.

## Upstream 0.13.0 and glTF integration — 2026-10-15

Rebased onto the 0.13.0 release and glTF import change (#1512). Resolved conflicts
in the dependency lists, browser file picker, import-command descriptions and
dependency notices by retaining both import formats and regenerating the notices.
The pre-rebase manifest covers 207 pending files: none are missing, and all are
byte-identical except the four files that also received upstream changes
(`ATTRIBUTION.md`, `Cargo.lock`, `crates/engine/src/cmd/file.rs`, and
`crates/io/Cargo.toml`). The preservation stash has already been restored.

Post-rebase validation:

- **304 I/O tests passed, 32 ignored** (40.12 s), including the new glTF coverage.
- All **17 archives open, save and reopen under both recovery preferences**
  (34/34 checks). Each retains geometry and stable saved history/mesh volume.
  This is partial recovery, not proof of complete or geometrically exact import.
  Gridfinity retains six bodies and explicit incomplete-shell warnings.
- **6/17 complete editable imports pass**, unchanged from before the rebase:
  Minimal, Crescent, Eye, Narrow, Round and Square. The corpus test exits 101
  because the strict gate fails; the separate recovery test passes (232.27 s
  total test time, 275.82 s including compilation).
- Three engine glTF tests and nine UI file-opening tests pass. The original
  touching-tip fillet fixture passes analytic volume, editing, undo and reopening
  checks (6.01 s). The other 172 recipes were not rerun in this integration check.
- HardEdgeBasin opens and prepares its display in **2.029 s**: 14 bodies and
  14,898 triangles. Its five known feature limitations, source preservation and
  warm-cache reuse assertions remain unchanged.
- All-target Clippy passes for I/O, CLI and UI. All eleven WebAssembly targets
  compile. Formatting, attribution and layering checks pass; the command list
  and dependency notices were regenerated. Native probe tests in `f3d_import`
  remain opt-in (52 ignored); this is not a full workspace CI run.

The first strict failures are:

| File | First failing area |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline class/version, feature 621 |
| Style 2 | Unsupported history flags, feature 3963 at byte 38 |
| Style 3 | Projected composite cap requires multiple contributors |
| Style 4 | Unsupported joined blend selection mode |
| Style 5 | Fillet6: curved-edge chain has no acceptable end face |
| Style 6 | Unsupported composite cap mapping count |
| Style 7 | Unsupported body recipe entry count |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Missing chamfer reference, feature 16766 at byte 40 |
| R36S Grip | Unsupported timeline class/version, feature 1160 |
| adapter-x20-makita | Missing fillet reference, feature 18835 at byte 27 |

Style 5's refusal comes from `crates/kernel/src/curveblend.rs:1096`: an open
chain endpoint must meet a face outside the chain's side-face sets. The native
references resolve, but the geometry builder rejects that endpoint topology.
The original thin-wing reproduction described below produces the same refusal.

The final remote check still reports `a13124ac`. No new feature commit, push or
PR: complete-history acceptance remains red, and full workspace CI has not run.
Logs use `/private/tmp/f3d-a131-`; `results.json` records each check's exit code
and elapsed time. The I/O log is `io.log`, and the per-file diagnostics and
recovery reports are in `corpus.log`.

## Fillet-generated face tracking — 2026-10-15

Style 5's Fillet6 now resolves all **13 native selections to 17 unique kernel
spans**. The new path follows fillet output labels through their tracked source
edges in the pre-fillet model state, validates retired qualifiers against both
the operation map and exact current inputs, and resolves touching round faces
and finite edge chains across internal seams. Cached spline envelopes are
validated but their coordinates do not choose geometry. The native selection
gate passes with uniqueness and span-count assertions (37.31 s).

Actual regeneration still fails: Fillet6 reports **"an open chain of curved
edges must end at a face"**, followed by the straight-edge fallback refusal.
Recovery still has eleven features and three omissions. The failure has moved
from reference reconstruction into the geometry kernel; resolving the selections
does not mean Fillet6 imported. Face-dependency diagnostics now include the
failing feature's name and error rather than hiding it behind a generic message.

The new original `partial-cylinder-touching-tip-fillets` fixture has a 2 mm wing
and two radius-1 mm tip rounds that meet on one cylinder. Analytic corner removal,
radius/source-height edits, suppression, undo and reopening pass (7.44 s).
Fine-resolution I/O checks verify both the touching radius and a separated
0.75 mm radius, and reject expansion into unowned lineage. The rendered fixture
was visually checked at
`/Users/chess/projects/cad/repro/f3d/partial-cylinder-touching-tip-fillets.png`.
The saved suite now contains 173 original recipes.

All **296 normal I/O tests pass**, with 32 opt-in tests ignored (29.62 s).
I/O/CLI all-target Clippy passes (39.39 s); final I/O Clippy passes again after
the diagnostic edits (4.15 s). All thirteen targeted blend tests pass (25.78 s),
and attribution and whitespace checks pass.
Upstream still reports `843c5485`. Full workspace CI and complete-history
acceptance remain unresolved; no new commit, push or PR.

Logs use `/private/tmp/f3d-fillet-face-`: `io-suite.log`, `native-v4.log`,
`native-final.log`, `final-tests.log`, `clippy.log`, and `corpus.log`.
The new fixture's logs are `/private/tmp/f3d-touching-tip-author.log` and
`/private/tmp/f3d-touching-tip-history.log`; actual recovery is recorded in
`/private/tmp/f3d-fillet6-recovery.json`.

The same remaining curved-chain failure is reproduced without native data by
`/Users/chess/projects/cad/repro/f3d/compound-kernel/thin-rounded-wing.json`
and `thin-rounded-wing-perimeter.json`. The first constructs an original thin
wing, touching tip rounds and connected concave root/shoulder rounds and saves
`thin-rounded-wing.solidcraft`. The second selects its seventeen perimeter spans
for a radius-0.25 mm fillet and reproduces the kernel refusal. The diagnostic is
`/private/tmp/f3d-thin-perimeter-kernel.log`.

The final corpus run retains **34/34 recovery checks** and **6/17 complete
editable imports** (254.89 s; exit 101 for the strict gate only). Style 7 improves
from six to **seven editable features**, with two bodies and nine warnings. Its
first strict failure advances to C-Pattern2's unsupported body-recipe entry count.
Style 5 retains eleven features and three warnings, now identifying Fillet6's
actual kernel refusal. Gridfinity's incomplete-shell warnings remain explicit.

## Combined concave rounds and fresh upstream check — 2026-10-15

Fetched upstream again: `origin/main` remains `843c5485`, already an ancestor
of this branch. No additional rebase or conflict resolution was needed.

Two concave rounds meeting at a corner now use the oriented-strip intersection
builder. A separate fallback partitions selected edges by shared topological
vertices, rebuilding disconnected groups while retaining provenance and requiring
later selections to remain on the current body. Mixed convex/concave corners,
three concave strips and two concave chamfers remain explicitly unsupported.
The corner module passes eight tests (one existing ignored test).

Style 5 now retains **Fillet1**, including all seven selected edge spans. Its
recovery has eleven editable features and three omission warnings. A native test
changes Fillet1's radius, saves/reopens and regenerates the model, and verifies
that the radius and geometry change (51.56 s). The initial new test assertion
mistakenly evaluated the pre-edit expression; it now reads the saved feature's
current expression. Complete import advances to Fillet6's unsupported blend-cache
carrier. Sketch2's projected boundary and its dependent Extrude2 remain omitted.

Two original saved designs cover the combined wing corners, with and without
the separate shoulder rings. Independent circular-strip integration includes the
corner intersection volume. Both 1 mm and 1.5 mm radii agree within 0.02 mm³ at
a common 0.000025 mm chord tolerance; checks include reversed corner selections,
closed geometry, source-height edits, suppression and reopening. The saved suite
contains 172 recipes. The combined model was rendered and visually checked at
`/Users/chess/projects/cad/repro/f3d/partial-cylinder-combined-all-fillet.png`.

All 289 normal I/O tests pass (31 opt-in tests ignored; 28.87 s), as do
kernel/I/O/CLI all-target Clippy, attribution and whitespace checks. Complete
editable import still passes **6/17**: Minimal, Crescent, Eye, Narrow, Round and
Square. The first failures in the other files are:

| File | First failing area |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline feature class/version (621) |
| Style 2 | Unsupported history flags (3963, byte 38) |
| Style 3 | Projected composite cap selection |
| Style 4 | Joined blend selection mode / edge identity |
| Style 5 | Fillet6 blend-cache carrier / edge ownership |
| Style 6 | Composite cap mapping count |
| Style 7 | Duplicate fillet label / invalid blend side label |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Chamfer reference missing (16766, byte 40) |
| R36S Grip | Unsupported timeline feature class/version (1160) |
| adapter-x20-makita | Fillet reference missing (18835, byte 27) |

Evidence logs use `/private/tmp/f3d-final-recheck-`: `native.log`, `io.log`,
`corpus.log`, `history.log`, `performance.log`, `clippy.log`, and `assets.log`.
Full workspace CI remains unverified. No new commit, push or PR: the requested
all-checks-passing condition is not met.

The finished corpus run passes **34/34 recovery checks**: all 17 files open,
save and reopen in both preferences with geometry and unchanged round-trip volume.
The combined command exits 101 solely for the failed complete-history gate
(243.40 s). Gridfinity retains six bodies and explicit open-shell warnings, so
this does not establish solid fidelity. Its historical topology limit is a
software decoder work budget, not detected hardware capacity. All **172 original
saved-history recipes**, plus the legacy offset fixture, pass their edit, undo,
volume and reopen checks (255.80 s). These concurrent runs are not isolated
performance comparisons.

HardEdgeBasin's cold open and display regression passes in **2.207687125 s**,
with 14 bodies and 14,898 triangles; source preservation and warm cache reuse
checks pass. The final upstream fetch remains `843c5485`, already integrated.

## Concave wing-root blends — 2026-10-15

The original cylinder/wing reproduction isolated a kernel refusal: when a blend
contact went beyond an old seam endpoint, the kernel required the extension to
lie inside the old planar end face. A concave round adds material and extends
that face into the adjacent empty region. Such an extension is now checked
against the actual planar end face: it must stay outside, touch no other boundary,
and remain on the continued seam carrier. Convex behavior retains the existing
inside-face check. Oversized rounds that cross the lower shoulder or consume the
wing tip remain refused. Boundary tests cover holes, crossing, overlap, and entry
into the old face in two plane orientations. Curved-blend diagnostics now identify
which contact or end restriction failed.

The vertical and lower circular wing-root fillets now regenerate independently.
Two new original saved fixtures bring the history suite to 170 recipes. The
vertical round's added volume is derived from Green's theorem on its two-circle
and straight-wall cross-section; the lower round integrates the circular-strip
area of its torus segment, clipped by the wing side planes. At radii 1 and 1.5 mm,
saved parameter edits and reopening agree with these independent added volumes
within 0.02 mm³ at 0.0001 mm tessellation tolerance. The fixtures also check source
height edits, suppression and undo. The vertical root render was visually checked:
`/Users/chess/projects/cad/repro/f3d/partial-cylinder-wing-vertical-fillet.png`.

All 287 normal I/O tests pass (31 opt-in tests ignored), all 16 curved-blend tests
pass, and the three extension checks pass. Both new saved-history recipes pass
individually (5.81 s and 6.95 s); the precise saved radius/volume check passes
(5.18 s). Kernel/I/O/CLI all-target Clippy and asset attribution pass. All 170
saved-history recipes plus the legacy offset case pass (213.07 s). The corpus
retains 34/34 recovery checks and 6/17 complete editable imports (208.10 s; strict
history gate fails). The latest upstream fetch still reports `843c5485`.
Full workspace CI remains unverified.

This fixes individual edges, not the combined native feature. Style 5 still omits
Fillet1: it now fails while constructing the end of one blend where it meets a
second blend, rather than rejecting the outward seam extension. The same failure
is reproduced by the original three-edge `wing-roots-and-bottom.json` script in
`repro/f3d/compound-kernel`. The corner fallback explicitly refuses concave blends
meeting at a corner (`curvecorner.rs`); that remains the next kernel investigation.
Fillet6, Sketch2 and Extrude2 remain omitted too. No new commit or PR.

Evidence logs use `/private/tmp/f3d-wing-`: `io-suite.log`, `saved-volume.log`,
`extension-tests.log`, `curveblend-tests.log`, `vertical-history.log`,
`bottom-history.log`, `history-suite.log`, `native.log`, `combined.log`,
`clippy.log`, and `corpus.log`.

## Sketch clipboard upstream integration — 2026-10-15

Rebased onto `843c5485` (sketch copy/cut/paste). Rebase and restoration were
conflict-free. All 193 pending files were preserved. Only the Japanese and
Chinese locale tables and generated command reference changed during integration;
these include upstream additions. Stash
`86f1db34d2fd34b0005ebd498e5832be217f3e79` is **already applied**.
The preservation manifest is `/private/tmp/f3d-before-843c-manifest.json`.

Two original saved-history fixtures now exercise the neck and base shoulder
fillets independently, bringing the recipe count to 168. The ordinary I/O test
integrates circular fillet cross-sections analytically and checks both 1 mm and
1.5 mm radii after saving/reopening. At a common tessellation tolerance of
0.0001 mm, volume-change errors must remain below 0.02 mm³ and total-volume
errors below 0.5 mm³. This test passes after rebase (19.58 s). These fixtures do
not certify the separate failing wing-root or wing-arc fillets.

Post-rebase checks completed:

- Both new saved recipes pass replay, radius/height edits, suppression, undo and
  reopening (neck 10.85 s; base 6.14 s).
- HardEdgeBasin opens and displays in **2.463098875 s**, with 14 bodies and
  14,898 triangles; its performance regression gate passes.
- All seven upstream sketch-clipboard engine tests pass.
- I/O and CLI all-target Clippy pass with warnings denied; UI all-target
  compilation passes. Asset attribution passes for all 86 assets.
- Complete editable imports remain **6/17**, with the same failure points as
  the preceding upstream revision.
- **34/34 recovery checks pass**: all 17 archives open, save and reopen in both
  preferences. Combined corpus time is 215.26 s; exit 101 reflects the failed
  complete-history gate. Recovered geometry is not proof of source fidelity;
  Gridfinity still reports incomplete shells.
- Formatting and whitespace checks pass.

Remaining complete-history failures:

| File | First failing area |
| --- | --- |
| Modular Clamp V1 | Unsupported timeline feature class/version |
| Style 2 | Unsupported history flags |
| Style 3 | Projected composite face selection |
| Style 4 | Fillet edge identity / joined selection mode |
| Style 5 | Kernel cannot build Fillet1 on the recovered joined edges |
| Style 6 | Composite cap mapping |
| Style 7 | Duplicate fillet label / edge ownership |
| style-1 | Ambiguous blend boundary carrier |
| Gridfinity Storage Box Shelves | Chamfer record missing object reference |
| R36S Grip | Unsupported timeline feature class/version |
| adapter-x20-makita | Fillet record missing object reference |

Minimal, Crescent, Eye, Narrow, Round and Square pass complete editable import.
The requested condition for opening a PR is therefore not met.

Logs use the prefix `/private/tmp/f3d-843c-`: `rings.log`, `neck.log`, `base.log`,
`performance.log`, `clipboard.log`, `clippy.log`, `ui.log`, `assets.log`, and
`corpus.log`. Full workspace CI remains unverified; no new commit or PR.

## Compound blend identities and latest integration — 2026-10-15

Closed native edges now retain their one seam vertex, two incident surfaces and
opaque intersection signatures. Finite edges retain both endpoints. Exact face
lineage can resolve curved face families without applying the planar-support
restriction. Chamfer output labels preserve their explicit input-group mapping;
face-extruded circular sides follow their owned source tracker back to the live
cap profile and its chamfer. Every selected span must be unique and have the
expected endpoint incidence. A closed selection must cover one complete circle,
including both kernel seam pieces, without overlaps or missing arcs.

Style 5's five Fillet1 selectors now resolve to seven exact current edges: two
vertical wing roots, two two-piece shoulder rings, and the lower wing arc. The
opt-in test checks every selector. The feature still fails in the kernel's rolling
blend, which reports a size refusal with the planar fallback's curved-face error.
Strict imports now report the failing dependency's name and actual error:
`cannot regenerate blend dependency 11 (Fillet1): ...`. Recovery still retains
ten features with four omission warnings. Resolving the selections is not proof
that the fillet or complete native history has imported successfully.

Original tests cover the circle and wing intersections through diameter edits,
incomplete rings, collapsed face identities, seam-roundoff intervals, and malformed
closed-edge records. The existing tip-fillet analytic removal and native radius
edit tests remain passing. A temporary endpoint-pick regression discovered during
implementation was corrected by retaining midpoint picks for straight edges.

Rebased onto `7934edca` after upstream's text Boolean, extrusion-hole, dialog and
start-page changes landed. Rebase and restoration were conflict-free. All 189
pending files were restored from stash
`9a3fd234eafee74c25b3755a2b1887fc3dbe5490` (**already applied; do not reapply**).
184 were byte-identical; the five integrated files are document evaluation,
kernel prism construction, dialogs, and Japanese/Chinese locale tables. The
content manifest is `/private/tmp/f3d-before-7934edca-manifest.json`.

Post-rebase validation:

- **285 I/O unit tests pass** (20.17 s).
- All **166 original saved-history recipes** and the legacy offset case pass
  replay, geometry/volume checks, edits, suppression, undo and reopening (210.80 s).
- All **34/34 recovery checks** pass. Complete editable history remains **6/17**;
  combined corpus time is 236.97 s, exiting 101 for that failed strict gate.
- Both native joined-blend tests pass, including all five Fillet1 selections and
  Fillet3's saved radius edit/volume check (44.05 s).
- HardEdgeBasin passes at **2.687665791 s**, 14 bodies and 14,898 triangles.
- Both new upstream ring-cut/text-Boolean regression tests pass (14.26 s).
- UI all-target compilation and document/I/O/CLI all-target Clippy pass.

These runs shared the machine and are not isolated benchmark comparisons.
Style 7 editability opening took 24.93 s; Style 5 took 13.80 s. Gridfinity retains
six recovered bodies and explicit incomplete-shell warnings. Full workspace CI
and complete-history/source-fidelity acceptance remain unresolved. No additional
feature commit, push or PR was made.

Original kernel isolation scripts and saved successful regenerations are in
`/Users/chess/projects/cad/repro/f3d/compound-kernel`. On the
`partial-cylinder-wall-join` fixture, each radius-1 shoulder ring regenerates
individually; the vertical wing-root and lower wing-arc fillets each fail alone.
Thus the kernel problem is not restricted to selecting all seven edges together.
Regeneration alone does not certify the successful rings' volume; a separate
fixed-resolution analytic volume check accompanies this diagnosis.

Evidence: `/private/tmp/f3d-7934-io-suite.log`,
`/private/tmp/f3d-7934-editable-history.log`, `/private/tmp/f3d-7934-corpus.log`,
`/private/tmp/f3d-7934-native-fillets.log`, `/private/tmp/f3d-7934-performance.log`,
`/private/tmp/f3d-7934-upstream-extrusions.log`, `/private/tmp/f3d-7934-ui.log`,
`/private/tmp/f3d-7934-clippy.log`, and `/private/tmp/f3d-compound-ring-volumes.log`.

## Requested latest-upstream opening recheck — 2026-10-15

Fetched `origin/main` again: it remains `b6806b16`, already an ancestor of this
branch. No further rebase or conflict resolution was necessary, and pending
implementation work was preserved. All 13 extracted F3D files were verified
byte-for-byte against their supplied ZIP members before testing.

Fresh results on the current working tree:

- Both import preferences pass all **34/34 open/save/reopen checks** across 17
  designs, retaining geometry, feature serialization and round-trip mesh volume.
- Complete editable-history acceptance still passes **6/17**: Minimal, Crescent,
  Eye, Narrow, Round and Square. The combined corpus command exits 101 because
  this strict gate fails; the recovery test itself passes. Total test time:
  **217.24 seconds**.
- HardEdgeBasin cold open plus display passes in **2.226772542 seconds**, with
  14 bodies and 14,898 triangles, within its 20-second deadline. Expected existing
  feature limitations, source preservation and warm-cache reuse checks pass.
- I/O unit tests: **281 passed, 30 ignored**, 9.80 seconds. Formatting and
  whitespace checks pass.

First complete-history failures:

| Design | First unsupported operation or reference |
| --- | --- |
| Modular Clamp | Timeline feature 621, class `584D9526`, version 2 |
| Style 2 | Feature 3963: history flags at record byte 38 |
| Style 3 | Projected composite cap / body-edge identity |
| Style 4 | Fillet edge identity; joined selection mode |
| Style 5 | Fillet edge identity; joined edge/intersection branch |
| Style 6 | Projected face boundary / composite cap mapping count |
| Style 7 | Duplicate fillet face label / body-edge ownership |
| style-1 | Fillet edge identity; joined edge/intersection branch |
| Gridfinity | Chamfer feature 16766: missing reference at byte 40 |
| R36S Grip | Timeline feature 1160, class `D7015B81`, version 4 |
| adapter-x20-makita | Fillet feature 18835: missing reference at byte 27 |

Recovery success does not certify source-shape fidelity. Gridfinity retains six
bodies with explicit open-shell warnings, so its solid mass properties are not
reliable. Its historical topology refusal is a decoder software work budget
(4,000,000,000 units; requested work 24,036,730,880), not a hardware memory limit.
The bounded final-body fallback succeeds. Style 7 still takes **23.59 seconds**
to open in editability mode (six features), versus **0.74 seconds** for correctness
mode (31 recovered bodies); this run is not an isolated performance comparison.

Full workspace CI was not repeated after the failed complete-history acceptance
gate. No commit, push or PR was made: the requested all-checks-passing condition
is not met. Evidence: `/private/tmp/f3d-latest-upstream-corpus.log`,
`/private/tmp/f3d-latest-upstream-io.log`, and
`/private/tmp/f3d-latest-upstream-performance.log`.

## Sketch-text integration — 2026-10-15

Rebased onto `b6806b16` after upstream sketch-text expressions landed. The conflict
in `Document::param_graph` retains both text-expression references and imported
offset-expression references. All pending work was restored from preservation
stash `d2331fa3840d46d663bcd28d2e022eb2016e3be7` (already applied; do not reapply).
Of 186 pending files, 180 remained byte-identical; the six differences integrate
upstream text support in document/evaluation/format/parameters/projection/link code.

All three joined-blend checks pass on this integration (31.72 seconds), including
record/ownership rejection, original analytic removal and native Style 5's saved
radius edit. All eight upstream text-expression tests, UI all-target compilation,
and the new mixed text/imported-offset dependency-and-renaming test pass too.
Full workspace CI and the entire supplied corpus were not repeated for this
integration; complete-history acceptance and the earlier robustness failure
remain unresolved. No new feature commit, push or PR was made.

Evidence: `/private/tmp/f3d-b6806b-joined-blend-tests.log`,
`/private/tmp/f3d-b6806b-text-expr-tests.log`,
`/private/tmp/f3d-b6806b-param-integration.log`, and
`/private/tmp/f3d-b6806b-ui-check.log`.

## Joined cap blend verification — 2026-10-15

The apparent tip-fillet volume failure was traced to adaptive measurement, not
incorrect removal. Adding a radius-1 mm fillet tightens the general measurement
API's tessellation tolerance for the entire body, including the untouched large
cylinder. Subtracting those differently resolved volumes produced the misleading
1.65 mm³ discrepancy. At a fixed 0.0001 mm chord tolerance, removal is 1.610104 mm³
against analytic 1.609514 mm³; at 0.00001 mm it converges to 1.609657 mm³.

The regression now uses the same fine tolerance before and after, retaining the
0.02 mm³ removal bound, and also checks total volume against an independent
cylinder/frustum/circular-segment formula. Radii 1 and 1.5 mm both pass. Exact
endpoint incidence is additionally checked for collapsing distinct native faces.
A new original `partial-cylinder-tip-fillet` recipe and saved design check radius
and source-height edits, suppression, undo, reopening and analytic volume.
The rendered example shows both rounded wing-tip edges and the retained neck/base.
The suite now has 166 original recipes; the new recipe passes its focused replay.

Style 5's focused local import/save/reopen check passes with ten features and four
omission warnings. Editing its imported Fillet3 radius to 75% of the original
also passes saved-history regeneration and the analytic change in removed volume
within 0.02 mm³ at a consistent chord tolerance (14.15 seconds for the test).
The follow-up native edit test and I/O test-target Clippy pass on `e20cf185`;
UI all-target compilation passes there too. Native edit evidence:
`/private/tmp/f3d-joined-blend-native-edit.log`. Fillet1 requires cylinder/plane intersections and curved-edge
selectors; Fillet6 also references faces produced by preceding blends. These
remain explicit omissions, along with the final sketch/extrusion. This work does
not establish complete import, and no PR has been opened.

The final rebase onto `e20cf185` (flat-pattern navigation) completed without
conflicts; all 186 pending files stayed byte-identical. Preservation stash
`a2da9c088fa2700abc18fbf626a6f9a4f4691e42` is already applied; do not reapply it.
The upstream change does not touch importer, document, kernel or CLI test sources.

I/O unit suite: **281 passed, 30 opt-in tests ignored** (10.39 seconds). Asset
attribution and I/O/CLI all-target Clippy with warnings denied pass. Evidence: `/private/tmp/f3d-joined-blend-volume-diagnosis.log`,
`/private/tmp/f3d-joined-blend-consistent-volume.log`,
`/private/tmp/f3d-tip-fillet-history.log`, `/private/tmp/f3d-joined-blend-io-suite.log`,
and `/private/tmp/f3d-joined-blend-clippy.log`. Local visual evidence:
`/Users/chess/projects/cad/repro/f3d/partial-cylinder-tip-fillet-solidcraft.png`.

## Follow-up rebase onto 6ad391fd — 2026-10-15

Rebased onto upstream's agent-chat integration and chamfer wrong-way-volume guard.
Cargo.lock and THIRD-PARTY-LICENSES.txt conflicted; they were regenerated against
the combined dependency graph. Pending work was restored from preservation stash
`076b621b32fb4343705248d4f31e8ad40dfd9ea3` (already applied; do not reapply).
All 184 pending files were restored, with 179 byte-identical. The five integrated
files are Cargo.lock, ROADMAP, Japanese/Chinese locales, and the kernel module
list (upstream's new test module). All 13 extracted F3D ZIP members again match
the supplied archives byte for byte.

The complete-history gate still passes **6/17**: Minimal, Crescent, Eye, Narrow,
Round and Square. The remaining first failures are unsupported feature classes
(Clamp/R36S), history flags (Style 2), projected profiles (Styles 3/6), compound
blend references (Styles 4/5/7 and style-1), and missing references
(Gridfinity/adapter). Detailed errors are in `/private/tmp/f3d-6ad-corpus.log`.

Recovery passes **34/34 open/save/reopen checks**: all 17 files retain geometry in
both preferences, with no regeneration errors and stable round-trip mesh volume.
The combined strict/recovery run took 449.63 seconds and exits 101 solely because
the complete-history gate fails. Gridfinity retains six bodies with explicit
open-shell warnings; neither recovery mode establishes final-source fidelity or
valid solid mass properties for incomplete shells.

Style 7's editability open took 117.93 seconds (six features), so a substantial
opening delay remains even though recovery succeeds. This run shared the machine
with other checks; it does not establish whether upstream introduced a regression.
A short later sample caught native BREP graph decoding/retention, rather than a
hardware allocation failure, during the recovery pass. The local profile is
`/private/tmp/f3d-6ad-corpus-sample.txt`.

The pending joined-blend implementation now retains Style 5's Fillet3 along with
Sketch5 and Extrude7: ten editable features, four omission warnings. Its focused
native save/reopen/regeneration test passes, but this does **not** certify the
resulting shape. The original analytic regression fails: radius 1 mm produces
16044.565167 mm³ instead of 16042.913492 mm³. Its saved-history fixture was corrected
to declare the fillet's model parameter name; the volume assertion remains intact.
This was unresolved at the recheck; the later fixed-resolution investigation above resolves this particular discrepancy.

HardEdgeBasin passes cold-open/display, geometry, source-preservation and warm-cache
checks: **2.676648375 seconds**, 14 bodies, 14,898 triangles, within the 20-second
budget. This overlapped other checks and is not an isolated benchmark comparison.
Both new upstream wrong-way chamfer tests pass (14.29 seconds). UI all-target
compilation passes. Formatting and whitespace checks pass. Full CI was not rerun:
complete-history acceptance and the focused analytic regression already fail.
No new feature commit, push or PR has been made.

Logs: `/private/tmp/f3d-6ad-composite-tests.log`,
`/private/tmp/f3d-6ad-chamfer-tests.log`, `/private/tmp/f3d-6ad-performance.log`,
`/private/tmp/f3d-6ad-ui-check.log`, and `/private/tmp/f3d-6ad-licences-network.log`.

## Earlier recheck through 02fe495d — 2026-10-15

Rebased first through `be985f13`, resolving two conflict regions in
`crates/kernel/src/prism.rs`. The resolution retains both the F3D branch's
whole-prism axis prioritization and upstream's general/tool-cap candidate axes.
Then rebased without conflicts through `02fe495d` after the material-card,
texture-normalization and joint-unit updates landed during validation.
All 182 pending files were preserved: 173 remained byte-identical after the first
integration, and 178 after the final one. The final four differences are upstream
additions to Cargo.lock, ROADMAP and the Japanese/Chinese locales. Backup stashes
`43a0622c09d3166910b13514e12d132ded098127` and
`468bc0030469ee81c5935869c5bfdc7702c9785d` are already applied; do not reapply them.

The fresh corpus run confirms **6/17 complete editable imports** and **34/34
recovery open/save/reopen checks**, with geometry in every archive for both
preferences. All 13 F3D members extracted from the supplied ZIP packs match their
archives byte for byte. The combined run exits 101 for the complete-history gate
(821.89 seconds while sharing the machine with full CI and builds). These are not
isolated performance timings. A profile of the slow clamp check found display
meshing of native spline surfaces; it eventually completed successfully.
Gridfinity retains six bodies and explicit open-shell warnings, so successful
recovery does not establish final-shape fidelity or reliable mass properties.

Style 5 now retains **nine editable features**, including Sketch5's live composite
cap projection and Extrude7's top recess, with five omitted-feature warnings.
Its first strict failure has moved to the composite fillet edge reference.
The focused native parameter-edit regression passes after rebasing: height 9→10
moves the linked sketch by 1 mm and changes volume from 6952.574342 to
7568.298512 mm³ (analytic expectation 7568.326502 mm³). Saving and reopening also
preserves the linked history.

Validation found and fixed a sheet-metal regression: adding only cap certificates
to a cut tool mixed certified caps with raw walls, which the flat-material
validator correctly refused. Sheet cut tools now retain their raw prism metadata.
The formerly failing sheet UI tests, topology rebuild tests and action-matrix
sample all pass with this change.

Validation on `be985f13`:

- Formatting, Clippy with warnings denied, assets, licenses, dependency layers,
  WASM and doctests pass.
- I/O: 279 tests; document: 249; sketch: 213; UI scenarios: 111 pass.
- The complete editable-history regression suite passes, including original
  analytic-volume, parameter-edit and saved-history fixtures (759.24 seconds).
- Topology rebuild and all 19 active action-matrix tests pass.
- HardEdgeBasin's opening/display/source-preservation/warm-cache regression passes
  its 20-second budget; the test binary reports 2.10 seconds total runtime.
- The viewport theme-cache test failed in the parallel UI suite, then passed in
  isolation (0.08 seconds). Its cache key includes a global theme flag, a plausible
  source of interference between parallel tests; that cause is not yet proven.
- The engine robustness regression
  `tests::robustness::a_boundary_found_across_a_seam_is_not_used` fails independently
  too: seed 20 returns `boolean Cut: the operation failed: no result` (56.74 seconds).
  Its attribution to upstream versus pending feature changes remains unresolved.
- The remaining kernel and engine test processes were stopped after the broad run
  had spent 1051.9 seconds executing tests. **Full CI is incomplete and not green**;
  the web-offline stage was not reached. This is not a full-suite pass.

The final `02fe495d` integration passes UI all-target compilation, including its
engine, I/O, document and kernel dependencies. The final upstream changes do not
alter the F3D importer, document/kernel code or corpus test; these sources remain
byte-identical to the freshly tested versions. Full CI was not repeated solely for
this final integration. No new feature commit, push or PR was made: complete-history
acceptance and the reproducible boolean robustness regression still fail.

Evidence:

- `/private/tmp/f3d-be985-corpus.log`
- `/private/tmp/f3d-be985-style5-edit.log`
- `/private/tmp/f3d-be985-ci.log`
- `/private/tmp/f3d-be985-theme-recheck.log`
- `/private/tmp/f3d-be985-seam-recheck.log`
- `/private/tmp/f3d-be985-corpus-sample.txt`
- `/private/tmp/f3d-02fe-integration-check.log`

## Earlier requested recheck

Rebased without conflicts onto `8b7fa260` (material-condition diagnostics), then
`aa31e564` (shipping the CLI inside the macOS app), which landed during validation.
All 175 pending files remained byte-identical through both rebases, before this
report update. Preservation stashes `e9c936980953a551d5d88439b4a3564f65aea71a`
and `520f24dab13ec546327f563d4bc5daa95e2dbe13` are already applied; do not reapply.

The fresh corpus run on `8b7fa260` confirms **6/17 complete editable imports** and
**34/34 recovery open/save/reopen checks**, with geometry in every file in both
preferences. All 13 F3D members extracted from the two supplied ZIP packs were
verified byte-for-byte against their archives. The first complete-history failures
remain those in the table below. The combined run exits 101 for the complete-history
acceptance gate after 208.49 seconds; recovery passes. Gridfinity still reports
open shells, so recovery does not establish reliable mass properties or final shape
fidelity. Style 5 retains seven features in editability mode and still stops at
Sketch5's projected boundary for complete import.

HardEdgeBasin passes opening, display, geometry, source preservation and warm-cache
checks at **2.112 seconds**, with 14 bodies and 14,898 triangles. It overlapped the
corpus run, so this is not an isolated benchmark comparison. Evidence:
`/private/tmp/f3d-8b7f-corpus.log` and `/private/tmp/f3d-8b7f-performance.log`.

The final upstream commit changes only `packaging/macos/package.sh`, whose shell
syntax check passes. Import, geometry and test sources are unchanged from the fresh
run, so those tests were not repeated for the packaging-only rebase. Full workspace
CI remains unverified. No feature commit, push or PR was made: complete-history
acceptance still fails for 11 files.

## Previous rechecks

The additional UI-only update from `e382a773` to `fd422a3f` also rebased without
conflicts. Preservation stash `2da103ff9482038b89c3d5160ea2057b0a71a59c` has already
been applied; do not reapply. All 175 pending files remain: 174 byte-identical,
with only upstream's outlined-button change added to `dialogs.rs`. Import,
geometry and corpus-test code are unchanged from the `e382a773` validation below.
The new hover-size regression and UI all-target compilation pass on `fd422a3f`
(`/private/tmp/f3d-fd422-ui-test.log`, `/private/tmp/f3d-fd422-ui-check.log`).

The latest rebase from `2bfec256` to `e382a773` completed without conflicts.
All 175 pending files were restored from preservation stash
`6be8b84ba79a043da5af7f7b9d77f37bc408792d` (already applied; do not reapply).
Of those files, 171 are byte-identical to the pre-rebase manifest. The other four
contain only upstream additions: preview progress in ROADMAP, Compare Bodies
translations in Japanese/Chinese, and the generated command listing.

Rebased first onto `3fa8fbd1`, then included the newly landed chamfer-chain fix
and web installation changes through `e8c340c5`. Conflicts in format declarations,
file-opening command descriptions and container documentation preserve both upstream
import/component support and the F3D extensions. The subsequent rebases were conflict-free. The final update adds upstream decimal-comma expression support (`2bfec256`).
All pending work was preserved and compared against the saved copies. Stashes
`6d49ae82d5a00d7e48a383aa2a009970a4ad423b` and
`adfabeffd1a56e19ef36aa7eb5491f3a0d0e78ba` have already been applied; do not reapply them.
The final preservation stash `f6054530115adaecaa33382c2f113b3fe6fde1e5` has also
already been applied. Older preservation stashes also remain applied.

## Earlier opening results (before composite projection)

The acceptance tests exercise all 17 supplied F3D archives, including files
extracted from the two ZIP packs. They use the engine's real `doc.open`, measure,
save into a native container and reopen.

- Complete editable history: **6/17 pass** — Minimal, Crescent, Eye, Narrow,
  Round and Square. The remaining failures are listed below.
- The latest recovery recheck passes **34/34 recovery open/save/reopen checks**,
  with geometry in **all 17 files** in either preference on `e382a773`. The combined
  strict/recovery run took 188.82 s and exits 101 for strict acceptance. The former
  empty-Gridfinity exception has been removed from the test. Gridfinity now
  recovers six bodies, but open-shell warnings mean their mass properties and
  final-source shape fidelity are not established. Strict acceptance still fails.
- Style 5 editability recovery now retains seven features and one body, including
  Sketch4's linked cap projection and Extrude6's partial curved-wall union, with
  zero regeneration errors and seven omission warnings. Its Extrude6 checkpoint
  measures 6963.300 mm³ against an analytic 6963.977 mm³ expectation. Correctness
  recovery previously returned one native body: 6868.534 mm³, fourteen warnings.
  Neither establishes final-source shape fidelity. Strict import still stops at
  Sketch5's unsupported projected boundary. Its composite support-plane reference
  now resolves; the boundary recipe contains additional, unsupported contributors.
- HardEdgeBasin open plus display: **2.905 seconds**, 14 bodies, 14,898 triangles.
  The 20-second deadline, expected five feature errors, bounds, source preservation
  and warm-cache reuse checks pass. This overlapped corpus tests and is not an
  isolated performance comparison.
- The four new original projected-cap histories pass individual replay, analytic
  volume and centroid, source edits, suppression, undo and saved-history checks.
  Bottom examples use radial/axial chamfer setbacks of 3/2 mm, top examples 2/3 mm.
  Their previously incorrect bottom expectations were corrected from the analytic
  formulas. The full 161-history suite, plus the legacy offset case, passes after that
  correction (135.32 s, on `e8c340c5`).
- Projection-creation flags, circular cap selector/tracker consistency and cut-cap
  provenance tests pass. The native Style 5 Sketch4 checkpoint preserves its
  history and volume on reopening (10.47 s on the first rebase).
- All seven expression tests pass on the final upstream revision, including
  decimal-comma input, function arguments and hostile-input coverage.
- UI all-target compilation, formatting, whitespace and asset attribution checks
  pass. A new projected-cap example was rendered and visually checked: the shoulder,
  extension and separate top extrusion are present. Full workspace CI has not
  passed and was not rerun for this recheck.

## First complete-history failures

| File | First failure |
|---|---|
| Modular Clamp | Unsupported timeline feature 621, class `584D9526` v2. |
| Style 2 | Fillet 3963: unsupported history flags at byte 38. |
| Styles 3 and 6 | Compound projected body selection: unsupported edge/face term count. |
| Style 4 and style-1 | Unsupported compound fillet edge identity. |
| Style 5 | Unsupported composite fillet edge term. Sketch5 and Extrude7 now reconstruct; recovery retains nine features. |
| Style 7 | Duplicate fillet face label / unsupported body-edge ownership. |
| Gridfinity | Chamfer 16766: missing object reference at byte 40. Recovery yields six bodies with open-shell warnings, without editable history. |
| R36S Grip | Unsupported timeline feature 1160, class `D7015B81` v4. |
| adapter-x20-makita | Fillet 18835: missing object reference at byte 27. |

No PR was opened: the requested all-files success condition is not met.
Pending feature work remains uncommitted in the isolated workspace.

## Follow-up target and cap checks (before solved-record recovery)

The latest remote check still reports `2bfec256`. Explicit extrusion target
references now select the intended body even when several independent bodies are
active. Coincident circular cuts retain the tool cap's provenance, allowing
subsequent face attachments to resolve in the original SolidCraft fixtures.

- I/O unit suite: 269 passed, 28 ignored. Target ownership, label substitution,
  consumed bodies, forward references and truncated records are covered.
- Targeted kernel tests: five passed, including coincident cut caps on top and
  bottom in three axis orientations and overlap witnesses that reject holes,
  tangency and disjoint regions.
- All 163 saved-history recipes, plus the legacy offset compatibility case, pass
  edit, undo and reopening checks (136.16 s). The two new plain-cut projection
  examples include analytic volume, centroid and bounds expectations.
- HardEdgeBasin open plus display: 2.101 s, 14 bodies, 14,898 triangles; the
  existing performance and preservation assertions pass.
- Native cap-projection controls: **132/132 pass** after the follow-up cut-end
  resolution fix (107.65 s), including parameter edits and container reopening.
  The 20 plain-cut failures came from widened coplanar tools losing their exact
  sketch-boundary certificate. Import now resolves the explicit Boolean-history
  cap role, verifies the nominal end plane and orientation, and retains a live
  history anchor. Stale geometry, foreign targets and ambiguous faces are refused.
- Supplied corpus: still 6/17 complete editable imports and 34/34 recovery
  open/save/reopen checks, with geometry in 16/17 files. Gridfinity still opens
  empty. The corpus command exits 101 because strict acceptance fails (180.12 s).
- Clippy with warnings denied passes for I/O, kernel and engine, all targets.
  Full workspace CI remains unverified; no new commit or PR was created.

Logs: `/private/tmp/f3d-target-cap-history-performance.log`,
`/private/tmp/f3d-target-cap-corpus.log`,
`/private/tmp/f3d-explicit-target-and-cap-native.log`,
`/private/tmp/f3d-explicit-target-io-suite.log`,
`/private/tmp/f3d-coincident-cap-tests.log`, and
`/private/tmp/f3d-target-cap-clippy.log`.

The cut-end follow-up also passes the I/O unit suite (269 passed, 28 ignored,
7.15 s). Its original regression covers both cap ends, source edits, reopening,
stale pre-edit geometry and wrong targets. Native control evidence is in
`/private/tmp/f3d-cut-end-native-controls.log`; unit evidence is in
`/private/tmp/f3d-cut-end-io-suite.log`. I/O Clippy with warnings denied also
passes (`/private/tmp/f3d-cut-end-clippy.log`). The remote still reports
`2bfec256` after these checks.

The final cut-end corpus recheck confirms 6/17 complete editable imports and
34/34 recovery open/save/reopen cases, with geometry in 32 of those 34 cases
(Gridfinity is empty in both modes). It exits 101 solely for strict acceptance,
after 187.81 s: `/private/tmp/f3d-cut-end-corpus.log`.

Before the curved-wall fix, Style 5's Extrude6 checkpoint was decoded separately.
Features through Sketch4 evaluated successfully; Extrude6 failed its Union. Its selected profile is
the portion of a narrow rectangle outside the projected circle, extruded down
along the existing cylindrical wall. This isolates a coincident curved-wall
Boolean failure after decoding, distinct from Sketch5's unsupported composite
face selector. The local diagnostic is `/private/tmp/f3d-style5-union-document.log`.

## Bounded solved-record recovery

The latest remote check still reports `2bfec256`; it is an ancestor of this
branch, so no additional rebase was needed. Gridfinity's former recovery failure
was a software work budget while binding complete historical topology: a
4,000,000,000-unit limit refused a 24,036,730,880-unit request. These are decoder
work units, not a measurement of available RAM or hardware capacity.

Native-body recovery now retries that specific history budget refusal once,
using an in-memory archive that omits only token-framed historical BREP tails.
The public cadmpeg ASM framing API locates the solved/history boundary; embedded
strings cannot masquerade as delimiters. Solved records, body maps and transforms
are retained byte-for-byte. The original file is untouched. The retry keeps the
same decoding budgets, and malformed data, allocation failures and unrelated
resource limits do not trigger it. This fallback does not restore editable history.

All 17 supplied archives now recover geometry in both modes and preserve the
recovered document and measured mesh volume across native save/reopen. Gridfinity
specifically retains six bodies and explicit open-shell warnings. Its rendered
assembly contains the shelves and side walls, but open shells make volume and
mass unreliable; the test is not evidence of watertight solids or source fidelity.

Validation: 273 I/O unit tests pass (28 opt-in tests ignored), including four new
framing, resource-bound and fallback-selection regressions. I/O and CLI all-target
Clippy pass with warnings denied. The 34 corpus recovery checks pass in 174.30 s.
The strict rerun confirms 6/17 complete editable imports, with the same first
failures listed above (10.78 s, expected failing acceptance gate). Full workspace CI is still unverified and
no PR has been opened.

Logs: `/private/tmp/f3d-solved-archive-unit.log`,
`/private/tmp/f3d-solved-archive-io-suite.log`,
`/private/tmp/f3d-solved-archive-clippy.log`, and
`/private/tmp/f3d-solved-archive-corpus.log`, and
`/private/tmp/f3d-solved-archive-strict-corpus.log`.

## Partial cylindrical-wall join

Style 5's decoded Extrude6 now evaluates. The coplanar tool-growth path previously
left a concave circular edge unchanged when its endpoints met straight edges at
non-tangent angles. A join can now shrink that arc into existing material and
recompute its endpoints from exact line–circle intersections. Near-tangent,
ambiguous, distant and invalid intersections keep the ordinary Boolean path.
The arc remains circular and its trimmed angular interval follows the new corners.

The original `partial-cylinder-wall-join` saved design adds a narrow wing to a
cylindrical neck above a beveled base. Its recipe uses analytic circular-segment
area times depth, plus cylinder/frustum volume, and checks depth edits and
suppression. The rendered wing meets the neck while leaving its lower part and
the beveled base intact. The suite now contains 164 original recipes.

The real Style 5 recovery retains seven features, one body and zero regeneration
errors. It still omits Sketch5 and subsequent unsupported features; this is not
a complete import. The next failure is a composite cap identity with multiple
native face contributors. Logs: `/private/tmp/f3d-style5-concave-eval.log`,
`/private/tmp/f3d-style5-partial-wall-recovery.log`, and
`/private/tmp/f3d-partial-wall-history.log`.

Validation passes all 164 saved-history recipes plus the legacy offset case
(145.92 s), including replay, volume, source edits, suppression, undo and reopening.
The document unit suite passes 244 tests (one ignored), including three new
corner/partial-wall regressions. Document, I/O and CLI all-target Clippy pass
with warnings denied; asset attribution and whitespace checks pass. Upstream
remains `2bfec256`. Full workspace CI remains unverified; no new commit or PR.

## Composite cap attachment

Unbounded cap recipes retain each operation's logical start/end role. Style 5
names the neck's end cap and the downward wing's start cap. Import resolves every
producer through exact Boolean-history cap aliases and checks the nominal plane
and normal; all contributors must name the same unique current face. The saved
attachment follows that resulting face through later edits, including its current
boundary, rather than retaining a fixed imported plane. Unsupported extents,
foreign targets, suppressed producers, wrong cap ends and ambiguous faces fail.

The original `partial-cylinder-cap-pocket` recipe adds a radius-2 mm,
depth-0.25 mm pocket to the merged cap: analytic removed volume π mm³. Depth and
base-height edits, wing and pocket suppression, undo and native reopening pass.
The pocket remains attached to the neck if the wing is suppressed. The rendered
pocket and partial-height wing were visually checked. There are now 165 original
saved-history recipes.

Before the latest rebase, 275 I/O tests passed (28 opt-in tests ignored), including
two new composite grammar/attachment tests. I/O and CLI all-target Clippy, asset
attribution and whitespace checks passed. The supplied corpus still had 6/17
complete imports and 34/34 recovery checks with geometry in every file (217.87 s).
Style 5 retains seven features; Sketch5 now fails at its projected outline rather
than the support plane. Its bounded outline record also names operations owned by
later fillets, so it must not be interpreted as an earlier plain cap boundary.

Logs: `/private/tmp/f3d-composite-cap-tests.log`,
`/private/tmp/f3d-composite-cap-history.log`,
`/private/tmp/f3d-composite-cap-io-suite.log`, and
`/private/tmp/f3d-composite-cap-corpus.log`.

## Local evidence

Latest rebase validation on `e382a773`: 275 I/O tests pass (28 ignored), I/O and
CLI all-target Clippy pass, UI all-target compilation passes, and the new saved
cap-pocket recipe passes all edit/reopen/undo checks (9.04 s). HardEdgeBasin passes
its opening gate at 2.905 s. The corpus confirms 6/17 complete editable imports
and 34/34 recovery checks with geometry in all 17 files (188.82 s). Full workspace
CI remains unverified; no PR was opened.

- `/private/tmp/f3d-e382-io-suite.log`
- `/private/tmp/f3d-e382-clippy.log`
- `/private/tmp/f3d-e382-ui.log`
- `/private/tmp/f3d-e382-pocket.log`
- `/private/tmp/f3d-e382-performance.log`
- `/private/tmp/f3d-e382-corpus.log`

- `/private/tmp/f3d-rebase-2bfe-corpus.log`
- `/private/tmp/f3d-rebase-2bfe-performance.log`
- `/private/tmp/f3d-rebase-2bfe-expr.log`

- `/private/tmp/f3d-rebase-e8c3-corpus.log`
- `/private/tmp/f3d-rebase-e8c3-performance.log`
- `/private/tmp/f3d-rebase-e8c3-history.log`
- `/private/tmp/f3d-rebase-e8c3-fixtures.log`
- `/private/tmp/f3d-rebase-e8c3-ui-check.log`
- `/private/tmp/f3d-rebase-3fa8-targeted.log`
- `/private/tmp/f3d-rebase-3fa8-checkpoint.log`
- `/Users/chess/projects/cad/repro/f3d/projected-face-extrude-chamfer-top-solidcraft.png`

Run the corpus with `SOLIDCRAFT_F3D_CORPUS` pointing to `test_files` and
`SOLIDCRAFT_F3D_ADDITIONAL_CORPUS` pointing to `repro/f3d/additional-corpus`:

```sh
cargo test --profile ci -p solidcraft-cli --test f3d_corpus -- --ignored --nocapture --test-threads=1
cargo test --profile ci -p solidcraft-cli --test open_performance -- --nocapture
```
