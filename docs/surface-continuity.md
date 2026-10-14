# Sampled surface continuity analysis

`inspect.continuity` evaluates seams between two distinct faces of a B-rep body in
model space. It changes view state only, not the design or undo history. Desktop
and headless snapshots show the same diagnostic edge polylines. These are on-top
inspection overlays: hidden edges can be shown too.

```json
{"command":"inspect.continuity","params":{"body":"Box","samples":33}}
```

Omit `body` to analyze visible world-space bodies, including their occurrence
placements. Select edges with `edges: [{"index": 0}, [5, 0, 0]]` on one body;
indices follow `model.edges`. Point picks must be within 0.01 mm of an edge
(the picking polyline uses 0.001 mm tessellation). Duplicate picks are evaluated
once. An explicitly empty edge list is an error, not a request for all edges.

## Measurements

Each edge returns its incident face indices, status, successful/failed station
counts, and the maximum of each metric with its world-space position and
normalized edge parameter:

- `gap_mm`: maximum of the separation between the two projected surface points
  and each surface point's distance from the B-rep edge sample. This also exposes
  an edge that does not lie on its supporting surfaces.
- `normal_angle_deg`: angle between oriented unit face normals (0–180 degrees).
  Opposite normals are not silently treated as equal.
- `curvature_jump_per_mm`: Frobenius norm of the difference between the signed
  second fundamental forms lifted into world coordinates. Comparing curvature
  directions as well as magnitudes avoids mistaking equal principal curvature
  values with different principal directions for a match. The metric is useful
  as a G2 diagnostic only after the G0 and G1 checks pass.
- `sampled_continuity`: highest passing sampled level, 0, 1, or 2; `null` if G0
  fails or evaluation is incomplete. No G3/G4 classification is provided.

For independent implementation, let the surface Jacobian be `J = [Su Sv]`,
its metric `G = JᵀJ`, and its second fundamental form in UV coordinates `H`,
formed by dotting second derivatives with the oriented unit normal. The
world-space symmetric tensor is `K = J G⁻¹ H G⁻¹ Jᵀ`. The reported curvature
metric is `||Ka - Kb||F`. A regular reparameterization changes the UV quantities
but not this tensor; uniform model scaling changes its units as expected.
This calculation is derived from differential geometry, not another CAD's
implementation. No commercial-CAD-produced geometry or measurements are used
in the regression tests.

Defaults are `gap_tolerance_mm: 0.00001`, `angle_tolerance_deg: 0.01`, and
`curvature_tolerance_per_mm: 0.00001`. Tolerances must be finite and non-negative;
the angle must not exceed 180 degrees. These are SolidCraft defaults, not
copied construction presets or a universal Class-A acceptance standard.

## Coverage and limits

Sampling uses 3–257 uniformly spaced **edge parameter** stations, including
endpoints (default 33). It is not uniform arc length. A passing result describes
those stations, **not a proof over the entire edge**, nor a guarantee of good
reflection flow over the surfaces' interiors. Use zebra/environment analysis
alongside it. Increase sampling for investigation, but do not treat denser
sampling as a mathematical certificate.

A singular surface chart, failed projection, invalid derivative, or non-finite
metric produces `incomplete`, with no passing classification. Open boundaries
and periodic seams belonging to only one distinct face have status `boundary`;
more than two incident faces yield `non_manifold`. These are not falsely marked
G2. Mesh bodies are unsupported. A request is capped at 100 bodies, 1,000 picks,
10,000 faces/edges and 40,000 face-edge uses per body, and 100,000 total stations.

Colors: green = sampled G2, amber = sampled G1, red = sampled G0 or failure of
G0, gray = incomplete evaluation. Boundaries without a two-face evaluation are
reported in JSON but are not drawn as diagnostic polylines. Existing diagnostics
are suppressed when their body's geometry/placement changes or it is hidden;
run the analysis again to inspect the new geometry. The toolbar command toggles
the analysis; scripts can clear it with `{"clear": true}`.

All results are published only after the complete request succeeds. Invalid
input leaves existing diagnostics and the document unchanged. Curve combs,
surface isocurves, independent edge-pair gap analysis, and G3/G4 diagnostics
remain separate follow-up work for issue #770.
