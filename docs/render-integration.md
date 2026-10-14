# Phased render and local-library integration

Tracked in #842; the parent render design remains #620. Contributions retain their
original ancestry and are validated together on a pinned upstream revision. The
integration PR is a reviewable combined tree; it does not assert that any constituent
PR has merged. Maintainers can choose the combined or individual merge path.

The refreshed stack incorporates upstream `b5efde94`, including the merged physical
card validator (#934), shared STEP text/import progress and GPU buffer chunking (#925).
Physical cards are candidate engineering records, independent of assigned mass
density and rendered looks; format 22 persists them. Upstream owns document
format 19 for editable associative offset dimensions. Saved render setups were added
in format 20; format 21 adds opt-in opaque optical parameters without reinterpreting
legacy scalar looks. The Design viewport retains upstream GPU
tessellation; the separate product scene continues to upload its prepared CPU meshes.
The already-merged HDR input contribution is retained through its original ancestry.

| Phase | Existing contribution / deliverable | Completion gate |
|---|---|---|
| 1 | R2 #667, R3 #706, R4a #800: saved setups, integrated product workspace and bounded CPU PNG jobs | Setup save/reopen/undo, document isolation, strict capabilities, cancellation, native exports and full repository gate |
| 2 | HDR input #837: cooperative Radiance preparation alongside the existing job host | Codec/host tests together, canvas format guard, unchanged CPU output/capabilities, native and wasm gates |
| 3 | First local scalar library #846, building on #785/#786 and proposal #792 | Portable catalog round trips, bounded hostile input, explicit revisions/conflicts, copied definitions, undo/cold reopen and actual UI evidence |
| 4 | Opaque PBR consumer #1004: saved optical parameters, existing assignment/editor, integrated GPU and CPU image paths | Atomic admission, unchanged mass/geometry, undo/copy/reopen, GPU/CPU color parity and original native evidence |
| 5 | Engineering-card persistence #1048 and portable library v2 #1062 | Typed file admission, atomic edits, v1 bytes/revisions, no lossy downgrade, source-independent reopen and actual library UI evidence |
| Follow-up | Texture sets, wood grain/mapping and HDRI (#813) | Actual lighting/texture interpretation, resource admission and separate native/browser visual evidence |

## Current capability boundary

The GPU host provides the product **viewport**. The CPU product executor provides
bounded preview **PNG images**, with opt-in opaque PBR and approximate sorted
transparency for legacy transparent looks. The [fixed studio consumer](render-opaque-pbr.md)
has no HDRI, transmission or cast shadows. The Radiance
utility prepares linear RGB values in their source colour space; it is not a lighting
implementation. Adding its codec never adds rendering capabilities; the opaque
consumer independently installs PBR in the actual product executors. HDRI/refraction/
final/EXR remain unsupported. Refused image requests preserve the last valid
image and CAD state. See [backend contracts](render-backends.md),
[settings](render-settings.md), [workspace](render-workspace.md),
[jobs](render-jobs.md) and [HDR input](render-hdr-input.md).

## Local-library first slice

Reuse existing `Look`, design-local physical materials and assignment commands.
Physical density, visual colour/opacity and render environments remain distinct.
The first v1 catalog handles scalar appearance and density/look definitions only.
V2 retains existing PBR parameters and engineering cards; neither contains texture
files or environment descriptors. Selected values
are copied into the existing document representation so reopening does not need the
catalog. Catalog provenance/revisions remain in the catalog, while engineering-card
sources/conditions are copied with their card. This adds
no persistent catalog-update link or new document-format version. It does not claim
the complete resource/provenance embedding proposed in #792.

Catalog inspection/export and application must use engine commands. Programmatic
calls never open a picker. An exact selected revision is required for application;
changed files and conflicting material names cannot silently alter a selection.
Applying an appearance must not change density/mass. Physical-material definitions
reuse existing creation/assignment behavior and have explicit conflict handling.
The source design and library draft/file are separate: exporting a reusable snapshot
does not edit the CAD design, and applying a definition is one undoable design edit.

## Validation and publishing

Run `cargo xtask ci` before each integration/code arc is committed. Record actual
counts, platform, pinned upstream and any failure in the PR, separately from remote
checks. The earlier Windows geometry failures #796/#878 were resolved by the now-merged
#906; render work neither suppressed them nor weakened their baselines. Historical
interrupted test binaries remain incomplete evidence, not passes. Current local CI
still stops at the independently tracked Windows MCP lint #907. The PR records
completed focused checks and actual remote results separately; no full local green
gate is inferred from them.
Native compilation, wasm compilation and actual browser interaction are separate
claims. Use original small examples and actual app/PNG evidence for visible changes;
do not commit generated review media or commercial reference assets.

Private `plan/` documents are not present in this checkout. The public contracts and
current repository instructions govern the available work; no missing contents are
invented. Renderer/resource representations are proposed in reviewable phases in #813, open to
maintainer and contributor feedback. No unpublished descriptor is assumed.
